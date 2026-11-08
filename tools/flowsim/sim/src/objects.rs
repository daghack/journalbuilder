//! The things placed in the flow. There are four families:
//!
//! - **obstacles** (pebble, rect, wall), which the fluid flows round, and
//!   particles stop at or bounce off;
//! - **stirrers** (jet, vortex, source), fixed particles that push the
//!   fluid itself;
//! - **attractors**, which pull (or, with a negative strength, push) the
//!   particles but never the fluid;
//! - **seeds** (seed_point, seed_line, seed_ring), where particles start.
//!
//! Sources and attractors can be `moving`: they then move like particles,
//! carried by the flow and pulled by attractors, the moving ones included.
//!
//! Angles are in degrees, clockwise on screen from the +x axis.

use crate::geom::{V2, segment_distance, v2};
use crate::grid::{Layout, sample_linear};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub id: u32,
    #[serde(flatten)]
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    /// A round obstacle.
    Pebble { pos: V2, radius: f32 },
    /// A rectangular obstacle, `size` wide and high, turned `angle` about
    /// its centre.
    Rect {
        pos: V2,
        size: V2,
        #[serde(default)]
        angle: f32,
    },
    /// A drawn wall: a polyline `thickness` thick.
    Wall { points: Vec<V2>, thickness: f32 },
    /// Pushes the fluid within about `radius` toward `speed` in the
    /// direction `angle`.
    Jet {
        pos: V2,
        angle: f32,
        speed: f32,
        radius: f32,
    },
    /// Spins the fluid within about `radius`: `spin` is the speed at the
    /// radius, clockwise when positive.
    Vortex { pos: V2, spin: f32, radius: f32 },
    /// Pours fluid out (or, with a negative `speed`, draws it in): `speed`
    /// is how fast it crosses the circle of `radius`. A `moving` one moves
    /// like a particle.
    Source {
        pos: V2,
        speed: f32,
        radius: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        moving: bool,
    },
    /// Pulls particles toward it at up to `strength` (pushes them away when
    /// negative), with a tangential `spin` that makes them orbit, falling
    /// off over `radius`. A particle closer than `capture` is caught and its
    /// path ends. A `moving` one moves like a particle (and catches none
    /// of the other moving ones).
    Attractor {
        pos: V2,
        strength: f32,
        radius: f32,
        #[serde(default)]
        falloff: Falloff,
        #[serde(default)]
        spin: f32,
        #[serde(default)]
        capture: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        moving: bool,
    },
    /// One particle start. While the flow runs it releases `emit_rate`
    /// particles a second.
    SeedPoint {
        pos: V2,
        #[serde(default)]
        emit_rate: f32,
    },
    /// `count` particle starts along the segment from `a` to `b`.
    SeedLine {
        a: V2,
        b: V2,
        count: u32,
        #[serde(default)]
        emit_rate: f32,
    },
    /// `count` particle starts round a circle.
    SeedRing {
        pos: V2,
        radius: f32,
        count: u32,
        #[serde(default)]
        emit_rate: f32,
    },
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Falloff {
    /// strength · r² / (d² + r²): strong close in, with a long reach.
    #[default]
    InverseSquare,
    /// strength · exp(−d²/r²): only near the attractor.
    Gaussian,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Obstacle,
    Stirrer,
    Attractor,
    Seed,
}

impl Kind {
    pub fn family(&self) -> Family {
        match self {
            Kind::Pebble { .. } | Kind::Rect { .. } | Kind::Wall { .. } => Family::Obstacle,
            Kind::Jet { .. } | Kind::Vortex { .. } | Kind::Source { .. } => Family::Stirrer,
            Kind::Attractor { .. } => Family::Attractor,
            Kind::SeedPoint { .. } | Kind::SeedLine { .. } | Kind::SeedRing { .. } => Family::Seed,
        }
    }

