//! Hatching: shading the inside of a closed shape with parallel lines.
//!
//! Every subpath of the shape is a ring (open ones are closed back to their
//! start), and the rings are filled together by a fill rule, so holes stay
//! empty: a ring hatches as a band, a glyph outline keeps its counters,
//! and overlapping shapes aren't hatched twice. Each line is its own
//! stroke, from where it enters the shape to where it leaves it, so lines
//! never run across the outside of concave shapes.

use parry2d::math::{Rot2, Vec2};
use rand::{Rng, RngCore};

use super::paths::StrokePath;

/// Which parts of overlapping rings count as inside.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum FillRule {
    /// Each ring is a shape, all of its inside however often it winds
    /// round (a circle drawn twice round is still filled), and shapes
    /// inside or overlapping others cut holes in them: a ring of two
    /// circles is a band, and where two shapes overlap is left out.
    #[default]
    Holes,
    /// Inside where a line out crosses an odd number of edges, as with
    /// SVG's `evenodd`: like `Holes`, but a ring that crosses itself (a
    /// star drawn in one line) or winds round twice leaves out the parts
    /// it goes round twice.
    EvenOdd,
    /// Inside where rings wind round a point any number of times, as with
    /// SVG's `nonzero`: overlapping shapes merge, and only rings that wind
    /// the other way cut holes.
    NonZero,
}

