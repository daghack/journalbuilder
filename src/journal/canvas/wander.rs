//! Wandering lines: text written along a line that strays smoothly up and
//! down from straight, as a hand's does.

use parry2d::math::Vec2;
use rand::{Rng, RngCore};

use crate::journal::paths::StrokePath;
use crate::journal::symbol::Symbol;

/// How far apart, in sizes, a wandering line's turns are.
const WANDER_STEP: f32 = 3.0;

/// A line that wanders smoothly up and down from straight: a random
/// offset every `WANDER_STEP` sizes along it (none at its start, so it
/// starts where it's put), joined by a Catmull-Rom spline.
pub struct Wander {
    step: f32,
    offsets: Vec<f32>,
}

impl Wander {
    /// A line `length` long, straying up to `amount` canvas units either
    /// way, for text set at `size`. Nothing is drawn from the generator
    /// for an amount of 0.
    pub fn new(amount: f32, size: f32, length: f32, rng: &mut dyn RngCore) -> Wander {
        let step = WANDER_STEP * size;
        let knots = (length / step).ceil().max(1.0) as usize + 1;
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
