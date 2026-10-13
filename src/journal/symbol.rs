use super::avoid::{shapes_within, AvoidanceTree};
use super::paths::{StrokePath, Walk};
use parry2d::math::Vec2;

use std::rc::Rc;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum SymbolType {
    Blank(f32, f32),
    Pathed(StrokePath),
}

#[derive(Debug, Clone)]
pub struct SymbolBase {
    symbol_type: SymbolType,
    /// The hull's bounding box, which text layout reads for every glyph.
    hull_aabb: parry2d::bounding_volume::Aabb,
    /// The convex hull of its strokes, unplaced: what text keeps clear of.
    hull: Vec<Vec2>,
}

impl SymbolBase {
    pub fn new(symbol_type: SymbolType) -> SymbolBase {
        let hull = SymbolBase::generate_hull(&symbol_type);
        SymbolBase {
            hull_aabb: hull.local_aabb(),
            hull: hull.points().to_vec(),
            symbol_type,
        }
    }
    pub fn is_blank(&self) -> bool {
        matches!(self.symbol_type, SymbolType::Blank(_, _))
    }

    fn generate_hull(symbol_type: &SymbolType ) -> parry2d::shape::ConvexPolygon {
        let points =
        match &symbol_type {
            SymbolType::Blank(w, h) => {
                let hw = w / 2.0;
                let hh = h / 2.0;
                vec![
                    Vec2::new(-hw, -hh),
                    Vec2::new(hw, -hh),
                    Vec2::new(hw, hh),
                    Vec2::new(-hw, hh),
                ]
            }
            SymbolType::Pathed(path) => {
                path.get_hull_points()
            }
        };
        if points.len() >= 3
            && let Some(hull) = parry2d::shape::ConvexPolygon::from_convex_hull(&parry2d::transformation::convex_hull(&points)) {
            return hull;
        }
        // A straight line (or a point, or nothing) has no area to wrap, so
        // use its bounding box, padded so layout still sees a little width
        // and height.
        const PAD: f32 = 0.02;
        let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for p in &points {
            min = min.min(*p);
            max = max.max(*p);
        }
        if points.is_empty() {
            (min, max) = (Vec2::ZERO, Vec2::ZERO);
        }
        let (min, max) = (min - Vec2::splat(PAD), max + Vec2::splat(PAD));
        parry2d::shape::ConvexPolygon::from_convex_hull(&[min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)])
            .expect("a padded box has area")
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    base: Rc<SymbolBase>,
    scale: f32,
    position: Vec2,
    rotation_rad: f32,
    /// How far the glyph leans, as italics do, in radians: its top goes
    /// forward and its bottom back, about its centre, before it's turned.
    skew_rad: f32,
    /// Room between this glyph and its neighbours when text is set, in
    /// font sizes: its font's character padding, half each side.
    padding: f32,
}

