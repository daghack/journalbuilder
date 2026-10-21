//! A star chart whose constellation is a Huffman tree: the code an M-ary
//! Huffman tree gives the symbols of a text in base N, built as the
//! `huffman` lock builds it (so the chart is that lock's code), laid out as
//! a tree, then relaxed by forces into a scatter that looks random but
//! whose links never cross.
//!
//! The tree is grown out from its root: each star's children, biggest
//! subtree first, go at a random angle and a varied distance from it,
//! wherever their link crosses none already drawn and their subtree has
//! most room. Then forces tidy it, each star with a personal space of its
//! own (stars nearer than their spaces push apart) and each link a length
//! of its own (links longer than theirs pull together), and links push
//! away stars that aren't theirs (as in PrEd). Only crowding and overlong
//! links are pushed and pulled, so the growth's randomness stays: forces
//! that pull every link to one length and push every star from every
//! other, as Fruchterman and Reingold's do, settle any tree into the same
//! even, botanical spray. A move is kept only if none of the moved star's
//! links then crosses another, and the star's clearance (how near links
//! come to stars, and how short its links are) doesn't fall, or stays
//! above what the stars need.

use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use parry2d::math::Vec2;

use super::geom::V;
use crate::journal::paths::StrokePath;

/// Where a chart goes and how it's drawn, besides its symbols and bases.
pub struct StarChartSettings {
    /// The middle of the chart's box.
    pub center: Vec2,
    pub width: f64,
    pub height: f64,
    /// Stars kept in the ellipse inscribed in the box, not the box.
    pub round: bool,
    /// How many times every symbol of the base counts besides its uses.
    pub smoothing: u32,
    /// How much stars' spaces and pulls, links' lengths and the growth's
    /// reaches differ, 0 to 1.
    pub variation: f64,
    /// How many steps the forces tidy the grown tree by.
    pub iterations: usize,
    /// The biggest star's radius; by default from the chart's size.
    pub star_size: Option<f64>,
    /// How far short of stars links stop; by default a quarter of
    /// `star_size`.
    pub gap: Option<f64>,
    pub seed: u64,
}

/// A star: a node of the Huffman tree.
pub struct Star {
    pub position: Vec2,
    pub radius: f64,
    /// Its symbol's count (smoothing included), or its leaves' together.
    pub weight: u64,
    pub depth: usize,
    /// The digits from the root to it: a leaf's is its symbol's code.
    pub code: Vec<u32>,
    /// A leaf's symbol.
    pub symbol: Option<u32>,
    pub parent: Option<usize>,
}

pub struct StarChart {
    /// The root first, then breadth first, each star's children in the
    /// order of their digits.
    pub stars: Vec<Star>,
    /// [parent, child] pairs, in the order the stars are.
    pub edges: Vec<(usize, usize)>,
    /// A circle for every star.
    pub outlines: StrokePath,
    /// A line for every link, stopping `gap` short of its stars.
    pub links: StrokePath,
}

pub fn huffman_stars(symbols: &[u32], input_base: u32, output_base: u32, settings: &StarChartSettings) -> Result<StarChart, String> {
    let s = settings;
    if !(2..=27).contains(&output_base) {
        return Err(format!("output_base must be from 2 to 27, got {}", output_base));
    }
    check_settings(s)?;
    let mut stars = huffman_tree(symbols, input_base, output_base, s.smoothing)?;
    let n = stars.len();
    let room = Room::new(s, n)?;
    let (light, heavy) = stars.iter().fold((u64::MAX, 0), |(lo, hi), st| (lo.min(st.weight), hi.max(st.weight)));
    for star in &mut stars {
        // Bigger by weight, on a log scale as magnitudes are: the root,
        // which weighs everything, is the biggest, the rarest symbols a
        // third of its size.
        let t = if heavy > light { ((star.weight as f64).ln() - (light as f64).ln()) / ((heavy as f64).ln() - (light as f64).ln()) } else { 1.0 };
        star.radius = room.star_size * (1.0 + 2.0 * t) / 3.0;
    }
    let parents: Vec<Option<usize>> = stars.iter().map(|st| st.parent).collect();
    let (edges, children) = links_of(&parents);
    let radius: Vec<f64> = stars.iter().map(|st| st.radius).collect();
    let pos = place(&children, &edges, &radius, &room, s);

    for (star, &p) in stars.iter_mut().zip(&pos) {
        star.position = vec2(p);
    }
    let mut outlines = StrokePath::new();
    for (star, &p) in stars.iter().zip(&pos) {
        outlines.append(StrokePath::circle(vec2(p), star.radius as f32));
    }
    let mut links = StrokePath::new();
    for &(a, b) in &edges {
        let (pa, pb) = (pos[a], pos[b]);
        let along = pb - pa;
        let length = along.len();
        let (from, to) = (stars[a].radius + room.gap, length - stars[b].radius - room.gap);
        if to > from {
            let unit = along * (1.0 / length);
            links.append(StrokePath::line(vec2(pa + unit * from), vec2(pa + unit * to)));
        }
    }
    Ok(StarChart { stars, edges, outlines, links })
}

