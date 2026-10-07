//! Brushes and stroke styles for rhai scripts.

use rhai::{CustomType, Dynamic, Engine, EvalAltResult, ImmutableString, TypeBuilder};

use super::gradient;
use super::options::float;
use super::RhaiResult;
use crate::journal::canvas::{Brush, PressureProfile, StrokeColor};
use crate::journal::canvas::StrokeStyle as JournalStrokeStyle;

#[derive(Debug, Default, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct BrushSettings {
    pub(super) brush_style: ImmutableString,
    #[rhai_type(skip)]
    pub(super) min_pressure: f64,
    #[rhai_type(skip)]
    pub(super) max_pressure: f64,
    #[rhai_type(skip)]
    pub(super) size: f64,
    #[rhai_type(skip)]
    pub(super) pressure_profile: PressureProfile,
}

impl BrushSettings {
    /// `new_brush` is registered in `JournalEngine::new`, which checks the
    /// name against Darkly's brush library.
    pub fn new_brush(brush_style: &str, min_pressure: f64, max_pressure: f64, size: f64) -> Self {
        Self {
            brush_style: brush_style.into(),
            min_pressure,
            max_pressure,
            size,
            pressure_profile: PressureProfile::default(),
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
        builder.with_get_set("pressure_profile", Self::get_pressure_profile, Self::set_pressure_profile);
    }
}

impl From<BrushSettings> for Brush {
    fn from(bs: BrushSettings) -> Brush {
        Brush {
            name: bs.brush_style.to_string(),
            size: bs.size as f32,
            min_pressure: bs.min_pressure as f32,
            max_pressure: bs.max_pressure as f32,
            pressure_profile: bs.pressure_profile,
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
}

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
    /// `new_strokestyle(width, color, brush)`.
    ///
    /// `width` means different things in the two outputs:
    /// - Darkly (the default render): it multiplies the brush's pressure,
    ///   so pressure runs from `width * min_pressure` to
    ///   `width * max_pressure`. The brush's `size` sets how wide its marks
    ///   are; pressure makes them heavier (how much depends on the brush).
    ///   Darkly caps pressure at 1, so past `width = 1 / max_pressure` a
    ///   wider stroke looks no heavier, and the render warns.
    /// - SVG (`--svg`): it's the stroke's width in canvas units.
    ///
    /// `color` is a CSS colour string, or a gradient (see gradient.rs).
    pub fn new(width: Dynamic, color: Dynamic, brush: BrushSettings) -> Result<Self, Box<EvalAltResult>> {
        Ok(Self {
            width: float("new_strokestyle", "the width", &width)?,
            color: gradient::colour_or_gradient("new_strokestyle", color)?,
            brush,
        })
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
            .with_fn("new_strokestyle", Self::new)
            .with_get_set("width", |s: &mut Self| Dynamic::from(s.width), |s: &mut Self, width: Dynamic| -> RhaiResult<()> {
                s.width = float("style.width", "the width", &width)?;
                Ok(())
            })
            .with_get_set("color", Self::get_color, Self::set_color)
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
    engine.register_fn("new_brush", move |name: &str, min_pressure: Dynamic, max_pressure: Dynamic, size: Dynamic| -> RhaiResult<BrushSettings> {
        const CALLER: &str = "new_brush";
        if !brush_names.iter().any(|known| known == name) {
            return Err(format!("{}: unknown brush '{}'; available: {}", CALLER, name, brush_names.join(", ")).into());
        }
        let brush = BrushSettings::new_brush(name, float(CALLER, "min_pressure", &min_pressure)?, float(CALLER, "max_pressure", &max_pressure)?, float(CALLER, "size", &size)?);
        brush.check(CALLER)?;
        Ok(brush)
    });
    engine
        .build_type::<BrushSettings>()
        .build_type::<StrokeStyle>();
}
