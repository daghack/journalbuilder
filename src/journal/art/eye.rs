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
    /// Lids of straight lines, from each corner to a point above and below
    /// the middle, as the `symbols` font's eye has, rather than arcs.
    pub flat: bool,
}

impl Default for EyeShape {
    fn default() -> Self {
        EyeShape { angle: 0.0, gaze: 0.5, openness: 0.32, flat: false }
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
        flat: shape.flat,
    };
    // Drawn about the origin along +x, then turned and moved into place.
    let place = Place { center: center.into(), turn: shape.angle as f64 };
    let looking = gaze_angle as f64 - shape.angle as f64;
    let iris_radius = drawing.lid * 1.0 * (1.0 + drawing.wobble(&mut rng, 0.05));
    // Looking all the way, the iris goes as far toward the corner as the
    // lids are still open to most of its height (they close as 1 - t² from
    // the middle, or 1 - t when straight), and a little toward the lid.
    let open = (1.0 - 0.8 * iris_radius / drawing.lid).max(0.0);
    let along = if shape.flat { open } else { open.sqrt() };
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
    let almond = Almond { half: drawing.half, lid: drawing.lid, flat: shape.flat };
    for _ in 0..drawing.times(&mut rng, 5.0).max(2) {
        let steps = drawing.steps(8, 40);
        lines.extend(almond.clip(&drawing.ring(&mut rng, iris_center, iris_radius, steps), 0.0).into_iter().map(|points| vec![points]));
    }
    let toward = if room.x > 0.0 { (iris_center.x / room.x).clamp(-1.0, 1.0) } else { 0.0 };
    lines.extend(drawing.meridian(&mut rng, iris_center, iris_radius, toward).into_iter().map(|points| vec![points]));

    let mut accent = Vec::new();
    for side in [-1.0, 1.0] {
        for _ in 0..drawing.times(&mut rng, 1.5) {
            accent.push(drawing.lid_stroke(&mut rng, side, 1.08, 1.0));
        }
    }
    accent.push(drawing.lid_stroke(&mut rng, 1.0, 0.8, 0.82));

    let wash = almond.clip(&drawing.wash(&mut rng, iris_center, iris_radius), iris_radius * 0.15).into_iter().map(|points| vec![points]).collect();
    // Watercolour dries darkest at its edge: rings of it round the iris.
    let rim: Vec<Stroke> = (0..drawing.times(&mut rng, 2.0) + 1)
        .map(|_| {
            let radius = iris_radius * rng.random_range(0.8..0.88);
            almond.clip(&drawing.ring(&mut rng, iris_center, radius, 40), iris_radius * 0.06)
        })
        .flatten()
        .map(|points| vec![points])
        .collect();

