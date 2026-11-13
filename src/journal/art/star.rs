//! Stars that aren't right: an eye ringed with lashes like spikes, a
//! spiral that draws the eye in, a disc banded dark and light round a
//! pupil, a burst of needles of no two lengths, a dark thing that weeps
//! ink. Each comes as parts, to ink apart: its rays, its body's lines, the
//! shapes to fill dark, strokes to wash in colour, tears running down, and
//! the small rings dotted round it.
//!
//! Nothing about one is even: rays differ in length and lean, rings
//! wobble, by its seed.

use std::f64::consts::{PI, TAU};

use parry2d::math::Vec2;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::eye::{eye, EyeShape};
use super::geom::{spline, spline_closed, V};
use crate::journal::paths::StrokePath;

/// A star's parts, to draw in different inks.
pub struct EerieStar {
    /// Its rays: spikes, lashes or needles, each a stroke or a narrow wedge.
    pub rays: StrokePath,
    /// Its body's lines: lids and iris, spiral, rings or core.
    pub body: StrokePath,
    /// Closed shapes to fill dark, one inside another cutting holes: a
    /// pupil, a core, every other band.
    pub dark: StrokePath,
    /// Strokes to wash in colour: an iris's.
    pub wash: StrokePath,
    /// Tears running down from it, each ending in a drop.
    pub tears: StrokePath,
    /// Small rings dotted round it.
    pub dots: StrokePath,
}

/// Which star.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StarKind {
    /// An eye, its pupil a slit, ringed with lashes like spikes.
    Lashed,
    /// A spiral out from a pupil to a rim dotted round.
    Spiral,
    /// Rings one inside another, every other band dark, round a pupil.
    Banded,
    /// Hair-thin rays of every length from a small dark core, some bent.
    Needle,
    /// A dark, lumpen core with stubby rays, weeping.
    Weeping,
}