impl Symbol {
    pub fn new_default_from_base_rc(base: Rc<SymbolBase>) -> Symbol {
        Symbol::new_from_base_rc(base, 1.0, Vec2::new(0.0, 0.0), 0.0)
    }
    pub fn new_from_base_rc(base: Rc<SymbolBase>, scale: f32, position: Vec2, rotation_rad: f32) -> Symbol {
        Symbol {
            base,
            scale,
            position,
            rotation_rad,
            skew_rad: 0.0,
            padding: 0.0,
        }
    }
    /// The same glyph with `padding` (in font sizes) between it and its
    /// neighbours when text is set. What text keeps clear of is still its
    /// strokes' hull.
    pub fn with_padding(mut self, padding: f32) -> Symbol {
        self.padding = padding;
        self
    }
    /// The same glyph leant over by `skew` radians (see `skew_rad`).
    pub fn with_skew(mut self, skew: f32) -> Symbol {
        self.skew_rad = skew;
        self
    }
    pub fn position(&self) -> Vec2 {
        self.position
    }
    pub fn set_position(&mut self, position: Vec2) {
        self.position = position;
    }
    pub fn translate_by(&mut self, delta: Vec2) {
        self.position += delta;
    }
    pub fn scale_by(&mut self, scale: f32) {
        self.scale *= scale;
    }
    pub fn rotate_rad_by(&mut self, delta: f32) {
        self.rotation_rad += delta;
    }
    fn placed_path(&self, path: &StrokePath) -> StrokePath {
        let path = if self.skew_rad == 0.0 { path.clone() } else { path.clone().sheared_by(self.skew_rad.tan()) };
        path.rotated_by(self.rotation_rad)
            .scaled_by(self.scale / 2.0)
            .translated_by(self.position)
    }
    /// The hull's points leant and turned as the glyph is, but not scaled
    /// or moved: in glyph units, about its centre.
    fn shaped_hull(&self) -> impl Iterator<Item = Vec2> + '_ {
        let (shear, rotation) = (self.skew_rad.tan(), parry2d::math::Rot2::new(self.rotation_rad));
        self.base.hull.iter().map(move |p| rotation * Vec2::new(p.x - shear * p.y, p.y))
    }
    pub fn placed_strokepath(&self) -> StrokePath {
        let symbol_base = self.base.as_ref();
        match &symbol_base.symbol_type {
            SymbolType::Blank(_, _) => StrokePath::new(),
            SymbolType::Pathed(path) => self.placed_path(path),
        }
    }

    /// The bounding box of the glyph's hull, leant and turned as the glyph
    /// is but not scaled or moved (its path spans about -1..1), widened
    /// across by its padding. Half the padding goes each side, and a font
    /// size is 2 glyph units, so each side gains `padding` units.
    pub fn hull_aabb(&self) -> parry2d::bounding_volume::Aabb {
        let mut b = if self.skew_rad == 0.0 && self.rotation_rad == 0.0 {
            self.base.hull_aabb
        } else {
            let (min, max) = self.shaped_hull().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(min, max), p| (min.min(p), max.max(p)));
            parry2d::bounding_volume::Aabb::new(min, max)
        };
        b.mins.x -= self.padding;
        b.maxs.x += self.padding;
        b
    }
    /// The convex hull of the glyph's strokes as placed: the room it takes.
    pub fn footprint(&self) -> Vec<Vec2> {
        self.shaped_hull().map(|p| p * (self.scale / 2.0) + self.position).collect()
    }
    /// The glyph's outline as it would be drawn scaled up by `size` (as text
    /// layout scales glyphs), for setting it beside others.
    pub fn outline(&self, size: f32) -> Outline {
        let s = self.scale * size / 2.0;
        let points: Vec<Vec2> = self.shaped_hull().map(|p| p * s).collect();
        Outline::new(&points, self.padding * s, self.skew_rad.tan())
    }

    pub fn rotation_rad(&self) -> f32 {
        self.rotation_rad
    }
    pub fn set_rotation_rad(&mut self, rotation: f32) {
        self.rotation_rad = rotation;
    }
    pub fn skew_rad(&self) -> f32 {
        self.skew_rad
    }
    pub fn set_skew_rad(&mut self, skew: f32) {
        self.skew_rad = skew;
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }
    pub fn is_blank(&self) -> bool {
        self.base.is_blank()
    }
}

impl From<Symbol> for StrokePath {
    fn from(val: Symbol) -> Self {
        val.placed_strokepath()
    }
}

impl From<&Symbol> for StrokePath {
    fn from(val: &Symbol) -> Self {
        val.placed_strokepath()
    }
}

impl From<Vec<Symbol>> for StrokePath {
    fn from(val: Vec<Symbol>) -> Self {
        let mut combined_path = StrokePath::new();
        for symbol in val {
            let placed_path = symbol.placed_strokepath();
            combined_path = combined_path.merge_path(&placed_path);
        }
        combined_path
    }
}

/// How far a glyph may reach under or over its neighbour, for each unit
/// they're apart up and down. Each corner of an outline throws a cone up
/// and down at this slope (leant as the glyph leans), and glyphs keep
/// clear of each other's cones as well as their outlines: so glyphs that
/// share no height (an apostrophe after an `n`, a hyphen after a quote)
/// sit nearly side by side rather than slide past each other, and glyphs
/// tuck less into round shoulders than into straight sides. Leant cones
/// keep leant text spaced exactly as upright.
const TUCK: f32 = 0.3;

/// A glyph's outline across a line of text, for setting it beside others:
/// its hull as drawn (leant, turned and scaled as it will be), from its
/// position, x along the line and y down across it.
#[derive(Debug, Clone)]
pub struct Outline {
    /// The hull's left and right edges, top to bottom.
    left_edge: Vec<Vec2>,
    right_edge: Vec<Vec2>,
    /// The hull's bounding box.
    min: Vec2,
    max: Vec2,
    /// The room it keeps from its neighbours, each side.
    pad: f32,
    /// How far across the glyph leans for each unit up (see `TUCK`).
    lean: f32,
}

