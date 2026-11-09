//! A graph laid out by forces: its nodes boxes of the sizes given (a name
//! and glyphs, say), its edges lines between their borders, each edge's
//! label a box of the size given set on its line: along it, beside it, or
//! upright across it.
//!
//! Boxes and labels are oriented rectangles; whatever overlaps is pushed
//! apart along the axis it overlaps least on. Each pair of joined nodes is
//! pulled or pushed to a length that leaves room between their borders for
//! the longest of its edges' labels; edges push away the nodes that aren't
//! theirs (as in PrEd), and labels push away boxes and labels that aren't
//! theirs. A weak pull to the middle keeps the pieces of a graph together,
//! and every node is spread from the others by a little of Fruchterman and
//! Reingold's repulsion, so a graph unfolds rather than crumpling. The
//! moves cool as they go, from the best of a few seeded starts.
//!
//! Only `+ - * /` and square roots are used while it's laid out (no
//! `hypot`, no trigonometry), so it lays out the same in the browser.

use std::f64::consts::{FRAC_PI_2, PI};

use parry2d::math::Vec2;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::geom::V;
use super::starchart::segments_meet;

/// Where an edge's label goes on its line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelPlace {
    /// On the line, turned to it: the line breaks for it.
    Along,
    /// Turned to the line, above it: the line runs under it unbroken.
    Beside,
    /// Upright, across the line, which breaks round it.
    Upright,
}

impl LabelPlace {
    pub fn named(name: &str) -> Result<LabelPlace, String> {
        match name {
            "along" => Ok(LabelPlace::Along),
            "beside" => Ok(LabelPlace::Beside),
            "upright" => Ok(LabelPlace::Upright),
            other => Err(format!("unknown label_place '{}'; it's \"along\", \"beside\" or \"upright\"", other)),
        }
    }
}

pub struct GraphLayoutSettings {
    /// The box the graph is laid out in, from (0, 0); grown, keeping its
    /// shape, when what's in it needs more room.
    pub width: f64,
    pub height: f64,
    /// Room kept between boxes, labels and lines.
    pub gap: f64,
    pub seed: u64,
    /// How many steps the forces take, from each start.
    pub iterations: usize,
    pub place: LabelPlace,
}

/// The most steps a layout takes.
pub const MOST_ITERATIONS: usize = 5000;

/// How many seeded starts are tried; the best is kept.
const STARTS: u64 = 4;

/// An edge as it's drawn.
#[derive(Clone, Debug)]
pub struct EdgeLayout {
    /// Where its line leaves its first node's box and meets its second's,
    /// `gap` short of each.
    pub start: Vec2,
    pub end: Vec2,
    /// The middle of its label.
    pub label: Vec2,
    /// How far to turn the label, in radians: the line's angle, folded so
    /// text reads left to right (and up, on an upright line); 0 for an
    /// upright label.
    pub angle: f64,
    /// The line's pieces: from `from` to `to`, broken round the label
    /// where it's set on the line.
    pub pieces: Vec<(Vec2, Vec2)>,
}

#[derive(Debug)]
pub struct GraphLayout {
    /// Each node's box's middle.
    pub points: Vec<Vec2>,
    /// The box it was laid out in: the one asked for, or bigger.
    pub size: (f64, f64),
    pub edges: Vec<EdgeLayout>,
}

/// Lay out a graph of nodes of `sizes` (width, height), joined by `edges`
/// (pairs of indices), each with a label of the size given or none.
pub fn lay_out_graph(sizes: &[(f64, f64)], edges: &[(usize, usize)], labels: &[Option<(f64, f64)>], s: &GraphLayoutSettings) -> Result<GraphLayout, String> {
    check(sizes, edges, labels, s)?;
    let graph = Graph::new(sizes, edges, labels, s);
    if sizes.is_empty() {
        return Ok(GraphLayout { points: Vec::new(), size: (graph.width, graph.height), edges: Vec::new() });
    }
    let mut best: Option<(f64, Vec<V>)> = None;
    for start in 0..STARTS {
        let mut rng = StdRng::seed_from_u64(s.seed.wrapping_mul(STARTS).wrapping_add(start));
        let mut pos = graph.start(&mut rng);
        graph.relax(&mut pos, s.iterations, &mut rng);
        graph.tidy(&mut pos);
        let score = graph.score(&pos);
        if best.as_ref().is_none_or(|(b, _)| score < *b) {
            best = Some((score, pos));
        }
    }
    let (_, mut pos) = best.expect("a start was tried");
    let size = graph.fit(&mut pos);
    let lines = graph.lines(&pos);
    Ok(GraphLayout {
        points: pos.iter().map(|&p| vec2(p)).collect(),
        size,
        edges: lines.iter().map(|l| graph.edge_layout(l, &lines)).collect(),
    })
}

