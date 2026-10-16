//! Spanning trees over a set of points: the branches, veins, cracks and
//! mazes that join every point to every other by one way only.
//!
//! A tree is its edges, `(parent, child)` indices into the points, in the
//! order the algorithm added them, each pointing away from the root:
//! drawing them in order grows the tree out from there.

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, VecDeque};

use rand::seq::{IndexedRandom, SliceRandom};
use rand::Rng;

/// How far apart two points are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Metric {
    /// As the crow flies.
    Euclidean,
    /// The euclidean distance squared: the same minimum trees, but
    /// shortest paths prefer many short hops to one long one.
    SquaredEuclidean,
    /// Along the axes, as a taxi in a grid of streets.
    Manhattan,
    /// The farthest along any one axis, as a king moves on a chessboard.
    Chebyshev,
}

impl Metric {
    pub const NAMES: [&'static str; 4] = ["euclidean", "squared_euclidean", "manhattan", "chebyshev"];

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "euclidean" | "euclidian" => Metric::Euclidean,
            "squared_euclidean" => Metric::SquaredEuclidean,
            "manhattan" | "taxicab" => Metric::Manhattan,
            "chebyshev" => Metric::Chebyshev,
            _ => return None,
        })
    }

    pub fn distance(self, a: &[f64], b: &[f64]) -> f64 {
        let deltas = a.iter().zip(b).map(|(p, q)| (p - q).abs());
        match self {
            Metric::Euclidean => deltas.map(|d| d * d).sum::<f64>().sqrt(),
            Metric::SquaredEuclidean => deltas.map(|d| d * d).sum(),
            Metric::Manhattan => deltas.sum(),
            Metric::Chebyshev => deltas.fold(0.0, f64::max),
        }
    }
}

/// Which tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Algorithm {
    /// The minimum spanning tree (the shortest total length), grown from
    /// the root a nearest point at a time.
    Prim,
    /// The minimum spanning tree, its edges shortest first: fragments
    /// that join up, like frost.
    Kruskal,
    /// The maximum spanning tree: the longest total length, all long
    /// crossing lines.
    Maximum,
    /// Each point joined by its shortest path to the root, over the
    /// neighbour graph: veins and rivers, radiating out.
    ShortestPath,
    /// Breadth first from the root over the neighbour graph, nearest
    /// neighbours first: rings spreading out by hops.
    BreadthFirst,
    /// A random walk deep before it backtracks, over the neighbour graph:
    /// long winding corridors, as in a maze.
    DepthFirst,
    /// A uniformly random spanning tree of the neighbour graph (Wilson's
    /// algorithm): an even, natural-looking maze.
    Random,
}

impl Algorithm {
    pub const NAMES: [&'static str; 7] = ["prim", "kruskal", "maximum", "shortest_path", "breadth_first", "depth_first", "random"];

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "prim" | "minimum" => Algorithm::Prim,
            "kruskal" => Algorithm::Kruskal,
            "maximum" => Algorithm::Maximum,
            "shortest_path" => Algorithm::ShortestPath,
            "breadth_first" => Algorithm::BreadthFirst,
            "depth_first" => Algorithm::DepthFirst,
            "random" => Algorithm::Random,
            _ => return None,
        })
    }

    /// Whether it works over the neighbour graph, not every pair.
    pub fn uses_neighbours(self) -> bool {
        matches!(self, Algorithm::ShortestPath | Algorithm::BreadthFirst | Algorithm::DepthFirst | Algorithm::Random)
    }
}

/// The tree's edges, `(parent, child)`, grown from `root`. Over the
/// neighbour graph, each point is joined to its `neighbours` nearest (and
/// it to them), and the minimum spanning tree's edges keep it in one piece.
pub fn spanning_tree(points: &[Vec<f64>], algorithm: Algorithm, metric: Metric, root: usize, neighbours: usize, rng: &mut impl Rng) -> Vec<(usize, usize)> {
    if points.len() < 2 {
        return Vec::new();
    }
    assert!(root < points.len());
    let distance = |a: usize, b: usize| metric.distance(&points[a], &points[b]);
    match algorithm {
        Algorithm::Prim => prim(points.len(), root, distance, false),
        Algorithm::Maximum => prim(points.len(), root, distance, true),
        Algorithm::Kruskal => kruskal(points.len(), root, distance),
        _ => {
            let graph = neighbour_graph(points.len(), neighbours, root, &distance);
            match algorithm {
                Algorithm::ShortestPath => shortest_paths(&graph, root),
                Algorithm::BreadthFirst => breadth_first(&graph, root),
                Algorithm::DepthFirst => depth_first(&graph, root, rng),
                _ => wilson(&graph, root, rng),
            }
        }
    }
}

