use scribe::journal::{StrokePath, symboldict};
use parry2d::math::{Vec2, Rot2};
use scribe::journal::symbol::{Symbol, AvoidanceKdTree};
use scribe::journal::canvas::{Canvas, StrokeStyle, CanvasShape, CanvasStyleOptions, textbox::{TextBox, TextBoxOptions}};
use scribe::journal::canvas::artistic::{symbols_in_circle, SymbolCircleParams, SymbolCircleStyleOptions, MazeOptions};
use scribe::journal::styles;
use scribe::journal::parse::markdown::{parse_markdown_content};
use scribe::journal::parse::intermediate::{JournalContent, TextStylizing};
use scribe::journal::rhaientries::JournalEngine;

use scribe::{composite_from_locktypes};
use scribe::lockbuilder::locks::maze::{MazeLock};
use scribe::lockbuilder::locks::doublering::{DoubleRingLock, DoubleRingOptions, KeyedDoubleRingLock, KeyedDoubleRingOptions};
use scribe::lockbuilder::locks::util::mergealpha::{MergeAlphaInputs, MergeAlpha26Options};
use scribe::lockbuilder::locks::util::splitsymbols::{SplitSymbolsLock, SplitSymbolsOptions};
use scribe::lockbuilder::locks::shift::{ShiftCipherLock, ShiftCipherOptions, VignCipherLock, VignCipherOptions};
use scribe::lockbuilder::locks::huffman::{HuffmanEncodingLock, HuffmanEncodingOptions};
use scribe::lockbuilder::locks::substitution::{SubstitutionCipherLock, SubstitutionCipherOptions};
use scribe::lockbuilder::{InfoVar, LockConfig, CompositeLock, Lock};

use std::rc::Rc;
use std::collections::{HashMap, VecDeque};

use stretch::{node::Node, style::*, geometry::{Size, Rect}, Stretch};

fn load_markdown_file(filename: &str) -> String {
    std::fs::read_to_string(filename).expect("Failed to read markdown file")
}

pub struct LockParseArgs {
    pub live_locks: HashMap<String, CompositeLock>,
    pub live_inputs: HashMap<String, HashMap<String, InfoVar>>,
    pub live_outputs: HashMap<String, HashMap<String, InfoVar>>,
}

fn column_layout(width: f32, height: f32, cols: usize) -> Result<(stretch::node::Node, stretch::Stretch), stretch::Error> {
    let mut stretch = stretch::node::Stretch::new();

    let mut children = Vec::new();
    for _ in 0..cols {
        let child = stretch.new_node(
            Style { size: Size { width: Dimension::Percent(1.0), height: Dimension::Auto }, ..Default::default() },
            vec![],
        )?;
        children.push(child);
    }

    let node = stretch.new_node(
        Style {
            size: Size { width: Dimension::Points(width), height: Dimension::Points(height) },
            justify_content: JustifyContent::Center,
            ..Default::default()
        },
        children,
    )?;

    stretch.compute_layout(node, Size::undefined())?;
    Ok((node, stretch))
}

fn centered_layout(width: f32, height: f32, tbmargin_percentile: f32, lrmargin_percentile: f32) -> Result<(stretch::node::Node, stretch::Stretch), stretch::Error> {
    let mut stretch = stretch::node::Stretch::new();

    let child = stretch.new_node(
        Style {
            size: Size { width: Dimension::Percent(1.0), height: Dimension::Auto },
                margin: Rect {
                    start: Dimension::Percent(lrmargin_percentile),
                    end: Dimension::Percent(lrmargin_percentile),
                    top: Dimension::Percent(tbmargin_percentile),
                    bottom: Dimension::Percent(tbmargin_percentile),
                },
            ..Default::default()
        },
        vec![],
    )?;

    let node = stretch.new_node(
        Style {
            size: Size { width: Dimension::Points(width), height: Dimension::Points(height) },
            ..Default::default()
        },
        vec![child],
    )?;

    stretch.compute_layout(node, Size::undefined())?;
    Ok((node, stretch))
}

pub struct SymbolBuilderArgs<'builder, 'symdict> {
    pub stroke_style: &'builder StrokeStyle,
    pub symdict: &'symdict symboldict::SymbolPathDict,
    pub ternary_builder: &'builder styles::SymbolStringBuilder<'symdict>,
    pub lower_builder: &'builder styles::SymbolStringBuilder<'symdict>,
    pub upper_builder: &'builder styles::SymbolStringBuilder<'symdict>,
    pub mixed_builder: &'builder styles::SymbolStringBuilder<'symdict>,
}

