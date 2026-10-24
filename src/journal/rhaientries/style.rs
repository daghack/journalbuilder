//! Styles for rhai scripts: what strokes are drawn in.
//!
//! ```rhai
//! // The ink: a colour (or gradient), a Darkly brush and how wide its marks are.
//! let ink = new_brush_style("#253443", "Ink Pen", 1.2);
//! let pen = new_brush_style("#253443", "Ink Pen", 1.2, #{
//!     min_pressure: 0.4, max_pressure: 0.8,   // default 0.3 and 0.7
//!     pressure_profile: "taper",              // or "ramp" (the default) or "constant"
//!     pressure_wander: 0.2,                   // pressure wandering 20% either way (default 0)
//!     pressure_wander_length: 8.0,            // rising and falling about every 8 units (default 12)
//!     pool: 0.4,                              // ink pooling: 40% heavier at the bottom (default 0)
//!     pool_depth: 0.25,                       // over the lowest quarter of each path (default 0.3)
//! });
//!
//! // The hand: how it goes over its strokes and sets glyphs and lines.
//! let hand = new_handwriting_style(#{
//!     passes: 2,                     // each stroke drawn twice (default 1)
//!     pass_end_jitter: 1.0,          // each pass's ends astray by up to 1 unit
//!     stroke_point_jitter: 0.5,      // every point of every pass astray by up to 0.5
//!     glyph_position_jitter: 1.0,    // each glyph moved up to 1 unit (or [across, down])
//!     glyph_rotation_jitter: 0.05,   // and turned up to 0.05 radians
//!     glyph_scale_jitter: 0.1,       // and scaled up to 10%, either way
//!     baseline_wander: 3.0,          // write_line's and write_block's lines wander up to 3 units
//! });
//!
//! let style = new_style(pen, hand);  // both parts, their settings flattened:
//! style.color = "#9b2318";
//! style.passes = 3;
//! let plain = default_style();       // Ink Pen, black, 1.2 wide
//! ```
//!
//! Every jitter is in canvas units (rotation in radians, scale a fraction),
//! drawn from the journal's generator when strokes are added to a canvas.
//! In SVG output a stroke is `brush_size * max_pressure` wide.

use rand::RngCore;
use rhai::{Dynamic, Engine, Map};
use std::sync::OnceLock;

use super::gradient;
use super::options::{above_zero, at_least_zero, one_of, register_settings, setting_number, Settings};
use super::RhaiResult;
use crate::journal::canvas::{Brush, PressureProfile, StrokeColor};
use crate::journal::canvas::StrokeStyle as JournalStrokeStyle;
use crate::journal::paths::StrokePath;

/// The most passes a style draws its strokes in.
pub(super) const MAX_PASSES: i64 = 64;

/// Darkly's brushes, by name, sorted.
fn brush_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names: Vec<String> = ::darkly::brush::builtin_brushes::all().into_iter().map(|b| b.metadata.name).collect();
        names.sort();
        names
    })
}

/// The ink: a colour or gradient, a Darkly brush, and how it presses.
#[derive(Debug, Clone)]
pub struct BrushStyle {
    color: StrokeColor,
    brush: String,
    brush_size: f64,
    min_pressure: f64,
    max_pressure: f64,
    pressure_profile: PressureProfile,
    pressure_wander: f64,
    pressure_wander_length: f64,
    pool: f64,
    pool_depth: f64,
}

impl BrushStyle {
    fn new(caller: &str, color: &Dynamic, brush: &Dynamic, brush_size: &Dynamic) -> RhaiResult<BrushStyle> {
        let mut style = BrushStyle {
            color: StrokeColor::Solid("black".into()),
            brush: "Ink Pen".into(),
            brush_size: 1.2,
            min_pressure: 0.3,
            max_pressure: 0.7,
            pressure_profile: PressureProfile::default(),
            pressure_wander: 0.0,
            pressure_wander_length: 12.0,
            pool: 0.0,
            pool_depth: 0.3,
        };
        style.set(caller, "color", color)?;
        style.set(caller, "brush", brush)?;
        style.set(caller, "brush_size", brush_size)?;
        Ok(style)
    }
}

