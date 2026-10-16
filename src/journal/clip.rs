//! Cutting a path by a closed shape: erasing the parts of it inside the
//! shape (a foreground figure's outline over lines behind it), or the
//! parts outside.
//!
//! Lines and curves are cut where they cross the shape's outline and kept
//! as lines and curves, the pieces of each exactly the curve it was. The
//! shape's subpaths are rings, as for hatching (open ones are closed back
//! to their start), and which parts count as inside is set by a fill rule.

use parry2d::math::Vec2;

use super::hatch::{inside, FillRule};
use super::paths::{PathCmd, StrokePath};

/// Which side of the shape's outline to keep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Keep {
    Outside,
    Inside,
}

/// A line or curve of a path, from its start to its end.
#[derive(Clone, Copy, Debug)]
enum Segment {
    Line(Vec2, Vec2),
    Quad(Vec2, Vec2, Vec2),
    Cubic(Vec2, Vec2, Vec2, Vec2),
}

impl Segment {
    fn at(&self, t: f32) -> Vec2 {
        let u = 1.0 - t;
        match *self {
            Segment::Line(a, b) => a * u + b * t,
            Segment::Quad(a, c, b) => a * (u * u) + c * (2.0 * u * t) + b * (t * t),
            Segment::Cubic(a, c1, c2, b) => a * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + b * (t * t * t),
        }
    }

    /// The two halves either side of `t`, by de Casteljau.
    fn split(&self, t: f32) -> (Segment, Segment) {
        let lerp = |p: Vec2, q: Vec2| p + (q - p) * t;
        match *self {
            Segment::Line(a, b) => {
                let m = lerp(a, b);
                (Segment::Line(a, m), Segment::Line(m, b))
            }
            Segment::Quad(a, c, b) => {
                let (ac, cb) = (lerp(a, c), lerp(c, b));
                let m = lerp(ac, cb);
                (Segment::Quad(a, ac, m), Segment::Quad(m, cb, b))
            }
            Segment::Cubic(a, c1, c2, b) => {
                let (p, q, r) = (lerp(a, c1), lerp(c1, c2), lerp(c2, b));
                let (pq, qr) = (lerp(p, q), lerp(q, r));
                let m = lerp(pq, qr);
                (Segment::Cubic(a, p, pq, m), Segment::Cubic(m, qr, r, b))
            }
        }
    }

    /// The part from `t0` to `t1`.
    fn part(&self, t0: f32, t1: f32) -> Segment {
        let head = if t1 < 1.0 { self.split(t1).0 } else { *self };
        if t0 > 0.0 { head.split(t0 / t1).1 } else { head }
    }

    fn start(&self) -> Vec2 {
        match *self {
            Segment::Line(a, _) | Segment::Quad(a, _, _) | Segment::Cubic(a, _, _, _) => a,
        }
    }

    fn command(&self) -> PathCmd {
        match *self {
            Segment::Line(_, b) => PathCmd::LineTo(b.x, b.y),
            Segment::Quad(_, c, b) => PathCmd::QuadraticCurveTo(c.x, c.y, b.x, b.y),
            Segment::Cubic(_, c1, c2, b) => PathCmd::CubicCurveTo(c1.x, c1.y, c2.x, c2.y, b.x, b.y),
        }
    }

    /// Pieces enough that chords stay within `tolerance` of the curve (as
    /// `StrokePath::flattened`).
    fn pieces(&self, tolerance: f32) -> usize {
        let second = |p0: Vec2, p1: Vec2, p2: Vec2| (p0 - 2.0 * p1 + p2).length();
        let bend = match *self {
            Segment::Line(..) => return 1,
            Segment::Quad(a, c, b) => 2.0 * second(a, c, b),
            Segment::Cubic(a, c1, c2, b) => 6.0 * second(a, c1, c2).max(second(c1, c2, b)),
        };
        ((bend / (8.0 * tolerance)).sqrt().ceil() as usize).clamp(1, 1000)
    }
}