/// The M-ary Huffman tree of `symbols` as stars, root first and breadth
/// first, each star's children in the order of their digits, padding left
/// out: as huffman_stars charts it, not yet placed or sized.
pub fn huffman_tree(symbols: &[u32], input_base: u32, output_base: u32, smoothing: u32) -> Result<Vec<Star>, String> {
    if !(2..=27).contains(&output_base) {
        return Err(format!("output_base must be from 2 to 27, got {}", output_base));
    }
    let (nodes, root) = huffman(symbols, input_base, output_base, smoothing)?;
    Ok(flatten(&nodes, root).0)
}

/// A tree's stars placed as huffman_stars places them, the tree given by
/// each star's parent (the root first, every parent before its children)
/// and its stars sized by depth: the root biggest, the deepest a third its
/// size. Gives each star's position and radius.
pub fn lay_out_tree(parents: &[Option<usize>], settings: &StarChartSettings) -> Result<(Vec<Vec2>, Vec<f64>), String> {
    check_settings(settings)?;
    let n = parents.len();
    if n == 0 || parents[0].is_some() {
        return Err("the tree's first star must be its root".into());
    }
    let mut depth = vec![0usize; n];
    for (i, parent) in parents.iter().enumerate().skip(1) {
        match parent {
            Some(p) if *p < i => depth[i] = depth[*p] + 1,
            _ => return Err(format!("star {}'s parent must come before it", i)),
        }
    }
    let room = Room::new(settings, n)?;
    let deepest = depth.iter().copied().max().unwrap_or(0).max(1) as f64;
    let radius: Vec<f64> = depth.iter().map(|&d| room.star_size * (1.0 + 2.0 * (1.0 - d as f64 / deepest)) / 3.0).collect();
    let (edges, children) = links_of(parents);
    let pos = place(&children, &edges, &radius, &room, settings);
    Ok((pos.into_iter().map(vec2).collect(), radius))
}

/// The settings checked, apart from what needs the number of stars.
fn check_settings(s: &StarChartSettings) -> Result<(), String> {
    if !(s.width > 0.0 && s.width.is_finite() && s.height > 0.0 && s.height.is_finite()) {
        return Err(format!("width and height must be positive numbers, got {} and {}", s.width, s.height));
    }
    if !V::from(s.center).is_finite() {
        return Err("the centre must be finite".into());
    }
    if !(0.0..=1.0).contains(&s.variation) {
        return Err(format!("variation runs from 0 to 1, got {}", s.variation));
    }
    if s.iterations > 10_000 {
        return Err(format!("iterations must be at most 10000, got {}", s.iterations));
    }
    if let Some(size) = s.star_size {
        if !(size > 0.0 && size.is_finite()) {
            return Err(format!("star_size must be a positive number, got {}", size));
        }
    }
    if let Some(gap) = s.gap {
        if !(gap >= 0.0 && gap.is_finite()) {
            return Err(format!("gap must be a number of at least 0, got {}", gap));
        }
    }
    Ok(())
}

/// The chart's box and how much room `n` stars have in it.
struct Room {
    bounds: Bounds,
    /// How far apart stars spread evenly through the chart would be.
    k: f64,
    star_size: f64,
    gap: f64,
}

impl Room {
    fn new(s: &StarChartSettings, n: usize) -> Result<Room, String> {
        let bounds = Bounds { center: V::from(s.center), a: s.width / 2.0, b: s.height / 2.0, round: s.round };
        let area = if s.round { PI * bounds.a * bounds.b } else { s.width * s.height };
        let k = (area / n as f64).sqrt();
        let star_size = s.star_size.unwrap_or(k / 7.0);
        let gap = s.gap.unwrap_or(star_size / 4.0);
        if star_size >= bounds.a.min(bounds.b) {
            return Err(format!("the chart, {} by {}, is too small for stars {} across", s.width, s.height, star_size * 2.0));
        }
        Ok(Room { bounds, k, star_size, gap })
    }
}