pub fn draw_ordering_eye(ordering: &str, bounds: &parry2d::bounding_volume::Aabb, symbuild: &SymbolBuilderArgs) -> Canvas {
    let symbols = symbuild.ternary_builder.build(ordering);
    let style_options = StrokeStyle {
        // black hsla
        color: "hsla(210, 30%, 20%, 0.8)".to_string(),
        width: 0.5,
    };
    let params = SymbolCircleParams {
        rotation: 0.0,
        center: Vec2::new(0.0, 0.0),
        radius: 20.0,
        style_options: Some(SymbolCircleStyleOptions {
            stroke_style: style_options.clone(),
            fill_style: None,
        }),
        symbol_scale: 10.0,
    };
    let eye_width_r = 2.5;
    let inner_circle = CanvasShape::Circle{
        radius: params.radius - params.symbol_scale / 1.5,
        center: Vec2::new(0.0, 0.0),
    };
    let outer_circle_rad = params.radius + params.symbol_scale / 1.5;
    let outer_circle = CanvasShape::Circle{
        center: Vec2::new(0.0, 0.0),
        radius: outer_circle_rad,
    };
    let left_corner = Vec2::new(-eye_width_r* outer_circle_rad, 0.0);
    let right_corner = Vec2::new(eye_width_r* outer_circle_rad, 0.0);
    let bottom_corner = Vec2::new(0.0, outer_circle_rad);
    let top_corner = Vec2::new(0.0, -outer_circle_rad);

    let rot = Rot2::new(std::f32::consts::FRAC_PI_2);

    let bl_vector : Vec2 = left_corner - bottom_corner;
    let bl_control_a : Vec2 = bottom_corner + (bl_vector / 3.0);
    let bl_control_b : Vec2 = bottom_corner + 2.0 * (bl_vector / 3.0);
    let bl_perp = rot * bl_vector.normalize();
    let bl_control_point_a = bl_control_a - bl_perp * (params.radius / 2.0);
    let bl_control_point_b = bl_control_b - bl_perp * (params.radius / 2.0);
    let bl_corner = StrokePath::new()
        .move_to(bottom_corner.x, bottom_corner.y)
        .cubic_curve_to(
            bl_control_point_a.x, bl_control_point_a.y,
            bl_control_point_b.x, bl_control_point_b.y,
            left_corner.x, left_corner.y
        );

    let tl_vector : Vec2 = left_corner - top_corner;
    let tl_perp = rot * tl_vector.normalize();
    let tl_control_a : Vec2 = top_corner + (tl_vector / 3.0);
    let tl_control_b : Vec2 = top_corner + 2.0 * (tl_vector / 3.0);
    let tl_control_point_a = tl_control_a + tl_perp * (params.radius / 2.0);
    let tl_control_point_b = tl_control_b + tl_perp * (params.radius / 2.0);
    let tl_corner = StrokePath::new()
        .move_to(top_corner.x, top_corner.y)
        .cubic_curve_to(
            tl_control_point_a.x, tl_control_point_a.y,
            tl_control_point_b.x, tl_control_point_b.y,
            left_corner.x, left_corner.y
        );

    let br_vector : Vec2 = right_corner - bottom_corner;
    let br_contror_a : Vec2 = bottom_corner + (br_vector / 3.0);
    let br_contror_b : Vec2 = bottom_corner + 2.0 * (br_vector / 3.0);
    let br_perp = rot * br_vector.normalize();
    let br_contror_point_a = br_contror_a + br_perp * (params.radius / 2.0);
    let br_contror_point_b = br_contror_b + br_perp * (params.radius / 2.0);
    let br_corner = StrokePath::new()
        .move_to(bottom_corner.x, bottom_corner.y)
        .cubic_curve_to(
            br_contror_point_a.x, br_contror_point_a.y,
            br_contror_point_b.x, br_contror_point_b.y,
            right_corner.x, left_corner.y
        );

    let tr_vector : Vec2 = right_corner - top_corner;
    let tr_perp = rot * tr_vector.normalize();
    let tr_contror_a : Vec2 = top_corner + (tr_vector / 3.0);
    let tr_contror_b : Vec2 = top_corner + 2.0 * (tr_vector / 3.0);
    let tr_contror_point_a = tr_contror_a - tr_perp * (params.radius / 2.0);
    let tr_contror_point_b = tr_contror_b - tr_perp * (params.radius / 2.0);
    let tr_corner = StrokePath::new()
        .move_to(top_corner.x, top_corner.y)
        .cubic_curve_to(
            tr_contror_point_a.x, tr_contror_point_a.y,
            tr_contror_point_b.x, tr_contror_point_b.y,
            right_corner.x, left_corner.y
        );

    let mut circle_canvas = Canvas::new();
    let symbol_canvas = symbols_in_circle(symbols, &params);
    for _ in 0..3 {
        let mut symbol_canvas = symbol_canvas.clone();
        symbol_canvas.add_strokes(inner_circle.clone(), style_options.clone());
        symbol_canvas.add_strokes(outer_circle.clone(), style_options.clone());
        symbol_canvas.add_strokes(bl_corner.clone(), style_options.clone());
        symbol_canvas.add_strokes(tl_corner.clone(), style_options.clone());
        symbol_canvas.add_strokes(br_corner.clone(), style_options.clone());
        symbol_canvas.add_strokes(tr_corner.clone(), style_options.clone());
        symbol_canvas.roughen_by(params.symbol_scale / 10.0);
        circle_canvas.merge_canvas(symbol_canvas);
    }
    let invisible_line = CanvasShape::LineSegment {
        start: bottom_corner,
        end: bottom_corner + Vec2::new(0.0, params.symbol_scale),
    };
    circle_canvas.add_strokes(invisible_line, StrokeStyle { color: "rgba(0,0,0,0)".to_string(), width: 0.0 });
    circle_canvas.fit_within_width(bounds, 0.7);
    circle_canvas
}

