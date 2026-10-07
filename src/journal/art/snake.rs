//! Snakes seen from above: the outline of a snake whose body winds from its
//! head to the tip of its tail, passing over and under itself where it
//! loops back.
//!
//! How one is made:
//!
//! 1. The body's width along its spine (`Profile`): a rounded head, a
//!    narrower neck, the body at full width, and a long tapering tail.
//! 2. The spine (`Spine`): straight through the head, then a chain of
//!    circular arcs. Their curvatures start out as a random meander (the
//!    sine-generated curve meandering rivers follow, with its wavelength and
//!    swing wandering) and are then steered, changing them as little as it
//!    takes, until the tail's tip lands on the tail end. No arc bends tighter
//!    than twice the body's half width there, so the inside of every bend
//!    stays open.
//! 3. Crossings (`Body`): where the body overlaps itself, one strand goes
//!    over, and the outline of the other stops at its edge. Strands
//!    alternate over and under along the body, except that the head always
//!    goes over.
//!
//! Several meanders are tried, all drawn from the seed, and the one that
//! needed the least steering and crosses itself most cleanly is drawn.

use std::collections::{BTreeSet, HashMap};
use std::f64::consts::{PI, TAU};
use std::ops::{Add, Mul, Sub};

use parry2d::math::Vec2;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::journal::paths::StrokePath;

/// Half widths along the body, as fractions of its full width. The head is
/// a pit viper's: as broad across the jaws as the body, over a neck about
/// half as wide.
const JAW_HALF_WIDTH: f64 = 0.5;
const NECK_HALF_WIDTH: f64 = 0.27;
const BODY_HALF_WIDTH: f64 = 0.5;
/// From the snout's tip to the widest part of the head (the angle of the
/// jaws), and on to the narrowest part of the neck, in full widths. The
/// head (up to the neck) stays straight.
const JAW: f64 = 1.05;
const HEAD_LENGTH: f64 = 1.45;
/// One side of the head from the snout's tip to the jaw, as a cubic Bézier
/// of (along, half width) points in full widths: square across the tip for
/// a blunt snout, then nearly straight sides spreading to the jaw.
const HEAD_SIDE: [(f64, f64); 4] = [(0.0, 0.0), (0.0, 0.22), (0.8, JAW_HALF_WIDTH), (JAW, JAW_HALF_WIDTH)];
/// The shortest snake, in full widths.
const MIN_LENGTH: f64 = 4.0;
/// How sharply the tail tapers to its point: higher is thinner.
const TAIL_TAPER: f64 = 1.5;
/// The fraction of the snake that tapers into the tail.
const TAIL_FRACTION: f64 = 0.4;
/// The shortest wavelength the body meanders with, in full widths, however
/// little it swings.
const MIN_WAVELENGTH: f64 = 8.0;
/// The fraction of the body behind the head that the tail's curl takes up.
const CURL_FRACTION: f64 = 0.15;

/// Meanders tried per snake.
const CANDIDATES: usize = 8;

/// A point or direction; the snake is worked out in f64, then written as an
/// f32 path.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct V {
    x: f64,
    y: f64,
}

