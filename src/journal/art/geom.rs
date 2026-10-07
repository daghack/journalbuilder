//! Geometry the art shares: an f64 point type, arcs, and turning dense
//! polylines into smooth paths.

use std::ops::{Add, Mul, Sub};

use parry2d::math::Vec2;

use crate::journal::paths::StrokePath;

/// A point or direction; the art is worked out in f64, then written as an
/// f32 path.
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
        self.x.hypot(self.y)
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
