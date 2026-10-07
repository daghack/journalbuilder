//! Following a drawn path, such as a maze's: its lines (any number of
//! subpaths, which may branch off each other anywhere along them, or not
//! touch at all) as a graph, and that graph as chains for things to grow
//! along, each wandering a little off its line.
//!
//! Points closer than a snapping distance are one point, so a subpath that
//! starts on another, or ends near one, joins it. The graph splits into
//! strands at its junctions and dead ends, and each connected part is
//! walked from one of its ends (the one nearest where the path starts):
//! at each junction the chain carries on the
//! straightest way, and every other way starts a chain of its own,
//! branching from the first. A loop is followed once, and ends where it
//! comes back round.

use std::collections::{HashMap, HashSet};
use std::f64::consts::TAU;

use rand::rngs::StdRng;
use rand::Rng;

use super::geom::V;
use crate::journal::paths::StrokePath;

/// A line to grow along, and where it branches from.
pub(super) struct Chain {
    pub points: Vec<V>,
    /// The chain it branches from, and the index of the point on that
    /// chain it starts at.
    pub parent: Option<(usize, usize)>,
    /// Whether it ends at a dead end of the path (rather than where it
    /// meets a part already followed).
    pub dead_end: bool,
}

struct Graph {
    points: Vec<V>,
    links: Vec<Vec<usize>>,
}

impl Graph {
    /// `path`'s lines as points at most `step` apart, those within `snap`
    /// of each other merged.
    fn new(path: &StrokePath, step: f64, snap: f64) -> Self {
        let mut graph = Graph { points: Vec::new(), links: Vec::new() };
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        let cell = |p: V| ((p.x / snap).floor() as i64, (p.y / snap).floor() as i64);
        for line in path.polylines(step as f32) {
            let mut previous: Option<usize> = None;
            for p in line {
                let p = V::from(p);
                let (cx, cy) = cell(p);
                let near = (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| (cx + dx, cy + dy)))
                    .flat_map(|key| cells.get(&key).into_iter().flatten().copied())
                    .filter(|&i| Some(i) != previous && (graph.points[i] - p).len() < snap)
                    .min_by(|&a, &b| (graph.points[a] - p).len().total_cmp(&(graph.points[b] - p).len()));
                let index = near.unwrap_or_else(|| {
                    graph.points.push(p);
                    graph.links.push(Vec::new());
                    cells.entry(cell(p)).or_default().push(graph.points.len() - 1);
                    graph.points.len() - 1
                });
                if let Some(before) = previous.filter(|&b| b != index && !graph.links[b].contains(&index)) {
                    graph.links[before].push(index);
                    graph.links[index].push(before);
                }
                previous = Some(index);
            }
        }
        graph
    }

    /// Its strands: runs of points between junctions and dead ends (or, for
    /// a loop with neither, from any point round and back).
    fn strands(&self) -> Vec<Vec<usize>> {
        let is_node = |i: usize| self.links[i].len() != 2;
        let mut used: HashSet<(usize, usize)> = HashSet::new();
        let mut strands = Vec::new();
        let walk = |from: usize, first: usize, used: &mut HashSet<(usize, usize)>| -> Vec<usize> {
            let mut strand = vec![from, first];
            used.insert((from, first));
            used.insert((first, from));
            let (mut before, mut at) = (from, first);
            while !is_node(at) && at != from {
                let next = self.links[at].iter().copied().find(|&n| n != before).expect("a point inside a strand has two links");
                used.insert((at, next));
                used.insert((next, at));
                strand.push(next);
                (before, at) = (at, next);
            }
            strand
        };
        for node in (0..self.points.len()).filter(|&i| is_node(i)) {
            for &next in &self.links[node] {
                if !used.contains(&(node, next)) {
                    strands.push(walk(node, next, &mut used));
                }
            }
        }
        for start in 0..self.points.len() {
            if let Some(&next) = self.links[start].iter().find(|&&n| !used.contains(&(start, n))) {
                strands.push(walk(start, next, &mut used));
            }
        }
        strands
    }
}

/// Where a chain starts: its first strand, the point it starts from, and
/// the chain and point it branches from, if any.
type Start = (usize, usize, Option<(usize, usize)>);

