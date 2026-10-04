use parry2d::bounding_volume;
use crate::journal::symbol::{Symbol, SymbolPlacementArgs, AvoidanceKdTree, place_along_path, place_words_along_path};
use crate::journal::paths::StrokePath;
use super::{Canvas, CanvasColorMap, StrokeStyle};
use tiny_skia::ColorU8;
use parry2d::math::{Vec2};
use kdtree::KdTree;

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
    pub expected_stroke_width: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct ColorMappedTextBoxOptions {
    pub symbol_scale: f32,
    pub sample_spacing: f32,
    pub stroke_width: f32,
    pub width: f32,
    pub height: f32,
}

pub struct ColorMappedTextBoxCollection {
    cmboxes: Vec<ColorMappedTextBox>
}

impl ColorMappedTextBoxCollection {
    pub fn new(colormaps: Vec<CanvasColorMap>, options: &ColorMappedTextBoxOptions) -> Self {
        let mut cmboxes = Vec::new();
        for colormap in colormaps.into_iter() {
            cmboxes.push(ColorMappedTextBox::new(colormap, options.clone()));
        }
        Self {
            cmboxes,
        }
    }
    pub fn new_with_recolor<F>(colormaps: Vec<(CanvasColorMap, F)>, options: &ColorMappedTextBoxOptions) -> Self
    where F: 'static + Fn (&Vec<(f32, &(ColorU8, Vec2))>) -> String {
        let mut cmboxes = Vec::new();
        for (colormap, recolor_func) in colormaps.into_iter() {
            cmboxes.push(ColorMappedTextBox::new_with_recolor(colormap, options.clone(), recolor_func));
        }
        Self {
            cmboxes,
        }
    }
    pub fn place_all_symbols_on_all_canvases(&self, symbols: Vec<Symbol>) -> Vec<Canvas> {
        let mut canvases = Vec::new();
        let mut textbox_iter = self.cmboxes.iter().cycle();
        let mut symbols = symbols;
        loop {
            let textbox = textbox_iter.next().unwrap();
            let (canvas, unplaced) = textbox.place_symbols_on_canvas(symbols);
            if canvas.strokes.is_empty() {
                break;
            }
            canvases.push(canvas);
            symbols = unplaced;
        }
        canvases
    }
}

pub type RemapColorFunc = Box<dyn Fn (&Vec<(f32, &(ColorU8, Vec2))>) -> String >;

pub struct ColorMappedTextBox {
    options: ColorMappedTextBoxOptions,
    color_tree: KdTree<f32, (ColorU8, Vec2), [f32; 2]>,
    avoid_tree: AvoidanceKdTree,
    text_bounds: parry2d::bounding_volume::Aabb,
    recolor_func: Option<RemapColorFunc>,
}


