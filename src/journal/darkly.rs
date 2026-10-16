pub use darkly::engine::DarklyEngine;
use darkly::engine::{types::StrokeOp, ExportImageResult};
use darkly::layer::LayerId;
use darkly::gpu::context::GpuContext;
use darkly::coord::CanvasRect;
use darkly::config::ConfigValue;
use image::{ImageBuffer, Rgba};
use parry2d::math::Vec2;
use rayon::prelude::*;
use crate::journal::paths::{CurveSample};
use crate::journal::canvas::{Canvas, ColoredStroke, StrokeColor};
use crate::journal::compose::{self, Placed};
use crate::journal::gradient::{colorize, Along, GradientKind};
use std::collections::HashSet;

#[cfg(not(target_arch = "wasm32"))]
pub fn new_engine(width: u32, height: u32) -> DarklyEngine {
    block_on(new_engine_async(width, height)).unwrap_or_else(|e| panic!("{}", e))
}

/// A headless engine on the first GPU adapter (WebGPU in a browser), or
/// why there isn't one.
pub async fn new_engine_async(width: u32, height: u32) -> Result<DarklyEngine, String> {
    // Stroke prediction extrapolates and re-renders a tail ahead of the pen
    // to hide input latency; offline it only costs time. (Config is
    // thread-local, so this applies to engines used on this thread.)
    darkly::config::set("input.predictionHorizon", ConfigValue::Float(0.0));
    let instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
    let instance = wgpu::Instance::new(instance_desc);
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await
        .map_err(|e| format!("Request for adapter failed: {}", e))?;
    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default()).await
        .map_err(|e| format!("Request for device failed: {}", e))?;
    #[cfg(target_arch = "wasm32")]
    browser::watch(&device, &queue);
    let gpu = GpuContext::new_headless(device, queue);
    let mut engine = DarklyEngine::new(gpu, width, height);
    engine.add_raster_layer(None);
    Ok(engine)
}

/// Run a render to the end on this thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    smol::block_on(future)
}

/// Give the GPU time to finish work the engine is waiting on. Natively
/// that's a short sleep; in a browser, readbacks only complete once control
/// goes back to its event loop, so it waits for a timer there.
async fn wait_for_gpu() {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::sleep(std::time::Duration::from_millis(2));
    #[cfg(target_arch = "wasm32")]
    browser_pause().await;
}

/// Draw a frame, so the engine finishes the bookkeeping of the strokes
/// drawn so far (see `Pen::draw`). In a browser, also wait for the GPU to
/// finish the work sent so far and the readbacks it makes to land: there
/// nothing else holds back how far the strokes run ahead of the GPU, and
/// the GPU memory the work and readbacks hold grows until the browser's GPU
/// process gives out. Fails if the GPU has (see `gpu_failure`).
async fn catch_up(engine: &mut DarklyEngine) -> Result<(), String> {
    engine.render(0.0);
    #[cfg(target_arch = "wasm32")]
    {
        browser::work_done().await?;
        browser_pause().await;
        engine.render(0.0);
    }
    gpu_failure()
}

/// Why the GPU can't go on, if it can't: in a browser, if it's been lost
/// or has reported an error. Natively those panic instead, as wgpu does by
/// default.
fn gpu_failure() -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    if let Some(failure) = browser::failure() {
        return Err(failure);
    }
    Ok(())
}