/// The chains to grow along `path`: parents before the chains that branch
/// from them.
pub(super) fn chains(path: &StrokePath, step: f64, snap: f64) -> Vec<Chain> {
    let graph = Graph::new(path, step, snap);
    let strands = graph.strands();
    // Each point's strands, by their ends.
    let mut at: HashMap<usize, Vec<usize>> = HashMap::new();
    for (s, strand) in strands.iter().enumerate() {
        at.entry(strand[0]).or_default().push(s);
        at.entry(strand[strand.len() - 1]).or_default().push(s);
    }
    let oriented = |s: usize, from: usize| -> Vec<usize> {
        let strand = &strands[s];
        if strand[0] == from { strand.clone() } else { strand.iter().rev().copied().collect() }
    };
    // Which way a run of points heads, a few points in from `end`.
    let heading = |points: &[usize], from_end: bool| -> V {
        let n = points.len();
        let k = 3.min(n - 1);
        let (a, b) = if from_end { (points[n - 1 - k], points[n - 1]) } else { (points[0], points[k]) };
        let d = graph.points[b] - graph.points[a];
        d * (1.0 / d.len().max(1e-12))
    };

    let mut chains: Vec<Chain> = Vec::new();
    let mut followed: HashSet<usize> = HashSet::new();
    let mut reached: HashSet<usize> = HashSet::new();
    // Start each connected part at a dead end if it has one, the one
    // nearest where the path starts first.
    let mut starts: Vec<usize> = (0..graph.points.len()).filter(|&i| graph.links[i].len() == 1).collect();
    if let Some(&origin) = graph.points.first() {
        starts.sort_by(|&a, &b| (graph.points[a] - origin).len().total_cmp(&(graph.points[b] - origin).len()));
    }
    starts.extend(0..graph.points.len());
    for root in starts {
        let Some(first) = at.get(&root).and_then(|list| list.iter().copied().find(|s| !followed.contains(s))) else { continue };
        if reached.contains(&root) {
            continue;
        }
        reached.insert(root);
        let mut todo: Vec<Start> = vec![(first, root, None)];
        for &other in &at[&root] {
            if other != first {
                todo.push((other, root, Some((chains.len(), 0))));
            }
        }
        // Branches wait until the chain they branch from exists.
        todo.reverse();
        while let Some((strand, from, parent)) = todo.pop() {
            if followed.contains(&strand) {
                continue;
            }
            let index = chains.len();
            let mut points: Vec<usize> = vec![from];
            let (mut strand, mut from) = (strand, from);
            let mut branches = Vec::new();
            let dead_end = loop {
                followed.insert(strand);
                let run = oriented(strand, from);
                points.extend(&run[1..]);
                let end = run[run.len() - 1];
                if !reached.insert(end) {
                    break false;
                }
                let onward: Vec<usize> = at[&end].iter().copied().filter(|s| !followed.contains(s)).collect();
                if onward.is_empty() {
                    break graph.links[end].len() == 1;
                }
                // Carry on the straightest way; the others branch off.
                let coming = heading(&points, true);
                let next = *onward.iter().max_by(|&&a, &&b| {
                    let along = |s: usize| coming.dot(heading(&oriented(s, end), false));
                    along(a).total_cmp(&along(b))
                }).expect("there's a way on");
                for &other in onward.iter().filter(|&&s| s != next) {
                    branches.push((other, end, Some((index, points.len() - 1))));
                }
                (strand, from) = (next, end);
            };
            chains.push(Chain { points: points.iter().map(|&i| graph.points[i]).collect(), parent, dead_end });
            todo.extend(branches.into_iter().rev());
        }
    }
    chains
}

/// A chain's line made natural: rounded at its corners (smoothed over
/// about `smooth`), and wandering from side to side by up to `swing` over
/// about `wavelength`, starting on its line. `step` apart.
pub(super) fn wander(points: &[V], step: f64, smooth: f64, swing: f64, wavelength: f64, rng: &mut StdRng) -> Vec<V> {
    let even = resample(points, step);
    let n = even.len();
    if n < 3 {
        return even;
    }
    // Gaussian smoothing over a window that narrows towards the ends, so
    // they stay put and straight lines stay straight.
    let reach = ((3.0 * smooth / step).ceil() as isize).max(1);
    let weights: Vec<f64> = (-reach..=reach).map(|k| (-0.5 * (k as f64 * step / smooth).powi(2)).exp()).collect();
    let smoothed: Vec<V> = (0..n as isize).map(|i| {
        let half = reach.min(i).min(n as isize - 1 - i);
        let (sum, total) = (-half..=half).fold((V::default(), 0.0), |(sum, total), k| {
            let w = weights[(k + reach) as usize];
            (sum + even[(i + k) as usize] * w, total + w)
        });
        sum * (1.0 / total)
    }).collect();
    let waves = [(0.65, wavelength * rng.random_range(0.8..1.2), rng.random_range(0.0..TAU)), (0.35, wavelength * rng.random_range(0.35..0.5), rng.random_range(0.0..TAU))];
    (0..n).map(|i| {
        let s = i as f64 * step;
        let (a, b) = (smoothed[i.saturating_sub(1)], smoothed[(i + 1).min(n - 1)]);
        let along = (b - a) * (1.0 / (b - a).len().max(1e-12));
        let ease = (s / (wavelength / 4.0)).min(1.0);
        let offset = swing * ease * ease * waves.iter().map(|&(w, l, phase)| w * (TAU * s / l + phase).sin()).sum::<f64>();
        smoothed[i] + along.perp() * offset
    }).collect()
}

