//! Brushes and stroke styles for rhai scripts.
//!
//! ```rhai
//! let pen = new_brush("Ink Pen", 0.4, 0.8, 2.0);    // name, min and max pressure, size
//! pen.pressure_profile = "taper";                   // or "ramp" (the default) or "constant"
//! // Or as the brush is made: options set any property, checked alike.
//! let pen = new_brush("Ink Pen", 0.4, 0.8, 2.0, #{ pressure_profile: "taper", pool: 0.4 });
//!
//! // Ink pooling: strokes heavier toward the bottom of each path (each
//! // glyph, for text), here 40% heavier at the very bottom, rising over the
//! // lowest quarter of its height.
//! pen.pool = 0.4;                // default 0
//! pen.pool_depth = 0.25;         // default 0.3, a fraction of the height
//!
//! // Pressure wandering along each stroke, up to 20% either way, rising
//! // and falling about every 8 canvas units: organic ramps and tapers.
//! pen.pressure_variation = 0.2;  // default 0
//! pen.variation_length = 8.0;    // default 12
//!
//! let ink = new_style("#253443", pen);          // colour (or gradient), brush; width 1
//! let heavy = new_style("#253443", pen, #{ width: 1.5, passes: 2 });
//!
//! // Strokes gone over again and again, as a hand going over its writing:
//! // each stroke drawn in `passes` passes, each pass's ends straying by up
//! // to `jitter` canvas units. With one pass, jitter just roughens a hand.
//! ink.passes = 6;      // default 1
//! ink.jitter = 4.0;    // default 0
//! ```
//!
//! Passes are drawn when a canvas is rendered (or measured, or avoided), so
//! anything drawn in the style, text included, is gone over. Their jitter
//! is seeded from each stroke's own points, so it's the same every run.

use rand::rngs::StdRng;
use rand::SeedableRng;
use rhai::{CustomType, Dynamic, Engine, EvalAltResult, ImmutableString, Map, NativeCallContext, TypeBuilder};

use super::gradient;
use super::options::{apply_props, float};
use super::RhaiResult;
use crate::journal::canvas::{Brush, PressureProfile, StrokeColor};
use crate::journal::canvas::StrokeStyle as JournalStrokeStyle;
use crate::journal::paths::StrokePath;

#[derive(Debug, Default, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct BrushSettings {
    /// The Darkly brush's name (`brush.name`).
    pub(super) name: ImmutableString,
    #[rhai_type(skip)]
    pub(super) min_pressure: f64,
    #[rhai_type(skip)]
    pub(super) max_pressure: f64,
    #[rhai_type(skip)]
    pub(super) size: f64,
    #[rhai_type(skip)]
    pub(super) pressure_profile: PressureProfile,
    #[rhai_type(skip)]
    pub(super) pool: f64,
    #[rhai_type(skip)]
    pub(super) pool_depth: f64,
    #[rhai_type(skip)]
    pub(super) pressure_variation: f64,
    #[rhai_type(skip)]
    pub(super) variation_length: f64,
}