/// Prim's algorithm over every pair: O(n²), without a heap.
fn prim(n: usize, root: usize, distance: impl Fn(usize, usize) -> f64, maximum: bool) -> Vec<(usize, usize)> {
    let better = |a: f64, b: f64| if maximum { a > b } else { a < b };
    let mut in_tree = vec![false; n];
    // For each point not in the tree, the nearest (or farthest) one in it.
    let mut best: Vec<(f64, usize)> = (0..n).map(|i| (distance(root, i), root)).collect();
    in_tree[root] = true;
    let mut edges = Vec::with_capacity(n - 1);
    for _ in 1..n {
        let next = (0..n).filter(|&i| !in_tree[i])
            .reduce(|a, b| if better(best[b].0, best[a].0) { b } else { a })
            .expect("points left");
        in_tree[next] = true;
        edges.push((best[next].1, next));
        for i in 0..n {
            if !in_tree[i] {
                let d = distance(next, i);
                if better(d, best[i].0) {
                    best[i] = (d, next);
                }
            }
        }
    }
    edges
}

/// The tree's edges in their own order, each turned to point away from
/// `root`.
fn orient(n: usize, root: usize, edges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut adjacent = vec![Vec::new(); n];
    for &(a, b) in &edges {
        adjacent[a].push(b);
        adjacent[b].push(a);
    }
    let mut parent = vec![usize::MAX; n];
    parent[root] = root;
    let mut queue = VecDeque::from([root]);
    while let Some(a) = queue.pop_front() {
        for &b in &adjacent[a] {
            if parent[b] == usize::MAX {
                parent[b] = a;
                queue.push_back(b);
            }
        }
    }
    edges.into_iter().map(|(a, b)| if parent[b] == a { (a, b) } else { (b, a) }).collect()
}

/// Kruskal's algorithm adds the minimum spanning tree's edges shortest
/// first; found by Prim's, without the n² pairs Kruskal's sorts.
fn kruskal(n: usize, root: usize, distance: impl Fn(usize, usize) -> f64) -> Vec<(usize, usize)> {
    let mut edges = prim(n, root, &distance, false);
    edges.sort_by(|&(a, b), &(c, d)| distance(a, b).total_cmp(&distance(c, d)));
    orient(n, root, edges)
}

/// Each point's neighbours, nearest first, with how far: its `k` nearest,
/// the points it's among the `k` nearest of, and its minimum spanning
/// tree edges, so the graph is connected.
fn neighbour_graph(n: usize, k: usize, root: usize, distance: &impl Fn(usize, usize) -> f64) -> Vec<Vec<(usize, f64)>> {
    let mut adjacent: Vec<Vec<usize>> = vec![Vec::new(); n];
    for a in 0..n {
        let mut others: Vec<(f64, usize)> = (0..n).filter(|&b| b != a).map(|b| (distance(a, b), b)).collect();
        let k = k.min(others.len());
        if k > 0 {
            others.select_nth_unstable_by(k - 1, |p, q| p.0.total_cmp(&q.0));
        }
        for &(_, b) in &others[..k] {
            adjacent[a].push(b);
            adjacent[b].push(a);
        }
    }
    for (a, b) in prim(n, root, distance, false) {
        adjacent[a].push(b);
        adjacent[b].push(a);
    }
    adjacent.into_iter().enumerate().map(|(a, mut list)| {
        list.sort_unstable();
        list.dedup();
        let mut list: Vec<(usize, f64)> = list.into_iter().map(|b| (b, distance(a, b))).collect();
        list.sort_by(|p, q| p.1.total_cmp(&q.1).then(p.0.cmp(&q.0)));
        list
    }).collect()
}

/// A heap entry for Dijkstra: the nearest first.
#[derive(PartialEq)]
struct Reach(f64, usize, usize);

impl Eq for Reach {}

impl PartialOrd for Reach {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Reach {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0).then(self.1.cmp(&other.1))
    }
}

fn shortest_paths(graph: &[Vec<(usize, f64)>], root: usize) -> Vec<(usize, usize)> {
    let mut settled = vec![false; graph.len()];
    let mut best = vec![f64::INFINITY; graph.len()];
    best[root] = 0.0;
    let mut heap = BinaryHeap::from([Reverse(Reach(0.0, root, root))]);
    let mut edges = Vec::with_capacity(graph.len() - 1);
    while let Some(Reverse(Reach(d, a, from))) = heap.pop() {
        if settled[a] {
            continue;
        }
        settled[a] = true;
        if a != root {
            edges.push((from, a));
        }
        for &(b, w) in &graph[a] {
            if !settled[b] && d + w < best[b] {
                best[b] = d + w;
                heap.push(Reverse(Reach(d + w, b, a)));
            }
        }
    }
    edges
}

fn breadth_first(graph: &[Vec<(usize, f64)>], root: usize) -> Vec<(usize, usize)> {
    let mut seen = vec![false; graph.len()];
    seen[root] = true;
    let mut queue = VecDeque::from([root]);
    let mut edges = Vec::with_capacity(graph.len() - 1);
    while let Some(a) = queue.pop_front() {
        for &(b, _) in &graph[a] {
            if !seen[b] {
                seen[b] = true;
                edges.push((a, b));
                queue.push_back(b);
            }
        }
    }
    edges
}