fn check(sizes: &[(f64, f64)], edges: &[(usize, usize)], labels: &[Option<(f64, f64)>], s: &GraphLayoutSettings) -> Result<(), String> {
    if !(s.width > 0.0 && s.width.is_finite() && s.height > 0.0 && s.height.is_finite()) {
        return Err(format!("width and height must be positive numbers, got {} and {}", s.width, s.height));
    }
    if !(s.gap >= 0.0 && s.gap.is_finite()) {
        return Err(format!("gap must be a number of at least 0, got {}", s.gap));
    }
    if s.iterations > MOST_ITERATIONS {
        return Err(format!("iterations must be at most {}, got {}", MOST_ITERATIONS, s.iterations));
    }
    let size_ok = |(w, h): (f64, f64)| w >= 0.0 && h >= 0.0 && w.is_finite() && h.is_finite();
    if let Some(k) = sizes.iter().position(|&wh| !size_ok(wh)) {
        return Err(format!("sizes[{}] should be a width and height of at least 0, got {:?}", k, sizes[k]));
    }
    for (e, &(a, b)) in edges.iter().enumerate() {
        if a >= sizes.len() || b >= sizes.len() {
            return Err(format!("edge {} joins {} and {}, but there are {} nodes", e, a, b, sizes.len()));
        }
        if a == b {
            return Err(format!("edge {} joins node {} to itself", e, a));
        }
    }
    if labels.len() != edges.len() {
        return Err(format!("there are {} labels for {} edges", labels.len(), edges.len()));
    }
    if let Some(e) = labels.iter().position(|l| l.is_some_and(|wh| !size_ok(wh))) {
        return Err(format!("labels[{}] should be a width and height of at least 0, got {:?}", e, labels[e]));
    }
    Ok(())
}

fn vec2(p: V) -> Vec2 {
    Vec2::new(p.x as f32, p.y as f32)
}

/// A vector's length, as a square root, the same everywhere.
fn norm(v: V) -> f64 {
    (v.x * v.x + v.y * v.y).sqrt()
}

/// `v` made a unit vector, or +x if it's too short to have a direction.
fn unit(v: V) -> V {
    let n = norm(v);
    if n > 1e-9 { v * (1.0 / n) } else { V::new(1.0, 0.0) }
}

/// A rectangle about `c`, its sides `half` from it along `u` (a unit
/// vector) and across.
#[derive(Clone, Copy, Debug)]
struct Rect {
    c: V,
    u: V,
    half: V,
}

impl Rect {
    fn upright(c: V, half: V) -> Rect {
        Rect { c, u: V::new(1.0, 0.0), half }
    }
    /// How far it reaches from its middle along the unit vector `k`.
    fn reach(&self, k: V) -> f64 {
        self.half.x * self.u.dot(k).abs() + self.half.y * self.u.perp().dot(k).abs()
    }
    /// Where the line from `p` going `u` (a unit vector) is over this,
    /// grown by `grow` all round: from and to how far along it.
    fn across(&self, p: V, u: V, grow: f64) -> Option<(f64, f64)> {
        let (d, n) = (p - self.c, self.u.perp());
        slab(V::new(d.dot(self.u), d.dot(n)), V::new(u.dot(self.u), u.dot(n)), self.half + V::new(grow, grow))
    }
    /// How far `b` would move, and which way, to come `margin` clear of
    /// this: `None` if it's that far off already.
    fn overlap(&self, b: &Rect, margin: f64) -> Option<(V, f64)> {
        let d = b.c - self.c;
        let mut least: Option<(V, f64)> = None;
        for k in [self.u, self.u.perp(), b.u, b.u.perp()] {
            let along = d.dot(k);
            let depth = self.reach(k) + b.reach(k) + margin - along.abs();
            if depth <= 0.0 {
                return None;
            }
            if least.is_none_or(|(_, l)| depth < l) {
                least = Some((if along < 0.0 { k * -1.0 } else { k }, depth));
            }
        }
        least
    }
}

/// How far from a box's middle its border is, going along the unit
/// vector `u`.
fn border(half: V, u: V) -> f64 {
    let (ux, uy) = (u.x.abs(), u.y.abs());
    match (ux > 1e-12, uy > 1e-12) {
        (true, true) => (half.x / ux).min(half.y / uy),
        (true, false) => half.x / ux,
        (false, true) => half.y / uy,
        (false, false) => 0.0,
    }
}

/// How far along the line from `p`, heading `u`, it leaves the box about
/// `c` (it starts inside), or enters it (it starts outside, `entering`).
fn crossing(p: V, u: V, c: V, half: V, entering: bool) -> Option<f64> {
    slab(p - c, u, half).map(|(t0, t1)| if entering { t0 } else { t1 })
}

/// Where the line from `p` going `u` is in the box `half` about (0, 0):
/// from and to how far along it.
fn slab(p: V, u: V, half: V) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (f64::MIN, f64::MAX);
    for (pv, uv, hv) in [(p.x, u.x, half.x), (p.y, u.y, half.y)] {
        if uv.abs() < 1e-12 {
            if pv.abs() > hv {
                return None;
            }
            continue;
        }
        let (a, b) = ((-hv - pv) / uv, (hv - pv) / uv);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    if t0 > t1 { None } else { Some((t0, t1)) }
}

/// Two joined nodes, and the edges between them.
struct Pair {
    a: usize,
    b: usize,
    edges: Vec<usize>,
}

/// An edge's line where things are now.
struct Line {
    edge: usize,
    /// Where its line starts and its way (in the edge's direction), and
    /// how far along it is free of both boxes.
    start: V,
    u: V,
    free: (f64, f64),
    /// Its label's box, if it has one, and how far along the line the
    /// label's middle is.
    label: Option<(Rect, f64)>,
}

struct Graph {
    half: Vec<V>,
    edges: Vec<(usize, usize)>,
    /// Each edge's label's width and height, halved (zero for none).
    labels: Vec<Option<V>>,
    pairs: Vec<Pair>,
    place: LabelPlace,
    gap: f64,
    width: f64,
    height: f64,
    /// How far apart nodes spread evenly would be.
    k: f64,
}

