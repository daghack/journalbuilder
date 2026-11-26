//! Ribbons woven from trees: each tree (points joined by edges, as a
//! spanning tree gives them) laid out as a few long ribbons, which pass
//! over and under each other where they cross, and carry text only where
//! they can be seen.
//!
//! 1. Chains (`chains`): a tree's longest path is its trunk; every other
//!    neighbour of a point on a chain starts a branch, which runs from
//!    there down the longest way to a leaf. A branch starts on the line
//!    down the middle of the chain it leaves, so its start is tucked under
//!    that ribbon.
//! 2. Sections: each chain is a smooth curve through its points, cut into
//!    sections a twelfth of its width apart.
//! 3. Crossings (`find_crossings`): where two ribbons' bands overlap (or a
//!    ribbon's own, far apart along it), one goes over. A branch's start
//!    is always under the ribbon it leaves (a tuck); otherwise each ribbon
//!    goes over and under in turn along its length, as far as all the
//!    ribbons can at once (`layer`).
//! 4. Drawing: each ribbon's edges, its ends (swallowtail, pointed or cut)
//!    and the two rules inside its edges, stopping at the edge of a ribbon
//!    over them.
//! 5. Text (`text_lines`): the stretches of each ribbon that nothing
//!    passes over, kept clear of what does, as lines to set text along
//!    and the band between the rules for it to stay inside.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use parry2d::math::Vec2 as PVec;

use super::geom::{simplify, spline, spline_points, V};
use super::trail::resample;
use crate::journal::canvas::textbox::Region;
use crate::journal::paths::StrokePath;

/// A tree to weave: points, the edges joining them (index pairs, either
/// way round), and how wide its ribbons are.
#[derive(Clone, Debug)]
pub struct RibbonTree {
    pub points: Vec<PVec>,
    pub edges: Vec<(usize, usize)>,
    pub width: f32,
}

/// How a ribbon's free ends are cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RibbonEnd {
    /// A notch cut into the end, leaving two tails.
    Swallowtail,
    /// Narrowing to a point.
    Pointed,
    /// Cut straight across.
    Cut,
    /// Trailing off past the end into a wicked point that hooks round to
    /// one side, like a claw or a fang.
    Claw,
}