fn depth_first(graph: &[Vec<(usize, f64)>], root: usize, rng: &mut impl Rng) -> Vec<(usize, usize)> {
    let mut seen = vec![false; graph.len()];
    seen[root] = true;
    let mut stack = vec![root];
    let mut edges = Vec::with_capacity(graph.len() - 1);
    let mut unseen = Vec::new();
    while let Some(&a) = stack.last() {
        unseen.clear();
        unseen.extend(graph[a].iter().map(|&(b, _)| b).filter(|&b| !seen[b]));
        match unseen.choose(rng) {
            Some(&b) => {
                seen[b] = true;
                edges.push((a, b));
                stack.push(b);
            }
            None => {
                stack.pop();
            }
        }
    }
    edges
}

/// Wilson's algorithm: from each point not yet in the tree, a random walk
/// with its loops erased, until it reaches the tree, which it then joins.
fn wilson(graph: &[Vec<(usize, f64)>], root: usize, rng: &mut impl Rng) -> Vec<(usize, usize)> {
    let n = graph.len();
    let mut in_tree = vec![false; n];
    in_tree[root] = true;
    // Where each point's walk last left it for.
    let mut next = vec![usize::MAX; n];
    let mut order: Vec<usize> = (0..n).collect();
    order.shuffle(rng);
    let mut edges = Vec::with_capacity(n - 1);
    let mut path = Vec::new();
    for start in order {
        let mut at = start;
        while !in_tree[at] {
            next[at] = graph[at][rng.random_range(0..graph[at].len())].0;
            at = next[at];
        }
        // Retrace the walk, loops erased, and join it on from the tree's end.
        path.clear();
        at = start;
        while !in_tree[at] {
            path.push(at);
            at = next[at];
        }
        for &a in path.iter().rev() {
            in_tree[a] = true;
            edges.push((next[a], a));
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn points(n: usize) -> Vec<Vec<f64>> {
        let mut rng = StdRng::seed_from_u64(5);
        (0..n).map(|_| vec![rng.random::<f64>() * 100.0, rng.random::<f64>() * 100.0]).collect()
    }

    #[test]
    fn every_algorithm_gives_a_tree_from_the_root() {
        let points = points(60);
        for name in Algorithm::NAMES {
            for metric in Metric::NAMES {
                let algorithm = Algorithm::from_name(name).unwrap();
                let edges = spanning_tree(&points, algorithm, Metric::from_name(metric).unwrap(), 3, 6, &mut StdRng::seed_from_u64(1));
                assert_eq!(edges.len(), points.len() - 1, "{} {}", name, metric);
                // Every child has one parent, and, edge by edge, the
                // parent is already in the tree, or will be, from the root.
                let mut parent = vec![usize::MAX; points.len()];
                for &(a, b) in &edges {
                    assert!(b != 3 && parent[b] == usize::MAX, "{}: {} has two parents", name, b);
                    parent[b] = a;
                }
                for mut i in 0..points.len() {
                    let mut steps = 0;
                    while i != 3 {
                        i = parent[i];
                        steps += 1;
                        assert!(steps < points.len(), "{}: a cycle", name);
                    }
                }
                if !matches!(algorithm, Algorithm::Kruskal) {
                    let mut grown = vec![false; points.len()];
                    grown[3] = true;
                    for &(a, b) in &edges {
                        assert!(grown[a], "{}: {} isn't in the tree yet", name, a);
                        grown[b] = true;
                    }
                }
            }
        }
    }

    #[test]
    fn prim_and_kruskal_agree_and_maximum_is_longer() {
        let points = points(40);
        let length = |edges: &[(usize, usize)]| edges.iter().map(|&(a, b)| Metric::Euclidean.distance(&points[a], &points[b])).sum::<f64>();
        let mut rng = StdRng::seed_from_u64(0);
        let prim = spanning_tree(&points, Algorithm::Prim, Metric::Euclidean, 0, 6, &mut rng);
        let kruskal = spanning_tree(&points, Algorithm::Kruskal, Metric::Euclidean, 0, 6, &mut rng);
        let maximum = spanning_tree(&points, Algorithm::Maximum, Metric::Euclidean, 0, 6, &mut rng);
        let random = spanning_tree(&points, Algorithm::Random, Metric::Euclidean, 0, 6, &mut rng);
        assert!((length(&prim) - length(&kruskal)).abs() < 1e-9);
        assert!(length(&random) > length(&prim) && length(&maximum) > length(&random));
    }

    #[test]
    fn metrics_measure() {
        let (a, b) = ([0.0, 0.0], [3.0, -4.0]);
        assert_eq!(Metric::Euclidean.distance(&a, &b), 5.0);
        assert_eq!(Metric::SquaredEuclidean.distance(&a, &b), 25.0);
        assert_eq!(Metric::Manhattan.distance(&a, &b), 7.0);
        assert_eq!(Metric::Chebyshev.distance(&a, &b), 4.0);
    }
}