/// Each star's [parent, child] links, in the stars' order, and children.
fn links_of(parents: &[Option<usize>]) -> (Vec<(usize, usize)>, Vec<Vec<usize>>) {
    let mut edges = Vec::new();
    let mut children = vec![Vec::new(); parents.len()];
    for (i, parent) in parents.iter().enumerate() {
        if let Some(p) = *parent {
            edges.push((p, i));
            children[p].push(i);
        }
    }
    (edges, children)
}

/// The tree grown, then tidied by forces.
fn place(children: &[Vec<usize>], edges: &[(usize, usize)], radius: &[f64], room: &Room, s: &StarChartSettings) -> Vec<V> {
    let n = children.len();
    let Room { k, star_size, gap, .. } = *room;
    let bounds = room.bounds;
    let mut rng = StdRng::seed_from_u64(s.seed);
    let start = grown(children, &bounds, star_size, k, s.variation, &mut rng);
    let mut layout = Layout {
        edges,
        incident: incident(n, edges),
        pos: start,
        room: radius.iter().map(|r| r + gap + 0.12 * k).collect(),
        shortest: edges.iter().map(|&(a, b)| radius[a] + radius[b] + 2.0 * gap + 0.25 * k).collect(),
        radius: radius.to_vec(),
        bounds,
    };
    debug_assert!(layout.planar(), "the grown tree should have no crossings");
    // Each star has a space and a pull of its own, bigger stars more
    // space, and each link a length of its own.
    let mut uneven = |most: f64| (s.variation * most.ln() * rng.random_range(-1.0..=1.0)).exp();
    let space: Vec<f64> = layout.radius.iter().map(|r| 0.35 * k * uneven(4.0) * (0.7 + 0.6 * r / star_size)).collect();
    let pull: Vec<f64> = (0..n).map(|_| uneven(2.0)).collect();
    let length: Vec<f64> = edges.iter().map(|_| k * uneven(3.0)).collect();
    layout.relax(&Forces { k, space, pull, length }, s.iterations, s.variation, &mut rng);
    debug_assert!(layout.planar(), "relaxing shouldn't make links cross");
    layout.pos
}

/// A node of the Huffman tree as it's built.
struct Node {
    weight: u64,
    symbol: Option<u32>,
    children: Vec<usize>,
    /// A weightless leaf added so every merge takes M nodes: it takes a
    /// digit, but isn't a star.
    padding: bool,
}

/// The M-ary Huffman tree of `symbols`, as the huffman lock builds it:
/// leaves made in symbol order, padded with weightless leaves until every
/// merge is full, the M lightest merged at a time, ties going to the node
/// made first. Gives the nodes and the root's index.
fn huffman(symbols: &[u32], input_base: u32, output_base: u32, smoothing: u32) -> Result<(Vec<Node>, usize), String> {
    let mut counts = vec![smoothing as u64; input_base as usize];
    for &s in symbols {
        *counts.get_mut(s as usize).ok_or_else(|| format!("symbol {} isn't in base {}", s, input_base))? += 1;
    }
    let mut nodes = Vec::new();
    let mut queue: Vec<(u64, usize)> = Vec::new();
    for (symbol, &weight) in counts.iter().enumerate() {
        if weight > 0 {
            queue.push((weight, nodes.len()));
            nodes.push(Node { weight, symbol: Some(symbol as u32), children: Vec::new(), padding: false });
        }
    }
    if queue.is_empty() {
        return Err("there are no symbols and smoothing is 0, so there's nothing to build a code from".into());
    }
    let m = output_base as usize;
    while queue.len() < 2 || (queue.len() - 1) % (m - 1) != 0 {
        queue.push((0, nodes.len()));
        nodes.push(Node { weight: 0, symbol: None, children: Vec::new(), padding: true });
    }
    while queue.len() > 1 {
        queue.sort();
        let merged: Vec<(u64, usize)> = queue.drain(..m).collect();
        let weight = merged.iter().map(|&(w, _)| w).sum();
        queue.push((weight, nodes.len()));
        nodes.push(Node { weight, symbol: None, children: merged.iter().map(|&(_, id)| id).collect(), padding: false });
    }
    Ok((nodes, queue[0].1))
}

/// The tree's stars, root first and breadth first, padding left out; its
/// [parent, child] edges; and each star's children.
fn flatten(nodes: &[Node], root: usize) -> (Vec<Star>, Vec<(usize, usize)>, Vec<Vec<usize>>) {
    let star = |node: &Node, depth, code, parent| Star { position: Vec2::ZERO, radius: 0.0, weight: node.weight, depth, code, symbol: node.symbol, parent };
    let mut stars = vec![star(&nodes[root], 0, Vec::new(), None)];
    let mut edges = Vec::new();
    let mut children = vec![Vec::new()];
    let mut queue = VecDeque::from([(root, 0)]);
    while let Some((node, i)) = queue.pop_front() {
        for (digit, &child) in nodes[node].children.iter().enumerate() {
            if nodes[child].padding {
                continue;
            }
            let j = stars.len();
            let mut code = stars[i].code.clone();
            code.push(digit as u32);
            stars.push(star(&nodes[child], stars[i].depth + 1, code, Some(i)));
            children.push(Vec::new());
            children[i].push(j);
            edges.push((i, j));
            queue.push_back((child, j));
        }
    }
    (stars, edges, children)
}