pub fn parse_lock_codeblock<'out>(
    lang: &str, part_types: HashMap<&str, &str>, code: &str,
    lock_args: &'out mut LockParseArgs,
) -> Option<(String, &'out Vec<u32>)> {
        let LockParseArgs {
            live_locks,
            live_inputs,
            live_outputs,
        } = lock_args;
    if part_types.contains_key("definition") {
        let lock_config: LockConfig = serde_json::from_str(code).expect("Failed to parse lock definition");
        let composite_func = composite_from_config_func!(
            "shift_cipher", ShiftCipherLock, ShiftCipherOptions,
            "vigenere_cipher", VignCipherLock, VignCipherOptions,
            "huffman_encoding", HuffmanEncodingLock, HuffmanEncodingOptions,
            "doublering", DoubleRingLock, DoubleRingOptions,
            "keyed_doublering", KeyedDoubleRingLock, KeyedDoubleRingOptions,
            "merge_alpha", MergeAlphaInputs, MergeAlpha26Options,
            "split_symbols", SplitSymbolsLock, SplitSymbolsOptions,
            "substitution_cipher", SubstitutionCipherLock, SubstitutionCipherOptions,
            "maze_lock", MazeLock, MazeOptions
        );
        let lock = CompositeLock::from_lockconfig(lock_config.clone(), composite_func)
            .expect("Failed to create lock from config");
        let lock = composite_from_locktypes!(lock_config.clone(),
            "shift_cipher", ShiftCipherLock, ShiftCipherOptions,
            "vigenere_cipher", VignCipherLock, VignCipherOptions,
            "huffman_encoding", HuffmanEncodingLock, HuffmanEncodingOptions,
            "doublering", DoubleRingLock, DoubleRingOptions,
            "keyed_doublering", KeyedDoubleRingLock, KeyedDoubleRingOptions,
            "merge_alpha", MergeAlphaInputs, MergeAlpha26Options,
            "split_symbols", SplitSymbolsLock, SplitSymbolsOptions,
            "substitution_cipher", SubstitutionCipherLock, SubstitutionCipherOptions,
            "maze_lock", MazeLock, MazeOptions
        ).expect("Failed to create lock from config");
        if !live_locks.contains_key(&lock_config.name) {
            live_locks.insert(lock_config.name, lock);
        }
        return None;
    }
    if part_types.contains_key("input") {
        let lockname = lang.split(':').nth(1).expect("No lock name found");
        let input_name = part_types.get("input").expect("No input name found");
        let input_val: InfoVar = code.trim().to_string().into();
        // if the input doesn't exist for this lock, create a new entry
        // and if it does exist, overwrite it (since we may want to update
        // inputs and re-run locks)
        let lock_inputs = live_inputs.entry(lockname.to_string()).or_insert_with(HashMap::new);
        lock_inputs.insert(input_name.to_string(), input_val);
        // because we've updated an input, we should also clear any outputs for
        // this lock, since they may no longer be valid
        live_outputs.remove(lockname);
    }
    if part_types.contains_key("output") {
        // if it's an output, we expect the meta to be in the format "lock:lockname output:outputname"
        let lockname = lang.split(':').nth(1).expect("No lock name found");
        let output_name = part_types.get("output").expect("No output name found");
        if !live_outputs.contains_key(lockname) {
            let lock = live_locks.get(lockname).expect("Lock not found for output");
            let input_vars = live_inputs.get(lockname).expect("Input variables not found for lock");
            let msg = format!("Failed to get lock info for output: lockname={}, output_name={}", lockname, output_name);
            let output_vars = lock.lock_info(input_vars).expect(&msg);
            live_outputs.insert(lockname.to_string(), output_vars.into_iter().collect());
        }
        let outputs = live_outputs.get(lockname).expect("No outputs found for lock");
        for (out_name, out_var) in outputs {
            if out_name == output_name {
                let content : &Vec<u32> = out_var.borrow_data().expect("Failed to borrow output data as Vec<usize>");
                if !content.is_empty() {
                    return Some((out_name.clone(), content));
                }
            }
        }
    }
    return None;
}

pub fn get_style_scaling(style: &TextStylizing) -> f32 {
    let base_scale = style.scaling.unwrap_or(1.0);
    let heading_scale = style.heading_level.map(|level| {
        match level {
            1 => 2.5,
            2 => 1.8,
            3 => 1.2,
            _ => 1.0,
        }
    }).unwrap_or(1.0);
    base_scale * heading_scale
}