impl Graph {
    fn new(sizes: &[(f64, f64)], edges: &[(usize, usize)], labels: &[Option<(f64, f64)>], s: &GraphLayoutSettings) -> Graph {
        let half: Vec<V> = sizes.iter().map(|&(w, h)| V::new(w / 2.0, h / 2.0)).collect();
        let labels: Vec<Option<V>> = labels.iter().map(|l| l.map(|(w, h)| V::new(w / 2.0, h / 2.0))).collect();
        // Edges between the same two nodes, either way, are a pair, in the
        // order their first edges come.
        let mut pairs: Vec<Pair> = Vec::new();
        for (e, &(a, b)) in edges.iter().enumerate() {
            let (lo, hi) = (a.min(b), a.max(b));
            match pairs.iter_mut().find(|p| p.a == lo && p.b == hi) {
                Some(pair) => pair.edges.push(e),
                None => pairs.push(Pair { a: lo, b: hi, edges: vec![e] }),
            }
        }
        let mut g = Graph { half, edges: edges.to_vec(), labels, pairs, place: s.place, gap: s.gap, width: s.width, height: s.height, k: 0.0 };
        // Room enough for what's in it, spread out: grown in proportion
        // if the box asked for hasn't it.
        let gap = s.gap;
        let boxes: f64 = g.half.iter().map(|h| (2.0 * h.x + 2.0 * gap) * (2.0 * h.y + 2.0 * gap)).sum();
        let labels: f64 = g.labels.iter().flatten().map(|h| (2.0 * h.x + gap) * (2.0 * h.y + gap)).sum();
        let need = 2.5 * (boxes + labels);
        let longest = (0..g.pairs.len()).map(|p| g.rest_length(p, V::new(1.0, 0.0)).max(g.rest_length(p, V::new(0.0, 1.0)))).fold(0.0, f64::max);
        let widest = g.half.iter().map(|h| 2.0 * h.x.max(h.y)).fold(0.0, f64::max);
        let grow = (need / (s.width * s.height)).sqrt().max((longest + widest) / s.width.min(s.height)).max(1.0);
        (g.width, g.height) = (s.width * grow, s.height * grow);
        g.k = (g.width * g.height / g.half.len().max(1) as f64).sqrt();
        g
    }

    /// The label's half size as it's set on a line going `u`, along and
    /// across it.
    fn label_reach(&self, e: usize, u: V) -> V {
        match (self.labels[e], self.place) {
            (None, _) => V::new(0.0, 0.0),
            (Some(h), LabelPlace::Along | LabelPlace::Beside) => h,
            (Some(h), LabelPlace::Upright) => {
                let r = Rect::upright(V::new(0.0, 0.0), h);
                V::new(r.reach(u), r.reach(u.perp()))
            }
        }
    }

    /// How long the free part of an edge's line must be going `u`: its
    /// label's length, or a stub.
    fn needs(&self, e: usize, u: V) -> f64 {
        let stub = 4.0 * self.gap;
        match self.labels[e] {
            None => stub,
            Some(_) => (2.0 * self.label_reach(e, u).x + 2.0 * self.gap).max(stub),
        }
    }

    /// How far apart a pair's middles should be, going `u`: room for its
    /// longest label, or for all of them one after another.
    fn rest_length(&self, p: usize, u: V) -> f64 {
        let pair = &self.pairs[p];
        let room = if self.spread(p, u).1 {
            let labels: f64 = pair.edges.iter().filter(|&&e| self.labels[e].is_some()).map(|&e| 2.0 * self.label_reach(e, u).x + self.gap).sum();
            (labels + self.gap).max(4.0 * self.gap)
        } else {
            pair.edges.iter().map(|&e| self.needs(e, u)).fold(0.0, f64::max)
        };
        border(self.half[pair.a], u) + border(self.half[pair.b], u) + 2.0 * self.gap + room
    }

    /// How a pair's edges are set out going `u`, from its first node to its
    /// second: how far across each edge's line is offset, and whether their
    /// labels go one after another along the lines, as they do when
    /// there's no room in the boxes for them side by side. Two lines with
    /// their labels beside them are close together, each label on its
    /// line's outer side.
    fn spread(&self, p: usize, u: V) -> (Vec<f64>, bool) {
        let pair = &self.pairs[p];
        let k = pair.edges.len();
        if k == 1 {
            return (vec![0.0], false);
        }
        let n = u.perp();
        let reach = |node: usize| Rect::upright(V::default(), self.half[node]).reach(n);
        let room = 0.8 * reach(pair.a).min(reach(pair.b));
        if k == 2 && self.place == LabelPlace::Beside {
            let half = (1.5 * self.gap).min(room);
            return (vec![-half, half], false);
        }
        let thickness: Vec<f64> = pair.edges.iter().map(|&e| {
            let across = 2.0 * self.label_reach(e, u).y;
            match self.place {
                LabelPlace::Beside => across + self.gap,
                _ => across.max(self.gap),
            }
        }).collect();
        let total: f64 = thickness.iter().sum::<f64>() + self.gap * (k as f64 - 1.0);
        // The lines stay inside both boxes.
        if total <= 2.0 * room {
            let mut at = -total / 2.0;
            let offsets = thickness.iter().map(|t| {
                let mid = at + t / 2.0;
                at += t + self.gap;
                mid
            }).collect();
            return (offsets, false);
        }
        let step = (3.0 * self.gap).min(2.0 * room / (k as f64 - 1.0));
        ((0..k).map(|i| (i as f64 - (k as f64 - 1.0) / 2.0) * step).collect(), true)
    }