impl Settings for BrushStyle {
    const NAME: &'static str = "brush_style";
    const KEYS: &'static [&'static str] = &[
        "color", "brush", "brush_size", "min_pressure", "max_pressure", "pressure_profile", "pressure_wander", "pressure_wander_length", "pool", "pool_depth",
    ];
    fn get(&self, key: &str) -> Dynamic {
        match key {
            "color" => match &self.color {
                StrokeColor::Solid(color) => color.into(),
                StrokeColor::Gradient(g) => Dynamic::from(g.clone()),
            },
            "brush" => self.brush.clone().into(),
            "brush_size" => self.brush_size.into(),
            "min_pressure" => self.min_pressure.into(),
            "max_pressure" => self.max_pressure.into(),
            "pressure_profile" => self.pressure_profile.name().into(),
            "pressure_wander" => self.pressure_wander.into(),
            "pressure_wander_length" => self.pressure_wander_length.into(),
            "pool" => self.pool.into(),
            "pool_depth" => self.pool_depth.into(),
            _ => Dynamic::UNIT,
        }
    }
    fn set(&mut self, caller: &str, key: &str, value: &Dynamic) -> RhaiResult<()> {
        match key {
            "color" => self.color = gradient::colour_or_gradient(caller, value.clone())?,
            "brush" => self.brush = one_of(caller, key, value, &brush_names().iter().map(String::as_str).collect::<Vec<_>>())?,
            "brush_size" => self.brush_size = above_zero(caller, key, value)?,
            "min_pressure" | "max_pressure" => {
                let pressure = setting_number(caller, key, value)?;
                // Darkly pressure runs from 0 to 1.
                if !(0.0..=1.0).contains(&pressure) {
                    return Err(format!("{}: '{}' must be from 0 to 1, got {}", caller, key, pressure).into());
                }
                if key == "min_pressure" { self.min_pressure = pressure } else { self.max_pressure = pressure }
            }
            "pressure_profile" => {
                let name = one_of(caller, key, value, &PressureProfile::NAMES)?;
                self.pressure_profile = PressureProfile::from_name(&name).expect("a profile's name");
            }
            "pressure_wander" => {
                let wander = at_least_zero(caller, key, value)?;
                if wander >= 1.0 {
                    return Err(format!("{}: 'pressure_wander' is a fraction either way, below 1, got {}", caller, wander).into());
                }
                self.pressure_wander = wander;
            }
            "pressure_wander_length" => self.pressure_wander_length = above_zero(caller, key, value)?,
            "pool" => self.pool = at_least_zero(caller, key, value)?,
            "pool_depth" => {
                let depth = above_zero(caller, key, value)?;
                if depth > 1.0 {
                    return Err(format!("{}: 'pool_depth' is a fraction of the height, at most 1, got {}", caller, depth).into());
                }
                self.pool_depth = depth;
            }
            _ => unreachable!("{} isn't a brush setting", key),
        }
        Ok(())
    }
    fn check(&self, caller: &str) -> RhaiResult<()> {
        if self.min_pressure > self.max_pressure {
            return Err(format!("{}: 'min_pressure' ({}) can't be above 'max_pressure' ({})", caller, self.min_pressure, self.max_pressure).into());
        }
        Ok(())
    }
}

/// The hand: how it goes over its strokes, and sets glyphs and lines.
#[derive(Debug, Clone, Default)]
pub struct HandwritingStyle {
    passes: i64,
    pass_end_jitter: f64,
    stroke_point_jitter: f64,
    /// Across and down.
    glyph_position_jitter: (f64, f64),
    glyph_rotation_jitter: f64,
    glyph_scale_jitter: f64,
    baseline_wander: f64,
}