pub fn draw_placed_symbols(canvas: &mut Canvas, placed_symbols: &Vec<(Symbol, &TextStylizing)>, symbuild: &SymbolBuilderArgs, symbol_scale: f32) {
    let cutoff_distance = symbol_scale * 2.5; // adjust this multiplier as needed
    let normal_stroke_style = symbuild.stroke_style.clone();
    let mut underlined_symbol_points = Vec::new();
    let mut underline_stroke_style = None;
    let mut strikethrough_symbol_points = Vec::new();
    let mut strikethrough_stroke_style = None;
    for (symbol, style) in placed_symbols.iter() {
        let mut stroke_style = normal_stroke_style.clone();
        if let Some(color) = &style.color {
            stroke_style.color = color.clone();
        }
        let mut add_underline = false;
        if style.underline {
            let bbox = symbol.placed_bounds();
            let bottom_y = bbox.center().y + bbox.half_extents().y;
            let bottom = Vec2::new(bbox.center().x, bottom_y);
            let last_point = underlined_symbol_points.last();
            if let Some(last_point) = last_point {
                let distance = bottom.distance(*last_point);
                if distance < cutoff_distance {
                    underline_stroke_style = Some(stroke_style.clone());
                    underlined_symbol_points.push(bottom);
                } else {
                    add_underline = true;
                }
            } else {
                underline_stroke_style = Some(stroke_style.clone());
                underlined_symbol_points.push(bottom);
            }
        } else {
            add_underline = true;
        }
        if add_underline && !underlined_symbol_points.is_empty() {
            let offset_scalar = symbol_scale * 0.3;
            let underline_style = underline_stroke_style.take().unwrap_or(stroke_style.clone());
            let linecount = 5;
            let y_offset = symbol_scale * 0.3;

            let first_point = underlined_symbol_points.first().unwrap() - Vec2::new(symbol_scale * 0.5, 0.0);
            let last_point = underlined_symbol_points.last().unwrap() + Vec2::new(symbol_scale * 0.5, 0.0);
            let avg_y = underlined_symbol_points.iter().fold(0.0, |acc, p| acc + p.y) / underlined_symbol_points.len() as f32;
            let start_x = first_point.x;
            let end_x = last_point.x;
            let mut path = StrokePath::new()
                .move_to(start_x, avg_y);
            for i in 0..linecount {
                let rand_x_offset = (rand::random::<f32>() - 0.5) * offset_scalar;
                let rand_y_offset = (rand::random::<f32>() - 0.5) * offset_scalar;
                let curve_y_offset_1 = (rand::random::<f32>() - 0.5) * offset_scalar * 2.0;
                let curve_y_offset_2 = (rand::random::<f32>() - 0.5) * offset_scalar * 2.0;
                let control_x_1 = start_x + (end_x - start_x) / 3.0 + rand_x_offset;
                let control_x_2 = start_x + (end_x - start_x) * 2.0 / 3.0 + rand_x_offset;
                let control_y_1 = avg_y + curve_y_offset_1;
                let control_y_2 = avg_y + curve_y_offset_2;
                let mut next_x = end_x;
                if i % 2 == 1 {
                    next_x = start_x;
                }
                let next_x = next_x + rand_x_offset;
                let next_y = avg_y + rand_y_offset + y_offset;
                path = path.cubic_curve_to(
                    control_x_1, control_y_1,
                    control_x_2, control_y_2,
                    next_x, next_y
                );
            }
            canvas.add_strokes(path, underline_style);
            underlined_symbol_points.clear();
        }

        let mut add_strikethrough = false;
        if style.strikethrough {
            let bbox = symbol.placed_bounds();
            let center = bbox.center();
            let last_point = strikethrough_symbol_points.last();
            if let Some(last_point) = last_point {
                let distance = center.distance(*last_point);
                if distance < cutoff_distance {
                    strikethrough_stroke_style = Some(stroke_style.clone());
                    strikethrough_symbol_points.push(center);
                } else {
                    add_strikethrough = true;
                }
            } else {
                strikethrough_stroke_style = Some(stroke_style.clone());
                strikethrough_symbol_points.push(center);
            }
        } else {
            add_strikethrough = true;
        }
        if add_strikethrough && !strikethrough_symbol_points.is_empty() {
            let offset_scalar = symbol_scale * 0.5;
            let strikethrough_style = strikethrough_stroke_style.take().unwrap_or(stroke_style.clone());
            let linecount = 5;
            let first_point = strikethrough_symbol_points.first().unwrap() - Vec2::new(symbol_scale * 0.5, 0.0);
            let last_point = strikethrough_symbol_points.last().unwrap() + Vec2::new(symbol_scale * 0.5, 0.0);
            let mut path = StrokePath::new()
                .move_to(first_point.x, first_point.y);
            for i in 0..linecount {
                let rand_x_offset = (rand::random::<f32>() - 0.5) * offset_scalar;
                let rand_y_offset = (rand::random::<f32>() - 0.5) * offset_scalar;
                let curve_y_offset_1 = (rand::random::<f32>() - 0.5) * offset_scalar * 2.0;
                let curve_y_offset_2 = (rand::random::<f32>() - 0.5) * offset_scalar * 2.0;
                let mut next_x = last_point.x;
                if i % 2 == 1 {
                    next_x = first_point.x;
                }
                let next_x = next_x + rand_x_offset;
                let next_y = strikethrough_symbol_points.first().unwrap().y + rand_y_offset;
                let control_x_1 = (first_point.x + last_point.x) / 3.0 + rand_x_offset;
                let control_x_2 = (first_point.x + last_point.x) * 2.0 / 3.0 + rand_x_offset;
                let control_y_1 = strikethrough_symbol_points.first().unwrap().y + curve_y_offset_1;
                let control_y_2 = strikethrough_symbol_points.first().unwrap().y + curve_y_offset_2;
                path = path.cubic_curve_to(
                    control_x_1, control_y_1,
                    control_x_2, control_y_2,
                    next_x, next_y
                );
            }
            canvas.add_strokes(path, strikethrough_style);
            strikethrough_symbol_points.clear();
        }
        if style.bold {
            stroke_style.width *= 2.0;
            for _ in 0..5 {
                let rough_symbol : StrokePath = symbol.clone().into();
                let rough_symbol = rough_symbol.roughened_by(1.5);
                canvas.add_strokes(rough_symbol, stroke_style.clone());
            }
        }
        let scale_factor = get_style_scaling(style);
        let max_copies_placed = 16;
        let place_count = style.heading_level.map(|level| max_copies_placed - level.max(5) * 2).unwrap_or(1);
        for _ in 0..place_count {
            let path : StrokePath = symbol.into();
            let path = path.roughened_by(symbol_scale * scale_factor / 10.0);
            canvas.add_strokes(path, stroke_style.clone());
        }
        canvas.add_strokes(symbol, stroke_style);
    }
}

