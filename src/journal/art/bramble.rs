//! Blackberry brambles in outline: an arching, thorny cane that branches
//! and twists, with serrated compound leaves, and five-petalled flowers
//! from closed buds to full blooms.
//!
//! The plant grows from the base of its cane. Along each cane or branch,
//! nodes alternate sides: compound leaves lower down, then flowers (and now
//! and then a branch, which grows the same way), with a flower at the tip.
//! Flowers are modelled in 3D (see `flower`): open ones face the viewer
//! more or less, half-open ones and buds turn further away along their
//! stalks. Every leaf and flower takes up room, and one that would crowd
//! another is tried at another angle or left out. Thorns, hooked towards
//! the tip the way the cane grows, line the canes between the nodes.
//!
//! In the drawing, flowers lie over leaves and leaves over the vines, and
//! branches cross over or under each other; stalks, branches and thorns
//! merge into the stems they grow from (see `Scene`).

use std::f64::consts::{PI, TAU};

use parry2d::math::Vec2;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::flower::{self, Pose};
use super::leaf;
use super::geom::{arc_end, V};
use super::scene::Scene;
use crate::journal::paths::StrokePath;

/// A bramble's parts as paths, to stroke separately.
pub struct Bramble {
    /// The canes and branches with their thorns, and the stalks of the
    /// leaves, flowers and buds.
    pub vines: StrokePath,
    /// The leaflets, with their midribs and veins.
    pub leaves: StrokePath,
    /// The flowers' petals, sepals, stamens and centres, from buds to full
    /// blooms.
    pub flowers: StrokePath,
}

/// How far open a flower is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Bud,
    Opening,
    Open,
}