    // Each stroke's pieces drawn on from one another, turning sharply
    // where they meet.
    let path = |strokes: Vec<Stroke>| strokes.iter().fold(StrokePath::new(), |path, pieces| {
        pieces.iter().enumerate().fold(path, |path, (i, points)| spline(path, &place.all(points), i > 0))
    });
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

/// How far a straight lid is off the axis `x` along it: `height` in the
/// middle, falling straight to the corners `half` either side, and on
/// past them.
fn lid_line(half: f64, height: f64, x: f64) -> f64 {
    height * (1.0 - x.abs() / half)
}

/// A line drawn in pieces, each going on from the last's end with a corner.
type Stroke = Vec<Vec<V>>;

/// The eye's opening, between true lids, about the origin along +x.
struct Almond {
    half: f64,
    lid: f64,
    flat: bool,
}

impl Almond {
    /// How far `p` is inside the opening, across the axis: negative outside.
    fn margin(&self, p: V) -> f64 {
        let lid = if self.flat { lid_line(self.half, self.lid, p.x) } else { lid_arc(self.half, self.lid, p.x) };
        lid - p.y.abs()
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
    /// Straight lids, meeting in a point above and below the middle.
    flat: bool,
}

impl Drawing {
    /// A random amount up to `scale` either way, times the messiness.
    fn wobble(&self, rng: &mut StdRng, scale: f64) -> f64 {
        rng.random_range(-1.0..=1.0) * scale * self.m
    }
    /// How many points a line is drawn through: as few as its shape needs
    /// when neat (`least`), up to `most` as it gets messy and wanders, so
    /// a neat line is a few curves, each moved whole when jittered.
    fn steps(&self, least: usize, most: usize) -> usize {
        least + ((most - least) as f64 * self.m).round() as usize
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
    /// them, as a hand sweeping through the corner does. Straight lids
    /// turn sharply in the middle: two pieces, meeting there.
    fn lid_stroke(&self, rng: &mut StdRng, side: f64, bulge: f64, reach: f64) -> Stroke {
        let height = self.lid * bulge * (1.0 + self.wobble(rng, 0.18));
        let overshoot = 0.03 + 0.2 * self.m;
        let (t0, t1) = (-reach - rng.random_range(0.0..=overshoot), reach + rng.random_range(0.0..=overshoot));
        let tilt = self.wobble(rng, 0.05);
        let shift = V::new(self.wobble(rng, 0.03 * self.half), self.wobble(rng, 0.12 * self.lid));
        let wander = self.wander(rng, 0.06 * self.lid);
        let (half, flat) = (self.half, self.flat);
        let lid = move |x: f64| if flat { lid_line(half, height, x) } else { lid_arc(half, height, x) };
        let point = |t: f64| {
            let p = V::new(self.half * t, side * lid(self.half * t) + wander((t - t0) / (t1 - t0)));
            V::new(p.x - p.y * tilt, p.y + p.x * tilt) + shift
        };
        let piece = |from: f64, to: f64, steps: usize| (0..=steps).map(|i| point(from + (to - from) * i as f64 / steps as f64)).collect();
        if flat {
            vec![piece(t0, 0.0, self.steps(1, 12)), piece(0.0, t1, self.steps(1, 12))]
        } else {
            vec![piece(t0, t1, self.steps(4, 24))]
        }
    }

    /// One ring round the iris through `steps` + 1 points: a loop a little
    /// lumpy, a little off the last, drifting in or out so that, gone round
    /// a bit more than once, its ends don't meet.
    fn ring(&self, rng: &mut StdRng, center: V, radius: f64, steps: usize) -> Vec<V> {
        let center = center + V::new(self.wobble(rng, 0.1 * radius), self.wobble(rng, 0.1 * radius));
        let squash = 1.0 + self.wobble(rng, 0.08);
        let drift = self.wobble(rng, 0.07);
        let lumps = self.wander(rng, 0.05);
        let start = rng.random_range(0.0..TAU);
        let sweep = TAU * (1.0 + rng.random_range(0.0..=0.05 + 0.3 * self.m));
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
                let steps = self.steps(2, 16);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// How far the line work strays from the axis from 40 to 60 out, of the
/// 100 to the corners.
    fn lid_height_half_way(flat: bool) -> f32 {
        let shape = EyeShape { gaze: 0.0, flat, ..EyeShape::default() };
        let e = eye(Vec2::new(0.0, 0.0), 200.0, 0.0, 0.0, shape, 1).expect("an eye");
        e.lines.flattened(0.1).into_iter().flat_map(|(points, _)| points)
            .filter(|p| (40.0..=60.0).contains(&p.x.abs())).map(|p| p.y.abs()).fold(0.0, f32::max)
    }

    #[test]
    fn flat_lids_run_straight_from_the_corners() {
        // 32 from the axis in the middle: straight lids are 19.2 off it
        // at most from 40 out, arcs 27.3.
        let (flat, arc) = (lid_height_half_way(true), lid_height_half_way(false));
        assert!(flat < 19.5, "{}", flat);
        assert!(arc > 26.0, "{}", arc);
    }

    #[test]
    fn neat_lines_are_few_curves() {
        // Neat, a flat eye's lids are two straight pieces each: their
        // corners are the only points a jitter would move.
        let shape = EyeShape { gaze: 0.0, flat: true, ..EyeShape::default() };
        let neat = eye(Vec2::new(0.0, 0.0), 200.0, 0.0, 0.0, shape, 1).expect("an eye");
        let messy = eye(Vec2::new(0.0, 0.0), 200.0, 0.0, 1.0, shape, 1).expect("an eye");
        let count = |e: &Eye| e.lines.commands().len();
        assert!(count(&neat) < 40, "{}", count(&neat));
        assert!(count(&messy) > 4 * count(&neat), "{} vs {}", count(&messy), count(&neat));
    }
}
