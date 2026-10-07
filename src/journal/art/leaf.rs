//! Compound leaves modelled in 3D and drawn in outline (see `solid`).
//!
//! Each leaflet is a surface that folds up a little either side of its
//! midrib, droops and swings along its length, twists, and ripples at its
//! edge, with its midrib turned towards or away from the viewer and its
//! blade rolled about it. Its margin is cut into teeth that point towards
//! its tip (every other one smaller), kept sharp when drawn, and its midrib
//! and veins are drawn on its surface. The leaflets of a leaf hide each
//! other where they overlap.

use std::f64::consts::{PI, TAU};

use rand::rngs::StdRng;
use rand::Rng;

use super::geom::V;
use super::solid::{Model, Run, Surface, P3};

/// Where a leaflet grows from on the page, which way, and how big.
pub(super) struct Placement {
    pub base: V,
    pub heading: f64,
    pub length: f64,
    pub width: f64,
}

/// A leaf as drawn: its leaflets' surfaces on the page, its visible lines,
/// and the room each leaflet takes up, as a disc.
pub(super) struct Drawn {
    pub triangles: Vec<[V; 3]>,
    pub lines: Vec<Run>,
    pub room: Vec<(V, f64)>,
}

pub(super) fn leaf(leaflets: &[Placement], rng: &mut StdRng) -> Drawn {
    let scale = leaflets.iter().map(|l| l.length).fold(0.0, f64::max);
    let (surfaces, room): (Vec<Surface>, Vec<(V, f64)>) = leaflets.iter().map(|placement| leaflet(placement, rng)).unzip();
    let model = Model::new(surfaces, scale);
    let mut lines = Vec::new();
    model.draw(&mut lines);
    Drawn { triangles: model.triangles(), lines, room }
}

/// The half width `u` along a leaflet (from its base to its tip), teeth and
/// all, and where to draw its sides through to keep the teeth sharp: u
/// values, and whether each is a corner.
fn margin(length: f64, width: f64, teeth: usize) -> (impl Fn(f64) -> f64 + use<>, Vec<(f64, bool)>) {
    // Rounded at the base, widest a little below the middle, drawn out to
    // a point.
    let (a, b): (f64, f64) = (0.55, 1.15);
    let peak = (a / (a + b)).powf(a) * (b / (a + b)).powf(b);
    let shape = move |u: f64| width / 2.0 * u.max(0.0).powf(a) * (1.0 - u).max(0.0).powf(b) / peak;
    let (from, to) = (0.1, 0.97);
    let step = (to - from) / teeth as f64;
    let tooth = 0.03 * length;
    let half = move |u: f64| {
        if !(from..to).contains(&u) {
            return shape(u);
        }
        // Rising gently to a tip, then dropping to the next notch; fading
        // where the leaflet narrows.
        let k = ((u - from) / step) as usize;
        let t = (u - from) / step - k as f64;
        let fade = (shape(from + (k as f64 + 0.8) * step) / (0.2 * width)).min(1.0);
        let depth = tooth * fade * if k.is_multiple_of(2) { 1.0 } else { 0.6 };
        let offset = if t < 0.8 { -0.4 + t / 0.8 } else { 0.6 - (t - 0.8) / 0.2 };
        (shape(u) + depth * offset).max(0.0)
    };
    let mut samples = vec![(0.0, true)];
    samples.extend((1..4).map(|k| (from * k as f64 / 4.0, false)));
    for k in 0..teeth {
        let u0 = from + k as f64 * step;
        samples.extend([(u0, true), (u0 + 0.45 * step, false), (u0 + 0.8 * step, true)]);
    }
    samples.extend([(to, true), ((to + 1.0) / 2.0, false), (1.0, true)]);
    (half, samples)
}

