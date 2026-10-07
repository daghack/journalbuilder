//! What text keeps clear of.
//!
//! An `AvoidanceTree` holds lines, each with a clearance (how close
//! anything may come to it), and polygons whose insides are out of bounds
//! too. Text placement asks it whether a glyph's footprint, the convex hull
//! of its strokes as placed, comes within any line's clearance or overlaps
//! a polygon; see `symbol::place_along_path`.
//!
//! Curves are flattened into short segments and kept in a grid, as are
//! polygons, so a query only looks at what's near it.

use parry2d::math::Vec2;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use crate::journal::canvas::Canvas;
use crate::journal::paths::StrokePath;
use crate::journal::symbol::Symbol;

/// Curves are flattened to within this many canvas units of themselves.
const FLATTEN_TOLERANCE: f32 = 0.2;
/// The grid's cell size, in canvas units. Segments are cut to at most this
/// long and filed under the cell their middle is in, so a query looking
/// half a cell further than it reaches finds each segment once.
const CELL: f32 = 24.0;

#[derive(Clone, Copy, Debug)]
struct Segment {
    a: Vec2,
    b: Vec2,
    clearance: f32,
}

/// A closed outline whose inside is out of bounds, and which nothing may
/// come within `clearance` of.
#[derive(Clone, Debug)]
struct Polygon {
    points: Vec<Vec2>,
    min: Vec2,
    max: Vec2,
    clearance: f32,
}

impl Polygon {
    /// Whether `shape` (with bounds `min` to `max`) comes within the
    /// polygon's clearance plus `padding` of it, or overlaps it: as
    /// `shapes_within`, with edges far from the shape skipped by their
    /// bounds.
    fn near(&self, shape: &[Vec2], min: Vec2, max: Vec2, padding: f32) -> bool {
        let limit = self.clearance + padding;
        if self.min.x - limit > max.x || self.min.y - limit > max.y || self.max.x + limit < min.x || self.max.y + limit < min.y {
            return false;
        }
        let n = self.points.len();
        (0..n).any(|i| {
            let (p, q) = (self.points[i], self.points[(i + 1) % n]);
            let (lo, hi) = (p.min(q) - Vec2::splat(limit), p.max(q) + Vec2::splat(limit));
            !(lo.x > max.x || lo.y > max.y || hi.x < min.x || hi.y < min.y) && shape_within(shape, p, q, limit)
        }) || (shape.len() > 2 && point_in_polygon(self.points[0], shape))
            || point_in_polygon(shape[0], &self.points)
    }
}

/// Polygons up to this far across, from their middles, are filed in the
/// grid; bigger ones are checked one by one.
const MAX_FILED_HALF: f32 = 2.0 * CELL;

type Cell = (i32, i32);

/// Hashes grid cells quickly: the default hasher resists attacks that don't
/// matter here and was much of the cost of building and asking a tree.
#[derive(Clone, Copy, Debug, Default)]
struct CellHasher(u64);

impl Hasher for CellHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_i32(&mut self, n: i32) {
        self.write_u64(n as u32 as u64);
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

type Grid = HashMap<Cell, Vec<u32>, BuildHasherDefault<CellHasher>>;

#[derive(Clone, Debug, Default)]
pub struct AvoidanceTree {
    segments: Vec<Segment>,
    polygons: Vec<Polygon>,
    /// Segments by the cell their middle is in.
    segment_cells: Grid,
    /// Polygons up to `MAX_FILED_HALF` across by the cell their middle is
    /// in, and how far the widest of them reaches from its middle.
    polygon_cells: Grid,
    filed_half: f32,
    /// Bigger polygons.
    large_polygons: Vec<u32>,
    /// The largest clearance of anything: how far past a query's bounds to
    /// look.
    max_clearance: f32,
}

fn cell_of(p: Vec2) -> Cell {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

/// The cells covering the box from `min` to `max`.
fn cells_over(min: Vec2, max: Vec2) -> impl Iterator<Item = Cell> {
    let (lo, hi) = (cell_of(min), cell_of(max));
    (lo.1..=hi.1).flat_map(move |y| (lo.0..=hi.0).map(move |x| (x, y)))
}

/// What's filed in `grid` within `reach` of the box from `min` to `max`.
fn near(grid: &Grid, min: Vec2, max: Vec2, reach: f32) -> impl Iterator<Item = u32> + '_ {
    cells_over(min - Vec2::splat(reach), max + Vec2::splat(reach))
        .filter_map(|cell| grid.get(&cell))
        .flatten()
        .copied()
}

fn bounds(points: &[Vec2]) -> (Vec2, Vec2) {
    points.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(min, max), p| (min.min(*p), max.max(*p)))
}