pub fn journal_content_to_canvas<'j, 'src>(
    content: &'j JournalContent<'src>, bounds: &parry2d::bounding_volume::Aabb,
    lock_args: &mut LockParseArgs,
    avoid_tree: Option<&AvoidanceKdTree>, symbuild: &SymbolBuilderArgs,
    default_style: &TextStylizing,
    symbol_scale: f32,
) -> (Canvas, Option<Vec<(Symbol, TextStylizing)>>) {
    let mut canvas = Canvas::new();
    let opts = TextBoxOptions {
        bounds: bounds.clone(),
        jagged_edge: 0.5,
        layout: scribe::journal::canvas::textbox::TextBoxLineMode::WordsByBlankSymbol,
        symbol_scale,
        avoid_tree,
        expected_stroke_width: Some(3.0),
    };
    let mut remaining_content = None;
    match content {
        JournalContent::UnplacedSymbols{symbols} => {
            let style_scaling = symbols
                .first()
                .map(|sym| get_style_scaling(&sym.1))
                .unwrap_or(1.0);
            let tbox_opts = TextBoxOptions {
                symbol_scale: symbol_scale * style_scaling,
                ..opts
            };
            let textbox = TextBox::new(tbox_opts);
            let (symbols, styles) : (Vec<Symbol>, Vec<TextStylizing>) = symbols
                .iter()
                .map(|(sym, style)| (sym.clone(), style.clone()))
                .unzip();
            let (placed_symbols, unplaced_symbols) = textbox.fill_bounds_with(symbols);
            let placed_symbols = placed_symbols
                .into_iter()
                .enumerate()
                .map(|(i, sym)| {
                    let style = styles.get(i).expect("Failed to get style for symbol index");
                    (sym, style)
                });
            let unplaced_symbols: Vec<(Symbol, TextStylizing)> = unplaced_symbols
                .into_iter()
                .enumerate()
                .map(|(i, sym)| {
                    let index = i + placed_symbols.len();
                    let style = styles.get(index).expect("Failed to get style for symbol index");
                    (sym, style.clone())
                })
                .collect();
            draw_placed_symbols(&mut canvas, &placed_symbols.collect(), symbuild, symbol_scale);
            if !unplaced_symbols.is_empty() {
                remaining_content.replace(unplaced_symbols);
            }
        },
        JournalContent::TextBlock{styled_text} => {
            // TODO: Handle styled text blocks with different scalings.
            // TODO: Handle inline code.
            let combined_text: String = styled_text.iter().map(|s| s.content_slice).collect();
            let style_scaling = styled_text
                .first()
                .map(|s| get_style_scaling(&s.content))
                .unwrap_or(1.0);
            let tbox_opts = TextBoxOptions {
                symbol_scale: symbol_scale * style_scaling,
                ..opts
            };
            let textbox = TextBox::new(tbox_opts);
            let symbols = symbuild.upper_builder.build(&combined_text);
            let (placed_symbols, unplaced_symbols) = textbox.fill_bounds_with(symbols);

            let placed_symbols : Vec<(Symbol, &TextStylizing)> = placed_symbols.into_iter().enumerate().map(|(i, sym)| {
                let style = content
                    .get_style_of_char_offset(i)
                    .expect("Failed to get style for symbol index");
                (sym, style)
            }).collect();

            let unplaced_symbols : Vec<(Symbol, TextStylizing)> = unplaced_symbols.into_iter().enumerate().map(|(i, sym)| {
                let index = i + placed_symbols.len();
                let style = content
                    .get_style_of_char_offset(index)
                    .expect("Failed to get style for symbol index")
                    .clone();
                (sym, style)
            }).collect();

            draw_placed_symbols(&mut canvas, &placed_symbols, symbuild, symbol_scale);
            if !unplaced_symbols.is_empty() {
                remaining_content.replace(unplaced_symbols);
            }
        }
        JournalContent::CodeBlock{language, content: code} => {
            if let Some(lang) = language {
                // split lang on ' '. The first element is the lang, and the rest are meta options
                // in the format "key:value"
                let lang_parts: Vec<&str> = lang.split_whitespace().collect();
                let lang = lang_parts.first().unwrap_or(&"");
                let meta_parts = lang_parts[1..].to_vec();
                let mut part_types: HashMap<&str, &str> = HashMap::new();
                for part in meta_parts.iter() {
                    let mut split = part.split(':');
                    let part_type = split.next().unwrap_or("");
                    let part_value = split.next().unwrap_or("");
                    part_types.insert(part_type, part_value);
                }
                if lang.starts_with("lock") {
                    let output = parse_lock_codeblock(lang, part_types, code, lock_args);
                    if let Some((var_name, content)) = output {
                        let content_str = content.iter().map(|&v| {
                            let c = if v == 0 {
                                ' '
                            } else {
                                std::char::from_u32((v - 1) + ('a' as u32)).unwrap_or('?')
                            };
                            c
                        }).collect::<String>();

                        let is_ordering =
                            var_name.split('.')
                                .last().unwrap_or("") == "ordering"
                            && content.len() == 27;

                        if is_ordering {
                            let circle_canvas = draw_ordering_eye(
                                &content_str, bounds, symbuild);
                            return (circle_canvas, None);
                        }
                        let symbols = symbuild.ternary_builder.build(content_str.as_str());
                        let symbols : Vec<(Symbol, TextStylizing)> = symbols.into_iter().map(|sym| (sym, default_style.clone())).collect();
                        remaining_content.replace(symbols);
                    }
                } else if lang.starts_with("style") {
                    println!("TODO: Handle style code block in journal content");
                }
            }
        }
        JournalContent::CanvasEmbed{alt: _, url: _} => {
            println!("TODO: Handle canvas embed in journal content");
            println!("\t- Single Symbols");
            println!("\t- Ordering Eyes (after converting from specialized codeblocks)");
        }
        JournalContent::ThematicBreak => {
            println!("TODO: Handle thematic break in journal content");
        }
    }
    (canvas, remaining_content)
}

pub fn get_stretch_leaf_nodes(stretch: &Stretch, node: Node) -> Vec<Node> {
    let mut leaf_nodes = Vec::new();
    let children = stretch.children(node).unwrap_or(Vec::new());
    if children.is_empty() {
        leaf_nodes.push(node);
    } else {
        for child in children {
            leaf_nodes.extend(get_stretch_leaf_nodes(stretch, child));
        }
    }
    leaf_nodes
}

pub fn fill_layout_with_journal_content<'src>(stretch: &Stretch, node: Node, symbol_scale: f32, symbuild_arg: &SymbolBuilderArgs, content_vecdeq: &mut VecDeque<JournalContent<'src>>, lock_args: &mut LockParseArgs) -> Canvas {
    let children = get_stretch_leaf_nodes(stretch, node);
    let mut layout_canvas = Canvas::new();
    for child in children {
        let leaf_canvas =
            fill_leaf_with_journal_content(stretch, child, symbol_scale, symbuild_arg, content_vecdeq, lock_args);
        layout_canvas.merge_canvas(leaf_canvas);
    }
    layout_canvas
}

