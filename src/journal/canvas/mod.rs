use rayon::prelude::*;
use serde::Deserialize;
use crate::journal::{symbol::AvoidanceKdTree, StrokePath, img::pixmap_from_svg_tree};
use parry2d::math::{Vec2};
use noise::{NoiseFn, Perlin};
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
}


#[derive(Clone, Debug)]
pub enum CanvasShape {
    Circle { radius: f32, center: Vec2 },
    Rectangle { width: f32, height: f32, center: Vec2 },
    LineSegment { start: Vec2, end: Vec2 },
}

impl Into<StrokePath> for CanvasShape {
    fn into(self) -> StrokePath {
        match self {
            CanvasShape::Circle { radius, center } => {
                StrokePath::circle(center, radius)
            }
            CanvasShape::Rectangle { width, height, center } => {
                StrokePath::rectangle(center, width, height)
            }
            CanvasShape::LineSegment { start, end } => {
                StrokePath::line(start, end)
            }
        }
    }
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

#[derive(Clone, Debug)]
pub struct Canvas {
    // set of strokes
    strokes: Vec<ColoredStroke>,
    bounds: bounding_volume::Aabb,
}

impl Into<StrokePath> for Canvas {
    fn into(self) -> StrokePath {
        let mut combined_path = StrokePath::new();
        for stroke in self.strokes {
            combined_path = combined_path + stroke.path;
        }
        combined_path
    }
}

impl Into<StrokePath> for &Canvas {
    fn into(self) -> StrokePath {
        let mut combined_path = StrokePath::new();
        for stroke in &self.strokes {
            combined_path = combined_path + stroke.path.clone();
        }
        combined_path
    }
}

impl Canvas {
    pub fn new() -> Self {
        Canvas {
            strokes: Vec::new(),
            bounds: bounding_volume::Aabb::new_invalid(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty()
    }
    pub fn merge_canvas(&mut self, other: Canvas) {
        for stroke in other.strokes {
            self.add_strokes(stroke.path, stroke.style);
        }
    }
    pub fn draw_canvas(&mut self, other: &Canvas) {
        for stroke in &other.strokes {
            self.add_strokes(stroke.path.clone(), stroke.style.clone());
        }
    }
    pub fn bounds(&self) -> bounding_volume::Aabb {
        self.bounds
    }
    fn update_bounds(&mut self, pos: Vec2) {
        self.bounds.take_point(pos);
    }
    pub fn calc_tight_bounds(&self) -> bounding_volume::Aabb {
        let mut tight_bounds = bounding_volume::Aabb::new_invalid();
        for stroke in &self.strokes {
            let samples = stroke.path.sample_points_along(1.0);
            for pos in samples {
                tight_bounds.take_point(pos.point);
            }
        }
        tight_bounds
    }
    pub fn clear(&mut self) {
        // clear all strokes
        self.strokes.clear();
        self.bounds = bounding_volume::Aabb::new_invalid();
    }
    pub fn add_strokes<ISP: Into<StrokePath>>(&mut self, into_path: ISP, style: StrokeStyle) {
        let path: StrokePath = into_path.into();
        // add stroke to canvas
        for pos in path.points_within() {
            let width_vec = Vec2::new(
                style.width,
                style.width,
            );
            self.update_bounds(pos);
            self.update_bounds(pos + width_vec);
            self.update_bounds(pos - width_vec);
        }
        self.strokes.push(ColoredStroke {
            path,
            style,
        });
    }
    pub fn add_border_strokes(&mut self, style: StrokeStyle) {
        let bounds = self.bounds();
        let rectangle = CanvasShape::Rectangle {
            center: (bounds.mins + bounds.maxs) / 2.0,
            width: bounds.maxs.x - bounds.mins.x,
            height: bounds.maxs.y - bounds.mins.y,
        };
        self.add_strokes(rectangle, style);
    }
    pub fn strokes(&self) -> &Vec<ColoredStroke> {
        &self.strokes
    }
    pub fn to_svg_group(&self, id: Option<&str>, style: Option<&CanvasStyleOptions>) -> String {
        let mut svg = String::new();
        for stroke in &self.strokes {
            if let Some(opts) = style {
                svg.push_str(&self.stylized_path(stroke, opts));
            } else {
                let ColoredStroke{path, style} = stroke;
                svg.push_str(&format!(
                    "<path d=\"{}\" style=\"fill:none;stroke:{};stroke-width:{};\" />",
                    path.to_string(),
                    style.color,
                    style.width,
                ));
            }
        }
        if let Some(id) = id {
            format!("<g id=\"{}\">{}</g>", id, svg)
        } else {
            format!("<g>{}</g>", svg)
        }
    }
    pub fn to_svg(&self, stylized: Option<&CanvasStyleOptions>) -> String {
        let mut svg = String::new();
        let (min, max) = (self.bounds.mins, self.bounds.maxs);
        let invalid_bounds = min.x > max.x || min.y > max.y;
        let (width, height, viewbox) = if !invalid_bounds {
            let viewbox = format!(
                "{} {} {} {}",
                min.x,
                min.y,
                (max.x - min.x).abs(),
                (max.y - min.y).abs()
            );
            ((max.x - min.x).abs(), (max.y - min.y).abs(), viewbox)
        } else {
            (100.0, 100.0, "0 0 100 100".to_string())
        };
        svg.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{}\">",
            width, height, viewbox
        ));
        svg.push_str(&self.to_svg_group(None, stylized));
        svg.push_str("</svg>");
        svg
    }
    pub fn stylize_pixmap(pixmap: &mut tiny_skia::Pixmap, stylized: &CanvasStyleOptions) {
        if stylized.noise_amplitude > 0.0 {
            let perlin = Perlin::new(rand::random::<u32>());
            let width = pixmap.width();
            pixmap.pixels_mut().par_iter_mut().enumerate().for_each(|(i, p)| {
                let x = (i as u32 % width) as f64;
                let y = (i as u32 / width) as f64;
                let octaves = 4;
                let noise_num = 15.0;
                let noise_value = (0..octaves).fold(0.0, |acc, octave| {
                    let noise_scale = noise_num / (10.0_f64).powi(octave as i32);
                    acc + perlin.get([x * noise_scale, y * noise_scale]) / (2.0_f64).powi(octave as i32)
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
    pub fn save_to_png(&self, path: String, scale: f32, stylized: Option<&CanvasStyleOptions>) {
        let png_data = self.to_png(scale, stylized);
        let path = format!("{}.png", path);
        std::fs::write(path, png_data).expect("Failed to write PNG file");
    }
    pub fn to_png(&self, scale: f32, stylized: Option<&CanvasStyleOptions>) -> Vec<u8> {
        let svg_data = self.to_svg(stylized.clone());
        let svg_tree = usvg::Tree::from_str(&svg_data, &usvg::Options::default()).expect("Failed to parse SVG");
        let mut pixmap = pixmap_from_svg_tree(&svg_tree, scale, scale);
        if let Some(opts) = stylized {
            Self::stylize_pixmap(&mut pixmap, opts);
        }
        pixmap.encode_png().expect("Failed to encode PNG")
    }
    pub fn save_to_svg(&self, path: String, stylized: Option<&CanvasStyleOptions>) {
        let svg_data = self.to_svg(stylized);
        let path = format!("{}.svg", path);
        std::fs::write(path, svg_data).expect("Failed to write SVG file");
    }
    pub fn stylized_path(&self, stroke: &ColoredStroke, opts: &CanvasStyleOptions) -> String {
        let pos_roughen = opts.roughness;
        let distance = opts.point_sample_distance;
        let trail_off_percent = opts.trail_off_percent;
        let mut path = stroke.path.clone();
        let color = stroke.style.color.clone();
        let width = stroke.style.width;
        let halfwidth = width / 2.0;
        path.adjust_by(|&cmd| {
            cmd.to_cubic_curve().pos_roughen_by(pos_roughen)
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
        format!("<path d=\"{}\" fill=\"{}\" stroke=\"{}\" />", path.to_string(), color, color)
    }
    pub fn roughen_by(&mut self, roughness: f32) {
        for stroke in &mut self.strokes {
            stroke.path.adjust_by(|&cmd| {
                cmd.to_cubic_curve().pos_roughen_by(roughness)
            });
        }
    }
    pub fn translate_by(&mut self, translation: Vec2) {
        for stroke in &mut self.strokes {
            stroke.path.translate_by(translation);
        }
        self.bounds.mins += translation;
        self.bounds.maxs += translation;
    }
    pub fn scale_by(&mut self, scale: f32) {
        for stroke in &mut self.strokes {
            stroke.path.scale_by(scale);
        }
        self.bounds.mins *= scale;
        self.bounds.maxs *= scale;
    }
    pub fn rotate_by(&mut self, rot_origin: Vec2, rot_rad: f32) {
        self.bounds = bounding_volume::Aabb::new_invalid();
        for stroke in self.strokes.iter_mut() {
            stroke.path.rotate_around(rot_origin, rot_rad);
        }
        let points : Vec<(Vec<Vec2>, f32)> = self.strokes
            .iter()
            .map(|stroke| (stroke.path.points_within(), stroke.style.width))
            .collect();

        for (points, width) in points {
            let width_vec = Vec2::new(
                width,
                width,
            );
            for point in points {
                self.update_bounds(point);
                self.update_bounds(point + width_vec);
                self.update_bounds(point - width_vec);
            }
        }
    }
    pub fn get_avoid_tree(&self, avoid_radius: f32) -> AvoidanceKdTree {
        let stroke_path: StrokePath = self.into();
        let avoid_tree = stroke_path.build_kdtree(avoid_radius, None);
        AvoidanceKdTree::new(avoid_tree, avoid_radius)
    }
    pub fn normalize(&mut self) {
        // scale and translate canvas to fit within (0,0) to (1,1)
        let bounds = self.bounds();
        let width = bounds.maxs.x - bounds.mins.x;
        let height = bounds.maxs.y - bounds.mins.y;
        let scale = 1.0 / width.max(height);
        self.scale_by(scale);
        let new_bounds = self.bounds();
        let translation = Vec2::new(
            -new_bounds.mins.x,
            -new_bounds.mins.y,
        );
        self.translate_by(translation);
    }
    pub fn fit_within_width(&mut self, bounds: &bounding_volume::Aabb, ext_scale: f32) {
        let current_bounds = self.calc_tight_bounds();
        let current_width = current_bounds.maxs.x - current_bounds.mins.x;
        let target_width = bounds.maxs.x - bounds.mins.x;
        let scale = target_width / current_width;
        self.scale_by(scale*ext_scale);
        let new_bounds = self.bounds();
        // center width within target bounds
        let translation = Vec2::new(
            bounds.mins.x + (target_width - (new_bounds.maxs.x - new_bounds.mins.x)) / 2.0 - new_bounds.mins.x,
            bounds.mins.y - new_bounds.mins.y,
        );
        self.translate_by(translation);
    }
    pub fn fit_within_bounds(&mut self, bounds: &bounding_volume::Aabb) {
        let current_bounds = self.calc_tight_bounds();
        let current_width = current_bounds.maxs.x - current_bounds.mins.x;
        let current_height = current_bounds.maxs.y - current_bounds.mins.y;
        let target_width = bounds.maxs.x - bounds.mins.x;
        let target_height = bounds.maxs.y - bounds.mins.y;
        let scale_x = target_width / current_width;
        let scale_y = target_height / current_height;
        let scale = scale_x.min(scale_y);
        self.scale_by(scale);
        let new_bounds = self.bounds();
        // next we need to translate the canvas so that the top-left corner of the new bounds is at
        // the top-left corner of the target bounds
        let translation = Vec2::new(
            bounds.mins.x - new_bounds.mins.x,
            bounds.mins.y - new_bounds.mins.y,
        );
        self.translate_by(translation);
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
