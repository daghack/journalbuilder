//! Flowers modelled in 3D and drawn in outline.
//!
//! Petals and sepals are curved surfaces around a receptacle: each leaves
//! it at an angle, curls along its length and cups across it. How far, and
//! how big the petals have grown, follows the flower's openness: from a
//! closed bud wrapped in its sepals, through a half-open cup, to a full
//! bloom with its sepals folded back behind. Seen from any angle, its lines
//! are the petals' and sepals' edges, the folds where their surfaces turn
//! away from the viewer, the stamens and the receptacle's rim, each drawn
//! where nothing nearer hides it.
//!
//! Coordinates are the page's, x right and y down, with z towards the
//! viewer; the view is straight down z.

use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::ops::{Add, Mul, Sub};

use rand::rngs::StdRng;
use rand::Rng;

use super::geom::V;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct P3 {
    x: f64,
    y: f64,
    z: f64,
}

impl P3 {
    const fn new(x: f64, y: f64, z: f64) -> Self {
        P3 { x, y, z }
    }
    fn dot(self, o: P3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    fn cross(self, o: P3) -> P3 {
        P3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    fn unit(self) -> P3 {
        self * (1.0 / self.dot(self).sqrt())
    }
    fn lerp(self, o: P3, t: f64) -> P3 {
        self + (o - self) * t
    }
    /// Where it lands on the page.
    fn flat(self) -> V {
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

/// A flower as drawn: its surfaces on the page, which hide whatever is
/// behind it, its visible lines, and the middle of what it covers and how
/// far it reaches from there.
pub(super) struct Bloom {
    pub triangles: Vec<[V; 3]>,
    pub lines: Vec<Vec<V>>,
    pub middle: V,
    pub reach: f64,
}

/// How a flower sits on the page: centred on `center`, `radius` to its
/// petals' tips when fully open, `openness` from 0 (a closed bud) to 1 (a
/// full bloom), and its axis tilted `tilt` radians from facing the viewer
/// towards `facing`, an angle on the page (so `tilt` pi/2 is side on,
/// facing that way).
pub(super) struct Pose {
    pub center: V,
    pub radius: f64,
    pub openness: f64,
    pub tilt: f64,
    pub facing: f64,
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// A surface over (u, v) in [0, 1] x [-1, 1], sampled on a grid; its sides
/// (v = -1 and 1, from the base round the tip) are drawn if `edges`.
struct Surface {
    point: Box<dyn Fn(f64, f64) -> P3>,
    nu: usize,
    nv: usize,
    edges: bool,
}

/// The flower's frame: its centre, its axis (the way it faces) and two
/// directions across it.
struct Frame {
    center: P3,
    axis: P3,
    x: P3,
    y: P3,
}

impl Frame {
    /// Pointing out from the axis at `angle` around it.
    fn radial(&self, angle: f64) -> P3 {
        self.x * angle.cos() + self.y * angle.sin()
    }
}

/// A petal or sepal: where it leaves the receptacle, how it bends, and its
/// half width along it (`width(u)`, at most `widest`).
struct Organ {
    angle: f64,
    base_radius: f64,
    base_height: f64,
    length: f64,
    /// The angle it leaves at, up from flat towards the axis, and how much
    /// more it turns towards the axis by its tip: evenly, with `curl_power`
    /// 1, or more towards the tip with more.
    elevation: f64,
    curl: f64,
    curl_power: f64,
    /// How far across its width it cups, in radians at its widest.
    cup: f64,
    /// How far it twists about its length by its tip.
    twist: f64,
    widest: f64,
    width: Box<dyn Fn(f64) -> f64>,
}

impl Organ {
    fn surface(self, frame: &Frame) -> Surface {
        let radial = frame.radial(self.angle);
        let across = frame.radial(self.angle + FRAC_PI_2);
        let up = frame.axis;
        let base = frame.center + radial * self.base_radius + up * self.base_height;
        // The midline, bending as it goes.
        const STEPS: usize = 48;
        let heading = |s: f64| self.elevation + self.curl * s.powf(self.curl_power);
        let mut midline = vec![base];
        for k in 0..STEPS {
            let theta = heading((k as f64 + 0.5) / STEPS as f64);
            let last = midline[k];
            midline.push(last + (radial * theta.cos() + up * theta.sin()) * (self.length / STEPS as f64));
        }
        let Organ { curl, curl_power, cup, twist, widest, width, elevation, .. } = self;
        let point = move |u: f64, v: f64| {
            let k = (u * STEPS as f64).clamp(0.0, STEPS as f64 - 1e-9);
            let (i, t) = (k as usize, k.fract());
            let at = midline[i].lerp(midline[i + 1], t);
            let theta = elevation + curl * u.powf(curl_power);
            let normal = radial * -theta.sin() + up * theta.cos();
            let (sin, cos) = (twist * u).sin_cos();
            let (side, normal) = (across * cos + normal * sin, normal * cos - across * sin);
            // Cupped across as an arc, so the width doesn't stretch.
            let a = v * width(u);
            let k = cup / widest;
            let (out, rise) = if k.abs() < 1e-9 { (a, 0.0) } else { ((k * a).sin() / k, (1.0 - (k * a).cos()) / k) };
            at + side * out + normal * rise
        };
        Surface { point: Box::new(point), nu: 18, nv: 10, edges: true }
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
type Tagged = (P3, Option<(usize, f64, f64)>);

struct Model {
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
    fn new(surfaces: Vec<Surface>, radius: f64) -> Self {
        let mut triangles = Vec::new();
        for (index, surface) in surfaces.iter().enumerate() {
            let (nu, nv) = (surface.nu, surface.nv);
            let grid: Vec<Vec<P3>> = (0..=nu).map(|i| (0..=nv).map(|j| {
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
        let cell = radius / 8.0;
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
        Model { surfaces, triangles, grid, cell, margin: 0.004 * radius, speck: 0.04 * radius }
    }

    /// Whether a surface nearer than `q` covers it, other than the one its
    /// line runs along, close by.
    fn hidden(&self, (q, on): Tagged) -> bool {
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

    /// The visible stretches of a line, on the page.
    fn visible(&self, line: &[Tagged], out: &mut Vec<Vec<V>>) {
        let start = out.len();
        self.stretches(line, out);
        let speck = self.speck;
        let mut kept = out.split_off(start);
        kept.retain(|run| run.windows(2).map(|w| (w[1] - w[0]).len()).sum::<f64>() >= speck);
        out.extend(kept);
    }

    fn stretches(&self, line: &[Tagged], out: &mut Vec<Vec<V>>) {
        let hidden: Vec<bool> = line.iter().map(|&p| self.hidden(p)).collect();
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
            between(seen, unseen, (lo + hi) / 2.0).0.flat()
        };
        let mut run: Vec<V> = Vec::new();
        for i in 0..line.len() {
            match (i > 0 && hidden[i - 1], hidden[i]) {
                (true, false) => {
                    run.push(edge(line[i], line[i - 1]));
                    run.push(line[i].0.flat());
                }
                (false, false) => run.push(line[i].0.flat()),
                (false, true) => {
                    if i > 0 {
                        run.push(edge(line[i - 1], line[i]));
                    }
                    if run.len() > 1 {
                        out.push(std::mem::take(&mut run));
                    }
                    run.clear();
                }
                (true, true) => {}
            }
        }
        if run.len() > 1 {
            out.push(run);
        }
    }

    /// The sides of a surface, from the base round the tip and back.
    fn edges(&self, index: usize) -> Vec<Tagged> {
        let surface = &self.surfaces[index];
        let n = 3 * surface.nu;
        let side = |v: f64, k: usize| {
            let u = k as f64 / n as f64;
            ((surface.point)(u, v), Some((index, u, v)))
        };
        (0..=n).map(|k| side(-1.0, k)).chain((0..n).rev().map(|k| side(1.0, k))).collect()
    }

    /// The folds of a surface: where it turns from facing the viewer to
    /// facing away, traced across its grid (marching squares).
    fn folds(&self, index: usize) -> Vec<Vec<Tagged>> {
        let surface = &self.surfaces[index];
        let (nu, nv) = (surface.nu * 2, surface.nv * 2);
        let at = |i: usize, j: usize| (i as f64 / nu as f64, -1.0 + 2.0 * j as f64 / nv as f64);
        let (du, dv) = (0.25 / nu as f64, 0.5 / nv as f64);
        let facing: Vec<Vec<f64>> = (0..=nu).map(|i| (0..=nv).map(|j| {
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
            let (a, b) = (facing[i][j], facing[i2][j2]);
            if (a > 0.0) == (b > 0.0) {
                return None;
            }
            let t = a / (a - b);
            let ((u0, v0), (u1, v1)) = (at(i, j), at(i2, j2));
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
        let mut used: std::collections::HashSet<Edge> = std::collections::HashSet::new();
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
                ((surface.point)(u, v), Some((index, u, v)))
            }).collect());
        }
        lines
    }
}

pub(super) fn bloom(pose: &Pose, rng: &mut StdRng) -> Bloom {
    let Pose { center, radius, openness: open, tilt, facing } = *pose;
    // The flower's frame on the page.
    let axis = P3::new(tilt.sin() * facing.cos(), tilt.sin() * facing.sin(), tilt.cos());
    let helper = if axis.z.abs() < 0.9 { P3::new(0.0, 0.0, 1.0) } else { P3::new(1.0, 0.0, 0.0) };
    let x0 = helper.cross(axis).unit();
    let y0 = axis.cross(x0);
    let spin = rng.random_range(0.0..TAU);
    let frame = Frame {
        center: P3::new(center.x, center.y, 0.0),
        axis,
        x: x0 * spin.cos() + y0 * spin.sin(),
        y: y0 * spin.cos() - x0 * spin.sin(),
    };

    let mut surfaces = Vec::new();
    // Sepals open first, as the bud swells.
    let sepals_open = smooth(open * 1.6);
    let start = rng.random_range(0.0..TAU);
    for k in 0..5 {
        // Closed, they leave the base low and arch evenly over the top into
        // a ball, broadest round its middle and cupped to its curve; open,
        // they're narrower, pointed, and folded back.
        let length = radius * mix(0.72, 0.44, sepals_open) * rng.random_range(0.92..1.08);
        let widest = 0.36 * length;
        let ovate = move |u: f64| (PI * u).sin().max(0.0).powf(0.75) * (1.0 - 0.3 * u);
        let lanceolate = move |u: f64| (1.0 - u).powf(0.8) * (0.8 + 0.2 * (PI * u).sin());
        surfaces.push(Organ {
            angle: start + (k as f64 + 0.5) * TAU / 5.0 + rng.random_range(-0.1..0.1),
            base_radius: radius * mix(0.16, 0.1, sepals_open),
            base_height: -0.03 * radius,
            length,
            elevation: mix(0.55, -0.75, sepals_open) + rng.random_range(-0.08..0.08),
            curl: mix(2.35, -0.3, sepals_open),
            curl_power: mix(1.0, 1.3, sepals_open),
            cup: mix(0.9, 0.35, sepals_open),
            twist: rng.random_range(-0.15..0.15),
            widest,
            width: Box::new(move |u: f64| widest * mix(ovate(u), lanceolate(u), sepals_open)),
        }.surface(&frame));
    }
    // Petals grow as they open; each overlaps the next a little outside it.
    let grown = smooth(open);
    let mut order: Vec<usize> = (0..5).collect();
    for k in (1..5).rev() {
        order.swap(k, rng.random_range(0..=k));
    }
    for (k, &rank) in order.iter().enumerate() {
        let length = radius * mix(0.3, 1.0, grown) * rng.random_range(0.92..1.06);
        let widest = length * rng.random_range(0.44..0.52);
        let (wave, frequency, phase) = (rng.random_range(0.02..0.06), rng.random_range(2.0..4.0), rng.random_range(0.0..TAU));
        surfaces.push(Organ {
            angle: start + k as f64 * TAU / 5.0 + rng.random_range(-0.12..0.12),
            base_radius: radius * (0.06 + 0.03 * open + 0.008 * rank as f64),
            base_height: -0.02 * radius * rank as f64,
            length,
            elevation: mix(0.95, 0.1, open.powf(0.9)) - 0.06 * rank as f64 + rng.random_range(-0.06..0.06),
            curl: mix(2.2, -0.1, open) + rng.random_range(-0.14..0.14),
            curl_power: mix(1.0, 1.3, open),
            cup: mix(1.6, 0.45, open),
            twist: rng.random_range(-0.2..0.2),
            widest,
            width: Box::new(move |u: f64| {
                let shape = if u < 0.62 {
                    0.22 + 0.78 * (FRAC_PI_2 * u / 0.62).sin()
                } else {
                    (1.0 - ((u - 0.62) / 0.38).powi(2)).max(0.0).sqrt()
                };
                widest * shape * (1.0 + wave * u * (frequency * PI * u + phase).sin())
            }),
        }.surface(&frame));
    }
    // The cup under the flower that its stalk runs into, and the
    // receptacle in its middle.
    let cup_frame = (frame.center, frame.axis, frame.x, frame.y);
    surfaces.push(Surface {
        point: Box::new(move |u: f64, v: f64| {
            let (c, a, x, y) = cup_frame;
            let angle = PI * v;
            c + (x * angle.cos() + y * angle.sin()) * (radius * mix(0.03, 0.1, u)) + a * (radius * mix(-0.09, -0.03, u))
        }),
        nu: 4,
        nv: 16,
        edges: false,
    });
    surfaces.push(Surface {
        point: Box::new(move |u: f64, v: f64| {
            let (c, a, x, y) = cup_frame;
            let angle = PI * v;
            c + (x * angle.cos() + y * angle.sin()) * (radius * 0.1 * u) + a * (radius * 0.02)
        }),
        nu: 2,
        nv: 16,
        edges: false,
    });
    let receptacle = surfaces.len() - 1;

    let model = Model::new(surfaces, radius);
    let mut lines = Vec::new();
    for index in 0..model.surfaces.len() {
        if model.surfaces[index].edges {
            model.visible(&model.edges(index), &mut lines);
        }
        // The receptacle is flat: its rim is its only line.
        if index != receptacle {
            for fold in model.folds(index) {
                model.visible(&fold, &mut lines);
            }
        }
    }
    // The receptacle's rim, and stamens rising from it, once it's open.
    if open > 0.25 {
        let rim: Vec<Tagged> = (0..=32).map(|k| {
            let v = -1.0 + 2.0 * k as f64 / 32.0;
            ((model.surfaces[receptacle].point)(1.0, v), None)
        }).collect();
        model.visible(&rim, &mut lines);
        let count = rng.random_range(22..32);
        for k in 0..count {
            let angle = TAU * k as f64 / count as f64 + rng.random_range(-0.08..0.08);
            let rise = mix(1.2, 0.5, open) + rng.random_range(-0.15..0.15);
            let length = radius * mix(0.2, 0.34, open) * rng.random_range(0.85..1.15);
            let base = frame.center + frame.radial(angle) * (0.1 * radius) + frame.axis * (0.02 * radius);
            let out = frame.radial(angle);
            let stamen: Vec<Tagged> = (0..=4).map(|j| {
                let t = j as f64 / 4.0;
                let theta = rise - 0.3 * t;
                (base + (out * theta.cos() + frame.axis * theta.sin()) * (length * t), None)
            }).collect();
            let tip = stamen[stamen.len() - 1];
            model.visible(&stamen, &mut lines);
            if !model.hidden(tip) {
                let anther = tip.0.flat();
                lines.push((0..=8).map(|j| anther + V::angle(TAU * j as f64 / 8.0) * (0.022 * radius)).collect());
            }
        }
    }
    let triangles: Vec<[V; 3]> = model.triangles.iter().map(|t| t.corners.map(|p| p.flat())).collect();
    let count = 3.0 * triangles.len() as f64;
    let middle = triangles.iter().flatten().fold(V::default(), |sum, &p| sum + p) * (1.0 / count);
    let reach = triangles.iter().flatten().map(|&p| (p - middle).len()).fold(0.0, f64::max);
    Bloom { triangles, lines, middle, reach }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn draw(openness: f64, tilt: f64, facing: f64) -> Bloom {
        let pose = Pose { center: V::new(0.0, 0.0), radius: 100.0, openness, tilt, facing };
        bloom(&pose, &mut StdRng::seed_from_u64(4))
    }

    /// Anthers: the small closed loops at the stamens' tips.
    fn anthers(bloom: &Bloom) -> usize {
        bloom.lines.iter().filter(|line| line.len() == 9 && (line[0] - line[8]).len() < 1e-9).count()
    }

    #[test]
    fn flowers_open_from_closed_buds() {
        let (bud, open) = (draw(0.0, 0.0, 0.0), draw(1.0, 0.0, 0.0));
        assert!(open.reach > 1.8 * bud.reach, "bud {} vs open {}", bud.reach, open.reach);
        // A bud's sepals hide its stamens; an open flower's face shows them.
        assert_eq!(anthers(&bud), 0);
        assert!(anthers(&open) > 15, "{} anthers", anthers(&open));
        for bloom in [&bud, &open] {
            assert!(!bloom.lines.is_empty());
            assert!(bloom.lines.iter().flatten().chain(bloom.triangles.iter().flatten()).all(|p| p.is_finite()));
        }
    }

    #[test]
    fn a_flower_seen_side_on_faces_away_from_its_stalk() {
        // Facing +x, side on, a bud or half-open flower reaches out to the
        // right of where it sits, much further than its sepals fold back.
        for openness in [0.0, 0.5] {
            let bloom = draw(openness, FRAC_PI_2, 0.0);
            let xs = bloom.triangles.iter().flatten().map(|p| p.x);
            let (back, out) = xs.fold((0.0, 0.0), |(lo, hi): (f64, f64), x| (lo.min(x), hi.max(x)));
            assert!(out > 1.5 * -back && out > 30.0, "openness {}: reaches {} forward, {} back", openness, out, -back);
        }
    }

    #[test]
    fn the_same_generator_draws_the_same_flower() {
        let lines = |bloom: Bloom| format!("{:?}", bloom.lines);
        assert_eq!(lines(draw(0.6, 0.8, 1.0)), lines(draw(0.6, 0.8, 1.0)));
    }
}
