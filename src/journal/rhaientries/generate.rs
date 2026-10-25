//! Generators for illustrations: scattered points, the trees that join
//! them, and balanced-ternary digits.
//!
//! ```rhai
//! let seeds = distribute_points("poisson_disk", 300, [[0, 1200], [0, 800]]);  // [vec2]
//! let stars = distribute_points("halton", 50, [[0, 1], [0, 1], [0, 1]]);      // [vec3]
//! for edge in seeds.spanning_tree(#{ algorithm: "shortest_path", metric: "euclidean", root: 0 }) {
//!     let (parent, child) = (seeds[edge[0]], seeds[edge[1]]);
//!     ...
//! }
//! let five = balanced_ternary(5);        // +--: 9 - 3 - 1
//! five.digits();                         // [1, -1, -1]
//! five.digits(2);                        // [[0, 1], [-1, -1]]: twos, padded in front
//! balanced_ternary(5, #{ width: 5 });   // 00+--
//! balanced_ternary("+--").value;         // 5, as balanced_ternary([1, -1, -1]).value
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
use super::{Path, RhaiResult, Vec2};

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

/// `distribute_points(kind, count, bounds)`: vec2s, vec3s or vec4s, as
/// many coordinates as `bounds` has pairs.
fn distribution(kind: &str, count: INT, ranges: &Array, rng: &SharedRng) -> RhaiResult<Array> {
    const CALLER: &str = "distribute_points";
    let kind = Distribution::from_name(kind)
        .ok_or_else(|| format!("{}: unknown distribution '{}'; distributions: {}", CALLER, kind, Distribution::NAMES.join(", ")))?;
    if !(0..=MAX_POINTS).contains(&count) {
        return Err(format!("{}: count should be from 0 to {}, got {}", CALLER, MAX_POINTS, count).into());
    }
    if !(2..=4).contains(&ranges.len()) {
        return Err(format!("{}: the bounds should be [[low, high], ...], 2, 3 or 4 pairs (for vec2s, vec3s or vec4s), got {}", CALLER, ranges.len()).into());
    }
    let bounds = bounds(CALLER, ranges.len(), ranges)?;
    Ok(distribute(kind, count as usize, &bounds, &mut *rng.borrow_mut()).iter().map(|p| from_coordinates(p)).collect())
}

/// More points than this is a mistake, not a picture.
const MAX_POINTS: INT = 1_000_000;

/// More points than this would take a spanning tree too long: it compares
/// every pair.
const MAX_TREE_POINTS: usize = 20_000;

const TREE_KEYS: [&str; 4] = ["algorithm", "metric", "root", "neighbours"];

