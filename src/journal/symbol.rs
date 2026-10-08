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
        path.clone()
            .rotated_by(self.rotation_rad)
            .scaled_by(self.scale / 2.0)
            .translated_by(self.position)
    }
    pub fn placed_strokepath(&self) -> StrokePath {
        let symbol_base = self.base.as_ref();
        match &symbol_base.symbol_type {
            SymbolType::Blank(_, _) => StrokePath::new(),
            SymbolType::Pathed(path) => self.placed_path(path),
        }
    }

    /// The bounding box of the unplaced glyph's hull (its path spans about
    /// -1..1), widened across by its padding: the room text gives it.
    /// Half the padding goes each side, and a font size is 2 glyph units,
    /// so each side gains `padding` units.
    pub fn hull_aabb(&self) -> parry2d::bounding_volume::Aabb {
        let mut b = self.base.hull_aabb;
        b.mins.x -= self.padding;
        b.maxs.x += self.padding;
        b
    }
    /// The convex hull of the glyph's strokes as placed: the room it takes.
    pub fn footprint(&self) -> Vec<Vec2> {
        let rotation = parry2d::math::Rot2::new(self.rotation_rad);
        self.base.hull.iter().map(|p| rotation * *p * (self.scale / 2.0) + self.position).collect()
    }

    pub fn rotation_rad(&self) -> f32 {
        self.rotation_rad
    }
    pub fn set_rotation_rad(&mut self, rotation: f32) {
        self.rotation_rad = rotation;
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

/// Where and how glyphs go along a path.
#[derive(Debug, Clone)]
pub struct SymbolPlacementArgs<'a> {
    /// What the glyphs keep clear of, besides each other, and how much
    /// further than its own clearances they keep from it.
    pub avoid: Option<&'a AvoidanceTree>,
    pub padding: f32,
    pub symbol_scale: f32,
    /// Extra space between glyphs, as a fraction of `symbol_scale`; 0 sets
    /// them at their widths.
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

/// Samples per curve when measuring a path to place glyphs along it.
const SAMPLES_PER_CURVE: usize = 20;
/// When a glyph is blocked, how far on to try next, as a fraction of the
/// symbol scale; then how many halvings close in on the first clear spot.
const SEARCH_STEP: f32 = 0.25;
const REFINE_STEPS: usize = 4;

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
}

/// Glyphs placed one after another, and where each starts and ends along
/// its walk.
type Run = (Vec<Symbol>, Vec<(f32, f32)>);

impl<'a> Placer<'a> {
    fn new(args: &'a SymbolPlacementArgs<'a>) -> Self {
        Placer { args, placed: AvoidanceTree::new(), recent: VecDeque::new(), offset: 0.0 }
    }

    /// A glyph's width along the path.
    fn width(&self, symbol: &Symbol) -> f32 {
        symbol.hull_aabb().extents().x * self.args.symbol_scale / 2.0
    }

    fn gap(&self) -> f32 {
        self.args.spacing * self.args.symbol_scale
    }

    /// `symbol` placed to start `start` along `walk`, or `None` if it would
    /// run off the end: centred between its ends, turned to the path where
    /// it starts, and scaled up by the symbol scale.
    fn spot(&self, walk: &Walk, start: f32, symbol: &Symbol) -> Option<Symbol> {
        let (l, r) = (walk.at(start)?, walk.at(start + self.width(symbol))?);
        let height = symbol.hull_aabb().extents().y * self.args.symbol_scale / 2.0;
        let mut placed = symbol.clone();
        placed.set_position(l.point.lerp(r.point, 0.5) + l.normal * (self.args.baseline_shift * height));
        placed.rotate_rad_by(l.normal.y.atan2(l.normal.x) - std::f32::consts::FRAC_PI_2);
        placed.scale_by(self.args.symbol_scale);
        Some(placed)
    }

    /// Like `spot`, but a glyph that would run off the end goes at the end.
    /// For spaces, which needn't fit.
    fn spot_or_end(&self, walk: &Walk, start: f32, symbol: &Symbol) -> Symbol {
        self.spot(walk, start, symbol)
            .or_else(|| self.spot(walk, (walk.length() - self.width(symbol)).max(0.0), symbol))
            .unwrap_or_else(|| {
                let mut placed = symbol.clone();
                placed.scale_by(self.args.symbol_scale);
                placed
            })
    }

    /// Whether a glyph placed starting `start` along the current walk hits
    /// anything.
    fn blocked(&self, placed: &Symbol, start: f32) -> bool {
        if placed.is_blank() {
            return false;
        }
        let footprint = placed.footprint();
        let at = self.offset + start;
        self.args.avoid.is_some_and(|tree| tree.blocks(&footprint, self.args.padding))
            || self.placed.blocks(&footprint, 0.0)
            || self.recent.iter().any(|(other, end)| at - end > self.args.symbol_scale && shapes_within(&footprint, other, 0.0))
    }

    /// `run` glyph after glyph from `start`: `Some(Err(()))` if one is
    /// blocked, `None` if they run off the end.
    fn try_run(&self, walk: &Walk, start: f32, run: &[Symbol]) -> Option<Result<Run, ()>> {
        let (mut placed, mut spans) = (Vec::new(), Vec::new());
        let mut at = start;
        for symbol in run {
            let glyph = self.spot(walk, at, symbol)?;
            if self.blocked(&glyph, at) {
                return Some(Err(()));
            }
            let end = at + self.width(symbol);
            placed.push(glyph);
            spans.push((at, end));
            at = end + self.gap();
        }
        Some(Ok((placed, spans)))
    }

