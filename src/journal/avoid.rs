//! What text keeps clear of.
//!
//! An `AvoidanceTree` holds lines, each with a clearance (how close
//! anything may come to it), and regions whose insides are out of bounds
//! too. Text placement asks it whether a glyph's footprint, the convex hull
//! of its strokes as placed, comes within any line's clearance or overlaps
//! a region; see `symbol::place_along_path`.
//!
//! Curves are flattened into short segments and kept in a grid, so a query
//! only looks at what's near it.

use parry2d::math::Vec2;
use std::collections::HashMap;

use crate::journal::canvas::Canvas;
use crate::journal::paths::StrokePath;
use crate::journal::symbol::Symbol;

/// Curves are flattened to within this many canvas units of themselves.
const FLATTEN_TOLERANCE: f32 = 0.2;
/// The grid's cell size, in canvas units. Segments are cut to at most this
/// long, so each sits in a few cells.
const CELL: f32 = 24.0;

#[derive(Clone, Copy, Debug)]
struct Segment {
    a: Vec2,
    b: Vec2,
    clearance: f32,
}

/// A closed outline whose inside is out of bounds. Its edges are also kept
/// as segments, with its clearance.
#[derive(Clone, Debug)]
struct Region {
    points: Vec<Vec2>,
    min: Vec2,
    max: Vec2,
}

type Cell = (i32, i32);

#[derive(Clone, Debug, Default)]
pub struct AvoidanceTree {
    segments: Vec<Segment>,
    regions: Vec<Region>,
    segment_cells: HashMap<Cell, Vec<u32>>,
    region_cells: HashMap<Cell, Vec<u32>>,
    /// The largest clearance of any segment: how far past a query's bounds
    /// to look.
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

fn bounds(points: &[Vec2]) -> (Vec2, Vec2) {
    points.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(min, max), p| (min.min(*p), max.max(*p)))
}

impl AvoidanceTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() && self.regions.is_empty()
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
            for cell in cells_over(p.min(q), p.max(q)) {
                self.segment_cells.entry(cell).or_default().push(index);
            }
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

    /// Keep out of a closed outline, and `clearance` away from its edge.
    pub fn add_polygon(&mut self, points: &[Vec2], clearance: f32) {
        if points.len() < 3 || !points.iter().all(|p| p.is_finite()) {
            self.add_polyline(points, false, clearance);
            return;
        }
        self.add_polyline(points, true, clearance);
        let (min, max) = bounds(points);
        let index = self.regions.len() as u32;
        self.regions.push(Region { points: points.to_vec(), min, max });
        for cell in cells_over(min, max) {
            self.region_cells.entry(cell).or_default().push(index);
        }
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
        for region in &other.regions {
            let index = self.regions.len() as u32;
            self.regions.push(region.clone());
            for cell in cells_over(region.min, region.max) {
                self.region_cells.entry(cell).or_default().push(index);
            }
        }
    }

    /// Whether a shape comes within a line's clearance (plus `padding`) or
    /// overlaps a region. The shape is a point, a segment, or a polygon
    /// (closed, its inside included).
    pub fn blocks(&self, shape: &[Vec2], padding: f32) -> bool {
        if shape.is_empty() || self.is_empty() {
            return false;
        }
        let (min, max) = bounds(shape);
        let reach = self.max_clearance + padding.max(0.0);
        let mut candidates: Vec<u32> = cells_over(min - Vec2::splat(reach), max + Vec2::splat(reach))
            .filter_map(|cell| self.segment_cells.get(&cell))
            .flatten()
            .copied()
            .collect();
        candidates.sort_unstable();
        candidates.dedup();
        for index in candidates {
            let s = &self.segments[index as usize];
            let limit = s.clearance + padding.max(0.0);
            let (smin, smax) = (s.a.min(s.b) - Vec2::splat(limit), s.a.max(s.b) + Vec2::splat(limit));
            if smin.x > max.x || smin.y > max.y || smax.x < min.x || smax.y < min.y {
                continue;
            }
            if shape_within(shape, s.a, s.b, limit) {
                return true;
            }
        }
        let mut regions: Vec<u32> = cells_over(min, max)
            .filter_map(|cell| self.region_cells.get(&cell))
            .flatten()
            .copied()
            .collect();
        regions.sort_unstable();
        regions.dedup();
        regions.into_iter().any(|index| {
            let r = &self.regions[index as usize];
            if r.min.x > max.x || r.min.y > max.y || r.max.x < min.x || r.max.y < min.y {
                return false;
            }
            // Edges crossing were caught above, so the shapes overlap only
            // if one holds the other.
            point_in_polygon(shape[0], &r.points) || (shape.len() > 2 && point_in_polygon(r.points[0], shape))
        })
    }

    /// Whether a point comes within a line's clearance or lies in a region.
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