impl FillRule {
    pub const NAMES: [&'static str; 3] = ["holes", "evenodd", "nonzero"];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "holes" => Some(Self::Holes),
            "evenodd" => Some(Self::EvenOdd),
            "nonzero" => Some(Self::NonZero),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HatchOptions {
    /// Distance between neighbouring lines.
    pub spacing: f32,
    /// One layer of lines at each of these angles (radians), so two make
    /// cross-hatching.
    pub angles: Vec<f32>,
    /// How far each end of a line is jittered, along the line and across.
    pub roughness: f32,
    /// How far each line bends away from straight, at most.
    pub bow: f32,
    /// How far inside the outline lines stop, measured along them.
    pub inset: f32,
    pub fill_rule: FillRule,
}

impl HatchOptions {
    pub fn new(spacing: f32, angle: f32) -> Self {
        HatchOptions { spacing, angles: vec![angle], roughness: 0.0, bow: 0.0, inset: 0.0, fill_rule: FillRule::Holes }
    }
}

/// Lines shorter than this fraction of the spacing (where a line grazes a
/// curve) are left out: by hand they'd be dots.
const MIN_LINE: f32 = 0.25;

/// The hatching of `path`'s inside as a path of separate lines. Random
/// jitter and bows come from `rng`.
pub fn hatch(path: &StrokePath, options: &HatchOptions, rng: &mut dyn RngCore) -> StrokePath {
    // Flat enough that hatching follows curves to well within a line's
    // width, however large the shape.
    let tolerance = (options.spacing / 40.0).clamp(0.005, 0.25);
    let rings: Vec<Vec<Vec2>> = path.flattened(tolerance).into_iter()
        .map(|(points, _)| points)
        .filter(|points| points.len() >= 3)
        .collect();
    let mut hatching = StrokePath::new();
    for &angle in &options.angles {
        let (to_line, from_line) = (Rot2::new(-angle), Rot2::new(angle));
        // Turned so the lines run along x.
        let turned: Vec<Vec<Vec2>> = rings.iter().map(|ring| ring.iter().map(|p| to_line * *p).collect()).collect();
        for (y, x0, x1) in scanlines(&turned, options.spacing, options.fill_rule) {
            let (x0, x1) = (x0 + options.inset, x1 - options.inset);
            if x1 - x0 < options.spacing * MIN_LINE {
                continue;
            }
            let mut jitter = |p: Vec2| {
                let r = options.roughness;
                if r > 0.0 {
                    p + Vec2::new(rng.random_range(-r..=r), rng.random_range(-r..=r) / 2.0)
                } else {
                    p
                }
            };
            let (start, end) = (jitter(Vec2::new(x0, y)), jitter(Vec2::new(x1, y)));
            let (start, end) = (from_line * start, from_line * end);
            hatching = hatching.move_to(start.x, start.y);
            if options.bow > 0.0 {
                // A quadratic's middle is half as far off its chord as its
                // control point.
                let across = from_line * Vec2::new(0.0, 1.0);
                let control = (start + end) / 2.0 + across * (2.0 * rng.random_range(-options.bow..=options.bow));
                hatching = hatching.quadratic_curve_to(control.x, control.y, end.x, end.y);
            } else {
                hatching = hatching.line_to(end.x, end.y);
            }
        }
    }
    hatching
}

/// An edge of a ring, from its lower y to its higher, which way it ran,
/// and which ring it's on.
struct Edge {
    low: Vec2,
    high: Vec2,
    winding: i32,
    ring: usize,
}

impl Edge {
    fn x_at(&self, y: f32) -> f32 {
        self.low.x + (y - self.low.y) * (self.high.x - self.low.x) / (self.high.y - self.low.y)
    }
}

/// The insides of `rings` along horizontal lines `spacing` apart, as
/// `(y, x0, x1)`, line by line and left to right. The lines are centred on
/// the rings, so the hatching is even from edge to edge.
fn scanlines(rings: &[Vec<Vec2>], spacing: f32, rule: FillRule) -> Vec<(f32, f32, f32)> {
    let mut edges: Vec<Edge> = Vec::new();
    for (r, ring) in rings.iter().enumerate() {
        for (i, &a) in ring.iter().enumerate() {
            let b = ring[(i + 1) % ring.len()];
            // Lines parallel to an edge cross it at its ends, which the
            // neighbouring edges count.
            if a.y != b.y {
                let (low, high, winding) = if a.y < b.y { (a, b, 1) } else { (b, a, -1) };
                edges.push(Edge { low, high, winding, ring: r });
            }
        }
    }
    if edges.is_empty() {
        return Vec::new();
    }
    edges.sort_by(|a, b| a.low.y.total_cmp(&b.low.y));
    let (min, max) = edges.iter().fold((f32::MAX, f32::MIN), |(lo, hi), e| (lo.min(e.low.y), hi.max(e.high.y)));
    // As many lines as fit, at least one, centred: half a spacing or more
    // from the extremes, so none runs along a flat top or bottom.
    let lines = (((max - min) / spacing).floor() as usize).max(1);
    let first = min + (max - min - (lines - 1) as f32 * spacing) / 2.0;

    let mut spans = Vec::new();
    let mut active: Vec<&Edge> = Vec::new();
    let mut next = 0;
    let mut crossings: Vec<(f32, i32, usize)> = Vec::new();
    // How often each ring has wound round so far along the line, and how
    // many rings that's put us inside.
    let mut ring_depth = vec![0; rings.len()];
    for k in 0..lines {
        let y = first + k as f32 * spacing;
        while next < edges.len() && edges[next].low.y <= y {
            active.push(&edges[next]);
            next += 1;
        }
        // Each edge holds its low end but not its high one, so a line
        // through a vertex counts it once, and through a peak or a dip
        // twice or not at all.
        active.retain(|e| e.high.y > y);
        crossings.clear();
        crossings.extend(active.iter().map(|e| (e.x_at(y), e.winding, e.ring)));
        crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
        ring_depth.fill(0);
        let (mut depth, mut rings_inside) = (0, 0);
        let mut entered = 0.0;
        for &(x, winding, ring) in &crossings {
            let was_inside = inside(depth, rings_inside, rule);
            depth += winding;
            let ring_was_inside = ring_depth[ring] != 0;
            ring_depth[ring] += winding;
            match (ring_was_inside, ring_depth[ring] != 0) {
                (false, true) => rings_inside += 1,
                (true, false) => rings_inside -= 1,
                _ => {}
            }
            match (was_inside, inside(depth, rings_inside, rule)) {
                (false, true) => entered = x,
                (true, false) => spans.push((y, entered, x)),
                _ => {}
            }
        }
    }
    spans
}

/// Whether a point is inside, given the rings' total winding round it and
/// how many rings it's inside.
fn inside(depth: i32, rings_inside: i32, rule: FillRule) -> bool {
    match rule {
        FillRule::Holes => rings_inside % 2 != 0,
        FillRule::EvenOdd => depth % 2 != 0,
        FillRule::NonZero => depth != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn hatch_lines(path: &StrokePath, options: &HatchOptions) -> Vec<(Vec2, Vec2)> {
        let hatching = hatch(path, options, &mut StdRng::seed_from_u64(1));
        hatching.flattened(0.01).into_iter().map(|(points, _)| (points[0], *points.last().unwrap())).collect()
    }

    #[test]
    fn each_line_is_its_own_stroke_inside_the_shape() {
        let circle = StrokePath::circle(Vec2::ZERO, 20.0);
        let lines = hatch_lines(&circle, &HatchOptions::new(3.0, 0.7));
        assert_eq!(lines.len(), 13, "40 across at 3 apart");
        for (a, b) in lines {
            assert!(a.length() < 20.05 && b.length() < 20.05, "{:?}-{:?} leaves the circle", a, b);
            // Lines are at the angle asked for.
            let d = (b - a).normalize();
            assert!((d.y.atan2(d.x) - 0.7).abs() < 1e-3);
        }
    }

    #[test]
    fn holes_are_left_empty() {
        let ring = StrokePath::circle(Vec2::ZERO, 20.0) + StrokePath::circle(Vec2::ZERO, 10.0);
        let lines = hatch_lines(&ring, &HatchOptions::new(2.0, 0.0));
        // A line across the hole would have its middle in it.
        for (a, b) in &lines {
            assert!(((*a + *b) / 2.0).length() > 9.9, "{:?}-{:?} crosses the hole", a, b);
        }
        // Lines level with the hole are in two pieces, one each side.
        assert_eq!(lines.iter().filter(|(a, _)| a.y.abs() < 9.0).count() % 2, 0);
    }

    #[test]
    fn fill_rules_differ_on_overlaps() {
        // Two overlapping squares drawn the same way round.
        let squares = StrokePath::rectangle(Vec2::ZERO, 20.0, 20.0) + StrokePath::rectangle(Vec2::new(10.0, 0.0), 20.0, 20.0);
        let mut options = HatchOptions::new(4.0, 0.0);
        let holes = hatch_lines(&squares, &options);
        options.fill_rule = FillRule::EvenOdd;
        assert_eq!(hatch_lines(&squares, &options), holes);
        options.fill_rule = FillRule::NonZero;
        let nonzero = hatch_lines(&squares, &options);
        // Holes and even-odd leave the overlap out, so each line is two
        // pieces; nonzero merges them into one.
        assert_eq!(holes.len(), 2 * nonzero.len());
        for (a, b) in nonzero {
            assert!((a.x + 10.0).abs() < 1e-3 && (b.x - 20.0).abs() < 1e-3, "{:?}-{:?}", a, b);
        }
    }

    #[test]
    fn a_ring_wound_twice_is_filled_unless_evenodd() {
        // new_circle's circles go round twice.
        let twice = StrokePath::circle(Vec2::ZERO, 20.0);
        let mut options = HatchOptions::new(3.0, 0.0);
        assert_eq!(hatch_lines(&twice, &options).len(), 13);
        options.fill_rule = FillRule::NonZero;
        assert_eq!(hatch_lines(&twice, &options).len(), 13);
        options.fill_rule = FillRule::EvenOdd;
        assert!(hatch_lines(&twice, &options).is_empty());
    }

    #[test]
    fn concave_shapes_are_hatched_in_pieces() {
        // A U: lines through its arms are two strokes, not one across the gap.
        let u = StrokePath::new().move_to(0.0, 0.0).line_to(10.0, 0.0).line_to(10.0, 20.0).line_to(20.0, 20.0)
            .line_to(20.0, 0.0).line_to(30.0, 0.0).line_to(30.0, 30.0).line_to(0.0, 30.0).close_path();
        for (a, b) in hatch_lines(&u, &HatchOptions::new(3.0, 0.0)) {
            if a.y < 20.0 {
                assert!(b.x <= 10.0 + 1e-3 || a.x >= 20.0 - 1e-3, "{:?}-{:?} crosses the gap", a, b);
            }
        }
    }

    #[test]
    fn cross_hatching_inset_and_bows() {
        let square = StrokePath::rectangle(Vec2::ZERO, 30.0, 30.0);
        let mut options = HatchOptions::new(3.0, 0.0);
        let single = hatch_lines(&square, &options).len();
        options.angles = vec![0.0, std::f32::consts::FRAC_PI_2];
        assert_eq!(hatch_lines(&square, &options).len(), 2 * single);
        options.inset = 2.0;
        for (a, b) in hatch_lines(&square, &options) {
            assert!(a.x.abs().max(a.y.abs()) <= 15.0 && b.x.abs().max(b.y.abs()) <= 15.0);
            assert!((a - b).length() <= 26.0 + 1e-3);
        }
        options.bow = 1.5;
        let bowed = hatch(&square, &options, &mut StdRng::seed_from_u64(3));
        assert!(bowed.to_string().contains('Q'));
    }
}