    /// Each node somewhere in the box at random.
    fn start(&self, rng: &mut StdRng) -> Vec<V> {
        self.half.iter().map(|h| {
            let mut span = |side: f64, half: f64| if side > 2.0 * half { rng.random_range(half..side - half) } else { side / 2.0 };
            let x = span(self.width, h.x);
            V::new(x, span(self.height, h.y))
        }).collect()
    }

    /// Each pair's edges' lines where the nodes are now.
    fn lines(&self, pos: &[V]) -> Vec<Line> {
        let mut lines: Vec<Line> = Vec::with_capacity(self.edges.len());
        for (p, pair) in self.pairs.iter().enumerate() {
            // Pairs are set out lower index to higher, so edges both ways
            // are offset the same way round.
            let pa = pos[pair.a];
            let u = unit(pos[pair.b] - pa);
            let (offsets, one_after_another) = self.spread(p, u);
            let mut made: Vec<Line> = pair.edges.iter().zip(&offsets).map(|(&e, &offset)| {
                let (a, b) = self.edges[e];
                let (from, to) = (pos[a] + u.perp() * offset, pos[b] + u.perp() * offset);
                let eu = unit(to - from);
                let length = norm(to - from);
                // Lines stop a gap short of the boxes, every way.
                let grown = |node: usize| self.half[node] + V::new(self.gap, self.gap);
                let leave = crossing(from, eu, pos[a], grown(a), false).unwrap_or(0.0).max(0.0);
                let enter = crossing(from, eu, pos[b], grown(b), true).unwrap_or(length).min(length);
                Line { edge: e, start: from, u: eu, free: (leave, enter), label: None }
            }).collect();
            // Where each label's middle is, along its line: halfway, or
            // one after another down the middle of the stretch every line
            // of the pair is free on.
            let middles: Vec<f64> = if one_after_another {
                // Along the pair, from its first node: where a line starts,
                // and which way it goes.
                let frame = |l: &Line| ((l.start - pa).dot(u), if l.u.dot(u) >= 0.0 { 1.0 } else { -1.0 });
                let (lo, hi) = made.iter().fold((f64::MIN, f64::MAX), |(lo, hi), l| {
                    let (s0, way) = frame(l);
                    let (x, y) = (s0 + way * l.free.0, s0 + way * l.free.1);
                    (lo.max(x.min(y)), hi.min(x.max(y)))
                });
                let lengths: Vec<f64> = made.iter().map(|l| if self.labels[l.edge].is_some() { 2.0 * self.label_reach(l.edge, u).x } else { 0.0 }).collect();
                let count = lengths.iter().filter(|&&l| l > 0.0).count() as f64;
                let mut at = (lo + hi) / 2.0 - (lengths.iter().sum::<f64>() + self.gap * (count - 1.0).max(0.0)) / 2.0;
                made.iter().zip(&lengths).map(|(l, &length)| {
                    let (s0, way) = frame(l);
                    let middle = at + length / 2.0;
                    if length > 0.0 {
                        at += length + self.gap;
                    }
                    (middle - s0) * way
                }).collect()
            } else {
                made.iter().map(|l| (l.free.0 + l.free.1) / 2.0).collect()
            };
            // Labels beside lines one after another go beside the
            // outermost, clear of the rest; beside two lines, each goes on
            // its own line's outer side.
            let up_is_n = up(reading(u)).dot(u.perp()) >= 0.0;
            let outer = offsets.iter().copied().fold(if up_is_n { f64::MIN } else { f64::MAX }, |o, x| if up_is_n { o.max(x) } else { o.min(x) });
            let two = offsets.len() == 2 && self.place == LabelPlace::Beside;
            for ((line, middle), &offset) in made.iter_mut().zip(middles).zip(&offsets) {
                let base = if one_after_another && self.place == LabelPlace::Beside { line.start + u.perp() * (outer - offset) } else { line.start };
                let above = !two || (offset >= 0.0) == up_is_n;
                line.label = self.labels[line.edge].map(|h| (self.label_box(base, line.u, middle, h, above), middle));
            }
            lines.append(&mut made);
        }
        lines.sort_by_key(|l| l.edge);
        lines
    }

    /// Move the layout to the middle of its box, growing the box, in
    /// proportion, if it's too big for it: its size.
    fn fit(&self, pos: &mut [V]) -> (f64, f64) {
        let (mut lo, mut hi) = (V::new(f64::MAX, f64::MAX), V::new(f64::MIN, f64::MIN));
        let mut take = |c: V, reach: V| {
            lo = V::new(lo.x.min(c.x - reach.x), lo.y.min(c.y - reach.y));
            hi = V::new(hi.x.max(c.x + reach.x), hi.y.max(c.y + reach.y));
        };
        for (&c, &half) in pos.iter().zip(&self.half) {
            take(c, half);
        }
        for line in self.lines(pos) {
            if let Some((r, _)) = line.label {
                take(r.c, V::new(r.reach(V::new(1.0, 0.0)), r.reach(V::new(0.0, 1.0))));
            }
        }
        let (w, h) = (hi.x - lo.x, hi.y - lo.y);
        let grow = (w / self.width).max(h / self.height).max(1.0);
        let size = (self.width * grow, self.height * grow);
        let by = V::new((size.0 - w) / 2.0 - lo.x, (size.1 - h) / 2.0 - lo.y);
        for p in pos.iter_mut() {
            *p = *p + by;
        }
        size
    }

