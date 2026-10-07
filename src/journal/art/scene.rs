//! Line drawings of overlapping shapes. Each shape is a closed outline at a
//! depth, with any open strokes drawn on it (veins, stamens); where shapes
//! overlap, the lines of the one behind stop at the edge of the one in
//! front. Shapes that grow out of each other (a branch from its stem, a
//! thorn from its cane) are joined: near where they meet, each hides the
//! other's outline, so the two read as one.
//!
//! Each shape belongs to a part, and a part's visible lines come out as one
//! path, so parts can be stroked differently.

use std::cell::OnceCell;
use std::collections::HashMap;

use super::geom::{simplify, spline, spline_closed, V};
use crate::journal::paths::StrokePath;

struct Shape<T> {
    part: T,
    /// Higher is nearer.
    depth: f64,
    /// Closed: the last point joins back to the first. A shape made of
    /// triangles has none: it hides with its triangles, and draws only its
    /// strokes.
    outline: Vec<V>,
    triangles: Option<Triangles>,
    /// Outline points a curve mustn't smooth over, such as the tips of
    /// teeth and thorns.
    corners: Vec<bool>,
    /// Open lines drawn on it, and which of their points are corners.
    strokes: Vec<Run>,
    joins: Vec<Join>,
    min: V,
    max: V,
}

/// Triangles covering a shape, bucketed on a grid to find those under a
/// point quickly.
struct Triangles {
    triangles: Vec<[V; 3]>,
    cell: f64,
    grid: HashMap<(i64, i64), Vec<usize>>,
}

impl Triangles {
    fn new(triangles: Vec<[V; 3]>, cell: f64) -> Self {
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (t, corners) in triangles.iter().enumerate() {
            let key = |p: V| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
            let keys = corners.map(key);
            let (x0, x1) = (keys.iter().map(|k| k.0).min().unwrap(), keys.iter().map(|k| k.0).max().unwrap());
            let (y0, y1) = (keys.iter().map(|k| k.1).min().unwrap(), keys.iter().map(|k| k.1).max().unwrap());
            for x in x0..=x1 {
                for y in y0..=y1 {
                    grid.entry((x, y)).or_default().push(t);
                }
            }
        }
        Triangles { triangles, cell, grid }
    }

    fn contains(&self, q: V) -> bool {
        let key = ((q.x / self.cell).floor() as i64, (q.y / self.cell).floor() as i64);
        self.grid.get(&key).is_some_and(|near| near.iter().any(|&t| {
            let [a, b, c] = self.triangles[t];
            let side = |p: V, r: V| (r.x - p.x) * (q.y - p.y) - (r.y - p.y) * (q.x - p.x);
            let (d1, d2, d3) = (side(a, b), side(b, c), side(c, a));
            !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
        }))
    }
}

/// Another shape this one grows out of, or that grows out of it, at `at`.
struct Join {
    shape: usize,
    at: V,
    reach: f64,
}

pub(super) struct Scene<T> {
    shapes: Vec<Shape<T>>,
    /// How closely drawn curves follow the lines, and the furthest apart
    /// the points they're drawn through may be.
    tolerance: f64,
    max_gap: f64,
    /// Visible stretches of line shorter than this aren't drawn.
    speck: f64,
    /// The shapes bucketed by where they reach on the page, built when
    /// first needed (after the shapes are all added).
    index: OnceCell<HashMap<(i64, i64), Vec<usize>>>,
}

/// A stretch of line: its points, and whether each is a corner.
pub(super) type Run = Vec<(V, bool)>;

impl<T: Copy + PartialEq> Scene<T> {
    pub fn new(tolerance: f64, max_gap: f64, speck: f64) -> Self {
        Scene { shapes: Vec::new(), tolerance, max_gap, speck, index: OnceCell::new() }
    }

    /// Add a shape: its closed outline (not repeating the first point),
    /// which of its points are corners, and its depth (higher is nearer).
    /// Returns its index.
    pub fn add(&mut self, part: T, depth: f64, outline: Vec<V>, corners: Vec<bool>) -> usize {
        debug_assert_eq!(outline.len(), corners.len());
        let min = outline.iter().fold(V::new(f64::INFINITY, f64::INFINITY), |m, p| V::new(m.x.min(p.x), m.y.min(p.y)));
        let max = outline.iter().fold(V::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |m, p| V::new(m.x.max(p.x), m.y.max(p.y)));
        self.shapes.push(Shape { part, depth, outline, triangles: None, corners, strokes: Vec::new(), joins: Vec::new(), min, max });
        self.index = OnceCell::new();
        self.shapes.len() - 1
    }

