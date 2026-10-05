use darkly::engine::{DarklyEngine, types::StrokeOp, ExportImageResult};
use darkly::gpu::context::GpuContext;
use darkly::coord::CanvasRect;
use darkly::config::ConfigValue;
use image::{ImageBuffer, Rgba, imageops::FilterType};
use crate::journal::paths::{CurveSample};
use crate::journal::canvas::{Brush, Canvas, ColoredStroke};
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

/// Render a canvas with Darkly into a `width` x `height` image.
///
/// `brush_scale` multiplies every brush size: brush sizes are in pixels, so
/// a render at a different scale must scale them too to look the same.
/// With `supersample` > 1 the canvas is rendered at that multiple of the
/// size (brushes included) and filtered back down, which smooths the
/// aliasing Darkly produces for brush marks thinner than about a pixel.
///
/// Each stroke is drawn with its own brush, or `default_brush` if it has
/// none; panics if a stroke has neither.
pub fn render_canvas_scaled(
    canvas: &Canvas,
    engine: &mut DarklyEngine,
    default_brush: Option<&Brush>,
    width: u32,
    height: u32,
    brush_scale: f32,
    supersample: f32,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let k = supersample.max(1.0);
    // Fractional factors are fine: the render rounds to whole pixels, and
    // the filter scales back down by any ratio.
    let render_width = (width as f32 * k).round() as u32;
    let render_height = (height as f32 * k).round() as u32;
    resize_canvas(engine, render_width, render_height);
    let rendered = render_canvas(canvas, engine, default_brush, brush_scale * k);
    if (render_width, render_height) == (width, height) {
        rendered
    } else {
        downsample(&rendered, width, height)
    }
}

/// Resize an RGBA image with a Lanczos filter, in premultiplied alpha so the
/// transparent background's color doesn't bleed into stroke edges.
fn downsample(image: &ImageBuffer<Rgba<u8>, Vec<u8>>, width: u32, height: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let premultiplied = ImageBuffer::from_fn(image.width(), image.height(), |x, y| {
        let [r, g, b, a] = image.get_pixel(x, y).0.map(|v| v as f32 / 255.0);
        Rgba([r * a, g * a, b * a, a])
    });
    let resized = image::imageops::resize(&premultiplied, width, height, FilterType::Lanczos3);
    ImageBuffer::from_fn(width, height, |x, y| {
        // Lanczos overshoots slightly; clamp alpha, and color to at most alpha.
        let [r, g, b, a] = resized.get_pixel(x, y).0;
        let a = a.clamp(0.0, 1.0);
        let unpremultiply = |c: f32| if a > 0.0 { (c.clamp(0.0, a) / a * 255.0).round() as u8 } else { 0 };
        Rgba([unpremultiply(r), unpremultiply(g), unpremultiply(b), (a * 255.0).round() as u8])
    })
}

/// Render a canvas into the engine's current canvas size.
fn render_canvas(canvas: &Canvas, engine: &mut DarklyEngine, default_brush: Option<&Brush>, brush_scale: f32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let layer = engine.add_raster_layer(None);
    let (engine_canvas_width, engine_canvas_height) = engine.canvas_dimensions();
    let bounds = canvas.bounds();
    let canvas_dims = bounds.extents();
    let w_ratio =  engine_canvas_width as f32 / canvas_dims.x as f32;
    let h_ratio = engine_canvas_height as f32 / canvas_dims.y as f32;
    let sample_spacing = SAMPLE_SPACING_PX / w_ratio.max(h_ratio);

    let strokes = canvas.strokes();
    let mut warned_colors = HashSet::new();
    // Only reload the brush / reset its size when they change between strokes.
    let mut loaded_name: Option<&str> = None;
    let mut loaded_size: Option<f32> = None;
    for ColoredStroke { path, style } in strokes {
        let brush = style.brush.as_ref().or(default_brush)
            .expect("Stroke has no brush and no default brush was given");
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
        let [cr, cg, cb, ca] = style.rgba().unwrap_or_else(|err| {
            if warned_colors.insert(style.color.clone()) {
                eprintln!("{}; drawing in black.", err);
            }
            [0.0, 0.0, 0.0, 1.0]
        });
        let substrokes = path.to_substrokes();
        for substroke in substrokes {
            // One stroke_path call per substroke: Darkly stabilizes and draws
            // the whole stroke at once, instead of re-rendering and
            // committing on every point as live stroke_to does.
            let mut ops = Vec::new();
            let samples = substroke.get_point_samples(sample_spacing);
            let sample_count = samples.len();
            for (i, CurveSample { point,  .. }) in samples.into_iter().enumerate() {
                let t_param = i as f32 / sample_count as f32;
                let pressure = style.width * (t_param * pressure_range + pressure_min);
                ops.push(StrokeOp::BrushStroke {
                    x: (point.x - bounds.mins.x) * w_ratio,
                    y: (point.y - bounds.mins.y) * h_ratio,
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