impl V {
    const fn new(x: f64, y: f64) -> Self {
        V { x, y }
    }
    /// The unit vector at `angle` radians from +x, towards +y.
    fn angle(angle: f64) -> Self {
        V::new(angle.cos(), angle.sin())
    }
    fn dot(self, other: V) -> f64 {
        self.x * other.x + self.y * other.y
    }
    fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
    /// A quarter turn from +x towards +y.
    fn perp(self) -> V {
        V::new(-self.y, self.x)
    }
    fn lerp(self, other: V, t: f64) -> V {
        self + (other - self) * t
    }
    fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for V {
    type Output = V;
    fn add(self, o: V) -> V {
        V::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for V {
    type Output = V;
    fn sub(self, o: V) -> V {
        V::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for V {
    type Output = V;
    fn mul(self, s: f64) -> V {
        V::new(self.x * s, self.y * s)
    }
}

impl From<Vec2> for V {
    fn from(v: Vec2) -> V {
        V::new(v.x as f64, v.y as f64)
    }
}

/// The outline of a snake seen from above, `length` long along its spine
/// (snout to tail tip) and `max_width` across at its widest. Its head is
/// centred on `head_center`, facing `head_angle` (radians: 0 faces +x and
/// pi/2 faces +y, down the page), and its body winds to the tip of its tail
/// at `tail_end`. `seed` picks the winding.
///
/// Where the body crosses itself, the outline of the strand underneath stops
/// at the edge of the one on top, so the path is one closed outline (from
/// the tail's tip, round the head and back) only when the body never
/// overlaps; otherwise it's several open ones.
pub fn snake_path(head_center: Vec2, head_angle: f32, tail_end: Vec2, length: f32, max_width: f32, seed: u64) -> Result<StrokePath, String> {
    let (_, body) = fit(head_center.into(), head_angle as f64, tail_end.into(), length as f64, max_width as f64, seed)?;
    Ok(body.outline())
}

/// The spine and body `snake_path` outlines.
fn fit(head_center: V, angle: f64, tail_end: V, length: f64, width: f64, seed: u64) -> Result<(Spine, Body), String> {
    if !(head_center.is_finite() && tail_end.is_finite() && angle.is_finite() && length.is_finite() && width.is_finite()) {
        return Err("every argument must be a finite number".into());
    }
    if !(width > 0.0) {
        return Err(format!("max_width must be positive, got {}", width));
    }
    if !(length >= MIN_LENGTH * width) {
        return Err(format!(
            "snake_length must be at least {} times max_width ({}), got {}",
            MIN_LENGTH, MIN_LENGTH * width, length,
        ));
    }
    let profile = Profile::new(length, width);
    let head = profile.head();
    let facing = V::angle(angle);
    let neck = head_center - facing * (head / 2.0);
    let body_length = length - head;
    // A straight body only just reaches, and only straight behind the head.
    if (tail_end - neck).len() > 0.99 * body_length {
        return Err(format!(
            "the tail end is {:.1} from the head's center, too far for a snake {} long, which reaches at most {:.1}",
            (tail_end - head_center).len(), length, length - head / 2.0,
        ));
    }

    let arcs = ((body_length / (width / 3.0)).ceil() as usize).max(8);
    let base = Spine {
        start: head_center + facing * (head / 2.0),
        heading: angle + PI,
        head,
        step: body_length / arcs as f64,
        curvatures: vec![0.0; arcs],
    };
    // The most each arc may curve: to a radius of twice the widest half
    // width along it, or a quarter width at the thin end of the tail.
    let limits: Vec<f64> = (0..arcs).map(|i| {
        let (from, to) = (head + i as f64 * base.step, head + (i + 1) as f64 * base.step);
        let half = [from, (from + to) / 2.0, to].map(|s| profile.half_width(s)).into_iter().fold(0.0, f64::max);
        1.0 / (2.0 * half).max(0.25 * width)
    }).collect();

    let mut rng = StdRng::seed_from_u64(seed);
    let best_of = |candidates: Vec<(Meander, bool)>| {
        let mut best: Option<(f64, Spine, Body)> = None;
        for (meander, over_first) in candidates {
            let Some((spine, deviation)) = fit_spine(&base, &limits, meander, tail_end, width) else {
                continue;
            };
            let body = Body::new(&spine, &profile, over_first);
            let score = deviation + body.penalty();
            if best.as_ref().is_none_or(|(best_score, _, _)| score < *best_score) {
                best = Some((score, spine, body));
            }
        }
        best
    };
    let candidates = (0..CANDIDATES).map(|_| (Meander::random(&mut rng), rng.random::<bool>())).collect();
    // With little room to spare, every meander may wander too far to steer
    // back; a body that doesn't meander at all can still fit.
    let best = best_of(candidates).or_else(|| best_of(vec![(Meander::still(), false)]));
    let (_, spine, body) = best.ok_or_else(|| format!(
        "couldn't wind a snake {} long from its head to the tail end without bending it too sharply; make it longer, or move the tail end",
        length,
    ))?;
    Ok((spine, body))
}

/// How wide the body is along its spine.
struct Profile {
    width: f64,
    length: f64,
    /// Where the neck has widened to the full body.
    neck_end: f64,
    /// Where the tail starts to taper.
    taper_start: f64,
}

impl Profile {
    fn new(length: f64, width: f64) -> Self {
        let head = HEAD_LENGTH * width;
        let neck_end = head + (2.5 * width).min(0.3 * (length - head));
        Profile {
            width,
            length,
            neck_end,
            taper_start: ((1.0 - TAIL_FRACTION) * length).max(neck_end),
        }
    }
    /// The straight part of the spine, from the snout's tip to the neck.
    fn head(&self) -> f64 {
        HEAD_LENGTH * self.width
    }
    /// Half the body's width `s` along the spine from the snout's tip.
    fn half_width(&self, s: f64) -> f64 {
        let jaw = JAW * self.width;
        let head = self.head();
        let half = if s <= 0.0 || s >= self.length {
            0.0
        } else if s < jaw {
            head_side(s / self.width)
        } else if s < head {
            // Rounding the angle of the jaws, then in sharply to the neck.
            ease(JAW_HALF_WIDTH, NECK_HALF_WIDTH, (s - jaw) / (head - jaw))
        } else if s < self.neck_end {
            ease(NECK_HALF_WIDTH, BODY_HALF_WIDTH, (s - head) / (self.neck_end - head))
        } else if s < self.taper_start {
            BODY_HALF_WIDTH
        } else {
            // Level where the taper starts, and coming to a point.
            let u = (s - self.taper_start) / (self.length - self.taper_start);
            BODY_HALF_WIDTH * (1.0 - u).powf(TAIL_TAPER) * (1.0 + TAIL_TAPER * u)
        };
        half * self.width
    }
}

/// The head's half width (in full widths) `along` full widths from the
/// snout's tip to the jaw, on `HEAD_SIDE`.
fn head_side(along: f64) -> f64 {
    let point = |t: f64| {
        let u = 1.0 - t;
        let weights = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
        HEAD_SIDE.iter().zip(weights).fold((0.0, 0.0), |(x, y), (&(px, py), w)| (x + w * px, y + w * py))
    };
    // The curve only runs forward along the head, so find where it reaches
    // `along` by bisection.
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if point(mid).0 < along { lo = mid } else { hi = mid }
    }
    point((lo + hi) / 2.0).1
}

/// From `a` to `b` as `t` runs from 0 to 1, level at both ends.
fn ease(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * (1.0 - (PI * t).cos()) / 2.0
}

/// The snake's spine: straight from the snout's tip through the head, then
/// a chain of circular arcs of equal length.
#[derive(Clone)]
struct Spine {
    start: V,
    /// The direction from the snout back through the head.
    heading: f64,
    /// The straight part's length.
    head: f64,
    /// Each arc's length.
    step: f64,
    /// Each arc's curvature (1 / radius), positive turning from +x towards
    /// +y.
    curvatures: Vec<f64>,
}

impl Spine {
    /// Where each arc starts (and the last one ends), and the heading there.
    fn joints(&self) -> Vec<(V, f64)> {
        let mut at = self.start + V::angle(self.heading) * self.head;
        let mut heading = self.heading;
        let mut joints = Vec::with_capacity(self.curvatures.len() + 1);
        joints.push((at, heading));
        for &k in &self.curvatures {
            at = arc_end(at, heading, k, self.step);
            heading += k * self.step;
            joints.push((at, heading));
        }
        joints
    }
    /// Where the tail's tip is.
    fn end(&self) -> V {
        self.joints().last().expect("a spine has joints").0
    }
    /// The point and heading `s` along the spine from the snout's tip.
    fn at(&self, joints: &[(V, f64)], s: f64) -> (V, f64) {
        if s <= self.head {
            return (self.start + V::angle(self.heading) * s, self.heading);
        }
        let i = (((s - self.head) / self.step) as usize).min(self.curvatures.len() - 1);
        let along = (s - self.head - i as f64 * self.step).clamp(0.0, self.step);
        let ((at, heading), k) = (joints[i], self.curvatures[i]);
        (arc_end(at, heading, k, along), heading + k * along)
    }
}

/// The end of an arc `length` long with curvature `k`, from `start` heading
/// `heading`.
fn arc_end(start: V, heading: f64, k: f64, length: f64) -> V {
    // The chord turns half as far as the arc and is sin(x)/x as long, for
    // x the half turn.
    let half_turn = k * length / 2.0;
    let chord = if half_turn.abs() < 1e-4 {
        length * (1.0 - half_turn * half_turn / 6.0)
    } else {
        length * half_turn.sin() / half_turn
    };
    start + V::angle(heading + half_turn) * chord
}

#[derive(Clone)]
/// A random meander for the body: how far its heading swings from side to
/// side along it, as a sine-generated curve whose wavelength and swing
/// wander, skewed so its loops lean, with a curl at the tail.
struct Meander {
    /// The furthest the heading swings either way, in radians; its sign
    /// picks the way the first bend goes.
    swing: f64,
    phase: f64,
    /// Set by `fit_spine`: as tight as the body allows (but at least
    /// MIN_WAVELENGTH), then scaled by `looseness`.
    wavelength: f64,
    looseness: f64,
    /// Third harmonics, which lean the loops.
    skew: [f64; 2],
    /// Wanders in the wavelength and in the swing: (amount, cycles per
    /// wavelength, phase).
    stretch: [(f64, f64, f64); 2],
    swell: [(f64, f64, f64); 2],
    /// How far the tail curls, in radians.
    curl: f64,
}

impl Meander {
    fn random(rng: &mut StdRng) -> Self {
        let side = |rng: &mut StdRng| if rng.random::<bool>() { 1.0 } else { -1.0 };
        let wander = |rng: &mut StdRng, amount: f64| {
            (amount * rng.random_range(0.5..1.0), rng.random_range(0.3..0.8), rng.random_range(0.0..TAU))
        };
        Meander {
            swing: side(rng) * rng.random_range(2.0..2.6),
            phase: rng.random_range(0.0..TAU),
            wavelength: 1.0,
            looseness: rng.random_range(1.0..1.4),
            skew: [rng.random_range(-0.15..0.15), rng.random_range(-0.15..0.15)],
            stretch: [wander(rng, 0.12), wander(rng, 0.08)],
            swell: [wander(rng, 0.2), wander(rng, 0.1)],
            curl: side(rng) * rng.random_range(0.3..1.0) * PI,
        }
    }
    /// No meander and no curl: a body as straight as it can be.
    fn still() -> Self {
        Meander {
            swing: 0.0,
            phase: 0.0,
            wavelength: 1.0,
            looseness: 1.0,
            skew: [0.0; 2],
            stretch: [(0.0, 1.0, 0.0); 2],
            swell: [(0.0, 1.0, 0.0); 2],
            curl: 0.0,
        }
    }
    /// The heading's swing `x` along the body (from the neck).
    fn swing_at(&self, x: f64) -> f64 {
        let cycles = x / self.wavelength;
        let mut phase = self.phase + TAU * cycles;
        for &(amount, freq, offset) in &self.stretch {
            // The phase of a wavelength stretched by amount * sin(...).
            phase += amount / freq * (offset.cos() - (TAU * freq * cycles + offset).cos());
        }
        let swell = 1.0 + self.swell.iter().map(|&(amount, freq, offset)| amount * (TAU * freq * cycles + offset).sin()).sum::<f64>();
        self.swing * (swell * phase.sin() + self.skew[0] * (3.0 * phase).cos() + self.skew[1] * (3.0 * phase).sin())
    }
    /// How far the tail has curled `x` along a body `length` long.
    fn curl_at(&self, x: f64, length: f64) -> f64 {
        let start = (1.0 - CURL_FRACTION) * length;
        if x <= start {
            return 0.0;
        }
        let t = (x - start) / (length - start);
        self.curl * t * t * (3.0 - 2.0 * t)
    }
    /// Each arc's curvature as the meander's swing and the tail's curl.
    fn curvatures(&self, step: f64, arcs: usize) -> (Vec<f64>, Vec<f64>) {
        let length = step * arcs as f64;
        let turn = |f: &dyn Fn(f64) -> f64| -> Vec<f64> {
            (0..arcs).map(|i| (f((i + 1) as f64 * step) - f(i as f64 * step)) / step).collect()
        };
        (turn(&|x| self.swing_at(x)), turn(&|x| self.curl_at(x, length)))
    }
}

/// Fit `base`'s arcs to `meander`, swinging it only as wide as it takes to
/// leave the tail about the right distance from the neck, then steer them
/// to `goal`. Returns the spine and how far it had to be steered from the
/// meander (dimensionless), or `None` if it couldn't reach.
fn fit_spine(base: &Spine, limits: &[f64], meander: Meander, goal: V, width: f64) -> Option<(Spine, f64)> {
    let arcs = base.curvatures.len();
    // The tightest wavelength that keeps within the limits at full swing.
    let mut tightest = meander.clone();
    tightest.wavelength = 10.0 * width;
    for _ in 0..4 {
        let (swing, _) = tightest.curvatures(base.step, arcs);
        let bend = swing.iter().zip(limits).map(|(k, limit)| k.abs() / limit).fold(0.0, f64::max);
        if bend > 0.0 {
            tightest.wavelength *= bend / 0.9;
        }
    }
    // Swinging less, the bends stay as tight over a shorter wavelength.
    let target_at = |scale: f64| -> Vec<f64> {
        let mut shaped = meander.clone();
        shaped.swing *= scale;
        shaped.wavelength = (scale * tightest.wavelength).max(MIN_WAVELENGTH * width) * meander.looseness;
        let (swing, curl) = shaped.curvatures(base.step, arcs);
        (0..arcs).map(|i| (swing[i] + curl[i]).clamp(-limits[i], limits[i])).collect()
    };

    let mut spine = base.clone();
    let neck = spine.joints()[0].0;
    let reach = (goal - neck).len();
    let reach_at = |scale: f64| -> f64 {
        let mut trial = base.clone();
        trial.curvatures = target_at(scale);
        (trial.end() - neck).len()
    };
    // Reach falls as the swing grows (more or less).
    let scale = if reach_at(1.0) >= reach {
        1.0
    } else if reach_at(0.0) <= reach {
        0.0
    } else {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..30 {
            let mid = (lo + hi) / 2.0;
            if reach_at(mid) > reach { lo = mid } else { hi = mid }
        }
        (lo + hi) / 2.0
    };
    let target = target_at(scale);
    if !steer(&mut spine, &target, limits, goal, 1e-4 * width) {
        return None;
    }
    let deviation = spine.curvatures.iter().zip(&target).map(|(k, t)| (k - t).powi(2)).sum::<f64>() * spine.step * width;
    Some((spine, deviation))
}

/// Bend `spine`'s arcs, starting from `target`, until its end lands within
/// `tolerance` of `goal`, changing them as little as it can and bending
/// none past its limit (Gauss-Newton, holding arcs at their limits when a
/// step would push them further). False if it can't get there.
fn steer(spine: &mut Spine, target: &[f64], limits: &[f64], goal: V, tolerance: f64) -> bool {
    let arcs = target.len();
    let clamped = |k: Vec<f64>| -> Vec<f64> { k.iter().zip(limits).map(|(k, limit)| k.clamp(-limit, *limit)).collect() };
    spine.curvatures = clamped(target.to_vec());
    let mut miss = spine.end() - goal;
    for _ in 0..100 {
        if miss.len() < tolerance {
            return true;
        }
        // How the end moves as each arc bends: about the arc's middle.
        let joints = spine.joints();
        let end = joints[arcs].0;
        let pulls: Vec<V> = (0..arcs).map(|i| {
            let (at, heading) = joints[i];
            let middle = arc_end(at, heading, spine.curvatures[i], spine.step / 2.0);
            (end - middle).perp() * spine.step
        }).collect();
        let mut held = vec![false; arcs];
        let mut change = least_change(&spine.curvatures, target, &pulls, &held, miss);
        let mut more_held = false;
        for i in 0..arcs {
            let k = spine.curvatures[i];
            if k.abs() >= limits[i] * (1.0 - 1e-9) && change[i] * k > 0.0 {
                held[i] = true;
                more_held = true;
            }
        }
        if more_held {
            change = least_change(&spine.curvatures, target, &pulls, &held, miss);
        }
        let mut scale = 1.0;
        loop {
            let trial = clamped(spine.curvatures.iter().zip(&change).map(|(k, c)| k + c * scale).collect());
            let previous = std::mem::replace(&mut spine.curvatures, trial);
            let trial_miss = spine.end() - goal;
            if trial_miss.len() < miss.len() {
                miss = trial_miss;
                break;
            }
            spine.curvatures = previous;
            scale /= 2.0;
            if scale < 1e-3 {
                return false;
            }
        }
    }
    miss.len() < tolerance
}

/// The smallest change to the arcs' distance from `target` (the arcs not
/// `held`) that, as far as `pulls` (how the end moves per unit of each
/// arc's curvature) tells, moves the end by `-miss`.
fn least_change(curvatures: &[f64], target: &[f64], pulls: &[V], held: &[bool], miss: V) -> Vec<f64> {
    // change = g - pullᵀ mu, for g the way back to the target, with mu
    // solving (Σ pull pullᵀ) mu = miss + Σ pull g.
    let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
    let mut rhs = miss;
    let back = |i: usize| if held[i] { 0.0 } else { target[i] - curvatures[i] };
    for (i, pull) in pulls.iter().enumerate() {
        if held[i] {
            continue;
        }
        a += pull.x * pull.x;
        b += pull.x * pull.y;
        c += pull.y * pull.y;
        rhs = rhs + *pull * back(i);
    }
    let ridge = 1e-12 * (a + c) + 1e-300;
    let (a, c) = (a + ridge, c + ridge);
    let det = a * c - b * b;
    let mu = V::new((c * rhs.x - b * rhs.y) / det, (a * rhs.y - b * rhs.x) / det);
    (0..pulls.len()).map(|i| if held[i] { 0.0 } else { back(i) - pulls[i].dot(mu) }).collect()
}

/// A cross-section of the body.
struct Section {
    /// How far along the spine, from the snout's tip.
    s: f64,
    at: V,
    /// A unit vector to the snake's left.
    left: V,
    half: f64,
}

impl Section {
    fn left_edge(&self) -> V {
        self.at + self.left * self.half
    }
    fn right_edge(&self) -> V {
        self.at - self.left * self.half
    }
}

/// Sections nearer than this along the spine are neighbours rather than
/// crossings. With bends no tighter than twice the half width, a body that
/// loops back overlaps itself only much further along.
fn apart(a: &Section, b: &Section) -> f64 {
    4.0 * a.half.max(b.half)
}

/// Where the body overlaps itself: two stretches of sections, the one
/// nearer the head first.
struct Crossing {
    strands: [(usize, usize); 2],
    /// Which strand goes on top.
    over: usize,
    /// How deep the strands overlap at most, as a fraction of their half
    /// widths together: small for strands that only graze each other.
    depth: f64,
}

/// Sections either side of a crossing's strands that the outline checks
/// too, to cover gaps between the sections.
const PAD: usize = 3;

/// The snake's body: its sections from the snout's tip to the tail's tip,
/// and where it overlaps itself.
struct Body {
    sections: Vec<Section>,
    crossings: Vec<Crossing>,
    width: f64,
    head: f64,
}

impl Body {
    fn new(spine: &Spine, profile: &Profile, over_first: bool) -> Self {
        let width = profile.width;
        let joints = spine.joints();
        // Closer around the snout's tip, where its width changes fastest.
        let snout = JAW * width;
        let mut along: Vec<f64> = (0..16).map(|i| snout * (1.0 - (i as f64 / 16.0 * PI / 2.0).cos())).collect();
        let rest = profile.length - snout;
        let count = (rest / (width / 20.0).max(profile.length / 4000.0)).ceil() as usize;
        along.extend((0..=count).map(|i| snout + rest * i as f64 / count as f64));
        let sections = along.into_iter().map(|s| {
            let (at, heading) = spine.at(&joints, s);
            Section { s, at, left: V::angle(heading).perp(), half: profile.half_width(s) }
        }).collect();
        let mut body = Body { sections, crossings: Vec::new(), width, head: profile.head() };
        body.find_crossings();
        body.layer(over_first);
        body
    }

    /// Group overlapping pairs of sections into crossings.
    fn find_crossings(&mut self) {
        let sections = &self.sections;
        // Pairs of sections whose bodies overlap, found through a grid of
        // cells as wide as the body.
        let cell = |p: V| ((p.x / self.width).floor() as i64, (p.y / self.width).floor() as i64);
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, section) in sections.iter().enumerate() {
            grid.entry(cell(section.at)).or_default().push(i);
        }
        let mut pairs = BTreeSet::new();
        for (i, a) in sections.iter().enumerate() {
            let (cx, cy) = cell(a.at);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for &j in grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                        let b = &sections[j];
                        if j > i && b.s - a.s > apart(a, b) && (a.at - b.at).len() < a.half + b.half {
                            pairs.insert((i, j));
                        }
                    }
                }
            }
        }
        // Pairs next to each other belong to the same crossing.
        let mut crossings = Vec::new();
        while let Some(&first) = pairs.iter().next() {
            pairs.remove(&first);
            let mut todo = vec![first];
            let (mut a, mut b) = ((first.0, first.0), (first.1, first.1));
            let mut depth: f64 = 0.0;
            while let Some((i, j)) = todo.pop() {
                a = (a.0.min(i), a.1.max(i));
                b = (b.0.min(j), b.1.max(j));
                let (p, q) = (&sections[i], &sections[j]);
                depth = depth.max(1.0 - (p.at - q.at).len() / (p.half + q.half));
                for di in -1..=1_isize {
                    for dj in -1..=1_isize {
                        let next = (i.wrapping_add_signed(di), j.wrapping_add_signed(dj));
                        if pairs.remove(&next) {
                            todo.push(next);
                        }
                    }
                }
            }
            crossings.push(Crossing { strands: [a, b], over: 0, depth });
        }
        self.crossings = crossings;
    }