impl HandwritingStyle {
    pub(super) fn new() -> HandwritingStyle {
        HandwritingStyle { passes: 1, ..HandwritingStyle::default() }
    }
    /// How far each placed glyph strays, across and down, at most.
    pub(super) fn glyph_position_jitter(&self) -> (f64, f64) {
        self.glyph_position_jitter
    }
    pub(super) fn glyph_rotation_jitter(&self) -> f64 {
        self.glyph_rotation_jitter
    }
    pub(super) fn glyph_scale_jitter(&self) -> f64 {
        self.glyph_scale_jitter
    }
    /// How far written lines wander from straight, at most.
    pub(super) fn baseline_wander(&self) -> f32 {
        self.baseline_wander as f32
    }
}

impl Settings for HandwritingStyle {
    const NAME: &'static str = "handwriting_style";
    const KEYS: &'static [&'static str] = &[
        "passes", "pass_end_jitter", "stroke_point_jitter", "glyph_position_jitter", "glyph_rotation_jitter", "glyph_scale_jitter", "baseline_wander",
    ];
    fn get(&self, key: &str) -> Dynamic {
        match key {
            "passes" => self.passes.into(),
            "pass_end_jitter" => self.pass_end_jitter.into(),
            "stroke_point_jitter" => self.stroke_point_jitter.into(),
            "glyph_position_jitter" => match self.glyph_position_jitter {
                (x, y) if x == y => x.into(),
                (x, y) => Dynamic::from_array(vec![x.into(), y.into()]),
            },
            "glyph_rotation_jitter" => self.glyph_rotation_jitter.into(),
            "glyph_scale_jitter" => self.glyph_scale_jitter.into(),
            "baseline_wander" => self.baseline_wander.into(),
            _ => Dynamic::UNIT,
        }
    }
    fn set(&mut self, caller: &str, key: &str, value: &Dynamic) -> RhaiResult<()> {
        match key {
            "passes" => {
                let passes = value.as_int().map_err(|t| format!("{}: 'passes' should be a whole number, got {}", caller, t))?;
                if !(1..=MAX_PASSES).contains(&passes) {
                    return Err(format!("{}: 'passes' should be from 1 to {}, got {}", caller, MAX_PASSES, passes).into());
                }
                self.passes = passes;
            }
            "pass_end_jitter" => self.pass_end_jitter = at_least_zero(caller, key, value)?,
            "stroke_point_jitter" => self.stroke_point_jitter = at_least_zero(caller, key, value)?,
            "glyph_position_jitter" => {
                self.glyph_position_jitter = match value.clone().into_array() {
                    Ok(pair) => match pair.as_slice() {
                        [x, y] => (at_least_zero(caller, key, x)?, at_least_zero(caller, key, y)?),
                        _ => return Err(format!("{}: 'glyph_position_jitter' should be a number or [across, down]", caller).into()),
                    },
                    Err(_) => {
                        let n = at_least_zero(caller, key, value)?;
                        (n, n)
                    }
                }
            }
            "glyph_rotation_jitter" => self.glyph_rotation_jitter = at_least_zero(caller, key, value)?,
            "glyph_scale_jitter" => {
                let scale = at_least_zero(caller, key, value)?;
                if scale >= 1.0 {
                    return Err(format!("{}: 'glyph_scale_jitter' is a fraction either way, below 1, got {}", caller, scale).into());
                }
                self.glyph_scale_jitter = scale;
            }
            "baseline_wander" => self.baseline_wander = at_least_zero(caller, key, value)?,
            _ => unreachable!("{} isn't a handwriting setting", key),
        }
        Ok(())
    }
}

/// What strokes are drawn in: the ink and the hand, their settings
/// flattened into one.
#[derive(Debug, Clone)]
pub struct Style {
    brush: BrushStyle,
    handwriting: HandwritingStyle,
}

