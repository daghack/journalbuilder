use parry2d::bounding_volume;
use crate::journal::symbol::{Symbol, SymbolPlacementArgs, AvoidanceKdTree, place_along_path, place_words_along_path};
use crate::journal::paths::StrokePath;
use parry2d::math::{Vec2};

#[derive(Clone)]
pub struct TextBoxOptions<'kd> {
    // jagged edge controls the amount (in terms of symbol scale) of jaggedness on the edges of the
    // textbox. A value of 0.0 means flat edges, and a value of 1.0 means that the left hand edge
    // will be offset by up to 1.0 x symbol_scale amount randomly.
    pub jagged_edge: f32,
    pub layout: TextBoxLineMode,
    pub bounds: bounding_volume::Aabb,
    pub symbol_scale: f32,
    pub avoid_tree: Option<&'kd AvoidanceKdTree>,
}


#[derive(Debug, Clone)]
pub enum TextBoxLineMode {
    Flat,
    WordsByBlankSymbol,
}

pub struct TextBox<'kd> {
    layout: TextBoxLineMode,
    jagged_edge: f32,
    bounds: bounding_volume::Aabb,
    symbol_scale: f32,
    avoid_tree: Option<&'kd AvoidanceKdTree>,
}

impl<'kd> TextBox<'kd> {
    pub fn new(options: TextBoxOptions) -> TextBox {
        TextBox {
            jagged_edge: options.jagged_edge,
            layout: options.layout,
            avoid_tree: options.avoid_tree,
            bounds: options.bounds,
            symbol_scale: options.symbol_scale,
        }
    }
    pub fn fill_line_with(&self, symbols: Vec<Symbol>, y_pos: f32) -> (Vec<Symbol>, Vec<Symbol>) {
        let offset_x = rand::random::<f32>() * self.jagged_edge * self.symbol_scale;
        let current_x = self.bounds.mins.x + offset_x;
        let width = (self.bounds.extents().x * 1.05) - offset_x;
        let stroke = StrokePath::new().move_to(current_x, y_pos).line_to(current_x + width, y_pos);
        match self.layout {
            TextBoxLineMode::Flat => {
                place_along_path(symbols, &stroke, &SymbolPlacementArgs {
                    avoid_kdtree: self.avoid_tree,
                    symbol_scale: self.symbol_scale,
                })
            },
            TextBoxLineMode::WordsByBlankSymbol => {
                place_words_along_path(symbols, &stroke, &SymbolPlacementArgs {
                    avoid_kdtree: self.avoid_tree,
                    symbol_scale: self.symbol_scale,
                })
            },
        }
    }
    pub fn fill_bounds_with(&self, mut to_place: Vec<Symbol>) -> (Vec<Symbol>, Vec<Symbol>) {
        let min_y = self.bounds.mins.y;
        let mut current_y = min_y;
        let height = self.bounds.extents().y;
        let mut placed_symbols = Vec::new();
        while current_y - min_y < height {
            let (mut placed_line, not_placed) = self.fill_line_with(to_place, current_y);
            to_place = not_placed;
            let mut max_height: f32 = 0.0;
            for symbol in &mut placed_line {
                let height = symbol.get_convex_hull().local_aabb().extents().y * self.symbol_scale / 2.0;
                max_height =  max_height.max(height);
                let symbol_center = symbol.position();
                let new_pos = symbol_center + Vec2::new(0.0, height / 4.0);
                symbol.set_position(new_pos);
            }
            placed_symbols.extend(placed_line);
            if max_height == 0.0 {
                max_height = self.symbol_scale;
            }
            current_y += max_height;
            if to_place.is_empty() {
               break;
            }
        }
        (placed_symbols, to_place)
    }
    pub fn place_all_symbols(&self, mut symbols: Vec<Symbol>) -> Vec<Vec<Symbol>> {
        // This effectively calls fill_bounds_with_ repeatedly until all symbols are placed
        let mut all_symbols = Vec::new();
        loop {
            let (placed_symbols, not_placed) = self.fill_bounds_with(symbols);
            if placed_symbols.is_empty() {
                break;
            }
            all_symbols.push(placed_symbols);
            symbols = not_placed;
        }
        all_symbols
    }
}