pub fn fill_leaf_with_journal_content<'src>(stretch: &Stretch, node: Node, symbol_scale: f32, symbuild_arg: &SymbolBuilderArgs, content_vecdeq: &mut VecDeque<JournalContent<'src>>, lock_args: &mut LockParseArgs) -> Canvas {

    let stretch_layout = stretch.layout(node).expect("Failed to get layout for node");
    let layout_left = stretch_layout.location.x;
    let layout_top = stretch_layout.location.y;
    let layout_right = stretch_layout.location.x + stretch_layout.size.width;
    let layout_bottom = stretch_layout.location.y + stretch_layout.size.height;

    let allowed_overlap = 25.0;
    let mut current_y_offset = 0.0;
    let mut parent_canvas = Canvas::new();
    while layout_top + current_y_offset < layout_bottom && !content_vecdeq.is_empty() {
        let content = content_vecdeq.pop_front().unwrap();
        let current_y = layout_top + current_y_offset;
        let bounds = parry2d::bounding_volume::Aabb::new(
            Vec2::new(layout_left, current_y),
            Vec2::new(layout_right, layout_bottom),
        );

        let default_style = TextStylizing::default();
        let (content_canvas, remaining_content) =
            journal_content_to_canvas(&content, &bounds, lock_args, None, symbuild_arg, &default_style, symbol_scale);
        let mut content_canvas_height = content_canvas.bounds().extents().y;

        if content_canvas_height.is_infinite() {
            // occurs when the canvas is empty, so we treat it as having
            // no height to avoid blocking content that could fit later in the layout.
            content_canvas_height = 0.0;
        }
        let next_y = current_y + content_canvas_height;
        if next_y > layout_bottom + allowed_overlap {
            // what we placed exceeds the layout bound more than the allowable overlap,
            // so we put the content back and break to move on to the next layout area.
            content_vecdeq.push_front(content);
            break;
        }
        parent_canvas.merge_canvas(content_canvas);
        current_y_offset += content_canvas_height;
        if let Some(remaining) = remaining_content {
            let unplaced_symbols = JournalContent::UnplacedSymbols{symbols: remaining};
            content_vecdeq.push_front(unplaced_symbols);
        }
    }
    parent_canvas
}

#[derive(Debug)]
pub struct StyleCipheredContent<'s> {
    pub paragraph: &'s str,
    pub messages: Vec<&'s str>,
}

pub fn style_cipher_to_markdown(paragraph: &StyleCipheredContent) -> Option<String> {
    let StyleCipheredContent {
        paragraph: noise_str,
        messages,
    } = paragraph;
    let mut all_unplaced = Vec::new();
    let mut successful_results = Vec::new();
    for msg in messages.iter() {
        successful_results.push(Vec::new());
        let result = find_string_throughout_string(noise_str, msg);
        let mut unplaced_message_chars = Vec::new();
        for (i, result) in result.into_iter().enumerate() {
            let current_results = successful_results.last_mut().unwrap();
            let inner_index = match result {
                Ok(idx) => idx,
                Err(idx) => idx,
            };
            if result.is_ok() {
                current_results.push(inner_index);
            } else {
                let msg_char = msg.chars().nth(i).unwrap_or('?');
                unplaced_message_chars.push(msg_char);
            }
        }
        all_unplaced.extend(unplaced_message_chars.drain(..));
    }
    if all_unplaced.len() > 0 {
        print!("All unplaced message chars: ");
        for c in all_unplaced.iter() {
            print!("\x1b[31m{}\x1b[0m", c);
        }
        println!();
        return None
    } else {
        println!("All message chars successfully placed in noise.");
    }
    let mut colored_indexes = successful_results.get(0).expect("No successful results found").into_iter();
    let mut current_colored = None;
    let mut underline_indexes = successful_results.get(1).expect("No successful results found").into_iter();
    let mut current_underline = None;
    let mut bold_indexes = successful_results.get(2).expect("No successful results found").into_iter();
    let mut current_bold = None;
    let mut styled_ranges = Vec::new();
    let mut current_range_start = 0;
    #[derive(Debug, Clone, Eq, PartialEq)]
    struct CurrentStyle {
        underline: bool,
        bold: bool,
        colored: bool,
    }
    impl CurrentStyle {
        pub fn markdown_style_string(&self) -> String {
            let mut parts = Vec::new();
            if self.bold {
                parts.push("bold");
            }
            if self.underline && !self.colored {
                parts.push("color:darkblue");
            }
            if self.colored && !self.underline {
                parts.push("color:darkred");
            }
            if self.colored && self.underline {
                parts.push("color:darkgreen");
            }
            parts.join(";")
        }
    }
    let mut last_style = CurrentStyle {
        underline: false,
        bold: false,
        colored: false,
    };
    let mut current_styled_text = String::new();
    for (i, c) in noise_str.chars().enumerate() {
        let underline_i = current_underline.take().unwrap_or_else(|| underline_indexes.next().unwrap_or(&usize::MAX));
        let bold_i = current_bold.take().unwrap_or_else(|| bold_indexes.next().unwrap_or(&usize::MAX));
        let colored_i = current_colored.take().unwrap_or_else(|| colored_indexes.next().unwrap_or(&usize::MAX));
        
        let current_style = CurrentStyle {
            underline: i == *underline_i,
            bold: i == *bold_i,
            colored: i == *colored_i,
        };
        if current_style != last_style {
            let characters = current_styled_text.drain(..).collect::<String>();
            styled_ranges.push((current_range_start, i, last_style.clone(), characters));
            current_range_start = i;
        }

        if !current_style.underline {
            current_underline = Some(underline_i);
        }
        if !current_style.bold {
             current_bold = Some(bold_i);
        }
        if !current_style.colored {
            current_colored = Some(colored_i);
        }

        last_style = current_style;
        current_styled_text.push(c);
    }
    styled_ranges.push((current_range_start, noise_str.len(), last_style.clone(), current_styled_text.drain(..).collect()));
    let mut markdown = String::new();
    let non_style = CurrentStyle {
        underline: false,
        bold: false,
        colored: false,
    };
    for (_, _, style, text) in styled_ranges.into_iter() {
        if style == non_style {
            markdown.push_str(&text);
            continue;
        }
        let style_string = style.markdown_style_string();
        let next_string = format!("[{}]({})", text, style_string);
        markdown.push_str(&next_string);
    }
    Some(markdown)
}

