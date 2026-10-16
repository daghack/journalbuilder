//! Sets of points scattered through a box, of any number of dimensions:
//! seeds for stippling, scattered ornaments, the nodes of trees and webs.
//!
//! Every distribution gives exactly as many points as asked for, inside
//! the box (each coordinate from its axis's low end up to its high end).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use rand::seq::index::sample;
use rand::Rng;

/// How the points are scattered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Distribution {
    /// Each point anywhere, independently: clumps and gaps.
    UniformRandom,
    /// Evenly spread at random, no two close (blue noise), by weighted
    /// sample elimination (Yuksel 2015): random candidates, five per
    /// point, thinned by removing the most crowded first.
    PoissonDisk,
    /// One point at random in each of `count` cells of a grid, which
    /// stops clumps without looking regular.
    JitteredGrid,
    /// The centers of a grid's cells, row by row (x fastest): every cell
    /// when `count` fills the grid, otherwise a partial last row.
    Grid,
    /// The Halton sequence (bases 2, 3, 5, 7), shifted at random: a
    /// quasi-random fill, even at every count, that never repeats.
    Halton,
    /// The generalised golden-ratio sequence (Roberts' R sequence),
    /// shifted at random: like Halton, but more even.
    Golden,
}

impl Distribution {
    pub const NAMES: [&'static str; 6] = ["uniform_random", "poisson_disk", "jittered_grid", "grid", "halton", "golden"];

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "uniform_random" => Distribution::UniformRandom,
            "poisson_disk" => Distribution::PoissonDisk,
            "jittered_grid" => Distribution::JitteredGrid,
            "grid" => Distribution::Grid,
            "halton" => Distribution::Halton,
            "golden" => Distribution::Golden,
            _ => return None,
        })
    }
}

/// `count` points in the box `bounds`, an axis's `(low, high)` each, with
/// `low < high`: points of `bounds.len()` coordinates (1 to 4).
pub fn distribute(kind: Distribution, count: usize, bounds: &[(f64, f64)], rng: &mut impl Rng) -> Vec<Vec<f64>> {
    assert!((1..=4).contains(&bounds.len()) && bounds.iter().all(|(low, high)| low < high));
    // Points in the unit cube, then stretched to the box.
    let unit = match kind {
        Distribution::UniformRandom => uniform(count, bounds.len(), rng),
        Distribution::PoissonDisk => poisson_disk(count, bounds, rng),
        Distribution::JitteredGrid => grid(count, bounds, Some(rng)),
        Distribution::Grid => grid(count, bounds, None::<&mut rand::rngs::StdRng>),
        Distribution::Halton => halton(count, bounds.len(), rng),
        Distribution::Golden => golden(count, bounds.len(), rng),
    };
    unit.into_iter()
        .map(|p| p.iter().zip(bounds).map(|(t, (low, high))| (low + t * (high - low)).clamp(*low, *high)).collect())
        .collect()
}

fn uniform(count: usize, dims: usize, rng: &mut impl Rng) -> Vec<Vec<f64>> {
    (0..count).map(|_| (0..dims).map(|_| rng.random::<f64>()).collect()).collect()
}

/// How many cells along each axis: at least `count` cells in all, as near
/// square as the box allows.
fn grid_shape(count: usize, bounds: &[(f64, f64)]) -> Vec<usize> {
    let lengths: Vec<f64> = bounds.iter().map(|(low, high)| high - low).collect();
    let volume: f64 = lengths.iter().product();
    let per_unit = (count as f64 / volume).powf(1.0 / lengths.len() as f64);
    let mut cells: Vec<usize> = lengths.iter().map(|l| ((l * per_unit).floor() as usize).max(1)).collect();
    while cells.iter().product::<usize>() < count {
        // Split the axis whose cells are longest.
        let widest = (0..cells.len())
            .max_by(|&a, &b| (lengths[a] / cells[a] as f64).total_cmp(&(lengths[b] / cells[b] as f64)))
            .expect("at least one axis");
        cells[widest] += 1;
    }
    cells
}

