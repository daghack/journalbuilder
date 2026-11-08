//! Fluids: a frozen fluid's field (from the flow simulator, kept in a
//! journal's `fluids/NAME.json`) laid over a box of the canvas, and how it
//! distorts the strokes drawn there.
//!
//! - **position**: each point carried along the flow for a time
//!   (`advect`), pushed by the velocity there (`offset`), or pushed up a
//!   channel's slope (`gradient`);
//! - **jitter**: smooth wobble, as strong as a channel is;
//! - **weight**, **opacity** and **color**: how wide, how strong and what
//!   colour the ink is at each point, by a channel.
//!
//! Position and jitter move a stroke's points when the fluid is applied
//! (`apply`); weight, opacity and colour stay with the stroke
//! (`ColoredStroke::fluids`) and are read point by point as it's drawn, in
//! SVG and in Darkly alike. A fluid acts only inside its box.

use flowsim::field::{Channel, EncodedField, Field};
use noise::{NoiseFn, Perlin};
use parry2d::math::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::journal::canvas::{parse_color, Canvas, ColoredStroke};
use crate::journal::gradient::{Gradient, GradientKind};
use crate::journal::paths::StrokePath;

pub use flowsim::field::Channel as FluidChannel;

/// How a fluid moves points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum By {
    /// Carried along the flow for `amount` seconds.
    #[default]
    Advect,
    /// Moved by the velocity there times `amount` (seconds).
    Offset,
    /// Moved up `channel`'s slope, `amount` units where it's steepest.
    Gradient,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PositionDistort {
    pub by: By,
    pub amount: f32,
    /// The channel `gradient` climbs.
    pub channel: Channel,
    /// How far apart points are put along strokes before they're moved.
    pub step: f32,
}