/// A subpath as segments, whether it was closed, and whether its close
/// drew a line back to the start (rather than being there already).
struct Subpath {
    segments: Vec<Segment>,
    closed: bool,
    closing_line: bool,
}

fn subpaths(path: &StrokePath) -> Vec<Subpath> {
    let mut out: Vec<Subpath> = Vec::new();
    let (mut current, mut start) = (Vec2::ZERO, Vec2::ZERO);
    for cmd in path.commands() {
        if let PathCmd::MoveTo(x, y) = *cmd {
            current = Vec2::new(x, y);
            start = current;
            out.push(Subpath { segments: Vec::new(), closed: false, closing_line: false });
            continue;
        }
        // Drawing on after a close starts a new subpath at the start.
        if out.last().is_none_or(|s| s.closed) {
            out.push(Subpath { segments: Vec::new(), closed: false, closing_line: false });
        }
        let subpath = out.last_mut().expect("a subpath was started");
        let segment = match *cmd {
            PathCmd::LineTo(x, y) => Segment::Line(current, Vec2::new(x, y)),
            PathCmd::QuadraticCurveTo(cx, cy, x, y) => Segment::Quad(current, Vec2::new(cx, cy), Vec2::new(x, y)),
            PathCmd::CubicCurveTo(c1x, c1y, c2x, c2y, x, y) => Segment::Cubic(current, Vec2::new(c1x, c1y), Vec2::new(c2x, c2y), Vec2::new(x, y)),
            PathCmd::ClosePath => {
                subpath.closed = true;
                if current == start {
                    continue;
                }
                subpath.closing_line = true;
                Segment::Line(current, start)
            }
            PathCmd::MoveTo(..) => unreachable!(),
        };
        subpath.segments.push(segment);
        current = cmd.ending_point().unwrap_or(start);
    }
    out
}

/// The shape's outline, as rings of edges.
struct Shape {
    rings: Vec<Vec<Vec2>>,
    min: Vec2,
    max: Vec2,
    rule: FillRule,
}

impl Shape {
    fn new(shape: &StrokePath, tolerance: f32, rule: FillRule) -> Self {
        let rings: Vec<Vec<Vec2>> = shape.flattened(tolerance).into_iter()
            .map(|(points, _)| points)
            .filter(|points| points.len() >= 3)
            .collect();
        let (min, max) = rings.iter().flatten().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        Shape { rings, min, max, rule }
    }

    fn edges(&self) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
        self.rings.iter().flat_map(|ring| (0..ring.len()).map(move |i| (ring[i], ring[(i + 1) % ring.len()])))
    }

    fn contains(&self, p: Vec2) -> bool {
        if p.x < self.min.x || p.y < self.min.y || p.x > self.max.x || p.y > self.max.y {
            return false;
        }
        let (mut depth, mut rings_inside) = (0, 0);
        for ring in &self.rings {
            // The ring's winding number round p.
            let mut winding = 0;
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                let side = (b - a).perp_dot(p - a);
                if a.y <= p.y && b.y > p.y && side > 0.0 {
                    winding += 1;
                } else if a.y > p.y && b.y <= p.y && side < 0.0 {
                    winding -= 1;
                }
            }
            depth += winding;
            if winding != 0 {
                rings_inside += 1;
            }
        }
        inside(depth, rings_inside, self.rule)
    }

    /// Where along the chord from `p` to `q` (0 to 1) it crosses the
    /// outline.
    fn crossings(&self, p: Vec2, q: Vec2, out: &mut Vec<f32>) {
        if p.x.max(q.x) < self.min.x || p.y.max(q.y) < self.min.y || p.x.min(q.x) > self.max.x || p.y.min(q.y) > self.max.y {
            return;
        }
        let r = q - p;
        for (a, b) in self.edges() {
            let w = b - a;
            let denominator = r.perp_dot(w);
            if denominator == 0.0 {
                continue;
            }
            let s = (a - p).perp_dot(w) / denominator;
            let u = (a - p).perp_dot(r) / denominator;
            if (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&u) {
                out.push(s);
            }
        }
    }
}