    /// Add a shape covered by `triangles`, drawn as `strokes` alone.
    pub fn add_triangles(&mut self, part: T, depth: f64, triangles: Vec<[V; 3]>, strokes: Vec<Run>, cell: f64) -> usize {
        let corners = triangles.iter().flatten();
        let min = corners.clone().fold(V::new(f64::INFINITY, f64::INFINITY), |m, p| V::new(m.x.min(p.x), m.y.min(p.y)));
        let max = corners.fold(V::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |m, p| V::new(m.x.max(p.x), m.y.max(p.y)));
        let triangles = Some(Triangles::new(triangles, cell));
        self.shapes.push(Shape { part, depth, outline: Vec::new(), triangles, corners: Vec::new(), strokes, joins: Vec::new(), min, max });
        self.index = OnceCell::new();
        self.shapes.len() - 1
    }

    /// Join two shapes that grow out of each other at `at`: within `reach`
    /// of it, each hides the other's outline whatever their depths.
    pub fn join(&mut self, a: usize, b: usize, at: V, reach: f64) {
        self.shapes[a].joins.push(Join { shape: b, at, reach });
        self.shapes[b].joins.push(Join { shape: a, at, reach });
    }

    fn contains(&self, shape: usize, q: V) -> bool {
        let shape = &self.shapes[shape];
        if q.x < shape.min.x || q.x > shape.max.x || q.y < shape.min.y || q.y > shape.max.y {
            return false;
        }
        if let Some(triangles) = &shape.triangles {
            return triangles.contains(q);
        }
        // Even-odd crossings of a ray to the right of q.
        let points = &shape.outline;
        let mut inside = false;
        let mut j = points.len() - 1;
        for i in 0..points.len() {
            let (a, b) = (points[i], points[j]);
            if (a.y > q.y) != (b.y > q.y) && q.x < (b.x - a.x) * (q.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Whether `q`, on one of shape `owner`'s lines, is hidden: inside a
    /// shape in front of it, or one joined to it near the join.
    fn hidden(&self, owner: usize, q: V) -> bool {
        let own = &self.shapes[owner];
        let cell = 4.0 * self.max_gap;
        let key = |p: V| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
        let index = self.index.get_or_init(|| {
            let mut index: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
            for (i, shape) in self.shapes.iter().enumerate() {
                let ((x0, y0), (x1, y1)) = (key(shape.min), key(shape.max));
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        index.entry((x, y)).or_default().push(i);
                    }
                }
            }
            index
        });
        index.get(&key(q)).into_iter().flatten().any(|&other| {
            other != owner
                && (self.shapes[other].depth > own.depth
                    || own.joins.iter().any(|join| join.shape == other && (q - join.at).len() < join.reach))
                && self.contains(other, q)
        })
    }

    /// Where the line between a visible point and a hidden one meets the
    /// edge of what hides it.
    fn edge(&self, owner: usize, visible: V, hidden: V) -> V {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..20 {
            let mid = (lo + hi) / 2.0;
            if self.hidden(owner, visible.lerp(hidden, mid)) { hi = mid } else { lo = mid }
        }
        visible.lerp(hidden, (lo + hi) / 2.0)
    }

    /// The visible runs of one of `owner`'s lines. A closed line's runs
    /// wrap round its end; `None` means a closed line is all visible.
    fn runs(&self, owner: usize, points: &[V], corners: &[bool], closed: bool) -> Option<Vec<Run>> {
        let hidden: Vec<bool> = points.iter().map(|&q| self.hidden(owner, q)).collect();
        let n = points.len();
        let start = match hidden.iter().position(|&h| h) {
            None if closed => return None,
            None => return Some(vec![points.iter().copied().zip(corners.iter().copied()).collect()]),
            // A closed line's runs start after a hidden point; an open
            // line's at its start.
            Some(first) => if closed { first } else { 0 },
        };
        let mut runs = Vec::new();
        let mut run: Run = Vec::new();
        if !closed && !hidden[0] {
            run.push((points[0], corners[0]));
        }
        let steps = if closed { n } else { n - 1 };
        for step in 1..=steps {
            let (before, i) = ((start + step - 1) % n, (start + step) % n);
            match (hidden[before], hidden[i]) {
                (true, false) => {
                    run.push((self.edge(owner, points[i], points[before]), false));
                    run.push((points[i], corners[i]));
                }
                (false, false) => run.push((points[i], corners[i])),
                (false, true) => {
                    run.push((self.edge(owner, points[before], points[i]), false));
                    runs.push(std::mem::take(&mut run));
                }
                (true, true) => {}
            }
        }
        if !run.is_empty() {
            runs.push(run);
        }
        runs.retain(|run| run.windows(2).map(|w| (w[1].0 - w[0].0).len()).sum::<f64>() >= self.speck);
        Some(runs)
    }

    /// A run as one stroke, smooth between its corners.
    fn draw(&self, mut path: StrokePath, run: &[(V, bool)]) -> StrokePath {
        let mut piece = vec![run[0].0];
        let mut started = false;
        for (i, &(q, corner)) in run.iter().enumerate().skip(1) {
            piece.push(q);
            if corner || i == run.len() - 1 {
                path = spline(path, &simplify(&piece, self.tolerance, self.max_gap), started);
                started = true;
                piece = vec![q];
            }
        }
        path
    }

    /// The visible lines of every shape of `part`, as one path.
    pub fn path(&self, part: T) -> StrokePath {
        let mut path = StrokePath::new();
        for (i, shape) in self.shapes.iter().enumerate().filter(|(_, shape)| shape.part == part) {
            match (shape.outline.is_empty(), self.runs(i, &shape.outline, &shape.corners, true)) {
                (true, _) => {}
                (false, Some(runs)) => {
                    for run in runs {
                        path = self.draw(path, &run);
                    }
                }
                (false, None) => match shape.corners.iter().position(|&c| c) {
                    // All visible: round from a corner and back to it, or
                    // as one smooth loop.
                    Some(corner) => {
                        let n = shape.outline.len();
                        let run: Run = (0..=n).map(|k| ((shape.outline[(corner + k) % n]), shape.corners[(corner + k) % n])).collect();
                        path = self.draw(path, &run);
                    }
                    None => {
                        let mut loop_points = shape.outline.clone();
                        loop_points.push(loop_points[0]);
                        let mut kept = simplify(&loop_points, self.tolerance, self.max_gap);
                        kept.pop();
                        path = spline_closed(path, &kept);
                    }
                },
            }
            for stroke in &shape.strokes {
                let (points, corners): (Vec<V>, Vec<bool>) = stroke.iter().copied().unzip();
                for run in self.runs(i, &points, &corners, false).unwrap_or_default() {
                    path = self.draw(path, &run);
                }
            }
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(center: V, half: f64) -> Vec<V> {
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter()
            .map(|&(x, y)| center + V::new(x, y) * half)
            .collect()
    }

    /// A square with points all along its sides, and which are its corners.
    fn fine_square(center: V, half: f64) -> (Vec<V>, Vec<bool>) {
        let corners = square(center, half);
        (0..4).flat_map(|i| (0..20).map(move |k| (i, k)))
            .map(|(i, k)| (corners[i].lerp(corners[(i + 1) % 4], k as f64 / 20.0), k == 0))
            .unzip()
    }

    fn ends(path: &StrokePath) -> Vec<(f32, f32)> {
        path.to_string().split('M').skip(1).map(|sub| {
            let nums: Vec<f32> = sub.split_whitespace().filter_map(|t| t.parse().ok()).collect();
            (nums[0], nums[1])
        }).collect()
    }

    #[test]
    fn the_shape_behind_stops_at_the_edge_of_the_one_in_front() {
        let mut scene: Scene<u8> = Scene::new(0.01, 1.0, 0.01);
        let (back, corners) = fine_square(V::new(0.0, 0.0), 1.0);
        scene.add(0, 0.0, back, corners);
        let (front, corners) = fine_square(V::new(1.0, 0.0), 0.5);
        scene.add(1, 1.0, front, corners);
        // The back square's right side runs under the front one from y = -0.5
        // to 0.5, so its outline is one open run ending on that square's edge.
        let path = scene.path(0);
        assert_eq!(ends(&path).len(), 1, "{}", path);
        let points = path.points_within();
        let (first, last) = (points[0], points[points.len() - 1]);
        for p in [first, last] {
            assert!((p.x - 1.0).abs() < 1e-3 && (p.y.abs() - 0.5).abs() < 1e-3, "{:?}", p);
        }
        // The front square is drawn whole, from a corner round and back.
        assert_eq!(ends(&scene.path(1)).len(), 1);
    }

    #[test]
    fn joined_shapes_hide_each_others_outline_near_the_join() {
        let mut scene: Scene<u8> = Scene::new(0.01, 1.0, 0.01);
        let (stem, corners) = fine_square(V::new(0.0, 0.0), 1.0);
        let a = scene.add(0, 1.0, stem, corners);
        let (bud, corners) = fine_square(V::new(1.0, 0.0), 0.5);
        let b = scene.add(0, 0.0, bud, corners);
        scene.join(a, b, V::new(1.0, 0.0), 2.0);
        // Both outlines are cut where they overlap, leaving the outline of
        // the two together: two runs, one from each square.
        let path = scene.path(0);
        assert_eq!(ends(&path).len(), 2, "{}", path);
        for p in path.points_within() {
            let inside_stem = p.x.abs() < 1.0 - 1e-3 && p.y.abs() < 1.0 - 1e-3;
            let inside_bud = (p.x - 1.0).abs() < 0.5 - 1e-3 && p.y.abs() < 0.5 - 1e-3;
            assert!(!inside_stem && !inside_bud, "{:?} is inside the joined shapes", p);
        }
    }
}
