use parry2d::bounding_volume;
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
    pub bounds: bounding_volume::Aabb,
    pub symbol_scale: f32,
    /// What the text keeps clear of, and how much further than the tree's
    /// own clearances it keeps.
    pub avoid: Option<&'a AvoidanceTree>,
    pub padding: f32,
    /// Extra space between glyphs, as a fraction of `symbol_scale`.
    pub spacing: f32,
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

pub struct TextBox<'a> {
    layout: TextBoxLineMode,
    jagged_edge: f32,
    bounds: bounding_volume::Aabb,
    symbol_scale: f32,
    avoid: Option<&'a AvoidanceTree>,
    padding: f32,
    spacing: f32,
}

impl<'a> TextBox<'a> {
    pub fn new(options: TextBoxOptions<'a>) -> TextBox<'a> {
        TextBox {
            jagged_edge: options.jagged_edge,
            layout: options.layout,
            avoid: options.avoid,
            padding: options.padding,
            bounds: options.bounds,
            symbol_scale: options.symbol_scale,
            spacing: options.spacing,
        }
    }
    pub fn fill_line_with(&self, symbols: Vec<Symbol>, y_pos: f32, rng: &mut dyn RngCore) -> (Vec<Symbol>, Vec<Symbol>) {
        let offset_x = rng.random::<f32>() * self.jagged_edge * self.symbol_scale;
        let current_x = self.bounds.mins.x + offset_x;
        let width = self.bounds.extents().x - offset_x;
        let stroke = StrokePath::new().move_to(current_x, y_pos).line_to(current_x + width, y_pos);
        let args = SymbolPlacementArgs {
            avoid: self.avoid,
            padding: self.padding,
            symbol_scale: self.symbol_scale,
            spacing: self.spacing,
            baseline_shift: BASELINE_SHIFT,
        };
        match self.layout {
            TextBoxLineMode::Flat => place_along_path(symbols, &stroke, &args),
            TextBoxLineMode::WordsByBlankSymbol(split) => place_words_along_path(symbols, &stroke, &args, split),
        }
    }
    pub fn fill_bounds_with(&self, mut to_place: Vec<Symbol>, rng: &mut dyn RngCore) -> (Vec<Symbol>, Vec<Symbol>) {
        let min_y = self.bounds.mins.y;
        let mut current_y = min_y;
        let height = self.bounds.extents().y;
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