/// One leaflet's surface, and the room it takes up on the page.
fn leaflet(placement: &Placement, rng: &mut StdRng) -> (Surface, (V, f64)) {
    let Placement { base, heading, length, width } = *placement;
    // The midrib turned towards or away from the viewer, and the blade
    // rolled about it, more often a little than a lot.
    let elevation: f64 = rng.random_range(-0.45..0.45);
    let along = P3::new(heading.cos() * elevation.cos(), heading.sin() * elevation.cos(), elevation.sin());
    let flat_across = P3::new(-heading.sin(), heading.cos(), 0.0);
    let flat_normal = along.cross(flat_across);
    let roll = rng.random_range(-1.0..1.0f64).powi(3) * 1.1;
    let across = flat_across * roll.cos() + flat_normal * roll.sin();
    let normal = flat_normal * roll.cos() - flat_across * roll.sin();
    // How far each half rises from flat, how far the tip droops away from
    // the face and swings aside (in lengths), how far it twists, and the
    // ripple along its edge.
    let fold: f64 = rng.random_range(0.12..0.45);
    let droop = rng.random_range(0.04..0.2);
    let swing = rng.random_range(-0.08..0.08);
    let twist: f64 = rng.random_range(-0.35..0.35);
    let (ripple, ripples, phase) = (rng.random_range(0.0..0.025) * length, rng.random_range(3.0..7.0), rng.random_range(0.0..TAU));
    let teeth = rng.random_range(13..19);
    let (half, edges) = margin(length, width, teeth);
    let start = P3::on_page(base);
    let point = move |u: f64, v: f64| {
        let (sin, cos) = (twist * u).sin_cos();
        let (side, up) = (across * cos + normal * sin, normal * cos - across * sin);
        let midrib = start + along * (u * length) - up * (droop * length * u * u) + side * (swing * length * u * u);
        let out = v.abs() * half(u);
        midrib + side * (v.signum() * out * fold.cos()) + up * (out * fold.sin() + ripple * v * v * (ripples * PI * u + phase).sin())
    };

    // The midrib, and veins from it towards the teeth.
    let mut lines = vec![(0..=12).map(|k| (0.93 * k as f64 / 12.0, 0.0)).collect::<Vec<_>>()];
    let count = teeth / 2;
    for k in 0..count {
        let u = 0.08 + 0.72 * k as f64 / count as f64;
        for sign in [1.0, -1.0] {
            let u = u + 0.02 * (sign + 1.0);
            // Leaving the midrib angled forward, bending further forward
            // towards the margin.
            lines.push((0..=5).map(|j| {
                let t = j as f64 / 5.0;
                (u + 0.16 * t + 0.05 * t * t, sign * 0.78 * t)
            }).collect());
        }
    }

    // The room it takes: about its middle, out to its tip, base and sides.
    let middle = point(0.5, 0.0).flat();
    let reach = [(0.0, 0.0), (1.0, 0.0), (0.3, 1.0), (0.3, -1.0), (0.6, 1.0), (0.6, -1.0)].iter()
        .map(|&(u, v)| (point(u, v).flat() - middle).len())
        .fold(0.0, f64::max);
    let surface = Surface { point: Box::new(point), nu: 48, nv: 8, edges: Some(edges), lines, folds: true, crease: true };
    (surface, (middle, 0.9 * reach))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn teeth_point_towards_the_tip() {
        let (half, edges) = margin(100.0, 50.0, 15);
        // Each tooth's tip stands out past the line between the notches
        // either side of it.
        let corners: Vec<f64> = edges.iter().filter(|&&(u, c)| c && u > 0.0 && u < 0.97).map(|&(u, _)| u).collect();
        for w in corners.windows(3).step_by(2) {
            let (notch, tip, next) = (half(w[0]), half(w[1]), half(w[2]));
            let chord = notch + (next - notch) * (w[1] - w[0]) / (w[2] - w[0]);
            assert!(tip > chord + 0.5, "{:?}: {} {} {}", w, notch, tip, next);
            // The rise is longer than the drop: the tooth leans forward.
            assert!(w[1] - w[0] > 2.0 * (w[2] - w[1]), "{:?}", w);
        }
    }

    #[test]
    fn a_leaf_draws_its_leaflets() {
        let leaflets: Vec<Placement> = (0..3).map(|k| Placement {
            base: V::new(0.0, 0.0),
            heading: -1.0 + k as f64,
            length: 120.0,
            width: 60.0,
        }).collect();
        let drawn = leaf(&leaflets, &mut StdRng::seed_from_u64(2));
        assert_eq!(drawn.room.len(), 3);
        assert!(drawn.lines.len() >= 3, "{} lines", drawn.lines.len());
        assert!(drawn.lines.iter().flatten().all(|(p, _)| p.is_finite()));
        // Teeth are drawn as corners.
        assert!(drawn.lines.iter().flatten().filter(|(_, corner)| *corner).count() > 30);
    }
}
