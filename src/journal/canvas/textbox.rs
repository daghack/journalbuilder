use parry2d::bounding_volume;
use parry2d::math::Vec2;
use crate::journal::avoid::{shapes_within, AvoidanceTree};
use crate::journal::canvas::wander::Wander;
use crate::journal::symbol::{Symbol, SymbolPlacementArgs, WordRules, place_along_path, place_words_along_path};
use crate::journal::paths::StrokePath;
use crate::journal::typeset::Align;
use rand::{Rng, RngCore};

#[derive(Clone)]
pub struct TextBoxOptions<'a> {
    // jagged edge controls the amount (in terms of symbol scale) of jaggedness on the edges of the
    // textbox. A value of 0.0 means flat edges, and a value of 1.0 means that the left hand edge
    // will be offset by up to 1.0 x symbol_scale amount randomly.
    pub jagged_edge: f32,
    pub layout: TextBoxLineMode,
    /// Where the text goes: the inside of these closed outlines (each
    /// joining its last point to its first); an outline inside another is
    /// a hole.
    pub shape: Vec<Vec<Vec2>>,
    /// The lines' angle, in radians: 0 runs them left to right, each
    /// below the last; others turn them about, lines following on to
    /// their right-hand side.
    pub angle: f32,
    pub symbol_scale: f32,
    /// What the text keeps clear of, and how much further than the tree's
    /// own clearances it keeps.
    pub avoid: Option<&'a AvoidanceTree>,
    pub padding: f32,
    /// Extra space between glyphs, as a fraction of `size`.
    pub spacing: f32,
    /// Where each line's glyphs go along it: from its start, moved along as
    /// far as they still fit (centred between, or to the end), or spread
    /// out to reach both ends, all but the text's last line.
    pub align: Align,
    /// Whether glyphs' ink stays wholly inside the shape, lines reaching
    /// its edges; otherwise lines hold glyphs where the band they take up
    /// is inside it, as write_block's line boxes, which glyphs overhang.
    pub contain: bool,
    /// Where the text sits down a box that keeps it inside: 0 from the
    /// top, 1 at the bottom, or between, the room left under it shared out
    /// above it.
    pub valign: f32,
    /// How far glyphs' ink may reach past the shape's edge, in a box that
    /// keeps them inside: past that, they go on the next line.
    pub overhang: f32,
    /// How far each line strays up and down from straight, as a hand's
    /// wanders, in a box that keeps glyphs inside.
    pub wander: f32,
    /// How far each line's gap from the last may be off, either way.
    pub line_gap_jitter: f32,
}

/// A rectangle as a shape for `TextBoxOptions`.
pub fn rectangle(bounds: bounding_volume::Aabb) -> Vec<Vec<Vec2>> {
    let (min, max) = (bounds.mins, bounds.maxs);
    vec![vec![min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)]]
}

#[derive(Debug, Clone)]
pub enum TextBoxLineMode {
    Flat,
    /// Words kept whole, wrapping at blanks, unless broken round what they
    /// avoid.
    WordsByBlankSymbol(WordRules),
}

/// Glyphs sit a quarter of their height below each line.
const BASELINE_SHIFT: f32 = 0.25;

/// How many places across the band a line's glyphs might take up the shape
/// is looked at, to find where along the line they could go.
const BAND_SAMPLES: usize = 9;

/// The band looked at, in symbol scales above and below the line: more than
/// any glyph takes up, as each glyph's own ink is what's checked.
const BAND_ABOVE: f32 = 0.5;
const BAND_BELOW: f32 = 1.5;

/// How near, in symbol scales, a line's spans are joined: nearer than a
/// glyph could go between.
const SPAN_JOIN: f32 = 0.05;

/// How far apart uneven lines' glyphs keep, in symbol scales: their
/// outlines don't take in the pen's width.
const LINE_GAP: f32 = 0.12;

/// How long the straight pieces of a wandering line are, in symbol
/// scales.
const WANDER_PIECE: f32 = 0.25;

/// How far down to try the next line when one holds nothing, in symbol
/// scales: little, so the first line starts as near the shape's top as
/// its glyphs fit.
const EMPTY_LINE_STEP: f32 = 0.05;

/// For boxes that don't keep glyphs inside: how many places across a
/// line's band, from a quarter of the symbol scale above the line to three
/// quarters below, it's checked to be inside the shape.
const LOOSE_BAND_SAMPLES: usize = 5;

/// A closed shape, as a text box's: closed outlines, each inside where it
/// winds round at all, and a place inside an even number of them outside.
#[derive(Debug)]
pub struct Region {
    rings: Vec<Vec<Vec2>>,
    /// How far what it holds may reach past its edge.
    overhang: f32,
}

impl Region {
    pub fn new(rings: Vec<Vec<Vec2>>) -> Region {
        Region { rings: rings.into_iter().filter(|ring| ring.len() >= 3).collect(), overhang: 0.0 }
    }

    /// The region holding what reaches up to `overhang` past its edge.
    pub fn with_overhang(self, overhang: f32) -> Region {
        Region { overhang: overhang.max(0.0), ..self }
    }

