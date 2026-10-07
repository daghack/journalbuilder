//! Curved surfaces in 3D, drawn in outline: their edges, the lines drawn on
//! them, and their folds (where a surface turns from facing the viewer to
//! facing away), each where no nearer surface hides it. Flowers and leaves
//! are built from these.
//!
//! Coordinates are the page's, x right and y down, with z towards the
//! viewer; the view is straight down z.

use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::ops::{Add, Mul, Sub};

use super::geom::V;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct P3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl P3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        P3 { x, y, z }
    }
    /// On the page, at depth 0.
    pub fn on_page(v: V) -> Self {
        P3::new(v.x, v.y, 0.0)
    }
    pub fn dot(self, o: P3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: P3) -> P3 {
        P3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn unit(self) -> P3 {
        self * (1.0 / self.dot(self).sqrt())
    }
    pub fn lerp(self, o: P3, t: f64) -> P3 {
        self + (o - self) * t
    }
    /// Where it lands on the page.
    pub fn flat(self) -> V {
        V::new(self.x, self.y)
    }
}

impl Add for P3 {
    type Output = P3;
    fn add(self, o: P3) -> P3 {
        P3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for P3 {
    type Output = P3;
    fn sub(self, o: P3) -> P3 {
        P3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f64> for P3 {
    type Output = P3;
    fn mul(self, s: f64) -> P3 {
        P3::new(self.x * s, self.y * s, self.z * s)
    }
}

pub(super) fn mix(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// A surface over (u, v) in [0, 1] x [-1, 1], sampled on an `nu` by `nv`
/// grid to hide what's behind it. Sampled on several threads at once.
pub(super) struct Surface {
    pub point: Box<dyn Fn(f64, f64) -> P3 + Send + Sync>,
    pub nu: usize,
    pub nv: usize,
    /// Where to draw its sides (v = -1 and 1) through: u values from its
    /// base to its tip, and whether each is a corner. `None` leaves them
    /// undrawn.
    pub edges: Option<Vec<(f64, bool)>>,
    /// Lines on it, as (u, v) polylines.
    pub lines: Vec<Vec<(f64, f64)>>,
    /// Whether to draw its folds, and whether it's creased along v = 0, so
    /// that a fold there is the crease rather than one to draw.
    pub folds: bool,
    pub crease: bool,
}

impl Surface {
    /// Smooth sides, drawn through `n + 1` evenly spaced points.
    pub fn smooth_edges(n: usize) -> Option<Vec<(f64, bool)>> {
        Some((0..=n).map(|k| (k as f64 / n as f64, false)).collect())
    }
}

/// A triangle of a surface on the page, with the depth of its corners, the
/// surface it's on and where on it.
struct Triangle {
    corners: [P3; 3],
    surface: usize,
    u: f64,
    v: f64,
    min: V,
    max: V,
}

impl Triangle {
    /// Its depth over the page point `q`, if it covers it.
    fn depth_at(&self, q: V) -> Option<f64> {
        if q.x < self.min.x || q.x > self.max.x || q.y < self.min.y || q.y > self.max.y {
            return None;
        }
        let [a, b, c] = self.corners;
        let det = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
        if det.abs() < 1e-12 {
            return None;
        }
        let l1 = ((b.y - c.y) * (q.x - c.x) + (c.x - b.x) * (q.y - c.y)) / det;
        let l2 = ((c.y - a.y) * (q.x - c.x) + (a.x - c.x) * (q.y - c.y)) / det;
        let l3 = 1.0 - l1 - l2;
        (l1 >= -1e-9 && l2 >= -1e-9 && l3 >= -1e-9).then_some(l1 * a.z + l2 * b.z + l3 * c.z)
    }
}

/// A point on a line to draw, and the surface (and where on it) the line
/// runs along, if any, which doesn't hide its own line close by.
pub(super) type Tagged = (P3, Option<(usize, f64, f64)>);

/// A line in 3D: its points, and whether each is a corner.
pub(super) type Line = Vec<(Tagged, bool)>;

/// A drawn line on the page: its points, and whether each is a corner.
pub(super) type Run = Vec<(V, bool)>;

/// Surfaces ready to draw: their triangles, bucketed on the page.
pub(super) struct Model {
    surfaces: Vec<Surface>,
    triangles: Vec<Triangle>,
    grid: HashMap<(i64, i64), Vec<usize>>,
    cell: f64,
    /// How much nearer a surface must be to hide a line.
    margin: f64,
    /// Visible stretches of line shorter than this aren't drawn.
    speck: f64,
}

impl Model {
    /// The model of `surfaces` about `scale` across.
    pub fn new(surfaces: Vec<Surface>, scale: f64) -> Self {
        let mut triangles = Vec::new();
        for (index, surface) in surfaces.iter().enumerate() {
            let (nu, nv) = (surface.nu, surface.nv);
            let grid: Vec<Vec<P3>> = (0..=nu).into_par_iter().map(|i| (0..=nv).map(|j| {
                (surface.point)(i as f64 / nu as f64, -1.0 + 2.0 * j as f64 / nv as f64)
            }).collect()).collect();
            for i in 0..nu {
                for j in 0..nv {
                    let (u, v) = ((i as f64 + 0.5) / nu as f64, -1.0 + 2.0 * (j as f64 + 0.5) / nv as f64);
                    for corners in [[grid[i][j], grid[i + 1][j], grid[i + 1][j + 1]], [grid[i][j], grid[i + 1][j + 1], grid[i][j + 1]]] {
                        let min = V::new(corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min), corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min));
                        let max = V::new(corners.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max), corners.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max));
                        triangles.push(Triangle { corners, surface: index, u, v, min, max });
                    }
                }
            }
        }
        let cell = scale / 8.0;
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (t, triangle) in triangles.iter().enumerate() {
            let (x0, y0) = ((triangle.min.x / cell).floor() as i64, (triangle.min.y / cell).floor() as i64);
            let (x1, y1) = ((triangle.max.x / cell).floor() as i64, (triangle.max.y / cell).floor() as i64);
            for x in x0..=x1 {
                for y in y0..=y1 {
                    grid.entry((x, y)).or_default().push(t);
                }
            }
        }
        Model { surfaces, triangles, grid, cell, margin: 0.004 * scale, speck: 0.04 * scale }
    }

    /// The surfaces' triangles on the page.
    pub fn triangles(&self) -> Vec<[V; 3]> {
        self.triangles.iter().map(|t| t.corners.map(|p| p.flat())).collect()
    }

    /// A point on surface `index`, tagged as on it.
    pub fn on(&self, index: usize, u: f64, v: f64) -> Tagged {
        ((self.surfaces[index].point)(u, v), Some((index, u, v)))
    }

    /// Whether a surface nearer than `q` covers it, other than the one its
    /// line runs along, close by.
    pub fn hidden(&self, (q, on): Tagged) -> bool {
        let key = ((q.x / self.cell).floor() as i64, (q.y / self.cell).floor() as i64);
        let Some(near) = self.grid.get(&key) else { return false };
        near.iter().any(|&t| {
            let triangle = &self.triangles[t];
            if let Some((surface, u, v)) = on {
                let own = &self.surfaces[surface];
                if triangle.surface == surface
                    && (triangle.u - u).abs() < 1.6 / own.nu as f64
                    && (triangle.v - v).abs() < 3.2 / own.nv as f64 {
                    return false;
                }
            }
            triangle.depth_at(q.flat()).is_some_and(|depth| depth > q.z + self.margin)
        })
    }

    /// Every surface's edges, lines and folds, where they're visible.
    pub fn draw(&self, out: &mut Vec<Run>) {
        let mut lines: Vec<Line> = Vec::new();
        for index in 0..self.surfaces.len() {
            let surface = &self.surfaces[index];
            if let Some(us) = &surface.edges {
                let side = |v: f64| us.iter().map(move |&(u, corner)| (self.on(index, u, v), corner));
                lines.push(side(-1.0).chain(side(1.0).rev()).collect());
            }
            for line in &surface.lines {
                lines.push(line.iter().map(|&(u, v)| (self.on(index, u, v), false)).collect());
            }
            if surface.folds {
                lines.extend(self.folds(index));
            }
        }
        // Each line's visible stretches, on several threads at once, kept
        // in order.
        let visible: Vec<Vec<Run>> = lines.par_iter().map(|line| {
            let mut runs = Vec::new();
            self.visible(line, &mut runs);
            runs
        }).collect();
        out.extend(visible.into_iter().flatten());
    }

    /// The visible stretches of a line, on the page.
    pub fn visible(&self, line: &[(Tagged, bool)], out: &mut Vec<Run>) {
        let hidden: Vec<bool> = line.iter().map(|&(p, _)| self.hidden(p)).collect();
        let between = |a: Tagged, b: Tagged, t: f64| -> Tagged {
            let on = match (a.1, b.1) {
                (Some((s, u0, v0)), Some((_, u1, v1))) => Some((s, mix(u0, u1, t), mix(v0, v1, t))),
                _ => a.1,
            };
            (a.0.lerp(b.0, t), on)
        };
        // Where the line between a visible point and a hidden one goes under.
        let edge = |seen: Tagged, unseen: Tagged| {
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..16 {
                let mid = (lo + hi) / 2.0;
                if self.hidden(between(seen, unseen, mid)) { hi = mid } else { lo = mid }
            }
            (between(seen, unseen, (lo + hi) / 2.0).0.flat(), false)
        };
        let mut runs: Vec<Run> = Vec::new();
        let mut run: Run = Vec::new();
        for i in 0..line.len() {
            let (point, corner) = line[i];
            match (i > 0 && hidden[i - 1], hidden[i]) {
                (true, false) => {
                    run.push(edge(point, line[i - 1].0));
                    run.push((point.0.flat(), corner));
                }
                (false, false) => run.push((point.0.flat(), corner)),
                (false, true) => {
                    if i > 0 {
                        run.push(edge(line[i - 1].0, point));
                    }
                    runs.push(std::mem::take(&mut run));
                }
                (true, true) => {}
            }
        }
        runs.push(run);
        out.extend(runs.into_iter().filter(|run| {
            run.len() > 1 && run.windows(2).map(|w| (w[1].0 - w[0].0).len()).sum::<f64>() >= self.speck
        }));
    }

    /// The folds of a surface: where it turns from facing the viewer to
    /// facing away, traced across its grid (marching squares).
    fn folds(&self, index: usize) -> Vec<Line> {
        let surface = &self.surfaces[index];
        let (nu, nv) = (surface.nu * 2, surface.nv * 2);
        let at = |i: usize, j: usize| (i as f64 / nu as f64, -1.0 + 2.0 * j as f64 / nv as f64);
        let (du, dv) = (0.25 / nu as f64, 0.5 / nv as f64);
        let facing: Vec<Vec<f64>> = (0..=nu).into_par_iter().map(|i| (0..=nv).map(|j| {
            // Just inside the ends, where a surface can pinch to a point
            // and have no facing.
            let (u, v) = at(i, j);
            let u = u.clamp(0.02, 0.98);
            let p = |u: f64, v: f64| (surface.point)(u.clamp(0.0, 1.0), v.clamp(-1.0, 1.0));
            let along = p(u + du, v) - p(u - du, v);
            let across = p(u, v + dv) - p(u, v - dv);
            along.cross(across).z
        }).collect()).collect();
        // Where the facing changes sign along each grid edge; an edge is
        // (cell corner, whether it runs along v).
        let crossing = |i: usize, j: usize, along_v: bool| -> Option<(f64, f64)> {
            let (i2, j2) = if along_v { (i, j + 1) } else { (i + 1, j) };
            let ((u0, v0), (u1, v1)) = (at(i, j), at(i2, j2));
            if surface.crease && (v0 == 0.0 || v1 == 0.0) {
                return None;
            }
            let (a, b) = (facing[i][j], facing[i2][j2]);
            if (a > 0.0) == (b > 0.0) {
                return None;
            }
            let t = a / (a - b);
            Some((mix(u0, u1, t), mix(v0, v1, t)))
        };
        type Edge = (usize, usize, bool);
        let mut links: HashMap<Edge, Vec<Edge>> = HashMap::new();
        for i in 0..nu {
            for j in 0..nv {
                let sides: Vec<Edge> = [(i, j, false), (i + 1, j, true), (i, j + 1, false), (i, j, true)].into_iter()
                    .filter(|&(a, b, along_v)| crossing(a, b, along_v).is_some())
                    .collect();
                let pairs: Vec<(Edge, Edge)> = match sides.len() {
                    2 => vec![(sides[0], sides[1])],
                    // A saddle: pair them by the cell's middle.
                    4 => {
                        let middle = facing[i][j] + facing[i + 1][j] + facing[i][j + 1] + facing[i + 1][j + 1];
                        if (middle > 0.0) == (facing[i][j] > 0.0) {
                            vec![(sides[0], sides[1]), (sides[2], sides[3])]
                        } else {
                            vec![(sides[0], sides[3]), (sides[1], sides[2])]
                        }
                    }
                    _ => vec![],
                };
                for (a, b) in pairs {
                    links.entry(a).or_default().push(b);
                    links.entry(b).or_default().push(a);
                }
            }
        }
        // Chain the crossings into lines, starting from their ends.
        let mut keys: Vec<Edge> = links.keys().copied().collect();
        keys.sort();
        keys.sort_by_key(|k| links[k].len());
        let mut used: HashSet<Edge> = HashSet::new();
        let mut lines = Vec::new();
        for start in keys {
            if used.contains(&start) {
                continue;
            }
            let mut chain = vec![start];
            used.insert(start);
            let mut current = start;
            while let Some(&next) = links[&current].iter().find(|e| !used.contains(e)) {
                used.insert(next);
                chain.push(next);
                current = next;
            }
            if chain.len() < 2 {
                continue;
            }
            lines.push(chain.into_iter().map(|(i, j, along_v)| {
                let (u, v) = crossing(i, j, along_v).expect("linked edges cross");
                (self.on(index, u, v), false)
            }).collect());
        }
        lines
    }
}