/// The tree, whose stars are breadth first, grown out from its root near
/// the chart's middle, less `margin` from its edge: each star's children,
/// biggest subtree first, each the best of a few tries at a random angle
/// and a varied distance (bigger subtrees reaching further): where its
/// link crosses none, and its subtree would have most room, away from
/// stars already placed and the chart's edge. Where nothing fits, a short
/// link into the widest gap between its parent's links.
fn grown(children: &[Vec<usize>], bounds: &Bounds, margin: f64, k: f64, variation: f64, rng: &mut StdRng) -> Vec<V> {
    let n = children.len();
    let mut size = vec![1usize; n];
    for i in (0..n).rev() {
        for &c in &children[i] {
            size[i] += size[c];
        }
    }
    let (a, b) = bounds.inner(margin);
    let mut pos = vec![V::default(); n];
    pos[0] = loop {
        let q = V::new(rng.random_range(-0.3..=0.3), rng.random_range(-0.3..=0.3));
        if !bounds.round || q.dot(q) <= 0.09 {
            break bounds.center + V::new(q.x * a, q.y * b);
        }
    };
    let mut links: Vec<(usize, usize)> = Vec::new();
    let mut placed = vec![0];
    let crosses = |links: &[(usize, usize)], pos: &[V], v: usize, q: V| {
        links.iter().any(|&(x, y)| x != v && y != v && segments_meet(pos[v], q, pos[x], pos[y]))
    };
    for v in 0..n {
        let mut kids = children[v].clone();
        kids.sort_by_key(|&c| std::cmp::Reverse(size[c]));
        for c in kids {
            // The room its subtree wants: a disc holding its stars as far
            // apart as stars spread evenly would be.
            let want = k * (size[c] as f64 / PI).sqrt();
            let mut best: Option<(f64, V)> = None;
            for attempt in 0..48 {
                // Shorter and shorter while nothing fits.
                let shrink = 0.5f64.powi(attempt / 16);
                let reach = k * (0.6 + 0.35 * (size[c] as f64).ln()) * (0.9 * variation * rng.random_range(-1.0..=1.0)).exp() * shrink;
                let q = pos[v] + V::angle(rng.random_range(0.0..TAU)) * reach;
                let edge = bounds.inside(q, margin);
                if edge < 0.0 || crosses(&links, &pos, v, q) {
                    continue;
                }
                let others = || placed.iter().filter(|&&w| w != v);
                let clear = others().map(|&w| (pos[w] - q).len().min(to_segment(pos[w], pos[v], q))).fold(f64::MAX, f64::min)
                    .min(links.iter().map(|&(x, y)| to_segment(q, pos[x], pos[y])).fold(f64::MAX, f64::min));
                let crowd = placed.iter().map(|&w| (1.0 - (pos[w] - q).len() / want).max(0.0)).sum::<f64>()
                    + 2.0 * (1.0 - edge / want).max(0.0);
                // Away from v's other links, round it.
                let heading = (q - pos[v]).y.atan2((q - pos[v]).x);
                let nearest_turn = links.iter().filter(|&&(x, y)| x == v || y == v).map(|&(x, y)| {
                    let d = pos[if x == v { y } else { x }] - pos[v];
                    let t = (d.y.atan2(d.x) - heading).rem_euclid(TAU);
                    t.min(TAU - t)
                }).fold(PI, f64::min);
                let fan = SPREAD * TAU / (children[v].len() + 1) as f64;
                let score = (clear / k).min(1.0) - crowd - 2.0 * (1.0 - nearest_turn / fan).max(0.0) + 0.3 * rng.random_range(0.0..1.0);
                if best.is_none_or(|(s, _)| score > s) {
                    best = Some((score, q));
                }
                if attempt >= 15 && best.is_some() {
                    break;
                }
            }
            pos[c] = best.map(|(_, q)| q).unwrap_or_else(|| {
                let mut turns: Vec<f64> = links.iter().filter(|&&(x, y)| x == v || y == v).map(|&(x, y)| {
                    let d = pos[if x == v { y } else { x }] - pos[v];
                    d.y.atan2(d.x)
                }).collect();
                turns.sort_by(f64::total_cmp);
                let heading = match turns.len() {
                    0 => rng.random_range(0.0..TAU),
                    m => (0..m).map(|i| {
                        let next = if i + 1 < m { turns[i + 1] } else { turns[0] + TAU };
                        (next - turns[i], turns[i] + (next - turns[i]) / 2.0)
                    }).max_by(|x, y| x.0.total_cmp(&y.0)).unwrap().1,
                };
                let mut reach = 0.3 * k;
                let mut q = pos[v] + V::angle(heading) * reach;
                while (crosses(&links, &pos, v, q) || bounds.inside(q, margin) < 0.0) && reach > 1e-6 * k {
                    reach /= 2.0;
                    q = pos[v] + V::angle(heading) * reach;
                }
                q
            });
            links.push((v, c));
            placed.push(c);
        }
    }
    pos
}