    /// `run` placed glyph after glyph from the first spot at or after
    /// `from` where it's all clear, or `None` if it runs off the end first.
    fn fit_run(&self, walk: &Walk, from: f32, run: &[Symbol]) -> Option<Run> {
        let step = (self.args.symbol_scale * SEARCH_STEP).max(1e-3);
        let mut start = from;
        let mut blocked_at = None;
        loop {
            match self.try_run(walk, start, run)? {
                Ok(found) => {
                    let Some(mut low) = blocked_at else { return Some(found) };
                    // Close in on where the obstacle ends.
                    let (mut high, mut best) = (start, found);
                    for _ in 0..REFINE_STEPS {
                        let mid = (low + high) / 2.0;
                        match self.try_run(walk, mid, run) {
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

    /// Keep a glyph placed from `start` to `end` along the current walk.
    fn commit(&mut self, placed: &Symbol, start: f32, end: f32) {
        let at = self.offset + start;
        while self.recent.front().is_some_and(|(_, e)| at - e > self.args.symbol_scale) {
            let (footprint, _) = self.recent.pop_front().expect("checked");
            self.placed.add_polygon(&footprint, 0.0);
        }
        if !placed.is_blank() {
            self.recent.push_back((placed.footprint(), self.offset + end));
        }
    }

    /// Done with a walk. The next one starts somewhere else, so every glyph
    /// so far is an obstacle to it.
    fn finish_walk(&mut self, walk: &Walk) {
        for (footprint, _) in self.recent.drain(..) {
            self.placed.add_polygon(&footprint, 0.0);
        }
        self.offset += walk.length();
    }
}

/// Place glyphs one by one along a path's subpaths, each at the first spot
/// clear of `args.avoid` and of the glyphs before it, until the path runs
/// out. Returns the glyphs placed and those left over.
pub fn place_along_path(mut symbols: Vec<Symbol>, path: &StrokePath, args: &SymbolPlacementArgs) -> (Vec<Symbol>, Vec<Symbol>) {
    let mut placer = Placer::new(args);
    let mut next = 0;
    for substroke in path.to_substrokes() {
        if next == symbols.len() {
            break;
        }
        let walk = Walk::new(&substroke, SAMPLES_PER_CURVE);
        let mut cursor = 0.0;
        while next < symbols.len() {
            let Some((mut placed, spans)) = placer.fit_run(&walk, cursor, &symbols[next..=next]) else { break };
            let ((start, end), glyph) = (spans[0], placed.pop().expect("one glyph"));
            placer.commit(&glyph, start, end);
            symbols[next] = glyph;
            cursor = end + placer.gap();
            next += 1;
        }
        placer.finish_walk(&walk);
    }
    let unplaced = symbols.split_off(next);
    (symbols, unplaced)
}

/// Like `place_along_path`, but word by word: a word (the glyphs up to a
/// blank, which goes with it) is kept together on one subpath, the subpaths
/// being lines. A word longer than a whole line is split over lines.
pub fn place_words_along_path(symbols: Vec<Symbol>, path: &StrokePath, args: &SymbolPlacementArgs) -> (Vec<Symbol>, Vec<Symbol>) {
    let mut words: VecDeque<Vec<Symbol>> = VecDeque::new();
    let mut word = Vec::new();
    for symbol in symbols {
        let blank = symbol.is_blank();
        word.push(symbol);
        if blank {
            words.push_back(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push_back(word);
    }

    let mut placer = Placer::new(args);
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
            let letters = word.len() - word.iter().rev().take_while(|s| s.is_blank()).count();
            let fitted = if letters == 0 { Some((Vec::new(), Vec::new())) } else { placer.fit_run(&walk, cursor, &word[..letters]) };
            match fitted {
                Some((glyphs, spans)) => {
                    for (glyph, (start, end)) in glyphs.iter().zip(&spans) {
                        placer.commit(glyph, *start, *end);
                    }
                    if let Some((_, end)) = spans.last() {
                        cursor = end + placer.gap();
                    }
                    placed.extend(glyphs);
                    for blank in &word[letters..] {
                        placed.push(placer.spot_or_end(&walk, cursor, blank));
                        cursor += placer.width(blank) + placer.gap();
                    }
                    words.pop_front();
                    line_used = true;
                }
                None => {
                    let width: f32 = word[..letters].iter().map(|s| placer.width(s) + placer.gap()).sum::<f32>() - placer.gap();
                    if !line_used && width > walk.length() {
                        // Too long for any line: as much as fits here, and
                        // the rest on the next.
                        let mut fitted = 0;
                        while fitted < letters {
                            let Some((mut glyphs, spans)) = placer.fit_run(&walk, cursor, &word[fitted..=fitted]) else { break };
                            let ((start, end), glyph) = (spans[0], glyphs.pop().expect("one glyph"));
                            placer.commit(&glyph, start, end);
                            placed.push(glyph);
                            cursor = end + placer.gap();
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
    (placed, words.into_iter().flatten().collect())
}
