//! A many-headed serpent along a path, such as a maze's: its body follows
//! the path's lines (see `trail`), slithering a little off them and forking
//! where the path forks, with its tail at the end it's followed from and a
//! head at every other dead end.
//!
//! Each chain of the path is a stretch of body: the first tapers from the
//! tail, and each that forks off leaves its parent a little thinner, as a
//! neck. A chain ending at a dead end narrows to a neck and ends in a pit
//! viper's head, kept straight; one that ends where it meets the body
//! already drawn tapers away. Where a stretch forks off, the two outlines
//! merge; stretches that cross go over or under.

use std::f64::consts::{PI, TAU};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::geom::V;
use super::scene::Scene;
use super::snake::{head_half_width, head_length, tail_half_width};
use super::trail;
use crate::journal::paths::StrokePath;

/// A stretch of body as drawn: its spine and half widths, and its shape.
struct Stretch {
    points: Vec<V>,
    halves: Vec<f64>,
    /// Its own half width, between its ends.
    body: f64,
    shape: usize,
}

impl Stretch {
    /// The point on its spine nearest `q`, and its half width there.
    fn nearest(&self, q: V) -> (V, f64) {
        let i = (0..self.points.len()).min_by(|&a, &b| (self.points[a] - q).len().total_cmp(&(self.points[b] - q).len())).expect("a stretch has points");
        (self.points[i], self.halves[i])
    }
}