    fn edges(ring: &[Vec2]) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
        ring.iter().enumerate().map(move |(i, &a)| (a, ring[(i + 1) % ring.len()]))
    }

    /// Whether a point is inside.
    pub fn contains(&self, p: Vec2) -> bool {
        let rings_inside = self.rings.iter().filter(|ring| {
            let mut winding = 0;
            for (a, b) in Region::edges(ring) {
                if (a.y > p.y) != (b.y > p.y) {
                    let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
                    if x > p.x {
                        winding += if b.y > a.y { 1 } else { -1 };
                    }
                }
            }
            winding != 0
        }).count();
        rings_inside % 2 == 1
    }

    /// Whether a closed outline (a placed glyph's footprint) is inside,
    /// or reaches no further past the edge than the overhang.
    pub fn holds(&self, outline: &[Vec2]) -> bool {
        self.holds_wholly(outline) || (self.overhang > 0.0 && self.holds_near(outline))
    }

    /// Whether a closed outline is wholly inside: none of its edges
    /// crosses the shape's, it starts inside, and no outline of the shape
    /// (a hole) is inside it.
    fn holds_wholly(&self, outline: &[Vec2]) -> bool {
        let Some(&first) = outline.first() else { return true };
        let (lo, hi) = outline.iter().fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let crosses = |(a, b): (Vec2, Vec2), (c, d): (Vec2, Vec2)| {
            let side = |p: Vec2, q: Vec2, r: Vec2| (q - p).perp_dot(r - p);
            let (d1, d2) = (side(c, d, a), side(c, d, b));
            let (d3, d4) = (side(a, b, c), side(a, b, d));
            (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
        };
        for ring in &self.rings {
            for edge in Region::edges(ring) {
                let (a, b) = edge;
                if a.x.max(b.x) < lo.x || a.x.min(b.x) > hi.x || a.y.max(b.y) < lo.y || a.y.min(b.y) > hi.y {
                    continue;
                }
                if Region::edges(outline).any(|side| crosses(side, edge)) {
                    return false;
                }
            }
        }
        let around = Region::new(vec![outline.to_vec()]);
        self.contains(first) && !self.rings.iter().any(|ring| around.contains(ring[0]))
    }

    /// Whether a closed outline reaches no further than the overhang past
    /// the edge: every point along its edges is inside or that near the
    /// edge, and no corner of the shape's edge is further than that inside
    /// it.
    fn holds_near(&self, outline: &[Vec2]) -> bool {
        let reach = self.overhang;
        let step = (reach / 2.0).max(1e-3);
        for (a, b) in Region::edges(outline) {
            let pieces = ((a.distance(b) / step).ceil() as usize).max(1);
            for k in 0..pieces {
                let p = a.lerp(b, k as f32 / pieces as f32);
                if !self.contains(p) && !Region::near_edge(&self.rings, p, reach) {
                    return false;
                }
            }
        }
        let around = Region::new(vec![outline.to_vec()]);
        let outlines = [outline.to_vec()];
        self.rings.iter().flatten().all(|&corner| !around.contains(corner) || Region::near_edge(&outlines, corner, reach))
    }

    /// Whether `p` is within `reach` of an edge of `rings`.
    fn near_edge(rings: &[Vec<Vec2>], p: Vec2, reach: f32) -> bool {
        rings.iter().any(|ring| Region::edges(ring).any(|(a, b)| {
            if a.x.min(b.x) > p.x + reach || a.x.max(b.x) < p.x - reach || a.y.min(b.y) > p.y + reach || a.y.max(b.y) < p.y - reach {
                return false;
            }
            let ab = b - a;
            let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            p.distance(a + ab * t) <= reach
        }))
    }
}

pub struct TextBox<'a> {
    layout: TextBoxLineMode,
    jagged_edge: f32,
    /// The shape, turned so the lines run along +x.
    shape: Vec<Vec<Vec2>>,
    /// The shape where it is, which glyphs' ink stays inside, if it's
    /// kept to.
    region: Option<Region>,
    /// Along the lines, and on to the next.
    along: Vec2,
    across: Vec2,
    symbol_scale: f32,
    avoid: Option<&'a AvoidanceTree>,
    padding: f32,
    spacing: f32,
    align: Align,
    valign: f32,
    overhang: f32,
    wander: f32,
    line_gap_jitter: f32,
}

/// How many halvings close in on how far down text can be set and still
/// all fit, when valign's full share of the room would lose some.
const VALIGN_STEPS: usize = 8;

/// How many halvings close in on how far a line's glyphs can be moved.
const ALIGN_STEPS: usize = 14;