/// Each star's links, by index into the edges.
fn incident(n: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
    let mut incident = vec![Vec::new(); n];
    for (e, &(a, b)) in edges.iter().enumerate() {
        incident[a].push(e);
        incident[b].push(e);
    }
    incident
}

/// The chart's box, or the ellipse in it: `a` and `b` are half its width
/// and height.
#[derive(Clone, Copy)]
struct Bounds {
    center: V,
    a: f64,
    b: f64,
    round: bool,
}

impl Bounds {
    /// Half the width and height of where a star of radius `margin` can
    /// be, wholly inside.
    fn inner(&self, margin: f64) -> (f64, f64) {
        ((self.a - margin).max(1e-9), (self.b - margin).max(1e-9))
    }

    /// How far inside where a star of radius `margin` can be `p` is
    /// (negative outside): for the ellipse, along the line from its middle.
    fn inside(&self, p: V, margin: f64) -> f64 {
        let (a, b) = self.inner(margin);
        let q = p - self.center;
        if self.round {
            let rho = ((q.x / a).powi(2) + (q.y / b).powi(2)).sqrt();
            if rho < 1e-12 { a.min(b) } else { (1.0 - rho) * q.len() / rho }
        } else {
            (a - q.x.abs()).min(b - q.y.abs())
        }
    }

    /// `p` brought inside, for a star of radius `margin`.
    fn clamp(&self, p: V, margin: f64) -> V {
        let (a, b) = self.inner(margin);
        let q = p - self.center;
        if self.round {
            let rho = ((q.x / a).powi(2) + (q.y / b).powi(2)).sqrt();
            if rho > 1.0 { self.center + q * (1.0 / rho) } else { p }
        } else {
            self.center + V::new(q.x.clamp(-a, a), q.y.clamp(-b, b))
        }
    }

    /// The push off the edge of a star of radius `margin` at `p` that wants
    /// `space` from it: by as much as it's nearer.
    fn push(&self, p: V, margin: f64, space: f64) -> V {
        let (a, b) = self.inner(margin);
        let q = p - self.center;
        let off = |near: f64| (space - near).max(0.0);
        if self.round {
            let r = q.len();
            if r < 1e-12 {
                return V::default();
            }
            q * (-off(self.inside(p, margin)) / r)
        } else {
            V::new(off(q.x + a) - off(a - q.x), off(q.y + b) - off(b - q.y))
        }
    }
}

/// How evenly links round a star are spread: those nearer each other than
/// this share of an even spread turn apart.
const SPREAD: f64 = 0.75;

/// What each star and link wants: `k` is how far apart stars spread
/// evenly would be.
struct Forces {
    k: f64,
    /// How near each star lets others come, and the chart's edge.
    space: Vec<f64>,
    /// How hard each star's links pull it.
    pull: Vec<f64>,
    /// How long each link can be before it pulls.
    length: Vec<f64>,
}

struct Layout<'a> {
    edges: &'a [(usize, usize)],
    incident: Vec<Vec<usize>>,
    pos: Vec<V>,
    radius: Vec<f64>,
    /// How near a link may come to each star's centre.
    room: Vec<f64>,
    /// How short each link may be, centre to centre.
    shortest: Vec<f64>,
    bounds: Bounds,
}