pub fn main() {
    let symd = symboldict::SymbolPathDict::from_file("config/symbols.json").expect("Failed to load symbols.json");
    let symdarc = std::sync::Arc::new(symd.clone());

    let mut entry_engine = JournalEngine::new(symdarc);
    let script = r#"
    "#;
    let result = entry_engine.build_entry_str(&script);
    match result {
        Ok(entry) => {
            println!("Publishing Svgs!");
            entry.publish_svgs("markdown_output");
        }
        Err(err) => {
            dbg!(err);
        }
    }

    let filepath = "/Users/daghack/garage/redpath/obsidianVaults/Loche/MarkdownParseTest.md";

    let width = 850.0;
    let height = 1100.0;
    let symbol_scale = 20.0;
    let (root_node, root_stretch) = centered_layout(width, height, 0.05, 0.15).expect("Failed to compute stretch layout");
    let mut layout_iter = vec![(root_node, &root_stretch)].into_iter().cycle();

    let ternary_builder = styles::ternary_builder(&symd, "balanced_ternary");
    let mixed_builder = styles::mixedcase_builder(&symd, "loche_upper", "light_lower");
    let upper_builder = styles::lowercase_builder(&symd, "loche_upper");
    let lower_builder = styles::lowercase_builder(&symd, "light_lower");

    //#TODO: Currently has an issue, probably around bytes vs chars, where an mdash or other
    //multi-byte character in the noise can throw off the indexing for the styles. For now, we just
    //have to make sure to only use single-byte characters in the noise, but ideally this would be
    //fixed to properly handle unicode.
    let copper_letter = StyleCipheredContent {
        paragraph: "Tonight God has filled my heart with understanding, poured into me that which should have been apparent. I underestimated you and your hunger for my attention. I should have noticed it sooner. You did, after all, have such a keen and victorious glint in your eye when you thought you had seen something in mine. Ocular reciprocity, we meet again.

When I think of you, even now, I think of you with that look. Bright, triumphant, and sharply gleaming above lenses of your iconically dysfunctional crimson shades. Such a pity that you managed to mistake me for some other poor soul of God, unique and strange. No. I am normal, Copper. Just one banal lion out of a thousand-and-one banal lions. Roar. Those dressings will serve me perfectly well. Close your eyes. Turn your gaze elsewhere. Pull your hand from the board. All the more to my advantages.

When I spoke to Alcona, I told her that I expected you to come for me by proxy. Imagine my delight when you came for me yourself and my despair of waking whole of body, spirit, and mind. Having turned away from the Path, I admit that I had never conceived I would find myself so profoundly disappointed by another Lazerine. Swallow your light and gag on it, Copper, you are capable of so much more, so much BETTER. SHE demands better of you. I woke up, having been so perfectly under your juvenile power, and found myself whole of mind, body, and spirit. Are you sure you're a Lazarine? Maybe you're just eager to try something delicious and new. I mean, fuck. Nibbling like a petulant child begging a more familiar meal? How simple. You know, I woke up and actually felt a pang of sorrow for you, Copper. You bear such hungers and yet find yourself so impotent to feast.

COPPER. COPPER. I had heard that somebody had appeared, impersonating me. This pleases me immensely. You are the second least dangerous thing I could imagine impersonating me. My rest, I confess, has greatly improved in the days since receiving your letter. Imitationz, as they say, are the ultimate flattery. By all accounts you were notably more interesting as me than is your custom. I'd suggest that you make it a permanent update, except I couldn't help but notice in the various recountings that some critical details were lacking. The wide sound profile is vitally important Copper. Please get the details RIGHT next time. No chimes, too many theiving hands, and far too nimble. With time, I have faith that you'll command a more nuanced, accurate impression of me in no time at all, provided you really apply yourself, Copper. Never mind the small critiques, what facade does not crack at least a little upon inspection. I have seen worse impressions and, truthfully, the more of me the merrier.

All said, please do better.
Loche",
        messages: vec![
            "ceaselesslyyouhavespuninmythoughtsicouldneverwalkawayfromourboardorstopourlittlegame",
            "walkwithmetalkwithmesateyourhungerattheendforwhatsafeastbetweentwofriendsbeyoucatormouse",
            "visionanddreamhereininscribeditwillbeginasitendstwicetwodifferentlazarinesandtwicetwodifferentknives",
        ],
    };
    
    let content = style_cipher_to_markdown(&copper_letter).expect("Failed to convert style cipher to markdown");
    //println!("Markdown content:\n{}", content);

    let intercontent = parse_markdown_content(&content);
    let mut intercontent : VecDeque<JournalContent> = intercontent.into_journal_content().into();

    let stroke_style = StrokeStyle {
        color: "hsla(210, 30%, 20%, 0.8)".to_string(),
        width: 0.5,
    };

    let mut finished_journal_pages = Vec::new();

    // at a top level, we have the layout iter, which contains
    // all the layouts. In each layout, there are multiple areas
    // to fill.

    let symbolbuilderargs = SymbolBuilderArgs {
        stroke_style: &stroke_style,
        symdict: &symd,
        ternary_builder: &ternary_builder,
        lower_builder: &lower_builder,
        upper_builder: &upper_builder,
        mixed_builder: &mixed_builder,
    };

    let mut lock_args = LockParseArgs {
        live_locks: HashMap::new(),
        live_inputs: HashMap::new(),
        live_outputs: HashMap::new(),
    };

    while intercontent.len() > 0 {
        let next_layout = layout_iter.next().expect("Layout iterator should never be empty");
        let mut page = fill_layout_with_journal_content(
            next_layout.1, next_layout.0,
            symbol_scale,
            &symbolbuilderargs,
            &mut intercontent,
            &mut lock_args,
        );
        // because the svg render automatically crops to the bounds of the content, we add an
        // invisible stroke around the layout_bounds.
        let layout_bounds = parry2d::bounding_volume::Aabb::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(width, height),
        );
        page.add_strokes(
            layout_bounds,
            StrokeStyle {
                color: "rgba(0,0,0,0)".to_string(),
                width: 0.0,
            }
        );
        finished_journal_pages.push(page);
    }

    let canvas_style = CanvasStyleOptions {
        roughness: 1.0,
        point_sample_distance: 1.0,
        trail_off_percent: 0.15,
        noise_amplitude: 0.3,
    };

    // mkdir output if it doesn't exist
    std::fs::create_dir_all("markdown_output").expect("Failed to create output directory");

    for (i, mut canvas) in finished_journal_pages.into_iter().enumerate() {
        // add random scribbles to the canvas to imitate Loche scribbling it out.
        let bounds = canvas.bounds();
        let width = bounds.extents().x;
        let height = bounds.extents().y;
        let left_x = bounds.center().x - width / 2.0;
        let right_x = bounds.center().x + width / 2.0;
        let top_y = bounds.center().y - height / 2.0;
        let bottom_y = bounds.center().y + height / 2.0;
        let offset_max = width.min(height) * 0.3;
        let scribble_count = 100;
        let mut last_y = top_y;
        let mut left_to_right = true;
        let mut stroke_path = StrokePath::new().move_to(left_x, last_y);
        for _ in 0..scribble_count {
            let next_x = if left_to_right {
                right_x - (rand::random::<f32>() * offset_max)
            } else {
                left_x + (rand::random::<f32>() * offset_max)
            };
            let mut next_y = f32::MAX;
            let mut big_jump = false;
            while next_y > bottom_y || next_y < top_y {
                let big_jump_chance = 0.3;
                if rand::random::<f32>() < big_jump_chance {
                    big_jump = true;
                    next_y = top_y + (rand::random::<f32>() * (bottom_y - top_y));
                } else {
                    next_y = last_y + (rand::random::<f32>() - 0.5) * offset_max;
                }
            }
            let control_x_1 = left_x + (right_x - left_x) / 3.0 + (rand::random::<f32>() - 0.5) * offset_max;
            let control_x_2 = left_x + (right_x - left_x) * 2.0 / 3.0 + (rand::random::<f32>() - 0.5) * offset_max;
            // because we emuluate a hand-drawn scribble, we want the control points to be farther
            // up the screen than the endpoints, to create more of an arc. So we subtract from the
            // y value instead of adding to it.
            let control_y_1 : f32;
            let control_y_2 : f32;
            if !big_jump {
                control_y_1 = next_y - (rand::random::<f32>() * offset_max / 10.0);
                control_y_2 = next_y - (rand::random::<f32>() * offset_max / 10.0);
            } else {
                if left_to_right {
                    control_y_1 = last_y - (rand::random::<f32>() * offset_max / 10.0);
                    control_y_2 = next_y + (rand::random::<f32>() * offset_max / 10.0);
                } else {
                    control_y_1 = last_y + (rand::random::<f32>() * offset_max / 10.0);
                    control_y_2 = next_y - (rand::random::<f32>() * offset_max / 10.0);
                }
            }
            stroke_path = stroke_path.cubic_curve_to(
                control_x_1, control_y_1,
                control_x_2, control_y_2,
                next_x, next_y
            );
            last_y = next_y;
            left_to_right = !left_to_right;
        }
        canvas.add_strokes(stroke_path, StrokeStyle {
            color: "black".to_string(),
            width: 3.0,
        });
        let svg_data = canvas.to_svg(None).into_bytes();
        let svg_filename = format!("markdown_output/journal_canvas_output_{}.svg", i);
        let mut svg_file = std::fs::File::create(&svg_filename).expect("Failed to create output file");
        std::io::copy(&mut &svg_data[..], &mut svg_file).expect("Failed to write SVG to file");

        let png = canvas.to_png(5.0, Some(&canvas_style));
        let png_filename = format!("markdown_output/journal_canvas_output_{}.png", i);
        let mut file = std::fs::File::create(&png_filename).expect("Failed to create output file");
        std::io::copy(&mut &png[..], &mut file).expect("Failed to write PNG to file");
    }

}

pub fn find_string_throughout_string(primary_content: &str, search_string: &str) -> Vec<Result<usize, usize>> {
    let primary_content = primary_content.to_lowercase();
    let search_string = search_string.to_lowercase();
    let mut indices = Vec::new();
    let mut search_string_contents = search_string.chars();
    let primary_content_chars = primary_content.chars().collect::<Vec<char>>();
    let mut search_start_index = 0;
    while let Some(next_char) = search_string_contents.next() {
        let found_index = primary_content_chars.iter()
            .enumerate()
            .skip(search_start_index)
            .find(|&(_, &c)| c == next_char)
            .map(|(idx, _)| idx);
        if let Some(idx) = found_index {
            indices.push(Ok(idx));
            search_start_index = idx + 1;
        } else {
            indices.push(Err(search_start_index));
            search_start_index += 1;
        }
    }
    indices
}