impl<'a> TextBox<'a> {
    pub fn new(options: TextBoxOptions<'a>) -> TextBox<'a> {
        let along = Vec2::new(options.angle.cos(), options.angle.sin());
        let across = Vec2::new(-along.y, along.x);
        let shape = options.shape.iter()
            .map(|ring| ring.iter().map(|p| Vec2::new(p.dot(along), p.dot(across))).collect())
            .collect();
        TextBox {
            region: options.contain.then(|| Region::new(options.shape.clone()).with_overhang(options.overhang)),
            jagged_edge: options.jagged_edge,
            layout: options.layout,
            avoid: options.avoid,
            padding: options.padding,
            shape,
            along,
            across,
            symbol_scale: options.symbol_scale,
            spacing: options.spacing,
            align: options.align,
            valign: options.valign,
            overhang: options.overhang,
            wander: options.wander,
            line_gap_jitter: options.line_gap_jitter,
        }
    }

    /// The longest a line can surely be: as wide as the turned shape, run
    /// on past its edges, less as far in as a jagged line may start. A
    /// word no longer than that waits for a line it fits (one piece of a
    /// line broken round a hole or a notch can be shorter), and a longer
    /// one is broken.
    fn longest_line(&self) -> f32 {
        let (lo, hi) = self.shape.iter().flatten().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
        let past = self.symbol_scale + self.overhang;
        // A jagged line starts in from the edge rather than before it.
        let start = if self.jagged_edge > 0.0 { -self.jagged_edge * self.symbol_scale } else { past };
        hi - lo + past + start
    }

    /// The turned shape's top and bottom.
    fn extent(&self) -> (f32, f32) {
        self.shape.iter().flatten().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)))
    }

    /// Where the turned shape's inside crosses the line at `y`, left to
    /// right. Each outline's inside is where it winds round at all, so one
    /// drawn on past its start (as circles are, for a clean join) is still
    /// filled; and a place inside an even number of outlines is outside.
    fn spans_at(&self, y: f32) -> Vec<(f32, f32)> {
        let mut edges: Vec<f32> = Vec::new();
        for ring in &self.shape {
            let mut crossings: Vec<(f32, i32)> = Vec::new();
            for (i, &a) in ring.iter().enumerate() {
                let b = ring[(i + 1) % ring.len()];
                if (a.y > y) != (b.y > y) {
                    crossings.push((a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y), if b.y > a.y { 1 } else { -1 }));
                }
            }
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0;
            for (x, turn) in crossings {
                let was_inside = winding != 0;
                winding += turn;
                if was_inside != (winding != 0) {
                    edges.push(x);
                }
            }
        }
        // Each outline's spans are apart, so their ends toggle in and out.
        edges.sort_by(f32::total_cmp);
        edges.chunks_exact(2).map(|pair| (pair[0], pair[1])).filter(|(l, r)| r > l).collect()
    }

    /// Where along a line at `y` glyphs might go: wherever the shape is,
    /// anywhere across a band taller than glyphs take up (a line by the
    /// shape's top or bottom looks at the shape just inside it). Each
    /// glyph's own ink is checked against the shape as it's placed, so
    /// these only need to hold every place one could go.
    fn line_spans(&self, y: f32) -> Vec<(f32, f32)> {
        if self.region.is_none() {
            return self.band_spans(y);
        }
        let (top, bottom) = self.extent();
        let inset = (bottom - top) * 1e-4;
        let (top, bottom) = (top + inset, bottom - inset);
        let (above, below) = (y - BAND_ABOVE * self.symbol_scale - self.wander, y + BAND_BELOW * self.symbol_scale + self.wander);
        let mut spans: Vec<(f32, f32)> = Vec::new();
        for i in 0..BAND_SAMPLES {
            let at = (above + (below - above) * i as f32 / (BAND_SAMPLES - 1) as f32).clamp(top, bottom);
            spans = union(&spans, &self.spans_at(at));
        }
        // Spans a hair apart are one: an outline drawn on past its start
        // (a circle's) leaves slivers by it, which glyphs reaching past the
        // edge could otherwise start a line in, a word broken to fit.
        let mut joined: Vec<(f32, f32)> = Vec::new();
        for (left, right) in spans {
            match joined.last_mut() {
                Some(last) if left - last.1 < SPAN_JOIN * self.symbol_scale => last.1 = last.1.max(right),
                _ => joined.push((left, right)),
            }
        }
        joined
    }

    /// For boxes that don't keep glyphs inside: where a line at `y` can
    /// hold glyphs, inside the shape all the way across the band they take
    /// up, from a quarter of the symbol scale above the line to three
    /// quarters below it, as far as it lies within the shape's top and
    /// bottom (a line near either looks at the shape just inside it, so a
    /// rectangle's lines run its full width).
    fn band_spans(&self, y: f32) -> Vec<(f32, f32)> {
        let (top, bottom) = self.extent();
        let inset = (bottom - top) * 1e-4;
        let (top, bottom) = (top + inset, bottom - inset);
        let (above, below) = (y - BASELINE_SHIFT * self.symbol_scale, y + (1.0 - BASELINE_SHIFT) * self.symbol_scale);
        let mut spans: Option<Vec<(f32, f32)>> = None;
        for i in 0..LOOSE_BAND_SAMPLES {
            let at = (above + (below - above) * i as f32 / (LOOSE_BAND_SAMPLES - 1) as f32).clamp(top, bottom);
            let here = self.spans_at(at);
            spans = Some(match spans {
                None => here,
                Some(before) => intersect(&before, &here),
            });
        }
        spans.unwrap_or_default()
    }

    /// A line of a box that doesn't keep glyphs inside: along its spans,
    /// from their starts (or jagged in from them).
    fn fill_loose_line(&self, symbols: Vec<Symbol>, y_pos: f32, rng: &mut dyn RngCore) -> (Vec<Symbol>, Vec<Symbol>) {
        let mut stroke = StrokePath::new();
        for (left, right) in self.band_spans(y_pos) {
            let offset_x = rng.random::<f32>() * self.jagged_edge * self.symbol_scale;
            let current_x = left + offset_x;
            let width = right - left - offset_x;
            let (start, end) = (self.place(current_x, y_pos), self.place(current_x + width, y_pos));
            stroke = stroke.move_to(start.x, start.y).line_to(end.x, end.y);
        }
        let args = SymbolPlacementArgs {
            avoid: self.avoid,
            padding: self.padding,
            symbol_scale: self.symbol_scale,
            spacing: self.spacing,
            baseline_shift: BASELINE_SHIFT,
            angle: None,
            within: None,
            scale_at: None,
            longest_line: None,
        };
        match self.layout {
            TextBoxLineMode::Flat => place_along_path(symbols, &stroke, &args),
            TextBoxLineMode::WordsByBlankSymbol(rules) => place_words_along_path(symbols, &stroke, &args, rules),
        }
    }

    pub fn fill_line_with(&self, symbols: Vec<Symbol>, y_pos: f32, rng: &mut dyn RngCore) -> (Vec<Symbol>, Vec<Symbol>) {
        let Some(region) = &self.region else { return self.fill_loose_line(symbols, y_pos, rng) };
        let spans = self.line_spans(y_pos);
        let args = SymbolPlacementArgs {
            avoid: self.avoid,
            padding: self.padding,
            symbol_scale: self.symbol_scale,
            spacing: self.spacing,
            baseline_shift: BASELINE_SHIFT,
            angle: None,
            within: Some(region),
            scale_at: None,
            longest_line: Some(self.longest_line()),
        };
        let mut placed = Vec::new();
        let mut to_place = symbols;
        // Each line runs on a little past the shape's edges (though not
        // into the next span's room), so glyphs' padding can hang over and
        // their ink meet the edge, or reach as far past it as they may.
        let past = self.symbol_scale + self.overhang;
        // A wandering line strays the same way across all its spans, from
        // where its first starts.
        let wander = match (spans.first(), spans.last()) {
            (Some(first), Some(last)) if self.wander > 0.0 => {
                let origin = first.0 - past;
                Some((Wander::new(self.wander, self.symbol_scale, last.1 + past - origin, rng), origin))
            }
            _ => None,
        };
        for (i, &(left, right)) in spans.iter().enumerate() {
            if to_place.is_empty() {
                break;
            }
            // Jagged lines start in from the edge.
            let room = |gap: f32| (gap / 2.0).min(self.symbol_scale);
            let before = if i == 0 { past } else { room(left - spans[i - 1].1) };
            let after = if i + 1 == spans.len() { past } else { room(spans[i + 1].0 - right) };
            let offset_x = rng.random::<f32>() * self.jagged_edge * self.symbol_scale;
            let from = if offset_x > 0.0 { left + offset_x } else { left - before };
            let to = right + after;
            if to <= from {
                continue;
            }
            let stroke = match &wander {
                None => {
                    let (start, end) = (self.place(from, y_pos), self.place(to, y_pos));
                    StrokePath::new().move_to(start.x, start.y).line_to(end.x, end.y)
                }
                Some((w, origin)) => {
                    // Finely enough that glyphs follow its curve.
                    let pieces = ((to - from) / (self.symbol_scale * WANDER_PIECE)).ceil().max(1.0) as usize;
                    let mut stroke = StrokePath::new();
                    for k in 0..=pieces {
                        let x = from + (to - from) * k as f32 / pieces as f32;
                        let p = self.place(x, y_pos + w.offset(x - origin));
                        stroke = if k == 0 { stroke.move_to(p.x, p.y) } else { stroke.line_to(p.x, p.y) };
                    }
                    stroke
                }
            };
            let (mut run, rest) = match self.layout {
                TextBoxLineMode::Flat => place_along_path(to_place, &stroke, &args),
                TextBoxLineMode::WordsByBlankSymbol(rules) => place_words_along_path(to_place, &stroke, &args, rules),
            };
            to_place = rest;
            // A jagged line keeps the edge it was given.
            if offset_x == 0.0 {
                self.align_run(&mut run, to_place.is_empty(), wander.as_ref());
            }
            placed.extend(run);
        }
        (placed, to_place)
    }

    /// Whether `glyph` fits somewhere on the line at `y_pos` with nothing
    /// else on it.
    fn holds_alone(&self, glyph: &Symbol, y_pos: f32) -> bool {
        let Some(region) = &self.region else { return true };
        let args = SymbolPlacementArgs {
            avoid: self.avoid,
            padding: self.padding,
            symbol_scale: self.symbol_scale,
            spacing: self.spacing,
            baseline_shift: BASELINE_SHIFT,
            angle: None,
            within: Some(region),
            scale_at: None,
            longest_line: None,
        };
        self.line_spans(y_pos).iter().any(|&(left, right)| {
            let (start, end) = (self.place(left - self.symbol_scale, y_pos), self.place(right + self.symbol_scale, y_pos));
            let stroke = StrokePath::new().move_to(start.x, start.y).line_to(end.x, end.y);
            !place_along_path(vec![glyph.clone()], &stroke, &args).0.is_empty()
        })
    }

    /// Whether glyphs placed on a line are where they may be: inside the
    /// shape and clear of what's avoided.
    fn fits(&self, glyphs: &[Symbol]) -> bool {
        glyphs.iter().filter(|g| !g.is_blank()).all(|g| {
            let footprint = g.footprint();
            self.region.as_ref().is_none_or(|region| region.holds(&footprint)) && !self.avoid.is_some_and(|tree| tree.blocks(&footprint, self.padding))
        })
    }

    /// Move a line's glyphs, set from its start, along it as `align` says:
    /// as far as they still fit, all of them (right) or half that
    /// (centre), or each further than the last so the last goes as far as
    /// it can (justify; not the text's last line). In words, justified
    /// lines spread at the blanks. On a wandering line (the wander, and
    /// where it starts along the line), they keep to its curve.
    fn align_run(&self, run: &mut [Symbol], last: bool, wander: Option<&(Wander, f32)>) {
        let shares: Vec<f32> = match self.align {
            Align::Left => return,
            Align::Justify if last => return,
            Align::Right | Align::Center => vec![1.0; run.len()],
            Align::Justify => {
                let letters = run.len() - run.iter().rev().take_while(|g| g.is_blank()).count();
                let words = matches!(self.layout, TextBoxLineMode::WordsByBlankSymbol(_));
                let gaps: Vec<usize> = (0..run.len()).map(|k| {
                    let k = k.min(letters.saturating_sub(1));
                    if words { run[..k].iter().filter(|g| g.is_blank()).count() } else { k }
                }).collect();
                let most = gaps.iter().copied().max().unwrap_or(0);
                if most == 0 {
                    return;
                }
                gaps.iter().map(|&g| g as f32 / most as f32).collect()
            }
        };
        let moved = |d: f32| -> Vec<Symbol> {
            run.iter().zip(&shares).map(|(g, share)| {
                let mut g = g.clone();
                let step = d * share;
                let x = g.position().dot(self.along);
                g.set_position(g.position() + self.along * step);
                if let Some((w, origin)) = wander {
                    let x = x - origin;
                    g.translate_by(self.across * (w.offset(x + step) - w.offset(x)));
                    g.rotate_rad_by(w.angle(x + step) - w.angle(x));
                }
                g
            }).collect()
        };
        // The furthest they go and still fit, closed in on by halving.
        let (mut low, mut high) = (0.0, 3.0 * self.symbol_scale);
        if self.fits(&moved(high)) {
            low = high;
        } else {
            for _ in 0..ALIGN_STEPS {
                let mid = (low + high) / 2.0;
                if self.fits(&moved(mid)) { low = mid } else { high = mid }
            }
        }
        let d = if self.align == Align::Center { low / 2.0 } else { low };
        if d > 0.0 {
            run.clone_from_slice(&moved(d));
        }
    }

    /// A point of the turned shape back where it is on the page.
    fn place(&self, x: f32, y: f32) -> Vec2 {
        self.along * x + self.across * y
    }

    pub fn fill_bounds_with(&self, to_place: Vec<Symbol>, rng: &mut dyn RngCore) -> (Vec<Symbol>, Vec<Symbol>) {
        let (min_y, max_y) = self.extent();
        // Glyphs reaching past the edge may start above the top.
        let top = min_y - self.overhang;
        if self.region.is_none() || self.valign <= 0.0 {
            let (placed, rest, _, _) = self.fill_from(top, to_place, rng);
            return (placed, rest);
        }
        // Set from the top, then again lower by the share of the room left
        // under the last line valign gives, unless that loses text that
        // fitted from the top.
        let (placed, rest, first, last) = self.fill_from(top, to_place.clone(), rng);
        // A hair short of all of it, so the last line isn't pushed out by
        // rounding.
        let room = self.room_below(&last, max_y - min_y) - 0.01 * self.symbol_scale;
        if room <= 0.0 {
            return (placed, rest);
        }
        let shift = room * self.valign.min(1.0);
        let (lower, lower_rest, _, _) = self.fill_from(first + shift, to_place.clone(), rng);
        if !rest.is_empty() || lower_rest.is_empty() {
            return (lower, lower_rest);
        }
        // Lower lines can be shorter: as far down as it all still fits.
        let (mut low, mut high, mut best) = (0.0, shift, (placed, rest));
        for _ in 0..VALIGN_STEPS {
            let mid = (low + high) / 2.0;
            let (moved, moved_rest, _, _) = self.fill_from(first + mid, to_place.clone(), rng);
            if moved_rest.is_empty() { (low, best) = (mid, (moved, moved_rest)) } else { high = mid }
        }
        best
    }

    /// The room left under a line: the most there is between a glyph's
    /// ink and the shape's edge straight below it (at a circle's bottom, the
    /// gap under the line's middle), up to `most`.
    fn room_below(&self, line: &[Symbol], most: f32) -> f32 {
        let mut room: f32 = 0.0;
        for glyph in line.iter().filter(|g| !g.is_blank()) {
            // Its lowest point across the lines, in the turned shape.
            let Some(low) = glyph.footprint().into_iter()
                .map(|p| Vec2::new(p.dot(self.along), p.dot(self.across)))
                .max_by(|a, b| a.y.total_cmp(&b.y)) else { continue };
            let edge = self.shape.iter()
                .flat_map(|ring| ring.iter().enumerate().map(move |(i, &a)| (a, ring[(i + 1) % ring.len()])))
                .filter(|(a, b)| (a.x > low.x) != (b.x > low.x))
                .map(|(a, b)| a.y + (low.x - a.x) * (b.y - a.y) / (b.x - a.x))
                .filter(|&y| y >= low.y)
                .fold(f32::INFINITY, f32::min);
            if edge.is_finite() {
                room = room.max(edge - low.y);
            }
        }
        room.min(most)
    }

    /// Lines from `top` down: what's placed, what isn't, where the first
    /// line that held anything is, and that last such line.
    fn fill_from(&self, top: f32, mut to_place: Vec<Symbol>, rng: &mut dyn RngCore) -> (Vec<Symbol>, Vec<Symbol>, f32, Vec<Symbol>) {
        let (min_y, max_y) = self.extent();
        let mut current_y = top;
        let height = max_y - min_y + self.overhang;
        let mut placed_symbols = Vec::new();
        let mut last = Vec::new();
        let mut first = top;
        let empty_step = if self.region.is_some() { EMPTY_LINE_STEP * self.symbol_scale } else { self.symbol_scale };
        // How far the current line has moved down for its glyphs to fit,
        // and to keep clear of the last line.
        let mut lowered = 0.0;
        let mut cleared = 0.0;
        // Uneven lines (wandering, or their gaps jittered) can come onto
        // the last: how far one goes lower, a step at a time, to clear it.
        let uneven = self.wander > 0.0 || self.line_gap_jitter > 0.0;
        let clear_step = (self.wander.max(self.line_gap_jitter) / 4.0).max(empty_step);
        let clear_reach = 2.0 * self.wander + self.line_gap_jitter + self.symbol_scale;
        let mut last_ink: Vec<Vec<Vec2>> = Vec::new();
        while current_y - min_y < height {
            let (placed_line, not_placed) = self.fill_line_with(to_place.clone(), current_y, rng);
            // A line that stops at a glyph too tall for it, where it would
            // stop with room to spare (a dotted i after a short m, at a
            // shape's top), goes lower, up to a glyph scale: else the rest
            // goes on a line barely below it, and over it.
            if !placed_line.is_empty() && lowered + empty_step <= self.symbol_scale && current_y + empty_step - min_y < height
                && not_placed.iter().find(|g| !g.is_blank()).is_some_and(|g| !self.holds_alone(g, current_y)) {
                current_y += empty_step;
                lowered += empty_step;
                continue;
            }
            if uneven && !last_ink.is_empty() && cleared < clear_reach && current_y + clear_step - min_y < height
                && placed_line.iter().filter(|g| !g.is_blank()).any(|g| {
                    let ink = g.footprint();
                    last_ink.iter().any(|other| shapes_within(&ink, other, LINE_GAP * self.symbol_scale))
                }) {
                current_y += clear_step;
                cleared += clear_step;
                continue;
            }
            (lowered, cleared) = (0.0, 0.0);
            to_place = not_placed;
            let max_height = placed_line.iter()
                .map(|symbol| symbol.hull_aabb().extents().y * self.symbol_scale / 2.0)
                .fold(0.0, f32::max);
            if !placed_line.is_empty() {
                if last.is_empty() {
                    first = current_y;
                }
                if uneven {
                    last_ink = placed_line.iter().filter(|g| !g.is_blank()).map(|g| g.footprint()).collect();
                }
                last = placed_line.clone();
            }
            placed_symbols.extend(placed_line);
            current_y += if max_height > 0.0 {
                // Each line's gap from the last a little off, if it may be.
                let off = if self.line_gap_jitter > 0.0 { rng.random_range(-self.line_gap_jitter..self.line_gap_jitter) } else { 0.0 };
                (max_height + off).max(empty_step)
            } else {
                empty_step
            };
            if to_place.is_empty() {
               break;
            }
        }
        (placed_symbols, to_place, first, last)
    }
}

