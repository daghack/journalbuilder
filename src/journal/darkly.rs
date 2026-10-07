use darkly::engine::{DarklyEngine, types::StrokeOp, ExportImageResult};
use darkly::gpu::context::GpuContext;
use darkly::coord::CanvasRect;
use darkly::config::ConfigValue;
use image::{ImageBuffer, Rgba};
use parry2d::math::Vec2;
use rayon::prelude::*;
use crate::journal::paths::{CurveSample};
use crate::journal::canvas::{Canvas, ColoredStroke};
use std::collections::HashSet;

pub fn new_engine(width: u32, height: u32) -> DarklyEngine {
    // Stroke prediction extrapolates and re-renders a tail ahead of the pen
    // to hide input latency; offline it only costs time. (Config is
    // thread-local, so this applies to engines used on this thread.)
    darkly::config::set("input.predictionHorizon", ConfigValue::Float(0.0));
    let instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
    let instance = wgpu::Instance::new(instance_desc);
    let adapter = smol::block_on(
        instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("Request for adapter failed.");
    let (device, queue) = smol::block_on(adapter
        .request_device(&wgpu::DeviceDescriptor::default()))
        .expect("Request for device failed.");
    let gpu = GpuContext::new_headless(device, queue);
    let mut engine = DarklyEngine::new(gpu, width, height);
    engine.add_raster_layer(None);
    engine
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

/// How many strokes to draw between frame ticks (see `render_canvas`).
const FRAME_EVERY_STROKES: u32 = 25;

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
pub fn render(canvas: &Canvas, engine: &mut DarklyEngine, scale: f32, supersample: f32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
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
    let rendered = render_canvas(canvas, engine, page_origin, (rx0 as f32, ry0 as f32), ratio, scale * k.0.max(k.1) as f32);
    if !supersampled {
        return rendered;
    }
    downsample(&rendered, (rx0, ry0), k, (x0, y0), ((x1 - x0) as u32, (y1 - y0) as u32))
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
fn render_canvas(canvas: &Canvas, engine: &mut DarklyEngine, page_origin: Vec2, offset: (f32, f32), ratio: Vec2, brush_scale: f32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let layer = engine.add_raster_layer(None);
    let (engine_canvas_width, engine_canvas_height) = engine.canvas_dimensions();
    let (w_ratio, h_ratio) = (ratio.x, ratio.y);
    let sample_spacing = SAMPLE_SPACING_PX / w_ratio.max(h_ratio);
    let origin = page_origin + Vec2::new(offset.0 / w_ratio, offset.1 / h_ratio);
    let view_max = origin + Vec2::new(engine_canvas_width as f32 / w_ratio, engine_canvas_height as f32 / h_ratio);

    let strokes = canvas.strokes();
    let mut warned_colors = HashSet::new();
    let mut warned_saturation = HashSet::new();
    // Only reload the brush / reset its size when they change between strokes.
    let mut loaded_name: Option<&str> = None;
    let mut loaded_size: Option<f32> = None;
    let mut drawn: u32 = 0;
    for (stroke, seed) in strokes.iter().zip(canvas.stroke_seeds()) {
        if !Canvas::stroke_reaches(stroke, origin, view_max, 0.0) {
            continue;
        }
        let ColoredStroke { path, style, .. } = stroke;
        let brush = style.brush.as_ref()
            .expect("Darkly strokes need a brush (rhai stroke styles always have one)");
        if loaded_name != Some(brush.name.as_str()) {
            engine.brush_load(&brush.name)
                .unwrap_or_else(|e| panic!("Failed to load brush '{}': {}", brush.name, e));
            loaded_name = Some(brush.name.as_str());
            loaded_size = None;
        }
        let size = brush.size * brush_scale;
        if loaded_size != Some(size) {
            engine.brush_set_exposed_port("brush_settings", "size", size).expect("Unable to set brush size");
            loaded_size = Some(size);
        }
        let pressure_min = brush.min_pressure;
        let pressure_range = brush.max_pressure - brush.min_pressure;
        // A style's width scales its brush's pressure here (in the plain
        // output it's a geometric width). Darkly caps pressure at 1, so
        // past that, wider strokes stop getting heavier.
        let peak_pressure = style.width * brush.max_pressure;
        if peak_pressure > 1.0 && warned_saturation.insert((style.width.to_bits(), brush.name.as_str(), brush.max_pressure.to_bits())) {
            eprintln!(
                "warning: width {} strokes with brush '{}' (pressure {}-{}) reach pressure {:.2}; Darkly caps pressure at 1, so their heaviest parts look the same",
                style.width, brush.name, brush.min_pressure, brush.max_pressure, peak_pressure,
            );
        }
        let [cr, cg, cb, ca] = style.rgba().unwrap_or_else(|err| {
            if warned_colors.insert(style.color.clone()) {
                eprintln!("{}; drawing in black.", err);
            }
            [0.0, 0.0, 0.0, 1.0]
        });
        let substrokes = path.to_substrokes();
        for (i, substroke) in substrokes.iter().enumerate() {
            // Darkly seeds each stroke's random brush texture from the wall
            // clock by default; numbering the strokes instead makes renders
            // reproducible while keeping neighbouring strokes distinct. (A
            // tile's strokes keep their numbers from the whole page.)
            engine.set_stroke_seed(Some(seed.wrapping_add(i as u32)));
            drawn = drawn.wrapping_add(1);
            // One stroke_path call per substroke: Darkly stabilizes and draws
            // the whole stroke at once, instead of re-rendering and
            // committing on every point as live stroke_to does.
            let mut ops = Vec::new();
            let samples = substroke.get_point_samples(sample_spacing);
            let sample_count = samples.len();
            for (i, CurveSample { point,  .. }) in samples.into_iter().enumerate() {
                let t_param = i as f32 / sample_count as f32;
                let pressure = style.width * (brush.pressure_profile.at(t_param) * pressure_range + pressure_min);
                ops.push(StrokeOp::BrushStroke {
                    // Where it is in the whole page's render, less the
                    // offset: a whole number, so a tile's strokes land
                    // exactly where the page's would.
                    x: (point.x - page_origin.x) * w_ratio - offset.0,
                    y: (point.y - page_origin.y) * h_ratio - offset.1,
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
            engine.stroke_path(layer, &ops).expect("stroke_path failed");
            // Darkly finishes each stroke's bookkeeping (its undo snapshot)
            // asynchronously, on the next frame. Offline we never draw frames
            // until the export, so without this every stroke's ~4 MB of GPU
            // readbacks piles up: journal_entry_18 peaked at 14 GB and was
            // killed. Ticking every few strokes keeps it near 250 MB.
            if drawn.is_multiple_of(FRAME_EVERY_STROKES) {
                engine.render(0.0);
            }
        }
    }

    // Composite the document to the surface. Call once per frame; the arg is the
    // animation clock in seconds (drives time-based veils). Returns whether it drew.
    engine.start_export();
    loop {
        engine.render(0.0);
        let poll = engine.poll_export_result();
        match poll {
            Some(export_img) => {
                let ExportImageResult {
                    width,
                    height,
                    rgba
                } = export_img;
                let buff : ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, rgba)
                    .expect("could not build image buffer");
                engine.remove_layer(layer).expect("Failed to remove layer");
                return buff;
            }
            _ => {
                std::thread::sleep(std::time::Duration::from_millis(2));
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