impl Stage {
    /// The room a flower at this stage will take, before it's modelled:
    /// buds and half-open flowers sit a little along the way they face.
    fn room(self, pose: &Pose) -> (V, f64) {
        let toward = V::angle(pose.facing) * pose.tilt.sin();
        match self {
            Stage::Bud => (pose.center + toward * (0.25 * pose.radius), 0.35 * pose.radius),
            Stage::Opening => (pose.center + toward * (0.4 * pose.radius), 0.7 * pose.radius),
            Stage::Open => (pose.center, pose.radius),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Vine,
    Leaf,
    Flower,
}

// Sizes are in flower widths.
/// Half the cane's width at its base and towards its tip.
const CANE_HALF_WIDTH: (f64, f64) = (0.03, 0.015);
/// Half the width of the stalks of leaves, flowers and buds.
const STALK_HALF_WIDTH: f64 = 0.012;
/// A leaf's end leaflet, at its longest.
const LEAFLET_LENGTH: f64 = 1.2;
/// The shortest cane.
const MIN_LENGTH: f64 = 2.0;
/// How many times branches branch.
const MAX_DEPTH: usize = 2;

/// Flowers lie over leaves, and leaves over vines.
const VINE_DEPTH: f64 = 0.0;
const LEAF_DEPTH: f64 = 10.0;
const FLOWER_DEPTH: f64 = 20.0;

/// A blackberry bramble whose cane grows from `base` at `angle` (radians:
/// 0 is +x, -pi/2 up the page), `length` long, with flowers about
/// `flower_size` across; everything else is sized to the flowers. `seed`
/// picks how it grows.
pub fn bramble(base: Vec2, angle: f32, length: f32, flower_size: f32, seed: u64) -> Result<Bramble, String> {
    let (base, angle, length, size) = (V::from(base), angle as f64, length as f64, flower_size as f64);
    if !(base.is_finite() && angle.is_finite() && length.is_finite() && size.is_finite()) {
        return Err("every argument must be a finite number".into());
    }
    if !(size > 0.0) {
        return Err(format!("flower_size must be positive, got {}", size));
    }
    if !(length >= MIN_LENGTH * size) {
        return Err(format!("length must be at least {} times flower_size ({}), got {}", MIN_LENGTH, MIN_LENGTH * size, length));
    }
    let mut grower = Grower {
        rng: StdRng::seed_from_u64(seed),
        size,
        scene: Scene::new(0.004 * size, 0.25 * size, 0.02 * size),
        room: Vec::new(),
    };
    let turn = grower.rng.random_range(0.5..1.1) * grower.sign();
    let bend = grower.bend(length, turn, 0.3);
    let halves = (CANE_HALF_WIDTH.0 * size, CANE_HALF_WIDTH.1 * size);
    let cane = grower.stem((base, angle), length, halves, &bend, VINE_DEPTH + 2.5, true);
    grower.grow(&cane, 0);
    let scene = grower.scene;
    Ok(Bramble { vines: scene.path(Part::Vine), leaves: scene.path(Part::Leaf), flowers: scene.path(Part::Flower) })
}

/// How a stem curves along its length: an arch that tightens towards the
/// tip, turning it `turn` radians in all, and gentler wiggles: (how far the
/// heading swings, cycles along the stem, phase).
struct Bend {
    length: f64,
    turn: f64,
    wiggles: Vec<(f64, f64, f64)>,
}

impl Bend {
    fn curvature(&self, s: f64) -> f64 {
        let u = s / self.length;
        let arch = self.turn * 2.0 * u / self.length;
        arch + self.wiggles.iter()
            .map(|&(swing, cycles, phase)| swing * TAU * cycles / self.length * (TAU * cycles * u + phase).cos())
            .sum::<f64>()
    }
}

/// A cane, branch or stalk: its spine, sampled evenly, and its shape.
struct Stem {
    shape: usize,
    points: Vec<V>,
    headings: Vec<f64>,
    halves: Vec<f64>,
    step: f64,
    depth: f64,
}

impl Stem {
    fn length(&self) -> f64 {
        self.step * (self.points.len() - 1) as f64
    }
    fn end(&self) -> (V, f64) {
        (self.points[self.points.len() - 1], self.headings[self.headings.len() - 1])
    }
    /// The point, heading and half width `s` along.
    fn at(&self, s: f64) -> (V, f64, f64) {
        let x = (s / self.step).clamp(0.0, (self.points.len() - 1) as f64);
        let i = (x as usize).min(self.points.len() - 2);
        let t = x - i as f64;
        let mix = |a: f64, b: f64| a + (b - a) * t;
        (self.points[i].lerp(self.points[i + 1], t), mix(self.headings[i], self.headings[i + 1]), mix(self.halves[i], self.halves[i + 1]))
    }
}

struct Grower {
    rng: StdRng,
    /// The flowers' size, which everything is sized by.
    size: f64,
    scene: Scene<Part>,
    /// The room leaves, flowers and buds take up, as discs.
    room: Vec<(V, f64)>,
}

impl Grower {
    fn sign(&mut self) -> f64 {
        if self.rng.random::<bool>() { 1.0 } else { -1.0 }
    }

    fn bend(&mut self, length: f64, turn: f64, wiggle: f64) -> Bend {
        let wiggles = (0..2).map(|_| {
            (wiggle * self.rng.random_range(0.3..1.0), self.rng.random_range(0.5..1.8), self.rng.random_range(0.0..TAU))
        }).collect();
        Bend { length, turn, wiggles }
    }

    /// The points and headings of a spine from `start`, and its step.
    fn spine(&self, start: V, heading: f64, length: f64, bend: &Bend) -> (Vec<V>, Vec<f64>, f64) {
        let n = ((length / (self.size / 40.0)).ceil() as usize).max(6);
        let step = length / n as f64;
        let (mut points, mut headings) = (vec![start], vec![heading]);
        for i in 0..n {
            let (at, heading) = (points[i], headings[i]);
            let k = bend.curvature((i as f64 + 0.5) * step);
            points.push(arc_end(at, heading, k, step));
            headings.push(heading + k * step);
        }
        (points, headings, step)
    }

    /// Where a spine from `start` ends, and its heading there.
    fn reach(&self, start: V, heading: f64, length: f64, bend: &Bend) -> (V, f64) {
        let (points, headings, _) = self.spine(start, heading, length, bend);
        (points[points.len() - 1], headings[headings.len() - 1])
    }

    /// Add a stem growing `from` a point and heading, whose half width runs
    /// from `halves.0` to `halves.1`, cut square at its base, and tapering to
    /// a point at its tip if `pointed` or rounded off (to end under a leaf or
    /// flower) if not.
    fn stem(&mut self, from: (V, f64), length: f64, halves: (f64, f64), bend: &Bend, depth: f64, pointed: bool) -> Stem {
        let (points, headings, step) = self.spine(from.0, from.1, length, bend);
        let n = points.len() - 1;
        let taper = 4.0 * halves.1;
        let halves: Vec<f64> = (0..=n).map(|i| {
            let s = i as f64 * step;
            let half = halves.0 + (halves.1 - halves.0) * s / length;
            if pointed && length - s < taper { half * ((length - s) / taper).sqrt() } else { half }
        }).collect();
        let left = |i: usize| V::angle(headings[i]).perp();
        let mut outline: Vec<V> = (0..=n).map(|i| points[i] + left(i) * halves[i]).collect();
        let mut corners = vec![false; outline.len()];
        corners[0] = true;
        if pointed {
            corners[n] = true;
        } else {
            for k in 1..6 {
                let angle = headings[n] + PI / 2.0 - PI * k as f64 / 6.0;
                outline.push(points[n] + V::angle(angle) * halves[n]);
                corners.push(false);
            }
        }
        let right_from = if pointed { n - 1 } else { n };
        outline.extend((0..=right_from).rev().map(|i| points[i] - left(i) * halves[i]));
        corners.resize(outline.len(), false);
        corners[outline.len() - 1] = true;
        let shape = self.scene.add(Part::Vine, depth, outline, corners);
        Stem { shape, points, headings, halves, step, depth }
    }

    /// Add a stalk from `s` along `stem`, merged into it.
    fn stalk(&mut self, stem: &Stem, s: f64, heading: f64, length: f64, bend: &Bend) -> Stem {
        let (at, _, half) = stem.at(s);
        let halves = (STALK_HALF_WIDTH * self.size, 0.85 * STALK_HALF_WIDTH * self.size);
        let stalk = self.stem((at, heading), length, halves, bend, stem.depth + 0.01, false);
        self.scene.join(stem.shape, stalk.shape, at, 4.0 * (half + halves.0));
        stalk
    }

    /// Whether these discs have room: none crowds what's already grown.
    fn fits(&self, discs: &[(V, f64)]) -> bool {
        discs.iter().all(|&(c, r)| self.room.iter().all(|&(c2, r2)| (c - c2).len() > 0.75 * (r + r2)))
    }

    /// Grow a cane or branch: nodes along it, a flower or bud at its tip,
    /// and thorns between.
    fn grow(&mut self, stem: &Stem, depth: usize) {
        let size = self.size;
        let length = stem.length();
        let mut s = length * self.rng.random_range(0.12..0.22);
        let mut side = self.sign();
        let mut nodes = Vec::new();
        while s < length - 0.3 * size {
            let t = s / length;
            let roll = self.rng.random::<f64>();
            let can_branch = depth < MAX_DEPTH && length - s > 2.0 * size;
            // Sparse leaves (and some bare stretches) low down, then flowers
            // and buds, with the odd leaf among them.
            let leafy = t < if depth == 0 { 0.45 } else { 0.3 };
            let placed = if leafy {
                if can_branch && roll < 0.2 {
                    self.branch(stem, s, side, depth)
                } else if roll < 0.8 {
                    self.leaf(stem, s, side, depth)
                } else {
                    false
                }
            } else if roll < 0.15 {
                self.leaf(stem, s, side, depth)
            } else if roll < 0.42 {
                self.flower(stem, s, side, Stage::Open)
            } else if roll < 0.62 {
                self.flower(stem, s, side, Stage::Opening)
            } else if roll < 0.88 || !can_branch {
                self.flower(stem, s, side, Stage::Bud)
            } else {
                self.branch(stem, s, side, depth)
            };
            if placed {
                nodes.push(s);
                side = -side;
            } else if self.rng.random::<bool>() {
                side = -side;
            }
            s += size * if leafy { self.rng.random_range(0.8..1.2) } else { self.rng.random_range(0.3..0.5) };
        }
        // A flower at the tip, or a bud if there's no room for more.
        let (end, heading) = stem.end();
        let roll = self.rng.random::<f64>();
        let stage = if roll < 0.5 { Stage::Open } else if roll < 0.8 { Stage::Opening } else { Stage::Bud };
        for stage in [stage, Stage::Bud] {
            let pose = self.pose(end, heading, stage);
            if self.fits(&[stage.room(&pose)]) {
                self.blossom(&pose);
                break;
            }
        }
        self.thorns(stem, &nodes);
    }

    /// A branch from `s` along `stem`, grown like a cane.
    fn branch(&mut self, stem: &Stem, s: f64, side: f64, depth: usize) -> bool {
        let (at, heading, half) = stem.at(s);
        let length = ((stem.length() - s) * self.rng.random_range(0.6..0.9)).max(1.5 * self.size);
        let heading = heading + side * self.rng.random_range(0.35..0.8);
        // Turning back across the stem or away from it, and twisting.
        let turn = side * self.rng.random_range(-0.4..1.2);
        let bend = self.bend(length, turn, 0.5);
        let depth_in_drawing = VINE_DEPTH + self.rng.random_range(0.0..5.0);
        let halves = (0.8 * half, 0.8 * CANE_HALF_WIDTH.1 * self.size);
        let branch = self.stem((at, heading), length, halves, &bend, depth_in_drawing, true);
        self.scene.join(stem.shape, branch.shape, at, 8.0 * half);
        self.grow(&branch, depth + 1);
        true
    }

    /// A compound leaf on a stalk from `s` along `stem`: a pair of
    /// leaflets on short stalks either side of a node (sometimes a second,
    /// smaller pair), and an end leaflet on a longer stalk beyond it.
    fn leaf(&mut self, stem: &Stem, s: f64, side: f64, depth: usize) -> bool {
        let size = self.size;
        let (at, heading, _) = stem.at(s);
        for _ in 0..6 {
            let dir = heading + side * self.rng.random_range(0.6..1.2);
            let length = size * self.rng.random_range(0.4..0.75);
            let turn = side * self.rng.random_range(-0.2..0.5);
            let bend = self.bend(length, turn, 0.0);
            let (node, node_heading) = self.reach(at, dir, length, &bend);
            let scale = if depth == 0 { 1.0 } else { 0.85 };
            let terminal = size * LEAFLET_LENGTH * scale * self.rng.random_range(0.8..1.0);
            // (stalk heading, stalk length, leaflet length) for each pair.
            let mut pairs = vec![(self.rng.random_range(0.75..1.1), size * self.rng.random_range(0.08..0.15), terminal * self.rng.random_range(0.72..0.85))];
            if depth == 0 && self.rng.random::<f64>() < 0.3 {
                pairs.push((self.rng.random_range(1.6..2.0), size * self.rng.random_range(0.05..0.09), terminal * self.rng.random_range(0.5..0.6)));
            }
            let mut stalks = Vec::new();
            let mut leaflets = Vec::new();
            for &(spread, stalk, l) in pairs.iter().rev() {
                for sign in [-1.0, 1.0] {
                    let angle = node_heading + sign * spread;
                    stalks.push((angle, stalk, self.bend(stalk, 0.0, 0.0)));
                    leaflets.push(leaf::Placement {
                        base: node + V::angle(angle) * stalk,
                        heading: angle - sign * self.rng.random_range(0.0..0.25),
                        length: l,
                        width: l * self.rng.random_range(0.5..0.58),
                    });
                }
            }
            let rachis = size * self.rng.random_range(0.35..0.55);
            let rachis_turn = self.rng.random_range(-0.2..0.2);
            let rachis_bend = self.bend(rachis, rachis_turn, 0.0);
            let rachis_heading = node_heading + self.rng.random_range(-0.1..0.1);
            let (end, end_heading) = self.reach(node, rachis_heading, rachis, &rachis_bend);
            stalks.push((rachis_heading, rachis, rachis_bend));
            leaflets.push(leaf::Placement {
                base: end,
                heading: end_heading + self.rng.random_range(-0.15..0.15),
                length: terminal,
                width: terminal * self.rng.random_range(0.48..0.56),
            });
            let drawn = leaf::leaf(&leaflets, &mut self.rng);
            if !self.fits(&drawn.room) {
                continue;
            }
            let petiole = self.stalk(stem, s, dir, length, &bend);
            for (heading, length, bend) in stalks {
                self.stalk(&petiole, petiole.length(), heading, length, &bend);
            }
            let depth = LEAF_DEPTH + self.rng.random_range(0.0..5.0);
            self.scene.add_triangles(Part::Leaf, depth, drawn.triangles, drawn.lines, size / 6.0);
            self.room.extend(drawn.room);
            return true;
        }
        false
    }

    /// A flower at `stage` on a stalk from `s` along `stem`, facing away
    /// from its stalk.
    fn flower(&mut self, stem: &Stem, s: f64, side: f64, stage: Stage) -> bool {
        let (at, heading, _) = stem.at(s);
        for _ in 0..6 {
            let dir = heading + side * self.rng.random_range(0.3..1.1);
            let length = self.size * match stage {
                Stage::Bud => self.rng.random_range(0.3..0.65),
                _ => self.rng.random_range(0.45..0.9),
            };
            let turn = side * self.rng.random_range(-0.3..0.6);
            let bend = self.bend(length, turn, 0.0);
            let (end, end_heading) = self.reach(at, dir, length, &bend);
            let pose = self.pose(end, end_heading, stage);
            if !self.fits(&[stage.room(&pose)]) {
                continue;
            }
            self.stalk(stem, s, dir, length, &bend);
            self.blossom(&pose);
            return true;
        }
        false
    }

    /// How a flower at `stage` sits at the end of a stalk.
    fn pose(&mut self, center: V, facing: f64, stage: Stage) -> Pose {
        let radius = self.size / 2.0 * self.rng.random_range(0.85..1.1);
        let (openness, tilt) = match stage {
            Stage::Bud => (self.rng.random_range(0.0..0.12), self.rng.random_range(1.0..1.57)),
            Stage::Opening => (self.rng.random_range(0.3..0.65), self.rng.random_range(0.6..1.45)),
            Stage::Open => (self.rng.random_range(0.8..1.0), self.rng.random_range(0.0..0.95)),
        };
        Pose { center, radius, openness, tilt, facing }
    }

    fn blossom(&mut self, pose: &Pose) {
        let bloom = flower::bloom(pose, &mut self.rng);
        let depth = FLOWER_DEPTH + self.rng.random_range(0.0..5.0);
        self.scene.add_triangles(Part::Flower, depth, bloom.triangles, bloom.lines, pose.radius / 6.0);
        self.room.push((bloom.middle, 0.9 * bloom.reach));
    }

    /// Thorns along a cane or branch, alternating sides, clear of its nodes.
    fn thorns(&mut self, stem: &Stem, nodes: &[f64]) {
        let size = self.size;
        let mut s = size * self.rng.random_range(0.1..0.3);
        let mut side = self.sign();
        while s < stem.length() - 0.15 * size {
            if nodes.iter().all(|&node| (node - s).abs() > 0.12 * size) {
                self.thorn(stem, s, side);
            }
            s += size * self.rng.random_range(0.18..0.32);
            if self.rng.random::<f64>() < 0.75 {
                side = -side;
            }
        }
    }

    /// A thorn on `side` of `stem`, `s` along it, hooked towards the
    /// stem's tip, the way it grows.
    fn thorn(&mut self, stem: &Stem, s: f64, side: f64) {
        let (outline, corners) = self.thorn_outline(stem, s, side);
        let (at, heading, half) = stem.at(s);
        let edge = at + V::angle(heading).perp() * (side * half);
        let reach = 2.0 * (outline[6] - edge).len() + 2.0 * half;
        let id = self.scene.add(Part::Vine, stem.depth + 0.001, outline, corners);
        self.scene.join(stem.shape, id, edge, reach);
    }

    /// A thorn's outline, from the back of its base round its tip (the
    /// middle point) to the front, and which points are corners.
    fn thorn_outline(&mut self, stem: &Stem, s: f64, side: f64) -> (Vec<V>, Vec<bool>) {
        let (at, heading, half) = stem.at(s);
        let out = V::angle(heading).perp() * side;
        // Below, `along` runs back towards the base, away from the hook.
        let along = V::angle(heading + PI);
        let edge = at + out * half;
        let length = half * self.rng.random_range(1.6..2.4) + 0.008 * self.size;
        let base = half * self.rng.random_range(0.8..1.1);
        // The base sits a little inside the stem.
        let back = edge - along * base - out * (0.4 * half);
        let front = edge + along * base - out * (0.4 * half);
        let tip = edge + out * length - along * (length * self.rng.random_range(0.5..0.8));
        // The edge facing the stem's tip curves in, the other bows out.
        let inner = edge - along * (base / 2.0 + 0.45 * length) + out * (0.2 * length);
        let outer = (tip + front) * 0.5 + out * (0.25 * length) + along * (0.15 * length);
        let quad = |a: V, c: V, b: V, t: f64| a * ((1.0 - t) * (1.0 - t)) + c * (2.0 * t * (1.0 - t)) + b * (t * t);
        let mut outline: Vec<V> = (0..6).map(|k| quad(back, inner, tip, k as f64 / 6.0)).collect();
        outline.extend((0..=6).map(|k| quad(tip, outer, front, k as f64 / 6.0)));
        let mut corners = vec![false; outline.len()];
        corners[0] = true;
        corners[6] = true;
        corners[12] = true;
        (outline, corners)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thorns_hook_towards_the_tip() {
        let size = 100.0;
        let mut grower = Grower { rng: StdRng::seed_from_u64(1), size, scene: Scene::new(0.4, 25.0, 2.0), room: Vec::new() };
        let straight = grower.bend(500.0, 0.0, 0.0);
        // A cane growing up the page.
        let cane = grower.stem((V::new(0.0, 0.0), -PI / 2.0), 500.0, (3.0, 1.5), &straight, 0.0, true);
        for side in [1.0, -1.0] {
            let (outline, _) = grower.thorn_outline(&cane, 250.0, side);
            // Its tip (the middle corner) is further up than its base.
            let base = (outline[0].y + outline[outline.len() - 1].y) / 2.0;
            assert!(outline[6].y < base - 1.0, "side {}: tip {:?}, base at {}", side, outline[6], base);
        }
    }

    fn grow(seed: u64) -> Bramble {
        bramble(Vec2::new(0.0, 0.0), -1.3, 1400.0, 200.0, seed).expect("the bramble should grow")
    }

    #[test]
    fn the_same_seed_grows_the_same_bramble() {
        let (a, b, c) = (grow(3), grow(3), grow(4));
        assert_eq!(a.vines.to_string(), b.vines.to_string());
        assert_eq!(a.leaves.to_string(), b.leaves.to_string());
        assert_eq!(a.flowers.to_string(), b.flowers.to_string());
        assert_ne!(a.vines.to_string(), c.vines.to_string());
    }

    #[test]
    fn every_part_is_drawn() {
        for seed in 0..6 {
            let bramble = grow(seed);
            for (name, part) in [("vines", &bramble.vines), ("leaves", &bramble.leaves), ("flowers", &bramble.flowers)] {
                let points = part.points_within();
                assert!(!points.is_empty(), "seed {}: no {}", seed, name);
                assert!(points.iter().all(|p| p.is_finite()), "seed {}: {} has a non-finite point", seed, name);
            }
        }
    }

    #[test]
    fn bad_arguments_are_errors() {
        let err = |length: f32, size: f32| bramble(Vec2::new(0.0, 0.0), 0.0, length, size, 1).err().expect("should fail");
        assert!(err(1000.0, 0.0).contains("flower_size must be positive"));
        assert!(err(300.0, 200.0).contains("at least 2 times flower_size (400), got 300"));
        assert!(err(f32::NAN, 200.0).contains("finite"));
    }
}
