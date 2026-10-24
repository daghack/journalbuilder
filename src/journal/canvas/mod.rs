use rayon::prelude::*;
use crate::journal::{avoid::AvoidanceTree, StrokePath};
use crate::journal::gradient::{Gradient, GradientKind};
use parry2d::math::{Vec2};
use noise::{NoiseFn, Perlin};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use parry2d::bounding_volume;

pub mod textbox;
pub mod artistic;

/// What a stroke is drawn in: a CSS colour, or a gradient.
#[derive(Clone, Debug, PartialEq)]
pub enum StrokeColor {
    Solid(String),
    Gradient(Gradient),
}

impl StrokeColor {
    pub fn gradient(&self) -> Option<&Gradient> {
        match self {
            StrokeColor::Gradient(g) => Some(g),
            StrokeColor::Solid(_) => None,
        }
    }
}

impl StrokeColor {
    /// The colour as ink that shows on paper: colours lighter than a
    /// mid-tone get as dark as they were light (their lightness mirrored,
    /// hue and saturation kept), so light inks meant for a dark background
    /// come out dark; darker ones stay as they are. Gradients' stops each.
    /// A solid colour that doesn't parse is left for the renderer to warn
    /// about.
    pub fn as_ink_on_paper(&self) -> StrokeColor {
        match self {
            StrokeColor::Solid(color) => match parse_color(color) {
                Ok(rgba) => StrokeColor::Solid(css(dark_ink(rgba))),
                Err(_) => self.clone(),
            },
            StrokeColor::Gradient(g) => {
                let stops = g.stops.iter().map(|(at, c)| (*at, dark_ink(*c))).collect();
                StrokeColor::Gradient(Gradient { stops, kind: g.kind })
            }
        }
    }
}

/// An sRGB RGBA colour no lighter than a mid-tone: lightness L (in HSL)
/// above 1/2 becomes 1 - L. With hue and saturation kept, that shifts every
/// channel by the same amount, 1 - (max + min).
fn dark_ink([r, g, b, a]: [f32; 4]) -> [f32; 4] {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let shift = (1.0 - (max + min)).min(0.0);
    [r + shift, g + shift, b + shift, a]
}

impl From<&str> for StrokeColor {
    fn from(color: &str) -> Self {
        StrokeColor::Solid(color.to_string())
    }
}

/// A colour as CSS, for SVG.
fn css(c: [f32; 4]) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("rgba({}, {}, {}, {})", byte(c[0]), byte(c[1]), byte(c[2]), c[3])
}

/// An SVG gradient element, `id`, for a gradient laid across the canvas.
fn svg_gradient(gradient: &Gradient, id: &str) -> String {
    let stops: String = gradient.stops.iter().map(|(at, c)| {
        format!("<stop offset=\"{}\" stop-color=\"{}\" />", at, css(*c))
    }).collect();
    match gradient.kind {
        GradientKind::Linear { from, to } => format!(
            "<linearGradient id=\"{}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{}</linearGradient>",
            id, from.x, from.y, to.x, to.y, stops),
        GradientKind::Radial { center, radius } => format!(
            "<radialGradient id=\"{}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" cy=\"{}\" r=\"{}\">{}</radialGradient>",
            id, center.x, center.y, radius, stops),
        GradientKind::Along | GradientKind::Vertical => String::new(),
    }
}

/// How long the pieces of a stroke are in SVG output, where its colour
/// changes along it: about `ALONG_PIECES` to a stroke, at least
/// `ALONG_PIECE_MIN` units.
const ALONG_PIECES: f32 = 48.0;
const ALONG_PIECE_MIN: f32 = 1.5;

#[derive(Clone, Debug)]
pub struct StrokeStyle {
    /// How wide the stroke is in SVG output (Darkly's marks are as wide as
    /// its brush).
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
    /// Ink pooling: how much heavier strokes get toward the bottom of each
    /// path (each glyph, for text), as a fraction (0.3 is 30% heavier at
    /// the very bottom), rising over the lowest `pool_depth` of its height.
    pub pool: f32,
    pub pool_depth: f32,
    /// How much pressure wanders along strokes, as a fraction either way,
    /// and over about how many canvas units it rises and falls.
    pub pressure_variation: f32,
    pub variation_length: f32,
}

