//! Geometry the art shares: an f64 point type, arcs, and turning dense
//! polylines into smooth paths.

use std::ops::{Add, Mul, Sub};

use parry2d::math::Vec2;

use crate::journal::paths::StrokePath;

/// A point or direction; the art is worked out in f64, then written as an
/// f32 path.
/// `x.hypot(y)`, which in the browser is correctly rounded, but with no
/// instruction for its multiplications makes up most of the time spent on
/// some drawings.
#[cfg(target_arch = "wasm32")]
pub(crate) fn hypot(x: f64, y: f64) -> f64 {
    exact_hypot(x, y).unwrap_or_else(|| x.hypot(y))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn hypot(x: f64, y: f64) -> f64 {
    x.hypot(y)
}

/// The correctly rounded `x.hypot(y)`: the sum the standard library's makes
/// of `x * x + y * y` and the root's error, the errors of its products got by
/// Dekker's splitting. `None` where the numbers are extreme or the answer is
/// too near halfway between two to tell.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn exact_hypot(x: f64, y: f64) -> Option<f64> {
    // Sorted, so that not a number fails the test below.
    let (x, y) = if x.abs() > y.abs() { (x.abs(), y.abs()) } else { (y.abs(), x.abs()) };
    if !(y > 1e-100 && x < 1e100 && x < y * 1e8) {
        return None;
    }
    // `a * a`'s rounding error, and `a * a`.
    let squared = |a: f64| {
        let split = 134217729.0 * a;
        let high = split - (split - a);
        let low = a - high;
        let square = a * a;
        (((high * high - square) + 2.0 * high * low) + low * low, square)
    };
    let ((x_error, x2), (y_error, y2)) = (squared(x), squared(y));
    let r2 = x2 + y2;
    let error = ((x2 - r2) + y2) + (x_error + y_error);
    let root = r2.sqrt();
    let (root_error, root2) = squared(root);
    let tail = root * (0.5 / r2) * (error - ((root2 - r2) + root_error));
    let sum = root + tail;
    let rest = tail - (sum - root);
    // How big what's left of the sum is, next to the sum: too big, or too
    // small, or about half of its last place, and it can't be rounded by.
    const EXPONENT: u64 = 0x7ff << 52;
    let size = rest.abs().to_bits().wrapping_add(0x3fe << 52).wrapping_sub(sum.to_bits() & EXPONENT);
    let near_half = (size.wrapping_sub(0x3c90000000000000).wrapping_add(16)) >> 5 == 0;
    (!near_half && (0x39b0000000000000..=0x3c9fffffffffff80).contains(&size)).then_some(sum)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct V {
    pub x: f64,
    pub y: f64,
}

impl V {
    pub const fn new(x: f64, y: f64) -> Self {
        V { x, y }
    }
    /// The unit vector at `angle` radians from +x, towards +y.
    pub fn angle(angle: f64) -> Self {
        V::new(angle.cos(), angle.sin())
    }
    pub fn dot(self, other: V) -> f64 {
        self.x * other.x + self.y * other.y
    }
    pub fn len(self) -> f64 {
        hypot(self.x, self.y)
    }
    /// A quarter turn from +x towards +y.
    pub fn perp(self) -> V {
        V::new(-self.y, self.x)
    }
    pub fn lerp(self, other: V, t: f64) -> V {
        self + (other - self) * t
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for V {
    type Output = V;
    fn add(self, o: V) -> V {
        V::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for V {
    type Output = V;
    fn sub(self, o: V) -> V {
        V::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for V {
    type Output = V;
    fn mul(self, s: f64) -> V {
        V::new(self.x * s, self.y * s)
    }
}

impl From<Vec2> for V {
    fn from(v: Vec2) -> V {
        V::new(v.x as f64, v.y as f64)
    }
}

/// The end of an arc `length` long with curvature `k`, from `start` heading
/// `heading`.
pub(super) fn arc_end(start: V, heading: f64, k: f64, length: f64) -> V {
    // The chord turns half as far as the arc and is sin(x)/x as long, for
    // x the half turn.
    let half_turn = k * length / 2.0;
    let chord = if half_turn.abs() < 1e-4 {
        length * (1.0 - half_turn * half_turn / 6.0)
    } else {
        length * half_turn.sin() / half_turn
    };
    start + V::angle(heading + half_turn) * chord
}

/// `points` thinned out to those needed to follow them within `tolerance`,
/// at most `max_gap` apart; the ends are always kept.
pub(super) fn simplify(points: &[V], tolerance: f64, max_gap: f64) -> Vec<V> {
    let mut points: Vec<V> = points.to_vec();
    points.dedup_by(|a, b| (*a - *b).len() < 1e-9);
    if points.len() < 3 {
        return points;
    }
    let off_chord = |p: V, a: V, b: V| {
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
        (p - a.lerp(b, t)).len()
    };
    let mut kept = vec![points[0]];
    let mut anchor = 0;
    for i in 2..points.len() {
        let (a, b) = (points[anchor], points[i]);
        let fits = (b - a).len() <= max_gap && points[anchor + 1..i].iter().all(|&p| off_chord(p, a, b) <= tolerance);
        if !fits {
            anchor = i - 1;
            kept.push(points[anchor]);
        }
    }
    kept.push(points[points.len() - 1]);
    kept
}

fn f32s(v: V) -> (f32, f32) {
    (v.x as f32, v.y as f32)
}

/// A unit-ish tangent at `point` between its neighbours, weighting each
/// side by the other's length so uneven spacing doesn't kink the curve.
fn tangent(before: V, point: V, after: V) -> V {
    let (d0, d1) = ((point - before).len(), (after - point).len());
    ((point - before) * (d1 / d0) + (after - point) * (d0 / d1)) * (1.0 / (d0 + d1))
}

/// The cubics from point to point, given each point's tangent.
fn cubics(mut path: StrokePath, points: &[V], tangents: &[V], segments: usize) -> StrokePath {
    let n = points.len();
    for i in 0..segments {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let third = (b - a).len() / 3.0;
        let (c1, c2, end) = (f32s(a + tangents[i] * third), f32s(b - tangents[(i + 1) % n] * third), f32s(b));
        path = path.cubic_curve_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
    }
    path
}

/// Smooth cubics through `points` (Catmull-Rom with chord-length tangents,
/// so uneven spacing doesn't kink it), added to `path`, continuing its
/// current stroke if `continues`.
pub(super) fn spline(mut path: StrokePath, points: &[V], continues: bool) -> StrokePath {
    if !continues {
        let (x, y) = f32s(points[0]);
        path = path.move_to(x, y);
    }
    let n = points.len();
    if n < 2 {
        return path;
    }
    let mut tangents = vec![V::default(); n];
    for i in 1..n - 1 {
        tangents[i] = tangent(points[i - 1], points[i], points[i + 1]);
    }
    // At the ends, as a parabola through the end and its neighbour would.
    let chord = |i: usize| (points[i + 1] - points[i]) * (1.0 / (points[i + 1] - points[i]).len());
    if n == 2 {
        tangents[0] = chord(0);
        tangents[1] = chord(0);
    } else {
        tangents[0] = chord(0) * 2.0 - tangents[1];
        tangents[n - 1] = chord(n - 2) * 2.0 - tangents[n - 2];
    }
    cubics(path, points, &tangents, n - 1)
}

/// A smooth closed curve through `points` and back to the first, as one
/// stroke starting there.
pub(super) fn spline_closed(path: StrokePath, points: &[V]) -> StrokePath {
    let n = points.len();
    if n < 3 {
        return spline(path, &[points, &points[..1]].concat(), false);
    }
    let (x, y) = f32s(points[0]);
    let path = path.move_to(x, y);
    let tangents: Vec<V> = (0..n).map(|i| tangent(points[(i + n - 1) % n], points[i], points[(i + 1) % n])).collect();
    cubics(path, points, &tangents, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A positive number as a whole number times 2 to a power.
    fn parts(value: f64) -> (u128, i32) {
        let bits = value.to_bits();
        ((bits & ((1 << 52) - 1) | 1 << 52) as u128, (bits >> 52) as i32 - 1075)
    }

    #[test]
    fn exact_hypot_is_correctly_rounded() {
        let mut state = 1u64;
        let mut next = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            state
        };
        let mut told = 0;
        for _ in 0..3_000_000 {
            // Within 2^9 of each other, for their squares to fit in 128 bits.
            let (a, b, c) = (next(), next(), next());
            let x = 1.0 + (a >> 12) as f64 * 2f64.powi(-52) * 510.0;
            let y = 1.0 + (b >> 12) as f64 * 2f64.powi(-52) * 2f64.powi((c % 2) as i32);
            let Some(found) = exact_hypot(if c & 2 == 0 { x } else { -x }, y) else { continue };
            told += 1;
            let ((mx, ex), (my, ey), (mr, er)) = (parts(x), parts(y), parts(found));
            let unit = ex.min(ey) - 1;
            let sum = (mx << (ex - unit)).pow(2) + (my << (ey - unit)).pow(2);
            // Half a last place either side of the answer, squared, in the
            // same units: the sum must be between them.
            let scale = 2 * (er - 1 - unit);
            let (low, high) = ((2 * mr - 1).pow(2) << scale, (2 * mr + 1).pow(2) << scale);
            assert!(low <= sum && sum <= high, "{} {} gave {}", x, y, found);
        }
        assert!(told > 2_900_000, "only told {}", told);
        for (x, y) in [(f64::NAN, 1.0), (1.0, f64::NAN), (f64::INFINITY, 1.0), (0.0, 5.0), (1e300, 1e300), (1e-300, 1e-300), (1.0, 1e-20)] {
            assert_eq!(exact_hypot(x, y), None);
        }
        // An exact answer leaves nothing to round by, so it's one to leave.
        assert_eq!(exact_hypot(3.0, -4.0), None);
        assert_eq!(exact_hypot(-1.0, 2.0), exact_hypot(2.0, 1.0));
    }
}
