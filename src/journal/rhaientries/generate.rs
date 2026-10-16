//! Generators for illustrations: scattered points, the trees that join
//! them, and balanced-ternary digits.
//!
//! ```rhai
//! let seeds = distribute_points_2("poisson_disk", 300, [[0, 1200], [0, 800]]);  // [vec2]
//! let stars = distribute_points_3("halton", 50, [[0, 1], [0, 1], [0, 1]]);      // [vec3]
//! for edge in seeds.spanning_tree("shortest_path", "euclidean", #{ root: 0 }) {
//!     let (parent, child) = (seeds[edge[0]], seeds[edge[1]]);
//!     ...
//! }
//! balanced_ternary(5);                   // [1, -1, -1]: 9 - 3 - 1
//! balanced_ternary(5, 4);                // [0, 1, -1, -1]
//! from_balanced_ternary([1, -1, -1]);    // 5
//! ```
//!
//! Distributions and random trees draw from the engine's generator, so
//! `seed` fixes them.

use rhai::{Array, Dynamic, Engine, Map, INT};

use crate::journal::distribute::{distribute, Distribution};
use crate::journal::spanning::{spanning_tree, Algorithm, Metric};

use super::options::{check_keys, float};
use super::random::SharedRng;
use super::vec::{coordinates, from_coordinates};
use super::RhaiResult;

/// A distribution's `[[low, high], ...]`, an axis each.
fn bounds(caller: &str, dims: usize, bounds: &Array) -> RhaiResult<Vec<(f64, f64)>> {
    let bad = || format!("{}: the bounds should be [[low, high], ...], {} pairs of numbers with low below high", caller, dims);
    if bounds.len() != dims {
        return Err(format!("{}, got {} pairs", bad(), bounds.len()).into());
    }
    bounds.iter().enumerate().map(|(axis, pair)| {
        let pair = pair.clone().into_array().map_err(|_| bad())?;
        let what = format!("axis {}'s bound", axis);
        match pair.as_slice() {
            [low, high] => {
                let (low, high) = (float(caller, &what, low)?, float(caller, &what, high)?);
                if !(low < high) {
                    return Err(format!("{}: axis {}'s bounds are [{}, {}]; the low end must be below the high end", caller, axis, low, high).into());
                }
                Ok((low, high))
            }
            _ => Err(bad().into()),
        }
    }).collect()
}

fn register_distribution(engine: &mut Engine, rng: &SharedRng, dims: usize) {
    let name = format!("distribute_points_{}", dims);
    let rng = rng.clone();
    let caller = name.clone();
    engine.register_fn(&name, move |kind: &str, count: INT, ranges: Array| -> RhaiResult<Array> {
        let kind = Distribution::from_name(kind)
            .ok_or_else(|| format!("{}: unknown distribution '{}'; distributions: {}", caller, kind, Distribution::NAMES.join(", ")))?;
        if !(0..=MAX_POINTS).contains(&count) {
            return Err(format!("{}: count should be from 0 to {}, got {}", caller, MAX_POINTS, count).into());
        }
        let bounds = bounds(&caller, dims, &ranges)?;
        Ok(distribute(kind, count as usize, &bounds, &mut *rng.borrow_mut()).iter().map(|p| from_coordinates(p)).collect())
    });
}

/// More points than this is a mistake, not a picture.
const MAX_POINTS: INT = 1_000_000;

/// More points than this would take a spanning tree too long: it compares
/// every pair.
const MAX_TREE_POINTS: usize = 20_000;

const TREE_KEYS: [&str; 2] = ["root", "neighbours"];

