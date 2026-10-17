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
use crate::journal::paths::StrokePath;
use crate::journal::symbol::Symbol;
use parry2d::math::Vec2;

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

/// The `path_jitter` setting, for `write_line` and `write_block`: how far,
/// at most, either way, in sizes, the line text is written along strays
/// up and down from straight, as a hand's lines do.
pub(super) fn path_jitter(caller: &str, settings: &Map) -> RhaiResult<f32> {
    match number(caller, settings, "path_jitter")? {
        None => Ok(0.0),
        Some(j) if j >= 0.0 && j.is_finite() => Ok(j),
        Some(j) => Err(format!("{}: 'path_jitter' must be a number of at least 0, got {}", caller, j).into()),
    }
}

/// How far apart, in sizes, a wandering line's turns are.
const WANDER_STEP: f32 = 3.0;

/// A line that wanders smoothly up and down from straight: a random
/// offset every `WANDER_STEP` sizes along it (none at its start, so it
/// starts where it's put), joined by a Catmull-Rom spline.
pub(super) struct Wander {
    step: f32,
    offsets: Vec<f32>,
}

impl Wander {
    /// A line `length` long, straying up to `jitter` sizes either way, for
    /// text set at `size`. Nothing is drawn from the generator for a
    /// jitter of 0.
    pub fn new(jitter: f32, size: f32, length: f32, rng: &mut dyn RngCore) -> Wander {
        let step = WANDER_STEP * size;
        let knots = (length / step).ceil().max(1.0) as usize + 1;
        let amount = jitter * size;
        let offsets = (0..knots)
            .map(|k| if k == 0 || amount == 0.0 { 0.0 } else { rng.random_range(-amount..amount) })
            .collect();
        Wander { step, offsets }
    }
    /// The offset at knot `k`, held level past the ends.
    fn at(&self, k: isize) -> f32 {
        self.offsets[k.clamp(0, self.offsets.len() as isize - 1) as usize]
    }
    /// The spline's slope at knot `k`, in offset per knot.
    fn tangent(&self, k: isize) -> f32 {
        (self.at(k + 1) - self.at(k - 1)) / 2.0
    }
    /// Knot `k` and how far on from it `x` (along the line) is, from 0 to 1.
    fn segment(&self, x: f32) -> (isize, f32) {
        let last = self.offsets.len() as isize - 2;
        let k = ((x / self.step).floor() as isize).clamp(0, last.max(0));
        (k, (x / self.step - k as f32).clamp(0.0, 1.0))
    }
    /// How far the line is from straight `x` along it (down the page positive).
    pub fn offset(&self, x: f32) -> f32 {
        let (k, t) = self.segment(x);
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * self.at(k) + (t3 - 2.0 * t2 + t) * self.tangent(k)
            + (-2.0 * t3 + 3.0 * t2) * self.at(k + 1) + (t3 - t2) * self.tangent(k + 1)
    }
    /// The way the line runs `x` along it, in radians.
    pub fn angle(&self, x: f32) -> f32 {
        let (k, t) = self.segment(x);
        let t2 = t * t;
        let per_knot = (6.0 * t2 - 6.0 * t) * self.at(k) + (3.0 * t2 - 4.0 * t + 1.0) * self.tangent(k)
            + (-6.0 * t2 + 6.0 * t) * self.at(k + 1) + (3.0 * t2 - 2.0 * t) * self.tangent(k + 1);
        (per_knot / self.step).atan()
    }
    /// The line from `start` to the right, as a path: a cubic between
    /// each pair of knots, the same curve `offset` follows.
    pub fn path(&self, start: Vec2) -> StrokePath {
        let mut path = StrokePath::new().move_to(start.x, start.y + self.at(0));
        for k in 0..self.offsets.len() as isize - 1 {
            let x = start.x + k as f32 * self.step;
            let (c1, c2) = (self.at(k) + self.tangent(k) / 3.0, self.at(k + 1) - self.tangent(k + 1) / 3.0);
            path = path.cubic_curve_to(
                x + self.step / 3.0, start.y + c1,
                x + self.step * 2.0 / 3.0, start.y + c2,
                x + self.step, start.y + self.at(k + 1),
            );
        }
        path
    }
    /// Glyphs set on a straight line from `left` moved onto this one:
    /// each moved by the offset under its centre and turned to the way
    /// the line runs there.
    pub fn bend(&self, glyphs: &mut [Symbol], left: f32) {
        for glyph in glyphs {
            let x = glyph.position().x - left;
            glyph.translate_by(Vec2::new(0.0, self.offset(x)));
            glyph.rotate_rad_by(self.angle(x));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn a_wander_starts_on_the_line_stays_near_it_and_matches_its_path() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let wander = Wander::new(0.5, 10.0, 200.0, &mut rng);
        assert_eq!(wander.offset(0.0), 0.0);
        let xs: Vec<f32> = (0..=200).map(|x| x as f32).collect();
        // Catmull-Rom overshoots its knots a little, never by half again.
        assert!(xs.iter().all(|&x| wander.offset(x).abs() <= 5.0 * 1.5), "strayed too far");
        assert!(xs.iter().any(|&x| wander.offset(x).abs() > 0.5), "didn't wander");
        // The path is the same curve, and runs the way `angle` says.
        for (points, _) in wander.path(Vec2::new(0.0, 0.0)).flattened(0.01) {
            for p in points.iter().filter(|p| p.x <= 200.0) {
                assert!((p.y - wander.offset(p.x)).abs() < 0.05, "path {:?} off the curve at {}", p, wander.offset(p.x));
            }
        }
        let x = 37.0;
        let rise = (wander.offset(x + 0.01) - wander.offset(x - 0.01)) / 0.02;
        assert!((wander.angle(x) - rise.atan()).abs() < 1e-2);
    }

    #[test]
    fn no_wander_draws_nothing() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let wander = Wander::new(0.0, 10.0, 200.0, &mut rng);
        assert!((0..200).all(|x| wander.offset(x as f32) == 0.0));
        let next: u64 = rng.random();
        assert_eq!(next, rand::rngs::StdRng::seed_from_u64(0).random::<u64>());
    }
}