/// The outline of a many-headed serpent `max_width` across at its widest,
/// along `path`. `seed` picks how it slithers.
pub fn snake_along(path: &StrokePath, max_width: f32, seed: u64) -> Result<StrokePath, String> {
    let width = max_width as f64;
    if !(width > 0.0) || !width.is_finite() {
        return Err(format!("max_width must be a positive number, got {}", max_width));
    }
    if !path.points_within().iter().all(|p| p.is_finite()) {
        return Err("the path has a non-finite (infinite or NaN) coordinate".into());
    }
    let chains = trail::chains(path, width / 4.0, width / 8.0);
    if chains.is_empty() {
        return Err("the path has no lines to follow".into());
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let mut scene: Scene<()> = Scene::new(0.004 * width, 0.5 * width, 0.08 * width);
    let step = width / 10.0;
    let mut stretches: Vec<Stretch> = Vec::new();
    for chain in &chains {
        let mut points = trail::wander(&chain.points, step, 0.6 * width, 0.35 * width, 7.0 * width, &mut rng);
        // Where it forks from, and how wide it is there; a fork's body is a
        // little thinner than its parent's.
        let start = chain.parent.map(|(parent, _)| {
            let parent = &stretches[parent];
            let (attach, half) = parent.nearest(chain.points[0]);
            (parent.shape, attach, half, (0.85 * parent.body).max(0.3 * width))
        });
        if let Some((_, attach, _, _)) = start {
            // Start on the parent's spine, easing onto this stretch's line.
            let blend = width;
            let mut s = 0.0;
            let shift = attach - points[0];
            for k in 0..points.len() {
                if k > 0 {
                    s += (points[k] - points[k - 1]).len();
                }
                let t = (1.0 - s / blend).max(0.0);
                points[k] = points[k] + shift * (t * t);
            }
        }
        let points = trail::resample(&points, step);
        let n = points.len();
        let length = step * (n - 1) as f64;
        let body = start.map_or(0.5 * width, |(_, _, _, body)| body);
        // Heads a little broader than the body, to read as heads.
        let head = if chain.dead_end { 2.5 * body } else { 0.0 };
        let points = if chain.dead_end { straighten(&points, step, head_length(head)) } else { points };
        let tail = (6.0 * width).min(0.4 * length);
        let halves: Vec<f64> = (0..n).map(|i| {
            let (from_start, to_end) = (i as f64 * step, length - i as f64 * step);
            let start_half = match start {
                None => tail_half_width(from_start, tail, body),
                Some((_, _, half, _)) => body + (0.95 * half - body) * (1.0 - (from_start / width).min(1.0)).powi(2),
            };
            let end_half = if chain.dead_end {
                head_half_width(to_end, head, body)
            } else {
                body * (to_end / (2.0 * width)).min(1.0).sqrt()
            };
            start_half.min(end_half)
        }).collect();
        let (outline, corners) = outline(&points, &halves);
        let depth = if start.is_none() { 0.5 } else { rng.random_range(0.0..1.0) };
        let shape = scene.add((), depth, outline, corners);
        if let Some((parent, attach, _, _)) = start {
            scene.join(parent, shape, attach, 3.0 * width);
        }
        stretches.push(Stretch { points, halves, body, shape });
    }
    Ok(scene.path(()))
}

/// `points`, `step` apart, with the last `straight` of them in a straight
/// line, as a head must be.
fn straighten(points: &[V], step: f64, straight: f64) -> Vec<V> {
    let n = points.len();
    let k = n.saturating_sub(1 + (straight / step).ceil() as usize).max(1);
    let (before, at) = (points[k - 1], points[k]);
    let along = (at - before) * (1.0 / (at - before).len().max(1e-12));
    let mut out = points[..=k].to_vec();
    out.extend((1..n - k).map(|j| at + along * (j as f64 * step)));
    out
}

/// The outline of a stretch of body with these half widths along its
/// spine: up one side and back down the other, meeting where a half width
/// is 0 (at a tail's or snout's tip), and cut square across where it isn't
/// (where it forks from its parent).
fn outline(points: &[V], halves: &[f64]) -> (Vec<V>, Vec<bool>) {
    let n = points.len();
    let mut headings: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        let d = points[(i + 1).min(n - 1)] - points[i.saturating_sub(1)];
        let raw = d.y.atan2(d.x);
        headings.push(match headings.last() {
            Some(&last) => last + (raw - last + PI).rem_euclid(TAU) - PI,
            None => raw,
        });
    }
    let left = |i: usize| V::angle(headings[i]).perp() * halves[i];
    let tip = |i: usize| halves[i] < 1e-9;
    let mut outline: Vec<V> = Vec::new();
    let mut corners: Vec<bool> = Vec::new();
    for (i, &point) in points.iter().enumerate() {
        outline.push(point + left(i));
        corners.push(i == 0 && !tip(0));
    }
    for i in (0..n).rev() {
        if (i == n - 1 && tip(n - 1)) || (i == 0 && tip(0)) {
            continue;
        }
        outline.push(points[i] - left(i));
        corners.push(i == 0);
    }
    (outline, corners)
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
    fn a_forked_path_grows_a_serpent_with_a_head_on_each_branch() {
        // A T from the left: a tail at the left end, heads at the right end
        // and the bottom.
        let tee = path(&[&[(0.0, 0.0), (400.0, 0.0)], &[(200.0, 0.0), (200.0, 300.0)]]);
        let outline = snake_along(&tee, 30.0, 1).expect("the serpent should fit");
        let points = outline.points_within();
        assert!(points.iter().all(|p| p.is_finite()));
        // Near both head ends, and near the tail's tip.
        for end in [(400.0, 0.0), (200.0, 300.0), (0.0, 0.0)] {
            let near = points.iter().map(|p| ((p.x - end.0).powi(2) + (p.y - end.1).powi(2)).sqrt()).fold(f32::INFINITY, f32::min);
            assert!(near < 30.0, "nothing drawn near {:?}: {}", end, near);
        }
        // The body, one shape per stretch, forks into three strokes' worth
        // of outline at least.
        assert!(outline.to_string().matches('M').count() >= 2, "{}", outline);
    }

    #[test]
    fn the_tail_is_where_the_path_starts_and_heads_end_the_rest() {
        let tee = path(&[&[(0.0, 0.0), (400.0, 0.0)], &[(200.0, 0.0), (200.0, 300.0)]]);
        let chains = trail::chains(&tee, 30.0 / 4.0, 30.0 / 8.0);
        assert!((chains[0].points[0] - V::new(0.0, 0.0)).len() < 1e-6, "the first stretch starts at the path's start");
        assert!(chains.iter().all(|c| c.dead_end), "both other ends are dead ends, so heads");
    }

    #[test]
    fn bad_arguments_are_errors() {
        let line = path(&[&[(0.0, 0.0), (100.0, 0.0)]]);
        assert!(snake_along(&line, 0.0, 1).unwrap_err().contains("max_width"));
        assert!(snake_along(&StrokePath::new(), 10.0, 1).unwrap_err().contains("no lines"));
    }
}