impl Layout<'_> {
    fn relax(&mut self, forces: &Forces, iterations: usize, variation: f64, rng: &mut StdRng) {
        let k = forces.k;
        let mut order: Vec<usize> = (0..self.pos.len()).collect();
        for step in 0..iterations {
            let cool = 1.0 - step as f64 / iterations as f64;
            let most = k * (0.3 * cool.powf(1.5) + 0.01);
            let moves = self.forces(forces);
            order.shuffle(rng);
            for &i in &order {
                let f = moves[i];
                let mut d = if f.len() > most { f * (most / f.len()) } else { f };
                // A random nudge, cooling with the rest, so stars pushed
                // alike don't settle alike.
                d = d + V::angle(rng.random_range(0.0..TAU)) * (0.5 * variation * most * rng.random_range(0.0..1.0));
                let before = self.clearance(i, self.pos[i]);
                for _ in 0..4 {
                    let to = self.bounds.clamp(self.pos[i] + d, self.radius[i]);
                    if !self.crosses(i, to) {
                        let after = self.clearance(i, to);
                        if after >= 0.0 || after >= before {
                            self.pos[i] = to;
                            break;
                        }
                    }
                    d = d * 0.5;
                }
            }
        }
    }

    /// How far each star is pushed and pulled.
    fn forces(&self, forces: &Forces) -> Vec<V> {
        let Forces { k, space, pull, length } = forces;
        let (k, pos) = (*k, &self.pos);
        let n = pos.len();
        let mut f = vec![V::default(); n];
        // Stars nearer than their spaces push apart.
        for i in 0..n {
            for j in i + 1..n {
                let d = pos[i] - pos[j];
                let dist = d.len().max(0.01 * k);
                let want = space[i] + space[j];
                if dist < want {
                    let s = 0.5 * (want - dist) / dist;
                    f[i] = f[i] + d * s;
                    f[j] = f[j] - d * s;
                }
            }
        }
        // Links longer than their lengths pull together.
        for (e, &(a, b)) in self.edges.iter().enumerate() {
            let d = pos[b] - pos[a];
            let dist = d.len().max(1e-9);
            if dist > length[e] {
                let s = 0.5 * (pull[a] * pull[b]).sqrt() * (dist - length[e]) / dist;
                f[a] = f[a] + d * s;
                f[b] = f[b] - d * s;
            }
        }
        // Links push away stars that aren't theirs, within reach, and the
        // stars push back (PrEd).
        for i in 0..n {
            let reach = self.room[i] + 0.15 * k;
            for &(a, b) in self.edges {
                if a == i || b == i {
                    continue;
                }
                let ab = pos[b] - pos[a];
                let t = (pos[i] - pos[a]).dot(ab) / ab.dot(ab).max(1e-12);
                if t <= 0.0 || t >= 1.0 {
                    continue;
                }
                let d = pos[i] - (pos[a] + ab * t);
                let dist = d.len().max(0.01 * k);
                if dist >= reach {
                    continue;
                }
                let s = (reach - dist) / dist;
                f[i] = f[i] + d * s;
                f[a] = f[a] - d * (s * (1.0 - t));
                f[b] = f[b] - d * (s * t);
            }
        }
        // Round each star, links nearer each other than an even share of
        // the circle (by SPREAD) turn apart: their far stars are pushed
        // sideways, each half the shortfall, so a star with many links
        // fans them out rather than bunching them.
        for v in 0..n {
            let links = &self.incident[v];
            if links.len() < 2 {
                continue;
            }
            let mut ways: Vec<(f64, usize)> = links.iter().map(|&e| {
                let (a, b) = self.edges[e];
                let u = if a == v { b } else { a };
                let d = pos[u] - pos[v];
                (d.y.atan2(d.x), u)
            }).collect();
            ways.sort_by(|x, y| x.0.total_cmp(&y.0));
            let want = SPREAD * TAU / ways.len() as f64;
            for i in 0..ways.len() {
                let (here, u) = ways[i];
                let (next, w) = ways[(i + 1) % ways.len()];
                let gap = if i + 1 < ways.len() { next - here } else { next + TAU - here };
                if gap < want {
                    let turn = 0.5 * (want - gap);
                    // Sideways, back for u and on for w.
                    let side = |p: V| (p - pos[v]).perp() * (1.0 / (p - pos[v]).len().max(1e-9));
                    let (du, dw) = ((pos[u] - pos[v]).len(), (pos[w] - pos[v]).len());
                    f[u] = f[u] - side(pos[u]) * (turn * du);
                    f[w] = f[w] + side(pos[w]) * (turn * dw);
                }
            }
        }
        for i in 0..n {
            f[i] = f[i] + self.bounds.push(pos[i], self.radius[i], space[i]);
        }
        f
    }

    /// Whether star i, moved to `to`, has a link that crosses or touches
    /// another (links meeting at a star don't count).
    fn crosses(&self, i: usize, to: V) -> bool {
        self.incident[i].iter().any(|&e| {
            let (a, b) = self.edges[e];
            let other = self.pos[if a == i { b } else { a }];
            self.edges.iter().any(|&(c, d)| {
                c != a && c != b && d != a && d != b && segments_meet(to, other, self.pos[c], self.pos[d])
            })
        })
    }

    /// How much room star i has at `to`, the least of: how much further
    /// than they need links not its own are from it, its links are from
    /// other stars, and its links are than as short as they may be.
    /// Negative where something's too near.
    fn clearance(&self, i: usize, to: V) -> f64 {
        let mut least = f64::MAX;
        for (e, &(a, b)) in self.edges.iter().enumerate() {
            if a != i && b != i {
                least = least.min(to_segment(to, self.pos[a], self.pos[b]) - self.room[i]);
                continue;
            }
            let u = if a == i { b } else { a };
            let other = self.pos[u];
            least = least.min((other - to).len() - self.shortest[e]);
            for (w, &p) in self.pos.iter().enumerate() {
                if w != i && w != u {
                    least = least.min(to_segment(p, to, other) - self.room[w]);
                }
            }
        }
        least
    }

    /// Whether no two links cross: each pair is tested.
    fn planar(&self) -> bool {
        !links_cross(&self.pos, self.edges)
    }
}

