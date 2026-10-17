use parry2d::bounding_volume;
use parry2d::math::Vec2;
use crate::journal::avoid::AvoidanceTree;
use crate::journal::symbol::{Symbol, SymbolPlacementArgs, WordSplit, place_along_path, place_words_along_path};
use crate::journal::paths::StrokePath;
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
    WordsByBlankSymbol(Option<WordSplit>),
}

/// Glyphs sit a quarter of their height below each line.
const BASELINE_SHIFT: f32 = 0.25;

/// How many places across a line's glyphs it's checked to be inside the
/// shape.
const BAND_SAMPLES: usize = 5;

pub struct TextBox<'a> {
    layout: TextBoxLineMode,
    jagged_edge: f32,
    /// The shape, turned so the lines run along +x.
    shape: Vec<Vec<Vec2>>,
    /// Along the lines, and on to the next.
    along: Vec2,
    across: Vec2,
    symbol_scale: f32,
    avoid: Option<&'a AvoidanceTree>,
    padding: f32,
    spacing: f32,
}

impl<'a> TextBox<'a> {
    pub fn new(options: TextBoxOptions<'a>) -> TextBox<'a> {
        let along = Vec2::new(options.angle.cos(), options.angle.sin());
        let across = Vec2::new(-along.y, along.x);
        let shape = options.shape.iter()
            .map(|ring| ring.iter().map(|p| Vec2::new(p.dot(along), p.dot(across))).collect())
            .collect();
        TextBox {
            jagged_edge: options.jagged_edge,
            layout: options.layout,
            avoid: options.avoid,
            padding: options.padding,
            shape,
            along,
            across,
            symbol_scale: options.symbol_scale,
            spacing: options.spacing,
        }
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

    /// Where a line at `y` can hold glyphs: inside the shape all the way
    /// across the band they take up, from a quarter of the symbol scale
    /// above the line to three quarters below it, as far as it lies
    /// within the shape's top and bottom (a line near either looks at the
    /// shape just inside it, so a rectangle's lines run its full width).
    fn line_spans(&self, y: f32) -> Vec<(f32, f32)> {
        let (top, bottom) = self.extent();
        let inset = (bottom - top) * 1e-4;
        let (top, bottom) = (top + inset, bottom - inset);
        let (above, below) = (y - BASELINE_SHIFT * self.symbol_scale, y + (1.0 - BASELINE_SHIFT) * self.symbol_scale);
        let mut spans: Option<Vec<(f32, f32)>> = None;
        for i in 0..BAND_SAMPLES {
            let at = (above + (below - above) * i as f32 / (BAND_SAMPLES - 1) as f32).clamp(top, bottom);
            let here = self.spans_at(at);
            spans = Some(match spans {
                None => here,
                Some(before) => intersect(&before, &here),
            });
        }
        spans.unwrap_or_default()
    }

    pub fn fill_line_with(&self, symbols: Vec<Symbol>, y_pos: f32, rng: &mut dyn RngCore) -> (Vec<Symbol>, Vec<Symbol>) {
        let mut stroke = StrokePath::new();
        for (left, right) in self.line_spans(y_pos) {
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
        };
        match self.layout {
            TextBoxLineMode::Flat => place_along_path(symbols, &stroke, &args),
            TextBoxLineMode::WordsByBlankSymbol(split) => place_words_along_path(symbols, &stroke, &args, split),
        }
    }

    /// A point of the turned shape back where it is on the page.
    fn place(&self, x: f32, y: f32) -> Vec2 {
        self.along * x + self.across * y
    }

    pub fn fill_bounds_with(&self, mut to_place: Vec<Symbol>, rng: &mut dyn RngCore) -> (Vec<Symbol>, Vec<Symbol>) {
        let (min_y, max_y) = self.extent();
        let mut current_y = min_y;
        let height = max_y - min_y;
        let mut placed_symbols = Vec::new();
        while current_y - min_y < height {
            let (placed_line, not_placed) = self.fill_line_with(to_place, current_y, rng);
            to_place = not_placed;
            let max_height = placed_line.iter()
                .map(|symbol| symbol.hull_aabb().extents().y * self.symbol_scale / 2.0)
                .fold(0.0, f32::max);
            placed_symbols.extend(placed_line);
            current_y += if max_height > 0.0 { max_height } else { self.symbol_scale };
            if to_place.is_empty() {
               break;
            }
        }
        (placed_symbols, to_place)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn textbox(shape: Vec<Vec<Vec2>>, angle: f32) -> TextBox<'static> {
        TextBox::new(TextBoxOptions {
            jagged_edge: 0.0,
            layout: TextBoxLineMode::Flat,
            shape,
            angle,
            symbol_scale: 10.0,
            avoid: None,
            padding: 0.0,
            spacing: 0.0,
        })
    }

    #[test]
    fn a_rectangle_runs_its_lines_its_full_width() {
        let tb = textbox(rectangle(bounding_volume::Aabb::new(Vec2::new(-50.0, -20.0), Vec2::new(50.0, 20.0))), 0.0);
        for y in [-20.0, 0.0, 19.0] {
            assert_eq!(tb.line_spans(y), vec![(-50.0, 50.0)], "at {}", y);
        }
    }

    #[test]
    fn holes_split_lines_and_glyphs_stay_out_of_reach_of_the_edge() {
        let square = |h: f32| vec![Vec2::new(-h, -h), Vec2::new(h, -h), Vec2::new(h, h), Vec2::new(-h, h)];
        let tb = textbox(vec![square(50.0), square(10.0)], 0.0);
        assert_eq!(tb.line_spans(-30.0), vec![(-50.0, 50.0)]);
        // The band from -2.5 to 7.5 crosses the hole.
        assert_eq!(tb.line_spans(0.0), vec![(-50.0, -10.0), (10.0, 50.0)]);
        // A circle drawn on past its start is filled all the same.
        let circle = StrokePath::circle(Vec2::ZERO, 80.0).flattened(0.1).into_iter().map(|(ring, _)| ring).collect();
        let spans = textbox(circle, 0.0).line_spans(0.0);
        assert!(spans.len() == 1 && spans[0].0 < -79.0 && spans[0].1 > 79.0, "{:?}", spans);
        // A diamond narrows below the line, so the band's bottom bounds it.
        let diamond = vec![vec![Vec2::new(0.0, -50.0), Vec2::new(50.0, 0.0), Vec2::new(0.0, 50.0), Vec2::new(-50.0, 0.0)]];
        let spans = textbox(diamond, 0.0).line_spans(10.0);
        assert_eq!(spans.len(), 1);
        assert!((spans[0].0 + 32.5).abs() < 1e-3 && (spans[0].1 - 32.5).abs() < 1e-3, "{:?}", spans);
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
