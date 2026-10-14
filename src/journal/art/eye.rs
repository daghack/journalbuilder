//! Eyes as drawn in a journal: an almond of lids gone over a few times,
//! their lines running on past the corners; an iris ringed and ringed
//! again, washed with colour that pools darkest at its edge, all of it
//! hidden where the lids cover it; and a line down through the iris, overshooting the lids
//! and bowed toward where the eye looks. Messiness sets how many times
//! each line is gone over and how far the goings-over stray.

use std::f64::consts::{PI, TAU};

use parry2d::math::Vec2;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::geom::{spline, V};
use crate::journal::paths::StrokePath;

/// An eye's parts, to draw in different inks.
pub struct Eye {
    /// Lids, iris rings and the line down the iris: the line work, in ink.
    pub lines: StrokePath,
    /// A second outline of the lids, a little outside them, and the inner
    /// edge of the lower lid: in the iris's colour, as if gone round with a
    /// coloured pen.
    pub accent: StrokePath,
    /// The iris's wash, a spiral out from its middle.
    pub wash: StrokePath,
    /// Where the wash pools, darker: rings round the iris's edge.
    pub rim: StrokePath,
    /// The iris's radius, which the wash's brush is sized by.
    pub iris_radius: f32,
}

/// How an eye is drawn, beyond where and how messily.
#[derive(Debug, Clone, Copy)]
pub struct EyeShape {
    /// The eye's tilt: the angle of the line from corner to corner.
    pub angle: f32,
    /// How far the iris looks, from 0 (straight ahead) to 1 (as far as it
    /// goes, its middle near the lids).
    pub gaze: f32,
    /// How open the eye is: its height over its length.
    pub openness: f32,
}

impl Default for EyeShape {
    fn default() -> Self {
        EyeShape { angle: 0.0, gaze: 0.5, openness: 0.32 }
    }
}