impl BrushSettings {
    /// `new_brush` is registered in `JournalEngine::new`, which checks the
    /// name against Darkly's brush library.
    pub fn new_brush(brush_style: &str, min_pressure: f64, max_pressure: f64, size: f64) -> Self {
        Self {
            name: brush_style.into(),
            min_pressure,
            max_pressure,
            size,
            pressure_profile: PressureProfile::default(),
            pool: 0.0,
            pool_depth: 0.3,
            pressure_variation: 0.0,
            variation_length: 12.0,
        }
    }
    fn get_pressure_profile(&mut self) -> String {
        self.pressure_profile.name().to_string()
    }
    /// `brush.pressure_profile = "taper"`: "ramp" (min to max, the default),
    /// "taper" (min to max at the middle and back) or "constant" (max).
    fn set_pressure_profile(&mut self, name: String) -> Result<(), Box<EvalAltResult>> {
        self.pressure_profile = PressureProfile::from_name(&name).ok_or_else(|| -> Box<EvalAltResult> {
            format!("brush.pressure_profile: unknown profile '{}'; expected one of {}", name, PressureProfile::NAMES.join(", ")).into()
        })?;
        Ok(())
    }
    /// The checks `new_brush` makes, `caller` failing them.
    fn check(&self, caller: &str) -> RhaiResult<()> {
        let (min, max) = (self.min_pressure, self.max_pressure);
        // Darkly pressure runs from 0 to 1 (see also the width warning in
        // darkly.rs).
        if !(0.0..=1.0).contains(&min) || !(0.0..=1.0).contains(&max) || min > max {
            return Err(format!("{}: pressures must satisfy 0 <= min <= max <= 1, got min {} and max {}", caller, min, max).into());
        }
        if !(self.size > 0.0) {
            return Err(format!("{}: size must be positive, got {}", caller, self.size).into());
        }
        if !(self.pool >= 0.0 && self.pool.is_finite()) {
            return Err(format!("{}: pool can't be negative, got {}", caller, self.pool).into());
        }
        if !(self.pool_depth > 0.0 && self.pool_depth <= 1.0) {
            return Err(format!("{}: pool_depth is a fraction of the height, above 0 and at most 1, got {}", caller, self.pool_depth).into());
        }
        if !(0.0..1.0).contains(&self.pressure_variation) {
            return Err(format!("{}: pressure_variation is a fraction either way, from 0 up to (not including) 1, got {}", caller, self.pressure_variation).into());
        }
        if !(self.variation_length > 0.0 && self.variation_length.is_finite()) {
            return Err(format!("{}: variation_length must be positive, got {}", caller, self.variation_length).into());
        }
        Ok(())
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        // Numbers may be whole or decimal; changed values are checked as
        // new_brush checks them.
        fn number_property(builder: &mut TypeBuilder<BrushSettings>, name: &'static str, field: fn(&mut BrushSettings) -> &mut f64) {
            builder.with_get_set(name,
                move |b: &mut BrushSettings| Dynamic::from(*field(b)),
                move |b: &mut BrushSettings, value: Dynamic| -> RhaiResult<()> {
                    let caller = format!("brush.{}", name);
                    let mut changed = b.clone();
                    *field(&mut changed) = float(&caller, name, &value)?;
                    changed.check(&caller)?;
                    *b = changed;
                    Ok(())
                });
        }
        number_property(builder, "min_pressure", |b| &mut b.min_pressure);
        number_property(builder, "max_pressure", |b| &mut b.max_pressure);
        number_property(builder, "size", |b| &mut b.size);
        number_property(builder, "pool", |b| &mut b.pool);
        number_property(builder, "pool_depth", |b| &mut b.pool_depth);
        number_property(builder, "pressure_variation", |b| &mut b.pressure_variation);
        number_property(builder, "variation_length", |b| &mut b.variation_length);
        builder.with_get_set("pressure_profile", Self::get_pressure_profile, Self::set_pressure_profile);
    }
}

impl From<BrushSettings> for Brush {
    fn from(bs: BrushSettings) -> Brush {
        Brush {
            name: bs.name.to_string(),
            size: bs.size as f32,
            min_pressure: bs.min_pressure as f32,
            max_pressure: bs.max_pressure as f32,
            pressure_profile: bs.pressure_profile,
            pool: bs.pool as f32,
            pool_depth: bs.pool_depth as f32,
            pressure_variation: bs.pressure_variation as f32,
            variation_length: bs.variation_length as f32,
        }
    }
}

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct StrokeStyle {
    #[rhai_type(skip)]
    pub(super) width: f64,
    /// A CSS colour, or a gradient.
    #[rhai_type(skip)]
    pub(super) color: StrokeColor,
    #[rhai_type(skip)]
    pub(super) brush: BrushSettings,
    /// How many times each stroke is drawn.
    #[rhai_type(skip)]
    pub(super) passes: i64,
    /// How far each pass's ends stray, in canvas units.
    #[rhai_type(skip)]
    pub(super) jitter: f64,
}

/// The most passes a style draws its strokes in.
pub(super) const MAX_PASSES: i64 = 64;

impl From<StrokeStyle> for JournalStrokeStyle {
    fn from(val: StrokeStyle) -> Self {
        JournalStrokeStyle {
            width: val.width as f32,
            color: val.color,
            brush: Some(val.brush.into()),
        }
    }
}