/// Whether any two links cross or touch (links meeting at a star don't
/// count).
fn links_cross(pos: &[V], edges: &[(usize, usize)]) -> bool {
    edges.iter().enumerate().any(|(e, &(a, b))| {
        edges[e + 1..].iter().any(|&(c, d)| c != a && c != b && d != a && d != b && segments_meet(pos[a], pos[b], pos[c], pos[d]))
    })
}

fn vec2(v: V) -> Vec2 {
    Vec2::new(v.x as f32, v.y as f32)
}

/// Which side of the line through a and b c is on, times twice the
/// triangle's area.
fn orient(a: V, b: V, c: V) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Whether segments ab and cd meet, touching included.
pub(super) fn segments_meet(a: V, b: V, c: V, d: V) -> bool {
    let (d1, d2, d3, d4) = (orient(c, d, a), orient(c, d, b), orient(a, b, c), orient(a, b, d));
    let apart = |p: f64, q: f64| (p > 0.0 && q < 0.0) || (p < 0.0 && q > 0.0);
    if apart(d1, d2) && apart(d3, d4) {
        return true;
    }
    let within = |p: V, q: V, r: V| r.x >= p.x.min(q.x) && r.x <= p.x.max(q.x) && r.y >= p.y.min(q.y) && r.y <= p.y.max(q.y);
    (d1 == 0.0 && within(c, d, a)) || (d2 == 0.0 && within(c, d, b)) || (d3 == 0.0 && within(a, b, c)) || (d4 == 0.0 && within(a, b, d))
}