    /// Pick which strand goes on top at each crossing: the head always,
    /// otherwise alternately over and under along the body, starting over
    /// or not by `over_first`.
    fn layer(&mut self, over_first: bool) {
        let sections = &self.sections;
        let mut decided: Vec<Option<usize>> = self.crossings.iter()
            .map(|c| (sections[c.strands[0].0].s < self.head).then_some(0))
            .collect();
        let mut passes: Vec<(f64, usize, usize)> = Vec::new();
        for (c, crossing) in self.crossings.iter().enumerate() {
            for (strand, &(lo, hi)) in crossing.strands.iter().enumerate() {
                passes.push(((sections[lo].s + sections[hi].s) / 2.0, c, strand));
            }
        }
        passes.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut over = over_first;
        for (_, c, strand) in passes {
            let top = *decided[c].get_or_insert(if over { strand } else { 1 - strand });
            over = top != strand;
        }
        for (crossing, top) in self.crossings.iter_mut().zip(decided) {
            crossing.over = top.expect("every crossing is decided");
        }
    }

    /// How deep `q`, on the outline of section `k`, lies inside strands
    /// passing over section `k`'s: positive when it's hidden.
    fn cover(&self, q: V, k: usize) -> f64 {
        let last = self.sections.len() - 1;
        let own = &self.sections[k];
        let mut depth = f64::NEG_INFINITY;
        for crossing in &self.crossings {
            let (under, over) = (crossing.strands[1 - crossing.over], crossing.strands[crossing.over]);
            if k + PAD < under.0 || k > under.1 + PAD {
                continue;
            }
            for m in over.0.saturating_sub(PAD)..(over.1 + PAD).min(last) {
                let (a, b) = (&self.sections[m], &self.sections[m + 1]);
                if (a.s - own.s).abs() <= apart(a, own) || (b.s - own.s).abs() <= apart(b, own) {
                    continue;
                }
                // The body between two sections, as a capsule whose width
                // runs from one to the other.
                let ab = b.at - a.at;
                let t = if ab.dot(ab) > 0.0 { ((q - a.at).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0) } else { 0.0 };
                let half = a.half + (b.half - a.half) * t;
                depth = depth.max(half - (q - a.at.lerp(b.at, t)).len());
            }
        }
        depth
    }