    /// The label's box, its middle `along` the line from `from` going `u`;
    /// beside it, `above` it or below.
    fn label_box(&self, from: V, u: V, along: f64, half: V, above: bool) -> Rect {
        let on = from + u * along;
        match self.place {
            LabelPlace::Along => Rect { c: on, u: reading(u), half },
            LabelPlace::Beside => {
                let r = reading(u);
                let side = if above { 1.0 } else { -1.0 };
                Rect { c: on + up(r) * (side * (half.y + self.gap / 2.0)), u: r, half }
            }
            LabelPlace::Upright => Rect::upright(on, half),
        }
    }

    /// How far each node would move: pushed and pulled.
    fn forces(&self, pos: &[V]) -> Vec<V> {
        let n = pos.len();
        let gap = self.gap;
        let mut f = vec![V::default(); n];
        let boxes: Vec<Rect> = pos.iter().zip(&self.half).map(|(&c, &h)| Rect::upright(c, h)).collect();
        // Every node spread from the others a little, and boxes that
        // overlap pushed apart.
        for i in 0..n {
            for j in i + 1..n {
                let d = pos[j] - pos[i];
                let dist = norm(d).max(0.01 * self.k);
                let spread = unit(d) * (0.05 * self.k * self.k / dist).min(0.05 * self.k);
                f[i] = f[i] - spread;
                f[j] = f[j] + spread;
                if let Some((way, depth)) = boxes[i].overlap(&boxes[j], 2.0 * gap) {
                    f[i] = f[i] - way * (0.5 * depth);
                    f[j] = f[j] + way * (0.5 * depth);
                }
            }
        }
        // Pairs to their lengths.
        for (p, pair) in self.pairs.iter().enumerate() {
            let d = pos[pair.b] - pos[pair.a];
            let u = unit(d);
            let pull = (norm(d) - self.rest_length(p, u)) * 0.5;
            f[pair.a] = f[pair.a] + u * (0.5 * pull);
            f[pair.b] = f[pair.b] - u * (0.5 * pull);
        }
        let lines = self.lines(pos);
        for line in &lines {
            let (a, b) = self.edges[line.edge];
            let (from, to) = (line.start + line.u * line.free.0, line.start + line.u * line.free.1);
            for k in 0..n {
                if k == a || k == b {
                    continue;
                }
                // Lines push away the boxes they pass too near, and the
                // boxes push back.
                let ab = to - from;
                let t = ((pos[k] - from).dot(ab) / ab.dot(ab).max(1e-12)).clamp(0.0, 1.0);
                let near = from + ab * t;
                let off = pos[k] - near;
                let away = if norm(off) > 1e-9 { unit(off) } else { line.u.perp() };
                let want = boxes[k].reach(away) + gap;
                let dist = off.dot(away);
                if dist < want {
                    let push = 0.5 * (want - dist);
                    f[k] = f[k] + away * push;
                    f[a] = f[a] - away * (push * (1.0 - t));
                    f[b] = f[b] - away * (push * t);
                }
                // Labels push away boxes that aren't theirs.
                if let Some((label, _)) = &line.label {
                    if let Some((way, depth)) = label.overlap(&boxes[k], gap) {
                        f[k] = f[k] + way * (0.5 * depth);
                        f[a] = f[a] - way * (0.25 * depth);
                        f[b] = f[b] - way * (0.25 * depth);
                    }
                }
            }
        }
        // Labels push each other apart.
        for (x, lx) in lines.iter().enumerate() {
            for ly in &lines[x + 1..] {
                let (Some((rx, _)), Some((ry, _))) = (&lx.label, &ly.label) else { continue };
                let ((xa, xb), (ya, yb)) = (self.edges[lx.edge], self.edges[ly.edge]);
                if (xa.min(xb), xa.max(xb)) == (ya.min(yb), ya.max(yb)) {
                    // A pair's own are offset already.
                    continue;
                }
                if let Some((way, depth)) = rx.overlap(ry, gap) {
                    for (node, s) in [(xa, -0.25), (xb, -0.25), (ya, 0.25), (yb, 0.25)] {
                        f[node] = f[node] + way * (s * depth);
                    }
                }
            }
        }
        // A weak pull to the middle, and a push in from the box's edges.
        let middle = V::new(self.width / 2.0, self.height / 2.0);
        for i in 0..n {
            f[i] = f[i] + (middle - pos[i]) * 0.01;
            let h = self.half[i];
            let inward = |p: f64, half: f64, side: f64| {
                if side <= 2.0 * half { side / 2.0 - p } else { (half - p).max(0.0) - (p + half - side).max(0.0) }
            };
            f[i] = f[i] + V::new(inward(pos[i].x, h.x, self.width), inward(pos[i].y, h.y, self.height));
        }
        f
    }