impl Outline {
    /// The outline of a convex `hull`, keeping `pad` from its neighbours.
    fn new(hull: &[Vec2], pad: f32, lean: f32) -> Outline {
        let mut sorted = hull.to_vec();
        sorted.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
        let (min, max) = sorted.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(min, max), p| (min.min(*p), max.max(*p)));
        Outline {
            left_edge: edge(&sorted, |turn| turn < 0.0, f32::min),
            right_edge: edge(&sorted, |turn| turn > 0.0, f32::max),
            min,
            max,
            pad,
            lean,
        }
    }
    /// Where its room starts across the line, from its position.
    pub fn left(&self) -> f32 {
        self.min.x - self.pad
    }
    /// Where its room ends across the line, from its position.
    pub fn right(&self) -> f32 {
        self.max.x + self.pad
    }
    pub fn width(&self) -> f32 {
        self.right() - self.left()
    }
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }
    /// How far right it reaches at height `y`: its right edge, or the
    /// furthest of its corners' cones (see `TUCK`), which reach past it
    /// above and below, and past its flattest parts.
    fn reach_right(&self, y: f32) -> f32 {
        let (up, down) = (TUCK - self.lean, TUCK + self.lean);
        let cones = self.right_edge.iter().map(|c| c.x - if y < c.y { up * (c.y - y) } else { down * (y - c.y) }).fold(f32::MIN, f32::max);
        if (self.min.y..=self.max.y).contains(&y) { cones.max(x_at(&self.right_edge, y)) } else { cones }
    }
    /// How far left it reaches at height `y`, as `reach_right`.
    fn reach_left(&self, y: f32) -> f32 {
        let (up, down) = (TUCK + self.lean, TUCK - self.lean);
        let cones = self.left_edge.iter().map(|c| c.x + if y < c.y { up * (c.y - y) } else { down * (y - c.y) }).fold(f32::MAX, f32::min);
        if (self.min.y..=self.max.y).contains(&y) { cones.min(x_at(&self.left_edge, y)) } else { cones }
    }
    /// How far past this glyph's position `next`'s must be for `next` to
    /// clear it across, at every height, by both their paddings.
    pub fn advance(&self, next: &Outline) -> f32 {
        // The reaches are made of straight pieces that bend only at the
        // corners' heights, so the closest they come is at one of those.
        let closest = self.right_edge.iter().chain(&next.left_edge)
            .map(|c| self.reach_right(c.y) - next.reach_left(c.y))
            .fold(f32::MIN, f32::max);
        closest + self.pad + next.pad
    }
}

/// One side of a convex polygon, from its points sorted top to bottom: the
/// chain whose turns `keeps` (by their cross products), with the points at
/// one height (a flat top or bottom) merged into the outermost by `pick`.
fn edge(sorted: &[Vec2], keeps: impl Fn(f32) -> bool, pick: fn(f32, f32) -> f32) -> Vec<Vec2> {
    let mut chain: Vec<Vec2> = Vec::new();
    for &p in sorted {
        while let [.., a, b] = chain[..] {
            if keeps((b - a).perp_dot(p - a)) {
                break;
            }
            chain.pop();
        }
        chain.push(p);
    }
    let mut merged: Vec<Vec2> = Vec::new();
    for p in chain {
        match merged.last_mut() {
            Some(last) if last.y == p.y => last.x = pick(last.x, p.x),
            _ => merged.push(p),
        }
    }
    merged
}

/// Where an edge (from top to bottom) is across at height `y`, held at its
/// ends above and below it.
fn x_at(edge: &[Vec2], y: f32) -> f32 {
    let i = edge.partition_point(|p| p.y < y);
    if i == 0 {
        return edge[0].x;
    }
    if i == edge.len() {
        return edge[i - 1].x;
    }
    let (a, b) = (edge[i - 1], edge[i]);
    a.x + (b.x - a.x) * (y - a.y) / (b.y - a.y)
}

/// How many glyphs back a glyph keeps clear of: one tucked under its
/// neighbour mustn't let the next run into the glyph before.
const LOOKBACK: usize = 4;

/// The glyphs set so far along a line, for setting the next as close as
/// their outlines allow.
#[derive(Debug, Clone, Default)]
pub struct Spacing {
    /// The last few glyphs set, the latest last: their outlines, and where
    /// their positions are along the line.
    set: Vec<(Rc<Outline>, f32)>,
}