/// `points.spanning_tree(#{ algorithm, metric, root, neighbours })`.
fn tree(points: &Array, options: &Map, rng: &SharedRng) -> RhaiResult<Array> {
    const CALLER: &str = "spanning_tree";
    check_keys(CALLER, options, &TREE_KEYS)?;
    let name = |key: &str, default: &str| options.get(key).map_or(default.to_string(), |v| v.to_string());
    let (algorithm, metric) = (name("algorithm", "prim"), name("metric", "euclidean"));
    let (algorithm, metric) = (algorithm.as_str(), metric.as_str());
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

/// A balanced-ternary number: digits -1, 0 and 1, most significant first,
/// each worth three times the next.
#[derive(Debug, Clone, PartialEq)]
pub struct BalancedTernary {
    digits: Vec<i8>,
}

impl BalancedTernary {
    const CALLER: &'static str = "balanced_ternary";
    /// `n` in as few digits as it needs, or at least `width` of them, the
    /// rest 0s in front.
    fn from_int(n: INT, options: &Map) -> RhaiResult<Self> {
        check_keys(Self::CALLER, options, &["width"])?;
        let width = match options.get("width") {
            None => 1,
            Some(w) => match w.as_int() {
                Ok(w) if w >= 1 => w as usize,
                _ => return Err(format!("{}: 'width' should be a whole number of at least 1, got {}", Self::CALLER, w).into()),
            },
        };
        let digits = balanced_ternary(n);
        let mut padded = vec![0i8; width.saturating_sub(digits.len())];
        padded.extend(digits.into_iter().map(|d| d as i8));
        Ok(BalancedTernary { digits: padded })
    }
    /// From digits, as given (leading 0s kept), however many there are.
    fn from_digits(digits: Vec<i8>) -> RhaiResult<Self> {
        if digits.is_empty() {
            return Err(format!("{}: needs at least one digit", Self::CALLER).into());
        }
        Ok(BalancedTernary { digits })
    }
    fn from_str(text: &str) -> RhaiResult<Self> {
        let digits = text.chars().map(|c| match c {
            '-' => Ok(-1),
            '0' => Ok(0),
            '+' => Ok(1),
            c => Err(format!("{}: digits are written -, 0 and +, got '{}'", Self::CALLER, c)),
        }).collect::<Result<Vec<i8>, _>>()?;
        Self::from_digits(digits)
    }
    fn from_array(digits: &Array) -> RhaiResult<Self> {
        let digits = digits.iter().map(|d| match d.as_int() {
            Ok(d @ -1..=1) => Ok(d as i8),
            _ => Err(format!("{}: digits should be -1, 0 or 1, got {}", Self::CALLER, d)),
        }).collect::<Result<Vec<i8>, _>>()?;
        Self::from_digits(digits)
    }
    /// The whole number it is, if it isn't too long to be one.
    pub fn value(&self) -> Option<INT> {
        self.digits.iter().try_fold(0 as INT, |n, &d| n.checked_mul(3)?.checked_add(d as INT))
    }
    pub fn digits(&self) -> &[i8] {
        &self.digits
    }
    /// The digits in chunks of `width`, the first padded with 0s in front.
    fn chunks(&self, width: INT) -> RhaiResult<Array> {
        if width < 1 {
            return Err(format!("digits: the width should be at least 1, got {}", width).into());
        }
        let width = width as usize;
        let mut digits = vec![0i8; (width - self.digits.len() % width) % width];
        digits.extend(&self.digits);
        Ok(digits.chunks(width).map(|chunk| Dynamic::from_array(chunk.iter().map(|&d| Dynamic::from(d as INT)).collect())).collect())
    }
}

impl std::fmt::Display for BalancedTernary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for d in &self.digits {
            f.write_str(match d { -1 => "-", 0 => "0", _ => "+" })?;
        }
        Ok(())
    }
}

/// `nearest(points, point)`: the index of the point nearest `point`.
fn nearest(points: &Array, point: &Dynamic) -> RhaiResult<INT> {
    const CALLER: &str = "nearest";
    let to = coordinates(CALLER, point)?;
    let mut best: Option<(usize, f64)> = None;
    for (i, p) in points.iter().enumerate() {
        let p = coordinates(CALLER, p)?;
        if p.len() != to.len() {
            return Err(format!("{}: the points and the point should be the same kind of vec", CALLER).into());
        }
        let d: f64 = p.iter().zip(&to).map(|(a, b)| (a - b) * (a - b)).sum();
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i as INT).ok_or_else(|| format!("{}: there are no points", CALLER).into())
}

/// `new_edges(points, pairs)`: a path of a line between the two points
/// each `[a, b]` pair indexes, as `spanning_tree` gives them.
fn edges(points: &Array, pairs: &Array) -> RhaiResult<Path> {
    const CALLER: &str = "new_edges";
    let points: Vec<Vec2> = points.iter().enumerate().map(|(i, p)| {
        p.clone().try_cast::<Vec2>().ok_or_else(|| -> Box<rhai::EvalAltResult> { format!("{}: point {} is a {}, not a vec2", CALLER, i, p.type_name()).into() })
    }).collect::<RhaiResult<_>>()?;
    let mut path = crate::journal::paths::StrokePath::new();
    for pair in pairs {
        let bad = || -> Box<rhai::EvalAltResult> { format!("{}: each pair should be [a, b], indices of points from 0 to {}, got {}", CALLER, points.len().saturating_sub(1), pair).into() };
        let pair = pair.clone().into_array().map_err(|_| bad())?;
        let index = |d: &Dynamic| d.as_int().ok().filter(|&i| (0..points.len() as INT).contains(&i)).map(|i| points[i as usize].clone());
        match pair.as_slice() {
            [a, b] => {
                let (a, b) = (index(a).ok_or_else(bad)?, index(b).ok_or_else(bad)?);
                path.append(crate::journal::paths::StrokePath::line(a.into(), b.into()));
            }
            _ => return Err(bad()),
        }
    }
    Ok(Path::new(path))
}

