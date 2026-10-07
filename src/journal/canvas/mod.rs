use rayon::prelude::*;
use serde::Deserialize;
use crate::journal::{avoid::AvoidanceTree, StrokePath};
use parry2d::math::{Vec2};
use noise::{NoiseFn, Perlin};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use parry2d::bounding_volume;

pub mod textbox;
pub mod artistic;

pub type StrokeColor = String;

#[derive(Clone, Debug)]
pub struct StrokeStyle {
    pub width: f32,
    pub color: StrokeColor,
    /// Brush used when rendering with Darkly; `None` uses the renderer's
    /// default brush.
    pub brush: Option<Brush>,
}

/// A Darkly brush preset and the settings it is drawn with.
#[derive(Clone, Debug, PartialEq)]
pub struct Brush {
    /// Darkly brush preset name, e.g. "Calligraphy" or "Rough Ink".
    pub name: String,
    pub size: f32,
    pub min_pressure: f32,
    pub max_pressure: f32,
    pub pressure_profile: PressureProfile,
}

/// How pressure runs along each stroke, between a brush's min and max.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PressureProfile {
    /// From min at the start up to max at the end, so strokes get heavier
    /// as they go.
    #[default]
    Ramp,
    /// From min up to max at the middle and back down: tapered ends.
    Taper,
    /// Max all the way.
    Constant,
}

impl PressureProfile {
    pub const NAMES: [&'static str; 3] = ["ramp", "taper", "constant"];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "ramp" => Some(Self::Ramp),
            "taper" => Some(Self::Taper),
            "constant" => Some(Self::Constant),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Ramp => "ramp",
            Self::Taper => "taper",
            Self::Constant => "constant",
        }
    }

    /// Where pressure sits between min (0) and max (1), `t` of the way
    /// along a stroke.
    pub fn at(self, t: f32) -> f32 {
        match self {
            Self::Ramp => t,
            Self::Taper => (t * std::f32::consts::PI).sin(),
            Self::Constant => 1.0,
        }
    }
}

impl StrokeStyle {
    /// The stroke color as sRGB RGBA floats in 0-1.
    pub fn rgba(&self) -> Result<[f32; 4], String> {
        parse_color(&self.color)
    }
    /// How far a stroke's marks reach either side of its line: half the
    /// wider of its width (in SVG output) and its brush (in Darkly's).
    pub fn reach(&self) -> f32 {
        let brush = self.brush.as_ref().map_or(0.0, |b| b.size);
        self.width.max(brush).max(0.0) / 2.0
    }
}

/// Parse a CSS color string (named, #hex, rgb()/rgba(), hsl()/hsla()) into
/// sRGB RGBA floats in 0-1.
pub fn parse_color(color: &str) -> Result<[f32; 4], String> {
    let c: svgtypes::Color = color.parse()
        .map_err(|e| format!("Invalid color '{}': {}", color, e))?;
    Ok([c.red, c.green, c.blue, c.alpha].map(|v| v as f32 / 255.0))
}

#[derive(Clone, Debug)]
pub struct ColoredStroke {
    pub path: StrokePath,
    pub style: StrokeStyle,
    /// Where this stroke's random texture is seeded from: the number of
    /// subpaths drawn before it on the canvas it came from. `None` counts
    /// them on this canvas. A tile keeps its strokes' numbers from the
    /// whole page, so a stroke drawn in several tiles looks the same in each.
    pub seed: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CanvasStyleOptions {
    pub roughness: f32,
    pub point_sample_distance: f32,
    pub trail_off_percent: f32,
    pub noise_amplitude: f32,
}

impl Default for CanvasStyleOptions {
    fn default() -> Self {
        CanvasStyleOptions {
            roughness: 0.0,
            point_sample_distance: 2.0,
            trail_off_percent: 0.1,
            noise_amplitude: 0.0,
        }
    }
}

/// One tile of a page split into a grid: a fixed region to render, rather
/// than the strokes' bounds. A page's tiles share one pixel grid, the one a
/// render of the whole page would have, so their renders fit together edge
/// to edge into it. Strokes are drawn whole and cut off at the tile's edges,
/// so they carry on across seams.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The whole page's region, which the tiles divide evenly.
    pub page: bounding_volume::Aabb,
    pub column: u32,
    pub columns: u32,
    pub row: u32,
    pub rows: u32,
    /// Seeds the SVG output's roughening and noise, as the whole page's
    /// file name would.
    pub svg_seed: u64,
}