impl Spacing {
    /// Where `next`'s room starts when it's set as close after the glyphs
    /// set as their outlines allow, `gap` further than their paddings.
    /// `cursor` is where it would start set at their widths: past the last
    /// glyph's room, the gap, and whatever came between (spaces, say),
    /// which it keeps.
    pub fn start(&self, next: &Outline, cursor: f32, gap: f32) -> f32 {
        let Some(((last, _), earlier)) = self.set.split_last() else { return cursor };
        let mut start = cursor + last.advance(next) - (last.right() - next.left());
        for (glyph, at) in earlier {
            start = start.max(at + glyph.advance(next) + gap + next.left());
        }
        start
    }
    /// Set a glyph, its room starting at `start`.
    pub fn push(&mut self, outline: Rc<Outline>, start: f32) {
        let at = start - outline.left();
        self.set.push((outline, at));
        if self.set.len() > LOOKBACK {
            self.set.remove(0);
        }
    }
}

/// Where and how glyphs go along a path.
#[derive(Debug, Clone)]
pub struct SymbolPlacementArgs<'a> {
    /// What the glyphs keep clear of, besides each other, and how much
    /// further than its own clearances they keep from it.
    pub avoid: Option<&'a AvoidanceTree>,
    pub padding: f32,
    pub symbol_scale: f32,
    /// Extra space between glyphs, as a fraction of `symbol_scale`; 0 sets
    /// them as close as their outlines and paddings allow.
    pub spacing: f32,
    /// How far below the path each glyph's centre sits, as a fraction of
    /// its height: "below" being the glyph's own down, along the path's
    /// normal.
    pub baseline_shift: f32,
}

impl SymbolPlacementArgs<'_> {
    pub fn new(symbol_scale: f32) -> Self {
        SymbolPlacementArgs { avoid: None, padding: 0.0, symbol_scale, spacing: 0.0, baseline_shift: 0.0 }
    }
}

/// Breaking words round what text avoids: a word that something blocks
/// partway goes on in pieces either side of it, instead of moving past it
/// whole. Each piece is at least `min_piece` glyphs, and keeps `padding`
/// (a fraction of the symbol scale) further from what it's split round
/// than the clearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WordSplit {
    pub min_piece: usize,
    pub padding: f32,
}

/// Samples per curve when measuring a path to place glyphs along it.
const SAMPLES_PER_CURVE: usize = 20;
/// When a glyph is blocked, how far on to try next, as a fraction of the
/// symbol scale; then how many halvings close in on the first clear spot.
const SEARCH_STEP: f32 = 0.25;
const REFINE_STEPS: usize = 4;

/// A glyph to place, and its outline at the size it's placed.
#[derive(Clone)]
struct Piece {
    symbol: Symbol,
    outline: Rc<Outline>,
}

/// Places glyphs along walks, clear of the avoidance tree and of each
/// other.
struct Placer<'a> {
    args: &'a SymbolPlacementArgs<'a>,
    /// Glyphs placed so far, once the text has moved on from them.
    placed: AvoidanceTree,
    /// The latest glyphs' footprints, with how far along the text each
    /// ends. They're the next glyphs' neighbours, which may touch them (on a
    /// bend, overlap them), so they only join `placed` once the text is a
    /// glyph further on: only text that a path loops back over collides.
    recent: VecDeque<(Vec<Vec2>, f32)>,
    /// How far along the text the current walk starts.
    offset: f32,
    /// The glyphs set so far along the current walk, which the next is set
    /// beside.
    spacing: Spacing,
}

/// Glyphs placed one after another: the glyphs, where each starts and ends
/// along its walk, and the spacing after them.
struct Run {
    glyphs: Vec<Symbol>,
    spans: Vec<(f32, f32)>,
    spacing: Spacing,
}

impl<'a> Placer<'a> {
    fn new(args: &'a SymbolPlacementArgs<'a>) -> Self {
        Placer { args, placed: AvoidanceTree::new(), recent: VecDeque::new(), offset: 0.0, spacing: Spacing::default() }
    }

    fn piece(&self, symbol: Symbol) -> Piece {
        Piece { outline: Rc::new(symbol.outline(self.args.symbol_scale)), symbol }
    }

    fn gap(&self) -> f32 {
        self.args.spacing * self.args.symbol_scale
    }