impl StrokeStyle {
    /// `new_style(color, brush[, #{ width, passes, jitter }])`.
    ///
    /// `width` (1 unless given) means different things in the two outputs:
    /// - Darkly (the default render): it multiplies the brush's pressure,
    ///   so pressure runs from `width * min_pressure` to
    ///   `width * max_pressure`. The brush's `size` sets how wide its marks
    ///   are; pressure makes them heavier (how much depends on the brush).
    ///   Darkly caps pressure at 1, so past `width = 1 / max_pressure` a
    ///   wider stroke looks no heavier, and the render warns.
    /// - SVG (`--svg`): it's the stroke's width in canvas units.
    ///
    /// `color` is a CSS colour string, or a gradient (see gradient.rs).
    pub fn new(color: Dynamic, brush: BrushSettings) -> Result<Self, Box<EvalAltResult>> {
        Ok(Self {
            width: 1.0,
            color: gradient::colour_or_gradient("new_style", color)?,
            brush,
            passes: 1,
            jitter: 0.0,
        })
    }
    /// A path as this style draws it: each subpath gone over `passes`
    /// times, each pass astray by up to `jitter` (see
    /// `StrokePath::retraced`), seeded from the path's own points.
    pub fn retrace(&self, path: StrokePath) -> StrokePath {
        if self.passes == 1 && self.jitter == 0.0 {
            return path;
        }
        let mut rng = StdRng::seed_from_u64(path.points_hash());
        path.retraced(self.passes as usize, self.jitter as f32, &mut rng)
    }
    /// `style.color`: the colour string, or the gradient.
    fn get_color(&mut self) -> Dynamic {
        match &self.color {
            StrokeColor::Solid(color) => color.into(),
            StrokeColor::Gradient(g) => Dynamic::from(g.clone()),
        }
    }
    fn set_color(&mut self, color: Dynamic) -> Result<(), Box<EvalAltResult>> {
        self.color = gradient::colour_or_gradient("style.color", color)?;
        Ok(())
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_style", Self::new)
            .with_fn("new_style", |ctx: NativeCallContext, color: Dynamic, brush: BrushSettings, options: Map| -> RhaiResult<Self> {
                apply_props(&ctx, "new_style", Self::new(color, brush)?, options, &["width", "passes", "jitter"])
            })
            .with_get_set("width", |s: &mut Self| Dynamic::from(s.width), |s: &mut Self, width: Dynamic| -> RhaiResult<()> {
                s.width = float("style.width", "the width", &width)?;
                Ok(())
            })
            .with_get_set("color", Self::get_color, Self::set_color)
            .with_get_set("passes", |s: &mut Self| Dynamic::from(s.passes), |s: &mut Self, passes: Dynamic| -> RhaiResult<()> {
                let passes = passes.as_int().map_err(|t| -> Box<EvalAltResult> { format!("style.passes: should be a whole number, got {}", t).into() })?;
                if !(1..=MAX_PASSES).contains(&passes) {
                    return Err(format!("style.passes: should be from 1 to {}, got {}", MAX_PASSES, passes).into());
                }
                s.passes = passes;
                Ok(())
            })
            .with_get_set("jitter", |s: &mut Self| Dynamic::from(s.jitter), |s: &mut Self, jitter: Dynamic| -> RhaiResult<()> {
                let jitter = float("style.jitter", "the jitter", &jitter)?;
                if jitter < 0.0 {
                    return Err(format!("style.jitter: can't be negative, got {}", jitter).into());
                }
                s.jitter = jitter;
                Ok(())
            })
        ;
    }
}

pub(super) fn register(engine: &mut Engine) {
    // Validate brush names when the script runs, rather than failing
    // mid-render when Darkly can't find the brush.
    let brush_names: Vec<String> = ::darkly::brush::builtin_brushes::all()
        .into_iter()
        .map(|brush| brush.metadata.name)
        .collect();
    // The names new_brush takes, sorted.
    let listed: rhai::Array = {
        let mut names = brush_names.clone();
        names.sort();
        names.into_iter().map(Dynamic::from).collect()
    };
    engine.register_fn("brush_names", move || listed.clone());
    let new_brush = move |name: &str, min_pressure: Dynamic, max_pressure: Dynamic, size: Dynamic| -> RhaiResult<BrushSettings> {
        const CALLER: &str = "new_brush";
        if !brush_names.iter().any(|known| known == name) {
            return Err(format!("{}: unknown brush '{}'; available: {}", CALLER, name, brush_names.join(", ")).into());
        }
        let brush = BrushSettings::new_brush(name, float(CALLER, "min_pressure", &min_pressure)?, float(CALLER, "max_pressure", &max_pressure)?, float(CALLER, "size", &size)?);
        brush.check(CALLER)?;
        Ok(brush)
    };
    let brush = new_brush.clone();
    engine.register_fn("new_brush", brush);
    engine.register_fn("new_brush", move |ctx: NativeCallContext, name: &str, min_pressure: Dynamic, max_pressure: Dynamic, size: Dynamic, options: Map| -> RhaiResult<BrushSettings> {
        let brush = new_brush(name, min_pressure, max_pressure, size)?;
        apply_props(&ctx, "new_brush", brush, options, &["pressure_profile", "pressure_variation", "variation_length", "pool", "pool_depth"])
    });
    engine
        .build_type::<BrushSettings>()
        .build_type::<StrokeStyle>();
}
