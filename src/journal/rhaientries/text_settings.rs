//! Text settings: the one map every text call takes, with the keys layout
//! roles take, so a role's settings can be passed straight in.
//!
//! ```rhai
//! const BODY = #{ font: font("plain"), size: 26, style: ink, line_height: 1.3 };
//! path.fill_with_text("lorem ipsum", BODY);                 // role keys it doesn't use are ignored
//! path.fill_with_text("lorem ipsum", BODY + #{ layout: "words", avoid: drawing });
//! write_line("lorem ipsum", vec2(100, 100), BODY + #{ rotation_jitter: 0.05 });
//! ```
//!
//! A call checks the keys it uses; role keys it doesn't use are ignored,
//! and any other key is an error. Every text call, and every role, takes
//! the jitters: each placed glyph moved, turned and scaled a little at
//! random, from the script's generator, as a hand sets it.
//!
//! - `position_jitter`: how far each glyph strays, at most, either way,
//!   in sizes; a number, or `[across, down]`.
//! - `rotation_jitter`: how far it turns, at most, either way, in radians.
//! - `scale_jitter`: how much its size changes, at most, either way, as a
//!   fraction (below 1).

use rand::{Rng, RngCore};
use rhai::Map;

use super::options::{as_number, number, positive};
use super::RhaiResult;
use super::StrokeStyle;
use crate::journal::fonts::Font;
use crate::journal::symbol::Symbol;

/// The jitter settings.
pub(super) const JITTER_KEYS: [&str; 3] = ["position_jitter", "rotation_jitter", "scale_jitter"];

/// Role settings: what roles and slots take (see layout.rs). `role` is
/// for slots only.
pub(super) const ROLE_KEYS: [&str; 21] = [
    "font", "style", "size", "line_height", "line_jitter", "align", "valign", "indent", "paragraph_gap", "spacing", "clearance", "jagged", "fit", "split_words", "split_padding", "em", "strong",
    "position_jitter", "rotation_jitter", "scale_jitter", "role",
];

/// The settings `caller` uses, `own`, from a text settings map: role keys
/// it doesn't use are dropped, and anything else unknown is an error.
pub(super) fn settings(caller: &str, settings: Map, own: &[&str]) -> RhaiResult<Map> {
    if let Some(key) = settings.keys().find(|k| !own.contains(&k.as_str()) && !ROLE_KEYS.contains(&k.as_str())) {
        return Err(format!("{}: unknown option '{}'; options: {} (and role settings, which it ignores)", caller, key, own.join(", ")).into());
    }
    Ok(settings.into_iter().filter(|(k, _)| own.contains(&k.as_str())).collect())
}

/// The `font` setting, which `caller` needs.
pub(super) fn font(caller: &str, settings: &Map) -> RhaiResult<Font> {
    let value = settings.get("font").ok_or_else(|| format!("{}: missing 'font', the font text is set in", caller))?;
    value.clone().try_cast::<Font>().ok_or_else(|| format!("{}: 'font' should be a font, got {}", caller, value.type_name()).into())
}

/// The `style` setting, which `caller` needs.
pub(super) fn style(caller: &str, settings: &Map) -> RhaiResult<StrokeStyle> {
    let value = settings.get("style").ok_or_else(|| format!("{}: missing 'style', the style strokes are drawn in", caller))?;
    value.clone().try_cast::<StrokeStyle>().ok_or_else(|| format!("{}: 'style' should be a style, got {}", caller, value.type_name()).into())
}

/// The `size` setting, the symbol scale, which `caller` needs.
pub(super) fn size(caller: &str, settings: &Map) -> RhaiResult<f32> {
    if !settings.contains_key("size") {
        return Err(format!("{}: missing 'size', the size glyphs are set at", caller).into());
    }
    positive(caller, settings, "size")
}

/// Each placed glyph set a little off true.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Jitter {
    /// Across and down, in sizes.
    position: (f64, f64),
    rotation: f64,
    scale: f64,
}

impl Jitter {
    /// The jitter settings in `settings`, each 0 unless given.
    pub fn parse(caller: &str, settings: &Map) -> RhaiResult<Jitter> {
        let at_least_zero = |key: &str, value: f32| -> RhaiResult<f64> {
            if value >= 0.0 && value.is_finite() {
                Ok(value as f64)
            } else {
                Err(format!("{}: '{}' must be a number of at least 0, got {}", caller, key, value).into())
            }
        };
        let position = match settings.get("position_jitter") {
            None => (0.0, 0.0),
            Some(v) if v.is_array() => {
                let pair = v.clone().into_array().expect("checked");
                match pair.as_slice() {
                    [x, y] => (
                        at_least_zero("position_jitter", as_number(caller, "'position_jitter'", x)?)?,
                        at_least_zero("position_jitter", as_number(caller, "'position_jitter'", y)?)?,
                    ),
                    _ => return Err(format!("{}: 'position_jitter' should be a number or [across, down]", caller).into()),
                }
            }
            Some(v) => {
                let n = at_least_zero("position_jitter", as_number(caller, "'position_jitter'", v)?)?;
                (n, n)
            }
        };
        let rotation = at_least_zero("rotation_jitter", number(caller, settings, "rotation_jitter")?.unwrap_or(0.0))?;
        let scale = at_least_zero("scale_jitter", number(caller, settings, "scale_jitter")?.unwrap_or(0.0))?;
        if scale >= 1.0 {
            return Err(format!("{}: 'scale_jitter' must be below 1, got {}", caller, scale).into());
        }
        Ok(Jitter { position, rotation, scale })
    }
    pub fn is_none(&self) -> bool {
        *self == Jitter::default()
    }
    /// Jitter placed glyphs set at `size`: for each, across, down, the
    /// turn and the scale, in that order. Nothing is drawn from the
    /// generator for a jitter of 0.
    pub fn apply(&self, glyphs: &mut [Symbol], size: f32, rng: &mut dyn RngCore) {
        if self.is_none() {
            return;
        }
        let mut either_way = |amount: f64| if amount > 0.0 { rng.random_range(-amount..amount) } else { 0.0 };
        for glyph in glyphs {
            let x = either_way(self.position.0) * size as f64;
            let y = either_way(self.position.1) * size as f64;
            glyph.translate_by(parry2d::math::Vec2::new(x as f32, y as f32));
            glyph.rotate_rad_by(either_way(self.rotation) as f32);
            glyph.scale_by((1.0 + either_way(self.scale)) as f32);
        }
    }
}