impl AvoidanceTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() && self.polygons.is_empty()
    }

    /// Keep `clearance` away from the line from `a` to `b`.
    pub fn add_segment(&mut self, a: Vec2, b: Vec2, clearance: f32) {
        if !(a.is_finite() && b.is_finite()) {
            return;
        }
        let clearance = clearance.max(0.0);
        self.max_clearance = self.max_clearance.max(clearance);
        let pieces = ((a.distance(b) / CELL).ceil() as usize).max(1);
        for k in 0..pieces {
            let (p, q) = (a.lerp(b, k as f32 / pieces as f32), a.lerp(b, (k + 1) as f32 / pieces as f32));
            let index = self.segments.len() as u32;
            self.segments.push(Segment { a: p, b: q, clearance });
            self.segment_cells.entry(cell_of(p.lerp(q, 0.5))).or_default().push(index);
        }
    }

    /// Keep `clearance` away from a polyline, closed or not. A single point
    /// is kept clear of too.
    pub fn add_polyline(&mut self, points: &[Vec2], closed: bool, clearance: f32) {
        match points {
            [] => {}
            [p] => self.add_segment(*p, *p, clearance),
            _ => {
                for pair in points.windows(2) {
                    self.add_segment(pair[0], pair[1], clearance);
                }
                if closed && points.len() > 2 {
                    self.add_segment(points[points.len() - 1], points[0], clearance);
                }
            }
        }
    }

    /// Keep `clearance` away from a path's lines.
    pub fn add_path(&mut self, path: &StrokePath, clearance: f32) {
        for (points, closed) in path.flattened(FLATTEN_TOLERANCE) {
            self.add_polyline(&points, closed, clearance);
        }
    }

    fn push_polygon(&mut self, polygon: Polygon) {
        let index = self.polygons.len() as u32;
        let half = (polygon.max - polygon.min).max_element() / 2.0;
        if half <= MAX_FILED_HALF {
            self.filed_half = self.filed_half.max(half);
            self.polygon_cells.entry(cell_of(polygon.min.lerp(polygon.max, 0.5))).or_default().push(index);
        } else {
            self.large_polygons.push(index);
        }
        self.max_clearance = self.max_clearance.max(polygon.clearance);
        self.polygons.push(polygon);
    }

    /// Keep out of a closed outline, and `clearance` away from its edge.
    pub fn add_polygon(&mut self, points: &[Vec2], clearance: f32) {
        if points.len() < 3 || !points.iter().all(|p| p.is_finite()) {
            self.add_polyline(points, false, clearance);
            return;
        }
        let (min, max) = bounds(points);
        self.push_polygon(Polygon { points: points.to_vec(), min, max, clearance: clearance.max(0.0) });
    }

    /// Keep out of the inside of each of a path's subpaths (open ones are
    /// closed with a straight line), and `clearance` away from their edges.
    pub fn add_region(&mut self, path: &StrokePath, clearance: f32) {
        for (points, _) in path.flattened(FLATTEN_TOLERANCE) {
            self.add_polygon(&points, clearance);
        }
    }

    /// Keep out of the convex hull of `points`, and `clearance` away from
    /// it: everything inside a drawing's outline, gaps included.
    pub fn add_hull(&mut self, points: &[Vec2], clearance: f32) {
        let points: Vec<Vec2> = points.iter().copied().filter(|p| p.is_finite()).collect();
        match parry2d::transformation::convex_hull(&points) {
            hull if hull.len() >= 3 => self.add_polygon(&hull, clearance),
            // Too few points, or all in a line: avoid them as a line.
            _ => {
                let (min, max) = bounds(&points);
                if !points.is_empty() {
                    self.add_segment(min, max, clearance);
                }
            }
        }
    }

    /// Keep `clearance` away from every stroke on a canvas, measured from
    /// the edges of its marks rather than the line down their middle.
    pub fn add_canvas(&mut self, canvas: &Canvas, clearance: f32) {
        for stroke in canvas.strokes() {
            self.add_path(&stroke.path, clearance + stroke.style.reach());
        }
    }

    /// Keep out of a glyph's footprint (the hull of its strokes, as
    /// placed), and `clearance` away from it. Blank glyphs take no room.
    pub fn add_symbol(&mut self, symbol: &Symbol, clearance: f32) {
        if !symbol.is_blank() {
            self.add_polygon(&symbol.footprint(), clearance);
        }
    }

    /// Everything in `other` too.
    pub fn merge(&mut self, other: &AvoidanceTree) {
        for s in &other.segments {
            self.add_segment(s.a, s.b, s.clearance);
        }
        for polygon in &other.polygons {
            self.push_polygon(polygon.clone());
        }
    }

    /// Whether a shape comes within a line's or polygon's clearance (plus
    /// `padding`) or overlaps a polygon. The shape is a point, a segment, or
    /// a convex polygon (closed, its inside included).
    pub fn blocks(&self, shape: &[Vec2], padding: f32) -> bool {
        if shape.is_empty() || self.is_empty() {
            return false;
        }
        let padding = padding.max(0.0);
        let (min, max) = bounds(shape);
        let mut segments = near(&self.segment_cells, min, max, self.max_clearance + padding + CELL / 2.0);
        if segments.any(|index| {
            let s = &self.segments[index as usize];
            let limit = s.clearance + padding;
            let (smin, smax) = (s.a.min(s.b) - Vec2::splat(limit), s.a.max(s.b) + Vec2::splat(limit));
            !(smin.x > max.x || smin.y > max.y || smax.x < min.x || smax.y < min.y) && shape_within(shape, s.a, s.b, limit)
        }) {
            return true;
        }
        let near_polygon = |index: u32| self.polygons[index as usize].near(shape, min, max, padding);
        near(&self.polygon_cells, min, max, self.filed_half + self.max_clearance + padding).any(near_polygon)
            || self.large_polygons.iter().any(|&index| near_polygon(index))
    }

    /// Whether a point comes within a line's clearance or lies in a polygon.
    pub fn blocks_point(&self, point: Vec2) -> bool {
        self.blocks(&[point], 0.0)
    }
}

