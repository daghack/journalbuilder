//! Setting text in a box, as a page layout's text slots do: glyphs in
//! straight lines, wrapped at blanks, broken into lines and paragraphs at
//! newlines, aligned, and kept clear of whatever's in the way (a drop cap,
//! an illustration) by an avoidance tree.
//!
//! Text comes as glyphs with the characters they were made from: a glyph
//! from a `'\n'` breaks the line, and blank lines (two newlines or more)
//! break paragraphs. Words are runs of glyphs between blank ones. Setting
//! stops when the box is full, saying how much of the text it used, so the
//! rest can go on in the next box.

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2;
use rand::{Rng, RngCore};
use std::cell::RefCell;
use std::rc::Rc;

use super::avoid::AvoidanceTree;
use super::symbol::{Outline, Spacing, Symbol, WordSplit};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
    /// Both edges straight, by spreading words out; a paragraph's last line
    /// is set left.
    Justify,
}

impl Align {
    pub const NAMES: [&'static str; 4] = ["left", "center", "right", "justify"];
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "left" => Some(Align::Left),
            "center" => Some(Align::Center),
            "right" => Some(Align::Right),
            "justify" => Some(Align::Justify),
            _ => None,
        }
    }
}

/// How text is set. Lengths are multiples of `size` unless said otherwise.
#[derive(Clone, Debug)]
pub struct TextStyle {
    /// The symbol scale: how big glyphs are, in canvas units.
    pub size: f32,
    /// Distance from one line to the next.
    pub line_height: f32,
    pub align: Align,
    /// How far a paragraph's first line is indented.
    pub indent: f32,
    /// Extra space between paragraphs, in lines.
    pub paragraph_gap: f32,
    /// Extra space between glyphs.
    pub spacing: f32,
    /// How far text keeps from what it avoids, in canvas units.
    pub clearance: f32,
    /// Lines start up to this much further in, at random.
    pub jagged: f32,
    /// Whether a word that doesn't fit before something the text avoids
    /// may break round it, going on past it on the same line (see
    /// `WordSplit`; its padding is in sizes too).
    pub split: Option<WordSplit>,
}

impl TextStyle {
    pub fn new(size: f32) -> Self {
        TextStyle {
            size,
            line_height: 1.25,
            align: Align::Left,
            indent: 0.0,
            paragraph_gap: 0.5,
            spacing: 0.0,
            clearance: size / 4.0,
            jagged: 0.0,
            split: None,
        }
    }
}

/// What setting text in a box did.
#[derive(Clone, Debug, Default)]
pub struct Setting {
    /// The glyphs placed, blanks left out.
    pub placed: Vec<Symbol>,
    /// How many glyphs of the text it used up, placed or skipped (blanks at
    /// line ends, line breaks).
    pub consumed: usize,
    /// From the box's top to the bottom of its last line.
    pub height: f32,
    /// The widest line, from the box's left edge.
    pub width: f32,
    /// Whether the text left over starts a paragraph.
    pub paragraph_start: bool,
}

/// Glyphs' outlines at one size, worked out as they're needed.
struct Outlines<'a> {
    glyphs: &'a [Symbol],
    size: f32,
    cache: RefCell<Vec<Option<Rc<Outline>>>>,
}

impl<'a> Outlines<'a> {
    fn new(glyphs: &'a [Symbol], size: f32) -> Self {
        Outlines { glyphs, size, cache: RefCell::new(vec![None; glyphs.len()]) }
    }
    fn get(&self, k: usize) -> Rc<Outline> {
        self.cache.borrow_mut()[k].get_or_insert_with(|| Rc::new(self.glyphs[k].outline(self.size))).clone()
    }
}

/// Stretches of the line from `x0` to `x1` whose band from `y0` to `y1` is
/// clear of `avoid`, at least `min_width` wide.
fn free_spans(x0: f32, x1: f32, y0: f32, y1: f32, avoid: Option<&AvoidanceTree>, style: &TextStyle) -> Vec<(f32, f32)> {
    let Some(avoid) = avoid.filter(|a| !a.is_empty()) else { return vec![(x0, x1)] };
    let probe = (style.size / 4.0).max(0.5);
    let mut spans: Vec<(f32, f32)> = Vec::new();
    let mut open: Option<f32> = None;
    let mut x = x0;
    while x < x1 {
        let next = (x + probe).min(x1);
        let cell = [Vec2::new(x, y0), Vec2::new(next, y0), Vec2::new(next, y1), Vec2::new(x, y1)];
        let blocked = avoid.blocks(&cell, style.clearance);
        match (blocked, open) {
            (false, None) => open = Some(x),
            (true, Some(start)) => {
                spans.push((start, x));
                open = None;
            }
            _ => {}
        }
        x = next;
    }
    if let Some(start) = open {
        spans.push((start, x1));
    }
    spans.retain(|(a, b)| b - a >= style.size);
    spans
}