fn tree(points: &Array, algorithm: &str, metric: &str, options: &Map, rng: &SharedRng) -> RhaiResult<Array> {
    const CALLER: &str = "spanning_tree";
    check_keys(CALLER, options, &TREE_KEYS)?;
    let algorithm = Algorithm::from_name(algorithm)
        .ok_or_else(|| format!("{}: unknown algorithm '{}'; algorithms: {}", CALLER, algorithm, Algorithm::NAMES.join(", ")))?;
    let metric = Metric::from_name(metric)
        .ok_or_else(|| format!("{}: unknown distance metric '{}'; metrics: {}", CALLER, metric, Metric::NAMES.join(", ")))?;
    if points.len() > MAX_TREE_POINTS {
        return Err(format!("{}: at most {} points, got {}", CALLER, MAX_TREE_POINTS, points.len()).into());
    }
    let points: Vec<Vec<f64>> = points.iter().map(|p| coordinates(CALLER, p)).collect::<RhaiResult<_>>()?;
    if points.windows(2).any(|pair| pair[0].len() != pair[1].len()) {
        return Err(format!("{}: the points should all be the same kind of vec", CALLER).into());
    }
    let whole = |key: &str, default: INT| -> RhaiResult<INT> {
        options.get(key).map_or(Ok(default), |v| v.as_int().map_err(|t| format!("{}: '{}' should be a whole number, got {}", CALLER, key, t).into()))
    };
    let root = whole("root", 0)?;
    if !points.is_empty() && !(0..points.len() as INT).contains(&root) {
        return Err(format!("{}: 'root' should be the index of a point, from 0 to {}, got {}", CALLER, points.len() - 1, root).into());
    }
    let neighbours = whole("neighbours", 4 * points.first().map_or(2, Vec::len) as INT - 2)?;
    if neighbours < 1 {
        return Err(format!("{}: 'neighbours' should be at least 1, got {}", CALLER, neighbours).into());
    }
    let edges = spanning_tree(&points, algorithm, metric, root as usize, neighbours as usize, &mut *rng.borrow_mut());
    Ok(edges.into_iter().map(|(a, b)| Dynamic::from_array(vec![Dynamic::from(a as INT), Dynamic::from(b as INT)])).collect())
}

/// `n`'s balanced-ternary digits, -1, 0 and 1, most significant first.
pub fn balanced_ternary(n: i64) -> Vec<i64> {
    let mut n = n as i128;
    let mut digits = Vec::new();
    while n != 0 {
        let digit = match n.rem_euclid(3) {
            2 => -1,
            d => d as i64,
        };
        digits.push(digit);
        n = (n - digit as i128) / 3;
    }
    if digits.is_empty() {
        digits.push(0);
    }
    digits.reverse();
    digits
}