impl RibbonEnd {
    pub const NAMES: [&'static str; 4] = ["swallowtail", "pointed", "cut", "claw"];

    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "swallowtail" => RibbonEnd::Swallowtail,
            "pointed" => RibbonEnd::Pointed,
            "cut" => RibbonEnd::Cut,
            "claw" => RibbonEnd::Claw,
            _ => return None,
        })
    }

    /// How far back the end's shape reaches unless told, in widths: for a
    /// claw, how far it trails past the end.
    fn depth(self) -> f64 {
        match self {
            RibbonEnd::Swallowtail => 0.6,
            RibbonEnd::Pointed => 0.8,
            RibbonEnd::Cut => 0.0,
            RibbonEnd::Claw => 3.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RibbonSettings {
    pub ends: RibbonEnd,
    /// How far back a swallowtail's notch or a point reaches; by default a
    /// share of the width (`RibbonEnd::depth`).
    pub end_depth: Option<f32>,
    /// How far inside the edges the rules are; by default a ninth of the
    /// width.
    pub rule_inset: Option<f32>,
    /// Turn the crossings no tuck settles the other way.
    pub flip: bool,
    /// Set each ribbon's text the way along it that reads right way up.
    pub upright: bool,
    /// How much wider or narrower than its width a ribbon gets along it,
    /// as a share of the width either way.
    pub width_variation: f32,
    /// How far a ribbon's middle wanders off the tree's edges, either way.
    pub wander: f32,
    /// Which wandering, widths and claws.
    pub seed: u64,
}

impl Default for RibbonSettings {
    fn default() -> Self {
        RibbonSettings { ends: RibbonEnd::Swallowtail, end_depth: None, rule_inset: None, flip: false, upright: true, width_variation: 0.0, wander: 0.0, seed: 0 }
    }
}

/// The most sections all the ribbons together may have.
const MOST_SECTIONS: usize = 200_000;
/// Sections a width apart, this many to the width.
const SECTIONS_PER_WIDTH: f64 = 12.0;
/// Sections either side of a crossing's strands that are checked too, to
/// cover the gaps between sections.
const PAD: usize = 3;
/// How long, in widths, a stretch heading back the other way must be for
/// upright text to turn round there.
const TURN_WIDTHS: f64 = 3.0;
/// How far back against the way it's read (as the cosine of its heading
/// from the way it's read) a stretch of upright text may head: further,
/// and the glyphs are as good as upside down.
const BACKWARD: f64 = -0.3;
/// The most a ribbon's width may vary by, as a share of it either way.
const MOST_VARIATION: f64 = 0.5;
/// How far apart, in widths, a ribbon's widest and narrowest are, about.
const VARIATION_WIDTHS: f64 = 3.0;
/// How long, in widths, a wandering middle's bends are, about; and how much
/// longer, for every width further it wanders.
const WANDER_WIDTHS: f64 = 4.0;
const WANDER_STRETCH: f64 = 6.0;
/// How far, in widths, a branch takes to wander and change width its own
/// way, from going as the ribbon it leaves does where it leaves.
const SETTLE_WIDTHS: f64 = 1.5;
/// A claw, each its own way between these: how far round it turns from
/// end to tip, in radians; how far it first veers off the way the ribbon
/// was heading, either way, and how soon (a share of its length) it's done
/// so; how its turning gathers towards the tip (the power of how far along
/// it is that it's turned that share of the way); how long it is, as a
/// share of the end's depth; how its sides close in (the power of how far
/// is left to go); and how close in its sides are where its rules stop, as
/// a share of the width.
const CLAW_TURN: [f64; 2] = [1.2, 3.4];
const CLAW_VEER: f64 = 0.7;
const CLAW_VEERED: f64 = 0.5;
const CLAW_HOOK: [f64; 2] = [1.5, 3.5];
const CLAW_LENGTH: [f64; 2] = [0.75, 1.25];
const CLAW_TAPER: f64 = 0.8;
const CLAW_RULES: f64 = 0.35;

/// A smooth wavering from -1 to 1 along `x`, about once a unit, the same
/// for the same `seed` and `salt`.
fn waver(seed: u64, salt: u64, x: f64) -> f64 {
    let value = |seed: u64, salt: u64, k: i64| chance(seed, salt, k) * 2.0 - 1.0;
    let smooth = |x: f64, salt: u64| {
        let k = x.floor();
        let t = x - k;
        let t = t * t * (3.0 - 2.0 * t);
        value(seed, salt, k as i64) * (1.0 - t) + value(seed, salt, k as i64 + 1) * t
    };
    ((smooth(x, salt) + 0.5 * smooth(x * 2.0 + 0.37, salt ^ 0x5555)) / 1.5).clamp(-1.0, 1.0)
}

/// A number from 0 to 1, any as likely as any other, the same for the same
/// `seed`, `salt` and `k`: splitmix64 of the three.
fn chance(seed: u64, salt: u64, k: i64) -> f64 {
    let mut z = seed ^ salt.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (k as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// `chance` spread from `range[0]` to `range[1]`.
fn between(range: [f64; 2], seed: u64, salt: u64, k: i64) -> f64 {
    range[0] + (range[1] - range[0]) * chance(seed, salt, k)
}

/// 0 at 0, rising smoothly to 1 at `over` and staying there.
fn settle(s: f64, over: f64) -> f64 {
    let t = (s / over.max(1e-9)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

struct Section {
    /// How far along the chain.
    s: f64,
    at: V,
    /// A unit vector square to the chain, to its left (or right: which
    /// doesn't matter, as long as it's the same all along).
    left: V,
    /// How much wider than its width the ribbon is here.
    w: f64,
}

/// One ribbon: a stretch of a tree from a free end or a tuck to a free
/// end.
struct Chain {
    tree: usize,
    sections: Vec<Section>,
    length: f64,
    half: f64,
    inset: f64,
    /// How each end is cut: `None` for a tucked start, cut square under the
    /// ribbon it leaves.
    ends: [Option<RibbonEnd>; 2],
    /// How far back each end's shape reaches (0 for a square one); for a
    /// claw, how long it is.
    depth: [f64; 2],
    /// The chain it leaves, and the section of it where.
    parent: Option<(usize, usize)>,
    /// The chains that leave it, with the section of it each leaves at.
    children: Vec<(usize, usize)>,
}

impl Chain {
    fn last(&self) -> usize {
        self.sections.len() - 1
    }

    /// Sections nearer than this along the chain are neighbours rather
    /// than crossings: with bends no tighter than the half width, a ribbon
    /// that loops back overlaps itself only much further along.
    fn apart(&self) -> f64 {
        4.0 * self.half
    }

    /// The section nearest `s` along the chain.
    fn section_at(&self, s: f64) -> usize {
        let k = self.sections.partition_point(|sec| sec.s < s);
        if k == 0 {
            return 0;
        }
        if k > self.last() {
            return self.last();
        }
        if s - self.sections[k - 1].s < self.sections[k].s - s { k - 1 } else { k }
    }

    /// The section nearest `p`.
    fn nearest(&self, p: V) -> usize {
        (0..self.sections.len()).min_by(|&i, &j| (self.sections[i].at - p).len().total_cmp(&(self.sections[j].at - p).len())).unwrap_or(0)
    }

    /// The way the chain heads at section `k`, a unit vector.
    fn heading(&self, k: usize) -> V {
        let d = self.sections[(k + 1).min(self.last())].at - self.sections[k.saturating_sub(1)].at;
        d * (1.0 / d.len().max(1e-12))
    }

    /// The point `y` to the left of the chain `s` along it, `y` being
    /// across a ribbon as wide as its width all along, without claws: the
    /// ribbon is wider or narrower there, and a claw curls and closes in.
    fn frame(&self, s: f64, y: f64) -> V {
        let k = self.sections.partition_point(|sec| sec.s < s).clamp(1, self.last());
        let (a, b) = (&self.sections[k - 1], &self.sections[k]);
        let t = ((s - a.s) / (b.s - a.s).max(1e-12)).clamp(0.0, 1.0);
        let left = a.left.lerp(b.left, t);
        let w = a.w + (b.w - a.w) * t;
        a.at.lerp(b.at, t) + left * (self.warp(s, y) * w / left.len().max(1e-12))
    }

    /// How much wider than its width the ribbon is `s` along it.
    fn width_at(&self, s: f64) -> f64 {
        let k = self.sections.partition_point(|sec| sec.s < s).clamp(1, self.last());
        let (a, b) = (&self.sections[k - 1], &self.sections[k]);
        let t = ((s - a.s) / (b.s - a.s).max(1e-12)).clamp(0.0, 1.0);
        a.w + (b.w - a.w) * t
    }

    /// How far into a claw `s` is (0 where it starts, 1 at its tip), and
    /// which end's, if it's in one.
    fn claw(&self, s: f64) -> Option<(f64, usize)> {
        (0..2).find_map(|k| {
            let d = self.depth[k];
            let from_end = if k == 1 { self.length - s } else { s };
            (self.ends[k] == Some(RibbonEnd::Claw) && d > 0.0 && from_end < d).then(|| (((d - from_end) / d).clamp(0.0, 1.0), k))
        })
    }

    /// `y` across a ribbon without claws, where it is across one with
    /// them: in a claw, closer in. (Its middle hooks round by itself.)
    fn warp(&self, s: f64, y: f64) -> f64 {
        match self.claw(s) {
            Some((u, _)) => y * (1.0 - u).powf(CLAW_TAPER),
            None => y,
        }
    }

    /// `warp` undone: where `y` across the ribbon would be without claws,
    /// or `None` at a claw's tip.
    fn unwarp(&self, s: f64, y: f64) -> Option<f64> {
        match self.claw(s) {
            Some((u, _)) => {
                let taper = (1.0 - u).powf(CLAW_TAPER);
                (taper > 1e-9).then(|| y / taper)
            }
            None => Some(y),
        }
    }

    /// The furthest the ribbon reaches from its middle at section `k`.
    fn reach(&self, k: usize) -> f64 {
        let sec = &self.sections[k];
        let across = match self.claw(sec.s) {
            Some((u, _)) => (1.0 - u).powf(CLAW_TAPER),
            None => 1.0,
        };
        self.half * sec.w * across
    }

    /// Whether `(s, y)`, as `locate` finds it, is on the ribbon or within
    /// `pad` of it, about: its band widened by `pad` either side, past its
    /// ends too.
    fn near(&self, s: f64, y: f64, pad: f64) -> bool {
        if s < -pad || s > self.length + pad {
            return false;
        }
        let w = self.width_at(s.clamp(0.0, self.length));
        match self.claw(s) {
            Some((u, _)) => {
                let taper = (1.0 - u).powf(CLAW_TAPER).max(0.05);
                (y / taper).abs() <= self.half + pad / (w * taper)
            }
            None => y.abs() <= self.half + pad / w,
        }
    }

    /// Whether `(s, y)`, along the chain and to its left, is on the
    /// ribbon narrowed (or widened) to `within` either side of its middle:
    /// past neither end, and not in a swallowtail's notch or beside a
    /// point.
    fn holds(&self, s: f64, y: f64, within: f64) -> bool {
        let h = self.half;
        if s < 0.0 || s > self.length {
            return false;
        }
        let Some(y) = self.unwarp(s, y) else { return false };
        if y.abs() > within {
            return false;
        }
        for (end, from_end, d) in [(self.ends[1], self.length - s, self.depth[1]), (self.ends[0], s, self.depth[0])] {
            if from_end >= d {
                continue;
            }
            match end {
                Some(RibbonEnd::Swallowtail) if y.abs() < h * (d - from_end) / d => return false,
                Some(RibbonEnd::Pointed) if y.abs() > h * from_end / d => return false,
                _ => {}
            }
        }
        true
    }

    /// Where `q` is against sections `lo..=hi` (as far as the chain goes):
    /// how far along the chain the nearest point on its middle is, how far
    /// to the left of it `q` is, and how far from it. Past the chain's ends
    /// the middle runs on straight, so what's beyond an end is found to be
    /// past it. Sections within the chain's `apart` of `skip` (far along
    /// it) aren't looked at. `None` if none are.
    fn locate(&self, q: V, lo: usize, hi: usize, skip: Option<f64>) -> Option<(f64, f64, f64)> {
        let last = self.last();
        let mut best: Option<(f64, f64, f64)> = None;
        for m in lo.min(last)..hi.min(last) {
            let (a, b) = (&self.sections[m], &self.sections[m + 1]);
            if skip.is_some_and(|s| (a.s - s).abs() <= self.apart() || (b.s - s).abs() <= self.apart()) {
                continue;
            }
            let ab = b.at - a.at;
            let len2 = ab.dot(ab);
            let free = if len2 > 0.0 { (q - a.at).dot(ab) / len2 } else { 0.0 };
            let distance = (q - (a.at + ab * free.clamp(0.0, 1.0))).len();
            if best.is_none_or(|(_, _, d)| distance < d) {
                // Nearest past an end of the chain, it's along the middle
                // run on straight.
                let beyond = (m == 0 && free < 0.0) || (m + 1 == last && free > 1.0);
                let t = if beyond { free } else { free.clamp(0.0, 1.0) };
                let p = a.at + ab * t;
                // How far to the left, all told: past the end of the
                // sections looked at, it's further than straight across.
                let left = a.left.lerp(b.left, t.clamp(0.0, 1.0));
                let w = a.w + (b.w - a.w) * t.clamp(0.0, 1.0);
                let across = (q - p).len() / w;
                let y = if (q - p).dot(left) < 0.0 { -across } else { across };
                best = Some((a.s + (b.s - a.s) * t, y, distance));
            }
        }
        best
    }

    /// The nearest `q` comes to the middle of sections `lo..=hi`, as in
    /// `locate` but stopping at the ends.
    #[cfg(test)]
    fn distance(&self, q: V, lo: usize, hi: usize, skip: Option<f64>) -> f64 {
        let last = self.last();
        let mut best = f64::INFINITY;
        for m in lo.min(last)..hi.min(last) {
            let (a, b) = (&self.sections[m], &self.sections[m + 1]);
            if skip.is_some_and(|s| (a.s - s).abs() <= self.apart() || (b.s - s).abs() <= self.apart()) {
                continue;
            }
            let ab = b.at - a.at;
            let len2 = ab.dot(ab);
            let t = if len2 > 0.0 { ((q - a.at).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
            best = best.min((q - (a.at + ab * t)).len());
        }
        best
    }

    /// Where the rules stop at each end, along the chain: where they meet
    /// the end's shape.
    fn rule_span(&self) -> (f64, f64) {
        let (h, i) = (self.half, self.inset);
        let back = |end: Option<RibbonEnd>, d: f64| match end {
            Some(RibbonEnd::Swallowtail) => d * i / h,
            Some(RibbonEnd::Pointed) => d * (h - i) / h,
            Some(RibbonEnd::Claw) => d * CLAW_RULES.powf(1.0 / CLAW_TAPER),
            _ => 0.0,
        };
        (back(self.ends[0], self.depth[0]), self.length - back(self.ends[1], self.depth[1]))
    }

    /// The edge's offset from the middle, `side` (1 or -1) of it, `s` along:
    /// the half width, but narrowing to a point.
    fn edge(&self, s: f64, side: f64) -> f64 {
        let mut y = self.half;
        for (end, from_end, d) in [(self.ends[1], self.length - s, self.depth[1]), (self.ends[0], s, self.depth[0])] {
            if end == Some(RibbonEnd::Pointed) && from_end < d {
                y = y.min(self.half * from_end.max(0.0) / d);
            }
        }
        side * y
    }

    /// The outline of the ribbon from `a` to `b` along it, as `(s, y,
    /// corner)`: up its left edge, round the end at `b` (shaped if it's the
    /// chain's end, square if not), back down its right edge, and round
    /// the end at `a`, back to the first point, which isn't repeated.
    fn outline(&self, a: f64, b: f64) -> Vec<(f64, f64, bool)> {
        let step = self.half * 2.0 / SECTIONS_PER_WIDTH;
        // The places along the chain from a to b: its sections, and the
        // shoulders of any points.
        let mut along: Vec<f64> = vec![a];
        along.extend(self.sections.iter().map(|sec| sec.s).filter(|&s| s > a + 1e-9 && s < b - 1e-9));
        for (k, end) in self.ends.iter().enumerate() {
            if *end == Some(RibbonEnd::Pointed) {
                let s = if k == 0 { self.depth[0] } else { self.length - self.depth[1] };
                if s > a && s < b {
                    along.push(s);
                }
            }
        }
        along.push(b);
        along.sort_by(f64::total_cmp);
        along.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
        let is_shoulder = |s: f64| {
            (self.ends[0] == Some(RibbonEnd::Pointed) && (s - self.depth[0]).abs() < 1e-9)
                || (self.ends[1] == Some(RibbonEnd::Pointed) && (s - (self.length - self.depth[1])).abs() < 1e-9)
        };
        // Points across from `from` to `to` (both `(s, y)`), not including
        // `from`, each a corner if it's `to` and that's a corner.
        let across = |out: &mut Vec<(f64, f64, bool)>, from: (f64, f64), to: (f64, f64), corner: bool| {
            let length = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
            let pieces = (length / step).ceil().max(1.0) as usize;
            for k in 1..=pieces {
                let t = k as f64 / pieces as f64;
                out.push((from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t, corner && k == pieces));
            }
        };
        let end_at = |s: f64| if (s - self.length).abs() < 1e-9 { Some(1) } else if s.abs() < 1e-9 { Some(0) } else { None };
        // The end at `s`, from the edge on one side to the other.
        let cap = |out: &mut Vec<(f64, f64, bool)>, s: f64, from_side: f64| {
            let inward = if end_at(s) == Some(0) { 1.0 } else { -1.0 };
            let (end, d) = match end_at(s) {
                Some(k) => (self.ends[k], self.depth[k]),
                None => (None, 0.0),
            };
            let start = (s, self.edge(s, from_side));
            match end {
                Some(RibbonEnd::Swallowtail) => {
                    let notch = (s + inward * d, 0.0);
                    across(out, start, notch, true);
                    across(out, notch, (s, self.edge(s, -from_side)), true);
                }
                // The point is where both edges meet.
                Some(RibbonEnd::Pointed | RibbonEnd::Claw) => {}
                _ => across(out, start, (s, self.edge(s, -from_side)), true),
            }
        };
        let mut out: Vec<(f64, f64, bool)> = Vec::new();
        for (i, &s) in along.iter().enumerate() {
            let corner = is_shoulder(s) || i == 0 || i == along.len() - 1;
            out.push((s, self.edge(s, 1.0), corner));
        }
        cap(&mut out, b, 1.0);
        for (i, &s) in along.iter().enumerate().rev() {
            // The end at b is already there, from the cap (or the tip of a
            // point, from the other side), and so is a point's tip at a.
            let tip = self.edge(s, 1.0).abs() < 1e-9 || (s.abs() < 1e-9 && self.ends[0] == Some(RibbonEnd::Claw));
            if i == along.len() - 1 || (i == 0 && tip) {
                continue;
            }
            out.push((s, self.edge(s, -1.0), is_shoulder(s) || i == 0 || i == along.len() - 1));
        }
        cap(&mut out, a, -1.0);
        // The cap ends back at the first point.
        if out.len() > 1 {
            let (s0, y0, _) = out[0];
            let (s1, y1, _) = out[out.len() - 1];
            if (s0 - s1).abs() < 1e-9 && (y0 - y1).abs() < 1e-9 {
                out.pop();
            }
        }
        out
    }

}

/// Where a crossing runs along one of its two ribbons: sections `lo..=hi`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Strand {
    chain: usize,
    lo: usize,
    hi: usize,
}

#[derive(Clone, Debug)]
struct Crossing {
    strands: [Strand; 2],
    /// Which strand goes over (at a joint, neither does).
    over: usize,
    /// Whether it's where ribbons join rather than cross: a branch's start
    /// and the ribbon it leaves, or the starts of two branches leaving
    /// together.
    joint: Joint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Joint {
    None,
    /// A branch's start, joining the ribbon it leaves, strand `parent`.
    Join { parent: usize },
    /// The starts of two branches leaving a ribbon at the same place.
    Siblings,
}

impl Crossing {
    /// At a joint, the strand drawn where the two run together: the ribbon
    /// a branch leaves, or the first of two branches.
    fn first(&self) -> Option<usize> {
        match self.joint {
            Joint::None => None,
            Joint::Join { parent } => Some(parent),
            Joint::Siblings => Some(usize::from(self.strands[1].chain < self.strands[0].chain)),
        }
    }
}

/// Where ribbons pile up, three or more over one place (two joined
/// ribbons counting as one): its crossings, with the layers of the pile
/// each's strands are in, numbered from 0, and how many layers.
struct Knot {
    crossings: Vec<usize>,
    ends: Vec<[usize; 2]>,
    layers: usize,
}

/// Where two joined ribbons' lines run together, how far (in widths) one
/// must stray inside the other's for it to be hidden.
const TIE: f64 = 0.02;
/// How near (in widths) the ends of joined ribbons' lines must be to be
/// drawn as one line.
const STITCH: f64 = 0.2;
/// How sharply (in radians) lines that meet must turn to meet at a corner.
const TURN: f64 = 0.26;
/// How far (in widths) a branch runs along the ribbon it leaves before
/// turning its own way, about.
const JOIN_WIDTHS: f64 = 1.0;

/// A line of a ribbon to set text along: its stretches nothing passes over,
/// in the order they're read, and the band between its rules over them,
/// which the text stays inside.
pub struct TextLine {
    /// Each stretch's line, and whether its glyphs are turned right round
    /// to read right way up along it.
    pub stretches: Vec<(StrokePath, bool)>,
    pub region: Region,
    /// How long its longest stretch is.
    pub longest: f32,
}

/// Trees woven into ribbons.
pub struct Ribbons {
    chains: Vec<Chain>,
    crossings: Vec<Crossing>,
    /// For each chain, the crossings it's in, and which strand it is.
    passes: Vec<Vec<(usize, usize)>>,
    /// For each chain, whether its text runs from its end back to its
    /// start, to read right way up.
    reversed: Vec<bool>,
    trees: usize,
    upright: bool,
}

/// Weave trees into ribbons. Fails if an edge is out of range or closes a
/// loop, a point isn't finite, or a width or setting is out of range.
pub fn weave(trees: &[RibbonTree], settings: &RibbonSettings) -> Result<Ribbons, String> {
    let variation = settings.width_variation as f64;
    if !(0.0..=MOST_VARIATION).contains(&variation) {
        return Err(format!("'width_variation' must be from 0 to {}, got {}", MOST_VARIATION, settings.width_variation));
    }
    let wander = settings.wander as f64;
    if !(wander >= 0.0 && wander.is_finite()) {
        return Err(format!("'wander' must be at least 0, got {}", settings.wander));
    }
    let seed = settings.seed;
    let mut chains: Vec<Chain> = Vec::new();
    // Each chain's middle before it wandered, and where each point of it
    // went, to find how far a branch's parent moved where it leaves it.
    let mut unmoved: Vec<Vec<(V, V)>> = Vec::new();
    let mut total = 0usize;
    for (t, tree) in trees.iter().enumerate() {
        let width = tree.width as f64;
        if !(width > 0.0 && width.is_finite()) {
            return Err(format!("tree {}'s width must be positive, got {}", t + 1, tree.width));
        }
        let half = width / 2.0;
        let inset = settings.rule_inset.map_or(width / 9.0, |i| i as f64);
        if !(inset >= 0.0 && inset < half) {
            return Err(format!("'rule_inset' must be at least 0 and less than half the width ({}), got {}", half, inset));
        }
        let depth = settings.end_depth.map_or(settings.ends.depth() * width, |d| d as f64);
        if !(depth >= 0.0 && depth.is_finite()) {
            return Err(format!("'end_depth' must be at least 0, got {}", depth));
        }
        let points: Vec<V> = tree.points.iter().map(|&p| V::from(p)).collect();
        if let Some(k) = points.iter().position(|p| !p.is_finite()) {
            return Err(format!("tree {}'s point {} isn't finite", t + 1, k));
        }
        let step = width / SECTIONS_PER_WIDTH;
        let first = chains.len();
        for branch in decompose(&points, &tree.edges).map_err(|err| format!("tree {}: {}", t + 1, err))? {
            let mut centre: Vec<V> = branch.nodes.iter().map(|&n| points[n]).collect();
            centre.dedup_by(|a, b| (*a - *b).len() < 1e-9);
            let parent = branch.parent.map(|(chain, node)| (first + chain, node));
            if centre.len() < 2 {
                // Two points in the same place: nothing to draw.
                chains.push(Chain { tree: t, sections: Vec::new(), length: 0.0, half, inset, ends: [None; 2], depth: [0.0; 2], parent: None, children: Vec::new() });
                unmoved.push(Vec::new());
                continue;
            }
            // A branch sets off along the ribbon it leaves, whichever way
            // it heads, to join it smoothly, and soon turns its own way.
            let start = parent.filter(|&(p, _)| chains[p].sections.len() >= 2).map(|(p, _)| {
                let ribbon: &Chain = &chains[p];
                let along = ribbon.heading(ribbon.nearest(centre[0]));
                let chord = centre[1] - centre[0];
                let along = if along.dot(chord) >= 0.0 { along } else { along * -1.0 };
                along * (JOIN_WIDTHS * width * 3.0 / chord.len()).min(1.0)
            });
            let along = resample(&spline_points(&centre, step / 4.0, start), step);
            let salt = chains.len() as u64;
            // Where it leaves its parent, how far the parent wandered, and
            // how much wider it is: a branch goes as it does there, then
            // settles into its own way.
            let (moved, parent_w) = match parent.filter(|&(p, _)| chains[p].sections.len() >= 2) {
                Some((p, _)) => {
                    let k = (0..unmoved[p].len()).min_by(|&i, &j| (unmoved[p][i].0 - along[0]).len().total_cmp(&(unmoved[p][j].0 - along[0]).len())).unwrap_or(0);
                    let (from, to) = unmoved[p].get(k).copied().unwrap_or_default();
                    (to - from, Some(chains[p].sections[chains[p].nearest(to)].w))
                }
                None => (V::default(), None),
            };
            let settling = if parent.is_some() { SETTLE_WIDTHS * width } else { 0.0 };
            let mut shifted: Vec<(V, V)> = along.iter().map(|&p| (p, p)).collect();
            let along = if wander > 0.0 || moved.len() > 0.0 {
                let bends = WANDER_WIDTHS * width + WANDER_STRETCH * wander;
                let n = along.len();
                let mut s = 0.0;
                let mut out = Vec::with_capacity(n);
                for i in 0..n {
                    if i > 0 {
                        s += (along[i] - along[i - 1]).len();
                    }
                    let dir = along[(i + 1).min(n - 1)] - along[i.saturating_sub(1)];
                    let left = dir.perp() * (1.0 / dir.len().max(1e-12));
                    let own = if parent.is_some() { settle(s, settling) } else { 1.0 };
                    out.push(along[i] + moved * (1.0 - own) + left * (wander * waver(seed, salt * 4 + 1, s / bends) * own));
                }
                shifted = along.iter().copied().zip(out.iter().copied()).collect();
                resample(&out, step)
            } else {
                along
            };
            let start_end = if parent.is_some() { None } else { Some(settings.ends) };
            let ends = [start_end, Some(settings.ends)];
            // A claw trails on past each end, veering off and hooking round
            // one way or the other, each its own way.
            let claws = (settings.ends == RibbonEnd::Claw && depth > 0.0 && along.len() >= 2).then(|| {
                [0, 1].map(|k| {
                    let roll = |i: i64| (seed, salt * 4 + 3, k * 8 + i);
                    let side = |i: i64| if chance(roll(i).0, roll(i).1, roll(i).2) < 0.5 { -1.0 } else { 1.0 };
                    let pick = |range: [f64; 2], i: i64| between(range, roll(i).0, roll(i).1, roll(i).2);
                    Claw {
                        turn: side(0) * pick(CLAW_TURN, 1),
                        veer: pick([-CLAW_VEER, CLAW_VEER], 2),
                        hook: pick(CLAW_HOOK, 3),
                        length: depth * pick(CLAW_LENGTH, 4),
                    }
                })
            });
            let along = match &claws {
                Some(claws) => {
                    let n = along.len();
                    let mut out = Vec::with_capacity(n + (claws[0].length + claws[1].length) as usize * 4 / step as usize);
                    if ends[0].is_some() {
                        let mut tail = claw_tail(along[0], along[0] - along[1], &claws[0], step / 4.0);
                        tail.reverse();
                        tail.pop();
                        out.extend(tail);
                    }
                    out.extend_from_slice(&along);
                    out.extend(claw_tail(along[n - 1], along[n - 1] - along[n - 2], &claws[1], step / 4.0).into_iter().skip(1));
                    resample(&out, step)
                }
                None => along,
            };
            total += along.len();
            if total > MOST_SECTIONS {
                return Err(format!("the ribbons are too long for their width: more than {} sections; make them wider or the trees smaller", MOST_SECTIONS));
            }
            let n = along.len();
            let mut s = 0.0;
            let sections: Vec<Section> = (0..n).map(|i| {
                if i > 0 {
                    s += (along[i] - along[i - 1]).len();
                }
                let dir = along[(i + 1).min(n - 1)] - along[i.saturating_sub(1)];
                let own = 1.0 + variation * waver(seed, salt * 4 + 2, s / (VARIATION_WIDTHS * width));
                let w = match parent_w {
                    Some(from) => from + (own - from) * settle(s, settling),
                    None => own,
                };
                Section { s, at: along[i], left: dir.perp() * (1.0 / dir.len().max(1e-12)), w }
            }).collect();
            let length = sections[n - 1].s;
            // Ends no deeper than a third of a short ribbon each; a claw is
            // as long as it was made, past the end.
            let deep = |k: usize| match ends[k] {
                Some(RibbonEnd::Claw) => claws.as_ref().map_or(0.0, |c| c[k].length.min(length)),
                Some(RibbonEnd::Cut) | None => 0.0,
                Some(_) => depth.min(length / 3.0),
            };
            chains.push(Chain { tree: t, sections, length, half, inset, ends, depth: [deep(0), deep(1)], parent: None, children: Vec::new() });
            unmoved.push(shifted);
            let index = chains.len() - 1;
            // Where it leaves its parent: the parent's section nearest the
            // point it starts at.
            if let Some((p, _)) = parent {
                if !chains[p].sections.is_empty() {
                    let k = chains[p].nearest(chains[index].sections[0].at);
                    chains[index].parent = Some((p, k));
                    chains[p].children.push((k, index));
                }
            }
        }
    }
    let mut ribbons = Ribbons { passes: vec![Vec::new(); chains.len()], reversed: vec![false; chains.len()], chains, crossings: Vec::new(), trees: trees.len(), upright: settings.upright };
    ribbons.find_crossings();
    ribbons.layer(settings.flip);
    Ok(ribbons)
}

/// How a claw goes: how far round it turns by its tip and how far it veers
/// off first, in radians, to the left if more than 0; how its turning
/// gathers towards the tip; and how long it is.
struct Claw {
    turn: f64,
    veer: f64,
    hook: f64,
    length: f64,
}

/// The middle of `claw`, from `from` heading `heading`, every `step` along
/// it: it veers off soon after it starts, and turns round by its tip,
/// hardly at first and more and more towards the tip.
fn claw_tail(from: V, heading: V, claw: &Claw, step: f64) -> Vec<V> {
    let pieces = (claw.length / step).ceil().max(1.0) as usize;
    let step = claw.length / pieces as f64;
    let start = heading.y.atan2(heading.x);
    // Left is a quarter turn from +x towards +y.
    let mut at = from;
    let mut out = vec![at];
    for i in 0..pieces {
        let u = (i as f64 + 0.5) / pieces as f64;
        at = at + V::angle(start + claw.veer * settle(u, CLAW_VEERED) + claw.turn * u.powf(claw.hook)) * step;
        out.push(at);
    }
    out
}

/// A chain of a tree's points: their indices in order, and the chain it
/// leaves and where (the index of its first point in that chain's).
#[derive(Debug)]
struct Branch {
    nodes: Vec<usize>,
    parent: Option<(usize, usize)>,
}

/// A tree (or forest) as chains: for each piece, its longest path, then
/// branches leaving it, each down the longest way from where it leaves,
/// and so on, in the order the text reads them: each chain, then the
/// branches leaving it in turn, each followed by its own.
fn decompose(points: &[V], edges: &[(usize, usize)]) -> Result<Vec<Branch>, String> {
    let n = points.len();
    let mut adjacent: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut joined: Vec<usize> = (0..n).collect();
    fn root(joined: &mut [usize], mut i: usize) -> usize {
        while joined[i] != i {
            joined[i] = joined[joined[i]];
            i = joined[i];
        }
        i
    }
    for &(a, b) in edges {
        if a >= n || b >= n {
            return Err(format!("edge [{}, {}] should join two of its {} points", a, b, n));
        }
        let (ra, rb) = (root(&mut joined, a), root(&mut joined, b));
        if ra == rb {
            return Err(format!("the edges should form a tree, but [{}, {}] closes a loop", a, b));
        }
        joined[ra] = rb;
        adjacent[a].push(b);
        adjacent[b].push(a);
    }
    for list in &mut adjacent {
        list.sort_unstable();
    }
    let length = |a: usize, b: usize| (points[a] - points[b]).len();
    // How far each point of a piece is from `from` along the tree, and the
    // point before it on the way.
    let reach = |from: usize| -> (Vec<(usize, f64)>, HashMap<usize, usize>) {
        let mut found = vec![(from, 0.0)];
        let mut before = HashMap::new();
        let mut k = 0;
        while k < found.len() {
            let (p, d) = found[k];
            for &q in &adjacent[p] {
                if q != *before.get(&p).unwrap_or(&usize::MAX) {
                    before.insert(q, p);
                    found.push((q, d + length(p, q)));
                }
            }
            k += 1;
        }
        (found, before)
    };
    let farthest = |found: &[(usize, f64)]| found.iter().fold(found[0], |best, &(p, d)| if d > best.1 { (p, d) } else { best }).0;

    let mut out: Vec<Branch> = Vec::new();
    let mut placed = vec![false; n];
    for &(first, _) in edges {
        if placed[first] {
            continue;
        }
        // The trunk: the farthest point from any, to the farthest from it,
        // starting from whichever end is nearer the first point.
        let (from_first, _) = reach(first);
        let a = farthest(&from_first);
        let (from_a, before) = reach(a);
        let b = farthest(&from_a);
        let mut trunk = vec![b];
        while let Some(&p) = before.get(trunk.last().expect("it has a point")) {
            trunk.push(p);
        }
        let near = |p: usize| from_first.iter().find(|&&(q, _)| q == p).map_or(0.0, |&(_, d)| d);
        if near(a) < near(b) {
            trunk.reverse();
        }
        // The piece rooted at the trunk's start: each point's parent, and
        // how far it is down to its farthest leaf, through which child.
        let (order, parent) = reach(trunk[0]);
        let mut height: HashMap<usize, f64> = HashMap::new();
        let mut tallest: HashMap<usize, usize> = HashMap::new();
        for &(p, _) in order.iter().rev() {
            let mut best: Option<(usize, f64)> = None;
            for &c in &adjacent[p] {
                if parent.get(&p) == Some(&c) {
                    continue;
                }
                let h = length(p, c) + height[&c];
                if best.is_none_or(|(_, b)| h > b) {
                    best = Some((c, h));
                }
            }
            height.insert(p, best.map_or(0.0, |(_, h)| h));
            if let Some((c, _)) = best {
                tallest.insert(p, c);
            }
        }
        // Chains depth first: a chain, then its branches in turn, each
        // with its own after it.
        let mut todo: Vec<Branch> = vec![Branch { nodes: trunk, parent: None }];
        while let Some(branch) = todo.pop() {
            let index = out.len();
            for &p in &branch.nodes {
                placed[p] = true;
            }
            let mut leaving: Vec<Branch> = Vec::new();
            for (k, &p) in branch.nodes.iter().enumerate() {
                // A branch's first point is on the chain it leaves.
                if k == 0 && branch.parent.is_some() {
                    continue;
                }
                let mut children: Vec<usize> = adjacent[p].iter().copied()
                    .filter(|&c| parent.get(&p) != Some(&c) && Some(&c) != branch.nodes.get(k + 1))
                    .collect();
                children.sort_by(|&x, &y| (length(p, y) + height[&y]).total_cmp(&(length(p, x) + height[&x])).then(x.cmp(&y)));
                for c in children {
                    let mut nodes = vec![p, c];
                    while let Some(&next) = tallest.get(nodes.last().expect("it has a point")) {
                        nodes.push(next);
                    }
                    leaving.push(Branch { nodes, parent: Some((index, k)) });
                }
            }
            out.push(branch);
            todo.extend(leaving.into_iter().rev());
        }
    }
    Ok(out)
}

/// Union-find over crossings' choices of which strand goes over, each
/// joined to another with whether they're the same or not.
struct Parity {
    parent: Vec<usize>,
    /// Whether each differs from its parent.
    differs: Vec<bool>,
}

impl Parity {
    fn new(n: usize) -> Self {
        Parity { parent: (0..n).collect(), differs: vec![false; n] }
    }

    /// The root of `x`'s set, and whether `x` differs from it.
    fn find(&mut self, x: usize) -> (usize, bool) {
        let p = self.parent[x];
        if p == x {
            return (x, false);
        }
        let (root, d) = self.find(p);
        self.differs[x] ^= d;
        self.parent[x] = root;
        (root, self.differs[x])
    }

    /// Have `a` and `b` differ (or not): false if that contradicts what's
    /// already settled.
    fn join(&mut self, a: usize, b: usize, differ: bool) -> bool {
        let ((ra, da), (rb, db)) = (self.find(a), self.find(b));
        if ra == rb {
            return (da ^ db) == differ;
        }
        self.parent[ra] = rb;
        self.differs[ra] = da ^ db ^ differ;
        true
    }
}

impl Ribbons {
    /// Group overlapping pairs of sections into crossings.
    fn find_crossings(&mut self) {
        let chains = &self.chains;
        let widest = chains.iter().filter(|c| !c.sections.is_empty()).map(|c| 2.0 * (0..c.sections.len()).map(|k| c.reach(k)).fold(0.0, f64::max) + c.half).fold(1e-9, f64::max);
        if widest <= 0.0 {
            return;
        }
        let cell = |p: V| ((p.x / widest).floor() as i64, (p.y / widest).floor() as i64);
        let mut grid: HashMap<(i64, i64), Vec<(usize, usize)>> = HashMap::new();
        for (c, chain) in chains.iter().enumerate() {
            for (k, section) in chain.sections.iter().enumerate() {
                grid.entry(cell(section.at)).or_default().push((c, k));
            }
        }
        let mut pairs: BTreeSet<(usize, usize, usize, usize)> = BTreeSet::new();
        for (c, chain) in chains.iter().enumerate() {
            let step = chain.half * 2.0 / SECTIONS_PER_WIDTH;
            for (k, a) in chain.sections.iter().enumerate() {
                let (cx, cy) = cell(a.at);
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for &(d, m) in grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                            if (d, m) <= (c, k) {
                                continue;
                            }
                            let other = &chains[d];
                            let b = &other.sections[m];
                            if d == c && (b.s - a.s).abs() <= chain.apart() {
                                continue;
                            }
                            if (a.at - b.at).len() < chain.reach(k) + other.reach(m) + step / 2.0 {
                                pairs.insert((c, k, d, m));
                            }
                        }
                    }
                }
            }
        }
        // Pairs next to each other on the same two chains belong to the
        // same crossing.
        let mut crossings = Vec::new();
        while let Some(&first) = pairs.iter().next() {
            pairs.remove(&first);
            let (c, d) = (first.0, first.2);
            let mut todo = vec![(first.1, first.3)];
            let (mut a, mut b) = ((first.1, first.1), (first.3, first.3));
            while let Some((i, j)) = todo.pop() {
                a = (a.0.min(i), a.1.max(i));
                b = (b.0.min(j), b.1.max(j));
                for di in -1..=1_isize {
                    for dj in -1..=1_isize {
                        let (ni, nj) = (i.wrapping_add_signed(di), j.wrapping_add_signed(dj));
                        if pairs.remove(&(c, ni, d, nj)) {
                            todo.push((ni, nj));
                        }
                    }
                }
            }
            let strands = [Strand { chain: c, lo: a.0, hi: a.1 }, Strand { chain: d, lo: b.0, hi: b.1 }];
            let joint = self.joint(&strands);
            crossings.push(Crossing { strands, over: 0, joint });
        }
        for (x, crossing) in crossings.iter().enumerate() {
            for (t, strand) in crossing.strands.iter().enumerate() {
                self.passes[strand.chain].push((x, t));
            }
        }
        self.crossings = crossings;
    }

    /// Whether two strands are a branch's start and the ribbon it leaves,
    /// or two branches' starts where they leave a ribbon together.
    fn joint(&self, strands: &[Strand; 2]) -> Joint {
        let chains = &self.chains;
        for t in 0..2 {
            let (branch, other) = (strands[t], strands[1 - t]);
            if branch.lo == 0 && chains[branch.chain].parent.is_some_and(|(p, _)| p == other.chain) {
                return Joint::Join { parent: 1 - t };
            }
        }
        let (a, b) = (&chains[strands[0].chain], &chains[strands[1].chain]);
        if strands[0].chain != strands[1].chain && strands[0].lo == 0 && strands[1].lo == 0 {
            if let (Some((pa, ka)), Some((pb, kb))) = (a.parent, b.parent) {
                if pa == pb && (chains[pa].sections[ka].s - chains[pa].sections[kb].s).abs() <= a.half + b.half {
                    return Joint::Siblings;
                }
            }
        }
        Joint::None
    }

    /// How far along its chain a strand's middle is.
    fn middle(&self, strand: &Strand) -> f64 {
        let sections = &self.chains[strand.chain].sections;
        (sections[strand.lo].s + sections[strand.hi].s) / 2.0
    }

    /// Choose which strand goes over at each crossing: over and under in
    /// turn along each ribbon, as far as they all can. Where they can't,
    /// the turn missed is between crossings far apart along a ribbon. Each
    /// piece of the weave starts over, or under with `flip`. Joints, where
    /// ribbons join rather than cross, aren't passes. Where ribbons pile
    /// up, each passes over or under the pile as a whole, and they're laid
    /// one over another, those passing over on top.
    fn layer(&mut self, flip: bool) {
        let count = self.crossings.len();
        let knots = self.knots();
        // What settles whether a ribbon is over at each of its passes: a
        // crossing's choice of strand (over at strand 0 if not), or the
        // choice of whether its layer of a knot is over.
        let mut layer_of: HashMap<(usize, usize), usize> = HashMap::new();
        let mut first_layer = Vec::with_capacity(knots.len());
        let mut nodes = count;
        for knot in &knots {
            first_layer.push(nodes);
            for (&x, ends) in knot.crossings.iter().zip(&knot.ends) {
                for t in 0..2 {
                    layer_of.insert((x, t), nodes + ends[t]);
                }
            }
            nodes += knot.layers;
        }
        // Over at a pass when the choice it's settled by, flipped or not,
        // is true.
        let settled = |x: usize, t: usize| -> (usize, bool) { layer_of.get(&(x, t)).map_or((x, t == 0), |&n| (n, false)) };
        let mut parity = Parity::new(nodes);
        // Passes along each chain, in order.
        let mut along: Vec<Vec<(f64, usize, usize)>> = vec![Vec::new(); self.chains.len()];
        for (x, crossing) in self.crossings.iter().enumerate() {
            if crossing.joint != Joint::None {
                continue;
            }
            for (t, strand) in crossing.strands.iter().enumerate() {
                along[strand.chain].push((self.middle(strand), x, t));
            }
        }
        // Over at one pass, under at the next; passes of one layer of a
        // knot are one.
        let mut turns: Vec<(f64, usize, usize, bool)> = Vec::new();
        for passes in &mut along {
            passes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            for w in passes.windows(2) {
                let ((s0, x, t), (s1, y, u)) = (w[0], w[1]);
                let ((a, flip_a), (b, flip_b)) = (settled(x, t), settled(y, u));
                if a != b {
                    turns.push((s1 - s0, a, b, !(flip_a ^ flip_b)));
                }
            }
        }
        turns.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        for (_, a, b, differ) in turns {
            parity.join(a, b, differ);
        }
        // Each piece of the weave nothing settles starts over at its first
        // pass, along the first chain it's on.
        let mut value: HashMap<usize, bool> = HashMap::new();
        for passes in &along {
            for &(_, x, t) in passes {
                let (n, flipped) = settled(x, t);
                let (r, d) = parity.find(n);
                value.entry(r).or_insert(!flip ^ flipped ^ d);
            }
        }
        let mut chosen = |n: usize| {
            let (r, d) = parity.find(n);
            value.get(&r).copied().unwrap_or(false) ^ d
        };
        for x in 0..count {
            if !layer_of.contains_key(&(x, 0)) {
                self.crossings[x].over = usize::from(chosen(x));
            }
        }
        // A knot's layers, those passing over first, the strand higher in
        // the pile over at each of its crossings.
        for (knot, &first) in knots.iter().zip(&first_layer) {
            let over: Vec<bool> = (0..knot.layers).map(|l| chosen(first + l)).collect();
            let mut order: Vec<usize> = (0..knot.layers).collect();
            order.sort_by_key(|&l| (!over[l], l));
            let mut height = vec![0; knot.layers];
            for (h, &l) in order.iter().enumerate() {
                height[l] = h;
            }
            for (&x, ends) in knot.crossings.iter().zip(&knot.ends) {
                self.crossings[x].over = usize::from(height[ends[1]] < height[ends[0]]);
            }
        }
        // Text reads right way up: run the other way along a trunk whose
        // visible stretches run more leftward than rightward. A branch's
        // runs on from where it leaves, outwards.
        if self.upright {
            for c in 0..self.chains.len() {
                let chain = &self.chains[c];
                if chain.sections.len() < 2 || chain.parent.is_some() {
                    continue;
                }
                let hidden = self.hidden_sections(c, 0.0);
                let rightward: f64 = chain.sections.windows(2).enumerate()
                    .filter(|(k, _)| !hidden[*k] && !hidden[*k + 1])
                    .map(|(_, w)| w[1].at.x - w[0].at.x)
                    .sum();
                self.reversed[c] = rightward < 0.0;
            }
        }
    }

    /// The crossings over each place where three or more passes of
    /// ribbons overlap, found across every crossing, each set once.
    fn piles(&self) -> BTreeSet<BTreeSet<usize>> {
        let mut found: BTreeSet<BTreeSet<usize>> = BTreeSet::new();
        let across = [-0.9, -0.6, -0.3, 0.0, 0.3, 0.6, 0.9];
        let within = |strand: &Strand, k: usize| k + PAD >= strand.lo && k <= strand.hi + PAD;
        for crossing in self.crossings.iter().filter(|x| x.joint == Joint::None) {
            let a = crossing.strands[0];
            let chain = &self.chains[a.chain];
            for k in a.lo..=a.hi {
                let sec = &chain.sections[k];
                for f in across {
                    let p = chain.frame(sec.s, f * chain.half);
                    // The passes over p, as chain and section: a's, and
                    // those of whatever crosses or joins it there.
                    let mut passes: Vec<(usize, usize)> = vec![(a.chain, k)];
                    for &(y, t) in &self.passes[a.chain] {
                        let cy = &self.crossings[y];
                        if !within(&cy.strands[t], k) {
                            continue;
                        }
                        let other = cy.strands[1 - t];
                        let on = &self.chains[other.chain];
                        let skip = (other.chain == a.chain).then_some(sec.s);
                        let Some((s, y, _)) = on.locate(p, other.lo.saturating_sub(PAD + 2), other.hi + PAD + 2, skip) else { continue };
                        let seen = passes.iter().any(|&(c, m)| c == other.chain && (self.chains[c].sections[m].s - s).abs() <= on.apart());
                        if on.holds(s, y, on.half) && !seen {
                            passes.push((other.chain, on.section_at(s)));
                        }
                    }
                    if passes.len() < 3 {
                        continue;
                    }
                    // The crossings between them.
                    let mut pile = BTreeSet::new();
                    for i in 0..passes.len() {
                        for j in i + 1..passes.len() {
                            let ((ci, ki), (cj, kj)) = (passes[i], passes[j]);
                            let link = self.passes[ci].iter().find(|&&(z, t)| {
                                let cz = &self.crossings[z];
                                within(&cz.strands[t], ki) && cz.strands[1 - t].chain == cj && within(&cz.strands[1 - t], kj)
                            });
                            if let Some(&(z, _)) = link {
                                pile.insert(z);
                            }
                        }
                    }
                    if pile.len() >= 2 {
                        found.insert(pile);
                    }
                }
            }
        }
        found
    }

    /// Where ribbons pile up: piles that share a crossing, together.
    fn knots(&self) -> Vec<Knot> {
        let n = self.crossings.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn root(parent: &mut [usize], x: usize) -> usize {
            let mut r = x;
            while parent[r] != r {
                r = parent[r];
            }
            parent[x] = r;
            r
        }
        let mut piled = vec![false; n];
        for pile in self.piles() {
            let first = *pile.first().expect("a pile has crossings");
            for x in pile {
                piled[x] = true;
                let (a, b) = (root(&mut parent, first), root(&mut parent, x));
                parent[a] = b;
            }
        }
        let mut members: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for x in 0..n {
            if piled[x] {
                members.entry(root(&mut parent, x)).or_default().push(x);
            }
        }
        members.into_values().map(|crossings| self.knot(&crossings)).collect()
    }

    /// A knot of these crossings: their strands in layers, those along the
    /// same stretch of a ribbon one, and those a joint joins one.
    fn knot(&self, all: &[usize]) -> Knot {
        let strands: Vec<(usize, usize)> = all.iter().flat_map(|&x| [(x, 0), (x, 1)]).collect();
        let mut layer: Vec<usize> = (0..strands.len()).collect();
        for i in 0..strands.len() {
            for j in i + 1..strands.len() {
                let ((x, t), (y, u)) = (strands[i], strands[j]);
                let (a, b) = (self.crossings[x].strands[t], self.crossings[y].strands[u]);
                let together = a.chain == b.chain && a.lo <= b.hi + PAD && b.lo <= a.hi + PAD;
                let joined = x == y && self.crossings[x].joint != Joint::None;
                if together || joined {
                    let (from, to) = (layer[i], layer[j]);
                    for l in &mut layer {
                        if *l == from {
                            *l = to;
                        }
                    }
                }
            }
        }
        let mut names: Vec<usize> = Vec::new();
        let mut knot = Knot { crossings: Vec::new(), ends: Vec::new(), layers: 0 };
        for (i, &x) in all.iter().enumerate() {
            if self.crossings[x].joint != Joint::None || layer[2 * i] == layer[2 * i + 1] {
                continue;
            }
            let ends = [layer[2 * i], layer[2 * i + 1]].map(|l| {
                names.iter().position(|&n| n == l).unwrap_or_else(|| {
                    names.push(l);
                    names.len() - 1
                })
            });
            knot.crossings.push(x);
            knot.ends.push(ends);
        }
        knot.layers = names.len();
        knot
    }

    /// A ring among a knot's layers, if there's one: the crossings round
    /// it.
    #[cfg(test)]
    fn ring(&self, knot: &Knot) -> Option<Vec<usize>> {
        let n = knot.layers;
        // Each layer, and those under it, with the crossing where.
        let mut under: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
        for (&x, ends) in knot.crossings.iter().zip(&knot.ends) {
            let over = self.crossings[x].over;
            under[ends[over]].push((ends[1 - over], x));
        }
        fn visit(at: usize, under: &[Vec<(usize, usize)>], state: &mut [u8], path: &mut Vec<usize>, via: &mut Vec<usize>) -> Option<Vec<usize>> {
            state[at] = 1;
            path.push(at);
            for &(next, x) in &under[at] {
                if state[next] == 1 {
                    let i = path.iter().position(|&l| l == next).expect("on the way");
                    let mut ring = via[i..].to_vec();
                    ring.push(x);
                    return Some(ring);
                }
                if state[next] == 0 {
                    via.push(x);
                    if let Some(ring) = visit(next, under, state, path, via) {
                        return Some(ring);
                    }
                    via.pop();
                }
            }
            path.pop();
            state[at] = 2;
            None
        }
        let mut state = vec![0u8; n];
        (0..n).find_map(|layer| if state[layer] == 0 { visit(layer, &under, &mut state, &mut Vec::new(), &mut Vec::new()) } else { None })
    }

    /// Whether `q`, on chain `c` by section `k`, is hidden: under a ribbon
    /// passing over it there, or inside one it joins. Where ribbons join,
    /// each hides what of the other's edges is inside its edges and, for
    /// a `rule`, what of the other's rules is inside its rules, so their
    /// outlines and rules fork.
    fn hidden(&self, c: usize, k: usize, q: V, rule: bool) -> bool {
        let own = &self.chains[c];
        self.passes[c].iter().any(|&(x, t)| {
            let crossing = &self.crossings[x];
            let mine = crossing.strands[t];
            if k + PAD < mine.lo || k > mine.hi + PAD {
                return false;
            }
            let (other, within) = match crossing.first() {
                None if crossing.over == t => return false,
                None => (crossing.strands[crossing.over], None),
                Some(first) => {
                    let other = crossing.strands[1 - t];
                    let chain = &self.chains[other.chain];
                    // Where they run together, the first is drawn.
                    let tie = TIE * chain.half * 2.0 * if first == t { -1.0 } else { 1.0 };
                    (other, Some(if rule { chain.half - chain.inset } else { chain.half } + tie))
                }
            };
            let chain = &self.chains[other.chain];
            let skip = (other.chain == c).then(|| own.sections[k].s);
            chain.locate(q, other.lo.saturating_sub(PAD + 2), other.hi + PAD + 2, skip)
                .is_some_and(|(s, y, _)| chain.holds(s, y, within.unwrap_or(chain.half)))
        })
    }

    /// Where the line from a visible point to a hidden one (on chain `c`
    /// by section `k`) meets the edge of what hides it.
    fn edge(&self, c: usize, k: usize, visible: V, hidden: V, rule: bool) -> V {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if self.hidden(c, k, visible.lerp(hidden, mid), rule) { hi = mid } else { lo = mid }
        }
        visible.lerp(hidden, (lo + hi) / 2.0)
    }

    /// The visible runs of a line on chain `c` (a rule, or an edge), its
    /// points `(q, section, corner)`; a closed line's runs wrap round its
    /// end. Each run is its points and which are corners, the curve not to
    /// be smoothed there.
    fn runs(&self, c: usize, points: &[(V, usize, bool)], closed: bool, rule: bool) -> Vec<Vec<(V, bool)>> {
        let n = points.len();
        if n < 2 {
            return Vec::new();
        }
        let hidden: Vec<bool> = points.iter().map(|&(q, k, _)| self.hidden(c, k, q, rule)).collect();
        let start = match hidden.iter().position(|&h| h) {
            None => {
                let mut run: Vec<(V, bool)> = points.iter().map(|&(q, _, corner)| (q, corner)).collect();
                if closed {
                    run.push(run[0]);
                }
                return vec![run];
            }
            Some(first) => if closed { first } else { 0 },
        };
        let mut runs = Vec::new();
        let mut run: Vec<(V, bool)> = Vec::new();
        if !closed && !hidden[0] {
            run.push((points[0].0, points[0].2));
        }
        let steps = if closed { n } else { n - 1 };
        for step in 1..=steps {
            let (before, i) = ((start + step - 1) % n, (start + step) % n);
            let (q, k, corner) = points[i];
            match (hidden[before], hidden[i]) {
                (true, false) => {
                    run.push((self.edge(c, points[before].1, q, points[before].0, rule), false));
                    run.push((q, corner));
                }
                (false, false) => run.push((q, corner)),
                (false, true) => {
                    run.push((self.edge(c, k, points[before].0, q, rule), false));
                    runs.push(std::mem::take(&mut run));
                }
                (true, true) => {}
            }
        }
        if !run.is_empty() {
            runs.push(run);
        }
        let speck = 0.08 * self.chains[c].half * 2.0;
        runs.retain(|run| run.len() > 1 && run.windows(2).map(|w| (w[1].0 - w[0].0).len()).sum::<f64>() >= speck);
        runs
    }

    /// Runs drawn as curves, smooth between their corners, one stroke each.
    fn draw(&self, mut path: StrokePath, runs: Vec<Vec<(V, bool)>>, width: f64) -> StrokePath {
        for run in runs {
            let mut piece = vec![run[0].0];
            let mut started = false;
            for (i, &(q, corner)) in run.iter().enumerate().skip(1) {
                piece.push(q);
                if corner || i == run.len() - 1 {
                    path = spline(path, &simplify(&piece, 0.005 * width, 0.6 * width), started);
                    started = true;
                    piece = vec![q];
                }
            }
        }
        path
    }

    /// The chains of a tree.
    fn chains_of(&self, tree: usize) -> impl Iterator<Item = (usize, &Chain)> {
        self.chains.iter().enumerate().filter(move |(_, c)| c.tree == tree && c.sections.len() >= 2)
    }

    /// Points of chain `c` on the page from `(s, y, corner)`, each with its
    /// nearest section. Edge points that a tight bend folds back are left
    /// out, leaving a corner.
    fn placed(&self, c: usize, outline: &[(f64, f64, bool)]) -> Vec<(V, usize, bool)> {
        let chain = &self.chains[c];
        let mut out: Vec<(V, usize, bool)> = Vec::new();
        let mut last: Option<(f64, f64)> = None;
        for &(s, y, corner) in outline {
            let q = chain.frame(s, y);
            let k = chain.section_at(s);
            // Along an edge (the same y, moving along), a point behind the
            // one before has been folded back by a bend.
            if let (Some((ls, ly)), Some(&(prev, _, _))) = (last, out.last()) {
                if (ly - y).abs() < 1e-9 && (s - ls).abs() > 1e-9 {
                    let forward = chain.sections[k].left.perp() * -(s - ls).signum();
                    if (q - prev).dot(forward) < 0.0 && !corner {
                        last = Some((s, y));
                        if let Some(prev) = out.last_mut() {
                            prev.2 = true;
                        }
                        continue;
                    }
                }
            }
            out.push((q, k, corner));
            last = Some((s, y));
        }
        out
    }

    /// The visible edges and ends of a tree's ribbons.
    pub fn lines(&self, tree: usize) -> StrokePath {
        let mut runs = Vec::new();
        let mut width = 0.0;
        for (c, chain) in self.chains_of(tree) {
            let points = self.placed(c, &chain.outline(0.0, chain.length));
            runs.extend(self.runs(c, &points, true, false).into_iter().map(|run| (run, c, c)));
            width = chain.half * 2.0;
        }
        self.draw(StrokePath::new(), self.stitch(runs, width), width)
    }

    /// The visible rules inside the edges of a tree's ribbons.
    pub fn rules(&self, tree: usize) -> StrokePath {
        let mut runs = Vec::new();
        let mut width = 0.0;
        for (c, chain) in self.chains_of(tree) {
            width = chain.half * 2.0;
            let (a, b) = chain.rule_span();
            if b <= a {
                continue;
            }
            for side in [1.0, -1.0] {
                let y = side * (chain.half - chain.inset);
                let mut line: Vec<(f64, f64, bool)> = vec![(a, y, false)];
                line.extend(chain.sections.iter().map(|sec| sec.s).filter(|&s| s > a + 1e-9 && s < b - 1e-9).map(|s| (s, y, false)));
                line.push((b, y, false));
                let points = self.placed(c, &line);
                runs.extend(self.runs(c, &points, false, true).into_iter().map(|run| (run, c, c)));
            }
        }
        self.draw(StrokePath::new(), self.stitch(runs, width), width)
    }

    /// Runs of line that meet end to end where joined ribbons' lines hand
    /// over from one to the other, made one: running on through the
    /// meeting, or meeting at a corner where their lines cross. Each run
    /// comes with the chains its start and its end are on; one whose ends
    /// meet is closed.
    fn stitch(&self, mut runs: Vec<(Vec<(V, bool)>, usize, usize)>, width: f64) -> Vec<Vec<(V, bool)>> {
        let joined: BTreeSet<(usize, usize)> = self.crossings.iter()
            .filter(|x| x.joint != Joint::None)
            .flat_map(|x| {
                let (a, b) = (x.strands[0].chain, x.strands[1].chain);
                [(a, b), (b, a)]
            })
            .collect();
        let near = STITCH * width;
        let open = |run: &[(V, bool)]| run.len() > 1 && (run[0].0 - run[run.len() - 1].0).len() > 1e-9;
        let end = |r: &(Vec<(V, bool)>, usize, usize), at_end: bool| if at_end { (r.0[r.0.len() - 1].0, r.2) } else { (r.0[0].0, r.1) };
        // Where a line coming in at `p` heading `u` meets one going out
        // from `q` heading `v`, and whether that's a corner.
        let meet = |p: V, u: V, q: V, v: V| {
            let cross = |a: V, b: V| a.x * b.y - a.y * b.x;
            if u.dot(v) < TURN.cos() && cross(u, v).abs() > 1e-6 {
                let x = p + u * (cross(q - p, v) / cross(u, v));
                if (x - p).len() < 2.0 * near && (x - q).len() < 2.0 * near {
                    return (x, true);
                }
            }
            ((p + q) * 0.5, u.dot(v) < TURN.cos())
        };
        let unit = |v: V| v * (1.0 / v.len().max(1e-12));
        loop {
            // The nearest two ends to meet: of two runs, or one's own.
            let mut best: Option<(f64, usize, bool, usize, bool)> = None;
            for i in 0..runs.len() {
                if !open(&runs[i].0) {
                    continue;
                }
                for j in i..runs.len() {
                    if !open(&runs[j].0) {
                        continue;
                    }
                    let ends: &[(bool, bool)] = if i == j { &[(true, false)] } else { &[(true, false), (true, true), (false, false), (false, true)] };
                    for &(at_end_i, at_end_j) in ends {
                        let ((p, cp), (q, cq)) = (end(&runs[i], at_end_i), end(&runs[j], at_end_j));
                        let d = (p - q).len();
                        if d < near && joined.contains(&(cp, cq)) && best.is_none_or(|b| d < b.0) {
                            best = Some((d, i, at_end_i, j, at_end_j));
                        }
                    }
                }
            }
            let Some((_, i, at_end_i, j, at_end_j)) = best else { break };
            if i == j {
                let run = &mut runs[i].0;
                let n = run.len();
                let (p, q) = (run[n - 1].0, run[0].0);
                let at = meet(p, unit(p - run[n - 2].0), q, unit(run[1].0 - q));
                run[0] = at;
                run[n - 1] = at;
                continue;
            }
            let (mut b, b_start, b_end) = runs.remove(j);
            let (mut a, a_start, a_end) = runs.remove(i);
            // `a` ends where `b` starts.
            let start = if at_end_i { a_start } else { a.reverse(); a_end };
            let finish = if at_end_j { b.reverse(); b_start } else { b_end };
            let (p, q) = (a[a.len() - 1].0, b[0].0);
            let at = meet(p, unit(p - a[a.len() - 2].0), q, unit(b[1].0 - q));
            a.pop();
            a.push(at);
            a.extend(b.into_iter().skip(1));
            runs.push((a, start, finish));
        }
        runs.into_iter().map(|(run, _, _)| run).collect()
    }

    /// The whole outlines of a tree's ribbons, hidden parts and all, each a
    /// closed subpath, wound alike: a ribbon that crosses itself is cut
    /// between the crossing strands, so no piece overlaps itself there.
    pub fn outline(&self, tree: usize) -> StrokePath {
        let mut path = StrokePath::new();
        for (c, chain) in self.chains_of(tree) {
            let mut cuts: Vec<f64> = self.passes[c].iter().filter_map(|&(x, t)| {
                let crossing = &self.crossings[x];
                let (a, b) = (crossing.strands[0], crossing.strands[1]);
                (t == 0 && a.chain == b.chain).then(|| (chain.sections[a.hi].s + chain.sections[b.lo].s) / 2.0)
            }).collect();
            cuts.push(0.0);
            cuts.push(chain.length);
            cuts.sort_by(f64::total_cmp);
            cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            for w in cuts.windows(2) {
                let points: Vec<V> = chain.outline(w[0], w[1]).iter().map(|&(s, y, _)| chain.frame(s, y)).collect();
                let points = simplify(&points, 0.002 * chain.half, chain.half);
                let mut iter = points.iter();
                let Some(first) = iter.next() else { continue };
                path = path.move_to(first.x as f32, first.y as f32);
                for p in iter {
                    path = path.line_to(p.x as f32, p.y as f32);
                }
                path = path.close_path();
            }
        }
        path
    }

    /// Which of chain `c`'s sections text mustn't go on: where the band
    /// between its rules (the band between its edges, for `pad` 0) comes
    /// within `pad` of a ribbon passing over it.
    fn hidden_sections(&self, c: usize, pad: f64) -> Vec<bool> {
        let own = &self.chains[c];
        let mut hidden = vec![false; own.sections.len()];
        let room = if pad > 0.0 { own.half - own.inset } else { own.half };
        let across = [-1.0, -0.5, 0.0, 0.5, 1.0];
        let near = |k: usize, over: &Strand| {
            let chain = &self.chains[over.chain];
            let skip = (over.chain == c).then(|| own.sections[k].s);
            let reach = PAD + ((own.half + chain.half + pad) * 3.0 / (chain.half * 2.0 / SECTIONS_PER_WIDTH)).ceil() as usize;
            let sec = &own.sections[k];
            across.iter().any(|f| {
                let q = own.frame(sec.s, f * room);
                chain.locate(q, over.lo.saturating_sub(reach), over.hi + reach, skip).is_some_and(|(s, y, _)| chain.near(s, y, pad))
            })
        };
        let most = 4 * SECTIONS_PER_WIDTH as usize;
        for &(x, t) in &self.passes[c] {
            let crossing = &self.crossings[x];
            let other = match crossing.joint {
                Joint::None if crossing.over == t => continue,
                Joint::None => crossing.over,
                // Joined, a branch's text keeps off the ribbon it leaves,
                // and two branches' off each other.
                Joint::Join { parent } if parent == t => continue,
                _ => 1 - t,
            };
            let (under, over) = (crossing.strands[t], crossing.strands[other]);
            for flag in &mut hidden[under.lo..=under.hi] {
                *flag = true;
            }
            // Out from the crossing until the band is clear of it.
            let mut k = under.lo;
            while k > 0 && under.lo - k < most && near(k - 1, &over) {
                k -= 1;
                hidden[k] = true;
            }
            let mut k = under.hi;
            while k < own.last() && k - under.hi < most && near(k + 1, &over) {
                k += 1;
                hidden[k] = true;
            }
        }
        hidden
    }

    /// The lines a tree's text goes along, in the order it reads them:
    /// each chain's stretches that nothing passes over, at least `pad`
    /// from anything that does and from its shaped ends, with the band
    /// `room` either side of the middle for the glyphs' ink. The lines run
    /// `lift` up the glyphs' way from the middle, for glyphs whose ink
    /// is that far below the line they're set on.
    pub fn text_lines(&self, tree: usize, room: f64, pad: f64, lift: f64) -> Vec<TextLine> {
        let mut order = Vec::new();
        let mut todo: Vec<usize> = self.chains_of(tree).filter(|(_, c)| c.parent.is_none()).map(|(i, _)| i).collect();
        todo.reverse();
        while let Some(c) = todo.pop() {
            order.push(c);
            let mut children = self.chains[c].children.clone();
            children.sort();
            if self.reversed[c] {
                children.reverse();
            }
            todo.extend(children.into_iter().rev().map(|(_, child)| child).filter(|&ch| self.chains[ch].sections.len() >= 2));
        }
        order.into_iter().map(|c| self.text_line(c, room, pad, lift)).collect()
    }

    fn text_line(&self, c: usize, room: f64, pad: f64, lift: f64) -> TextLine {
        let chain = &self.chains[c];
        let hidden = self.hidden_sections(c, pad);
        let (from, to) = (
            if chain.ends[0].is_some() { chain.depth[0] + pad } else { pad },
            chain.length - chain.depth[1] - pad,
        );
        let mut stretches: Vec<Vec<usize>> = Vec::new();
        let mut current: Vec<usize> = Vec::new();
        for (k, section) in chain.sections.iter().enumerate() {
            if hidden[k] || section.s < from || section.s > to {
                if current.len() > 1 {
                    stretches.push(std::mem::take(&mut current));
                }
                current.clear();
            } else {
                current.push(k);
            }
        }
        if current.len() > 1 {
            stretches.push(current);
        }
        // Upright, a stretch that turns back on itself is set in pieces,
        // each heading one way.
        if self.upright {
            stretches = stretches
                .into_iter()
                .flat_map(|stretch| turns(chain, stretch, TURN_WIDTHS * 2.0 * chain.half, chain.half))
                .flat_map(|piece| forward(chain, piece, room))
                .collect();
        }
        // Stretches in the order the chain is read. Upright, each reads
        // right way up: a trunk's set the way along it that does, a
        // branch's always outwards from where it leaves, its glyphs turned
        // right round where it heads leftward.
        if self.reversed[c] {
            stretches.reverse();
        }
        let mut flipped = vec![false; stretches.len()];
        for (stretch, flip) in stretches.iter_mut().zip(&mut flipped) {
            let rightward = chain.sections[stretch[stretch.len() - 1]].at.x - chain.sections[stretch[0]].at.x;
            if self.upright && rightward < 0.0 {
                if chain.parent.is_some() {
                    *flip = true;
                } else {
                    stretch.reverse();
                }
            }
        }
        let mut paths = Vec::new();
        let mut rings = Vec::new();
        let mut longest: f64 = 0.0;
        for (stretch, &flip) in stretches.iter().zip(&flipped) {
            let sections: Vec<&Section> = stretch.iter().map(|&k| &chain.sections[k]).collect();
            longest = longest.max((sections[sections.len() - 1].s - sections[0].s).abs());
            // The line moved up the glyphs' way by `lift`, their down
            // being the path's normal, or the other way turned round.
            let down = if (stretch[0] < stretch[stretch.len() - 1]) != flip { 1.0 } else { -1.0 };
            let mut path = StrokePath::new();
            for (i, sec) in sections.iter().enumerate() {
                let p = sec.at - sec.left * (down * lift);
                path = if i == 0 { path.move_to(p.x as f32, p.y as f32) } else { path.line_to(p.x as f32, p.y as f32) };
            }
            paths.push((path, flip));
            let mut ring: Vec<PVec> = sections.iter().map(|sec| sec.at + sec.left * room).map(|p| PVec::new(p.x as f32, p.y as f32)).collect();
            ring.extend(sections.iter().rev().map(|sec| sec.at - sec.left * room).map(|p| PVec::new(p.x as f32, p.y as f32)));
            rings.push(ring);
        }
        TextLine { stretches: paths, region: Region::new(rings), longest: longest as f32 }
    }

    /// How many trees there are.
    pub fn trees(&self) -> usize {
        self.trees
    }

    /// The narrowest a tree's ribbons' bands between their rules are, half
    /// across.
    pub fn room(&self, tree: usize) -> f64 {
        self.chains_of(tree)
            .map(|(_, c)| (c.half - c.inset) * c.sections.iter().filter(|sec| c.claw(sec.s).is_none()).map(|sec| sec.w).fold(f64::INFINITY, f64::min).min(1.0 + MOST_VARIATION))
            .fold(f64::INFINITY, f64::min)
    }

    /// How far apart a tree's sections are.
    pub fn step(&self, tree: usize) -> f64 {
        self.chains_of(tree).map(|(_, c)| c.half * 2.0 / SECTIONS_PER_WIDTH).fold(0.0, f64::max)
    }
}

/// A piece of a stretch, to be read rightward or leftward as it heads
/// overall, without where it heads so far back the other way that its
/// glyphs would be upside down, and as far either side of there as a
/// glyph is wide.
fn forward(chain: &Chain, piece: Vec<usize>, room: f64) -> Vec<Vec<usize>> {
    let n = piece.len();
    if n < 3 {
        return vec![piece];
    }
    let at = |i: usize| &chain.sections[piece[i]];
    let way = if at(n - 1).at.x >= at(0).at.x { 1.0 } else { -1.0 };
    let back: Vec<f64> = (0..n)
        .filter(|&i| {
            let heading = at((i + 1).min(n - 1)).at - at(i.saturating_sub(1)).at;
            heading.x * way < BACKWARD * heading.len()
        })
        .map(|i| at(i).s)
        .collect();
    if back.is_empty() {
        return vec![piece];
    }
    let mut out: Vec<Vec<usize>> = vec![Vec::new()];
    for i in 0..n {
        let s = at(i).s;
        if back.iter().any(|&b| (s - b).abs() <= room) {
            if !out.last().is_some_and(|p| p.is_empty()) {
                out.push(Vec::new());
            }
        } else {
            out.last_mut().expect("a piece").push(piece[i]);
        }
    }
    out.into_iter().filter(|p| p.len() > 1).collect()
}

/// A stretch of a chain's sections in pieces, each heading rightward or
/// leftward all along, so each can be read right way up: split where it
/// heads straight up or down, between pieces at least `least` long,
/// leaving `gap` either side of the turn. Shorter wiggles stay with the
/// piece they're in.
fn turns(chain: &Chain, stretch: Vec<usize>, least: f64, gap: f64) -> Vec<Vec<usize>> {
    let n = stretch.len();
    if n < 3 {
        return vec![stretch];
    }
    let at = |i: usize| &chain.sections[stretch[i]];
    let rightward = |i: usize| at((i + 1).min(n - 1)).at.x - at(i.saturating_sub(1)).at.x >= 0.0;
    // Runs heading one way, as first and last index into the stretch.
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for i in 1..n {
        if rightward(i) != rightward(start) {
            pieces.push((start, i - 1));
            start = i;
        }
    }
    pieces.push((start, n - 1));
    let long = |&(a, b): &(usize, usize)| (at(b).s - at(a).s).abs();
    // The shortest run too short to turn for joins those either side,
    // which head the same way, until none is.
    while pieces.len() > 1 {
        let Some((k, _)) = pieces.iter().enumerate().filter(|(_, p)| long(p) < least).min_by(|a, b| long(a.1).total_cmp(&long(b.1))) else {
            break;
        };
        let (lo, hi) = (k.saturating_sub(1), (k + 1).min(pieces.len() - 1));
        let joined = (pieces[lo].0, pieces[hi].1);
        pieces.splice(lo..=hi, [joined]);
    }
    if pieces.len() == 1 {
        return vec![stretch];
    }
    // Cut at each turn, the gap either side left out.
    let turn_at: Vec<f64> = pieces.windows(2).map(|w| (at(w[0].1).s + at(w[1].0).s) / 2.0).collect();
    pieces
        .iter()
        .enumerate()
        .map(|(k, &(a, b))| {
            (a..=b)
                .filter(|&i| {
                    let s = at(i).s;
                    (k == 0 || (s - turn_at[k - 1]).abs() > gap) && (k + 1 == pieces.len() || (s - turn_at[k]).abs() > gap)
                })
                .map(|i| stretch[i])
                .collect::<Vec<_>>()
        })
        .filter(|piece| piece.len() > 1)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(points: &[(f32, f32)], edges: &[(usize, usize)], width: f32) -> RibbonTree {
        RibbonTree { points: points.iter().map(|&(x, y)| PVec::new(x, y)).collect(), edges: edges.to_vec(), width }
    }

    fn line(from: (f32, f32), to: (f32, f32), width: f32) -> RibbonTree {
        tree(&[from, to], &[(0, 1)], width)
    }

    #[test]
    fn trees_split_into_a_trunk_and_branches() {
        // A Y: the trunk is the longest path, from one long arm to the
        // other; the short arm branches.
        let points: Vec<V> = [(0.0, 0.0), (100.0, 0.0), (200.0, 0.0), (100.0, 30.0)].iter().map(|&(x, y)| V::new(x, y)).collect();
        let branches = decompose(&points, &[(1, 0), (1, 2), (1, 3)]).unwrap();
        assert_eq!(branches.len(), 2);
        assert!(branches[0].nodes == vec![0, 1, 2] || branches[0].nodes == vec![2, 1, 0], "{:?}", branches[0].nodes);
        assert!(branches[0].parent.is_none());
        assert_eq!(branches[1].nodes, vec![1, 3]);
        assert_eq!(branches[1].parent, Some((0, 1)));
        // A comb: a spine with teeth; leaves - 1 chains, every edge once.
        let mut points = Vec::new();
        let mut edges = Vec::new();
        for k in 0..6 {
            points.push(V::new(k as f64 * 50.0, 0.0));
            points.push(V::new(k as f64 * 50.0, 20.0 + k as f64));
            if k > 0 {
                edges.push((2 * k - 2, 2 * k));
            }
            edges.push((2 * k, 2 * k + 1));
        }
        let branches = decompose(&points, &edges).unwrap();
        let leaves = (0..points.len()).filter(|&p| edges.iter().filter(|&&(a, b)| a == p || b == p).count() == 1).count();
        assert_eq!(branches.len(), leaves - 1);
        let covered: usize = branches.iter().map(|b| b.nodes.len() - 1).sum();
        assert_eq!(covered, edges.len());
        // Loops and stray indices are errors.
        assert!(decompose(&points[..3], &[(0, 1), (1, 2), (2, 0)]).unwrap_err().contains("closes a loop"));
        assert!(decompose(&points[..3], &[(0, 5)]).unwrap_err().contains("[0, 5]"));
    }

    #[test]
    fn ribbons_crossing_in_a_grid_weave() {
        // Three across and three down: each goes over and under in turn,
        // and neighbours the other way.
        let mut trees = Vec::new();
        for k in 0..3 {
            let at = 100.0 + 100.0 * k as f32;
            trees.push(line((0.0, at), (400.0, at), 30.0));
        }
        for k in 0..3 {
            let at = 100.0 + 100.0 * k as f32;
            trees.push(line((at, 0.0), (at, 400.0), 30.0));
        }
        let ribbons = weave(&trees, &RibbonSettings::default()).unwrap();
        assert_eq!(ribbons.crossings.len(), 9);
        let over = |across: usize, down: usize| {
            let x = ribbons.crossings.iter().find(|x| x.strands[0].chain == across && x.strands[1].chain == 3 + down).expect("they cross");
            x.over == 0
        };
        for a in 0..3 {
            for d in 0..3 {
                assert_eq!(over(a, d), (a + d) % 2 == 0, "across {} down {}", a, d);
            }
        }
        // Flipped, every crossing turns over.
        let flipped = weave(&trees, &RibbonSettings { flip: true, ..RibbonSettings::default() }).unwrap();
        for (x, y) in ribbons.crossings.iter().zip(&flipped.crossings) {
            assert_ne!(x.over, y.over);
        }
    }

    #[test]
    fn a_ribbon_crossing_itself_goes_over_then_under() {
        // A loop: out right, round, and back across itself, then across a
        // ribbon down the middle twice.
        let curl = tree(&[(0.0, 200.0), (300.0, 200.0), (300.0, 100.0), (200.0, 50.0), (150.0, 150.0), (150.0, 300.0)], &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)], 30.0);
        let ribbons = weave(&[curl], &RibbonSettings::default()).unwrap();
        let own: Vec<&Crossing> = ribbons.crossings.iter().filter(|x| x.strands[0].chain == x.strands[1].chain).collect();
        assert_eq!(own.len(), 1, "{:?}", ribbons.crossings);
        assert_eq!(own[0].over, 0, "it starts over");
    }

    #[test]
    fn a_branch_joins_the_ribbon_it_leaves_smoothly() {
        let t = tree(&[(0.0, 100.0), (200.0, 100.0), (400.0, 100.0), (200.0, 300.0)], &[(0, 1), (1, 2), (1, 3)], 40.0);
        let ribbons = weave(&[t], &RibbonSettings::default()).unwrap();
        assert_eq!(ribbons.chains.len(), 2);
        let join = ribbons.crossings.iter().find(|x| matches!(x.joint, Joint::Join { .. })).expect("a join");
        let Joint::Join { parent } = join.joint else { unreachable!() };
        assert_eq!(join.strands[parent].chain, 0);
        // It sets off along the trunk.
        assert!(ribbons.chains[1].heading(0).y.abs() < 0.15, "{:?}", ribbons.chains[1].heading(0));
        let drawn = |c: usize| {
            let chain = &ribbons.chains[c];
            let lines = ribbons.draw(StrokePath::new(), ribbons.runs(c, &ribbons.placed(c, &chain.outline(0.0, chain.length)), true, false), 40.0);
            lines.flattened(0.1).into_iter().flat_map(|(points, _)| points).collect::<Vec<_>>()
        };
        // None of the branch's lines are inside the trunk's band.
        for p in drawn(1) {
            assert!((p.y - 100.0).abs() >= 20.0 - 0.5, "{:?} is inside the trunk", p);
        }
        // The trunk's edge on the branch's side opens where it leaves.
        let mut near: Vec<f32> = drawn(0).iter().filter(|p| (p.y - 120.0).abs() < 0.5).map(|p| p.x).collect();
        near.sort_by(f32::total_cmp);
        assert!(near.windows(2).any(|w| w[1] - w[0] > 10.0 && w[0] >= 190.0 && w[1] <= 320.0), "{:?}", near);
        // The trunk's text runs on past the join; the branch's keeps off
        // the trunk.
        let lines = ribbons.text_lines(0, 10.0, 3.0, 0.0);
        let points: Vec<_> = lines.iter().flat_map(|l| &l.stretches).flat_map(|(path, _)| path.flattened(0.1)).flat_map(|(points, _)| points).collect();
        assert!(points.iter().any(|p| (p.y - 100.0).abs() < 1e-3 && (p.x - 200.0).abs() < 5.0));
        for p in points {
            let in_trunk = (p.y - 100.0).abs() < 20.0 + 3.0 - 0.5;
            let on_trunk = (p.y - 100.0).abs() < 1e-3;
            assert!(on_trunk || !in_trunk, "{:?}", p);
        }
    }

    #[test]
    fn ends_are_shaped() {
        for (ends, tip) in [(RibbonEnd::Swallowtail, 400.0 - 24.0), (RibbonEnd::Pointed, 400.0), (RibbonEnd::Cut, 400.0)] {
            let ribbons = weave(&[line((0.0, 0.0), (400.0, 0.0), 40.0)], &RibbonSettings { ends, ..RibbonSettings::default() }).unwrap();
            let chain = &ribbons.chains[0];
            let outline = chain.outline(0.0, chain.length);
            // Down the middle, the outline reaches to the notch or the tip.
            let middle = outline.iter().filter(|&&(_, y, _)| y.abs() < 1e-6).map(|&(s, _, _)| s).fold(f64::NEG_INFINITY, f64::max);
            match ends {
                RibbonEnd::Cut => assert!(outline.iter().any(|&(s, y, c)| (s - 400.0).abs() < 1e-6 && (y - 20.0).abs() < 1e-6 && c)),
                _ => assert!((middle - tip).abs() < 1e-6, "{:?}: {}", ends, middle),
            }
        }
    }

    /// How far round a chain turns from section `a` to section `b`, in
    /// radians, to the left if more than 0.
    fn turning(chain: &Chain, a: usize, b: usize) -> f64 {
        let dir = |i: usize| chain.sections[i + 1].at - chain.sections[i].at;
        (a..b - 1).map(|i| {
            let (u, v) = (dir(i), dir(i + 1));
            (u.x * v.y - u.y * v.x).atan2(u.dot(v))
        }).sum()
    }

    #[test]
    fn claws_trail_past_the_end_and_hook_round() {
        let mut turns = Vec::new();
        for seed in 0..12 {
            let width_variation = if seed % 2 == 1 { MOST_VARIATION as f32 } else { 0.0 };
            let settings = RibbonSettings { ends: RibbonEnd::Claw, width_variation, seed, ..RibbonSettings::default() };
            let ribbons = weave(&[line((0.0, 0.0), (400.0, 0.0), 40.0)], &settings).unwrap();
            let chain = &ribbons.chains[0];
            // Each claw as long as it chose, past the end: all 400 of the
            // ribbon are still there besides.
            let deep = RibbonEnd::Claw.depth() * 40.0;
            for d in chain.depth {
                assert!(d >= CLAW_LENGTH[0] * deep - 1e-9 && d <= CLAW_LENGTH[1] * deep + 1e-9, "seed {}: a claw {} long", seed, d);
            }
            assert!((chain.length - 400.0 - chain.depth[0] - chain.depth[1]).abs() < 2.0, "seed {}: {} long", seed, chain.length);
            for k in 0..2 {
                let (from, tip) = if k == 0 { (chain.depth[0], 0.0) } else { (chain.length - chain.depth[1], chain.length) };
                // It sets off from the leaf, and both edges meet at its tip.
                let leaf = V::new(if k == 0 { 0.0 } else { 400.0 }, 0.0);
                assert!((chain.frame(from, 0.0) - leaf).len() < 2.0, "seed {}: claw {} starts at {:?}", seed, k, chain.frame(from, 0.0));
                assert!((chain.frame(tip, chain.half) - chain.frame(tip, -chain.half)).len() < 1e-6);
                // Its edges never fold back on themselves, however tight it hooks.
                let ss: Vec<f64> = chain.sections.iter().map(|sec| sec.s).filter(|&s| if k == 0 { s <= from } else { s >= from }).collect();
                for side in [1.0, -1.0] {
                    for w in ss.windows(2) {
                        let ahead = chain.frame(w[1], side * chain.half) - chain.frame(w[0], side * chain.half);
                        let heading = chain.frame(w[1], 0.0) - chain.frame(w[0], 0.0);
                        assert!(ahead.dot(heading) > 0.0, "seed {}: claw {}'s edge folds at {}", seed, k, w[0]);
                    }
                }
                // How far round it turns, going out to its tip.
                let at = chain.section_at(from);
                turns.push(if k == 0 { -turning(chain, 0, at) } else { turning(chain, at, chain.last()) });
            }
        }
        // Either way, each its own way: some only a little, some right round.
        let sizes: Vec<f64> = turns.iter().map(|t| t.abs()).collect();
        let (least, most) = sizes.iter().fold((f64::MAX, 0.0f64), |(lo, hi), &t| (lo.min(t), hi.max(t)));
        assert!(least >= CLAW_TURN[0] - CLAW_VEER - 0.1 && most <= CLAW_TURN[1] + CLAW_VEER + 0.1, "{:?}", turns);
        assert!(most - least > 1.0, "{:?}", turns);
        assert!(turns.iter().any(|&t| t > 0.0) && turns.iter().any(|&t| t < 0.0), "{:?}", turns);
    }

    #[test]
    fn widths_vary_and_middles_wander() {
        let plain = weave(&[line((0.0, 0.0), (1200.0, 0.0), 20.0)], &RibbonSettings::default()).unwrap();
        assert!(plain.chains[0].sections.iter().all(|sec| sec.w == 1.0 && sec.at.y.abs() < 1e-9));
        let settings = |seed: u64| RibbonSettings { width_variation: 0.3, wander: 15.0, seed, ..RibbonSettings::default() };
        let ribbons = weave(&[line((0.0, 0.0), (1200.0, 0.0), 20.0)], &settings(4)).unwrap();
        let sections = &ribbons.chains[0].sections;
        let (narrowest, widest) = sections.iter().fold((f64::MAX, 0.0f64), |(lo, hi), sec| (lo.min(sec.w), hi.max(sec.w)));
        assert!(narrowest >= 0.7 - 1e-9 && widest <= 1.3 + 1e-9 && widest - narrowest > 0.2, "{} to {}", narrowest, widest);
        let furthest = sections.iter().map(|sec| sec.at.y.abs()).fold(0.0, f64::max);
        assert!(furthest <= 15.0 + 1e-6 && furthest > 5.0, "{}", furthest);
        // The same seed wanders the same way; another, another way.
        let draw = |seed: u64| format!("{:?}", weave(&[line((0.0, 0.0), (1200.0, 0.0), 20.0)], &settings(seed)).unwrap().lines(0));
        assert_eq!(draw(4), draw(4));
        assert_ne!(draw(4), draw(5));
        // A branch sets off from where its trunk wandered to, as wide as it
        // is there.
        let y = tree(&[(0.0, 0.0), (300.0, 0.0), (600.0, 0.0), (300.0, 200.0)], &[(0, 1), (1, 2), (1, 3)], 20.0);
        let ribbons = weave(&[y], &settings(4)).unwrap();
        let branch = &ribbons.chains[1];
        let (p, k) = branch.parent.unwrap();
        let join = &ribbons.chains[p].sections[k];
        assert!((branch.sections[0].at - join.at).len() < 2.0, "{:?} and {:?}", branch.sections[0].at, join.at);
        assert!((branch.sections[0].w - join.w).abs() < 0.02, "{} and {}", branch.sections[0].w, join.w);
        // Out of range.
        let one = || [line((0.0, 0.0), (100.0, 0.0), 20.0)];
        assert!(weave(&one(), &RibbonSettings { width_variation: 0.6, ..RibbonSettings::default() }).err().unwrap().contains("'width_variation'"));
        assert!(weave(&one(), &RibbonSettings { wander: -1.0, ..RibbonSettings::default() }).err().unwrap().contains("'wander'"));
    }

    #[test]
    fn text_keeps_clear_of_ribbons_over_it() {
        let trees = vec![
            tree(&[(0.0, 100.0), (200.0, 150.0), (400.0, 100.0)], &[(0, 1), (1, 2)], 40.0),
            tree(&[(100.0, 0.0), (150.0, 200.0), (300.0, 300.0)], &[(0, 1), (1, 2)], 40.0),
            tree(&[(250.0, 0.0), (300.0, 300.0)], &[(0, 1)], 30.0),
        ];
        let claws = RibbonSettings { ends: RibbonEnd::Claw, wander: 10.0, width_variation: 0.3, seed: 2, ..RibbonSettings::default() };
        for settings in [RibbonSettings::default(), claws] {
            let ribbons = weave(&trees, &settings).unwrap();
            let pad = 3.0;
            for t in 0..3 {
                for line in ribbons.text_lines(t, 12.0, pad, 0.0) {
                    for (points, _) in line.stretches.iter().flat_map(|(path, _)| path.flattened(0.1)) {
                        for p in points {
                            let q = V::new(p.x as f64, p.y as f64);
                            // On some chain of this tree; no chain over it there
                            // comes near.
                            let (c, k) = ribbons.chains.iter().enumerate().filter(|(_, ch)| ch.tree == t)
                                .flat_map(|(c, ch)| ch.sections.iter().enumerate().map(move |(k, s)| (c, k, (s.at - q).len())))
                                .min_by(|a, b| a.2.total_cmp(&b.2)).map(|(c, k, _)| (c, k)).unwrap();
                            for &(x, s) in &ribbons.passes[c] {
                                let crossing = &ribbons.crossings[x];
                                if crossing.over == s {
                                    continue;
                                }
                                let over = crossing.strands[crossing.over];
                                let chain = &ribbons.chains[over.chain];
                                let skip = (over.chain == c).then(|| ribbons.chains[c].sections[k].s);
                                let d = chain.distance(q, 0, chain.last(), skip);
                                // As wide as it is there.
                                let half = chain.locate(q, 0, chain.last(), skip).map_or(chain.half, |(s, _, _)| chain.reach(chain.section_at(s.clamp(0.0, chain.length))));
                                assert!(d > half + pad, "{:?}: tree {} text at {:?} is {} from a ribbon over it, {} wide there", settings.ends, t, q, d, 2.0 * half);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn weaving_is_the_same_every_time() {
        let trees = vec![
            tree(&[(0.0, 100.0), (200.0, 150.0), (400.0, 100.0), (200.0, 300.0)], &[(0, 1), (1, 2), (1, 3)], 40.0),
            tree(&[(100.0, 0.0), (150.0, 200.0), (300.0, 300.0)], &[(0, 1), (1, 2)], 30.0),
        ];
        let draw = || {
            let r = weave(&trees, &RibbonSettings::default()).unwrap();
            (0..2).map(|t| format!("{:?}{:?}{:?}", r.lines(t), r.rules(t), r.outline(t))).collect::<String>()
        };
        assert_eq!(draw(), draw());
    }

    impl Ribbons {
        /// What's drawn wrong: points of lines inside a ribbon over them
        /// (or, for lines, inside one they're joined to), and rings.
        fn faults(&self) -> Vec<String> {
            let mut faults = Vec::new();
            let within = |strand: &Strand, k: usize| k + PAD >= strand.lo && k <= strand.hi + PAD;
            for (c, chain) in self.chains.iter().enumerate() {
                if chain.sections.len() < 2 {
                    continue;
                }
                let mut lines: Vec<(Vec<(V, bool)>, bool)> = self.runs(c, &self.placed(c, &chain.outline(0.0, chain.length)), true, false).into_iter().map(|run| (run, false)).collect();
                let (a, b) = chain.rule_span();
                for side in [1.0, -1.0] {
                    let y = side * (chain.half - chain.inset);
                    let mut line: Vec<(f64, f64, bool)> = vec![(a, y, false)];
                    line.extend(chain.sections.iter().map(|sec| sec.s).filter(|&s| s > a + 1e-9 && s < b - 1e-9).map(|s| (s, y, false)));
                    line.push((b, y, false));
                    lines.extend(self.runs(c, &self.placed(c, &line), false, true).into_iter().map(|run| (run, true)));
                }
                for (run, rule) in lines {
                    for w in run.windows(2) {
                        for f in [0.25, 0.5, 0.75] {
                            let q = w[0].0.lerp(w[1].0, f);
                            let Some((own, _, _)) = chain.locate(q, 0, chain.last(), None) else { continue };
                            let k = chain.section_at(own);
                            for (d, other) in self.chains.iter().enumerate() {
                                if other.sections.len() < 2 {
                                    continue;
                                }
                                // A ribbon over itself is one that loops back, well along.
                                let skip = (d == c).then_some(own);
                                let Some((s, y, _)) = other.locate(q, 0, other.last(), skip) else { continue };
                                if d == c && (s - own).abs() < 2.0 * chain.apart() {
                                    continue;
                                }
                                let m = other.section_at(s);
                                let link = self.passes[c].iter().find(|&&(x, t)| {
                                    let x = &self.crossings[x];
                                    within(&x.strands[t], k) && x.strands[1 - t].chain == d && within(&x.strands[1 - t], m)
                                });
                                let joined = link.is_some_and(|&(x, _)| self.crossings[x].joint != Joint::None);
                                let inside = if rule && joined { other.half - other.inset } else { other.half };
                                // Well inside, on the page: a claw's tip,
                                // thinner than the line's points are apart,
                                // can slip between them.
                                let taper = other.reach(m) / (other.half * other.sections[m].w);
                                if !other.holds(s, y, inside - 0.05 * other.half * 2.0 / taper.max(1e-3)) {
                                    continue;
                                }
                                let what = if rule { "rule" } else { "edge" };
                                match link {
                                    None => faults.push(format!("chain {}'s {} at ({:.0}, {:.0}) is inside chain {}, which it doesn't cross there", c, what, q.x, q.y, d)),
                                    Some(&(x, t)) if self.crossings[x].joint == Joint::None && self.crossings[x].over == t => {}
                                    Some(&(x, _)) => faults.push(format!("chain {}'s {} at ({:.0}, {:.0}) is inside chain {} at crossing {} ({:?})", c, what, q.x, q.y, d, x, self.crossings[x].joint)),
                                }
                            }
                        }
                    }
                }
            }
            for knot in self.knots() {
                if let Some(ring) = self.ring(&knot) {
                    faults.push(format!("a ring of crossings {:?}", ring));
                }
            }
            faults
        }
    }

    /// Trees of points scattered over a page, no two too near, each
    /// joined by its shortest spanning tree, from a seed.
    fn scattered(seed: u64, trees: usize, points: usize, width: f32) -> Vec<RibbonTree> {
        let mut state = seed;
        let mut next = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 11) as f32 / (1u64 << 53) as f32
        };
        (0..trees)
            .map(|_| {
                let mut at: Vec<(f32, f32)> = Vec::new();
                for _ in 0..1000 {
                    if at.len() == points {
                        break;
                    }
                    let p = (80.0 + 1040.0 * next(), 80.0 + 740.0 * next());
                    if at.iter().all(|q| (q.0 - p.0).hypot(q.1 - p.1) > 150.0) {
                        at.push(p);
                    }
                }
                // Prim's.
                let mut edges = Vec::new();
                let mut done = vec![false; at.len()];
                done[0] = true;
                for _ in 1..at.len() {
                    let (a, b) = (0..at.len())
                        .filter(|&a| done[a])
                        .flat_map(|a| (0..at.len()).filter(|&b| !done[b]).map(move |b| (a, b)))
                        .min_by(|&(a, b), &(c, d)| (at[a].0 - at[b].0).hypot(at[a].1 - at[b].1).total_cmp(&(at[c].0 - at[d].0).hypot(at[c].1 - at[d].1)))
                        .unwrap();
                    done[b] = true;
                    edges.push((a, b));
                }
                tree(&at, &edges, width)
            })
            .collect()
    }

    #[test]
    fn scattered_weaves_are_drawn_right() {
        let mut all = Vec::new();
        for seed in 0..16 {
            for ends in [RibbonEnd::Swallowtail, RibbonEnd::Pointed, RibbonEnd::Claw] {
                let trees = scattered(seed, 3 + seed as usize % 2, 7 + seed as usize % 3, 40.0);
                // Claws on ribbons that wander and change width.
                let (wander, width_variation) = if ends == RibbonEnd::Claw { (12.0, 0.3) } else { (0.0, 0.0) };
                let ribbons = weave(&trees, &RibbonSettings { ends, wander, width_variation, seed, ..RibbonSettings::default() }).unwrap();
                let faults = ribbons.faults();
                if !faults.is_empty() {
                    all.push(format!("seed {} {:?}: {} faults, e.g. {:?}", seed, ends, faults.len(), &faults[..faults.len().min(4)]));
                }
            }
        }
        assert!(all.is_empty(), "{}", all.join("\n"));
    }
}