/// One glyph on a line: which, where its room starts, its width, and where
/// its room starts from its position.
struct Item {
    index: usize,
    x: f32,
    width: f32,
    left: f32,
}

impl Item {
    fn end(&self) -> f32 {
        self.x + self.width
    }
}

/// Set as much of `glyphs` (made from `chars`, one each) as fits in `rect`.
/// `paragraph_start` says whether the text starts a paragraph (and so its
/// first line is indented). Glyphs are scaled up by `style.size` and turned
/// as they were; jagged edges draw on `rng`.
pub fn set_text(rect: Aabb, style: &TextStyle, glyphs: &[Symbol], chars: &[char], paragraph_start: bool, avoid: Option<&AvoidanceTree>, rng: &mut dyn RngCore) -> Setting {
    let size = style.size;
    let line = style.line_height * size;
    let gap = style.spacing * size;
    let n = glyphs.len().min(chars.len());
    let mut setting = Setting { paragraph_start, ..Setting::default() };
    let mut i = 0;
    let mut top = rect.mins.y;
    let mut lines = 0;
    let is_break = |i: usize| chars[i] == '\n';
    let is_space = |i: usize| glyphs[i].is_blank() && chars[i] != '\n';
    let outlines = Outlines::new(&glyphs[..n], size);
    // Glyph k set beside the glyphs in `spacing`, `x` being where it would
    // start set at their widths.
    let place = |spacing: &mut Spacing, k: usize, x: f32| {
        let outline = outlines.get(k);
        let x = spacing.start(&outline, x, gap);
        spacing.push(outline.clone(), x);
        Item { index: k, x, width: outline.width(), left: outline.left() }
    };
    loop {
        // Spaces and breaks before the line's first word.
        let mut breaks = 0;
        while i < n && (is_space(i) || is_break(i)) {
            breaks += is_break(i) as usize;
            i += 1;
        }
        if breaks >= 2 {
            setting.paragraph_start = true;
        }
        if i == n {
            setting.consumed = n;
            break;
        }
        let paragraph_gap = if setting.paragraph_start && lines > 0 { style.paragraph_gap * line } else { 0.0 };
        if top + paragraph_gap + line > rect.maxs.y + 1e-3 {
            break;
        }
        top += paragraph_gap;
        let middle = top + line / 2.0;
        let mut spans = free_spans(rect.mins.x, rect.maxs.x, middle - size / 2.0, middle + size / 2.0, avoid, style);
        if let Some(first) = spans.first_mut() {
            let mut inset = rng.random::<f32>() * style.jagged * size;
            if setting.paragraph_start {
                inset += style.indent * size;
            }
            first.0 = (first.0 + inset).min(first.1);
        }
        // Words into each clear stretch in turn.
        let mut line_items: Vec<(Vec<Item>, (f32, f32))> = Vec::new();
        let mut ended_by_break = false;
        let line_start = i;
        let split_pad = style.split.map_or(0.0, |split| split.padding * size);
        // Whether the stretch before broke a word, which goes on here.
        let mut broke_word = false;
        for (span, &(start, end)) in spans.iter().enumerate() {
            let mut items = Vec::new();
            // The glyphs set in this stretch, which the next is set beside.
            let mut spacing = Spacing::default();
            let mut x = if broke_word { (start + split_pad).min(end) } else { start };
            broke_word = false;
            let mut line_done = false;
            while i < n {
                if is_break(i) {
                    ended_by_break = true;
                    line_done = true;
                    break;
                }
                // The word from i, set from x.
                let mut j = i;
                while j < n && !glyphs[j].is_blank() && !is_break(j) {
                    j += 1;
                }
                let mut after = spacing.clone();
                let mut at = x;
                let word: Vec<Item> = (i..j).map(|k| {
                    let item = place(&mut after, k, at);
                    at = item.end() + gap;
                    item
                }).collect();
                if word.last().is_none_or(|last| last.end() <= end + 1e-3) {
                    if let Some(last) = word.last() {
                        x = last.end() + gap;
                    }
                    items.extend(word);
                    spacing = after;
                    i = j;
                    // Spaces after it, if they fit; otherwise they end the stretch.
                    let mut full = false;
                    while i < n && is_space(i) {
                        let w = outlines.get(i).width();
                        i += 1;
                        if x + w > end {
                            full = true;
                            break;
                        }
                        x += w + gap;
                    }
                    if full {
                        break;
                    }
                } else if i == line_start && word[word.len() - 1].end() - word[0].x > rect.extents().x {
                    // Longer than a whole line: as much as fits, the rest
                    // on the next line.
                    while i < j {
                        let mut after = spacing.clone();
                        let item = place(&mut after, i, x);
                        if item.end() > end + 1e-3 && !items.is_empty() {
                            break;
                        }
                        x = item.end() + gap;
                        items.push(item);
                        spacing = after;
                        i += 1;
                    }
                    line_done = true;
                    break;
                } else {
                    // With split words, as much of the word as fits here
                    // (a piece of at least `min_piece` either side), the
                    // rest past what's in the way.
                    if let Some(split) = style.split.filter(|_| span + 1 < spans.len()) {
                        let min = split.min_piece.max(1);
                        let head = word.iter()
                            .take((j - i).saturating_sub(min))
                            .take_while(|item| item.end() <= end - split_pad + 1e-3)
                            .count();
                        if head >= min {
                            items.extend(word.into_iter().take(head));
                            i += head;
                            broke_word = true;
                        }
                    }
                    break;
                }
            }
            line_items.push((items, (start, end)));
            if line_done || i == n {
                break;
            }
        }
        let last_of_paragraph = ended_by_break || i == n;
        // Align each stretch's words, and place them.
        for (items, (_, end)) in &mut line_items {
            let Some(last) = items.last() else { continue };
            let slack = (*end - last.end()).max(0.0);
            let shift = match style.align {
                Align::Left | Align::Justify => 0.0,
                Align::Center => slack / 2.0,
                Align::Right => slack,
            };
            // Justified lines spread their slack over the gaps between words.
            let word_gaps: Vec<usize> = items.windows(2).enumerate()
                .filter(|(_, pair)| pair[1].index > pair[0].index + 1)
                .map(|(k, _)| k + 1)
                .collect();
            let spread = if style.align == Align::Justify && !last_of_paragraph && !word_gaps.is_empty() {
                slack / word_gaps.len() as f32
            } else {
                0.0
            };
            let mut extra = 0.0;
            for (k, item) in items.iter().enumerate() {
                if word_gaps.contains(&k) {
                    extra += spread;
                }
                let mut glyph = glyphs[item.index].clone();
                glyph.scale_by(size);
                glyph.set_position(Vec2::new(item.x + shift + extra - item.left, middle));
                setting.width = setting.width.max(item.end() + shift + extra - rect.mins.x);
                if !glyph.is_blank() {
                    setting.placed.push(glyph);
                }
            }
        }
        if i > line_start {
            setting.paragraph_start = false;
        }
        top += line;
        lines += 1;
        setting.height = top - rect.mins.y;
        setting.consumed = i;
    }
    setting
}