/// How far `p` is from the segment ab.
pub(super) fn to_segment(p: V, a: V, b: V) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.dot(ab).max(1e-12)).clamp(0.0, 1.0);
    (p - (a + ab * t)).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(seed: u64) -> StarChartSettings {
        StarChartSettings {
            center: Vec2::new(500.0, 400.0), width: 1000.0, height: 800.0, round: false,
            smoothing: 1, variation: 0.6, iterations: 150, star_size: None, gap: None, seed,
        }
    }

    /// Text in base 27: space 0, A to Z 1 to 26.
    fn base27(text: &str) -> Vec<u32> {
        text.chars().map(|c| if c == ' ' { 0 } else { c as u32 - 'A' as u32 + 1 }).collect()
    }

    fn positions(chart: &StarChart) -> Vec<V> {
        chart.stars.iter().map(|s| V::from(s.position)).collect()
    }

    #[test]
    fn the_tree_is_the_huffman_locks() {
        // Base 27 to binary with smoothing 1, as the lock's example: every
        // letter is a leaf, and E and S, the commonest, get short codes.
        let chart = huffman_stars(&base27("SEE THE SEA"), 27, 2, &StarChartSettings { iterations: 0, ..settings(1) }).unwrap();
        let leaves: Vec<&Star> = chart.stars.iter().filter(|s| s.symbol.is_some()).collect();
        assert_eq!(leaves.len(), 27);
        assert_eq!(chart.stars.len(), 53);
        assert_eq!(chart.edges.len(), 52);
        let code = |symbol: u32| leaves.iter().find(|s| s.symbol == Some(symbol)).unwrap().code.len();
        assert!(code(5) < code(26) && code(19) < code(26), "E and S should get shorter codes than Z");
        // A parent weighs what its children do together.
        for (i, star) in chart.stars.iter().enumerate() {
            let below: u64 = chart.edges.iter().filter(|e| e.0 == i).map(|e| chart.stars[e.1].weight).sum();
            assert!(star.symbol.is_some() || below == star.weight, "star {} weighs {}, its children {}", i, star.weight, below);
        }
        assert_eq!(chart.stars[0].weight, 11 + 27);
    }

    #[test]
    fn one_symbol_and_no_smoothing_gets_one_digit() {
        // As the lock's example: one symbol is padded to two leaves, and
        // the padding isn't a star.
        let chart = huffman_stars(&[7, 7, 7], 10, 2, &StarChartSettings { smoothing: 0, ..settings(0) }).unwrap();
        assert_eq!(chart.stars.len(), 2);
        assert_eq!(chart.stars[1].code, vec![1]);
        // Base 3 from three equally common symbols: one digit each.
        let chart = huffman_stars(&[0, 1, 2], 3, 3, &StarChartSettings { smoothing: 0, ..settings(0) }).unwrap();
        let codes: Vec<Vec<u32>> = chart.stars[1..].iter().map(|s| s.code.clone()).collect();
        assert_eq!(codes, vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn links_never_cross_and_stars_stay_inside() {
        // Fibonacci weights make a long, lopsided tree.
        let mut lopsided = Vec::new();
        let (mut a, mut b) = (1, 1);
        for s in 0..14u32 {
            lopsided.extend(std::iter::repeat_n(s, a));
            (a, b) = (b, a + b);
        }
        let texts = [base27("THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG"), lopsided];
        for seed in 0..4 {
            for (base, round, iterations) in [(2, false, 150), (2, false, 0), (3, true, 150), (5, false, 150)] {
                for symbols in &texts {
                    let s = StarChartSettings { round, iterations, ..settings(seed) };
                    let chart = huffman_stars(symbols, 27, base, &s).unwrap();
                    assert!(!links_cross(&positions(&chart), &chart.edges), "links cross: seed {}, base {}", seed, base);
                    for star in &chart.stars {
                        let q = V::from(star.position) - V::from(s.center);
                        // Positions are f32: a star clamped to the edge may
                        // round a hair past it.
                        let (a, b) = (s.width / 2.0 - star.radius + 1e-3, s.height / 2.0 - star.radius + 1e-3);
                        let inside = if round { (q.x / a).powi(2) + (q.y / b).powi(2) <= 1.0 + 1e-6 } else { q.x.abs() <= a && q.y.abs() <= b };
                        assert!(inside, "star at {:?} is outside the chart", star.position);
                    }
                }
            }
        }
    }

    #[test]
    fn links_keep_clear_of_stars() {
        // No link runs through a star not its own, or by one nearer than
        // its radius and the gap, and no stars overlap.
        for text in ["THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG", "SEE THE SEA", "A MAN A PLAN A CANAL PANAMA"] {
            for seed in 0..6 {
                for base in [2, 3, 4] {
                    let s = settings(seed);
                    let chart = huffman_stars(&base27(text), 27, base, &s).unwrap();
                    let pos = positions(&chart);
                    let gap = chart.stars.iter().map(|st| st.radius).fold(0.0, f64::max) / 4.0;
                    for (i, star) in chart.stars.iter().enumerate() {
                        for &(a, b) in &chart.edges {
                            if a != i && b != i {
                                let d = to_segment(pos[i], pos[a], pos[b]);
                                assert!(d > star.radius + gap, "{} seed {} base {}: a link passes {} from star {} of radius {}", text, seed, base, d, i, star.radius);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_seed_picks_the_layout() {
        let at = |seed| positions(&huffman_stars(&base27("SEE THE SEA"), 27, 2, &settings(seed)).unwrap());
        assert_eq!(at(4), at(4));
        assert_ne!(at(4), at(5));
    }

    #[test]
    fn bad_settings_are_named() {
        let err = |s: StarChartSettings, base| huffman_stars(&base27("SEE THE SEA"), 27, base, &s).err().unwrap();
        assert!(err(settings(0), 1).contains("output_base must be from 2 to 27"));
        assert!(err(StarChartSettings { variation: 1.5, ..settings(0) }, 2).contains("variation runs from 0 to 1"));
        assert!(err(StarChartSettings { width: 0.0, ..settings(0) }, 2).contains("width and height must be positive"));
        assert!(err(StarChartSettings { star_size: Some(500.0), ..settings(0) }, 2).contains("too small for stars"));
        assert!(err(StarChartSettings { iterations: 10_001, ..settings(0) }, 2).contains("iterations must be at most 10000"));
        assert!(huffman_stars(&[], 27, 2, &StarChartSettings { smoothing: 0, ..settings(0) }).err().unwrap().contains("nothing to build a code from"));
    }
}