    /// Where `piece` starts set after `spacing`, `cursor` being where it
    /// would start set at their widths. Blanks start at the cursor; glyphs
    /// are set beside the glyphs before them, whatever blanks are between.
    fn start(&self, spacing: &Spacing, piece: &Piece, cursor: f32) -> f32 {
        if piece.symbol.is_blank() { cursor } else { spacing.start(&piece.outline, cursor, self.gap()) }
    }

    /// `piece` placed with its room from `start` along `walk`, or `None` if
    /// it would run off the end: its room's middle halfway between the
    /// room's ends, turned to the path where it starts, and scaled up by the
    /// symbol scale.
    fn spot(&self, walk: &Walk, start: f32, piece: &Piece) -> Option<Symbol> {
        let outline = &piece.outline;
        let (l, r) = (walk.at(start)?, walk.at(start + outline.width())?);
        let turn = l.normal.y.atan2(l.normal.x) - std::f32::consts::FRAC_PI_2;
        let middle = (outline.left() + outline.right()) / 2.0;
        let mut placed = piece.symbol.clone();
        placed.set_position(l.point.lerp(r.point, 0.5) - Vec2::new(turn.cos(), turn.sin()) * middle + l.normal * (self.args.baseline_shift * outline.height()));
        placed.rotate_rad_by(turn);
        placed.scale_by(self.args.symbol_scale);
        Some(placed)
    }

    /// Like `spot`, but a glyph that would run off the end goes at the end.
    /// For spaces, which needn't fit.
    fn spot_or_end(&self, walk: &Walk, start: f32, piece: &Piece) -> Symbol {
        self.spot(walk, start, piece)
            .or_else(|| self.spot(walk, (walk.length() - piece.outline.width()).max(0.0), piece))
            .unwrap_or_else(|| {
                let mut placed = piece.symbol.clone();
                placed.scale_by(self.args.symbol_scale);
                placed
            })
    }

    /// Whether a glyph placed starting `start` along the current walk hits
    /// anything, keeping `pad` further than the padding from what's avoided.
    fn blocked(&self, placed: &Symbol, start: f32, pad: f32) -> bool {
        if placed.is_blank() {
            return false;
        }
        let footprint = placed.footprint();
        let at = self.offset + start;
        self.args.avoid.is_some_and(|tree| tree.blocks(&footprint, self.args.padding + pad))
            || self.placed.blocks(&footprint, 0.0)
            || self.recent.iter().any(|(other, end)| at - end > self.args.symbol_scale && shapes_within(&footprint, other, 0.0))
    }

    /// `run` glyph after glyph after `spacing`, the first from `start`:
    /// `Some(Err(()))` if one is blocked, `None` if they run off the end.
    fn try_run(&self, walk: &Walk, start: f32, run: &[Piece], spacing: &Spacing, pad: f32) -> Option<Result<Run, ()>> {
        let mut spacing = spacing.clone();
        let (mut glyphs, mut spans) = (Vec::new(), Vec::new());
        let mut at = start;
        for (k, piece) in run.iter().enumerate() {
            if k > 0 {
                at = self.start(&spacing, piece, at);
            }
            let glyph = self.spot(walk, at, piece)?;
            if self.blocked(&glyph, at, pad) {
                return Some(Err(()));
            }
            let end = at + piece.outline.width();
            if !piece.symbol.is_blank() {
                spacing.push(piece.outline.clone(), at);
            }
            glyphs.push(glyph);
            spans.push((at, end));
            at = end + self.gap();
        }
        Some(Ok(Run { glyphs, spans, spacing }))
    }

    /// `run` placed glyph after glyph after `spacing`, from the first spot
    /// at or after where it would be set (`from` being where it would start
    /// set at their widths) where it's all clear, or `None` if it runs off
    /// the end first.
    fn fit_run(&self, walk: &Walk, from: f32, run: &[Piece], spacing: &Spacing, pad: f32) -> Option<Run> {
        let step = (self.args.symbol_scale * SEARCH_STEP).max(1e-3);
        let mut start = self.start(spacing, &run[0], from);
        let mut blocked_at = None;
        loop {
            match self.try_run(walk, start, run, spacing, pad)? {
                Ok(found) => {
                    let Some(mut low) = blocked_at else { return Some(found) };
                    // Close in on where the obstacle ends.
                    let (mut high, mut best) = (start, found);
                    for _ in 0..REFINE_STEPS {
                        let mid = (low + high) / 2.0;
                        match self.try_run(walk, mid, run, spacing, pad) {
                            Some(Ok(closer)) => (high, best) = (mid, closer),
                            _ => low = mid,
                        }
                    }
                    return Some(best);
                }
                Err(()) => {
                    blocked_at = Some(start);
                    start += step;
                }
            }
        }
    }