    /// Where the outline between a visible point and a hidden one (on
    /// section `k`'s outline) meets the edge of the strand over it.
    fn edge(&self, visible: V, hidden: V, k: usize) -> V {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if self.cover(visible.lerp(hidden, mid), k) >= 0.0 { hi = mid } else { lo = mid }
        }
        visible.lerp(hidden, (lo + hi) / 2.0)
    }

    /// How badly the body crosses itself, to choose between meanders:
    /// strands that run over each other a long way, that only graze, or
    /// that go over the head or the tail's tip.
    fn penalty(&self) -> f64 {
        let span = |(lo, hi): (usize, usize)| self.sections[hi].s - self.sections[lo].s;
        let mut penalty = 0.0;
        for crossing in &self.crossings {
            let long = span(crossing.strands[0]).max(span(crossing.strands[1])) / self.width;
            penalty += (long - 4.0).max(0.0);
            if crossing.depth < 0.35 {
                penalty += 2.0;
            }
            if self.sections[crossing.strands[0].0].s < self.head {
                penalty += 3.0;
            }
        }
        let tip = self.sections.len() - 1;
        if self.cover(self.sections[tip].at, tip) >= 0.0 {
            penalty += 1.0;
        }
        penalty
    }

    /// Points around the outline, each with its section: from the tail's
    /// tip up the right side, round the snout, and back down the left side.
    /// The snout's and the tail's tips are on both sides, so appear once.
    fn outline_points(&self) -> Vec<(V, usize)> {
        let last = self.sections.len() - 1;
        let mut points: Vec<(V, usize)> = (0..=last).rev().map(|k| (self.sections[k].right_edge(), k)).collect();
        points.extend((1..last).map(|k| (self.sections[k].left_edge(), k)));
        points
    }

    /// The outline's visible runs, each a list of points and whether each
    /// is the tail's tip, a corner the curve mustn't smooth over. Hidden
    /// stretches end at the edge of the strand over them.
    fn visible_runs(&self) -> Vec<Vec<(V, bool)>> {
        let points = self.outline_points();
        let hidden: Vec<bool> = points.iter().map(|&(q, k)| self.cover(q, k) >= 0.0).collect();
        let Some(start) = hidden.iter().position(|&h| h) else {
            let mut run: Vec<(V, bool)> = points.iter().map(|&(q, _)| (q, false)).collect();
            run.push(run[0]);
            return vec![run];
        };
        let n = points.len();
        let mut runs = Vec::new();
        let mut run: Vec<(V, bool)> = Vec::new();
        for step in 1..=n {
            let (i, before) = ((start + step) % n, (start + step - 1) % n);
            let (q, k) = points[i];
            match (hidden[before], hidden[i]) {
                (true, false) => {
                    run.push((self.edge(q, points[before].0, points[before].1), false));
                    run.push((q, i == 0));
                }
                (false, false) => run.push((q, i == 0)),
                (false, true) => {
                    run.push((self.edge(points[before].0, q, k), false));
                    runs.push(std::mem::take(&mut run));
                }
                (true, true) => {}
            }
        }
        // Drop specks of outline between two strands passing over.
        runs.retain(|run| run.windows(2).map(|w| (w[1].0 - w[0].0).len()).sum::<f64>() >= 0.1 * self.width);
        runs
    }

    /// The visible runs as a path, one stroke each.
    fn outline(&self) -> StrokePath {
        let mut path = StrokePath::new();
        for run in self.visible_runs() {
            // Pieces between corners, continuing the same stroke.
            let mut piece = vec![run[0].0];
            let mut started = false;
            for (i, &(q, corner)) in run.iter().enumerate().skip(1) {
                piece.push(q);
                if corner || i == run.len() - 1 {
                    path = spline(path, &simplify(&piece, 0.005 * self.width, 0.6 * self.width), started);
                    started = true;
                    piece = vec![q];
                }
            }
        }
        path
    }
}