    /// Move the nodes by the forces, `iterations` times, cooling as they go.
    fn relax(&self, pos: &mut [V], iterations: usize, rng: &mut StdRng) {
        let size = self.width.max(self.height);
        for step in 0..iterations {
            let cool = 1.0 - step as f64 / iterations as f64;
            let most = size * (0.1 * cool * cool + 0.002);
            let moves = self.forces(pos);
            for (p, m) in pos.iter_mut().zip(moves) {
                let length = norm(m);
                let m = if length > most { m * (most / length) } else { m };
                // A nudge, cooling with the rest, so nodes pushed alike
                // don't settle alike.
                let nudge = V::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0)) * (0.02 * most * cool);
                *p = *p + m + nudge;
            }
        }
    }

    /// Push apart what still overlaps, without the pulls: boxes, labels
    /// over boxes, and labels over labels.
    fn tidy(&self, pos: &mut [V]) {
        let n = pos.len();
        for _ in 0..100 {
            let boxes: Vec<Rect> = pos.iter().zip(&self.half).map(|(&c, &h)| Rect::upright(c, h)).collect();
            let mut f = vec![V::default(); n];
            let mut moved = false;
            for i in 0..n {
                for j in i + 1..n {
                    if let Some((way, depth)) = boxes[i].overlap(&boxes[j], 2.0 * self.gap) {
                        f[i] = f[i] - way * (0.5 * depth + 1e-6);
                        f[j] = f[j] + way * (0.5 * depth + 1e-6);
                        moved = true;
                    }
                }
            }
            let lines = self.lines(pos);
            for line in &lines {
                let Some((label, _)) = &line.label else { continue };
                let (a, b) = self.edges[line.edge];
                for k in (0..n).filter(|&k| k != a && k != b) {
                    if let Some((way, depth)) = label.overlap(&boxes[k], self.gap) {
                        f[k] = f[k] + way * (depth + 1e-6);
                        moved = true;
                    }
                }
            }
            // Labels apart, each moved with its edge's nodes.
            for (x, lx) in lines.iter().enumerate() {
                for ly in &lines[x + 1..] {
                    let (Some((rx, _)), Some((ry, _))) = (&lx.label, &ly.label) else { continue };
                    let ((xa, xb), (ya, yb)) = (self.edges[lx.edge], self.edges[ly.edge]);
                    if (xa.min(xb), xa.max(xb)) == (ya.min(yb), ya.max(yb)) {
                        continue;
                    }
                    if let Some((way, depth)) = rx.overlap(ry, self.gap) {
                        for (node, s) in [(xa, -1.0), (xb, -1.0), (ya, 1.0), (yb, 1.0)] {
                            f[node] = f[node] + way * (s * (0.5 * depth + 1e-6));
                        }
                        moved = true;
                    }
                }
            }
            if !moved {
                break;
            }
            for (p, m) in pos.iter_mut().zip(f) {
                *p = *p + m;
            }
        }
    }

    /// How bad a layout is: what overlaps first, then lines that cross,
    /// then pairs far from their lengths.
    fn score(&self, pos: &[V]) -> f64 {
        let n = pos.len();
        let boxes: Vec<Rect> = pos.iter().zip(&self.half).map(|(&c, &h)| Rect::upright(c, h)).collect();
        let mut overlap = 0.0;
        for i in 0..n {
            for j in i + 1..n {
                overlap += boxes[i].overlap(&boxes[j], 0.0).map_or(0.0, |(_, d)| d);
            }
        }
        let lines = self.lines(pos);
        let ends = |l: &Line| (l.start + l.u * l.free.0, l.start + l.u * l.free.1);
        let mut crossings = 0.0;
        for (x, lx) in lines.iter().enumerate() {
            let (xa, xb) = self.edges[lx.edge];
            if let Some((label, _)) = &lx.label {
                for (k, b) in boxes.iter().enumerate() {
                    if k != xa && k != xb {
                        overlap += label.overlap(b, 0.0).map_or(0.0, |(_, d)| d);
                    }
                }
            }
            for ly in &lines[x + 1..] {
                let (ya, yb) = self.edges[ly.edge];
                if [ya, yb].contains(&xa) || [ya, yb].contains(&xb) {
                    continue;
                }
                let ((p, q), (r, s)) = (ends(lx), ends(ly));
                if segments_meet(p, q, r, s) {
                    crossings += 1.0;
                }
                if let (Some((rx, _)), Some((ry, _))) = (&lx.label, &ly.label) {
                    overlap += rx.overlap(ry, 0.0).map_or(0.0, |(_, d)| d);
                }
            }
        }
        let stretch: f64 = self.pairs.iter().enumerate().map(|(p, pair)| {
            let d = pos[pair.b] - pos[pair.a];
            (norm(d) - self.rest_length(p, unit(d))).abs()
        }).sum();
        overlap * 1e6 + crossings * 1e3 + stretch / self.k.max(1.0)
    }

    /// An edge's line as it's drawn, broken wherever a label lies over
    /// it: its own, unless that's beside it, and any other.
    fn edge_layout(&self, line: &Line, lines: &[Line]) -> EdgeLayout {
        let (leave, enter) = line.free;
        let at = |t: f64| line.start + line.u * t;
        let mut under: Vec<(f64, f64)> = lines.iter()
            .filter(|l| !(l.edge == line.edge && self.place == LabelPlace::Beside))
            .filter_map(|l| l.label.as_ref())
            .filter_map(|(rect, _)| rect.across(line.start, line.u, self.gap / 2.0))
            .collect();
        under.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Stubs between labels are left out.
        let mut pieces = Vec::new();
        let stub = self.gap;
        let mut piece = |a: f64, b: f64| if b - a > stub || (b - a > 1e-6 && (a == leave || b == enter)) { pieces.push((vec2(at(a)), vec2(at(b)))) };
        let mut t = leave;
        for (a, b) in under {
            piece(t, a.min(enter));
            t = t.max(b);
        }
        piece(t, enter);
        let (label, angle) = match &line.label {
            None => ((at(leave) + at(enter)) * 0.5, angle_of(reading(line.u))),
            Some((rect, _)) => (rect.c, if self.place == LabelPlace::Upright { 0.0 } else { angle_of(rect.u) }),
        };
        EdgeLayout { start: vec2(at(leave)), end: vec2(at(enter)), label: vec2(label), angle, pieces }
    }
}