impl StarKind {
    pub const NAMES: [&'static str; 5] = ["lashed", "spiral", "banded", "needle", "weeping"];

    pub fn named(name: &str) -> Option<StarKind> {
        Some(match name {
            "lashed" => StarKind::Lashed,
            "spiral" => StarKind::Spiral,
            "banded" => StarKind::Banded,
            "needle" => StarKind::Needle,
            "weeping" => StarKind::Weeping,
            _ => return None,
        })
    }

    /// How many rays, how far the longest reaches (in radii) and how many
    /// tears, unless the settings say.
    fn defaults(self) -> (u32, f32, u32) {
        match self {
            StarKind::Lashed => (26, 1.9, 0),
            StarKind::Spiral => (0, 1.5, 0),
            StarKind::Banded => (9, 1.7, 0),
            StarKind::Needle => (29, 2.6, 0),
            StarKind::Weeping => (11, 1.6, 3),
        }
    }
}

/// How a star is drawn, beyond where and how big.
#[derive(Debug, Clone, Copy)]
pub struct StarSettings {
    pub kind: StarKind,
    /// How many rays; the kind's own number unless given.
    pub rays: Option<u32>,
    /// How far the longest ray reaches from the centre, in radii; the
    /// kind's own unless given.
    pub reach: Option<f32>,
    /// How many tears run down from it; the kind's own unless given.
    pub tears: Option<u32>,
    /// How much its rays differ in length: 0 all alike, 1 some hardly there.
    pub unevenness: f32,
    /// How far it's turned.
    pub angle: f32,
}

impl Default for StarSettings {
    fn default() -> Self {
        StarSettings { kind: StarKind::Needle, rays: None, reach: None, tears: None, unevenness: 0.6, angle: 0.0 }
    }
}

/// The most rays or tears a star can be asked for.
pub const MOST_RAYS: u32 = 400;

/// A star about `center`, its body `radius` in radius, its rays reaching
/// beyond. The same seed draws the same star.
pub fn eerie_star(center: Vec2, radius: f32, settings: StarSettings, seed: u64) -> Result<EerieStar, String> {
    if !(radius > 0.0 && radius.is_finite()) {
        return Err(format!("radius must be a positive number, got {}", radius));
    }
    if !(center.x.is_finite() && center.y.is_finite() && settings.angle.is_finite()) {
        return Err("the centre and angle must be finite".into());
    }
    if !(0.0..=1.0).contains(&settings.unevenness) {
        return Err(format!("unevenness runs from 0 to 1, got {}", settings.unevenness));
    }
    let (rays, reach, tears) = settings.kind.defaults();
    let rays = settings.rays.unwrap_or(rays);
    let tears = settings.tears.unwrap_or(tears);
    let reach = settings.reach.unwrap_or(reach);
    if rays > MOST_RAYS || tears > MOST_RAYS {
        return Err(format!("a star has at most {} rays and tears, got {} and {}", MOST_RAYS, rays, tears));
    }
    if !(reach >= 1.0 && reach.is_finite()) {
        return Err(format!("reach must be at least 1 (the body's own radius), got {}", reach));
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let draw = Draw { c: center.into(), r: radius as f64, reach: reach as f64, uneven: settings.unevenness as f64, turn: settings.angle as f64 };
    let mut star = match settings.kind {
        StarKind::Lashed => draw.lashed(&mut rng, rays, seed)?,
        StarKind::Spiral => draw.spiral(&mut rng, rays),
        StarKind::Banded => draw.banded(&mut rng, rays),
        StarKind::Needle => draw.needle(&mut rng, rays),
        StarKind::Weeping => draw.weeping(&mut rng, rays),
    };
    star.tears = draw.tears(&mut rng, tears);
    Ok(star)
}

struct Draw {
    c: V,
    r: f64,
    reach: f64,
    uneven: f64,
    turn: f64,
}

fn path_of(points: &[V]) -> StrokePath {
    let mut path = StrokePath::new();
    for (i, p) in points.iter().enumerate() {
        path = if i == 0 { path.move_to(p.x as f32, p.y as f32) } else { path.line_to(p.x as f32, p.y as f32) };
    }
    path
}

fn empty() -> EerieStar {
    EerieStar { rays: StrokePath::new(), body: StrokePath::new(), dark: StrokePath::new(), wash: StrokePath::new(), tears: StrokePath::new(), dots: StrokePath::new() }
}

impl Draw {
    /// A point `r` radii from the centre at `angle`, the star's turn added.
    fn at(&self, angle: f64, r: f64) -> V {
        self.c + V::angle(angle + self.turn) * (r * self.r)
    }

    /// How long a ray is, from `least` radii up to `most`: shorter the more
    /// uneven the star, a few of them hardly there.
    fn length(&self, rng: &mut StdRng, least: f64, most: f64) -> f64 {
        let short: f64 = rng.random_range(0.0..1.0);
        least + (most - least) * (1.0 - self.uneven * short.powf(0.7))
    }

    /// A ring `r` radii out, wobbling by up to `wobble` of that, closed.
    fn ring(&self, rng: &mut StdRng, r: f64, wobble: f64) -> StrokePath {
        let phases: [f64; 3] = [rng.random_range(0.0..TAU), rng.random_range(0.0..TAU), rng.random_range(0.0..TAU)];
        let points: Vec<V> = (0..28)
            .map(|i| {
                let a = TAU * i as f64 / 28.0;
                let lump = (2.0 * a + phases[0]).sin() * 0.5 + (3.0 * a + phases[1]).sin() * 0.3 + (5.0 * a + phases[2]).sin() * 0.2;
                self.at(a, r * (1.0 + wobble * lump))
            })
            .collect();
        spline_closed(StrokePath::new(), &points).close_path()
    }

    /// A spike from the body at `angle`, its foot `from` radii out and
    /// `wide` radii across, its tip `to` radii out and leaning `lean`
    /// radians aside: two lines meeting at the tip, bowed a little.
    fn spike(&self, angle: f64, from: f64, to: f64, wide: f64, lean: f64) -> StrokePath {
        let half = (wide / 2.0 / from).atan();
        let tip = self.at(angle + lean, to);
        let mid = (from + to) / 2.0;
        let mut path = StrokePath::new();
        for side in [-1.0, 1.0] {
            let foot = self.at(angle + side * half, from);
            let bow = self.at(angle + lean * 0.3 + side * half * 0.35, mid);
            path = path.move_to(foot.x as f32, foot.y as f32).quadratic_curve_to(bow.x as f32, bow.y as f32, tip.x as f32, tip.y as f32);
        }
        path
    }

    /// An eye across the star, a slit for its pupil, and lashes all round
    /// its lids, longest over the middle of each.
    fn lashed(&self, rng: &mut StdRng, rays: u32, seed: u64) -> Result<EerieStar, String> {
        let mut star = empty();
        let openness = 0.55;
        let shape = EyeShape { angle: self.turn as f32, gaze: 0.0, openness, flat: false };
        let eye = eye(Vec2::new(self.c.x as f32, self.c.y as f32), (2.0 * self.r) as f32, 0.0, 0.2, shape, seed)?;
        star.body = eye.lines;
        star.wash = eye.wash;
        star.wash.append(eye.rim);
        // The pupil: a slit, standing across the eye.
        let iris = eye.iris_radius as f64 / self.r;
        let (top, bottom) = (self.at(-PI / 2.0, iris * 0.82), self.at(PI / 2.0, iris * 0.82));
        let (left, right) = (self.at(PI, iris * 0.24), self.at(0.0, iris * 0.24));
        star.dark = StrokePath::new()
            .move_to(top.x as f32, top.y as f32)
            .quadratic_curve_to(right.x as f32, right.y as f32, bottom.x as f32, bottom.y as f32)
            .quadratic_curve_to(left.x as f32, left.y as f32, top.x as f32, top.y as f32)
            .close_path();
        // Lashes: round both lids, which close as 1 - t² from the middle.
        let lid = openness as f64;
        for i in 0..rays {
            let upper = i % 2 == 0;
            let t = -0.94 + 1.88 * ((i / 2) as f64 + rng.random_range(0.15..0.85)) / (rays as f64 / 2.0).ceil().max(1.0);
            let side = if upper { -1.0 } else { 1.0 };
            let foot = V::new(t, side * lid * (1.0 - t * t));
            // Out along the lid's normal, fanned away from the middle.
            let normal = V::new(2.0 * t * lid, side).lerp(foot, 0.35);
            let angle = normal.y.atan2(normal.x);
            let at = foot.y.atan2(foot.x);
            let long = self.length(rng, 0.12, (self.reach - 1.0).max(0.15)) * (1.0 - 0.45 * t * t);
            let tip = foot + V::angle(angle + rng.random_range(-0.25..0.25)) * long;
            let wide = rng.random_range(0.035..0.07);
            let across = V::angle(at).perp() * (wide / 2.0);
            let bend = (foot + tip) * 0.5 + V::angle(angle).perp() * (long * rng.random_range(-0.18..0.18));
            let place = |p: V| self.at(p.y.atan2(p.x), p.len());
            let (a, b, bend, tip) = (place(foot + across), place(foot - across), place(bend), place(tip));
            star.rays.append(
                StrokePath::new()
                    .move_to(a.x as f32, a.y as f32)
                    .quadratic_curve_to(bend.x as f32, bend.y as f32, tip.x as f32, tip.y as f32)
                    .move_to(b.x as f32, b.y as f32)
                    .quadratic_curve_to(bend.x as f32, bend.y as f32, tip.x as f32, tip.y as f32),
            );
        }
        Ok(star)
    }

    /// A spiral out from a dark pupil to a wobbling rim, small rings
    /// dotted round outside it.
    fn spiral(&self, rng: &mut StdRng, rays: u32) -> EerieStar {
        let mut star = empty();
        let turns = rng.random_range(3.0..3.8);
        let start = rng.random_range(0.0..TAU);
        let way = if rng.random_range(0.0..1.0) < 0.5 { -1.0 } else { 1.0 };
        let steps = 120;
        let wander = rng.random_range(0.0..TAU);
        let points: Vec<V> = (0..=steps)
            .map(|i| {
                let s = i as f64 / steps as f64;
                let a = start + way * TAU * turns * s;
                self.at(a, (0.1 + 0.86 * s) * (1.0 + 0.035 * (a * 1.7 + wander).sin()))
            })
            .collect();
        star.body = spline(StrokePath::new(), &points, false);
        star.body.append(self.ring(rng, 1.0, 0.03));
        star.dark = self.ring(rng, 0.11, 0.08);
        let count = 16 + rng.random_range(0..5);
        for i in 0..count {
            let a = TAU * (i as f64 + rng.random_range(-0.2..0.2)) / count as f64;
            let at = self.at(a, rng.random_range(1.14..1.24));
            star.dots.append(StrokePath::circle(Vec2::new(at.x as f32, at.y as f32), (self.r * rng.random_range(0.035..0.06)) as f32));
        }
        self.spikes(rng, &mut star, rays, 1.3, 0.1);
        star
    }

    /// Rings one inside another round a pupil, every other band dark.
    fn banded(&self, rng: &mut StdRng, rays: u32) -> EerieStar {
        let mut star = empty();
        // An odd number, so the outermost band and the pupil are both dark.
        let rings = 5;
        for k in 1..=rings {
            // The pupil a little bigger than its share.
            let r = if k == 1 { 0.16 } else { k as f64 / rings as f64 };
            let ring = self.ring(rng, r, 0.035);
            star.dark.append(ring.clone());
            star.body.append(ring);
        }
        self.spikes(rng, &mut star, rays, 1.04, 0.16);
        star
    }

    /// Hair-thin rays of every length from a small dark core, the longest
    /// four a cross, some bent part way.
    fn needle(&self, rng: &mut StdRng, rays: u32) -> EerieStar {
        let mut star = empty();
        let core = 0.2;
        star.dark = self.ring(rng, core, 0.1);
        star.body = star.dark.clone();
        let halo = rng.random_range(0.5..0.62);
        star.body.append(self.ring(rng, halo, 0.04));
        let lean = rng.random_range(-0.4..0.4);
        for i in 0..rays {
            let a = lean + TAU * (i as f64 + rng.random_range(-0.3..0.3)) / rays.max(1) as f64;
            // Every so often one of the four that make its cross.
            let cross = rays >= 4 && i % (rays / 4).max(1) == 0;
            let to = if cross { self.reach * rng.random_range(0.9..1.0) } else { self.length(rng, 0.45, self.reach * 0.8) };
            let from = core * rng.random_range(1.05..1.5);
            let mut points = vec![self.at(a, from)];
            if rng.random_range(0.0..1.0) < 0.2 {
                let part = rng.random_range(0.45..0.8);
                points.push(self.at(a, from + (to - from) * part));
                points.push(self.at(a + rng.random_range(-0.22..0.22), to));
            } else {
                points.push(self.at(a + rng.random_range(-0.02..0.02), to));
            }
            star.rays.append(path_of(&points));
            // A second hair beside the long ones, a little shorter.
            if cross {
                star.rays.append(path_of(&[self.at(a + 0.03, from), self.at(a + 0.012, to * rng.random_range(0.6..0.85))]));
            }
        }
        star
    }

    /// A dark, lumpen core with stubby rays.
    fn weeping(&self, rng: &mut StdRng, rays: u32) -> EerieStar {
        let mut star = empty();
        star.dark = self.ring(rng, 0.86, 0.14);
        star.body = star.dark.clone();
        self.spikes(rng, &mut star, rays, 0.9, 0.2);
        star
    }

    /// `rays` spikes round the body, their feet `from` radii out and `wide`
    /// radii across, no two alike.
    fn spikes(&self, rng: &mut StdRng, star: &mut EerieStar, rays: u32, from: f64, wide: f64) {
        let lean = rng.random_range(0.0..TAU);
        for i in 0..rays {
            let a = lean + TAU * (i as f64 + rng.random_range(-0.25..0.25)) / rays as f64;
            let to = self.length(rng, from + 0.12, self.reach.max(from + 0.15));
            star.rays.append(self.spike(a, from, to, wide * rng.random_range(0.6..1.2), rng.random_range(-0.12..0.12)));
        }
    }

    /// Tears from the star's underside, straight down the page whichever
    /// way it's turned: each a line wavering down to a drop.
    fn tears(&self, rng: &mut StdRng, tears: u32) -> StrokePath {
        let mut path = StrokePath::new();
        for i in 0..tears {
            let across = if tears == 1 { rng.random_range(-0.3..0.3) } else { -0.62 + 1.24 * (i as f64 + rng.random_range(0.2..0.8)) / tears as f64 };
            let top = self.c + V::new(across, (1.0 - across * across).max(0.0).sqrt() * 0.8) * self.r;
            let long = self.r * rng.random_range(0.9..2.6);
            let sway = self.r * rng.random_range(-0.05..0.05);
            let points: Vec<V> = (0..=6)
                .map(|k| {
                    let s = k as f64 / 6.0;
                    top + V::new(sway * (s * PI * 1.5).sin(), long * s)
                })
                .collect();
            path = spline(path, &points, false);
            let drop = self.r * rng.random_range(0.06..0.11);
            let end = *points.last().unwrap() + V::new(0.0, drop * 0.6);
            path.append(StrokePath::circle(Vec2::new(end.x as f32, end.y as f32), drop as f32));
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn star(kind: StarKind, seed: u64) -> EerieStar {
        eerie_star(Vec2::new(100.0, 100.0), 40.0, StarSettings { kind, ..StarSettings::default() }, seed).unwrap()
    }

    fn reach(path: &StrokePath, from: Vec2) -> f32 {
        let mut far: f32 = 0.0;
        path.for_each_point(|p| far = far.max(((p.x - from.x).powi(2) + (p.y - from.y).powi(2)).sqrt()));
        far
    }

    #[test]
    fn every_kind_draws_and_the_same_seed_draws_the_same() {
        for name in StarKind::NAMES {
            let kind = StarKind::named(name).unwrap();
            let (a, b, other) = (star(kind, 7), star(kind, 7), star(kind, 8));
            assert!(!a.body.commands().is_empty() && !a.dark.commands().is_empty(), "{} has a body and a dark", name);
            assert_eq!(a.body.points_hash(), b.body.points_hash(), "{}", name);
            assert_eq!(a.rays.points_hash(), b.rays.points_hash(), "{}", name);
            assert_ne!(a.body.points_hash(), other.body.points_hash(), "{}", name);
        }
    }

    #[test]
    fn rays_keep_within_their_reach() {
        let centre = Vec2::new(100.0, 100.0);
        for name in StarKind::NAMES {
            let kind = StarKind::named(name).unwrap();
            let settings = StarSettings { kind, reach: Some(2.0), ..StarSettings::default() };
            let star = eerie_star(centre, 40.0, settings, 3).unwrap();
            assert!(reach(&star.rays, centre) <= 40.0 * 2.0 * 1.12, "{}'s rays reach {}", name, reach(&star.rays, centre));
        }
    }

    #[test]
    fn tears_run_down_and_are_counted() {
        let weeping = star(StarKind::Weeping, 1);
        assert_eq!(weeping.tears.subpaths().len(), 6, "three tears, a line and a drop each");
        let mut lowest: f32 = 0.0;
        weeping.tears.for_each_point(|p| lowest = lowest.max(p.y));
        assert!(lowest > 140.0, "tears reach below the star: {}", lowest);
        assert!(star(StarKind::Needle, 1).tears.commands().is_empty());
    }

    #[test]
    fn bad_settings_are_refused() {
        let at = Vec2::new(0.0, 0.0);
        let with = |settings: StarSettings| eerie_star(at, 10.0, settings, 0).err().unwrap_or_default();
        assert!(eerie_star(at, 0.0, StarSettings::default(), 0).is_err());
        assert!(with(StarSettings { unevenness: 1.5, ..StarSettings::default() }).contains("unevenness"));
        assert!(with(StarSettings { reach: Some(0.5), ..StarSettings::default() }).contains("reach"));
        assert!(with(StarSettings { rays: Some(100_000), ..StarSettings::default() }).contains("at most"));
    }
}