impl Frame {
    /// The whole page's size, as renders size a canvas: at least a unit
    /// each way.
    pub fn page_extents(&self) -> Vec2 {
        self.page.extents().max(Vec2::ONE)
    }
    /// This tile's pixels, `[x0, y0, x1, y1)`, in a render of the whole
    /// page `size` pixels across and down.
    pub fn pixels(&self, size: (u32, u32)) -> [u32; 4] {
        let edge = |i: u32, n: u32, total: u32| ((2 * i as u64 * total as u64 + n as u64) / (2 * n as u64)) as u32;
        [
            edge(self.column, self.columns, size.0),
            edge(self.row, self.rows, size.1),
            edge(self.column + 1, self.columns, size.0),
            edge(self.row + 1, self.rows, size.1),
        ]
    }
}

#[derive(Clone, Debug)]
pub struct Canvas {
    // set of strokes
    strokes: Vec<ColoredStroke>,
    bounds: bounding_volume::Aabb,
    frame: Option<Frame>,
}

impl From<Canvas> for StrokePath {
    fn from(val: Canvas) -> Self {
        let mut combined_path = StrokePath::new();
        for stroke in val.strokes {
            combined_path = combined_path + stroke.path;
        }
        combined_path
    }
}

impl From<&Canvas> for StrokePath {
    fn from(val: &Canvas) -> Self {
        let mut combined_path = StrokePath::new();
        for stroke in &val.strokes {
            combined_path = combined_path + stroke.path.clone();
        }
        combined_path
    }
}

/// Pixels rendered past a tile's edges in SVG output.
const SVG_BLEED_PX: u32 = 2;