/// Points along `points` every `step` (the last one at its end).
pub(super) fn resample(points: &[V], step: f64) -> Vec<V> {
    let mut out = vec![points[0]];
    let mut carried = 0.0;
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let length = (b - a).len();
        let mut s = step - carried;
        while s <= length {
            out.push(a.lerp(b, s / length));
            s += step;
        }
        carried = length - (s - step);
    }
    if (out[out.len() - 1] - points[points.len() - 1]).len() > 1e-9 {
        out.push(points[points.len() - 1]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(lines: &[&[(f32, f32)]]) -> StrokePath {
        lines.iter().fold(StrokePath::new(), |path, line| {
            line[1..].iter().fold(path.move_to(line[0].0, line[0].1), |path, &(x, y)| path.line_to(x, y))
        })
    }

    #[test]
    fn a_branch_starting_along_another_line_joins_it() {
        // A T: a line across, and one from the middle of it downwards.
        let chains = chains(&path(&[&[(0.0, 0.0), (100.0, 0.0)], &[(50.0, 0.0), (50.0, 80.0)]]), 5.0, 2.0);
        assert_eq!(chains.len(), 2, "one chain on, one branching");
        assert!(chains[0].parent.is_none() && chains[0].dead_end);
        let (parent, at) = chains[1].parent.expect("the second branches");
        assert_eq!(parent, 0);
        assert!((chains[0].points[at] - V::new(50.0, 0.0)).len() < 1e-6, "{:?}", chains[0].points[at]);
        assert!(chains[1].dead_end);
        // Every point is followed once: 100 across and 80 down.
        let length = |c: &Chain| c.points.windows(2).map(|w| (w[1] - w[0]).len()).sum::<f64>();
        assert!((chains.iter().map(length).sum::<f64>() - 180.0).abs() < 1e-6);
    }

    #[test]
    fn chains_carry_on_the_straightest_way() {
        // A cross: from the left end, the chain carries straight on to the
        // right, and the up and down arms branch.
        let chains = chains(&path(&[&[(0.0, 0.0), (100.0, 0.0)], &[(50.0, -40.0), (50.0, 40.0)]]), 5.0, 2.0);
        assert_eq!(chains.len(), 3);
        let main = &chains[0];
        let ends = (main.points[0], main.points[main.points.len() - 1]);
        assert!((ends.0.y.abs() + ends.1.y.abs()) < 1e-6 && (ends.1.x - ends.0.x).abs() > 99.0, "{:?}", ends);
    }

    #[test]
    fn separate_lines_and_loops_are_followed_once() {
        let chains = chains(&path(&[
            &[(0.0, 0.0), (40.0, 0.0)],
            &[(0.0, 100.0), (40.0, 100.0), (40.0, 140.0), (0.0, 140.0), (0.0, 100.0)],
        ]), 5.0, 2.0);
        assert_eq!(chains.len(), 2);
        assert!(chains.iter().all(|c| c.parent.is_none()));
        // The loop ends where it comes back round, not at a dead end.
        assert!(chains[0].dead_end != chains[1].dead_end);
    }

    #[test]
    fn wandering_stays_near_the_line_and_starts_on_it() {
        let line: Vec<V> = (0..=40).map(|k| V::new(10.0 * k as f64, 0.0)).collect();
        let mut rng = <StdRng as rand::SeedableRng>::seed_from_u64(1);
        let wandered = wander(&line, 5.0, 20.0, 15.0, 100.0, &mut rng);
        assert!((wandered[0] - line[0]).len() < 1e-9);
        assert!(wandered.iter().all(|p| p.y.abs() <= 15.0 + 1e-9));
        assert!(wandered.iter().any(|p| p.y.abs() > 5.0), "it should wander");
    }
}