pub(super) fn register(engine: &mut Engine, rng: SharedRng) {
    let r = rng.clone();
    engine.register_fn("distribute_points", move |kind: &str, count: INT, ranges: Array| distribution(kind, count, &ranges, &r));

    let r = rng.clone();
    engine.register_fn("spanning_tree", move |points: Array| tree(&points, &Map::new(), &r));
    let r = rng.clone();
    engine.register_fn("spanning_tree", move |points: Array, options: Map| tree(&points, &options, &r));
    engine.register_fn("nearest", |points: Array, point: Dynamic| nearest(&points, &point));
    engine.register_fn("new_edges", |points: Array, pairs: Array| edges(&points, &pairs));

    engine.register_type_with_name::<BalancedTernary>("BalancedTernary")
        .register_fn("balanced_ternary", |n: INT| BalancedTernary::from_int(n, &Map::new()))
        .register_fn("balanced_ternary", |n: INT, options: Map| BalancedTernary::from_int(n, &options))
        .register_fn("balanced_ternary", |text: &str| BalancedTernary::from_str(text))
        .register_fn("balanced_ternary", |digits: Array| BalancedTernary::from_array(&digits))
        .register_get("value", |bt: &mut BalancedTernary| -> RhaiResult<INT> {
            bt.value().ok_or_else(|| format!("value: {} digits are too many for a whole number", bt.digits.len()).into())
        })
        .register_fn("digits", |bt: &mut BalancedTernary| -> Array { bt.digits.iter().map(|&d| Dynamic::from(d as INT)).collect() })
        .register_fn("digits", |bt: &mut BalancedTernary, width: INT| bt.chunks(width))
        .register_fn("to_string", |bt: &mut BalancedTernary| bt.to_string())
        .register_fn("to_debug", |bt: &mut BalancedTernary| bt.to_string())
        .register_fn("==", |a: BalancedTernary, b: BalancedTernary| a == b)
        .register_fn("!=", |a: BalancedTernary, b: BalancedTernary| a != b);
}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use super::balanced_ternary;
    use crate::journal::fonts::FontBook;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::new(Rc::new(FontBook::from_file(crate::journal::fonts::DEFAULT_GLYPHSETS).unwrap()))
    }

    #[test]
    fn balanced_ternary_round_trips() {
        assert_eq!(balanced_ternary(0), vec![0]);
        assert_eq!(balanced_ternary(5), vec![1, -1, -1]);
        assert_eq!(balanced_ternary(-5), vec![-1, 1, 1]);
        let ok: bool = engine().engine.eval(r#"
            let five = balanced_ternary(5);
            let ok = five.digits() == [1, -1, -1] && `${five}` == "+--" && five.value == 5
                && five.digits(4) == [[0, 1, -1, -1]] && five.digits(2) == [[0, 1], [-1, -1]]
                && balanced_ternary(2026).digits(3) == [[0, 1, 0], [-1, 1, 0], [0, 0, 1]]
                && balanced_ternary("+--") == five && balanced_ternary([0, 1, -1, -1]).value == 5
                && balanced_ternary("00+").digits() == [0, 0, 1]
                && balanced_ternary(5, #{ width: 5 }).digits() == [0, 0, 1, -1, -1]
                && balanced_ternary(5, #{ width: 2 }) == five;
            // More digits than a whole number holds are still digits.
            let long = [];
            for i in 0..100 { long.push(1); }
            ok = ok && balanced_ternary(long).digits().len() == 100;
            for n in [-1000, -13, -1, 0, 1, 2, 3, 40, 9223372036854775807, -9223372036854775807] {
                ok = ok && balanced_ternary(balanced_ternary(n).digits()).value == n;
            }
            ok
        "#).unwrap();
        assert!(ok);
        let err = engine().engine.eval::<i64>("let d = []; for i in 0..41 { d.push(1); } balanced_ternary(d).value").unwrap_err();
        assert!(err.to_string().contains("too many"), "{}", err);
    }

    #[test]
    fn distributions_and_trees_from_scripts() {
        let ok: bool = engine().engine.eval(r#"
            seed(4);
            let a = distribute_points("poisson_disk", 50, [[0, 100], [0, 50.0]]);
            let b = distribute_points("halton", 20, [[0, 1], [0, 1], [-1, 1]]);
            let c = distribute_points("grid", 16, [[0, 1], [0, 1], [0, 1], [0, 1]]);
            let tree = a.spanning_tree(#{ algorithm: "depth_first", metric: "manhattan", root: 7, neighbours: 3 });
            a.len() == 50 && type_of(a[0]) == "Vec2" && type_of(b[0]) == "Vec3" && type_of(c[0]) == "Vec4"
                && tree.len() == 49 && tree[0][0] == 7 && spanning_tree(b, #{ metric: "chebyshev" }).len() == 19
                && spanning_tree([], #{ algorithm: "random" }) == [] && a.spanning_tree() == a.spanning_tree(#{ algorithm: "prim" })
                && nearest(a, a[9]) == 9 && new_edges(a, tree).bounds().width > 0
        "#).unwrap();
        assert!(ok);
        let again = |seed: i64| engine().engine.eval::<String>(&format!("seed({}); `${{distribute_points(\"uniform_random\", 3, [[0, 1], [0, 1]])}}`", seed)).unwrap();
        assert_eq!(again(1), again(1));
        assert_ne!(again(1), again(2));
    }

    #[test]
    fn bad_arguments_are_errors() {
        let e = engine();
        for (script, want) in [
            (r#"distribute_points("lumpy", 5, [[0, 1], [0, 1]])"#, "unknown distribution 'lumpy'; distributions: uniform_random"),
            (r#"distribute_points("grid", 5, [[0, 1]])"#, "2, 3 or 4 pairs"),
            (r#"distribute_points("grid", 5, [[0, 1], [1, 1], [0, 1]])"#, "axis 1's bounds are [1, 1]"),
            (r#"distribute_points("grid", -1, [[0, 1], [0, 1]])"#, "count should be from 0"),
            (r#"[vec2(0, 0), vec2(1, 1)].spanning_tree(#{ algorithm: "tallest", metric: "euclidean" })"#, "unknown algorithm 'tallest'"),
            (r#"[vec2(0, 0), vec2(1, 1)].spanning_tree(#{ algorithm: "prim", metric: "hamiltonian" })"#, "unknown distance metric 'hamiltonian'"),
            (r#"[vec2(0, 0), vec3(1, 1, 1)].spanning_tree(#{ algorithm: "prim", metric: "euclidean" })"#, "the same kind of vec"),
            (r#"[vec2(0, 0), 4].spanning_tree(#{ algorithm: "prim", metric: "euclidean" })"#, "points should be vec2s, vec3s or vec4s"),
            (r#"[vec2(0, 0), vec2(1, 1)].spanning_tree(#{ algorithm: "prim", metric: "euclidean", root: 2 })"#, "'root' should be the index of a point, from 0 to 1"),
            (r#"[vec2(0, 0)].spanning_tree(#{ algorithm: "prim", metric: "euclidean", leaves: 2 })"#, "unknown option 'leaves'"),
            ("balanced_ternary([1, 2])", "digits should be -1, 0 or 1"),
            ("balanced_ternary(\"+x\")", "got 'x'"),
            ("balanced_ternary(\"\")", "needs at least one digit"),
            ("balanced_ternary(5).digits(0)", "the width should be at least 1"),
            ("nearest([], vec2(0, 0))", "there are no points"),
            ("new_edges([vec2(0, 0)], [[0, 1]])", "each pair should be [a, b]"),
        ] {
            let err = e.engine.eval::<rhai::Dynamic>(script).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }
}