    /// Where a seed's particles start; nothing for other objects.
    pub fn seed_points(&self) -> Vec<V2> {
        match *self {
            Kind::SeedPoint { pos, .. } => vec![pos],
            Kind::SeedLine { a, b, count, .. } => {
                let n = count.max(1);
                if n == 1 {
                    return vec![a.lerp(b, 0.5)];
                }
                (0..n).map(|i| a.lerp(b, i as f32 / (n - 1) as f32)).collect()
            }
            Kind::SeedRing { pos, radius, count, .. } => {
                let n = count.max(1);
                (0..n)
                    .map(|i| {
                        let t = i as f32 / n as f32 * std::f32::consts::TAU;
                        pos + v2(t.cos(), t.sin()) * radius
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Whether it moves like a particle.
    pub fn moving(&self) -> bool {
        matches!(*self, Kind::Source { moving: true, .. } | Kind::Attractor { moving: true, .. })
    }

    /// Where a moving object is.
    pub fn pos(&self) -> Option<V2> {
        match *self {
            Kind::Source { pos, .. } | Kind::Attractor { pos, .. } => Some(pos),
            _ => None,
        }
    }

    /// Moves a source or attractor to `p`.
    pub fn set_pos(&mut self, p: V2) {
        if let Kind::Source { pos, .. } | Kind::Attractor { pos, .. } = self {
            *pos = p;
        }
    }

    pub fn emit_rate(&self) -> f32 {
        match *self {
            Kind::SeedPoint { emit_rate, .. }
            | Kind::SeedLine { emit_rate, .. }
            | Kind::SeedRing { emit_rate, .. } => emit_rate.max(0.0),
            _ => 0.0,
        }
    }
}

/// An obstacle ready to measure distances to.
#[derive(Clone, Debug)]
enum Shape {
    Circle { c: V2, r: f32 },
    Box { c: V2, half: V2, cos: f32, sin: f32 },
    Polyline { points: Vec<V2>, half: f32 },
    /// A picture's dark parts, grown by `extra`.
    Mask { mask: Rc<Mask>, extra: f32 },
}

impl Shape {
    fn sdf(&self, p: V2) -> f32 {
        match self {
            Shape::Circle { c, r } => p.dist(*c) - r,
            Shape::Box { c, half, cos, sin } => {
                let d = p - *c;
                // Into the box's frame: turned back by its angle.
                let l = v2(d.x * cos + d.y * sin, -d.x * sin + d.y * cos);
                let q = v2(l.x.abs() - half.x, l.y.abs() - half.y);
                let outside = v2(q.x.max(0.0), q.y.max(0.0)).len();
                outside + q.x.max(q.y).min(0.0)
            }
            Shape::Polyline { points, half } => {
                let d = match points.len() {
                    0 => f32::INFINITY,
                    1 => p.dist(points[0]),
                    _ => points
                        .windows(2)
                        .map(|w| segment_distance(p, w[0], w[1]))
                        .fold(f32::INFINITY, f32::min),
                };
                d - half
            }
            Shape::Mask { mask, extra } => mask.sdf(p) - extra,
        }
    }
}

/// An obstacle given as a picture, as a host draws it (a page's strokes,
/// say): `nx` × `ny` square cells `cell` across from the domain's corner,
/// as signed distances to the picture's solid parts, grown by `grow`.
#[derive(Clone, Debug)]
pub struct Mask {
    nx: usize,
    ny: usize,
    cell: f32,
    /// Per cell, negative inside.
    dist: Vec<f32>,
}

impl Mask {
    /// A mask from an alpha raster, row by row: cells at least half opaque
    /// are solid. `None` if none is.
    pub fn from_alpha(nx: usize, ny: usize, cell: f32, alpha: &[u8], grow: f32) -> Option<Mask> {
        if nx < 2 || ny < 2 || alpha.len() != nx * ny || !(cell > 0.0 && cell.is_finite()) {
            return None;
        }
        let inside: Vec<bool> = alpha.iter().map(|&a| a >= 128).collect();
        if !inside.contains(&true) {
            return None;
        }
        let to_inside = distance_transform(&inside, nx, ny);
        let outside: Vec<bool> = inside.iter().map(|x| !x).collect();
        let to_outside = distance_transform(&outside, nx, ny);
        let grow = grow.max(0.0);
        let dist = inside
            .iter()
            .enumerate()
            .map(|(k, &solid)| {
                // Cells meet halfway: the edge is half a cell from each.
                let cells = if solid { -(to_outside[k] - 0.5) } else { to_inside[k] - 0.5 };
                cells * cell - grow
            })
            .collect();
        Some(Mask { nx, ny, cell, dist })
    }

    /// The signed distance at `p`, interpolated, held at the edges.
    pub fn sdf(&self, p: V2) -> f32 {
        let layout = Layout { w: self.nx, rows: self.ny, ox: 0.5, oy: 0.5 };
        sample_linear(&self.dist, layout, 1.0 / self.cell, p)
    }
}

/// Each cell's distance, in cells, to the nearest one that's `on`: the
/// exact Euclidean distance transform (Felzenszwalb and Huttenlocher), a
/// pass along the rows and one down the columns.
fn distance_transform(on: &[bool], nx: usize, ny: usize) -> Vec<f32> {
    const FAR: f32 = 1e12;
    let mut grid: Vec<f32> = on.iter().map(|&x| if x { 0.0 } else { FAR }).collect();
    let n = nx.max(ny);
    let (mut f, mut d, mut v, mut z) = (vec![0.0; n], vec![0.0; n], vec![0usize; n], vec![0.0; n + 1]);
    for i in 0..nx {
        for j in 0..ny {
            f[j] = grid[i + j * nx];
        }
        squared_1d(&f[..ny], &mut d[..ny], &mut v, &mut z);
        for j in 0..ny {
            grid[i + j * nx] = d[j];
        }
    }
    for j in 0..ny {
        f[..nx].copy_from_slice(&grid[j * nx..(j + 1) * nx]);
        squared_1d(&f[..nx], &mut d[..nx], &mut v, &mut z);
        grid[j * nx..(j + 1) * nx].copy_from_slice(&d[..nx]);
    }
    grid.iter().map(|x| x.sqrt()).collect()
}

/// The squared distance transform of one row of costs: the lower envelope
/// of the parabolas rooted at each.
fn squared_1d(f: &[f32], d: &mut [f32], v: &mut [usize], z: &mut [f32]) {
    let n = f.len();
    let mut k = 0;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    let meet = |q: usize, p: usize| ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * q as f32 - 2.0 * p as f32);
    for q in 1..n {
        let mut s = meet(q, v[k]);
        while s <= z[k] {
            k -= 1;
            s = meet(q, v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let dq = q as f32 - v[k] as f32;
        *out = dq * dq + f[v[k]];
    }
}

/// The scene's obstacles, as signed distances: negative inside.
#[derive(Clone, Debug, Default)]
pub struct Obstacles {
    shapes: Vec<Shape>,
}

impl Obstacles {
    pub fn new(objects: &[Object]) -> Self {
        let shapes = objects
            .iter()
            .filter_map(|o| match &o.kind {
                Kind::Pebble { pos, radius } => Some(Shape::Circle { c: *pos, r: radius.max(0.0) }),
                Kind::Rect { pos, size, angle } => {
                    let (sin, cos) = angle.to_radians().sin_cos();
                    Some(Shape::Box {
                        c: *pos,
                        half: v2(size.x.abs() * 0.5, size.y.abs() * 0.5),
                        cos,
                        sin,
                    })
                }
                Kind::Wall { points, thickness } => Some(Shape::Polyline {
                    points: points.clone(),
                    half: thickness.max(0.0) * 0.5,
                }),
                _ => None,
            })
            .collect();
        Obstacles { shapes }
    }

    /// These obstacles and a mask's, grown by `extra`.
    pub fn with_mask(mut self, mask: Option<&Rc<Mask>>, extra: f32) -> Self {
        if let Some(mask) = mask {
            self.shapes.push(Shape::Mask { mask: mask.clone(), extra });
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }

    /// The signed distance to the nearest obstacle (infinite with none).
    pub fn sdf(&self, p: V2) -> f32 {
        self.shapes.iter().map(|s| s.sdf(p)).fold(f32::INFINITY, f32::min)
    }

    /// The outward direction at `p`, from the distance's gradient.
    pub fn normal(&self, p: V2) -> V2 {
        let e = 0.25;
        let dx = self.sdf(p + v2(e, 0.0)) - self.sdf(p - v2(e, 0.0));
        let dy = self.sdf(p + v2(0.0, e)) - self.sdf(p - v2(0.0, e));
        v2(dx, dy).normalized()
    }
}

#[derive(Clone, Debug)]
struct Attr {
    id: u32,
    pos: V2,
    strength: f32,
    r2: f32,
    gaussian: bool,
    spin: f32,
    capture2: f32,
}

/// The scene's attractors and repellers.
#[derive(Clone, Debug, Default)]
pub struct Attractors {
    list: Vec<Attr>,
}

impl Attractors {
    pub fn new(objects: &[Object]) -> Self {
        let list = objects
            .iter()
            .filter_map(|o| match o.kind {
                Kind::Attractor { pos, strength, radius, falloff, spin, capture, .. } => Some(Attr {
                    id: o.id,
                    pos,
                    strength,
                    r2: (radius * radius).max(1e-6),
                    gaussian: falloff == Falloff::Gaussian,
                    spin,
                    capture2: capture.max(0.0).powi(2),
                }),
                _ => None,
            })
            .collect();
        Attractors { list }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The drift the attractors add to a particle's velocity at `p`.
    pub fn drift(&self, p: V2) -> V2 {
        self.drift_except(p, None)
    }

    /// The drift at `p` from every attractor but the one with id `skip`:
    /// a moving attractor doesn't pull itself.
    pub fn drift_except(&self, p: V2, skip: Option<u32>) -> V2 {
        let mut f = V2::default();
        for a in &self.list {
            if Some(a.id) == skip {
                continue;
            }
            let d = a.pos - p;
            let d2 = d.len2();
            if d2 < 1e-8 {
                continue;
            }
            let profile = if a.gaussian { (-d2 / a.r2).exp() } else { a.r2 / (d2 + a.r2) };
            let dir = d / d2.sqrt();
            // Spin turns about the attractor clockwise on screen: the
            // tangent of -dir (pointing out) turned a quarter.
            f += (dir * a.strength + (-dir).perp() * a.spin) * profile;
        }
        f
    }

    /// Whether an attractor has caught a particle at `p`.
    pub fn captures(&self, p: V2) -> bool {
        self.list.iter().any(|a| a.capture2 > 0.0 && (a.pos - p).len2() <= a.capture2)
    }
}