/// Whether two polygons (or points, or segments) overlap or come within
/// `limit` of each other.
pub(crate) fn shapes_within(a: &[Vec2], b: &[Vec2], limit: f32) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let ((amin, amax), (bmin, bmax)) = (bounds(a), bounds(b));
    if amin.x - limit > bmax.x || amin.y - limit > bmax.y || bmin.x - limit > amax.x || bmin.y - limit > amax.y {
        return false;
    }
    edges(b).any(|(p, q)| shape_within(a, p, q, limit))
        || (a.len() > 2 && point_in_polygon(b[0], a))
        || (b.len() > 2 && point_in_polygon(a[0], b))
}

/// A shape's edges: a point is one zero-length edge, two points one edge,
/// more a closed polygon's.
fn edges(shape: &[Vec2]) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
    let count = match shape.len() {
        0 => 0,
        1 | 2 => 1,
        n => n,
    };
    (0..count).map(move |i| (shape[i], shape[(i + 1) % shape.len()]))
}

/// Whether the segment from `a` to `b` comes within `limit` of a shape (see
/// `AvoidanceTree::blocks`), touching included.
fn shape_within(shape: &[Vec2], a: Vec2, b: Vec2, limit: f32) -> bool {
    if shape.len() > 2 && point_in_polygon(a, shape) {
        return true;
    }
    let limit2 = limit * limit;
    edges(shape).any(|(p, q)| segment_distance2(p, q, a, b) <= limit2)
}

fn cross(o: Vec2, a: Vec2, b: Vec2) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn point_segment_distance2(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t).distance_squared(p)
}