impl ColorMappedTextBox {
    pub fn new(colormap: CanvasColorMap, options: ColorMappedTextBoxOptions) -> Self {
        let color_tree = colormap.into_kdtree(
            options.sample_spacing.clone(),
            options.width.clone(),
            options.height.clone(),
        );
        let text_bounds = parry2d::bounding_volume::Aabb::new(
            Vec2::new(-options.width/ 2.0, -options.height/ 2.0),
            Vec2::new(options.width/ 2.0, options.height/ 2.0),
        );
        let mut avoid_tree = KdTree::new(2);
        let iter_nearest = color_tree.iter_nearest(&[0.0, 0.0], &kdtree::distance::squared_euclidean).expect("Failed to create color map iterator");
        for (_, (color, point)) in iter_nearest {
            if color.alpha() > 0 {
                continue;
            }
            let point = [point.x + text_bounds.mins.x, point.y + text_bounds.mins.y];
            avoid_tree.add(point, (point[0], point[1])).expect("Failed to add point to KdTree");
        }
        let avoid_tree = AvoidanceKdTree::new(avoid_tree, options.symbol_scale / 2.0);
        Self {
            options,
            color_tree,
            avoid_tree,
            text_bounds,
            recolor_func: None,
        }
    }
    pub fn new_with_recolor<F>(colormap: CanvasColorMap, options: ColorMappedTextBoxOptions, recolor_func: F) -> Self
    where F: 'static + Fn (&Vec<(f32, &(ColorU8, Vec2))>) -> String {
        let mut textbox = Self::new(colormap, options);
        textbox.recolor_func = Some(Box::new(recolor_func));
        textbox
    }
    pub fn default(&self) -> TextBoxOptions<'_> {
        TextBoxOptions {
            jagged_edge: 0.0,
            layout: TextBoxLineMode::Flat,
            bounds: self.text_bounds.clone(),
            symbol_scale: self.options.symbol_scale.clone(),
            avoid_tree: Some(&self.avoid_tree),
            expected_stroke_width: Some(1.0),
        }
    }
    pub fn place_symbols_on_canvas(&self, symbols: Vec<Symbol>) -> (Canvas, Vec<Symbol>) {
        let textbox_opts = self.default();
        let textbox = TextBox::new(textbox_opts);
        let (symbols, unplaced) = textbox.fill_bounds_with(symbols);
        let mut canvas = Canvas::new();
        let min_vec : Vec2 = Vec2::new(
            self.text_bounds.mins.x,
            self.text_bounds.mins.y,
        );

        for symbol in symbols {
            let symbol_pos = symbol.position() - min_vec;
            let query_point = [symbol_pos.x, symbol_pos.y];
            let nearest = self.color_tree.nearest(&query_point, 10, &kdtree::distance::squared_euclidean);
            let default_func = Box::new(|nearest_results: &Vec<(f32, &(ColorU8, Vec2))>| {
                let default_color = "rgba(255, 255, 255, 1.0)";
                let mut count = 0;
                let mut avg_color = (0.0, 0.0, 0.0, 0.0);
                for (_, (color, _)) in nearest_results {
                    let alpha = color.alpha();
                    if alpha < 1 {
                        continue;
                    }
                    count += 1;
                    avg_color.0 += color.red() as f32;
                    avg_color.1 += color.green() as f32;
                    avg_color.2 += color.blue() as f32;
                    avg_color.3 += color.alpha() as f32;
                }
                if count == 0 {
                    return default_color.to_string();
                }
                let color = tiny_skia::ColorU8::from_rgba(
                    (avg_color.0 / count as f32) as u8,
                    (avg_color.1 / count as f32) as u8,
                    (avg_color.2 / count as f32) as u8,
                    (avg_color.3 / count as f32) as u8,
                ).premultiply();
                format!("rgba({}, {}, {}, {:.3})", color.red(), color.green(), color.blue(), color.alpha() as f32 / 255.0)
            }) as RemapColorFunc;
            let nearest = match nearest {
                Ok(results) => results,
                Err(_) => Vec::new(),
            };
            let color_func = match &self.recolor_func {
                Some(func) => func,
                None => &default_func,
            };
            let stroke_style = StrokeStyle {
                color: color_func(&nearest),
                width: self.options.stroke_width,
            };
            canvas.add_strokes(symbol, stroke_style);
        }
        (canvas, unplaced)
    }
    pub fn place_all_symbols_on_canvases(&self, mut symbols: Vec<Symbol>) -> Vec<Canvas> {
        let mut canvases = Vec::new();
        loop {
            let (canvas, unplaced) = self.place_symbols_on_canvas(symbols);
            if canvas.strokes.is_empty() {
                break;
            }
            canvases.push(canvas);
            symbols = unplaced;
        }
        canvases
    }
}

pub struct TextBoxCollection<'kd> {
    pub textboxes: Vec<TextBox<'kd>>,
}

impl <'kd> TextBoxCollection<'kd> {
    pub fn new(options: Vec<TextBoxOptions<'kd>>) -> TextBoxCollection<'kd> {
        let mut textboxes = Vec::new();
        for opt in options {
            let textbox = TextBox::new(opt);
            textboxes.push(textbox);
        }
        TextBoxCollection {
            textboxes,
        }
    }
    pub fn place_all_symbols(&self, mut symbols: Vec<Symbol>) -> Vec<Vec<Symbol>> {
        let mut all_textbox_symbols = Vec::new();
        let mut textbox_iter = self.textboxes.iter().cycle();
        loop {
            let textbox = textbox_iter.next().unwrap();
            let (placed_symbols, not_placed) = textbox.fill_bounds_with(symbols);
            if placed_symbols.is_empty() {
                break;
            }
            all_textbox_symbols.push(placed_symbols);
            symbols = not_placed;
        }
        all_textbox_symbols
    }
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