impl Brush {
    /// How much heavier a stroke is at height `y` in a path reaching from
    /// `top` to `bottom` (`y` down): 1 above its lowest `pool_depth`, rising
    /// smoothly to `1 + pool` at the bottom. Paths with no height don't
    /// pool.
    pub fn pooling(&self, y: f32, top: f32, bottom: f32) -> f32 {
        let height = bottom - top;
        if self.pool == 0.0 || !(height > 1e-3) {
            return 1.0;
        }
        let depth = (self.pool_depth * height).max(1e-6);
        let s = ((y - (bottom - depth)) / depth).clamp(0.0, 1.0);
        1.0 + self.pool * s * s * (3.0 - 2.0 * s)
    }
    /// The pressure's wandering along one stroke, seeded with `seed`, or
    /// `None` if it doesn't.
    pub fn variation(&self, seed: u32) -> Option<Variation> {
        (self.pressure_variation > 0.0).then(|| Variation {
            noise: Perlin::new(seed),
            amount: self.pressure_variation,
            length: self.variation_length.max(1e-3),
        })
    }
    /// The heaviest a stroke gets, as a multiple of its pressure profile:
    /// pooled and varied at once.
    pub fn heaviest(&self) -> f32 {
        (1.0 + self.pool) * (1.0 + self.pressure_variation)
    }
}

/// How a stroke's pressure wanders along it: smooth noise, rising and
/// falling about every `length` units.
pub struct Variation {
    noise: Perlin,
    amount: f32,
    length: f32,
}

impl Variation {
    /// The multiple of its pressure a stroke has `distance` units along
    /// it, within `1 ± amount`: two octaves of Perlin noise.
    pub fn at(&self, distance: f32) -> f32 {
        let x = (distance / self.length) as f64;
        // Perlin noise is 0 on whole numbers, so it's read off them. The
        // octaves rarely reach past ±0.9 together; that's stretched to ±1.
        let n = self.noise.get([x + 0.31, 0.47]) + 0.5 * self.noise.get([2.0 * x + 0.73, 3.19]);
        let n = (n as f32 / 0.9).clamp(-1.0, 1.0);
        (1.0 + self.amount * n).max(0.0)
    }
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
    /// The stroke colour as sRGB RGBA floats in 0-1: a gradient's first.
    pub fn rgba(&self) -> Result<[f32; 4], String> {
        match &self.color {
            StrokeColor::Solid(color) => parse_color(color),
            StrokeColor::Gradient(g) => Ok(g.first()),
        }
    }
    /// How far a stroke's marks reach either side of its line: half the
    /// wider of its width at its heaviest (in SVG output) and its brush (in
    /// Darkly's).
    pub fn reach(&self) -> f32 {
        let (brush, heaviest) = self.brush.as_ref().map_or((0.0, 1.0), |b| (b.size, b.heaviest()));
        (self.width * heaviest).max(brush).max(0.0) / 2.0
    }
}

/// Parse a CSS color string (named, #hex, rgb()/rgba(), hsl()/hsla()) into
/// sRGB RGBA floats in 0-1.
pub fn parse_color(color: &str) -> Result<[f32; 4], String> {
    let c: svgtypes::Color = color.parse()
        .map_err(|e| format!("Invalid color '{}': {}", color, e))?;
    Ok([c.red, c.green, c.blue, c.alpha].map(|v| v as f32 / 255.0))
}

/// The highest and lowest of some points (y being down).
fn top_and_bottom(points: impl Iterator<Item = Vec2>) -> (f32, f32) {
    points.fold((f32::MAX, f32::MIN), |(top, bottom), p| (top.min(p.y), bottom.max(p.y)))
}

/// How far down from `top` to `bottom` height `y` is, from 0 to 1; the
/// middle if they're level.
fn down(y: f32, top: f32, bottom: f32) -> f32 {
    if bottom > top { (y - top) / (bottom - top) } else { 0.5 }
}