fn segments_cross(p: Vec2, q: Vec2, a: Vec2, b: Vec2) -> bool {
    let (d1, d2) = (cross(a, b, p), cross(a, b, q));
    let (d3, d4) = (cross(p, q, a), cross(p, q, b));
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

/// The squared distance between two segments: 0 where they cross.
fn segment_distance2(p: Vec2, q: Vec2, a: Vec2, b: Vec2) -> f32 {
    if segments_cross(p, q, a, b) {
        return 0.0;
    }
    point_segment_distance2(p, a, b)
        .min(point_segment_distance2(q, a, b))
        .min(point_segment_distance2(a, p, q))
        .min(point_segment_distance2(b, p, q))
}

/// Whether a point is inside a polygon, by its winding number: outlines
/// that go round more than once (like `new_circle`'s, which overdraws to
/// close cleanly) still hold their insides.
fn point_in_polygon(p: Vec2, polygon: &[Vec2]) -> bool {
    let mut winding = 0;
    for (i, &a) in polygon.iter().enumerate() {
        let b = polygon[(i + 1) % polygon.len()];
        if a.y <= p.y {
            if b.y > p.y && cross(a, b, p) > 0.0 {
                winding += 1;
            }
        } else if b.y <= p.y && cross(a, b, p) < 0.0 {
            winding -= 1;
        }
    }
    winding != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(center: Vec2, half: f32) -> Vec<Vec2> {
        vec![center + Vec2::new(-half, -half), center + Vec2::new(half, -half), center + Vec2::new(half, half), center + Vec2::new(-half, half)]
    }

    #[test]
    fn lines_block_within_their_clearance() {
        let mut tree = AvoidanceTree::new();
        tree.add_path(&StrokePath::line(Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)), 5.0);
        assert!(tree.blocks_point(Vec2::new(50.0, 4.9)));
        assert!(!tree.blocks_point(Vec2::new(50.0, 5.1)));
        assert!(!tree.blocks_point(Vec2::new(106.0, 0.0)));
        // A box straddling the line, or with an edge within its clearance.
        assert!(tree.blocks(&square(Vec2::new(30.0, 0.0), 2.0), 0.0));
        assert!(tree.blocks(&square(Vec2::new(30.0, 8.0), 4.0), 0.0));
        assert!(!tree.blocks(&square(Vec2::new(30.0, 10.0), 4.0), 0.0));
        assert!(tree.blocks(&square(Vec2::new(30.0, 10.0), 4.0), 1.5));
        // A box much bigger than the line, around it.
        assert!(tree.blocks(&square(Vec2::new(50.0, 0.0), 500.0), 0.0));
    }

    #[test]
    fn regions_block_their_insides() {
        let mut tree = AvoidanceTree::new();
        tree.add_region(&StrokePath::circle(Vec2::ZERO, 100.0), 0.0);
        assert!(tree.blocks_point(Vec2::ZERO));
        assert!(tree.blocks(&square(Vec2::new(10.0, 10.0), 3.0), 0.0));
        assert!(!tree.blocks_point(Vec2::new(120.0, 0.0)));
        // Lines only block their edges.
        let mut ring = AvoidanceTree::new();
        ring.add_path(&StrokePath::circle(Vec2::ZERO, 100.0), 0.0);
        assert!(!ring.blocks_point(Vec2::ZERO));
        assert!(ring.blocks(&square(Vec2::new(100.0, 0.0), 3.0), 0.0));
    }

    #[test]
    fn long_curves_are_found_from_anywhere_along_them() {
        let mut tree = AvoidanceTree::new();
        tree.add_path(&StrokePath::arc(Vec2::ZERO, 1000.0, 0.0, 3.0), 2.0);
        for k in 0..30 {
            let angle = k as f32 / 10.0;
            let on = Vec2::new(angle.cos(), angle.sin()) * 1000.0;
            assert!(tree.blocks_point(on * 1.001), "{}", angle);
            assert!(!tree.blocks_point(on * 0.99), "{}", angle);
        }
    }

    #[test]
    fn merged_trees_block_like_both() {
        let (mut a, mut b) = (AvoidanceTree::new(), AvoidanceTree::new());
        a.add_path(&StrokePath::line(Vec2::ZERO, Vec2::new(10.0, 0.0)), 1.0);
        b.add_polygon(&square(Vec2::new(100.0, 100.0), 10.0), 0.0);
        a.merge(&b);
        assert!(a.blocks_point(Vec2::new(5.0, 0.5)) && a.blocks_point(Vec2::new(100.0, 100.0)));
        assert!(!a.blocks_point(Vec2::new(50.0, 50.0)));
    }

    #[test]
    fn shapes_within_checks_overlap_and_distance() {
        let a = square(Vec2::ZERO, 5.0);
        assert!(shapes_within(&a, &square(Vec2::new(9.0, 0.0), 5.0), 0.0));
        assert!(shapes_within(&a, &square(Vec2::ZERO, 1.0), 0.0));
        assert!(!shapes_within(&a, &square(Vec2::new(12.0, 0.0), 1.0), 0.0));
        assert!(shapes_within(&a, &square(Vec2::new(12.0, 0.0), 1.0), 6.5));
    }
}