/// A crossing at `t`, found along a chord, moved onto the curve itself:
/// by bisection, within `reach` either way, if the curve crosses there.
fn refine(segment: &Segment, shape: &Shape, t: f32, reach: f32) -> f32 {
    let (mut low, mut high) = ((t - reach).max(0.0), (t + reach).min(1.0));
    let inside_low = shape.contains(segment.at(low));
    if inside_low == shape.contains(segment.at(high)) {
        return t;
    }
    for _ in 0..24 {
        let middle = (low + high) / 2.0;
        if shape.contains(segment.at(middle)) == inside_low {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
}

/// The parts of `path` on the `keep` side of `shape`'s outline, as a new
/// path. Pieces that run on from one another stay one stroke, and a closed
/// subpath wholly kept stays closed.
pub fn clip(path: &StrokePath, shape: &StrokePath, rule: FillRule, keep: Keep) -> StrokePath {
    // Within a hundredth of the shape's size, and a quarter of a unit.
    let (lo, hi) = shape.points_within().iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let tolerance = ((hi - lo).length() / 100.0).clamp(0.01, 0.25);
    let shape = Shape::new(shape, tolerance, rule);
    let wanted = |p: Vec2| shape.contains(p) == (keep == Keep::Inside);

    let mut out: Vec<PathCmd> = Vec::new();
    // A run of segments, and whether to close it: dropping its last
    // segment, if it's the line back to the start, for the close.
    let mut emit = |run: &[Segment], close: Option<bool>| {
        let start = run[0].start();
        out.push(PathCmd::MoveTo(start.x, start.y));
        out.extend(run.iter().map(Segment::command));
        if let Some(closing_line) = close {
            if closing_line {
                out.pop();
            }
            out.push(PathCmd::ClosePath);
        }
    };
    for subpath in subpaths(path) {
        if subpath.segments.is_empty() {
            continue;
        }
        // The kept pieces, as runs of segments that run on from one another.
        let mut runs: Vec<Vec<Segment>> = Vec::new();
        let mut joined = false;
        let mut cut = false;
        let mut ts = Vec::new();
        for segment in &subpath.segments {
            ts.clear();
            ts.push(0.0);
            let n = segment.pieces(tolerance);
            let mut p = segment.start();
            for k in 0..n {
                let (t0, t1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
                let q = segment.at(t1);
                let before = ts.len();
                shape.crossings(p, q, &mut ts);
                // Chords stray from a curve by at most the tolerance, so
                // the curve crosses within a few tolerances of where its
                // chord does. Lines cross where their chord does.
                let reach = (t1 - t0) * (4.0 * tolerance / (q - p).length().max(1e-6)).min(0.5);
                for t in &mut ts[before..] {
                    *t = t0 + *t * (t1 - t0);
                    if !matches!(segment, Segment::Line(..)) {
                        *t = refine(segment, &shape, *t, reach);
                    }
                }
                p = q;
            }
            ts.push(1.0);
            ts.sort_by(f32::total_cmp);
            ts.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
            for pair in ts.windows(2) {
                let (t0, t1) = (pair[0], pair[1]);
                if !wanted(segment.at((t0 + t1) / 2.0)) {
                    joined = false;
                    cut = true;
                    continue;
                }
                let piece = if t0 == 0.0 && t1 == 1.0 { *segment } else { segment.part(t0, t1) };
                if t0 > 0.0 || t1 < 1.0 {
                    cut = true;
                }
                match runs.last_mut() {
                    Some(run) if joined => run.push(piece),
                    _ => runs.push(vec![piece]),
                }
                joined = true;
            }
        }
        if runs.is_empty() {
            continue;
        }
        if !cut {
            emit(&runs[0], subpath.closed.then_some(subpath.closing_line));
            continue;
        }
        // A closed subpath cut somewhere in the middle runs on from its end
        // round to its start.
        let first_kept_from_start = runs[0][0].start() == subpath.segments[0].start();
        if subpath.closed && joined && first_kept_from_start && runs.len() > 1 {
            let first = runs.remove(0);
            runs.last_mut().expect("more than one run").extend(first);
        }
        for run in &runs {
            emit(run, None);
        }
    }
    StrokePath::from_commands(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ends(path: &StrokePath) -> Vec<(Vec2, Vec2, bool)> {
        path.flattened(0.01).into_iter().map(|(points, closed)| (points[0], *points.last().unwrap(), closed)).collect()
    }

    fn close(a: Vec2, b: Vec2) -> bool {
        (a - b).length() < 0.05
    }

    #[test]
    fn a_line_through_a_square_loses_its_middle() {
        let line = StrokePath::line(Vec2::new(-20.0, 0.0), Vec2::new(20.0, 0.0));
        let square = StrokePath::rectangle(Vec2::ZERO, 10.0, 10.0);
        let outside = ends(&clip(&line, &square, FillRule::Holes, Keep::Outside));
        assert_eq!(outside.len(), 2);
        assert!(close(outside[0].0, Vec2::new(-20.0, 0.0)) && close(outside[0].1, Vec2::new(-5.0, 0.0)));
        assert!(close(outside[1].0, Vec2::new(5.0, 0.0)) && close(outside[1].1, Vec2::new(20.0, 0.0)));
        let inside = ends(&clip(&line, &square, FillRule::Holes, Keep::Inside));
        assert_eq!(inside.len(), 1);
        assert!(close(inside[0].0, Vec2::new(-5.0, 0.0)) && close(inside[0].1, Vec2::new(5.0, 0.0)));
    }

    #[test]
    fn curves_stay_curves_and_on_the_original() {
        // A circle of four cubics (StrokePath::circle overdraws in lines).
        let mut commands = StrokePath::circle(Vec2::ZERO, 10.0).commands()[..5].to_vec();
        commands.push(PathCmd::ClosePath);
        let circle = StrokePath::from_commands(commands);
        let square = StrokePath::rectangle(Vec2::new(10.0, 0.0), 8.0, 40.0);
        let cut = clip(&circle, &square, FillRule::Holes, Keep::Outside);
        assert!(cut.commands().iter().all(|c| !matches!(c, PathCmd::LineTo(..))));
        // One stroke, round from one cut to the other, every point still on
        // the circle and outside the square.
        let lines = cut.flattened(0.01);
        assert_eq!(lines.len(), 1, "{:?}", cut.commands());
        for p in &lines[0].0 {
            assert!((p.length() - 10.0).abs() < 0.05 && p.x <= 6.05, "{:?}", p);
        }
    }

    #[test]
    fn untouched_closed_paths_stay_closed_and_holes_are_kept() {
        let small = StrokePath::circle(Vec2::new(100.0, 0.0), 5.0);
        let cut = ends(&clip(&small, &StrokePath::rectangle(Vec2::ZERO, 10.0, 10.0), FillRule::Holes, Keep::Outside));
        assert_eq!(cut.len(), 1);
        assert!(cut[0].2);
        // A ring: a line through it keeps the part over the hole.
        let mut ring = StrokePath::circle(Vec2::ZERO, 20.0);
        ring.append(StrokePath::circle(Vec2::ZERO, 10.0));
        let line = StrokePath::line(Vec2::new(-30.0, 0.0), Vec2::new(30.0, 0.0));
        assert_eq!(ends(&clip(&line, &ring, FillRule::Holes, Keep::Outside)).len(), 3);
        assert_eq!(ends(&clip(&line, &ring, FillRule::NonZero, Keep::Outside)).len(), 2);
    }
}