    /// `word` from the first spot at or after `from` where it's clear, as
    /// `fit_run`; with `split`, a word that something blocks partway is
    /// broken round it rather than moved past it, if it can be.
    fn fit_word(&self, walk: &Walk, from: f32, word: &[Piece], spacing: &Spacing, split: Option<WordSplit>, pad: f32) -> Option<Run> {
        let whole = self.fit_run(walk, from, word, spacing, pad)?;
        match split {
            Some(split) if whole.spans[0].0 > self.start(spacing, &word[0], from) + 1e-3 => {
                Some(self.split_word(walk, from, word, spacing, split).unwrap_or(whole))
            }
            _ => Some(whole),
        }
    }

    /// `word` in pieces: the longest first piece that fits from `from`,
    /// and the rest past what stops it (broken again if need be), all
    /// `split.padding` further from what they avoid. `None` if no first
    /// piece fits, or the rest runs off the end.
    fn split_word(&self, walk: &Walk, from: f32, word: &[Piece], spacing: &Spacing, split: WordSplit) -> Option<Run> {
        let min = split.min_piece.max(1);
        let pad = split.padding * self.args.symbol_scale;
        let n = word.len();
        if n < 2 * min {
            return None;
        }
        let start = self.start(spacing, &word[0], from);
        for k in (min..=n - min).rev() {
            let Some(Ok(mut head)) = self.try_run(walk, start, &word[..k], spacing, pad) else { continue };
            let rest_from = head.spans.last().expect("a piece has glyphs").1 + self.gap();
            let rest = self.fit_word(walk, rest_from, &word[k..], &head.spacing, Some(split), pad)?;
            head.glyphs.extend(rest.glyphs);
            head.spans.extend(rest.spans);
            head.spacing = rest.spacing;
            return Some(head);
        }
        None
    }

    /// Keep a run's glyphs, and give them back.
    fn commit(&mut self, run: Run) -> Vec<Symbol> {
        for (placed, &(start, end)) in run.glyphs.iter().zip(&run.spans) {
            let at = self.offset + start;
            while self.recent.front().is_some_and(|(_, e)| at - e > self.args.symbol_scale) {
                let (footprint, _) = self.recent.pop_front().expect("checked");
                self.placed.add_polygon(&footprint, 0.0);
            }
            if !placed.is_blank() {
                self.recent.push_back((placed.footprint(), self.offset + end));
            }
        }
        self.spacing = run.spacing;
        run.glyphs
    }

    /// Done with a walk. The next one starts somewhere else, so every glyph
    /// so far is an obstacle to it.
    fn finish_walk(&mut self, walk: &Walk) {
        for (footprint, _) in self.recent.drain(..) {
            self.placed.add_polygon(&footprint, 0.0);
        }
        self.offset += walk.length();
        self.spacing = Spacing::default();
    }
}

/// Place glyphs one by one along a path's subpaths, each at the first spot
/// clear of `args.avoid` and of the glyphs before it, until the path runs
/// out. Returns the glyphs placed and those left over.
pub fn place_along_path(symbols: Vec<Symbol>, path: &StrokePath, args: &SymbolPlacementArgs) -> (Vec<Symbol>, Vec<Symbol>) {
    let mut placer = Placer::new(args);
    let pieces: Vec<Piece> = symbols.iter().map(|s| placer.piece(s.clone())).collect();
    let mut placed = Vec::new();
    for substroke in path.to_substrokes() {
        if placed.len() == pieces.len() {
            break;
        }
        let walk = Walk::new(&substroke, SAMPLES_PER_CURVE);
        let mut cursor = 0.0;
        while placed.len() < pieces.len() {
            let next = placed.len();
            let Some(run) = placer.fit_run(&walk, cursor, &pieces[next..=next], &placer.spacing, 0.0) else { break };
            cursor = run.spans[0].1 + placer.gap();
            placed.extend(placer.commit(run));
        }
        placer.finish_walk(&walk);
    }
    let unplaced = symbols[placed.len()..].to_vec();
    (placed, unplaced)
}