/// `points` thinned out to those needed to follow them within `tolerance`,
/// at most `max_gap` apart; the ends are always kept.
fn simplify(points: &[V], tolerance: f64, max_gap: f64) -> Vec<V> {
    let mut points: Vec<V> = points.to_vec();
    points.dedup_by(|a, b| (*a - *b).len() < 1e-9);
    if points.len() < 3 {
        return points;
    }
    let off_chord = |p: V, a: V, b: V| {
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
        (p - a.lerp(b, t)).len()
    };
    let mut kept = vec![points[0]];
    let mut anchor = 0;
    for i in 2..points.len() {
        let (a, b) = (points[anchor], points[i]);
        let fits = (b - a).len() <= max_gap && points[anchor + 1..i].iter().all(|&p| off_chord(p, a, b) <= tolerance);
        if !fits {
            anchor = i - 1;
            kept.push(points[anchor]);
        }
    }
    kept.push(points[points.len() - 1]);
    kept
}

/// Smooth cubics through `points` (Catmull-Rom with chord-length tangents,
/// so uneven spacing doesn't kink it), added to `path`, continuing its
/// current stroke if `continues`.
fn spline(mut path: StrokePath, points: &[V], continues: bool) -> StrokePath {
    let f = |v: V| (v.x as f32, v.y as f32);
    if !continues {
        let (x, y) = f(points[0]);
        path = path.move_to(x, y);
    }
    let n = points.len();
    if n < 2 {
        return path;
    }
    let gaps: Vec<f64> = points.windows(2).map(|w| (w[1] - w[0]).len()).collect();
    // A unit-ish tangent at each point.
    let mut tangents = vec![V::default(); n];
    for i in 1..n - 1 {
        let (d0, d1) = (gaps[i - 1], gaps[i]);
        tangents[i] = ((points[i] - points[i - 1]) * (d1 / d0) + (points[i + 1] - points[i]) * (d0 / d1)) * (1.0 / (d0 + d1));
    }
    // At the ends, as a parabola through the end and its neighbour would.
    let chord = |i: usize| (points[i + 1] - points[i]) * (1.0 / gaps[i]);
    if n == 2 {
        tangents[0] = chord(0);
        tangents[1] = chord(0);
    } else {
        tangents[0] = chord(0) * 2.0 - tangents[1];
        tangents[n - 1] = chord(n - 2) * 2.0 - tangents[n - 2];
    }
    for i in 0..n - 1 {
        let third = gaps[i] / 3.0;
        let (c1, c2, end) = (f(points[i] + tangents[i] * third), f(points[i + 1] - tangents[i + 1] * third), f(points[i + 1]));
        path = path.cubic_curve_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: f64 = 30.0;

    fn snake(head_center: V, angle: f64, tail_end: V, length: f64, seed: u64) -> (Spine, Body) {
        fit(head_center, angle, tail_end, length, WIDTH, seed)
            .unwrap_or_else(|err| panic!("facing {} to {:?}, {} long, seed {}: {}", angle, tail_end, length, seed, err))
    }

    /// A snake whose head is at the origin facing up the page, with its tail
    /// end `behind` it.
    fn upright(behind: f64, length: f64, seed: u64) -> (Spine, Body) {
        snake(V::new(0.0, 0.0), -PI / 2.0, V::new(0.0, behind), length, seed)
    }

    #[test]
    fn spines_run_from_the_head_to_the_tail_end_at_their_length() {
        // Taut to coiled, with the head facing away from the tail end, across
        // it, and straight at it.
        let tail_end = V::new(40.0, 300.0);
        for angle in [-PI / 2.0, 0.0, 2.5, PI / 2.0] {
            for length in [600.0, 1500.0] {
                let profile = Profile::new(length, WIDTH);
                for seed in 0..3 {
                    let (spine, body) = snake(V::new(0.0, 0.0), angle, tail_end, length, seed);
                    let at = format!("facing {}, {} long, seed {}", angle, length, seed);
                    assert!((spine.start - V::angle(angle) * (profile.head() / 2.0)).len() < 1e-9, "{}", at);
                    assert!((spine.head + spine.step * spine.curvatures.len() as f64 - length).abs() < 1e-6, "{}", at);
                    assert!((spine.end() - tail_end).len() < 1e-2, "{}: ends at {:?}", at, spine.end());
                    assert!((body.sections.last().unwrap().at - tail_end).len() < 1e-2, "{}", at);
                    // No bend tighter than twice the half width.
                    for (i, k) in spine.curvatures.iter().enumerate() {
                        let half = profile.half_width(spine.head + (i as f64 + 0.5) * spine.step);
                        assert!(k.abs() * (2.0 * half).max(0.25 * WIDTH) <= 1.0 + 1e-9, "{}: arc {} bends at radius {}", at, i, 1.0 / k);
                    }
                }
            }
        }
    }

    #[test]
    fn the_body_has_a_viper_head_a_neck_and_a_pointed_tail() {
        let profile = Profile::new(900.0, WIDTH);
        let half = |s: f64| profile.half_width(s);
        let (jaw, neck) = (JAW * WIDTH, HEAD_LENGTH * WIDTH);
        assert_eq!((half(0.0), half(900.0)), (0.0, 0.0));
        assert!((half(jaw) - WIDTH / 2.0).abs() < 1e-9 && half(neck) < 0.6 * WIDTH / 2.0, "jaw {}, neck {}", half(jaw), half(neck));
        assert_eq!(half(450.0), WIDTH / 2.0);
        // A blunt snout: a tenth of the way to the jaws, it's already more
        // than a quarter as wide as them. Then widening to the jaws, in to
        // the neck, out to the body, and narrowing all the way down the tail.
        assert!(half(0.1 * jaw) > WIDTH / 8.0, "snout {}", half(0.1 * jaw));
        // Every unit along: up to and from each turn.
        let along: Vec<f64> = (0..=900).map(|s| half(s as f64)).collect();
        let (jaw, neck) = (jaw as usize, neck as usize);
        let widest = neck + along[neck..].iter().position(|&h| h == WIDTH / 2.0).unwrap();
        let rising = |range: std::ops::Range<usize>| along[range].windows(2).all(|w| w[1] >= w[0]);
        let falling = |range: std::ops::Range<usize>| along[range].windows(2).all(|w| w[1] <= w[0]);
        assert!(rising(0..jaw + 1) && falling(jaw + 1..neck + 1) && rising(neck + 1..widest) && falling(widest..901));
        let (_, body) = upright(300.0, 900.0, 1);
        let widest = body.sections.iter().map(|s| 2.0 * s.half).fold(0.0, f64::max);
        assert!((widest - WIDTH).abs() < 1e-9, "widest {}", widest);
    }

    #[test]
    fn the_same_seed_draws_the_same_snake() {
        let draw = |seed| snake_path(Vec2::new(0.0, 0.0), -1.57, Vec2::new(20.0, 250.0), 900.0, 30.0, seed).unwrap().to_string();
        assert_eq!(draw(5), draw(5));
        assert_ne!(draw(5), draw(6));
    }

    #[test]
    fn an_uncrossed_body_is_one_outline_from_the_tail_tip_round() {
        // Taut: no slack to loop with.
        let (_, body) = upright(700.0, 900.0, 3);
        assert!(body.crossings.is_empty());
        let runs = body.visible_runs();
        assert_eq!(runs.len(), 1);
        let (first, last) = (runs[0][0].0, runs[0].last().unwrap().0);
        assert!((first - V::new(0.0, 700.0)).len() < 1e-2 && first == last, "{:?} to {:?}", first, last);
        assert_eq!(body.outline().to_string().matches('M').count(), 1);
    }

    #[test]
    fn crossings_cut_the_strand_underneath_at_the_edge_of_the_one_on_top() {
        // Coiled: the tail end just behind the head.
        let mut crossings = 0;
        for seed in 0..6 {
            let (_, body) = upright(60.0, 1500.0, seed);
            let points = body.outline_points();
            let hidden: Vec<bool> = points.iter().map(|&(q, k)| body.cover(q, k) >= 0.0).collect();
            let strand_hidden = |(lo, hi): (usize, usize)| points.iter().zip(&hidden).any(|(&(_, k), &h)| h && (lo..=hi).contains(&k));
            for (&(_, k), &h) in points.iter().zip(&hidden) {
                assert!(!(h && body.sections[k].s < body.head), "seed {}: the head is hidden", seed);
            }
            let tip = body.sections.len() - 1;
            // (The strand on top can still be cut where a third strand passes
            // over both.)
            for crossing in &body.crossings {
                // Unless it only grazes, or the tail's tip ends on top of the
                // other strand without reaching across it.
                let across = crossing.depth >= 0.35 && crossing.strands[1].1 != tip;
                assert!(!across || strand_hidden(crossing.strands[1 - crossing.over]), "seed {}: the strand underneath isn't cut", seed);
            }
            // Runs that don't end at the tail's tip end where the outline
            // passes under the edge of a strand.
            let tip = body.sections[tip].at;
            let n = points.len();
            for run in body.visible_runs() {
                for (end, _) in [run[0], run[run.len() - 1]] {
                    if (end - tip).len() < 1e-9 {
                        continue;
                    }
                    let between = (0..n).find(|&i| {
                        let (a, b) = (points[i].0, points[(i + 1) % n].0);
                        hidden[i] != hidden[(i + 1) % n] && ((end - a).len() + (b - end).len() - (b - a).len()).abs() < 1e-9
                    });
                    let i = between.unwrap_or_else(|| panic!("seed {}: a run ends at {:?}, not where the outline goes under", seed, end));
                    let under = if hidden[i] { points[i].1 } else { points[(i + 1) % n].1 };
                    assert!(body.cover(end, under).abs() < 1e-6 * WIDTH, "seed {}: a run ends off the edge at {:?}", seed, end);
                }
            }
            crossings += body.crossings.len();
        }
        assert!(crossings > 0, "coiled snakes should cross themselves");
    }

    #[test]
    fn impossible_snakes_are_errors() {
        let err = |angle: f32, tail_end: Vec2, length: f32, width: f32| {
            snake_path(Vec2::new(0.0, 0.0), angle, tail_end, length, width, 1).expect_err("should fail")
        };
        for (got, want) in [
            (err(0.0, Vec2::new(0.0, 100.0), 900.0, 0.0), "max_width must be positive"),
            (err(f32::NAN, Vec2::new(0.0, 100.0), 900.0, 30.0), "finite"),
            (err(0.0, Vec2::new(0.0, 100.0), 100.0, 30.0), "at least 4 times max_width (120), got 100"),
            (err(0.0, Vec2::new(-2000.0, 0.0), 900.0, 30.0), "too far for a snake 900 long"),
            // Facing its tail end, with no room to turn round.
            (err(0.0, Vec2::new(400.0, 0.0), 500.0, 30.0), "couldn't wind a snake 500 long"),
        ] {
            assert!(got.contains(want), "{}: {}", want, got);
        }
    }
}