/// Grid cells' centers (`jitter` none) or a random point in each of
/// `count` random cells, in the unit cube.
fn grid<R: Rng>(count: usize, bounds: &[(f64, f64)], jitter: Option<&mut R>) -> Vec<Vec<f64>> {
    if count == 0 {
        return Vec::new();
    }
    let cells = grid_shape(count, bounds);
    let total: usize = cells.iter().product();
    let cell = |mut index: usize| -> Vec<usize> {
        cells.iter().map(|n| {
            let i = index % n;
            index /= n;
            i
        }).collect()
    };
    match jitter {
        None => (0..count)
            .map(|index| cell(index).iter().zip(&cells).map(|(&i, &n)| (i as f64 + 0.5) / n as f64).collect())
            .collect(),
        Some(rng) => {
            let mut chosen = sample(rng, total, count).into_vec();
            chosen.sort_unstable();
            chosen.into_iter()
                .map(|index| cell(index).iter().zip(&cells).map(|(&i, &n)| (i as f64 + rng.random::<f64>()) / n as f64).collect())
                .collect()
        }
    }
}

/// The radical inverse of `index` in `base`: its digits mirrored about
/// the point.
fn radical_inverse(mut index: u64, base: u64) -> f64 {
    let (mut inverse, mut scale) = (0.0, 1.0 / base as f64);
    while index > 0 {
        inverse += (index % base) as f64 * scale;
        index /= base;
        scale /= base as f64;
    }
    inverse
}

fn halton(count: usize, dims: usize, rng: &mut impl Rng) -> Vec<Vec<f64>> {
    const BASES: [u64; 4] = [2, 3, 5, 7];
    let shift: Vec<f64> = (0..dims).map(|_| rng.random::<f64>()).collect();
    (1..=count as u64)
        .map(|i| (0..dims).map(|d| (radical_inverse(i, BASES[d]) + shift[d]).fract()).collect())
        .collect()
}

fn golden(count: usize, dims: usize, rng: &mut impl Rng) -> Vec<Vec<f64>> {
    // phi, the root of x^(d+1) = x + 1, by fixed-point iteration.
    let phi = (0..30).fold(2.0f64, |x, _| (1.0 + x).powf(1.0 / (dims as f64 + 1.0)));
    let alpha: Vec<f64> = (1..=dims).map(|d| phi.powi(-(d as i32))).collect();
    let shift: Vec<f64> = (0..dims).map(|_| rng.random::<f64>()).collect();
    (1..=count)
        .map(|n| (0..dims).map(|d| (shift[d] + n as f64 * alpha[d]).fract()).collect())
        .collect()
}

/// A candidate's crowding, a heap entry: the most crowded first.
#[derive(PartialEq)]
struct Crowding(f64, usize);

impl Eq for Crowding {}

impl PartialOrd for Crowding {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Crowding {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0).then(self.1.cmp(&other.1))
    }
}

