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

use super::avoid::AvoidanceTree;
use super::symbol::Symbol;

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

/// A glyph's width at `style`'s size, and where its outline's middle is
/// from its origin (glyphs are centred on their outlines across, not down,
/// so they keep to the line).
fn glyph_width(symbol: &Symbol, size: f32) -> (f32, f32) {
    let hull = symbol.hull_aabb();
    (hull.extents().x * size / 2.0, hull.center().x * size / 2.0)
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

/// One glyph on a line: which, where its box starts, and its width.
struct Item {
    index: usize,
    x: f32,
    width: f32,
    offset: f32,
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
        for &(start, end) in &spans {
            let mut items = Vec::new();
            let mut x = start;
            let mut line_done = false;
            while i < n {
                if is_break(i) {
                    ended_by_break = true;
                    line_done = true;
                    break;
                }
                // The word from i, and its width.
                let mut j = i;
                let mut width = 0.0;
                while j < n && !glyphs[j].is_blank() && !is_break(j) {
                    width += glyph_width(&glyphs[j], size).0 + gap;
                    j += 1;
                }
                let width = width - gap;
                if x + width <= end + 1e-3 {
                    for (k, glyph) in glyphs.iter().enumerate().take(j).skip(i) {
                        let (w, offset) = glyph_width(glyph, size);
                        items.push(Item { index: k, x, width: w, offset });
                        x += w + gap;
                    }
                    i = j;
                    // Spaces after it, if they fit; otherwise they end the stretch.
                    let mut full = false;
                    while i < n && is_space(i) {
                        let (w, _) = glyph_width(&glyphs[i], size);
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
                } else if i == line_start && width > rect.extents().x {
                    // Longer than a whole line: as much as fits, the rest
                    // on the next line.
                    while i < j {
                        let (w, offset) = glyph_width(&glyphs[i], size);
                        if x + w > end + 1e-3 && !items.is_empty() {
                            break;
                        }
                        items.push(Item { index: i, x, width: w, offset });
                        x += w + gap;
                        i += 1;
                    }
                    line_done = true;
                    break;
                } else {
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
            let slack = (*end - (last.x + last.width)).max(0.0);
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
                glyph.set_position(Vec2::new(item.x + shift + extra + item.width / 2.0 - item.offset, middle));
                setting.width = setting.width.max(item.x + shift + extra + item.width - rect.mins.x);
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
        assert!(last_x(ys[ys.len() - 1]) < 140.0);
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
