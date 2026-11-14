//! Times laying out a graph and evolving the layout (`graph::layout`'s
//! generations), and hashes the layouts, to see that a speedup changed
//! nothing.
//!
//! ```bash
//! cargo run --release --example evolve_bench -- --reps 5 --hashes /tmp/before.txt
//! ```
//!
//! Columns, in milliseconds, each the fastest of `--reps` runs: `forces` lays
//! the graph out with no generations, `evolved` with `--generations` (400,
//! modular_maze's), and `evolving` is the difference, with what a generation
//! costs in microseconds. `--hashes` writes a hash of every layout made, by
//! seed and label place, evolved and not. `--graph maze` times only the
//! graphs whose names start so.

use scribe::journal::art::{lay_out_graph, GraphLayout, GraphLayoutSettings, LabelPlace};
use std::fmt::Write as _;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Instant;

type Sizes = Vec<(f64, f64)>;
type Maybe = Vec<Option<(f64, f64)>>;

/// A graph to lay out: its nodes' sizes, its edges, their labels' sizes and
/// the nodes' names', and the box and gap it's laid out with.
struct Workload {
    name: &'static str,
    sizes: Sizes,
    edges: Vec<(usize, usize)>,
    labels: Maybe,
    names: Maybe,
    width: f64,
    height: f64,
    gap: f64,
}

/// The showcase's modular maze: 13 places, each a dot with its name boxed
/// beside it, and 16 ways, each with its steps.
fn maze() -> Workload {
    let names = [(148.2, 55.5), (167.3, 49.8), (173.7, 45.9), (213.9, 46.9), (169.8, 45.4), (240.1, 45.4), (160.2, 45.4), (240.4, 45.4), (178.6, 45.4), (220.2, 50.0), (188.3, 50.2), (180.5, 50.0), (175.9, 46.9)];
    let edges = vec![(0, 1), (0, 2), (0, 6), (0, 9), (1, 2), (1, 3), (1, 7), (1, 9), (2, 5), (3, 4), (3, 8), (3, 11), (5, 12), (9, 10), (11, 0), (12, 2)];
    let labels = [(8.8, 19.3), (11.4, 20.8), (11.9, 20.8), (11.8, 20.8), (8.8, 19.3), (11.4, 20.8), (11.9, 20.8), (11.2, 19.3), (11.3, 20.4), (8.8, 19.3), (11.9, 20.8), (11.2, 19.3), (11.4, 19.3), (8.8, 19.3), (11.4, 20.8), (11.3, 20.4)];
    Workload {
        name: "maze of 13",
        sizes: vec![(12.24, 12.24); names.len()],
        edges,
        labels: labels.iter().map(|&l| Some(l)).collect(),
        names: names.iter().map(|&n| Some(n)).collect(),
        width: 1000.0,
        height: 600.0,
        gap: 24.0,
    }
}

/// Five places as boxes, two of them joined both ways, some ways with no
/// label and one place with no name.
fn places() -> Workload {
    Workload {
        name: "places of 5",
        sizes: vec![(80.0, 36.0), (36.0, 36.0), (10.0, 10.0), (120.0, 36.0), (36.0, 36.0)],
        edges: vec![(0, 1), (1, 0), (1, 2), (2, 3), (3, 4), (4, 0), (2, 4)],
        labels: vec![Some((300.0, 60.0)), Some((200.0, 30.0)), None, Some((150.0, 30.0)), Some((220.0, 60.0)), None, Some((100.0, 30.0))],
        names: vec![Some((180.0, 50.0)), Some((160.0, 50.0)), Some((150.0, 50.0)), None, Some((200.0, 80.0))],
        width: 1000.0,
        height: 800.0,
        gap: 10.0,
    }
}

/// A ring of `n` places with a spoke across from every third, all named
/// and labelled: a bigger graph than journals draw.
fn ring(n: usize) -> Workload {
    let mut edges: Vec<(usize, usize)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
    edges.extend((0..n).step_by(3).map(|i| (i, (i + n / 2) % n)).filter(|&(a, b)| a != b));
    Workload {
        name: "ring of 30",
        sizes: vec![(14.0, 14.0); n],
        labels: (0..edges.len()).map(|e| Some((12.0 + (e % 4) as f64, 20.0))).collect(),
        names: (0..n).map(|i| Some((90.0 + 12.0 * (i % 5) as f64, 30.0))).collect(),
        edges,
        width: 1600.0,
        height: 1200.0,
        gap: 14.0,
    }
}

fn lay_out(w: &Workload, seed: u64, place: LabelPlace, generations: usize) -> GraphLayout {
    let settings = GraphLayoutSettings { width: w.width, height: w.height, gap: w.gap, seed, iterations: 300, place, generations };
    lay_out_graph(&w.sizes, &w.edges, &w.labels, &w.names, &settings).expect("the graph lays out")
}

fn hash(layout: &GraphLayout) -> u64 {
    let mut h = DefaultHasher::new();
    format!("{:?}", layout).hash(&mut h);
    h.finish()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let reps: usize = value("--reps").map_or(5, |v| v.parse().expect("--reps takes a number"));
    let generations: usize = value("--generations").map_or(400, |v| v.parse().expect("--generations takes a number"));
    let seeds: u64 = value("--seeds").map_or(6, |v| v.parse().expect("--seeds takes a number"));
    let hashes_to = value("--hashes");

    println!("{:<14}{:>10}{:>10}{:>10}{:>14}", "graph", "forces", "evolved", "evolving", "us/generation");
    let mut hashes = String::new();
    let only = value("--graph");
    for w in [maze(), places(), ring(30)].into_iter().filter(|w| only.as_ref().is_none_or(|o| w.name.starts_with(o.as_str()))) {
        let fastest = |generations: usize| {
            (0..reps).map(|_| {
                let t = Instant::now();
                std::hint::black_box(lay_out(&w, 1, LabelPlace::Along, generations));
                t.elapsed().as_secs_f64() * 1e3
            }).fold(f64::MAX, f64::min)
        };
        let (forces, evolved) = (fastest(0), fastest(generations));
        println!("{:<14}{:>10.2}{:>10.2}{:>10.2}{:>14.1}", w.name, forces, evolved, evolved - forces, (evolved - forces) * 1e3 / generations.max(1) as f64);
        if hashes_to.is_some() {
            for place in [LabelPlace::Along, LabelPlace::Beside, LabelPlace::Upright] {
                for seed in 0..seeds {
                    for g in [0, 60, generations] {
                        writeln!(hashes, "{} {:?} seed {} generations {}: {:016x}", w.name, place, seed, g, hash(&lay_out(&w, seed, place, g))).unwrap();
                    }
                }
            }
        }
    }
    if let Some(path) = hashes_to {
        std::fs::write(&path, hashes).expect("the hashes are written");
        println!("hashes written to {}", path);
    }
}