/// Where both sets of spans, each left to right and apart, overlap.
fn intersect(a: &[(f32, f32)], b: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() && j < b.len() {
        let (left, right) = (a[i].0.max(b[j].0), a[i].1.min(b[j].1));
        if right > left {
            out.push((left, right));
        }
        if a[i].1 < b[j].1 { i += 1 } else { j += 1 }
    }
    out
}

/// Where either set of spans, each left to right and apart, is: left to
/// right and apart.
fn union(a: &[(f32, f32)], b: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut all: Vec<(f32, f32)> = a.iter().chain(b).copied().collect();
    all.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out: Vec<(f32, f32)> = Vec::new();
    for (left, right) in all {
        match out.last_mut() {
            Some(last) if left <= last.1 => last.1 = last.1.max(right),
            _ => out.push((left, right)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::journal::symbol::{SymbolBase, SymbolType};
    use std::rc::Rc;

    fn options(shape: Vec<Vec<Vec2>>) -> TextBoxOptions<'static> {
        TextBoxOptions {
            jagged_edge: 0.0,
            layout: TextBoxLineMode::Flat,
            shape,
            angle: 0.0,
            symbol_scale: 10.0,
            avoid: None,
            padding: 0.0,
            spacing: 0.0,
            align: Align::Left,
            contain: true,
            valign: 0.0,
            overhang: 0.0,
            wander: 0.0,
            line_gap_jitter: 0.0,
        }
    }

    fn textbox(shape: Vec<Vec<Vec2>>, angle: f32) -> TextBox<'static> {
        TextBox::new(TextBoxOptions { angle, ..options(shape) })
    }

    #[test]
    fn a_short_glyph_doesnt_keep_a_tall_one_off_its_line() {
        // A short glyph (an m) fits at the box's top; a tall one (an i,
        // dotted) doesn't. The line goes lower, holding both, rather than
        // leaving the m alone on a line just above the rest.
        let glyph = |d: &str| Symbol::new_default_from_base_rc(Rc::new(SymbolBase::new(SymbolType::Pathed(d.parse().unwrap()))));
        let short = glyph("M -0.5 0 L 0.5 0.5");
        let tall = glyph("M -0.2 -1 L 0.2 0.5");
        let tb = textbox(rectangle(bounding_volume::Aabb::new(Vec2::new(-50.0, -20.0), Vec2::new(50.0, 20.0))), 0.0);
        let (placed, rest) = tb.fill_bounds_with(vec![short, tall], &mut rand::rngs::mock::StepRng::new(0, 1));
        assert!(rest.is_empty());
        assert!(placed[0].position().x < placed[1].position().x, "{:?} {:?}", placed[0].position(), placed[1].position());
    }

    #[test]
    fn a_rectangle_runs_its_lines_its_full_width() {
        let tb = textbox(rectangle(bounding_volume::Aabb::new(Vec2::new(-50.0, -20.0), Vec2::new(50.0, 20.0))), 0.0);
        for y in [-20.0, 0.0, 19.0] {
            assert_eq!(tb.line_spans(y), vec![(-50.0, 50.0)], "at {}", y);
        }
    }

    #[test]
    fn lines_look_wherever_glyphs_might_go() {
        let square = |h: f32| vec![Vec2::new(-h, -h), Vec2::new(h, -h), Vec2::new(h, h), Vec2::new(-h, h)];
        // The band from -35 to -15 misses the hole; from -7 to 13 it's
        // across it all the way.
        let tb = textbox(vec![square(50.0), square(15.0)], 0.0);
        assert_eq!(tb.line_spans(-30.0), vec![(-50.0, 50.0)]);
        assert_eq!(tb.line_spans(-2.0), vec![(-50.0, -15.0), (15.0, 50.0)]);
        // A circle drawn on past its start is filled all the same.
        let circle = StrokePath::circle(Vec2::ZERO, 80.0).flattened(0.1).into_iter().map(|(ring, _)| ring).collect();
        let spans = textbox(circle, 0.0).line_spans(0.0);
        assert!(spans.len() == 1 && spans[0].0 < -79.0 && spans[0].1 > 79.0, "{:?}", spans);
        // A diamond is widest at the band's top.
        let diamond = vec![vec![Vec2::new(0.0, -50.0), Vec2::new(50.0, 0.0), Vec2::new(0.0, 50.0), Vec2::new(-50.0, 0.0)]];
        let spans = textbox(diamond, 0.0).line_spans(10.0);
        assert_eq!(spans.len(), 1);
        assert!((spans[0].0 + 45.0).abs() < 1e-3 && (spans[0].1 - 45.0).abs() < 1e-3, "{:?}", spans);
    }

    #[test]
    fn loose_lines_hold_glyphs_where_their_band_is_inside() {
        let square = |h: f32| vec![Vec2::new(-h, -h), Vec2::new(h, -h), Vec2::new(h, h), Vec2::new(-h, h)];
        let tb = TextBox::new(TextBoxOptions { contain: false, ..options(vec![square(50.0), square(10.0)]) });
        assert_eq!(tb.line_spans(-30.0), vec![(-50.0, 50.0)]);
        // The band from -2.5 to 7.5 crosses the hole.
        assert_eq!(tb.line_spans(0.0), vec![(-50.0, -10.0), (10.0, 50.0)]);
        // A diamond narrows below the line, so the band's bottom bounds it.
        let diamond = vec![vec![Vec2::new(0.0, -50.0), Vec2::new(50.0, 0.0), Vec2::new(0.0, 50.0), Vec2::new(-50.0, 0.0)]];
        let spans = TextBox::new(TextBoxOptions { contain: false, ..options(diamond) }).line_spans(10.0);
        assert_eq!(spans.len(), 1);
        assert!((spans[0].0 + 32.5).abs() < 1e-3 && (spans[0].1 - 32.5).abs() < 1e-3, "{:?}", spans);
    }

    #[test]
    fn a_region_holds_only_what_is_wholly_inside() {
        let square = |c: Vec2, h: f32| vec![c + Vec2::new(-h, -h), c + Vec2::new(h, -h), c + Vec2::new(h, h), c + Vec2::new(-h, h)];
        let region = Region::new(vec![square(Vec2::ZERO, 50.0), square(Vec2::ZERO, 10.0)]);
        assert!(region.holds(&square(Vec2::new(30.0, 0.0), 5.0)));
        assert!(!region.holds(&square(Vec2::new(48.0, 0.0), 5.0)), "across the edge");
        assert!(!region.holds(&square(Vec2::ZERO, 5.0)), "in the hole");
        assert!(!region.holds(&square(Vec2::new(0.0, 0.0), 20.0)), "round the hole");
        assert!(!region.holds(&square(Vec2::new(100.0, 0.0), 5.0)), "outside");
    }

    fn boxes(count: usize) -> Vec<Symbol> {
        let base = Rc::new(SymbolBase::new(SymbolType::Pathed("M -1 -1 L 1 -1 L 1 1 L -1 1 Z".parse().unwrap())));
        (0..count).map(|_| Symbol::new_default_from_base_rc(base.clone())).collect()
    }

    #[test]
    fn justified_glyphs_fill_a_circle_to_its_edge() {
        let r = 80.0;
        let circle: Vec<Vec<Vec2>> = StrokePath::circle(Vec2::ZERO, r).flattened(0.1).into_iter().map(|(ring, _)| ring).collect();
        let tb = TextBox::new(TextBoxOptions { align: Align::Justify, ..options(circle.clone()) });
        let (placed, unplaced) = tb.fill_bounds_with(boxes(400), &mut <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(1));
        assert!(!placed.is_empty() && !unplaced.is_empty());
        let region = Region::new(circle);
        for glyph in &placed {
            assert!(region.holds(&glyph.footprint()), "{:?} leaves the circle", glyph.position());
        }
        // Each line's ends reach the circle, as near as a glyph's corner
        // can; and the first line is near the top.
        let mut lines: Vec<(f32, f32, f32)> = Vec::new();
        for glyph in &placed {
            let p = glyph.position();
            match lines.iter_mut().find(|l| (l.0 - p.y).abs() < 1.0) {
                Some(line) => { line.1 = line.1.min(p.x); line.2 = line.2.max(p.x); }
                None => lines.push((p.y, p.x, p.x)),
            }
        }
        for &(y, left, right) in &lines {
            let far = |x: f32| [-5.0f32, 5.0].iter().flat_map(|dx| [-5.0f32, 5.0].map(|dy| Vec2::new(x + dx, y + dy).length())).fold(0.0, f32::max);
            assert!(far(left) > r - 1.0 && far(right) > r - 1.0, "line at {} runs {}..{}", y, left, right);
        }
        let top = lines.iter().map(|l| l.0).fold(f32::MAX, f32::min);
        assert!(top < -r + 10.0, "first line at {}", top);
    }

    #[test]
    fn valign_shares_out_the_room_under_the_text() {
        let square = vec![Vec2::new(-50.0, -50.0), Vec2::new(50.0, -50.0), Vec2::new(50.0, 50.0), Vec2::new(-50.0, 50.0)];
        let ink = |valign: f32| {
            let tb = TextBox::new(TextBoxOptions { valign, ..options(vec![square.clone()]) });
            let (placed, _) = tb.fill_bounds_with(boxes(12), &mut <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(1));
            let ys = placed.iter().flat_map(|g| g.footprint()).map(|p| p.y);
            ys.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)))
        };
        let (top, _) = ink(0.0);
        assert!(top < -49.0, "top {}", top);
        let (_, bottom) = ink(1.0);
        assert!(bottom > 49.0, "bottom {}", bottom);
        let (top, bottom) = ink(0.5);
        assert!(((top + 50.0) - (50.0 - bottom)).abs() < 1.0, "{}..{}", top, bottom);
    }

    #[test]
    fn a_region_with_an_overhang_holds_what_reaches_that_far_past_it() {
        let square = |c: Vec2, h: f32| vec![c + Vec2::new(-h, -h), c + Vec2::new(h, -h), c + Vec2::new(h, h), c + Vec2::new(-h, h)];
        let region = Region::new(vec![square(Vec2::ZERO, 50.0), square(Vec2::ZERO, 10.0)]).with_overhang(5.0);
        assert!(region.holds(&square(Vec2::new(30.0, 0.0), 5.0)), "inside");
        assert!(region.holds(&square(Vec2::new(48.0, 0.0), 5.0)), "3 past the edge");
        assert!(!region.holds(&square(Vec2::new(53.0, 0.0), 5.0)), "8 past the edge");
        assert!(region.holds(&square(Vec2::new(0.0, 13.0), 5.0)), "2 into the hole");
        assert!(!region.holds(&square(Vec2::ZERO, 4.0)), "6 into the hole, all round");
        assert!(!region.holds(&square(Vec2::ZERO, 20.0)), "round the hole");
    }

    #[test]
    fn overhanging_glyphs_reach_past_the_edge_no_further() {
        let circle: Vec<Vec<Vec2>> = StrokePath::circle(Vec2::ZERO, 80.0).flattened(0.1).into_iter().map(|(ring, _)| ring).collect();
        let reach = |overhang: f32| {
            let tb = TextBox::new(TextBoxOptions { overhang, ..options(circle.clone()) });
            let (placed, _) = tb.fill_bounds_with(boxes(400), &mut <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(1));
            placed.iter().flat_map(|g| g.footprint()).map(|p| p.length()).fold(0.0, f32::max)
        };
        assert!(reach(0.0) <= 80.01, "{}", reach(0.0));
        let far = reach(6.0);
        assert!(far > 83.0 && far <= 86.01, "{}", far);
    }

    /// How many glyphs overlap others not next to them (on other lines):
    /// footprints shrunk a little about their middles, so glyphs that only
    /// touch don't count.
    fn overlapping(placed: &[Symbol]) -> usize {
        let shrunk: Vec<Vec<Vec2>> = placed.iter().map(|g| {
            let ink = g.footprint();
            let mid = ink.iter().fold(Vec2::ZERO, |a, p| a + *p) / ink.len() as f32;
            ink.iter().map(|p| mid + (*p - mid) * 0.9).collect()
        }).collect();
        (0..shrunk.len()).flat_map(|i| (i + 2..shrunk.len()).map(move |j| (i, j)))
            .filter(|&(i, j)| crate::journal::avoid::shapes_within(&shrunk[i], &shrunk[j], 0.0))
            .count()
    }

    #[test]
    fn wandering_lines_bend_inside_the_shape_clear_of_each_other() {
        let circle: Vec<Vec<Vec2>> = StrokePath::circle(Vec2::ZERO, 80.0).flattened(0.1).into_iter().map(|(ring, _)| ring).collect();
        let tb = TextBox::new(TextBoxOptions { wander: 6.0, ..options(circle.clone()) });
        let (placed, _) = tb.fill_bounds_with(boxes(400), &mut <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(2));
        assert!(placed.len() > 20);
        let region = Region::new(circle);
        assert!(placed.iter().all(|g| region.holds(&g.footprint())), "a glyph left the circle");
        assert!(placed.iter().any(|g| g.rotation_rad().abs() > 0.05), "the lines should bend");
        assert_eq!(overlapping(&placed), 0);
    }

    #[test]
    fn line_gap_jitter_spaces_lines_unevenly_without_overlap() {
        let square = vec![Vec2::new(-50.0, -50.0), Vec2::new(50.0, -50.0), Vec2::new(50.0, 50.0), Vec2::new(-50.0, 50.0)];
        let rows = |line_gap_jitter: f32| {
            let tb = TextBox::new(TextBoxOptions { line_gap_jitter, ..options(vec![square.clone()]) });
            let (placed, _) = tb.fill_bounds_with(boxes(200), &mut <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(3));
            assert_eq!(overlapping(&placed), 0);
            let mut ys: Vec<f32> = placed.iter().map(|g| g.position().y).collect();
            ys.sort_by(f32::total_cmp);
            ys.dedup_by(|a, b| (*a - *b).abs() < 0.5);
            ys.windows(2).map(|w| w[1] - w[0]).collect::<Vec<f32>>()
        };
        let even = rows(0.0);
        assert!(even.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-3), "{:?}", even);
        let uneven = rows(4.0);
        assert!(uneven.windows(2).any(|w| (w[0] - w[1]).abs() > 0.5), "{:?}", uneven);
    }

    #[test]
    fn turned_lines_run_along_the_angle() {
        let tb = textbox(rectangle(bounding_volume::Aabb::new(Vec2::new(-50.0, -50.0), Vec2::new(50.0, 50.0))), std::f32::consts::FRAC_PI_2);
        // Lines run down the page, the first at the right-hand edge.
        let (top, _) = tb.extent();
        let start = tb.place(tb.line_spans(top)[0].0, top);
        assert!((start - Vec2::new(50.0, -50.0)).length() < 1e-3, "{:?}", start);
    }
}
