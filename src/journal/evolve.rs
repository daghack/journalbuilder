//! Evolution: a population of candidates, the best of each generation
//! kept as they are and the rest bred from the fitter ones and changed a
//! little, generation after generation, the changes cooling as it goes.
//!
//! Anything can be evolved by implementing [`Evolve`] for the problem: what
//! a candidate is ([`Evolve::Genome`]), how bad one is ([`Evolve::cost`]),
//! how one changes ([`Evolve::mutate`]) and how two combine
//! ([`Evolve::cross`]). [`evolve`] starts from the candidates it's given
//! and never gives back one worse than the best of them.
//!
//! It's the same for a seed every time, and does no arithmetic of its own
//! beyond comparing costs, so a problem whose methods keep to `+ - * /` and
//! square roots evolves the same in the browser.
//!
//! A generation's candidates are bred one after another, from the one
//! source of chance, and then their costs, the dear part, are worked out
//! on every core at once: however many there are, a seed evolves the same.

use rand::rngs::StdRng;
use rand::Rng;
use rayon::prelude::*;

/// A problem whose candidates can be evolved.
pub trait Evolve: Sync {
    /// A candidate.
    type Genome: Clone + Send;

    /// How bad a candidate is: the lower the better. Finite. A
    /// generation's are worked out at once, on different cores.
    fn cost(&self, genome: &Self::Genome) -> f64;

    /// Change a candidate, more the hotter it is: `heat` falls from 1, at
    /// the first generation, toward 0, at the last.
    fn mutate(&self, genome: &mut Self::Genome, heat: f64, rng: &mut StdRng);

    /// A child of two candidates; by default, the first's copy.
    fn cross(&self, a: &Self::Genome, _b: &Self::Genome, _rng: &mut StdRng) -> Self::Genome {
        a.clone()
    }
}

/// How an evolution runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// How many candidates each generation holds.
    pub population: usize,
    /// How many generations are bred after the first.
    pub generations: usize,
    /// How many of each generation's best go on to the next unchanged.
    pub elite: usize,
    /// How many candidates are drawn to pick a parent: the best of them.
    pub tournament: usize,
    /// How likely a child is to have two parents (crossed) rather than one.
    pub cross: f64,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { population: 32, generations: 100, elite: 2, tournament: 3, cross: 0.4 }
    }
}

/// Evolve `problem`'s candidates from `seeds`: the first generation is the
/// seeds, then copies of them mutated at full heat until there are
/// `population`. Gives the best candidate found and its cost, or `None`
/// with no seeds.
pub fn evolve<P: Evolve>(problem: &P, seeds: &[P::Genome], settings: &Settings, rng: &mut StdRng) -> Option<(P::Genome, f64)> {
    if seeds.is_empty() {
        return None;
    }
    let size = settings.population.max(seeds.len()).max(1);
    let elite = settings.elite.clamp(1, size);
    let mut first: Vec<P::Genome> = seeds.to_vec();
    for k in 0..size - seeds.len() {
        let mut g = seeds[k % seeds.len()].clone();
        problem.mutate(&mut g, 1.0, rng);
        first.push(g);
    }
    let mut people = costed(problem, first);
    for generation in 0..settings.generations {
        // Best first; among equals, the earlier.
        people.sort_by(|a, b| a.1.total_cmp(&b.1));
        let heat = 1.0 - generation as f64 / settings.generations as f64;
        let children: Vec<P::Genome> = (elite..size).map(|_| {
            let a = pick(&people, settings.tournament, rng);
            let mut child = if rng.random_bool(settings.cross.clamp(0.0, 1.0)) {
                let b = pick(&people, settings.tournament, rng);
                problem.cross(&people[a].0, &people[b].0, rng)
            } else {
                people[a].0.clone()
            };
            problem.mutate(&mut child, heat, rng);
            child
        }).collect();
        people.truncate(elite);
        people.extend(costed(problem, children));
    }
    people.into_iter().min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Each candidate with its cost, in the order given: the costs worked
/// out on every core at once.
fn costed<P: Evolve>(problem: &P, genomes: Vec<P::Genome>) -> Vec<(P::Genome, f64)> {
    genomes.into_par_iter().map(|g| {
        let cost = problem.cost(&g);
        (g, cost)
    }).collect()
}

/// A parent, by tournament: the best of `size` drawn at random (the
/// people are sorted best first, so the least index drawn).
fn pick<G>(people: &[(G, f64)], size: usize, rng: &mut StdRng) -> usize {
    (0..size.max(1)).map(|_| rng.random_range(0..people.len())).min().unwrap_or(0)
}

/// A number from about -1 to 1, most often near 0: the mean of three
/// uniform ones, scaled (no logarithms, so it's the same in the browser).
pub fn wobble(rng: &mut StdRng) -> f64 {
    (rng.random_range(-1.0..1.0) + rng.random_range(-1.0..1.0) + rng.random_range(-1.0..1.0)) / 1.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    /// Points on a line, to be brought to their targets.
    struct Targets(Vec<f64>);

    impl Evolve for Targets {
        type Genome = Vec<f64>;
        fn cost(&self, g: &Vec<f64>) -> f64 {
            g.iter().zip(&self.0).map(|(x, t)| (x - t) * (x - t)).sum()
        }
        fn mutate(&self, g: &mut Vec<f64>, heat: f64, rng: &mut StdRng) {
            let i = rng.random_range(0..g.len());
            g[i] += wobble(rng) * (0.05 + 2.0 * heat);
        }
        fn cross(&self, a: &Vec<f64>, b: &Vec<f64>, rng: &mut StdRng) -> Vec<f64> {
            a.iter().zip(b).map(|(&x, &y)| if rng.random_bool(0.5) { x } else { y }).collect()
        }
    }

    fn run(seed: u64) -> (Vec<f64>, f64) {
        let problem = Targets(vec![3.0, -1.0, 7.5, 0.25]);
        let mut rng = StdRng::seed_from_u64(seed);
        evolve(&problem, &[vec![0.0; 4]], &Settings { generations: 300, ..Settings::default() }, &mut rng).unwrap()
    }

    #[test]
    fn it_comes_close_to_the_best() {
        let (_, cost) = run(1);
        assert!(cost < 0.05, "cost {}", cost);
    }

    #[test]
    fn a_seed_evolves_the_same_every_time() {
        assert_eq!(run(4), run(4));
        assert_ne!(run(4).0, run(5).0);
    }

    #[test]
    fn it_never_gives_back_worse_than_its_best_seed() {
        let problem = Targets(vec![1.0, 2.0]);
        let mut rng = StdRng::seed_from_u64(0);
        let seeds = [vec![9.0, 9.0], vec![1.0, 2.0]];
        let (best, cost) = evolve(&problem, &seeds, &Settings { generations: 3, ..Settings::default() }, &mut rng).unwrap();
        assert_eq!((best, cost), (vec![1.0, 2.0], 0.0));
        assert!(evolve(&problem, &[], &Settings::default(), &mut rng).is_none());
    }
}