/// A stroke's random generator for the SVG output's roughening, from the
/// canvas's seed and the stroke's.
fn stroke_rng(seed: u64, stroke_seed: u32) -> StdRng {
    StdRng::seed_from_u64(seed ^ (stroke_seed as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

impl Canvas {
    pub fn new() -> Self {
        Canvas {
            strokes: Vec::new(),
            bounds: bounding_volume::Aabb::new_invalid(),
            frame: None,
        }
    }
    pub fn bounds(&self) -> bounding_volume::Aabb {
        self.bounds
    }
    /// The size renders are drawn at: the bounds' extents, but at least one
    /// unit on each side, so a straight vertical or horizontal hairline
    /// still makes an image.
    pub fn render_extents(&self) -> Vec2 {
        self.bounds.extents().max(Vec2::ONE)
    }
    /// Grow the bounds (and so the render) by `margin` on every side.
    pub fn pad(&mut self, margin: f32) {
        if margin > 0.0 && self.bounds.mins.x <= self.bounds.maxs.x {
            self.bounds.mins -= Vec2::splat(margin);
            self.bounds.maxs += Vec2::splat(margin);
        }
    }
    pub fn has_strokes(&self) -> bool {
        !self.strokes.is_empty()
    }
    /// The tile of a page this canvas renders, if it's one.
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }
    pub fn set_frame(&mut self, frame: Option<Frame>) {
        self.frame = frame;
    }
    fn update_bounds(&mut self, pos: Vec2) {
        self.bounds.take_point(pos);
    }
    pub fn add_strokes<ISP: Into<StrokePath>>(&mut self, into_path: ISP, style: StrokeStyle) {
        self.add_seeded_stroke(into_path.into(), style, None);
    }
    /// Add a stroke, with its texture's seed (see `ColoredStroke::seed`).
    pub fn add_seeded_stroke(&mut self, path: StrokePath, style: StrokeStyle, seed: Option<u32>) {
        let width_vec = Vec2::splat(style.width);
        for pos in path.points_within() {
            self.update_bounds(pos);
            self.update_bounds(pos + width_vec);
            self.update_bounds(pos - width_vec);
        }
        self.strokes.push(ColoredStroke {
            path,
            style,
            seed,
        });
    }
    pub fn strokes(&self) -> &Vec<ColoredStroke> {
        &self.strokes
    }
    /// Each stroke's seed (see `ColoredStroke::seed`), counting subpaths in
    /// order where a stroke has none of its own.
    pub fn stroke_seeds(&self) -> Vec<u32> {
        let mut next = 0u32;
        self.strokes.iter().map(|stroke| {
            let seed = stroke.seed.unwrap_or(next);
            next = seed.wrapping_add(stroke.path.to_substrokes().len() as u32);
            seed
        }).collect()
    }
    /// Whether a stroke's marks might reach into the box from `min` to
    /// `max`: its control points' bounds, grown by `reach`.
    pub fn stroke_reaches(stroke: &ColoredStroke, min: Vec2, max: Vec2, reach: f32) -> bool {
        let points = stroke.path.points_within();
        if points.is_empty() {
            return false;
        }
        let (lo, hi) = points.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let reach = reach + stroke.style.reach() * 2.0;
        !(lo.x - reach > max.x || lo.y - reach > max.y || hi.x + reach < min.x || hi.y + reach < min.y)
    }
    /// The canvas's strokes as SVG paths in a group, each roughened (with a
    /// style) by a generator seeded from `seed` and the stroke's own seed,
    /// so a stroke looks the same whichever tile it's drawn in. Strokes that
    /// can't reach the box from `min` to `max` are left out.
    fn to_svg_group(&self, style: Option<&CanvasStyleOptions>, seed: u64, min: Vec2, max: Vec2) -> String {
        let mut svg = String::new();
        let reach = style.map_or(0.0, |s| s.roughness) + 1.0;
        for (stroke, stroke_seed) in self.strokes.iter().zip(self.stroke_seeds()) {
            if !Self::stroke_reaches(stroke, min, max, reach) {
                continue;
            }
            if let Some(opts) = style {
                svg.push_str(&self.stylized_path(stroke, opts, &mut stroke_rng(seed, stroke_seed)));
            } else {
                let ColoredStroke { path, style, .. } = stroke;
                svg.push_str(&format!(
                    "<path d=\"{}\" style=\"fill:none;stroke:{};stroke-width:{};\" />",
                    path,
                    style.color,
                    style.width,
                ));
            }
        }
        format!("<g>{}</g>", svg)
    }
    /// The canvas as SVG. With a style, strokes are roughened and tapered
    /// using generators seeded with `seed`, so the result is reproducible.
    pub fn to_svg(&self, stylized: Option<&CanvasStyleOptions>, seed: u64) -> String {
        self.to_svg_at(stylized, seed, 1.0).0
    }
    /// The canvas as SVG, and where its render at `scale` pixels per unit
    /// falls: the size of the image, and the page pixel of its top-left
    /// corner (only a tile's isn't 0, 0).
    fn to_svg_at(&self, stylized: Option<&CanvasStyleOptions>, seed: u64, scale: f32) -> (String, (u32, u32), (u32, u32)) {
        let (min, max) = (self.bounds.mins, self.bounds.maxs);
        let invalid_bounds = min.x > max.x || min.y > max.y;
        let (width, height, viewbox, pixels, offset) = if let Some(frame) = &self.frame {
            // The tile's pixels of the whole page's render, and the part of
            // the page they show.
            let extents = frame.page_extents();
            let page_size = (((extents.x * scale) as u32).max(1), ((extents.y * scale) as u32).max(1));
            let [x0, y0, x1, y1] = frame.pixels(page_size);
            let origin = frame.page.mins + Vec2::new(x0 as f32, y0 as f32) / scale;
            let size = Vec2::new((x1 - x0) as f32, (y1 - y0) as f32) / scale;
            let viewbox = format!("{} {} {} {}", origin.x, origin.y, size.x, size.y);
            ((x1 - x0) as f32 / scale, (y1 - y0) as f32 / scale, viewbox, (x1 - x0, y1 - y0), (x0, y0))
        } else if !invalid_bounds {
            let size = self.render_extents();
            // Center the bounds in the (possibly larger) render area.
            let origin = min - (size - (max - min)) / 2.0;
            let viewbox = format!("{} {} {} {}", origin.x, origin.y, size.x, size.y);
            (size.x, size.y, viewbox, (((size.x * scale) as u32).max(1), ((size.y * scale) as u32).max(1)), (0, 0))
        } else {
            (100.0, 100.0, "0 0 100 100".to_string(), (((100.0 * scale) as u32).max(1), ((100.0 * scale) as u32).max(1)), (0, 0))
        };
        let view: Vec<f32> = viewbox.split(' ').map(|v| v.parse().expect("numbers")).collect();
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{}\">",
            width, height, viewbox
        );
        svg.push_str(&self.to_svg_group(stylized, seed, Vec2::new(view[0], view[1]), Vec2::new(view[0] + view[2], view[1] + view[3])));
        svg.push_str("</svg>");
        (svg, pixels, offset)
    }
    /// Vary the opacity of a render with Perlin noise seeded by `seed`, read
    /// at page pixel `offset` plus each pixel's, so tiles' noise lines up.
    pub fn stylize_pixmap(pixmap: &mut tiny_skia::Pixmap, stylized: &CanvasStyleOptions, seed: u64, offset: (u32, u32)) {
        if stylized.noise_amplitude > 0.0 {
            let perlin = Perlin::new((seed ^ (seed >> 32)) as u32);
            let width = pixmap.width();
            pixmap.pixels_mut().par_iter_mut().enumerate().for_each(|(i, p)| {
                let x = (i as u32 % width + offset.0) as f64;
                let y = (i as u32 / width + offset.1) as f64;
                let octaves = 4;
                let noise_num = 15.0;
                let noise_value = (0..octaves).fold(0.0, |acc, octave| {
                    let noise_scale = noise_num / (10.0_f64).powi(octave);
                    acc + perlin.get([x * noise_scale, y * noise_scale]) / (2.0_f64).powi(octave)
                });

                let base_opacity = 0.95;
                let noise_amplitude = stylized.noise_amplitude;
                let variation = base_opacity + (noise_value as f32 * noise_amplitude);
                let variation = variation.clamp(0.0, 1.0);
                let color = p.demultiply();
                let alpha_f32 = color.alpha() as f32 / 255.0;
                let new_color = resvg::tiny_skia::ColorU8::from_rgba(
                    color.red(),
                    color.green(),
                    color.blue(),
                    (alpha_f32 * variation * 255.0).round() as u8,
                ).premultiply();
                *p = new_color;
            });
        }
    }
    /// The canvas as a PNG; see `to_svg` for `seed`.
    pub fn to_png(&self, scale: f32, stylized: Option<&CanvasStyleOptions>, seed: u64) -> Vec<u8> {
        self.to_svg_and_png(scale, stylized, seed).1
    }
    /// The canvas as SVG and that SVG rasterised at `scale` pixels per unit
    /// (with the style's noise added); see `to_svg` for `seed`.
    pub fn to_svg_and_png(&self, scale: f32, stylized: Option<&CanvasStyleOptions>, seed: u64) -> (String, Vec<u8>) {
        let (svg_data, (width, height), offset) = self.to_svg_at(stylized, seed, scale);
        let svg_tree = usvg::Tree::from_str(&svg_data, &usvg::Options::default()).expect("Failed to parse SVG");
        // A tile is drawn with a border of extra pixels and cut out of it,
        // so its edges are filled as they would be inside the whole page.
        let bleed = if self.frame.is_some() { SVG_BLEED_PX } else { 0 };
        let mut pixmap = tiny_skia::Pixmap::new(width + 2 * bleed, height + 2 * bleed).expect("Failed to create Pixmap");
        let transform = tiny_skia::Transform::from_translate(bleed as f32, bleed as f32).pre_scale(scale, scale);
        resvg::render(&svg_tree, transform, &mut pixmap.as_mut());
        if bleed > 0 {
            let rect = tiny_skia::IntRect::from_xywh(bleed as i32, bleed as i32, width, height).expect("a tile has pixels");
            pixmap = pixmap.clone_rect(rect).expect("the tile is inside its bled render");
        }
        if let Some(opts) = stylized {
            Self::stylize_pixmap(&mut pixmap, opts, seed, offset);
        }
        (svg_data, pixmap.encode_png().expect("Failed to encode PNG"))
    }
    pub fn stylized_path(&self, stroke: &ColoredStroke, opts: &CanvasStyleOptions, rng: &mut dyn RngCore) -> String {
        let pos_roughen = opts.roughness;
        let distance = opts.point_sample_distance;
        let trail_off_percent = opts.trail_off_percent;
        let mut path = stroke.path.to_cubics();
        let color = stroke.style.color.clone();
        let width = stroke.style.width;
        let halfwidth = width / 2.0;
        path.adjust_by(|&cmd| {
            cmd.pos_roughen_by(pos_roughen, rng)
        });
        let substrokes = path.to_substrokes();
        let mut path = StrokePath::new();
        for substroke in substrokes {
            let samples = substroke.get_point_samples(distance);
            if samples.len() < 2 {
                continue;
            }
            let firstpoint = samples[0].point;
            path = path.move_to(firstpoint.x, firstpoint.y);
            for (i, sample) in samples.iter().enumerate() {
                let percentile = i as f32 / (samples.len() - 1) as f32;

                let width = if percentile < trail_off_percent {
                    halfwidth * (percentile / trail_off_percent)
                } else if percentile > (1.0 - trail_off_percent) {
                    halfwidth * ((1.0 - percentile) / trail_off_percent)
                } else {
                    halfwidth
                };
                let point = sample.point + width * sample.normal;
                path = path.line_to(point.x, point.y);
            }
            // iterate samples in reverse order
            for (i, sample) in samples.iter().rev().enumerate() {
                let percentile = i as f32 / (samples.len() - 1) as f32;

                let width = if percentile < trail_off_percent {
                    halfwidth * (percentile / trail_off_percent)
                } else if percentile > (1.0 - trail_off_percent) {
                    halfwidth * ((1.0 - percentile) / trail_off_percent)
                } else {
                    halfwidth
                };
                let point = sample.point - width * sample.normal;
                path = path.line_to(point.x, point.y);
            }
            path = path.close_path();
        }
        format!("<path d=\"{}\" fill=\"{}\" stroke=\"{}\" />", path, color, color)
    }
    /// What keeps `clearance` away from this canvas's marks.
    pub fn avoidance(&self, clearance: f32) -> AvoidanceTree {
        let mut tree = AvoidanceTree::new();
        tree.add_canvas(self, clearance);
        tree
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::parse_color;

    #[test]
    fn parses_css_color_forms() {
        assert_eq!(parse_color("black"), Ok([0.0, 0.0, 0.0, 1.0]));
        assert_eq!(parse_color("darkred"), Ok([139.0 / 255.0, 0.0, 0.0, 1.0]));
        assert_eq!(parse_color("#00ff00"), Ok([0.0, 1.0, 0.0, 1.0]));
        assert_eq!(parse_color("rgba(0, 0, 0, 0.7)"), Ok([0.0, 0.0, 0.0, 179.0 / 255.0]));
        // hsl(120, 100%, 50%) is pure green; 4-arg hsl carries alpha.
        assert_eq!(parse_color("hsl(120, 100%, 50%, 0.8)"), Ok([0.0, 1.0, 0.0, 204.0 / 255.0]));
    }

    #[test]
    fn rejects_invalid_colors() {
        assert!(parse_color("darkredd").is_err());
        assert!(parse_color("").is_err());
    }
}

#[cfg(test)]
mod svg_output_tests {
    use super::*;

    #[test]
    fn styled_output_follows_the_seed() {
        let mut canvas = Canvas::new();
        let style = StrokeStyle { width: 2.0, color: "black".to_string(), brush: None };
        canvas.add_strokes(StrokePath::new().move_to(0.0, 0.0).cubic_curve_to(10.0, 0.0, 20.0, 10.0, 30.0, 30.0), style);
        let opts = CanvasStyleOptions { roughness: 2.0, point_sample_distance: 1.0, trail_off_percent: 0.15, noise_amplitude: 1.5 };
        assert_eq!(canvas.to_png(1.0, Some(&opts), 7), canvas.to_png(1.0, Some(&opts), 7));
        assert_ne!(canvas.to_png(1.0, Some(&opts), 7), canvas.to_png(1.0, Some(&opts), 8));
        assert_eq!(canvas.to_svg(Some(&opts), 7), canvas.to_svg(Some(&opts), 7));
    }
}

#[cfg(test)]
mod pressure_tests {
    use super::PressureProfile;

    #[test]
    fn profiles_run_between_min_and_max() {
        for t in [0.0, 0.25, 0.5, 0.75] {
            assert_eq!(PressureProfile::Ramp.at(t), t);
            assert_eq!(PressureProfile::Constant.at(t), 1.0);
        }
        let taper = |t| PressureProfile::Taper.at(t);
        assert!(taper(0.0).abs() < 1e-6 && taper(1.0).abs() < 1e-6 && (taper(0.5) - 1.0).abs() < 1e-6);
        assert!((taper(0.25) - taper(0.75)).abs() < 1e-6);
        for name in PressureProfile::NAMES {
            assert_eq!(PressureProfile::from_name(name).unwrap().name(), name);
        }
    }
}