/// The largest size up to `style.size` at which all of the text fits in
/// `rect`, to within a hundredth; `None` if it doesn't fit even tiny.
pub fn fitting_size(rect: Aabb, style: &TextStyle, glyphs: &[Symbol], chars: &[char], paragraph_start: bool, avoid: Option<&AvoidanceTree>) -> Option<f32> {
    let fits = |size: f32| {
        let mut sized = style.clone();
        sized.size = size;
        sized.clearance = style.clearance * size / style.size;
        let mut rng = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(0);
        set_text(rect, &sized, glyphs, chars, paragraph_start, avoid, &mut rng).consumed >= glyphs.len().min(chars.len())
    };
    if fits(style.size) {
        return Some(style.size);
    }
    let (mut low, mut high) = (style.size / 100.0, style.size);
    if !fits(low) {
        return None;
    }
    while high - low > style.size / 100.0 {
        let mid = (low + high) / 2.0;
        if fits(mid) { low = mid } else { high = mid }
    }
    Some(low)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::fonts::{FontBook, DEFAULT_FONTS};
    use rand::SeedableRng;

    /// Jagged edges are off in these tests, so the generator is unused.
    fn rng() -> rand::rngs::StdRng {
        rand::rngs::StdRng::seed_from_u64(0)
    }

    fn text(s: &str) -> (Vec<Symbol>, Vec<char>) {
        let symd = FontBook::from_file(DEFAULT_FONTS).unwrap();
        let glyphs = symd.font("loche_upper").unwrap().build(s, &symd, &mut rng());
        (glyphs, s.chars().collect())
    }

    fn rect(w: f32, h: f32) -> Aabb {
        Aabb::new(Vec2::ZERO, Vec2::new(w, h))
    }

    fn lines(setting: &Setting) -> Vec<f32> {
        let mut ys: Vec<f32> = setting.placed.iter().map(|g| g.position().y).collect();
        ys.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
        ys
    }

    #[test]
    fn text_wraps_at_words_and_stops_when_the_box_is_full() {
        let (g, c) = text(&"the quick brown fox jumps over the lazy dog again and again and again ".repeat(3));
        let setting = set_text(rect(150.0, 60.0), &TextStyle::new(10.0), &g, &c, true, None, &mut rng());
        assert_eq!(lines(&setting).len(), 4, "12.5 high lines in 60");
        assert!(setting.consumed < g.len() && setting.consumed > 0);
        // It stops between words: the rest starts with a word.
        assert!(c[setting.consumed - 1] == ' ' && c[setting.consumed] != ' ', "{:?}", &c[setting.consumed..]);
        for glyph in &setting.placed {
            let x = glyph.position().x;
            assert!(x > 0.0 && x < 150.0, "{}", x);
        }
    }

    #[test]
    fn newlines_break_lines_and_blank_lines_paragraphs() {
        let (g, c) = text("one\ntwo\n\nthree");
        let mut style = TextStyle::new(10.0);
        style.indent = 2.0;
        let setting = set_text(rect(500.0, 500.0), &style, &g, &c, true, None, &mut rng());
        let ys = lines(&setting);
        assert_eq!(ys.len(), 3);
        assert!((ys[1] - ys[0] - 12.5).abs() < 1e-3);
        assert!((ys[2] - ys[1] - 12.5 * 1.5).abs() < 1e-3, "a paragraph gap of half a line: {:?}", ys);
        // Paragraphs' first lines are indented; the line after a single break isn't.
        let first_x = |y: f32| setting.placed.iter().filter(|g| (g.position().y - y).abs() < 1e-3).map(|g| g.position().x).fold(f32::MAX, f32::min);
        assert!(first_x(ys[0]) > 20.0 && first_x(ys[1]) < 20.0 && first_x(ys[2]) > 20.0);
        assert_eq!(setting.consumed, g.len());
    }

    #[test]
    fn alignment_moves_lines() {
        let (g, c) = text("a short line");
        let x_of = |align| {
            let mut style = TextStyle::new(10.0);
            style.align = align;
            let s = set_text(rect(300.0, 20.0), &style, &g, &c, false, None, &mut rng());
            (s.placed.first().unwrap().position().x, s.placed.last().unwrap().position().x)
        };
        let (left, center, right) = (x_of(Align::Left), x_of(Align::Center), x_of(Align::Right));
        assert!(left.0 < center.0 && center.0 < right.0);
        assert!((right.1 - 300.0).abs() < 10.0, "{:?}", right);
        assert!(((center.0 - 0.0) - (300.0 - center.1)).abs() < 6.0, "{:?}", center);
    }

    #[test]
    fn justified_lines_reach_both_edges_but_the_last() {
        let (g, c) = text("the quick brown fox jumps over the lazy dog again and again and over the lazy dog the end");
        let mut style = TextStyle::new(10.0);
        style.align = Align::Justify;
        let s = set_text(rect(150.0, 200.0), &style, &g, &c, false, None, &mut rng());
        let ys = lines(&s);
        let last_x = |y: f32| s.placed.iter().filter(|g| (g.position().y - y).abs() < 1e-3).map(|g| g.position().x).fold(f32::MIN, f32::max);
        for y in &ys[..ys.len() - 1] {
            assert!(last_x(*y) > 140.0, "line at {} ends at {}", y, last_x(*y));
        }
        // The last line is set as it would be set left.
        let left = set_text(rect(150.0, 200.0), &TextStyle::new(10.0), &g, &c, false, None, &mut rng());
        let on_last = |s: &Setting| s.placed.iter().filter(|g| (g.position().y - ys[ys.len() - 1]).abs() < 1e-3).map(|g| g.position().x).collect::<Vec<_>>();
        assert_eq!(on_last(&s), on_last(&left));
        assert!(on_last(&s).len() > 2);
    }

    #[test]
    fn leant_text_is_set_as_closely_as_upright() {
        let (upright, c) = text("the quick brown fox jumps over the lazy dog");
        let leant: Vec<Symbol> = upright.iter().map(|g| if g.is_blank() { g.clone() } else { g.clone().with_skew(0.3) }).collect();
        let set = |g: &[Symbol]| set_text(rect(1000.0, 20.0), &TextStyle::new(10.0), g, &c, false, None, &mut rng());
        let (a, b) = (set(&upright), set(&leant));
        // Wider by about the lean of its last glyph, not of every glyph.
        let lean = 0.3f32.tan() * 10.0;
        assert!((b.width - a.width).abs() < lean, "{} -> {}", a.width, b.width);
        // And no glyph runs into another.
        for pair in b.placed.windows(2) {
            let shape = |g: &Symbol| parry2d::shape::ConvexPolygon::from_convex_hull(&g.footprint()).unwrap();
            let pose = parry2d::math::Pose::identity();
            let contact = parry2d::query::contact(&pose, &shape(&pair[0]), &pose, &shape(&pair[1]), 1.0).unwrap();
            assert!(contact.is_none_or(|c| c.dist > -1e-3), "{:?}", contact);
        }
    }

    #[test]
    fn split_words_break_round_what_text_avoids() {
        let (g, c) = text("abcdefghijklmnop");
        let mut avoid = AvoidanceTree::new();
        avoid.add_polygon(&[Vec2::new(60.0, -50.0), Vec2::new(70.0, -50.0), Vec2::new(70.0, 50.0), Vec2::new(60.0, 50.0)], 0.0);
        let set = |split| {
            let style = TextStyle { split, ..TextStyle::new(10.0) };
            set_text(rect(400.0, 20.0), &style, &g, &c, false, Some(&avoid), &mut rng())
        };
        let whole = set(None);
        assert!(whole.placed[0].position().x > 70.0, "a whole word goes past it");
        let split = set(Some(WordSplit { min_piece: 2, padding: 0.0 }));
        assert_eq!(split.placed.len(), 16);
        assert!(split.placed[0].position().x < 60.0 && split.placed[15].position().x > 70.0, "a split one goes either side");
        assert_eq!(lines(&split).len(), 1);
        let before = |s: &Setting| s.placed.iter().filter(|g| g.position().x < 60.0).count();
        let padded = set(Some(WordSplit { min_piece: 2, padding: 1.5 }));
        assert!(before(&padded) < before(&split) && padded.placed.len() == 16, "padding keeps pieces further off");
        let long = set(Some(WordSplit { min_piece: 20, padding: 0.0 }));
        assert!(long.placed[0].position().x > 70.0, "too short to split");
    }

    #[test]
    fn text_flows_around_what_it_avoids() {
        let (g, c) = text(&"the quick brown fox jumps over the lazy dog again and again ".repeat(4));
        let mut avoid = AvoidanceTree::new();
        // A drop cap's box in the top left corner.
        avoid.add_polygon(&[Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0), Vec2::new(40.0, 30.0), Vec2::new(0.0, 30.0)], 0.0);
        let s = set_text(rect(200.0, 100.0), &TextStyle::new(10.0), &g, &c, true, Some(&avoid), &mut rng());
        for glyph in &s.placed {
            let p = glyph.position();
            if p.y < 30.0 + 7.5 {
                assert!(p.x > 40.0, "a glyph at {:?} overlaps the drop cap", p);
            }
        }
        assert!(s.placed.iter().any(|g| g.position().y > 40.0 && g.position().x < 20.0), "lines below it start at the edge");
    }

    #[test]
    fn fitting_finds_a_size_that_fits() {
        let (g, c) = text("a heading that is too long");
        let style = TextStyle::new(40.0);
        let size = fitting_size(rect(200.0, 50.0), &style, &g, &c, false, None).expect("fits at some size");
        assert!(size < 40.0 && size > 5.0, "{}", size);
        let mut sized = style.clone();
        sized.size = size;
        assert_eq!(set_text(rect(200.0, 50.0), &sized, &g, &c, false, None, &mut rng()).consumed, g.len());
    }
}