/// The browser's GPU: whether it's failed, and waiting for it.
#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    struct Watch {
        queue: wgpu::Queue,
        failure: Arc<Mutex<Option<String>>>,
    }

    thread_local! {
        /// The GPU of the engine last made on this thread (one per worker).
        static GPU: RefCell<Option<Watch>> = const { RefCell::new(None) };
    }

    /// Keep note of `device` failing, rather than panicking (wgpu's default
    /// for errors), so a render can stop and say why.
    pub(super) fn watch(device: &wgpu::Device, queue: &wgpu::Queue) {
        let failure: Arc<Mutex<Option<String>>> = Arc::default();
        let lost = failure.clone();
        device.set_device_lost_callback(move |reason, message| {
            lost.lock().unwrap().get_or_insert_with(|| format!("the GPU was lost ({:?}): {}", reason, message));
        });
        let errored = failure.clone();
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            errored.lock().unwrap().get_or_insert_with(|| format!("the GPU reported an error: {}", error));
        }));
        GPU.set(Some(Watch { queue: queue.clone(), failure }));
    }

    pub(super) fn failure() -> Option<String> {
        GPU.with_borrow(|gpu| gpu.as_ref().and_then(|gpu| gpu.failure.lock().unwrap().clone()))
    }

    /// Wait until the GPU has done the work sent to it so far.
    pub(super) async fn work_done() -> Result<(), String> {
        let done = Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        let watched = GPU.with_borrow(|gpu| gpu.as_ref().map(|gpu| gpu.queue.on_submitted_work_done(move || flag.store(true, Ordering::Release))));
        if watched.is_none() {
            return Ok(());
        }
        while !done.load(Ordering::Acquire) {
            if let Some(failure) = failure() {
                return Err(failure);
            }
            super::browser_pause().await;
        }
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
async fn browser_pause() {
    use wasm_bindgen::{JsCast, JsValue};
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let global = js_sys::global();
        let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
            .ok()
            .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
        match set_timeout {
            Some(set_timeout) => { let _ = set_timeout.call2(&global, &resolve, &JsValue::from(0)); }
            None => { let _ = resolve.call0(&JsValue::NULL); }
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Resize the engine's canvas window so a single engine can be reused across
/// canvases of different sizes. Panics if Darkly rejects the size (zero or
/// over its max dimension), rather than silently rendering at the old size.
pub fn resize_canvas(engine: &mut DarklyEngine, width: u32, height: u32) {
    engine.resize_canvas(CanvasRect::from_xywh(0, 0, width, height));
    let actual = engine.canvas_dimensions();
    assert_eq!(actual, (width, height), "Darkly rejected canvas resize to {}x{}", width, height);
}

/// Distance between points fed to Darkly, in output pixels. Darkly's
/// stabilizer needs dense input in pixel space: sparser spacing breaks up
/// strokes, while denser spacing only costs time. Measuring in pixels keeps
/// the work proportional to the output size, so smaller renders are faster.
const SAMPLE_SPACING_PX: f32 = 1.0;
/// Simulated pen speed used to timestamp points (250 px/s, i.e. one sample
/// every 4ms at 1px spacing, roughly a 250Hz tablet). Darkly's stabilizer
/// reads `time_ms`, so feeding all zeros gives it no cadence to work with.
const PEN_SPEED_PX_PER_MS: f64 = 0.25;

/// How much of `render_async`'s progress is drawing strokes.
const STROKES_SHARE: f32 = 0.85;

/// How many strokes to draw between frame ticks (see `catch_up`). In a
/// browser, where each tick waits for the GPU, fewer, to keep what's
/// waiting on it small.
#[cfg(not(target_arch = "wasm32"))]
const FRAME_EVERY_STROKES: u32 = 25;
#[cfg(target_arch = "wasm32")]
const FRAME_EVERY_STROKES: u32 = 10;

/// Output pixels rendered past a tile's edges when supersampling, for the
/// filter back down to read: Lanczos-3 reaches 3 output pixels.
const BLEED_PX: i64 = 4;

/// Render a canvas with Darkly at `scale` pixels per canvas unit (brushes
/// included: their sizes are in units too).
///
/// The image covers the canvas's bounds, or for a tile of a page (see
/// `Frame`) its share of the page's pixels: tiles render on the grid of a
/// render of the whole page, strokes reaching in from outside included, so
/// they fit together into exactly that render.
///
/// With `supersample` > 1 the canvas is rendered at that multiple of the
/// size (brushes included) and filtered back down, which smooths the
/// aliasing Darkly produces for brush marks thinner than about a pixel.
///
/// Each stroke is drawn with its own brush; panics if a stroke has none.
#[cfg(not(target_arch = "wasm32"))]
pub fn render(canvas: &Canvas, engine: &mut DarklyEngine, scale: f32, supersample: f32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    block_on(render_async(canvas, engine, scale, supersample, &mut |_| {}))
        .unwrap_or_else(|e| panic!("{}", e))
}

/// `render`, giving the GPU time as it goes (see `wait_for_gpu`), with
/// `progress` called with how far through it is, from 0 to 1: drawing the
/// strokes takes it to `STROKES_SHARE`, reading back and filtering the rest.
/// Fails if the GPU does (see `gpu_failure`).
pub async fn render_async(canvas: &Canvas, engine: &mut DarklyEngine, scale: f32, supersample: f32, progress: &mut dyn FnMut(f32)) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, String> {
    // The page this canvas is (a tile of), as its own render would size it.
    let (page_origin, extents, tile) = match canvas.frame() {
        Some(frame) => (frame.page.mins, frame.page_extents(), Some(frame)),
        None => (canvas.bounds().mins, canvas.render_extents(), None),
    };
    let size = (((extents.x * scale) as u32).max(1), ((extents.y * scale) as u32).max(1));
    // Fractional factors are fine: the render rounds to whole pixels, and
    // the filter scales back down by any ratio.
    let k = supersample.max(1.0);
    let render_size = ((size.0 as f32 * k).round() as u32, (size.1 as f32 * k).round() as u32);
    let k = (render_size.0 as f64 / size.0 as f64, render_size.1 as f64 / size.1 as f64);
    // Render pixels per canvas unit.
    let ratio = Vec2::new(render_size.0 as f32 / extents.x, render_size.1 as f32 / extents.y);
    let [x0, y0, x1, y1] = tile.map_or([0, 0, size.0, size.1], |frame| frame.pixels(size)).map(|v| v as i64);
    let supersampled = k != (1.0, 1.0);
    let bleed = if supersampled { BLEED_PX } else { 0 };
    // The render pixels covering the tile and its bleed.
    let rx0 = ((x0 - bleed) as f64 * k.0).floor() as i64;
    let ry0 = ((y0 - bleed) as f64 * k.1).floor() as i64;
    let rx1 = ((x1 + bleed) as f64 * k.0).ceil() as i64;
    let ry1 = ((y1 + bleed) as f64 * k.1).ceil() as i64;
    resize_canvas(engine, (rx1 - rx0) as u32, (ry1 - ry0) as u32);
    let rendered = render_canvas(canvas, engine, page_origin, (rx0 as f32, ry0 as f32), ratio, scale * k.0.max(k.1) as f32, progress).await?;
    let rendered = if supersampled {
        downsample(&rendered, (rx0, ry0), k, (x0, y0), ((x1 - x0) as u32, (y1 - y0) as u32))
    } else {
        rendered
    };
    progress(1.0);
    Ok(rendered)
}

/// The Lanczos-3 kernel.
fn lanczos3(t: f64) -> f64 {
    if t == 0.0 {
        1.0
    } else if t.abs() < 3.0 {
        let pt = std::f64::consts::PI * t;
        3.0 * pt.sin() * (pt / 3.0).sin() / (pt * pt)
    } else {
        0.0
    }
}

/// For each of `count` output pixels from `out_origin`, the first source
/// pixel its filter reads and the weights. Output pixel `x` covers source
/// pixels `x * k` to `(x + 1) * k`, both counted from the page's corner;
/// the source starts at `src_origin`. Pixels past the source are
/// transparent, but still count in the weights' total.
fn filter_weights(out_origin: i64, count: u32, k: f64, src_origin: i64) -> Vec<(i64, Vec<f32>)> {
    (0..count as i64).map(|j| {
        let center = (out_origin + j) as f64 * k + k / 2.0 - src_origin as f64;
        let (first, last) = ((center - 3.0 * k).floor() as i64, (center + 3.0 * k).ceil() as i64);
        let weights: Vec<f64> = (first..=last).map(|i| lanczos3((i as f64 + 0.5 - center) / k)).collect();
        let total: f64 = weights.iter().sum();
        (first, weights.into_iter().map(|w| (w / total) as f32).collect())
    }).collect()
}

/// Filter a supersampled render down to output pixels with Lanczos-3, in
/// premultiplied alpha so the transparent background's color doesn't bleed
/// into stroke edges. `src` holds the render pixels from `src_origin` on a
/// grid `k` times as fine as the output's; the result is the output pixels
/// from `out_origin`, `out_size` of them. Both are counted from the page's
/// corner, so tiles of a page filter exactly as the whole page would.
fn downsample(src: &ImageBuffer<Rgba<u8>, Vec<u8>>, src_origin: (i64, i64), k: (f64, f64), out_origin: (i64, i64), out_size: (u32, u32)) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let (sw, sh) = (src.width() as i64, src.height() as i64);
    let premultiplied: Vec<[f32; 4]> = src.pixels().map(|p| {
        let [r, g, b, a] = p.0.map(|v| v as f32 / 255.0);
        [r * a, g * a, b * a, a]
    }).collect();
    let columns = filter_weights(out_origin.0, out_size.0, k.0, src_origin.0);
    let rows = filter_weights(out_origin.1, out_size.1, k.1, src_origin.1);
    // Across first, every source row to output columns...
    let across: Vec<[f32; 4]> = (0..sh).into_par_iter().flat_map_iter(|y| {
        let row = &premultiplied[(y * sw) as usize..((y + 1) * sw) as usize];
        columns.iter().map(move |(first, weights)| {
            let mut sum = [0.0f32; 4];
            for (i, w) in weights.iter().enumerate() {
                let x = first + i as i64;
                if (0..sw).contains(&x) {
                    let p = row[x as usize];
                    for c in 0..4 {
                        sum[c] += p[c] * w;
                    }
                }
            }
            sum
        })
    }).collect();
    // ...then down.
    let width = out_size.0 as usize;
    let pixels: Vec<u8> = rows.par_iter().flat_map_iter(|(first, weights)| {
        let across = &across;
        (0..width).flat_map(move |x| {
            let mut sum = [0.0f32; 4];
            for (i, w) in weights.iter().enumerate() {
                let y = first + i as i64;
                if (0..sh).contains(&y) {
                    let p = across[y as usize * width + x];
                    for c in 0..4 {
                        sum[c] += p[c] * w;
                    }
                }
            }
            // Lanczos overshoots slightly; clamp alpha, and color to at most alpha.
            let a = sum[3].clamp(0.0, 1.0);
            let unpremultiply = |c: f32| if a > 0.0 { (c.clamp(0.0, a) / a * 255.0).round() as u8 } else { 0 };
            [unpremultiply(sum[0]), unpremultiply(sum[1]), unpremultiply(sum[2]), (a * 255.0).round() as u8]
        })
    }).collect();
    ImageBuffer::from_raw(out_size.0, out_size.1, pixels).expect("one pixel per output position")
}

/// Render a canvas into the engine's current canvas size, which shows the
/// pixels from `offset` on of a render of the page at `page_origin`, with
/// `ratio` pixels per unit across and down. Strokes that can't reach it are
/// skipped.
///
/// Darkly draws a stroke in one colour, so strokes with a gradient are drawn
/// in white on a layer of their own (with any strokes next to them sharing
/// the gradient), then coloured pixel by pixel and laid over what's below.
/// Brushes that move ink (Smudge, Blur) don't reach across those layers.
async fn render_canvas(canvas: &Canvas, engine: &mut DarklyEngine, page_origin: Vec2, offset: (f32, f32), ratio: Vec2, brush_scale: f32, progress: &mut dyn FnMut(f32)) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, String> {
    let (engine_canvas_width, engine_canvas_height) = engine.canvas_dimensions();
    let origin = page_origin + Vec2::new(offset.0 / ratio.x, offset.1 / ratio.y);
    let view_max = origin + Vec2::new(engine_canvas_width as f32 / ratio.x, engine_canvas_height as f32 / ratio.y);
    let strokes: Vec<(&ColoredStroke, u32)> = canvas.strokes().iter().zip(canvas.stroke_seeds())
        .filter(|(stroke, _)| Canvas::stroke_reaches(stroke, origin, view_max, 0.0))
        .collect();
    let mut pen = Pen { engine, page_origin, offset, ratio, brush_scale, loaded_name: None, loaded_size: None, drawn: 0, warned_colors: HashSet::new(), warned_saturation: HashSet::new() };
    let mut rendered: Option<Placed> = None;
    let mut done = 0;
    for pass in strokes.chunk_by(|a, b| a.0.style.color.gradient() == b.0.style.color.gradient()) {
        let gradient = pass[0].0.style.color.gradient();
        let layer = pen.engine.add_raster_layer(None);
        let mut along = Vec::new();
        let mut reach: f32 = 0.0;
        for &(stroke, seed) in pass {
            let samples = pen.draw(layer, stroke, seed, gradient.is_some()).await?;
            done += 1;
            progress(STROKES_SHARE * done as f32 / strokes.len() as f32);
            if let Some(g) = gradient.filter(|g| g.per_stroke()) {
                let vertical = g.kind == GradientKind::Vertical;
                along.extend(samples.into_iter().map(|(p, t, down)| (p, if vertical { down } else { t })));
                reach = reach.max(stroke.style.brush.as_ref().map_or(0.0, |b| b.size) * brush_scale);
            }
        }
        let mut image = export(pen.engine, layer).await?;
        if let Some(gradient) = gradient {
            let to_canvas = |x: f32, y: f32| page_origin + Vec2::new((x + offset.0) / ratio.x, (y + offset.1) / ratio.y);
            let along = gradient.per_stroke().then(|| Along::new(along, reach));
            colorize(&mut image, gradient, to_canvas, along.as_ref());
        }
        let image = Placed { image, at: (0, 0) };
        match &mut rendered {
            None => rendered = Some(image),
            Some(below) => compose::draw(below, &image, compose::over),
        }
    }
    match rendered {
        Some(rendered) => Ok(rendered.image),
        // Nothing reaches it: an empty render.
        None => {
            let layer = pen.engine.add_raster_layer(None);
            export(pen.engine, layer).await
        }
    }
}

/// Draws strokes into the engine, keeping track of the brush it has
/// loaded and what it's warned about.
struct Pen<'a> {
    engine: &'a mut DarklyEngine,
    page_origin: Vec2,
    offset: (f32, f32),
    ratio: Vec2,
    brush_scale: f32,
    /// Only reload the brush / reset its size when they change between
    /// strokes.
    loaded_name: Option<String>,
    loaded_size: Option<f32>,
    drawn: u32,
    warned_colors: HashSet<String>,
    warned_saturation: HashSet<(u32, String, u32)>,
}

impl Pen<'_> {
    /// Draw a stroke on `layer`, in white if `white` (to be coloured
    /// after), returning its points in render pixels with how far along
    /// its subpath each is and how far down the whole path, from 0 to 1.
    async fn draw(&mut self, layer: LayerId, stroke: &ColoredStroke, seed: u32, white: bool) -> Result<Vec<(Vec2, f32, f32)>, String> {
        let ColoredStroke { path, style, .. } = stroke;
        let brush = style.brush.as_ref()
            .expect("Darkly strokes need a brush (rhai stroke styles always have one)");
        if self.loaded_name.as_deref() != Some(brush.name.as_str()) {
            self.engine.brush_load(&brush.name)
                .unwrap_or_else(|e| panic!("Failed to load brush '{}': {}", brush.name, e));
            self.loaded_name = Some(brush.name.clone());
            self.loaded_size = None;
            // Stabilization smooths a hand's jitter, and these points are
            // already smooth curves. Darkly also tunes it to screen pixels,
            // which on a headless engine smooths glyph-sized strokes into
            // short straight lines.
            self.engine.brush_set_exposed_port("brush_settings", "stabilize", 0.0)
                .unwrap_or_else(|e| panic!("Unable to turn off stabilization for brush '{}': {}", brush.name, e));
        }
        let size = brush.size * self.brush_scale;
        if self.loaded_size != Some(size) {
            self.engine.brush_set_exposed_port("brush_settings", "size", size).expect("Unable to set brush size");
            self.loaded_size = Some(size);
        }
        let pressure_min = brush.min_pressure;
        let pressure_range = brush.max_pressure - brush.min_pressure;
        // A style's width scales its brush's pressure here (in the plain
        // output it's a geometric width). Darkly caps pressure at 1, so
        // past that, wider strokes stop getting heavier. Pooling and
        // variation can take them past it too.
        let peak_pressure = style.width * brush.max_pressure * brush.heaviest();
        if peak_pressure > 1.0 && self.warned_saturation.insert((style.width.to_bits(), brush.name.clone(), peak_pressure.to_bits())) {
            eprintln!(
                "warning: width {} strokes with brush '{}' (pressure {}-{}{}) reach pressure {:.2}; Darkly caps pressure at 1, so their heaviest parts look the same",
                style.width, brush.name, brush.min_pressure, brush.max_pressure,
                if brush.heaviest() > 1.0 { format!(", pooled and varied up to {:.2}x", brush.heaviest()) } else { String::new() },
                peak_pressure,
            );
        }
        let [cr, cg, cb, ca] = if white {
            [1.0; 4]
        } else {
            style.rgba().unwrap_or_else(|err| {
                if let StrokeColor::Solid(color) = &style.color
                    && self.warned_colors.insert(color.clone()) {
                    eprintln!("{}; drawing in black.", err);
                }
                [0.0, 0.0, 0.0, 1.0]
            })
        };
        let (w_ratio, h_ratio) = (self.ratio.x, self.ratio.y);
        let sample_spacing = SAMPLE_SPACING_PX / w_ratio.max(h_ratio);
        let substrokes: Vec<Vec<CurveSample>> = path.to_substrokes().iter().map(|s| s.get_point_samples(sample_spacing)).collect();
        // The whole path's top and bottom, which it pools toward and its
        // vertical gradient runs between.
        let (top, bottom) = substrokes.iter().flatten()
            .fold((f32::MAX, f32::MIN), |(top, bottom), s| (top.min(s.point.y), bottom.max(s.point.y)));
        let mut points = Vec::new();
        for (i, samples) in substrokes.into_iter().enumerate() {
            // Darkly seeds each stroke's random brush texture from the wall
            // clock by default; numbering the strokes instead makes renders
            // reproducible while keeping neighbouring strokes distinct. (A
            // tile's strokes keep their numbers from the whole page.)
            let stroke_seed = seed.wrapping_add(i as u32);
            self.engine.set_stroke_seed(Some(stroke_seed));
            self.drawn = self.drawn.wrapping_add(1);
            let variation = brush.variation(stroke_seed);
            // One Darkly stroke per substroke. With no frame drawn between
            // its points, Darkly draws the whole stroke at once when it ends.
            self.engine.begin_stroke(layer).expect("begin_stroke failed");
            let sample_count = samples.len();
            let mut distance = 0.0;
            let mut last = samples.first().map(|s| s.point);
            for (i, CurveSample { point,  .. }) in samples.into_iter().enumerate() {
                distance += last.map_or(0.0, |l| l.distance(point));
                last = Some(point);
                let t_param = i as f32 / sample_count as f32;
                let pressure = style.width * (brush.pressure_profile.at(t_param) * pressure_range + pressure_min)
                    * brush.pooling(point.y, top, bottom)
                    * variation.as_ref().map_or(1.0, |v| v.at(distance));
                // Where it is in the whole page's render, less the offset:
                // a whole number, so a tile's strokes land exactly where the
                // page's would.
                let x = (point.x - self.page_origin.x) * w_ratio - self.offset.0;
                let y = (point.y - self.page_origin.y) * h_ratio - self.offset.1;
                let down = if bottom > top { (point.y - top) / (bottom - top) } else { 0.5 };
                points.push((Vec2::new(x, y), i as f32 / (sample_count - 1).max(1) as f32, down));
                self.engine.stroke_to(StrokeOp::BrushStroke {
                    x,
                    y,
                    pressure,
                    x_tilt: 0.0,
                    y_tilt: 0.0,
                    rotation: 0.0,
                    tangential_pressure: 0.0,
                    time_ms: i as f64 * SAMPLE_SPACING_PX as f64 / PEN_SPEED_PX_PER_MS,
                    cr,
                    cg,
                    cb,
                    ca,
                });
            }
            self.engine.end_stroke();
            // Darkly finishes each stroke's bookkeeping (its undo snapshot)
            // asynchronously, on the next frame. Offline we never draw frames
            // until the export, so without this every stroke's ~4 MB of GPU
            // readbacks piles up: journal_entry_18 peaked at 14 GB and was
            // killed. Ticking every few strokes keeps it near 250 MB.
            if self.drawn.is_multiple_of(FRAME_EVERY_STROKES) {
                catch_up(self.engine).await?;
            }
        }
        Ok(points)
    }
}