/// The way text along a line going `u` reads: left to right, or, on an
/// upright line, bottom to top.
fn reading(u: V) -> V {
    if u.x < -1e-12 || (u.x.abs() <= 1e-12 && u.y > 0.0) { u * -1.0 } else { u }
}

/// Up from text reading `r`, y being down.
fn up(r: V) -> V {
    V::new(r.y, -r.x)
}

/// The angle of a reading direction, from -pi/2 (up the page) to just
/// under pi/2.
fn angle_of(r: V) -> f64 {
    let a = r.y.atan2(r.x);
    if a >= FRAC_PI_2 { a - PI } else if a < -FRAC_PI_2 { a + PI } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(seed: u64, place: LabelPlace) -> GraphLayoutSettings {
        GraphLayoutSettings { width: 1000.0, height: 800.0, gap: 10.0, seed, iterations: 300, place }
    }

    /// Paper Mazes, Saint's Rest and the Cloister, and two more, with
    /// labels on most edges and two edges each way between one pair.
    fn places() -> (Vec<(f64, f64)>, Vec<(usize, usize)>, Vec<Option<(f64, f64)>>) {
        let sizes = vec![(180.0, 70.0), (160.0, 70.0), (150.0, 60.0), (120.0, 50.0), (200.0, 80.0)];
        let edges = vec![(0, 1), (1, 0), (1, 2), (2, 3), (3, 4), (4, 0), (2, 4)];
        let labels = vec![Some((300.0, 60.0)), Some((200.0, 30.0)), None, Some((150.0, 30.0)), Some((220.0, 60.0)), None, Some((100.0, 30.0))];
        (sizes, edges, labels)
    }

    fn lay_out(seed: u64, place: LabelPlace) -> GraphLayout {
        let (sizes, edges, labels) = places();
        lay_out_graph(&sizes, &edges, &labels, &settings(seed, place)).unwrap()
    }

    fn rect_of(p: Vec2, (w, h): (f64, f64)) -> Rect {
        Rect::upright(V::new(p.x as f64, p.y as f64), V::new(w / 2.0, h / 2.0))
    }

    #[test]
    fn a_seed_lays_out_the_same_every_time_and_seeds_differ() {
        let points = |l: &GraphLayout| l.points.iter().map(|p| (p.x.to_bits(), p.y.to_bits())).collect::<Vec<_>>();
        assert_eq!(points(&lay_out(3, LabelPlace::Along)), points(&lay_out(3, LabelPlace::Along)));
        assert_ne!(points(&lay_out(3, LabelPlace::Along)), points(&lay_out(4, LabelPlace::Along)));
    }

    #[test]
    fn boxes_and_labels_keep_apart_inside_the_box() {
        let (sizes, edges, labels) = places();
        for place in [LabelPlace::Along, LabelPlace::Beside, LabelPlace::Upright] {
            for seed in 0..4 {
                let l = lay_out(seed, place);
                let boxes: Vec<Rect> = l.points.iter().zip(&sizes).map(|(&p, &s)| rect_of(p, s)).collect();
                for i in 0..boxes.len() {
                    for j in i + 1..boxes.len() {
                        assert!(boxes[i].overlap(&boxes[j], 0.0).is_none(), "{:?} seed {}: boxes {} and {} overlap", place, seed, i, j);
                    }
                    let (c, h) = (boxes[i].c, boxes[i].half);
                    assert!(c.x - h.x >= -1.0 && c.y - h.y >= -1.0 && c.x + h.x <= l.size.0 + 1.0 && c.y + h.y <= l.size.1 + 1.0, "{:?} seed {}: box {} outside {:?}", place, seed, i, l.size);
                }
                for (e, edge) in l.edges.iter().enumerate() {
                    let Some((w, h)) = labels[e] else { continue };
                    let label = label_rect(edge, (w, h), place);
                    let reach = V::new(label.reach(V::new(1.0, 0.0)), label.reach(V::new(0.0, 1.0)));
                    assert!(label.c.x - reach.x >= -1.0 && label.c.y - reach.y >= -1.0 && label.c.x + reach.x <= l.size.0 + 1.0 && label.c.y + reach.y <= l.size.1 + 1.0, "{:?} seed {}: label {} outside {:?}", place, seed, e, l.size);
                    for (k, b) in boxes.iter().enumerate() {
                        if k != edges[e].0 && k != edges[e].1 {
                            assert!(label.overlap(b, 0.0).is_none(), "{:?} seed {}: label {} over box {}", place, seed, e, k);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn labels_read_left_to_right_and_lines_stop_at_borders() {
        let (sizes, edges, _) = places();
        for place in [LabelPlace::Along, LabelPlace::Beside] {
            let l = lay_out(1, place);
            for (e, edge) in l.edges.iter().enumerate() {
                assert!((-FRAC_PI_2..FRAC_PI_2).contains(&edge.angle), "{:?}: {}", place, edge.angle);
                // Each end is a gap from its node's box.
                for (end, node) in [(edge.start, edges[e].0), (edge.end, edges[e].1)] {
                    let r = rect_of(l.points[node], sizes[node]);
                    let d = V::new((end.x as f64 - r.c.x).abs() - r.half.x, (end.y as f64 - r.c.y).abs() - r.half.y);
                    assert!((d.x.max(d.y) - 10.0).abs() < 0.01, "{:?} edge {}: end {:?} from box {:?}", place, e, end, r);
                }
            }
        }
    }

    fn label_rect(edge: &EdgeLayout, (w, h): (f64, f64), place: LabelPlace) -> Rect {
        let u = if place == LabelPlace::Upright { V::new(1.0, 0.0) } else { V::angle(edge.angle) };
        Rect { c: V::new(edge.label.x as f64, edge.label.y as f64), u, half: V::new(w / 2.0, h / 2.0) }
    }

    #[test]
    fn edges_both_ways_between_two_nodes_are_drawn_apart() {
        let (_, _, labels) = places();
        for (place, seed) in [LabelPlace::Along, LabelPlace::Beside, LabelPlace::Upright].into_iter().flat_map(|p| (0..4).map(move |s| (p, s))) {
            let l = lay_out(seed, place);
            let (a, b) = (label_rect(&l.edges[0], labels[0].unwrap(), place), label_rect(&l.edges[1], labels[1].unwrap(), place));
            assert!(a.overlap(&b, 0.0).is_none(), "{:?} seed {}: the pair's labels overlap", place, seed);
            let (there, back) = (&l.edges[0], &l.edges[1]);
            let mid = |e: &EdgeLayout| (e.start + e.end) * 0.5;
            assert!((mid(there) - mid(back)).length() > 15.0, "{:?}: {:?} and {:?}", place, mid(there), mid(back));
            assert!((there.label - back.label).length() > 15.0, "{:?}: labels {:?} and {:?}", place, there.label, back.label);
        }
    }

    #[test]
    fn along_and_upright_lines_break_for_their_labels() {
        let along = lay_out(1, LabelPlace::Along);
        assert!(along.edges[0].pieces.len() >= 2);
        let beside = lay_out(1, LabelPlace::Beside);
        assert_eq!(beside.edges[5].pieces.len(), 1, "no label, no break: {:?}", beside.edges[5].pieces);
        assert_eq!(beside.edges[3].pieces.len(), 1, "beside a line, it's unbroken: {:?}", beside.edges[3].pieces);
        let upright = lay_out(1, LabelPlace::Upright);
        assert_eq!(upright.edges[0].angle, 0.0);
        assert!(upright.edges[0].pieces.len() >= 2);
    }

    #[test]
    fn no_line_runs_under_a_label() {
        let (_, _, labels) = places();
        for (place, seed) in [LabelPlace::Along, LabelPlace::Beside, LabelPlace::Upright].into_iter().flat_map(|p| (0..4).map(move |s| (p, s))) {
            let l = lay_out(seed, place);
            for (e, edge) in l.edges.iter().enumerate() {
                for &(a, b) in &edge.pieces {
                    let (a, b) = (V::new(a.x as f64, a.y as f64), V::new(b.x as f64, b.y as f64));
                    for (k, other) in l.edges.iter().enumerate() {
                        let Some(size) = labels[k] else { continue };
                        if k == e && place == LabelPlace::Beside {
                            continue;
                        }
                        let over = label_rect(other, size, place).across(a, unit(b - a), -0.5);
                        assert!(over.is_none_or(|(t0, t1)| t1 < 0.0 || t0 > norm(b - a)), "{:?} seed {}: edge {} runs under label {}", place, seed, e, k);
                    }
                }
            }
        }
    }

    #[test]
    fn a_crowded_box_grows() {
        let (sizes, edges, labels) = places();
        let small = GraphLayoutSettings { width: 200.0, height: 100.0, ..settings(0, LabelPlace::Along) };
        let l = lay_out_graph(&sizes, &edges, &labels, &small).unwrap();
        assert!(l.size.0 > 200.0 && (l.size.0 / l.size.1 - 2.0).abs() < 1e-9, "{:?}", l.size);
    }

    #[test]
    fn bad_graphs_and_settings_are_errors() {
        let ok = settings(0, LabelPlace::Along);
        let sizes = [(10.0, 10.0), (10.0, 10.0)];
        for (sizes, edges, labels, s, want) in [
            (&sizes[..], &[(0, 2)][..], &[None][..], &ok, "edge 0 joins 0 and 2, but there are 2 nodes"),
            (&sizes[..], &[(1, 1)][..], &[None][..], &ok, "edge 0 joins node 1 to itself"),
            (&sizes[..], &[(0, 1)][..], &[][..], &ok, "there are 0 labels for 1 edges"),
            (&[(-1.0, 2.0)][..], &[][..], &[][..], &ok, "sizes[0] should be a width and height of at least 0"),
        ] {
            let err = lay_out_graph(sizes, edges, labels, s).err().unwrap_or_default();
            assert!(err.contains(want), "{}: {}", want, err);
        }
        let many = GraphLayoutSettings { iterations: 6000, ..settings(0, LabelPlace::Along) };
        assert!(lay_out_graph(&sizes, &[], &[], &many).unwrap_err().contains("at most 5000"));
        assert!(LabelPlace::named("across").unwrap_err().contains("\"along\", \"beside\" or \"upright\""));
    }
}