pub(super) fn register(engine: &mut Engine, rng: SharedRng) {
    for dims in 2..=4 {
        register_distribution(engine, &rng, dims);
    }

    let r = rng.clone();
    engine.register_fn("spanning_tree", move |points: Array, algorithm: &str, metric: &str| tree(&points, algorithm, metric, &Map::new(), &r));
    let r = rng.clone();
    engine.register_fn("spanning_tree", move |points: Array, algorithm: &str, metric: &str, options: Map| tree(&points, algorithm, metric, &options, &r));

    let digits = |n: INT| -> Array { balanced_ternary(n).into_iter().map(Dynamic::from).collect() };
    engine.register_fn("balanced_ternary", digits);
    engine.register_fn("balanced_ternary", move |n: INT, width: INT| -> RhaiResult<Array> {
        let mut out = digits(n);
        if width < 1 || out.len() as INT > width {
            return Err(format!("balanced_ternary: {} takes {} digits, more than the width, {}", n, out.len(), width).into());
        }
        let padding = width as usize - out.len();
        out.splice(0..0, std::iter::repeat_n(Dynamic::from(0 as INT), padding));
        Ok(out)
    });
    engine.register_fn("from_balanced_ternary", |digits: Array| -> RhaiResult<INT> {
        digits.iter().try_fold(0 as INT, |n, d| match d.as_int() {
            Ok(d @ -1..=1) => n.checked_mul(3).and_then(|n| n.checked_add(d))
                .ok_or_else(|| "from_balanced_ternary: too many digits for a whole number".into()),
            _ => Err(format!("from_balanced_ternary: digits should be -1, 0 or 1, got {}", d).into()),
        })
    });
}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use super::balanced_ternary;
    use crate::journal::fonts::FontBook;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::new(Rc::new(FontBook::from_file(crate::journal::fonts::DEFAULT_FONTS).unwrap()))
    }

    #[test]
    fn balanced_ternary_round_trips() {
        assert_eq!(balanced_ternary(0), vec![0]);
        assert_eq!(balanced_ternary(5), vec![1, -1, -1]);
        assert_eq!(balanced_ternary(-5), vec![-1, 1, 1]);
        let ok: bool = engine().engine.eval(r#"
            let ok = balanced_ternary(5, 4) == [0, 1, -1, -1];
            for n in [-1000, -13, -1, 0, 1, 2, 3, 40, 9223372036854775807, -9223372036854775807] {
                ok = ok && from_balanced_ternary(balanced_ternary(n)) == n;
            }
            ok
        "#).unwrap();
        assert!(ok);
    }

    #[test]
    fn distributions_and_trees_from_scripts() {
        let ok: bool = engine().engine.eval(r#"
            seed(4);
            let a = distribute_points_2("poisson_disk", 50, [[0, 100], [0, 50.0]]);
            let b = distribute_points_3("halton", 20, [[0, 1], [0, 1], [-1, 1]]);
            let c = distribute_points_4("grid", 16, [[0, 1], [0, 1], [0, 1], [0, 1]]);
            let tree = a.spanning_tree("depth_first", "manhattan", #{ root: 7, neighbours: 3 });
            a.len() == 50 && type_of(a[0]) == "Vec2" && type_of(b[0]) == "Vec3" && type_of(c[0]) == "Vec4"
                && tree.len() == 49 && tree[0][0] == 7 && spanning_tree(b, "prim", "chebyshev").len() == 19
                && spanning_tree([], "random", "euclidean") == []
        "#).unwrap();
        assert!(ok);
        let again = |seed: i64| engine().engine.eval::<String>(&format!("seed({}); `${{distribute_points_2(\"uniform_random\", 3, [[0, 1], [0, 1]])}}`", seed)).unwrap();
        assert_eq!(again(1), again(1));
        assert_ne!(again(1), again(2));
    }

    #[test]
    fn bad_arguments_are_errors() {
        let e = engine();
        for (script, want) in [
            (r#"distribute_points_2("lumpy", 5, [[0, 1], [0, 1]])"#, "unknown distribution 'lumpy'; distributions: uniform_random"),
            (r#"distribute_points_2("grid", 5, [[0, 1]])"#, "2 pairs of numbers"),
            (r#"distribute_points_3("grid", 5, [[0, 1], [1, 1], [0, 1]])"#, "axis 1's bounds are [1, 1]"),
            (r#"distribute_points_2("grid", -1, [[0, 1], [0, 1]])"#, "count should be from 0"),
            (r#"[vec2(0, 0), vec2(1, 1)].spanning_tree("tallest", "euclidean")"#, "unknown algorithm 'tallest'"),
            (r#"[vec2(0, 0), vec2(1, 1)].spanning_tree("prim", "hamiltonian")"#, "unknown distance metric 'hamiltonian'"),
            (r#"[vec2(0, 0), vec3(1, 1, 1)].spanning_tree("prim", "euclidean")"#, "the same kind of vec"),
            (r#"[vec2(0, 0), 4].spanning_tree("prim", "euclidean")"#, "points should be vec2s, vec3s or vec4s"),
            (r#"[vec2(0, 0), vec2(1, 1)].spanning_tree("prim", "euclidean", #{ root: 2 })"#, "'root' should be the index of a point, from 0 to 1"),
            (r#"[vec2(0, 0)].spanning_tree("prim", "euclidean", #{ leaves: 2 })"#, "unknown option 'leaves'"),
            ("balanced_ternary(40, 3)", "more than the width"),
            ("from_balanced_ternary([1, 2])", "digits should be -1, 0 or 1"),
        ] {
            let err = e.engine.eval::<rhai::Dynamic>(script).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }
}