/// Like `place_along_path`, but word by word: a word (the glyphs up to a
/// blank, which goes with it) is kept together on one subpath, the subpaths
/// being lines. A word longer than a whole line is split over lines, and
/// with `split`, a word is broken round what it avoids (see `WordSplit`).
pub fn place_words_along_path(symbols: Vec<Symbol>, path: &StrokePath, args: &SymbolPlacementArgs, split: Option<WordSplit>) -> (Vec<Symbol>, Vec<Symbol>) {
    let mut placer = Placer::new(args);
    let mut words: VecDeque<Vec<Piece>> = VecDeque::new();
    let mut word = Vec::new();
    for symbol in symbols {
        let blank = symbol.is_blank();
        word.push(placer.piece(symbol));
        if blank {
            words.push_back(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push_back(word);
    }

    let mut placed = Vec::new();
    for substroke in path.to_substrokes() {
        if words.is_empty() {
            break;
        }
        let walk = Walk::new(&substroke, SAMPLES_PER_CURVE);
        let mut cursor = 0.0;
        let mut line_used = false;
        while let Some(word) = words.front_mut() {
            // Blanks after the word go wherever the word ends, even past
            // the end of the line.
            let letters = word.len() - word.iter().rev().take_while(|p| p.symbol.is_blank()).count();
            let fitted = if letters == 0 {
                Some(Run { glyphs: Vec::new(), spans: Vec::new(), spacing: placer.spacing.clone() })
            } else {
                placer.fit_word(&walk, cursor, &word[..letters], &placer.spacing, split, 0.0)
            };
            match fitted {
                Some(run) => {
                    if let Some((_, end)) = run.spans.last() {
                        cursor = end + placer.gap();
                    }
                    placed.extend(placer.commit(run));
                    for blank in &word[letters..] {
                        placed.push(placer.spot_or_end(&walk, cursor, blank));
                        cursor += blank.outline.width() + placer.gap();
                    }
                    words.pop_front();
                    line_used = true;
                }
                None => {
                    let width: f32 = word[..letters].iter().map(|p| p.outline.width() + placer.gap()).sum::<f32>() - placer.gap();
                    if !line_used && width > walk.length() {
                        // Too long for any line: as much as fits here, and
                        // the rest on the next.
                        let mut fitted = 0;
                        while fitted < letters {
                            let Some(run) = placer.fit_run(&walk, cursor, &word[fitted..=fitted], &placer.spacing, 0.0) else { break };
                            cursor = run.spans[0].1 + placer.gap();
                            placed.extend(placer.commit(run));
                            fitted += 1;
                        }
                        word.drain(..fitted);
                    }
                    break;
                }
            }
        }
        placer.finish_walk(&walk);
    }
    (placed, words.into_iter().flatten().map(|p| p.symbol).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outline(points: &[(f32, f32)], pad: f32) -> Outline {
        Outline::new(&points.iter().map(|&(x, y)| Vec2::new(x, y)).collect::<Vec<_>>(), pad, 0.0)
    }

    fn glyph(svg: &str) -> Symbol {
        Symbol::new_default_from_base_rc(Rc::new(SymbolBase::new(SymbolType::Pathed(svg.parse().unwrap()))))
    }

    #[test]
    fn boxes_are_set_at_their_widths() {
        let a = outline(&[(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)], 0.1);
        let b = outline(&[(-0.5, -1.0), (0.5, -1.0), (0.5, 1.0), (-0.5, 1.0)], 0.2);
        assert!((a.advance(&b) - 1.8).abs() < 1e-6, "{}", a.advance(&b));
        assert!((a.width() - 2.2).abs() < 1e-6 && (b.left() + 0.7).abs() < 1e-6);
    }

    #[test]
    fn glyphs_tuck_into_each_other_as_far_as_their_outlines_allow() {
        // A narrow A and V, their steep sides parallel, a quarter apart
        // across at every height: half what their widths would set them at.
        let a = outline(&[(0.0, -1.0), (0.25, 1.0), (-0.25, 1.0)], 0.0);
        let v = outline(&[(-0.25, -1.0), (0.25, -1.0), (0.0, 1.0)], 0.0);
        assert!((a.advance(&v) - 0.25).abs() < 1e-6, "{}", a.advance(&v));
        assert!((v.advance(&a) - 0.25).abs() < 1e-6, "{}", v.advance(&a));
        // Sides flatter than the cones are kept to the cones' slope: a
        // wide A and V tuck in less than their sides would let them.
        let a = outline(&[(0.0, -1.0), (1.0, 1.0), (-1.0, 1.0)], 0.0);
        let v = outline(&[(-1.0, -1.0), (1.0, -1.0), (0.0, 1.0)], 0.0);
        assert!((a.advance(&v) - (2.0 - 2.0 * TUCK)).abs() < 1e-6, "{}", a.advance(&v));
    }

    #[test]
    fn glyphs_that_share_no_height_tuck_only_a_little() {
        // A low box, and a mark half a unit above it: the mark comes in
        // over the box by TUCK of that half unit, not back over the box.
        let low = outline(&[(-1.0, 0.0), (1.0, 0.0), (1.0, 1.0), (-1.0, 1.0)], 0.0);
        let mark = outline(&[(-0.1, -1.0), (0.1, -1.0), (0.1, -0.5), (-0.1, -0.5)], 0.0);
        let side_by_side = 1.1;
        assert!((low.advance(&mark) - (side_by_side - TUCK * 0.5)).abs() < 1e-6, "{}", low.advance(&mark));
        assert!((mark.advance(&low) - (side_by_side - TUCK * 0.5)).abs() < 1e-6, "{}", mark.advance(&low));
    }

    #[test]
    fn leaning_glyphs_keeps_their_spacing() {
        const BAR: &str = "M -0.5 -1 L 0.5 -1 L 0.5 1 L -0.5 1 Z";
        let upright = glyph(BAR).outline(2.0);
        let leant = glyph(BAR).with_skew(0.4).outline(2.0);
        // Leant, it's wider by its lean, but set as close as it was.
        assert!((leant.width() - upright.width() - 2.0 * 0.4f32.tan()).abs() < 0.02, "{} -> {}", upright.width(), leant.width());
        assert!((leant.advance(&leant) - upright.advance(&upright)).abs() < 0.02, "{} -> {}", upright.advance(&upright), leant.advance(&leant));
        // So are glyphs that share no height, however far they lean.
        const LOW: &str = "M -0.8 0.2 L 0.8 0.2 L 0.8 1 L -0.8 1 Z";
        const MARK: &str = "M 0 -1 L 0 -0.4";
        for skew in [0.3, 1.0, -0.8] {
            let pair = |skew: f32| glyph(LOW).with_skew(skew).outline(2.0).advance(&glyph(MARK).with_skew(skew).outline(2.0));
            assert!((pair(skew) - pair(0.0)).abs() < 1e-4, "{}: {} -> {}", skew, pair(0.0), pair(skew));
        }
        // Its strokes and footprint lean with it, tops forward.
        let top = glyph(BAR).with_skew(0.4).footprint().into_iter().filter(|p| p.y < -0.4).map(|p| p.x).fold(f32::MIN, f32::max);
        assert!((top - (0.25 + 0.5 * 0.4f32.tan())).abs() < 0.01, "{}", top);
        let bounds = glyph(BAR).with_skew(0.4).placed_strokepath().points_within().iter().fold(f32::MIN, |m, p| m.max(p.x));
        assert!((bounds - (0.25 + 0.5 * 0.4f32.tan())).abs() < 1e-5, "{}", bounds);
    }

    #[test]
    fn a_glyph_keeps_clear_of_the_one_before_its_neighbour() {
        // An L, a mark tucked over its foot, then a bar as tall as the L:
        // past the mark, the bar would land on the L's foot.
        let l = Rc::new(outline(&[(-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)], 0.0));
        let mark = Rc::new(outline(&[(-0.1, -1.0), (0.1, -1.0), (0.1, -0.6), (-0.1, -0.6)], 0.0));
        let bar = outline(&[(-0.5, -1.0), (0.5, -1.0), (0.5, 1.0), (-0.5, 1.0)], 0.0);
        let mut spacing = Spacing::default();
        spacing.push(l.clone(), l.left());
        let mark_at = spacing.start(&mark, l.right(), 0.0) - mark.left();
        assert!((mark_at - l.advance(&mark)).abs() < 1e-6 && mark_at < l.right(), "the mark is over the foot: {}", mark_at);
        spacing.push(mark.clone(), mark_at + mark.left());
        assert!(mark_at + mark.advance(&bar) < l.advance(&bar), "set beside the mark alone, the bar would hit the L");
        let bar_at = spacing.start(&bar, mark_at + mark.right(), 0.0) - bar.left();
        assert!((bar_at - l.advance(&bar)).abs() < 1e-6, "the bar clears the L: {}", bar_at);
    }
}