/// An eye centred at `center`, `length` from corner to corner, looking
/// toward `gaze_angle` (radians, on the page: 0 is +x). `messiness` runs
/// from 0 (each line drawn once, true) to 1 (lines gone over four or five
/// times, wandering, overshooting). The same seed draws the same eye.
pub fn eye(center: Vec2, length: f32, gaze_angle: f32, messiness: f32, shape: EyeShape, seed: u64) -> Result<Eye, String> {
    if !(length > 0.0 && length.is_finite()) {
        return Err(format!("length must be a positive number, got {}", length));
    }
    if !(0.0..=1.0).contains(&messiness) {
        return Err(format!("messiness runs from 0 to 1, got {}", messiness));
    }
    if !(0.0..=1.0).contains(&shape.gaze) {
        return Err(format!("gaze runs from 0 to 1, got {}", shape.gaze));
    }
    if !(shape.openness > 0.05 && shape.openness <= 1.0) {
        return Err(format!("openness is the eye's height over its length, above 0.05 and at most 1, got {}", shape.openness));
    }
    if !(gaze_angle.is_finite() && shape.angle.is_finite()) {
        return Err("angles must be finite".to_string());
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let drawing = Drawing {
        m: messiness as f64,
        half: length as f64 / 2.0,
        lid: length as f64 * shape.openness as f64 / 2.0,
    };
    // Drawn about the origin along +x, then turned and moved into place.
    let place = Place { center: center.into(), turn: shape.angle as f64 };
    let looking = gaze_angle as f64 - shape.angle as f64;
    let iris_radius = drawing.lid * 1.0 * (1.0 + drawing.wobble(&mut rng, 0.05));
    // Looking all the way, the iris goes as far toward the corner as the
    // lids are still open to most of its height (they close as 1 - t² from
    // the middle), and a little toward the lid.
    let along = (1.0 - 0.8 * iris_radius / drawing.lid).max(0.0).sqrt();
    let room = V::new(drawing.half * along, drawing.lid * 0.3);
    let iris_center = V::new(looking.cos() * room.x, looking.sin() * room.y) * shape.gaze as f64;

    let mut lines = Vec::new();
    for side in [-1.0, 1.0] {
        for _ in 0..drawing.times(&mut rng, 4.0) {
            lines.push(drawing.lid_stroke(&mut rng, side, 1.0, 1.0));
        }
    }
    // The iris is cut off where the lids cover it: its rings clipped to the
    // almond, and the wash and its rim kept a little further in (their
    // colour still bleeds a little past the lids, as watercolour does).
    let almond = Almond { half: drawing.half, lid: drawing.lid };
    for _ in 0..drawing.times(&mut rng, 5.0).max(2) {
        lines.extend(almond.clip(&drawing.ring(&mut rng, iris_center, iris_radius), 0.0));
    }
    let toward = if room.x > 0.0 { (iris_center.x / room.x).clamp(-1.0, 1.0) } else { 0.0 };
    lines.extend(drawing.meridian(&mut rng, iris_center, iris_radius, toward));

    let mut accent = Vec::new();
    for side in [-1.0, 1.0] {
        for _ in 0..drawing.times(&mut rng, 1.5) {
            accent.push(drawing.lid_stroke(&mut rng, side, 1.08, 1.0));
        }
    }
    accent.push(drawing.lid_stroke(&mut rng, 1.0, 0.8, 0.82));

    let wash = almond.clip(&drawing.wash(&mut rng, iris_center, iris_radius), iris_radius * 0.15);
    // Watercolour dries darkest at its edge: rings of it round the iris.
    let rim: Vec<Vec<V>> = (0..drawing.times(&mut rng, 2.0) + 1)
        .map(|_| {
            let radius = iris_radius * rng.random_range(0.8..0.88);
            almond.clip(&drawing.ring(&mut rng, iris_center, radius), iris_radius * 0.06)
        })
        .flatten()
        .collect();

    let path = |strokes: Vec<Vec<V>>| strokes.iter().fold(StrokePath::new(), |path, points| spline(path, &place.all(points), false));
    Ok(Eye {
        lines: path(lines),
        accent: path(accent),
        wash: path(wash),
        rim: path(rim),
        iris_radius: iris_radius as f32,
    })
}

/// How far a lid is off the axis `x` along it: an arc of a circle through
/// both corners, `height` off it in the middle, `half` the eye's length
/// from the middle to each corner; past the corners, on along its tangent
/// there (so, below the axis).
fn lid_arc(half: f64, height: f64, x: f64) -> f64 {
    let radius = (half * half + height * height) / (2.0 * height);
    if x.abs() <= half {
        (radius * radius - x * x).sqrt() - (radius - height)
    } else {
        -(x.abs() - half) * half / (radius - height).max(1e-9)
    }
}

/// The eye's opening, between true lids, about the origin along +x.
struct Almond {
    half: f64,
    lid: f64,
}

impl Almond {
    /// How far `p` is inside the opening, across the axis: negative outside.
    fn margin(&self, p: V) -> f64 {
        lid_arc(self.half, self.lid, p.x) - p.y.abs()
    }
    /// The runs of `points` at least `inset` inside the opening, each cut
    /// where it crosses out (found by bisection between the points either
    /// side): a line hidden where the lids cover it.
    fn clip(&self, points: &[V], inset: f64) -> Vec<Vec<V>> {
        let inside = |p: V| self.margin(p) >= inset;
        let crossing = |mut a: V, mut b: V| {
            // `a` inside, `b` out.
            for _ in 0..20 {
                let mid = a.lerp(b, 0.5);
                if inside(mid) { a = mid } else { b = mid }
            }
            a
        };
        let mut runs = Vec::new();
        let mut run: Vec<V> = Vec::new();
        for (i, &p) in points.iter().enumerate() {
            match (inside(p), i.checked_sub(1).map(|j| points[j])) {
                (true, Some(prev)) if !inside(prev) => run.push(crossing(p, prev)),
                (false, Some(prev)) if inside(prev) => {
                    run.push(crossing(prev, p));
                    runs.push(std::mem::take(&mut run));
                }
                _ => {}
            }
            if inside(p) {
                run.push(p);
            }
        }
        runs.push(run);
        runs.retain(|run| run.len() >= 2);
        runs
    }
}

/// Where an eye drawn about the origin goes.
struct Place {
    center: V,
    turn: f64,
}

impl Place {
    fn all(&self, points: &[V]) -> Vec<V> {
        let (along, across) = (V::angle(self.turn), V::angle(self.turn).perp());
        points.iter().map(|p| self.center + along * p.x + across * p.y).collect()
    }
}

/// The eye's measures, and how messily it's drawn.
struct Drawing {
    m: f64,
    /// Half its length, centre to corner.
    half: f64,
    /// Half its height, centre to lid.
    lid: f64,
}

impl Drawing {
    /// A random amount up to `scale` either way, times the messiness.
    fn wobble(&self, rng: &mut StdRng, scale: f64) -> f64 {
        rng.random_range(-1.0..=1.0) * scale * self.m
    }
    /// How many times a line is gone over: once when neat, up to `1 + most`.
    fn times(&self, rng: &mut StdRng, most: f64) -> usize {
        1 + (self.m * most + rng.random_range(0.0..1.0) * self.m).floor() as usize
    }
    /// A slow wander along a stroke, `amplitude` either way at most: a few
    /// low harmonics with random phases, a function of 0..1 along it.
    fn wander(&self, rng: &mut StdRng, amplitude: f64) -> impl Fn(f64) -> f64 + use<> {
        let harmonics: Vec<(f64, f64, f64)> = (1..=3)
            .map(|k| (k as f64, rng.random_range(0.0..TAU), rng.random_range(0.3..1.0) * amplitude * self.m / k as f64))
            .collect();
        move |t| harmonics.iter().map(|(k, phase, a)| a * (k * PI * t + phase).sin()).sum()
    }

    /// One lid, the upper (`side` -1) or lower (+1), `bulge` times as far
    /// from the axis as the eye is open and `reach` times as long: an arc
    /// meeting the other lid in a point at each corner and running on past
    /// them, as a hand sweeping through the corner does.
    fn lid_stroke(&self, rng: &mut StdRng, side: f64, bulge: f64, reach: f64) -> Vec<V> {
        let height = self.lid * bulge * (1.0 + self.wobble(rng, 0.18));
        let overshoot = 0.03 + 0.2 * self.m;
        let (t0, t1) = (-reach - rng.random_range(0.0..=overshoot), reach + rng.random_range(0.0..=overshoot));
        let tilt = self.wobble(rng, 0.05);
        let shift = V::new(self.wobble(rng, 0.03 * self.half), self.wobble(rng, 0.12 * self.lid));
        let wander = self.wander(rng, 0.06 * self.lid);
        let half = self.half;
        let arc = move |x: f64| lid_arc(half, height, x);
        let steps = 24;
        (0..=steps)
            .map(|i| {
                let s = i as f64 / steps as f64;
                let t = t0 + (t1 - t0) * s;
                let p = V::new(self.half * t, side * arc(self.half * t) + wander(s));
                V::new(p.x - p.y * tilt, p.y + p.x * tilt) + shift
            })
            .collect()
    }

    /// One ring round the iris: a loop a little lumpy, a little off the
    /// last, drifting in or out so that, gone round a bit more than once,
    /// its ends don't meet.
    fn ring(&self, rng: &mut StdRng, center: V, radius: f64) -> Vec<V> {
        let center = center + V::new(self.wobble(rng, 0.1 * radius), self.wobble(rng, 0.1 * radius));
        let squash = 1.0 + self.wobble(rng, 0.08);
        let drift = self.wobble(rng, 0.07);
        let lumps = self.wander(rng, 0.05);
        let start = rng.random_range(0.0..TAU);
        let sweep = TAU * (1.0 + rng.random_range(0.0..=0.05 + 0.3 * self.m));
        let steps = 40;
        (0..=steps)
            .map(|i| {
                let s = i as f64 / steps as f64;
                let a = start + sweep * s;
                // Lumps repeat each turn, so the loop is the same shape round.
                let r = radius * (1.0 + drift * s + lumps(a / PI));
                center + V::new(a.cos() * r * squash, a.sin() * r / squash)
            })
            .collect()
    }

    /// The line down through the iris, gone over a time or two and
    /// reaching past the lids: curved as a meridian on the eyeball would
    /// be, bowing toward where the eye looks. `toward` is how far it looks
    /// along the eye, from -1 (all the way back) to 1 (all the way on).
    fn meridian(&self, rng: &mut StdRng, center: V, radius: f64, toward: f64) -> Vec<Vec<V>> {
        let lean = self.wobble(rng, 0.12);
        let bow = radius * (0.6 * toward + self.wobble(rng, 0.08));
        (0..self.times(rng, 2.0))
            .map(|_| {
                let turn = PI / 2.0 + lean + self.wobble(rng, 0.05);
                let (down, across) = (V::angle(turn), V::angle(turn - PI / 2.0));
                let reach = |rng: &mut StdRng| radius * rng.random_range(1.15..1.3 + 0.35 * self.m);
                let (up, below) = (reach(rng), reach(rng));
                let at = center + across * self.wobble(rng, 0.08 * radius);
                let bow = bow * (1.0 + self.wobble(rng, 0.25));
                let wander = self.wander(rng, 0.03 * radius);
                let steps = 16;
                (0..=steps)
                    .map(|i| {
                        let s = i as f64 / steps as f64;
                        let along = -up + (up + below) * s;
                        // Bowed most through the middle of the iris.
                        let u = along / up.max(below);
                        at + down * along + across * (bow * (1.0 - u * u) + wander(s))
                    })
                    .collect()
            })
            .collect()
    }

    /// The wash: a spiral out from the middle of the iris to its edge, its
    /// turns as far apart as the wash brush is wide, off the rings' centre
    /// and out toward them as the eye gets messier.
    fn wash(&self, rng: &mut StdRng, center: V, radius: f64) -> Vec<V> {
        let center = center + V::new(self.wobble(rng, 0.12 * radius), self.wobble(rng, 0.12 * radius));
        let outer = radius * (0.85 + rng.random_range(0.0..=0.15) * self.m);
        let turns = 3.0;
        let start = rng.random_range(0.0..TAU);
        let lumps = self.wander(rng, 0.06);
        let steps = 120;
        (0..=steps)
            .map(|i| {
                let s = i as f64 / steps as f64;
                let a = start + TAU * turns * s;
                let r = outer * s.sqrt().max(0.08) * (1.0 + lumps(a / PI));
                center + V::angle(a) * r
            })
            .collect()
    }
}
