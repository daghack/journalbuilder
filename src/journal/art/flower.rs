//! Flowers modelled in 3D and drawn in outline (see `solid`).
//!
//! Petals and sepals are curved surfaces around a receptacle: each leaves
//! it at an angle, curls along its length and cups across it. How far, and
//! how big the petals have grown, follows the flower's openness: from a
//! closed bud wrapped in its sepals, through a half-open cup, to a full
//! bloom with its sepals folded back behind. Seen from any angle, its lines
//! are the petals' and sepals' edges, the folds where their surfaces turn
//! away from the viewer, the stamens and the receptacle's rim, each drawn
//! where nothing nearer hides it.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use rand::rngs::StdRng;
use rand::Rng;

use super::geom::V;
use super::solid::{mix, Line, Model, Run, Surface, P3};

/// A flower as drawn: its surfaces on the page, which hide whatever is
/// behind it, its visible lines, and the middle of what it covers and how
/// far it reaches from there.
pub(super) struct Bloom {
    pub triangles: Vec<[V; 3]>,
    pub lines: Vec<Run>,
    pub middle: V,
    pub reach: f64,
}

/// How a flower sits on the page: centred on `center`, `radius` to its
/// petals' tips when fully open, `openness` from 0 (a closed bud) to 1 (a
/// full bloom), and its axis tilted `tilt` radians from facing the viewer
/// towards `facing`, an angle on the page (so `tilt` pi/2 is side on,
/// facing that way).
pub(super) struct Pose {
    pub center: V,
    pub radius: f64,
    pub openness: f64,
    pub tilt: f64,
    pub facing: f64,
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The flower's frame: its centre, its axis (the way it faces) and two
/// directions across it.
struct Frame {
    center: P3,
    axis: P3,
    x: P3,
    y: P3,
}

impl Frame {
    /// Pointing out from the axis at `angle` around it.
    fn radial(&self, angle: f64) -> P3 {
        self.x * angle.cos() + self.y * angle.sin()
    }
}

/// A petal or sepal: where it leaves the receptacle, how it bends, and its
/// half width along it (`width(u)`, at most `widest`).
struct Organ {
    angle: f64,
    base_radius: f64,
    base_height: f64,
    length: f64,
    /// The angle it leaves at, up from flat towards the axis, and how much
    /// more it turns towards the axis by its tip: evenly, with `curl_power`
    /// 1, or more towards the tip with more.
    elevation: f64,
    curl: f64,
    curl_power: f64,
    /// How far across its width it cups, in radians at its widest.
    cup: f64,
    /// How far it twists about its length by its tip.
    twist: f64,
    widest: f64,
    width: Box<dyn Fn(f64) -> f64>,
}

impl Organ {
    fn surface(self, frame: &Frame) -> Surface {
        let radial = frame.radial(self.angle);
        let across = frame.radial(self.angle + FRAC_PI_2);
        let up = frame.axis;
        let base = frame.center + radial * self.base_radius + up * self.base_height;
        // The midline, bending as it goes.
        const STEPS: usize = 48;
        let heading = |s: f64| self.elevation + self.curl * s.powf(self.curl_power);
        let mut midline = vec![base];
        for k in 0..STEPS {
            let theta = heading((k as f64 + 0.5) / STEPS as f64);
            let last = midline[k];
            midline.push(last + (radial * theta.cos() + up * theta.sin()) * (self.length / STEPS as f64));
        }
        let Organ { curl, curl_power, cup, twist, widest, width, elevation, .. } = self;
        let point = move |u: f64, v: f64| {
            let k = (u * STEPS as f64).clamp(0.0, STEPS as f64 - 1e-9);
            let (i, t) = (k as usize, k.fract());
            let at = midline[i].lerp(midline[i + 1], t);
            let theta = elevation + curl * u.powf(curl_power);
            let normal = radial * -theta.sin() + up * theta.cos();
            let (sin, cos) = (twist * u).sin_cos();
            let (side, normal) = (across * cos + normal * sin, normal * cos - across * sin);
            // Cupped across as an arc, so the width doesn't stretch.
            let a = v * width(u);
            let k = cup / widest;
            let (out, rise) = if k.abs() < 1e-9 { (a, 0.0) } else { ((k * a).sin() / k, (1.0 - (k * a).cos()) / k) };
            at + side * out + normal * rise
        };
        Surface { point: Box::new(point), nu: 18, nv: 10, edges: Surface::smooth_edges(54), lines: Vec::new(), folds: true, crease: false }
    }
}

pub(super) fn bloom(pose: &Pose, rng: &mut StdRng) -> Bloom {
    let Pose { center, radius, openness: open, tilt, facing } = *pose;
    // The flower's frame on the page.
    let axis = P3::new(tilt.sin() * facing.cos(), tilt.sin() * facing.sin(), tilt.cos());
    let helper = if axis.z.abs() < 0.9 { P3::new(0.0, 0.0, 1.0) } else { P3::new(1.0, 0.0, 0.0) };
    let x0 = helper.cross(axis).unit();
    let y0 = axis.cross(x0);
    let spin = rng.random_range(0.0..TAU);
    let frame = Frame {
        center: P3::new(center.x, center.y, 0.0),
        axis,
        x: x0 * spin.cos() + y0 * spin.sin(),
        y: y0 * spin.cos() - x0 * spin.sin(),
    };

    let mut surfaces = Vec::new();
    // Sepals open first, as the bud swells.
    let sepals_open = smooth(open * 1.6);
    let start = rng.random_range(0.0..TAU);
    for k in 0..5 {
        // Closed, they leave the base low and arch evenly over the top into
        // a ball, broadest round its middle and cupped to its curve; open,
        // they're narrower, pointed, and folded back.
        let length = radius * mix(0.72, 0.44, sepals_open) * rng.random_range(0.92..1.08);
        let widest = 0.36 * length;
        let ovate = move |u: f64| (PI * u).sin().max(0.0).powf(0.75) * (1.0 - 0.3 * u);
        let lanceolate = move |u: f64| (1.0 - u).powf(0.8) * (0.8 + 0.2 * (PI * u).sin());
        surfaces.push(Organ {
            angle: start + (k as f64 + 0.5) * TAU / 5.0 + rng.random_range(-0.1..0.1),
            base_radius: radius * mix(0.16, 0.1, sepals_open),
            base_height: -0.03 * radius,
            length,
            elevation: mix(0.55, -0.75, sepals_open) + rng.random_range(-0.08..0.08),
            curl: mix(2.35, -0.3, sepals_open),
            curl_power: mix(1.0, 1.3, sepals_open),
            cup: mix(0.9, 0.35, sepals_open),
            twist: rng.random_range(-0.15..0.15),
            widest,
            width: Box::new(move |u: f64| widest * mix(ovate(u), lanceolate(u), sepals_open)),
        }.surface(&frame));
    }
    // Petals grow as they open; each overlaps the next a little outside it.
    let grown = smooth(open);
    let mut order: Vec<usize> = (0..5).collect();
    for k in (1..5).rev() {
        order.swap(k, rng.random_range(0..=k));
    }
    for (k, &rank) in order.iter().enumerate() {
        let length = radius * mix(0.3, 1.0, grown) * rng.random_range(0.92..1.06);
        let widest = length * rng.random_range(0.44..0.52);
        let (wave, frequency, phase) = (rng.random_range(0.02..0.06), rng.random_range(2.0..4.0), rng.random_range(0.0..TAU));
        surfaces.push(Organ {
            angle: start + k as f64 * TAU / 5.0 + rng.random_range(-0.12..0.12),
            base_radius: radius * (0.06 + 0.03 * open + 0.008 * rank as f64),
            base_height: -0.02 * radius * rank as f64,
            length,
            elevation: mix(0.95, 0.1, open.powf(0.9)) - 0.06 * rank as f64 + rng.random_range(-0.06..0.06),
            curl: mix(2.2, -0.1, open) + rng.random_range(-0.14..0.14),
            curl_power: mix(1.0, 1.3, open),
            cup: mix(1.6, 0.45, open),
            twist: rng.random_range(-0.2..0.2),
            widest,
            width: Box::new(move |u: f64| {
                let shape = if u < 0.62 {
                    0.22 + 0.78 * (FRAC_PI_2 * u / 0.62).sin()
                } else {
                    (1.0 - ((u - 0.62) / 0.38).powi(2)).max(0.0).sqrt()
                };
                widest * shape * (1.0 + wave * u * (frequency * PI * u + phase).sin())
            }),
        }.surface(&frame));
    }
    // The cup under the flower that its stalk runs into, and the
    // receptacle in its middle.
    let cup_frame = (frame.center, frame.axis, frame.x, frame.y);
    surfaces.push(Surface {
        point: Box::new(move |u: f64, v: f64| {
            let (c, a, x, y) = cup_frame;
            let angle = PI * v;
            c + (x * angle.cos() + y * angle.sin()) * (radius * mix(0.03, 0.1, u)) + a * (radius * mix(-0.09, -0.03, u))
        }),
        nu: 4,
        nv: 16,
        edges: None,
        lines: Vec::new(),
        folds: true,
        crease: false,
    });
    surfaces.push(Surface {
        point: Box::new(move |u: f64, v: f64| {
            let (c, a, x, y) = cup_frame;
            let angle = PI * v;
            c + (x * angle.cos() + y * angle.sin()) * (radius * 0.1 * u) + a * (radius * 0.02)
        }),
        nu: 2,
        nv: 16,
        edges: None,
        lines: Vec::new(),
        // Flat: its rim, drawn once it's open, is its only line.
        folds: false,
        crease: false,
    });
    let receptacle = surfaces.len() - 1;

    let model = Model::new(surfaces, radius);
    let mut lines = Vec::new();
    model.draw(&mut lines);
    // The receptacle's rim, and stamens rising from it, once it's open.
    if open > 0.25 {
        let rim: Line = (0..=32).map(|k| (model.on(receptacle, 1.0, -1.0 + 2.0 * k as f64 / 32.0), false)).collect();
        model.visible(&rim, &mut lines);
        let count = rng.random_range(22..32);
        for k in 0..count {
            let angle = TAU * k as f64 / count as f64 + rng.random_range(-0.08..0.08);
            let rise = mix(1.2, 0.5, open) + rng.random_range(-0.15..0.15);
            let length = radius * mix(0.2, 0.34, open) * rng.random_range(0.85..1.15);
            let base = frame.center + frame.radial(angle) * (0.1 * radius) + frame.axis * (0.02 * radius);
            let out = frame.radial(angle);
            let stamen: Line = (0..=4).map(|j| {
                let t = j as f64 / 4.0;
                let theta = rise - 0.3 * t;
                ((base + (out * theta.cos() + frame.axis * theta.sin()) * (length * t), None), false)
            }).collect();
            let tip = stamen[stamen.len() - 1].0;
            model.visible(&stamen, &mut lines);
            if !model.hidden(tip) {
                let anther = tip.0.flat();
                lines.push((0..=8).map(|j| (anther + V::angle(TAU * j as f64 / 8.0) * (0.022 * radius), false)).collect());
            }
        }
    }
    let triangles = model.triangles();
    let count = 3.0 * triangles.len() as f64;
    let middle = triangles.iter().flatten().fold(V::default(), |sum, &p| sum + p) * (1.0 / count);
    let reach = triangles.iter().flatten().map(|&p| (p - middle).len()).fold(0.0, f64::max);
    Bloom { triangles, lines, middle, reach }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn draw(openness: f64, tilt: f64, facing: f64) -> Bloom {
        let pose = Pose { center: V::new(0.0, 0.0), radius: 100.0, openness, tilt, facing };
        bloom(&pose, &mut StdRng::seed_from_u64(4))
    }

    /// Anthers: the small closed loops at the stamens' tips.
    fn anthers(bloom: &Bloom) -> usize {
        bloom.lines.iter().filter(|line| line.len() == 9 && (line[0].0 - line[8].0).len() < 1e-9).count()
    }

    #[test]
    fn flowers_open_from_closed_buds() {
        let (bud, open) = (draw(0.0, 0.0, 0.0), draw(1.0, 0.0, 0.0));
        assert!(open.reach > 1.8 * bud.reach, "bud {} vs open {}", bud.reach, open.reach);
        // A bud's sepals hide its stamens; an open flower's face shows them.
        assert_eq!(anthers(&bud), 0);
        assert!(anthers(&open) > 15, "{} anthers", anthers(&open));
        for bloom in [&bud, &open] {
            assert!(!bloom.lines.is_empty());
            assert!(bloom.lines.iter().flatten().map(|p| &p.0).chain(bloom.triangles.iter().flatten()).all(|p| p.is_finite()));
        }
    }

    #[test]
    fn a_flower_seen_side_on_faces_away_from_its_stalk() {
        // Facing +x, side on, a bud or half-open flower reaches out to the
        // right of where it sits, much further than its sepals fold back.
        for openness in [0.0, 0.5] {
            let bloom = draw(openness, FRAC_PI_2, 0.0);
            let xs = bloom.triangles.iter().flatten().map(|p| p.x);
            let (back, out) = xs.fold((0.0, 0.0), |(lo, hi): (f64, f64), x| (lo.min(x), hi.max(x)));
            assert!(out > 1.5 * -back && out > 30.0, "openness {}: reaches {} forward, {} back", openness, out, -back);
        }
    }

    #[test]
    fn the_same_generator_draws_the_same_flower() {
        let lines = |bloom: Bloom| format!("{:?}", bloom.lines);
        assert_eq!(lines(draw(0.6, 0.8, 1.0)), lines(draw(0.6, 0.8, 1.0)));
    }
}