/// A stroke as a plain SVG path, `width` wide. `id` names its gradient,
/// if it has one across the canvas; along a stroke, it's cut into pieces,
/// each its own colour.
fn plain_path(stroke: &ColoredStroke, id: &str) -> String {
    let ColoredStroke { path, style, .. } = stroke;
    let line = |d: &dyn std::fmt::Display, paint: &str, cap: &str| format!(
        "<path d=\"{}\" style=\"fill:none;stroke:{};stroke-width:{};{}\" />", d, paint, style.width, cap);
    match &style.color {
        StrokeColor::Solid(color) => line(path, color, ""),
        StrokeColor::Gradient(gradient) if gradient.per_stroke() => {
            let mut svg = String::new();
            let subpaths = path.flattened(0.1);
            let (top, bottom) = top_and_bottom(subpaths.iter().flat_map(|(points, _)| points.iter().copied()));
            for (mut points, closed) in subpaths {
                if closed {
                    points.push(points[0]);
                }
                let lengths: Vec<f32> = std::iter::once(0.0).chain(points.windows(2).scan(0.0, |total, w| {
                    *total += w[0].distance(w[1]);
                    Some(*total)
                })).collect();
                let total = *lengths.last().expect("a subpath has a point");
                if total <= 0.0 {
                    continue;
                }
                let piece = (total / ALONG_PIECES).max(ALONG_PIECE_MIN);
                let mut i = 0;
                while i + 1 < points.len() {
                    // Points up to a piece's length on, at least one.
                    let mut end = i + 1;
                    while end + 1 < points.len() && lengths[end] - lengths[i] < piece {
                        end += 1;
                    }
                    let mut d = StrokePath::new().move_to(points[i].x, points[i].y);
                    for p in &points[i + 1..=end] {
                        d = d.line_to(p.x, p.y);
                    }
                    let t = match gradient.kind {
                        GradientKind::Vertical => down(points[i].lerp(points[end], 0.5).y, top, bottom),
                        _ => (lengths[i] + lengths[end]) / 2.0 / total,
                    };
                    svg.push_str(&line(&d, &css(gradient.at(t)), "stroke-linecap:round;stroke-linejoin:round;"));
                    i = end;
                }
            }
            svg
        }
        StrokeColor::Gradient(gradient) => format!("{}{}", svg_gradient(gradient, id), line(path, &format!("url(#{})", id), "")),
    }
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

/// How the SVG output draws strokes: roughened, sampled every
/// `point_sample_distance` units, tapered over `trail_off_percent` of each
/// end, and with noise in the rasterised image's opacity.
#[derive(Clone, Debug)]
pub struct SvgStyle {
    pub roughness: f32,
    pub point_sample_distance: f32,
    pub trail_off_percent: f32,
    pub noise_amplitude: f32,
}

impl SvgStyle {
    /// How a page's SVG output is drawn unless its script says otherwise.
    pub const DEFAULT: SvgStyle = SvgStyle {
        roughness: 1.0,
        point_sample_distance: 1.0,
        trail_off_percent: 0.15,
        noise_amplitude: 0.3,
    };
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
    /// The whole of `page`, as one tile: a page of a set size.
    pub fn whole(page: bounding_volume::Aabb) -> Frame {
        Frame { page, column: 0, columns: 1, row: 0, rows: 1, svg_seed: 0 }
    }
    /// Whether this is one tile of several.
    pub fn is_tile(&self) -> bool {
        self.columns * self.rows > 1
    }
    /// The whole page's size, as renders size a canvas: at least a unit
    /// each way.
    pub fn page_extents(&self) -> Vec2 {
        self.page.extents().max(Vec2::ONE)
    }
    /// The whole page's size in a render at `scale` pixels per unit.
    pub fn page_pixels(&self, scale: f32) -> (u32, u32) {
        let extents = self.page_extents();
        (((extents.x * scale) as u32).max(1), ((extents.y * scale) as u32).max(1))
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
    /// Where the top-left pixel of this canvas's render at `scale` pixels
    /// per unit falls on a grid of pixels from the canvas units' origin,
    /// so renders of a page's canvases can be laid together. A tile's
    /// render is placed on its whole page's grid.
    pub fn render_origin(&self, scale: f32) -> (i64, i64) {
        let px = |v: f32| (v * scale).round() as i64;
        match &self.frame {
            Some(frame) => {
                let [x0, y0, _, _] = frame.pixels(frame.page_pixels(scale));
                (px(frame.page.mins.x) + x0 as i64, px(frame.page.mins.y) + y0 as i64)
            }
            None => (px(self.bounds.mins.x), px(self.bounds.mins.y)),
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
    /// Change every stroke's colour with `f`.
    pub fn recolor(&mut self, f: impl Fn(&StrokeColor) -> StrokeColor) {
        for stroke in &mut self.strokes {
            stroke.style.color = f(&stroke.style.color);
        }
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
    fn to_svg_group(&self, style: Option<&SvgStyle>, seed: u64, min: Vec2, max: Vec2) -> String {
        let mut svg = String::new();
        let reach = style.map_or(0.0, |s| s.roughness) + 1.0;
        for (stroke, stroke_seed) in self.strokes.iter().zip(self.stroke_seeds()) {
            if !Self::stroke_reaches(stroke, min, max, reach) {
                continue;
            }
            // Ids for gradients, unique in the file.
            let id = format!("gradient_{}", stroke_seed);
            if let Some(opts) = style {
                svg.push_str(&self.stylized_path(stroke, opts, &mut stroke_rng(seed, stroke_seed), &id));
            } else {
                svg.push_str(&plain_path(stroke, &id));
            }
        }
        format!("<g>{}</g>", svg)
    }
    /// The canvas as SVG. With a style, strokes are roughened and tapered
    /// using generators seeded with `seed`, so the result is reproducible.
    pub fn to_svg(&self, stylized: Option<&SvgStyle>, seed: u64) -> String {
        self.to_svg_at(stylized, seed, 1.0).0
    }
    /// The canvas as SVG, and where its render at `scale` pixels per unit
    /// falls: the size of the image, and the page pixel of its top-left
    /// corner (only a tile's isn't 0, 0).
    fn to_svg_at(&self, stylized: Option<&SvgStyle>, seed: u64, scale: f32) -> (String, (u32, u32), (u32, u32)) {
        let (view, pixels, offset) = self.svg_view(scale);
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\">",
            view[2], view[3], view[0], view[1], view[2], view[3]
        );
        svg.push_str(&self.svg_group_in(stylized, seed, view));
        svg.push_str("</svg>");
        (svg, pixels, offset)
    }
    /// The canvas's strokes as an SVG group in canvas units, and the box
    /// of them its SVG shows, `[x, y, width, height]`: `to_svg` in parts,
    /// to lay several canvases together.
    pub fn to_svg_view(&self, stylized: Option<&SvgStyle>, seed: u64, scale: f32) -> (String, [f32; 4]) {
        let (view, _, _) = self.svg_view(scale);
        (self.svg_group_in(stylized, seed, view), view)
    }
    /// The box of canvas units the canvas's SVG shows, `[x, y, width,
    /// height]` (see `to_svg_view`), without drawing it.
    pub fn svg_view_box(&self, scale: f32) -> [f32; 4] {
        self.svg_view(scale).0
    }
    fn svg_group_in(&self, stylized: Option<&SvgStyle>, seed: u64, view: [f32; 4]) -> String {
        self.to_svg_group(stylized, seed, Vec2::new(view[0], view[1]), Vec2::new(view[0] + view[2], view[1] + view[3]))
    }
    /// The box of canvas units the SVG shows, `[x, y, width, height]`, the
    /// size of its render at `scale` pixels per unit, and the page pixel of
    /// that render's top-left corner (only a tile's isn't 0, 0).
    fn svg_view(&self, scale: f32) -> ([f32; 4], (u32, u32), (u32, u32)) {
        let (min, max) = (self.bounds.mins, self.bounds.maxs);
        let invalid_bounds = min.x > max.x || min.y > max.y;
        if let Some(frame) = &self.frame {
            // The tile's pixels of the whole page's render, and the part of
            // the page they show.
            let [x0, y0, x1, y1] = frame.pixels(frame.page_pixels(scale));
            let origin = frame.page.mins + Vec2::new(x0 as f32, y0 as f32) / scale;
            let size = Vec2::new((x1 - x0) as f32, (y1 - y0) as f32) / scale;
            ([origin.x, origin.y, size.x, size.y], (x1 - x0, y1 - y0), (x0, y0))
        } else if !invalid_bounds {
            let size = self.render_extents();
            // Center the bounds in the (possibly larger) render area.
            let origin = min - (size - (max - min)) / 2.0;
            ([origin.x, origin.y, size.x, size.y], (((size.x * scale) as u32).max(1), ((size.y * scale) as u32).max(1)), (0, 0))
        } else {
            ([0.0, 0.0, 100.0, 100.0], (((100.0 * scale) as u32).max(1), ((100.0 * scale) as u32).max(1)), (0, 0))
        }
    }
    /// Vary the opacity of a render with Perlin noise seeded by `seed`, read
    /// at page pixel `offset` plus each pixel's, so tiles' noise lines up.
    pub fn stylize_pixmap(pixmap: &mut tiny_skia::Pixmap, stylized: &SvgStyle, seed: u64, offset: (u32, u32)) {
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
    pub fn to_png(&self, scale: f32, stylized: Option<&SvgStyle>, seed: u64) -> Vec<u8> {
        self.to_svg_and_png(scale, stylized, seed).1
    }
    /// The canvas as SVG and that SVG rasterised at `scale` pixels per unit
    /// (with the style's noise added); see `to_svg` for `seed`.
    pub fn to_svg_and_png(&self, scale: f32, stylized: Option<&SvgStyle>, seed: u64) -> (String, Vec<u8>) {
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
    /// A stroke as SVG, roughened and tapered: each subpath an outline,
    /// filled. `id` names its gradient, if it has one across the canvas;
    /// along a stroke, its outline is cut into pieces, each its own colour.
    pub fn stylized_path(&self, stroke: &ColoredStroke, opts: &SvgStyle, rng: &mut dyn RngCore, id: &str) -> String {
        let pos_roughen = opts.roughness;
        let distance = opts.point_sample_distance;
        let trail_off_percent = opts.trail_off_percent;
        let mut path = stroke.path.to_cubics();
        let halfwidth = stroke.style.width / 2.0;
        path.adjust_by(|&cmd| {
            cmd.pos_roughen_by(pos_roughen, rng)
        });
        // Each subpath's sides: a point either side of each sample, as far
        // as the trailed-off width, pooled toward the bottom of the whole
        // path and varied along it, if its brush says so.
        let brush = stroke.style.brush.as_ref();
        let substrokes: Vec<_> = path.to_substrokes().iter().map(|s| s.get_point_samples(distance)).collect();
        let (top, bottom) = top_and_bottom(substrokes.iter().flatten().map(|s| s.point));
        let variation_seed = brush.filter(|b| b.pressure_variation > 0.0).map(|_| rng.next_u32());
        let mut outlines = Vec::new();
        for (k, samples) in substrokes.into_iter().enumerate() {
            if samples.len() < 2 {
                continue;
            }
            let last = (samples.len() - 1) as f32;
            let trail = |i: usize| {
                let percentile = i as f32 / last;
                if percentile < trail_off_percent {
                    halfwidth * (percentile / trail_off_percent)
                } else if percentile > (1.0 - trail_off_percent) {
                    halfwidth * ((1.0 - percentile) / trail_off_percent)
                } else {
                    halfwidth
                }
            };
            let variation = brush.zip(variation_seed).and_then(|(b, seed)| b.variation(seed.wrapping_add(k as u32)));
            let mut along = 0.0;
            let heavier: Vec<f32> = samples.iter().enumerate().map(|(i, sample)| {
                along += if i > 0 { samples[i - 1].point.distance(sample.point) } else { 0.0 };
                brush.map_or(1.0, |b| b.pooling(sample.point.y, top, bottom))
                    * variation.as_ref().map_or(1.0, |v| v.at(along))
            }).collect();
            // Reversed, the right side's trail runs from the other end.
            let n = samples.len();
            let sides: Vec<(Vec2, Vec2)> = samples.iter().enumerate()
                .map(|(i, sample)| (sample.point + trail(i) * heavier[i] * sample.normal, sample.point - trail(n - 1 - i) * heavier[i] * sample.normal))
                .collect();
            outlines.push((samples[0].point, sides));
        }
        let outline = |start: Option<Vec2>, sides: &[(Vec2, Vec2)]| {
            let mut path = StrokePath::new();
            let first = start.unwrap_or(sides[0].0);
            path = path.move_to(first.x, first.y);
            for (left, _) in sides {
                path = path.line_to(left.x, left.y);
            }
            for (_, right) in sides.iter().rev() {
                path = path.line_to(right.x, right.y);
            }
            path.close_path()
        };
        match &stroke.style.color {
            StrokeColor::Gradient(gradient) if gradient.per_stroke() => {
                // Pieces a few samples long, overlapping by one so they
                // meet, each in the colour at its middle: how far along
                // it is, or down the whole path.
                let mut svg = String::new();
                for (_, sides) in &outlines {
                    let last = (sides.len() - 1) as f32;
                    let step = ((sides.len() as f32 / ALONG_PIECES).ceil() as usize).max(2);
                    let mut i = 0;
                    while i + 1 < sides.len() {
                        let end = (i + step).min(sides.len() - 1);
                        let t = match gradient.kind {
                            GradientKind::Vertical => {
                                let (l, r) = sides[(i + end) / 2];
                                down((l.y + r.y) / 2.0, top, bottom)
                            }
                            _ => (i + end) as f32 / 2.0 / last,
                        };
                        let colour = css(gradient.at(t));
                        svg.push_str(&format!("<path d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"0.3\" />", outline(None, &sides[i..=end]), colour, colour));
                        i = end;
                    }
                }
                svg
            }
            color => {
                let mut path = StrokePath::new();
                for (start, sides) in &outlines {
                    path.append(outline(Some(*start), sides));
                }
                let (defs, paint) = match color {
                    StrokeColor::Solid(c) => (String::new(), c.clone()),
                    StrokeColor::Gradient(g) => (svg_gradient(g, id), format!("url(#{})", id)),
                };
                format!("{}<path d=\"{}\" fill=\"{}\" stroke=\"{}\" />", defs, path, paint, paint)
            }
        }
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
    fn light_inks_darken_for_paper_and_dark_ones_stay() {
        let ink = |c: &str| StrokeColor::from(c).as_ink_on_paper();
        assert_eq!(ink("#f2ead8"), StrokeColor::from("rgba(39, 31, 13, 1)"));
        assert_eq!(ink("rgba(255, 138, 122, 0.4)"), StrokeColor::from("rgba(133, 16, 0, 0.4)"));
        for dark in ["black", "darkred", "rgb(40, 60, 90)"] {
            assert_eq!(parse_color(&css(dark_ink(parse_color(dark).unwrap()))).unwrap(), parse_color(dark).unwrap(), "{}", dark);
        }
        assert_eq!(ink("not a colour"), StrokeColor::from("not a colour"));
        let StrokeColor::Gradient(g) = gradient(GradientKind::Along).as_ink_on_paper() else { panic!("still a gradient") };
        assert_eq!(g.stops[0].1, [1.0, 0.0, 0.0, 1.0], "pure red is a mid-tone, so it stays");
    }

    fn gradient(kind: GradientKind) -> StrokeColor {
        StrokeColor::Gradient(Gradient { stops: vec![(0.0, [1.0, 0.0, 0.0, 1.0]), (1.0, [0.0, 0.0, 1.0, 1.0])], kind })
    }

    #[test]
    fn gradients_in_svg() {
        let wave = StrokePath::new().move_to(0.0, 0.0).cubic_curve_to(30.0, -20.0, 60.0, 20.0, 90.0, 0.0);
        let opts = SvgStyle { roughness: 0.5, point_sample_distance: 1.0, trail_off_percent: 0.15, noise_amplitude: 0.0 };
        for styled in [None, Some(&opts)] {
            // Along a stroke: pieces from red to blue.
            let mut canvas = Canvas::new();
            canvas.add_strokes(wave.clone(), StrokeStyle { width: 2.0, color: gradient(GradientKind::Along), brush: None });
            let svg = canvas.to_svg(styled, 1);
            assert!(svg.matches("<path").count() > 10, "{}", svg);
            let reds: Vec<u8> = svg.split("rgba(").skip(1).map(|c| c.split(',').next().unwrap().parse().unwrap()).collect();
            assert!(reds[0] > 200 && *reds.last().unwrap() < 50 && reds.windows(2).all(|w| w[0] >= w[1]), "{:?}", reds);
            canvas.to_png(1.0, styled, 1);
            // Across the canvas: an SVG gradient the path is painted with.
            let mut canvas = Canvas::new();
            let kind = GradientKind::Linear { from: Vec2::new(0.0, 0.0), to: Vec2::new(90.0, 0.0) };
            canvas.add_strokes(wave.clone(), StrokeStyle { width: 2.0, color: gradient(kind), brush: None });
            let svg = canvas.to_svg(styled, 1);
            assert!(svg.contains("<linearGradient id=\"gradient_0\"") && svg.contains("url(#gradient_0)"), "{}", svg);
            canvas.to_png(1.0, styled, 1);
        }
    }

    #[test]
    fn vertical_gradients_run_down_each_path() {
        // An S down from y = 0 to 60, swinging back and forth: pieces go
        // from red at the top to blue at the bottom.
        let zigzag = StrokePath::new().move_to(0.0, 0.0).cubic_curve_to(60.0, 10.0, -60.0, 50.0, 0.0, 60.0);
        let opts = SvgStyle { roughness: 0.0, point_sample_distance: 1.0, trail_off_percent: 0.15, noise_amplitude: 0.0 };
        for styled in [None, Some(&opts)] {
            let mut canvas = Canvas::new();
            canvas.add_strokes(zigzag.clone(), StrokeStyle { width: 2.0, color: gradient(GradientKind::Vertical), brush: None });
            let svg = canvas.to_svg(styled, 1);
            let reds: Vec<u8> = svg.split("rgba(").skip(1).map(|c| c.split(',').next().unwrap().parse().unwrap()).collect();
            assert!(reds[0] > 200 && *reds.last().unwrap() < 50, "{:?}", reds);
            assert!(reds.windows(2).all(|w| w[0] >= w[1]), "never back up the path: {:?}", reds);
            canvas.to_png(1.0, styled, 1);
        }
    }

    #[test]
    fn styled_output_follows_the_seed() {
        let mut canvas = Canvas::new();
        let style = StrokeStyle { width: 2.0, color: "black".into(), brush: None };
        canvas.add_strokes(StrokePath::new().move_to(0.0, 0.0).cubic_curve_to(10.0, 0.0, 20.0, 10.0, 30.0, 30.0), style);
        let opts = SvgStyle { roughness: 2.0, point_sample_distance: 1.0, trail_off_percent: 0.15, noise_amplitude: 1.5 };
        assert_eq!(canvas.to_png(1.0, Some(&opts), 7), canvas.to_png(1.0, Some(&opts), 7));
        assert_ne!(canvas.to_png(1.0, Some(&opts), 7), canvas.to_png(1.0, Some(&opts), 8));
        assert_eq!(canvas.to_svg(Some(&opts), 7), canvas.to_svg(Some(&opts), 7));
    }
}

#[cfg(test)]
mod pressure_tests {
    use super::{Brush, PressureProfile};

    fn brush(pool: f32, pressure_variation: f32) -> Brush {
        Brush { name: "Ink Pen".into(), size: 1.0, min_pressure: 0.2, max_pressure: 0.8, pressure_profile: PressureProfile::Ramp, pool, pool_depth: 0.25, pressure_variation, variation_length: 10.0 }
    }

    #[test]
    fn ink_pools_toward_the_bottom() {
        let b = brush(0.4, 0.0);
        // A path from y = 0 to 100 (down): untouched above its lowest 25.
        assert_eq!(b.pooling(0.0, 0.0, 100.0), 1.0);
        assert_eq!(b.pooling(75.0, 0.0, 100.0), 1.0);
        assert!((b.pooling(100.0, 0.0, 100.0) - 1.4).abs() < 1e-6);
        let rising: Vec<f32> = (75..=100).map(|y| b.pooling(y as f32, 0.0, 100.0)).collect();
        assert!(rising.windows(2).all(|w| w[1] >= w[0]), "{:?}", rising);
        assert_eq!(b.pooling(3.0, 3.0, 3.0), 1.0, "a level path doesn't pool");
        assert_eq!(brush(0.0, 0.0).pooling(100.0, 0.0, 100.0), 1.0);
    }

    #[test]
    fn pressure_varies_smoothly_within_its_bounds() {
        assert!(brush(0.0, 0.0).variation(1).is_none());
        let v = brush(0.0, 0.3).variation(7).unwrap();
        let along: Vec<f32> = (0..2000).map(|i| v.at(i as f32 * 0.1)).collect();
        assert!(along.iter().all(|m| (0.7..=1.3).contains(m)), "within 30% either way");
        let (lo, hi) = along.iter().fold((f32::MAX, f32::MIN), |(a, b), m| (a.min(*m), b.max(*m)));
        assert!(lo < 0.9 && hi > 1.1, "it wanders: {}..{}", lo, hi);
        assert!(along.windows(2).all(|w| (w[1] - w[0]).abs() < 0.03), "smoothly");
        assert_eq!(v.at(12.5), brush(0.0, 0.3).variation(7).unwrap().at(12.5), "seeded");
        assert_ne!(v.at(12.5), brush(0.0, 0.3).variation(8).unwrap().at(12.5));
    }

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