/// Composite the document and read back `layer`'s render, then remove the
/// layer.
async fn export(engine: &mut DarklyEngine, layer: LayerId) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, String> {
    // Composite the document to the surface. Call once per frame; the arg is the
    // animation clock in seconds (drives time-based veils). Returns whether it drew.
    engine.start_export();
    loop {
        engine.render(0.0);
        match engine.poll_export_result() {
            Some(ExportImageResult { width, height, rgba }) => {
                let buff: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, rgba)
                    .expect("could not build image buffer");
                engine.remove_layer(layer).expect("Failed to remove layer");
                return Ok(buff);
            }
            None => {
                wait_for_gpu().await;
                gpu_failure()?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A render's worth of pixels from `origin`: a pattern defined on the
    /// whole page's render grid.
    fn pattern(origin: (i64, i64), size: (u32, u32)) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        ImageBuffer::from_fn(size.0, size.1, |x, y| {
            let (gx, gy) = (origin.0 + x as i64, origin.1 + y as i64);
            if (0..39).contains(&gx) && (0..30).contains(&gy) {
                Rgba([(gx * 7 % 256) as u8, (gy * 11 % 256) as u8, 90, ((gx * gy) % 200 + 55) as u8])
            } else {
                Rgba([0, 0, 0, 0])
            }
        })
    }

    #[test]
    fn tiles_downsample_exactly_as_the_whole_page() {
        // A page 26 x 20 output pixels, rendered 1.5 times as fine (the
        // pattern covers it, 39 x 30), filtered down whole and in tiles.
        let k = (1.5, 1.5);
        let whole = downsample(&pattern((0, 0), (39, 30)), (0, 0), k, (0, 0), (26, 20));
        for (x0, y0, x1, y1) in [(0, 0, 9, 7), (9, 0, 26, 7), (0, 7, 13, 20), (13, 7, 26, 20)] {
            let (rx0, ry0) = (((x0 - BLEED_PX) as f64 * k.0).floor() as i64, ((y0 - BLEED_PX) as f64 * k.1).floor() as i64);
            let (rx1, ry1) = (((x1 + BLEED_PX) as f64 * k.0).ceil() as i64, ((y1 + BLEED_PX) as f64 * k.1).ceil() as i64);
            let src = pattern((rx0, ry0), ((rx1 - rx0) as u32, (ry1 - ry0) as u32));
            let tile = downsample(&src, (rx0, ry0), k, (x0, y0), ((x1 - x0) as u32, (y1 - y0) as u32));
            for (x, y, p) in tile.enumerate_pixels() {
                assert_eq!(p, whole.get_pixel(x + x0 as u32, y + y0 as u32), "tile at {},{}: pixel {},{}", x0, y0, x, y);
            }
        }
    }

    #[test]
    fn downsampling_keeps_flat_color() {
        let src = ImageBuffer::from_pixel(30, 30, Rgba([200, 100, 50, 255]));
        let out = downsample(&src, (0, 0), (1.5, 1.5), (0, 0), (20, 20));
        // Away from the edges, where the transparency past them reaches.
        for y in 4..16 {
            for x in 4..16 {
                assert_eq!(out.get_pixel(x, y), &Rgba([200, 100, 50, 255]));
            }
        }
    }
}