/// Weighted sample elimination, in the unit cube, with distances measured
/// in the box (so a long box's points are spread evenly, not squashed).
fn poisson_disk(count: usize, bounds: &[(f64, f64)], rng: &mut impl Rng) -> Vec<Vec<f64>> {
    if count <= 1 {
        return uniform(count, bounds.len(), rng);
    }
    let dims = bounds.len();
    let lengths: Vec<f64> = bounds.iter().map(|(low, high)| high - low).collect();
    let candidates: Vec<Vec<f64>> = uniform(5 * count, dims, rng).into_iter()
        .map(|p| p.iter().zip(&lengths).map(|(t, l)| t * l).collect())
        .collect();

    // The spacing `count` points would have, packed as tightly as spheres
    // can be: the densest packing's density, over a unit ball's volume.
    let (density, ball) = match dims {
        1 => (1.0, 2.0),
        2 => (std::f64::consts::PI / 12f64.sqrt(), std::f64::consts::PI),
        3 => (std::f64::consts::PI / 18f64.sqrt(), 4.0 * std::f64::consts::PI / 3.0),
        _ => (std::f64::consts::PI.powi(2) / 16.0, std::f64::consts::PI.powi(2) / 2.0),
    };
    let volume: f64 = lengths.iter().product();
    let reach = 2.0 * (density * volume / (count as f64 * ball)).powf(1.0 / dims as f64);
    // Weight limiting: neighbours nearer than this count no more.
    let near = reach * (1.0 - (count as f64 / candidates.len() as f64).powf(1.5)) * 0.65;
    let weight = |d: f64| (1.0 - d.max(near) / reach).powi(8);

    // Each candidate's neighbours within `reach`, through a grid of cells
    // `reach` wide.
    let cell_of = |p: &[f64]| -> Vec<i64> { p.iter().map(|c| (c / reach).floor() as i64).collect() };
    let mut cells: HashMap<Vec<i64>, Vec<usize>> = HashMap::new();
    for (i, p) in candidates.iter().enumerate() {
        cells.entry(cell_of(p)).or_default().push(i);
    }
    let offsets: Vec<Vec<i64>> = (0..3usize.pow(dims as u32))
        .map(|mut k| (0..dims).map(|_| { let o = (k % 3) as i64 - 1; k /= 3; o }).collect())
        .collect();
    let mut neighbours: Vec<Vec<(usize, f64)>> = vec![Vec::new(); candidates.len()];
    let mut crowding = vec![0.0; candidates.len()];
    for (i, p) in candidates.iter().enumerate() {
        let home = cell_of(p);
        for offset in &offsets {
            let key: Vec<i64> = home.iter().zip(offset).map(|(c, o)| c + o).collect();
            for &j in cells.get(&key).into_iter().flatten() {
                if j == i {
                    continue;
                }
                let d = p.iter().zip(&candidates[j]).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt();
                if d < reach {
                    let w = weight(d);
                    neighbours[i].push((j, w));
                    crowding[i] += w;
                }
            }
        }
    }

    // Remove the most crowded candidate until `count` are left; entries
    // whose crowding has changed since they went on the heap are stale.
    let mut heap: BinaryHeap<Crowding> = crowding.iter().enumerate().map(|(i, &w)| Crowding(w, i)).collect();
    let mut alive = vec![true; candidates.len()];
    let mut left = candidates.len();
    while left > count {
        let Crowding(w, i) = heap.pop().expect("more candidates than points");
        if !alive[i] || w != crowding[i] {
            continue;
        }
        alive[i] = false;
        left -= 1;
        for &(j, wj) in &neighbours[i] {
            if alive[j] {
                crowding[j] -= wj;
                heap.push(Crowding(crowding[j], j));
            }
        }
    }
    candidates.into_iter().zip(alive)
        .filter(|(_, alive)| *alive)
        .map(|(p, _)| p.iter().zip(&lengths).map(|(c, l)| c / l).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn nearest_gap(points: &[Vec<f64>]) -> f64 {
        let mut gap = f64::INFINITY;
        for (i, p) in points.iter().enumerate() {
            for q in &points[i + 1..] {
                gap = gap.min(p.iter().zip(q).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt());
            }
        }
        gap
    }

    #[test]
    fn every_distribution_gives_count_points_in_the_box() {
        let bounds = [(-10.0, 30.0), (5.0, 15.0), (0.0, 1.0), (2.0, 3.0)];
        for name in Distribution::NAMES {
            for dims in 1..=4 {
                for count in [0, 1, 7, 100] {
                    let points = distribute(Distribution::from_name(name).unwrap(), count, &bounds[..dims], &mut StdRng::seed_from_u64(3));
                    assert_eq!(points.len(), count, "{} {}D", name, dims);
                    for p in &points {
                        assert_eq!(p.len(), dims);
                        assert!(p.iter().zip(&bounds).all(|(c, (low, high))| low <= c && c <= high), "{} {:?}", name, p);
                    }
                }
            }
        }
    }

    #[test]
    fn poisson_disk_points_keep_apart() {
        let bounds = [(0.0, 100.0), (0.0, 100.0)];
        let poisson = distribute(Distribution::PoissonDisk, 200, &bounds, &mut StdRng::seed_from_u64(1));
        let random = distribute(Distribution::UniformRandom, 200, &bounds, &mut StdRng::seed_from_u64(1));
        // 200 points packed hexagonally would be 7.6 apart.
        assert!(nearest_gap(&poisson) > 3.5, "{}", nearest_gap(&poisson));
        assert!(nearest_gap(&random) < nearest_gap(&poisson));
    }

    #[test]
    fn a_full_grid_is_regular() {
        let points = distribute(Distribution::Grid, 8, &[(0.0, 4.0), (0.0, 2.0)], &mut StdRng::seed_from_u64(0));
        assert_eq!(points[0], vec![0.5, 0.5]);
        assert_eq!(points[3], vec![3.5, 0.5]);
        assert_eq!(points[4], vec![0.5, 1.5]);
    }
}