impl Default for PositionDistort {
    fn default() -> Self {
        PositionDistort { by: By::Advect, amount: 1.0, channel: Channel::Dye, step: 2.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct JitterDistort {
    /// What sets how strong it is; `null` for the same everywhere.
    pub channel: Option<Channel>,
    /// How far points stray where the channel is at its top.
    pub amount: f32,
    /// About how far apart its turns are.
    pub length: f32,
    pub range: Option<[f32; 2]>,
    pub curve: f32,
}

impl Default for JitterDistort {
    fn default() -> Self {
        JitterDistort { channel: Some(Channel::Speed), amount: 3.0, length: 20.0, range: None, curve: 1.0 }
    }
}

/// A multiple from `from` (where the channel is at the bottom of its
/// range) to `to` (at its top).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct WeightDistort {
    pub channel: Option<Channel>,
    pub from: f32,
    pub to: f32,
    pub range: Option<[f32; 2]>,
    pub curve: f32,
}

impl Default for WeightDistort {
    fn default() -> Self {
        WeightDistort { channel: Some(Channel::Speed), from: 1.0, to: 2.0, range: None, curve: 1.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct OpacityDistort {
    pub channel: Option<Channel>,
    pub from: f32,
    pub to: f32,
    pub range: Option<[f32; 2]>,
    pub curve: f32,
}

impl Default for OpacityDistort {
    fn default() -> Self {
        OpacityDistort { channel: Some(Channel::Speed), from: 0.3, to: 1.0, range: None, curve: 1.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ColorDistort {
    pub channel: Option<Channel>,
    /// The colours from the bottom of the range to its top, evenly spaced.
    pub colors: Vec<String>,
    /// How far toward them the ink's own colour goes, from 0 to 1.
    pub mix: f32,
    pub range: Option<[f32; 2]>,
    pub curve: f32,
}

impl Default for ColorDistort {
    fn default() -> Self {
        ColorDistort { channel: Some(Channel::Dye), colors: vec!["#1d1d1f".into(), "#c0392b".into()], mix: 1.0, range: None, curve: 1.0 }
    }
}

/// The effects a fluid has, each `None` when off.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Effects {
    pub position_distort: Option<PositionDistort>,
    pub jitter_distort: Option<JitterDistort>,
    pub weight_distort: Option<WeightDistort>,
    pub opacity_distort: Option<OpacityDistort>,
    pub color_distort: Option<ColorDistort>,
}

/// Every effect's name, as files and scripts give them.
pub const EFFECTS: [&str; 5] = ["position_distort", "jitter_distort", "weight_distort", "opacity_distort", "color_distort"];

/// A fluid file, `fluids/NAME.json`, as journals read it. The Fluid tab
/// keeps more there (its scene and preview), which this leaves alone.
#[derive(Clone, Debug, Deserialize)]
pub struct FluidFile {
    #[serde(default)]
    pub at: Option<[f32; 2]>,
    #[serde(default)]
    pub size: Option<[f32; 2]>,
    #[serde(default)]
    pub field: Option<EncodedField>,
    #[serde(flatten)]
    pub effects: Effects,
}

/// A field, decoded, and what's worked out from it once.
#[derive(Debug)]
pub struct FluidField {
    pub field: Field,
    /// Displacement maps by how long points are carried, and the steepest
    /// slope of each channel.
    advected: Mutex<HashMap<u32, Arc<Vec<Vec2>>>>,
    steepest: Mutex<HashMap<&'static str, f32>>,
}

impl FluidField {
    pub fn new(field: Field) -> Self {
        FluidField { field, advected: Mutex::default(), steepest: Mutex::default() }
    }
    /// A field from the flow simulator's raw arrays (see
    /// `Field::from_raw`), as the Fluid tab sends them live.
    pub fn from_raw(nx: usize, ny: usize, h: f32, u: &[f32], v: &[f32], pressure: &[f32], dye: &[f32]) -> Result<Self, String> {
        Field::from_raw(nx, ny, h, u, v, pressure, dye).map(FluidField::new)
    }
}

/// A fluid laid over a box of the canvas, with its effects.
#[derive(Clone, Debug)]
pub struct Fluid {
    pub name: String,
    pub field: Arc<FluidField>,
    /// The box's top-left corner and size, in canvas units.
    pub at: Vec2,
    pub size: Vec2,
    pub effects: Effects,
}

fn pv(p: flowsim::V2) -> Vec2 {
    Vec2::new(p.x, p.y)
}

/// A channel's value scaled to 0..1 over `range` (or the field's own),
/// then curved.
fn level(field: &Field, channel: Option<Channel>, range: Option<[f32; 2]>, curve: f32, q: flowsim::V2) -> f32 {
    let Some(channel) = channel else { return 1.0 };
    let (lo, hi) = range.map_or_else(|| field.range(channel), |[lo, hi]| (lo, hi));
    let value = field.sample(channel, q);
    let t = if hi > lo { ((value - lo) / (hi - lo)).clamp(0.0, 1.0) } else { 0.0 };
    if curve > 0.0 && curve != 1.0 { t.powf(curve) } else { t }
}

impl Fluid {
    /// A fluid from a file's text, named `name`.
    pub fn from_file(name: &str, text: &str) -> Result<Fluid, String> {
        let file: FluidFile = serde_json::from_str(text).map_err(|e| format!("fluids/{}.json: {}", name, e))?;
        let encoded = file.field.ok_or_else(|| format!("fluids/{}.json has no field yet: open it in the Fluid tab and freeze the flow", name))?;
        let field = Field::decode(&encoded).map_err(|e| format!("fluids/{}.json: {}", name, e))?;
        let size = field.size();
        let at = file.at.map_or(Vec2::ZERO, |[x, y]| Vec2::new(x, y));
        let size = file.size.map_or(Vec2::new(size.x, size.y), |[w, h]| Vec2::new(w, h));
        let fluid = Fluid { name: name.to_string(), field: Arc::new(FluidField::new(field)), at, size, effects: file.effects };
        fluid.check()?;
        Ok(fluid)
    }

    /// A fluid of `field`, with the box and effects a fluid file's text
    /// (`settings`) gives; its own field, if it has one, is ignored.
    pub fn with_settings(name: &str, field: Arc<FluidField>, settings: &str) -> Result<Fluid, String> {
        let file: FluidFile = serde_json::from_str(settings).map_err(|e| format!("fluid '{}': {}", name, e))?;
        let size = field.field.size();
        let fluid = Fluid {
            name: name.to_string(),
            field,
            at: file.at.map_or(Vec2::ZERO, |[x, y]| Vec2::new(x, y)),
            size: file.size.map_or(Vec2::new(size.x, size.y), |[w, h]| Vec2::new(w, h)),
            effects: file.effects,
        };
        fluid.check()?;
        Ok(fluid)
    }

    /// A fluid of a field given whole, as the Fluid tab sends it live.
    pub fn from_field(name: &str, field: Field, at: Vec2, size: Vec2, effects: Effects) -> Result<Fluid, String> {
        let fluid = Fluid { name: name.to_string(), field: Arc::new(FluidField::new(field)), at, size, effects };
        fluid.check()?;
        Ok(fluid)
    }

    /// Whether its settings make sense, or what's wrong with them.
    pub fn check(&self) -> Result<(), String> {
        if !(self.size.x > 0.0 && self.size.y > 0.0 && self.size.is_finite() && self.at.is_finite()) {
            return Err(format!("fluid '{}': its box must have a positive, finite size, got {} by {}", self.name, self.size.x, self.size.y));
        }
        let e = &self.effects;
        if let Some(p) = &e.position_distort {
            if !(p.step > 0.0 && p.step.is_finite()) {
                return Err(format!("fluid '{}': position_distort's step must be positive, got {}", self.name, p.step));
            }
            if !p.amount.is_finite() {
                return Err(format!("fluid '{}': position_distort's amount must be a number", self.name));
            }
        }
        if let Some(j) = &e.jitter_distort
            && !(j.length > 0.0 && j.length.is_finite()) {
            return Err(format!("fluid '{}': jitter_distort's length must be positive, got {}", self.name, j.length));
        }
        let curves = [
            ("jitter_distort", e.jitter_distort.as_ref().map(|j| j.curve)),
            ("weight_distort", e.weight_distort.as_ref().map(|w| w.curve)),
            ("opacity_distort", e.opacity_distort.as_ref().map(|o| o.curve)),
            ("color_distort", e.color_distort.as_ref().map(|c| c.curve)),
        ];
        for (effect, curve) in curves {
            if let Some(curve) = curve
                && !(curve > 0.0 && curve.is_finite()) {
                return Err(format!("fluid '{}': {}'s curve must be positive, got {}", self.name, effect, curve));
            }
        }
        if let Some(w) = &e.weight_distort
            && !(w.from >= 0.0 && w.to >= 0.0 && w.from.is_finite() && w.to.is_finite()) {
            return Err(format!("fluid '{}': weight_distort's from and to can't be negative", self.name));
        }
        if let Some(o) = &e.opacity_distort
            && !(o.from >= 0.0 && o.to >= 0.0 && o.from.is_finite() && o.to.is_finite()) {
            return Err(format!("fluid '{}': opacity_distort's from and to can't be negative", self.name));
        }
        if let Some(c) = &e.color_distort {
            if c.colors.is_empty() {
                return Err(format!("fluid '{}': color_distort needs at least one colour", self.name));
            }
            for colour in &c.colors {
                parse_color(colour).map_err(|err| format!("fluid '{}': color_distort: {}", self.name, err))?;
            }
        }
        Ok(())
    }

    /// Where `p` (canvas units) is in the field, if it's in the box.
    fn to_field(&self, p: Vec2) -> Option<flowsim::V2> {
        let local = p - self.at;
        if local.x < 0.0 || local.y < 0.0 || local.x > self.size.x || local.y > self.size.y {
            return None;
        }
        let f = self.field.field.size();
        Some(flowsim::v2(local.x * f.x / self.size.x, local.y * f.y / self.size.y))
    }

    /// The field's units in canvas units, across and down.
    fn scale(&self) -> Vec2 {
        let f = self.field.field.size();
        Vec2::new(self.size.x / f.x, self.size.y / f.y)
    }

    /// The fluid's velocity at `p` in canvas units a second; nothing
    /// outside its box.
    pub fn velocity(&self, p: Vec2) -> Vec2 {
        self.to_field(p).map_or(Vec2::ZERO, |q| pv(self.field.field.velocity(q)) * self.scale())
    }

    /// `channel` at `p`, as the field has it (not scaled to a range); 0
    /// outside the box.
    pub fn sample(&self, channel: Channel, p: Vec2) -> f32 {
        self.to_field(p).map_or(0.0, |q| self.field.field.sample(channel, q))
    }

    /// Moves and jitters `stroke`'s points, then gives it the fluid's
    /// weight, opacity and colour to be drawn with. `seed` is its
    /// texture's (see `ColoredStroke::seed`), which seeds its jitter.
    pub fn apply(&self, stroke: &mut ColoredStroke, seed: u32) {
        let e = &self.effects;
        if e.position_distort.is_some() || e.jitter_distort.is_some() {
            let step = e.position_distort.as_ref().map_or(2.0, |p| p.step);
            let advected = e.position_distort.as_ref().filter(|p| p.by == By::Advect).map(|p| self.advected(p.amount));
            let steepest = e.position_distort.as_ref().filter(|p| p.by == By::Gradient).map(|p| self.steepest(p.channel));
            let jitter = e.jitter_distort.as_ref().map(|j| {
                let hash = crate::journal::stable_hash(&self.name) as u32;
                (j, Perlin::new(hash ^ seed.wrapping_mul(0x9e37_79b9)), Perlin::new(hash.rotate_left(16) ^ seed.wrapping_add(0x51ed_270b)))
            });
            let mut path = StrokePath::new();
            for (points, closed) in stroke.path.flattened(0.25) {
                let points = resample(&points, closed, step);
                for (i, p) in points.into_iter().enumerate() {
                    let mut p = p;
                    if let (Some(position), Some(q)) = (&e.position_distort, self.to_field(p)) {
                        p += match position.by {
                            By::Advect => advected.as_ref().map_or(Vec2::ZERO, |map| self.displacement(map, q)),
                            By::Offset => pv(self.field.field.velocity(q)) * self.scale() * position.amount,
                            By::Gradient => {
                                let slope = pv(self.field.field.gradient(position.channel, q)) / self.scale();
                                let top = steepest.unwrap_or(0.0);
                                if top > 0.0 { slope / top * position.amount } else { Vec2::ZERO }
                            }
                        };
                    }
                    if let Some((j, nx, ny)) = &jitter
                        && let Some(q) = self.to_field(p) {
                        let strength = j.amount * level(&self.field.field, j.channel, j.range, j.curve, q);
                        let at = [(p.x / j.length) as f64 + 0.37, (p.y / j.length) as f64 + 0.71];
                        p += Vec2::new(nx.get(at) as f32, ny.get(at) as f32) * strength;
                    }
                    path = if i == 0 { path.move_to(p.x, p.y) } else { path.line_to(p.x, p.y) };
                }
                if closed {
                    path = path.close_path();
                }
            }
            stroke.path = path;
        }
        if e.weight_distort.is_some() || e.opacity_distort.is_some() || e.color_distort.is_some() {
            stroke.fluids.push(Arc::new(StrokeFluid::new(self)));
        }
    }

    /// Where each of the field's cell centres ends up carried along the
    /// flow for `time` seconds, less where it started, in canvas units:
    /// worked out once for the field (RK4, steps no longer than half a
    /// cell), and read between centres for each point.
    fn advected(&self, time: f32) -> Arc<Vec<Vec2>> {
        let key = time.to_bits();
        if let Some(map) = self.field.advected.lock().expect("no panics holding it").get(&key) {
            return map.clone();
        }
        let field = &self.field.field;
        let (nx, ny, h) = (field.nx(), field.ny(), field.h());
        let top = (0..ny).flat_map(|j| (0..nx).map(move |i| (i, j)))
            .map(|(i, j)| field.grid().cell_velocity(i, j).len())
            .fold(0.0f32, f32::max);
        let steps = ((time.abs() * top / (0.5 * h)).ceil() as usize).clamp(1, 400);
        let dt = time / steps as f32;
        let size = field.size();
        let inside = |p: flowsim::V2| p.x >= 0.0 && p.y >= 0.0 && p.x <= size.x && p.y <= size.y;
        let flow = |p: flowsim::V2| if inside(p) { field.velocity(p) } else { flowsim::V2::default() };
        let scale = self.scale();
        let map: Vec<Vec2> = (0..ny).flat_map(|j| (0..nx).map(move |i| (i, j))).map(|(i, j)| {
            let start = flowsim::v2((i as f32 + 0.5) * h, (j as f32 + 0.5) * h);
            let mut p = start;
            for _ in 0..steps {
                let k1 = flow(p);
                let k2 = flow(p + k1 * (0.5 * dt));
                let k3 = flow(p + k2 * (0.5 * dt));
                let k4 = flow(p + k3 * dt);
                p += (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (dt / 6.0);
            }
            pv(p - start) * scale
        }).collect();
        let map = Arc::new(map);
        self.field.advected.lock().expect("no panics holding it").insert(key, map.clone());
        map
    }

    /// A displacement map read at field point `q`, between cell centres.
    fn displacement(&self, map: &[Vec2], q: flowsim::V2) -> Vec2 {
        let field = &self.field.field;
        let (nx, ny, h) = (field.nx(), field.ny(), field.h());
        let fx = (q.x / h - 0.5).clamp(0.0, (nx - 1) as f32);
        let fy = (q.y / h - 0.5).clamp(0.0, (ny - 1) as f32);
        let (i, j) = ((fx as usize).min(nx - 2), (fy as usize).min(ny - 2));
        let (tx, ty) = (fx - i as f32, fy - j as f32);
        let at = |i: usize, j: usize| map[i + j * nx];
        let top = at(i, j).lerp(at(i + 1, j), tx);
        let bottom = at(i, j + 1).lerp(at(i + 1, j + 1), tx);
        top.lerp(bottom, ty)
    }

    /// The steepest `channel` gets, per canvas unit, over the field's cells.
    fn steepest(&self, channel: Channel) -> f32 {
        if let Some(top) = self.field.steepest.lock().expect("no panics holding it").get(channel.name()) {
            return *top;
        }
        let field = &self.field.field;
        let scale = self.scale();
        let h = field.h();
        let top = (0..field.ny()).flat_map(|j| (0..field.nx()).map(move |i| (i, j)))
            .map(|(i, j)| (pv(field.gradient(channel, flowsim::v2((i as f32 + 0.5) * h, (j as f32 + 0.5) * h))) / scale).length())
            .fold(0.0f32, f32::max);
        self.field.steepest.lock().expect("no panics holding it").insert(channel.name(), top);
        top
    }
}

/// The ink of `canvases` as an alpha raster `nx` by `ny`, row by row, over
/// the box at `at` of `size` canvas units: how much of each cell is covered
/// (0 to 255), each stroke as wide as its marks reach. The flow simulator
/// takes it as obstacles (`Sim::set_mask`).
pub fn ink_mask<'a>(canvases: impl IntoIterator<Item = &'a Canvas>, at: [f32; 2], size: [f32; 2], nx: usize, ny: usize) -> Result<Vec<u8>, String> {
    let (at, size) = (Vec2::from(at), Vec2::from(size));
    if nx == 0 || ny == 0 || nx > 4096 || ny > 4096 || !(size.x > 0.0 && size.y > 0.0) {
        return Err(format!("a mask needs 1 to 4096 cells each way over a positive size, got {nx} x {ny} over {} by {}", size.x, size.y));
    }
    let mut pixmap = tiny_skia::Pixmap::new(nx as u32, ny as u32).ok_or("couldn't make the mask's raster")?;
    let (sx, sy) = (nx as f32 / size.x, ny as f32 / size.y);
    let transform = tiny_skia::Transform::from_row(sx, 0.0, 0.0, sy, -at.x * sx, -at.y * sy);
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    for stroke in canvases.into_iter().flat_map(|canvas| canvas.strokes()) {
        let mut builder = tiny_skia::PathBuilder::new();
        for (line, closed) in stroke.path.flattened(0.25 / sx.max(sy)) {
            builder.move_to(line[0].x, line[0].y);
            // A dot is a line no length, which round caps draw.
            for p in line.iter().skip(1).chain((line.len() == 1).then_some(&line[0])) {
                builder.line_to(p.x, p.y);
            }
            if closed {
                builder.close();
            }
        }
        let Some(path) = builder.finish() else { continue };
        let width = 2.0 * stroke.reach();
        if !(width > 0.0) {
            continue;
        }
        let pen = tiny_skia::Stroke { width, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
        pixmap.stroke_path(&path, &paint, &pen, transform, None);
    }
    Ok(pixmap.data().chunks_exact(4).map(|px| px[3]).collect())
}

/// A polyline's points no further apart than `step`, its own points kept.
pub(crate) fn resample(points: &[Vec2], closed: bool, step: f32) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(points.len());
    let Some(&first) = points.first() else { return out };
    out.push(first);
    let ends = points.windows(2).map(|w| (w[0], w[1]))
        .chain(closed.then(|| (points[points.len() - 1], first)).filter(|_| points.len() > 1));
    for (a, b) in ends {
        let pieces = ((a.distance(b) / step).ceil() as usize).clamp(1, 10_000);
        for k in 1..=pieces {
            out.push(a.lerp(b, k as f32 / pieces as f32));
        }
    }
    // A closed line comes back to its start, which closing adds again.
    if closed && out.len() > 1 {
        out.pop();
    }
    out
}

/// A fluid's weight, opacity and colour, as a stroke it distorted keeps
/// them, to read as it's drawn.
#[derive(Debug)]
pub struct StrokeFluid {
    fluid: Fluid,
    ramp: Option<Gradient>,
}

impl StrokeFluid {
    fn new(fluid: &Fluid) -> Self {
        let ramp = fluid.effects.color_distort.as_ref().map(|c| {
            let colours: Vec<[f32; 4]> = c.colors.iter().map(|s| parse_color(s).unwrap_or([0.0, 0.0, 0.0, 1.0])).collect();
            let last = (colours.len() - 1).max(1) as f32;
            Gradient { stops: colours.into_iter().enumerate().map(|(i, c)| (i as f32 / last, c)).collect(), kind: GradientKind::Along }
        });
        StrokeFluid { fluid: fluid.clone(), ramp }
    }

    /// How many times as wide the ink is at `p`.
    pub fn weight(&self, p: Vec2) -> f32 {
        match (&self.fluid.effects.weight_distort, self.fluid.to_field(p)) {
            (Some(w), Some(q)) => {
                let t = level(&self.fluid.field.field, w.channel, w.range, w.curve, q);
                w.from + (w.to - w.from) * t
            }
            _ => 1.0,
        }
    }

    /// The most `weight` can be anywhere.
    pub fn heaviest(&self) -> f32 {
        self.fluid.effects.weight_distort.as_ref().map_or(1.0, |w| w.from.max(w.to).max(1.0))
    }

    /// How many times as strong the ink is at `p`.
    pub fn opacity(&self, p: Vec2) -> f32 {
        match (&self.fluid.effects.opacity_distort, self.fluid.to_field(p)) {
            (Some(o), Some(q)) => {
                let t = level(&self.fluid.field.field, o.channel, o.range, o.curve, q);
                o.from + (o.to - o.from) * t
            }
            _ => 1.0,
        }
    }

    pub fn colours(&self) -> bool {
        self.ramp.is_some()
    }

    /// `base` (sRGB RGBA, 0-1) as the fluid colours it at `p`: mixed toward
    /// the ramp's colour there, its own opacity kept.
    pub fn colour(&self, p: Vec2, base: [f32; 4]) -> [f32; 4] {
        match (&self.fluid.effects.color_distort, &self.ramp, self.fluid.to_field(p)) {
            (Some(c), Some(ramp), Some(q)) => {
                let t = level(&self.fluid.field.field, c.channel, c.range, c.curve, q);
                let to = ramp.at(t);
                let mix = c.mix.clamp(0.0, 1.0) * to[3];
                [base[0] + (to[0] - base[0]) * mix, base[1] + (to[1] - base[1]) * mix, base[2] + (to[2] - base[2]) * mix, base[3]]
            }
            _ => base,
        }
    }
}

/// How several fluids together weight a stroke at `p`.
pub fn weight(fluids: &[Arc<StrokeFluid>], p: Vec2) -> f32 {
    fluids.iter().map(|f| f.weight(p)).product()
}

/// How several fluids together make a stroke's ink stronger at `p`.
pub fn opacity(fluids: &[Arc<StrokeFluid>], p: Vec2) -> f32 {
    fluids.iter().map(|f| f.opacity(p)).product()
}

/// `base` as several fluids colour it at `p`, in turn.
pub fn colour(fluids: &[Arc<StrokeFluid>], p: Vec2, base: [f32; 4]) -> [f32; 4] {
    fluids.iter().fold(base, |c, f| f.colour(p, c))
}

/// Whether any of the fluids colours strokes.
pub fn colours(fluids: &[Arc<StrokeFluid>]) -> bool {
    fluids.iter().any(|f| f.colours())
}

/// Whether any of the fluids fades strokes.
pub fn fades(fluids: &[Arc<StrokeFluid>]) -> bool {
    fluids.iter().any(|f| f.fluid.effects.opacity_distort.is_some())
}

/// Whether the fluids change anything as a stroke is drawn.
pub fn shades(fluids: &[Arc<StrokeFluid>]) -> bool {
    !fluids.is_empty()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use flowsim::grid::Grid;

    /// A uniform flow of `speed` units a second to the right over a 100 ×
    /// 50 box, with dye rising from 0 on the left to 1 on the right.
    pub(crate) fn rightward(speed: f32) -> Field {
        let mut g = Grid::new(20, 10, 5.0);
        g.u.iter_mut().for_each(|u| *u = speed);
        for j in 0..g.ny {
            for i in 0..g.nx {
                let k = g.ci(i, j);
                g.dye[k] = i as f32 / (g.nx - 1) as f32;
            }
        }
        Field::from_grid(&g)
    }

    fn fluid(effects: Effects) -> Fluid {
        Fluid::from_field("test", rightward(10.0), Vec2::new(0.0, 0.0), Vec2::new(100.0, 50.0), effects).unwrap()
    }

    fn stroke(path: StrokePath) -> ColoredStroke {
        ColoredStroke {
            path,
            style: crate::journal::canvas::StrokeStyle { width: 2.0, color: "black".into(), color_jitter: None, brush: None },
            seed: None,
            ink: None,
            fluids: Vec::new(),
        }
    }

    #[test]
    fn advecting_and_offsetting_move_points_with_the_flow() {
        for by in [By::Advect, By::Offset] {
            let f = fluid(Effects { position_distort: Some(PositionDistort { by, amount: 2.0, ..Default::default() }), ..Default::default() });
            let mut s = stroke(StrokePath::line(Vec2::new(20.0, 25.0), Vec2::new(30.0, 25.0)));
            f.apply(&mut s, 0);
            let points = s.path.points_within();
            assert!((points[0] - Vec2::new(40.0, 25.0)).length() < 0.5, "{:?}: {:?}", by, points[0]);
            assert!((points[points.len() - 1] - Vec2::new(50.0, 25.0)).length() < 0.5, "{:?}", by);
            assert!(points.len() >= 6, "resampled every 2 units: {}", points.len());
        }
    }

    #[test]
    fn a_fluid_leaves_what_is_outside_its_box_alone() {
        let f = fluid(Effects { position_distort: Some(PositionDistort { by: By::Offset, ..Default::default() }), ..Default::default() });
        let mut s = stroke(StrokePath::line(Vec2::new(200.0, 25.0), Vec2::new(210.0, 25.0)));
        f.apply(&mut s, 0);
        assert_eq!(s.path.points_within()[0], Vec2::new(200.0, 25.0));
    }

    #[test]
    fn weight_opacity_and_colour_follow_a_channel() {
        let f = fluid(Effects {
            weight_distort: Some(WeightDistort { channel: Some(Channel::Dye), from: 1.0, to: 3.0, ..Default::default() }),
            opacity_distort: Some(OpacityDistort { channel: Some(Channel::Dye), from: 0.0, to: 1.0, ..Default::default() }),
            color_distort: Some(ColorDistort { colors: vec!["#000000".into(), "#ff0000".into()], ..Default::default() }),
            ..Default::default()
        });
        let mut s = stroke(StrokePath::line(Vec2::new(10.0, 25.0), Vec2::new(90.0, 25.0)));
        f.apply(&mut s, 0);
        assert_eq!(s.fluids.len(), 1);
        let (left, right) = (Vec2::new(2.5, 25.0), Vec2::new(97.5, 25.0));
        assert!((weight(&s.fluids, left) - 1.0).abs() < 0.01 && (weight(&s.fluids, right) - 3.0).abs() < 0.01);
        assert!(opacity(&s.fluids, left) < 0.01 && opacity(&s.fluids, right) > 0.99);
        let red = colour(&s.fluids, right, [0.0, 0.0, 1.0, 0.5]);
        assert!(red[0] > 0.99 && red[2] < 0.01 && red[3] == 0.5, "{red:?}");
        assert_eq!(colour(&s.fluids, Vec2::new(500.0, 0.0), [0.0, 0.0, 1.0, 1.0]), [0.0, 0.0, 1.0, 1.0], "outside, as it was");
        assert_eq!(s.fluids[0].heaviest(), 3.0);
    }

    #[test]
    fn jitter_is_the_same_each_time_and_differs_between_strokes() {
        let f = fluid(Effects { jitter_distort: Some(JitterDistort { channel: None, amount: 4.0, ..Default::default() }), ..Default::default() });
        let line = StrokePath::line(Vec2::new(10.0, 25.0), Vec2::new(90.0, 25.0));
        let (mut a, mut b, mut c) = (stroke(line.clone()), stroke(line.clone()), stroke(line));
        f.apply(&mut a, 3);
        f.apply(&mut b, 3);
        f.apply(&mut c, 4);
        assert_eq!(a.path.points_within(), b.path.points_within());
        assert_ne!(a.path.points_within(), c.path.points_within());
        assert!(a.path.points_within().iter().all(|p| (p.y - 25.0).abs() <= 4.0));
    }

    #[test]
    fn fluid_files_read_and_say_what_is_wrong() {
        let encoded = serde_json::to_string(&rightward(10.0).encode()).unwrap();
        let text = format!(r#"{{ "version": 1, "at": [10, 20], "field": {encoded}, "weight_distort": {{ "to": 4 }}, "scene": {{}} }}"#);
        let f = Fluid::from_file("ripples", &text).unwrap();
        assert_eq!((f.at, f.size), (Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0)));
        assert_eq!(f.effects.weight_distort.as_ref().unwrap().to, 4.0);
        assert_eq!(f.effects.weight_distort.as_ref().unwrap().from, 1.0, "defaults fill in");
        assert!(Fluid::from_file("empty", r#"{ "version": 1 }"#).unwrap_err().contains("no field yet"));
        let bad = format!(r#"{{ "field": {encoded}, "weight_distort": {{ "too": 4 }} }}"#);
        assert!(Fluid::from_file("bad", &bad).unwrap_err().contains("too"));
        let bad = format!(r#"{{ "field": {encoded}, "color_distort": {{ "colors": ["nope"] }} }}"#);
        assert!(Fluid::from_file("bad", &bad).unwrap_err().contains("nope"));
    }

    #[test]
    fn closed_subpaths_stay_closed() {
        let f = fluid(Effects { position_distort: Some(PositionDistort { by: By::Offset, amount: 0.5, ..Default::default() }), ..Default::default() });
        let mut s = stroke(StrokePath::circle(Vec2::new(50.0, 25.0), 10.0));
        let before = s.path.substroke_count();
        f.apply(&mut s, 0);
        assert_eq!(s.path.substroke_count(), before);
        assert!(s.path.flattened(0.1).iter().all(|(_, closed)| *closed));
    }
}