impl Style {
    /// Black, in an Ink Pen 1.2 wide, by a steady hand.
    pub fn default_style() -> Style {
        let brush = BrushStyle::new("default_style", &"black".into(), &"Ink Pen".into(), &Dynamic::from(1.2)).expect("the default brush");
        Style { brush, handwriting: HandwritingStyle::new() }
    }
    /// How wide its brush's marks are.
    pub(super) fn brush_size(&self) -> f32 {
        self.brush.brush_size as f32
    }
    pub(super) fn handwriting(&self) -> &HandwritingStyle {
        &self.handwriting
    }
    #[cfg(test)]
    pub(super) fn color(&self) -> &StrokeColor {
        &self.brush.color
    }
    /// A path as this style draws it: each subpath gone over `passes`
    /// times, each pass astray by up to `pass_end_jitter` (see
    /// `StrokePath::retraced`) and each of its points by up to
    /// `stroke_point_jitter` (see `StrokePath::point_jittered`), drawn
    /// from `rng`.
    pub fn retrace(&self, path: StrokePath, rng: &mut dyn RngCore) -> StrokePath {
        let hand = &self.handwriting;
        if hand.passes == 1 && hand.pass_end_jitter == 0.0 && hand.stroke_point_jitter == 0.0 {
            return path;
        }
        let retraced = path.retraced(hand.passes as usize, hand.pass_end_jitter as f32, rng);
        if hand.stroke_point_jitter == 0.0 {
            return retraced;
        }
        retraced.point_jittered(hand.stroke_point_jitter as f32, rng)
    }
}

impl Settings for Style {
    const NAME: &'static str = "style";
    const KEYS: &'static [&'static str] = &[
        "color", "brush", "brush_size", "min_pressure", "max_pressure", "pressure_profile", "pressure_wander", "pressure_wander_length", "pool", "pool_depth",
        "passes", "pass_end_jitter", "stroke_point_jitter", "glyph_position_jitter", "glyph_rotation_jitter", "glyph_scale_jitter", "baseline_wander",
    ];
    fn get(&self, key: &str) -> Dynamic {
        if BrushStyle::KEYS.contains(&key) { self.brush.get(key) } else { self.handwriting.get(key) }
    }
    fn set(&mut self, caller: &str, key: &str, value: &Dynamic) -> RhaiResult<()> {
        if BrushStyle::KEYS.contains(&key) { self.brush.set(caller, key, value) } else { self.handwriting.set(caller, key, value) }
    }
    fn check(&self, caller: &str) -> RhaiResult<()> {
        self.brush.check(caller)
    }
}

impl From<&Style> for JournalStrokeStyle {
    fn from(style: &Style) -> Self {
        let b = &style.brush;
        JournalStrokeStyle {
            width: (b.brush_size * b.max_pressure) as f32,
            color: b.color.clone(),
            brush: Some(Brush {
                name: b.brush.clone(),
                size: b.brush_size as f32,
                min_pressure: b.min_pressure as f32,
                max_pressure: b.max_pressure as f32,
                pressure_profile: b.pressure_profile,
                pool: b.pool as f32,
                pool_depth: b.pool_depth as f32,
                pressure_variation: b.pressure_wander as f32,
                variation_length: b.pressure_wander_length as f32,
            }),
        }
    }
}

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("brush_names", || -> rhai::Array { brush_names().iter().cloned().map(Dynamic::from).collect() });

    engine.register_type_with_name::<BrushStyle>("BrushStyle");
    register_settings::<BrushStyle>(engine);
    let new_brush_style = |color: Dynamic, brush: Dynamic, brush_size: Dynamic| BrushStyle::new("new_brush_style", &color, &brush, &brush_size);
    engine.register_fn("new_brush_style", new_brush_style);
    engine.register_fn("new_brush_style", move |color: Dynamic, brush: Dynamic, brush_size: Dynamic, options: Map| {
        new_brush_style(color, brush, brush_size)?.with("new_brush_style", &options)
    });

    engine.register_type_with_name::<HandwritingStyle>("HandwritingStyle");
    register_settings::<HandwritingStyle>(engine);
    engine.register_fn("new_handwriting_style", HandwritingStyle::new);
    engine.register_fn("new_handwriting_style", |options: Map| HandwritingStyle::new().with("new_handwriting_style", &options));

    engine.register_type_with_name::<Style>("Style");
    register_settings::<Style>(engine);
    engine.register_fn("new_style", |brush: BrushStyle, handwriting: HandwritingStyle| Style { brush, handwriting });
    engine.register_fn("default_style", Style::default_style);
}
