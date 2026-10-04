use parry2d::math::{Vec2};

use crate::lockbuilder::*;
use crate::lockbuilder::locks::maze::{SymbolInfo};
use crate::journal::canvas::{Canvas, StrokeStyle, CanvasShape, textbox::{TextBox, TextBoxOptions, TextBoxLineMode}};
use crate::journal::JournalEntry;
use crate::journal::symboldict;
use crate::journal::symbol::Symbol;
use crate::journal::paths::StrokePath;
use crate::journal::styles;

use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};
use petgraph::graph::{UnGraph, NodeIndex};
use petgraph::algo::min_spanning_tree;
use rand::{Rng, SeedableRng, Fill};
use std::default::Default;

pub fn build_maze_canvas_from_entry(entry: &JournalEntry, symd: &symboldict::SymbolPathDict) -> Canvas {
    let input_vars = InfoVarSet::new();
    let journal_lock = entry.lock_entries.get("page_one_lock").expect("maze_lock not found");

    let (lock_output, _) = journal_lock.trace_lock_info(&input_vars).expect("Failed to lock info");
    let output_value_strings = string_infovar_set(&lock_output);
    let mut base_canvas = Canvas::new();
    let mut avoid_canvas = Canvas::new();
    let generate_maze : &BuiltMaze<SymbolInfo> = lock_output.get("maze_encode.generated_maze").expect("generated_maze not found").borrow_data::<BuiltMaze<SymbolInfo>>().expect("generated_maze is not a BuiltMaze");

    let radius = 50.0;
    let cell_width = radius;

    let maze_cells = generate_maze.all_cells();
    for cell in maze_cells {
        let pos = cell.position;
        let SymbolInfo { symbol: symbol_val, color, orientation } = cell.inner;
        let symbol_val = match symbol_val {
            x => x as i32 - 13,
        };
        let color = match color {
            1 => "darkred",
            2 => "darkgreen",
            3 => "darkblue",
            _ => "black",
        };
        let orientation = match orientation {
            0 => 0.0,
            1 => std::f32::consts::FRAC_PI_2,
            2 => std::f32::consts::PI,
            3 => 3.0 * std::f32::consts::FRAC_PI_2,
            _ => 0.0,
        };

        let symbol_name = format!("balanced_ternary_{}", symbol_val);
        let mut new_symbol = symd.new_symbol(&symbol_name).expect("Symbol not found");
        new_symbol.set_position(Vec2::new(pos.0 as f32 * cell_width, pos.1 as f32 * cell_width));
        new_symbol.rotate_rad_by(orientation as f32);
        new_symbol.scale_by(radius);
        base_canvas.add_strokes(new_symbol.clone(), StrokeStyle {
            color: color.to_string(),
            width: 5.0,
        });
        avoid_canvas.add_strokes(new_symbol, StrokeStyle {
            color: color.to_string(),
            width: 2.0,
        });
    }
    let maze_bounds = avoid_canvas.bounds();
    let bounds = maze_bounds.scaled_wrt_center(Vec2::new(1.2, 1.2));
    let symbol_scale = 52.0;
    let avoid_scale = symbol_scale / 2.0;
    let avoid_tree = Some(&avoid_canvas.get_avoid_tree(avoid_scale));
    let textbox = TextBox::new(TextBoxOptions {
        jagged_edge: 0.0,
        bounds,
        layout: TextBoxLineMode::Flat,
        symbol_scale: symbol_scale,
        avoid_tree,
    });

    let msg_str = output_value_strings.get("maze_content.key").expect("maze_content.key not found");
    let symbols = styles::ternary_builder(&symd, "balanced_ternary").build(msg_str);
    let (text_symbols, unplaced) = textbox.fill_bounds_with(symbols);
    if unplaced.len() > 0 {
        eprintln!("Warning: Not all text symbols were placed in the textbox");
        eprintln!("{} symbols were not placed", unplaced.len());
    }
    let scale_variance = 0.05;
    let pos_offset_variance = 5.0;
    let rotation_variance = std::f32::consts::PI / 16.0;
    for mut symbol in text_symbols {
        let rand_scale = 1.0 + (rand::random::<f32>() * scale_variance - (scale_variance / 2.0));
        let rand_offset_x = (rand::random::<f32>() * pos_offset_variance) - (pos_offset_variance / 2.0);
        let rand_offset_y = (rand::random::<f32>() * pos_offset_variance) - (pos_offset_variance / 2.0);
        let rand_rot = rand::random::<f32>() * rotation_variance - (rotation_variance / 2.0);
        symbol.scale_by(rand_scale);
        let pos = symbol.position();
        symbol.set_position(Vec2::new(pos.x + rand_offset_x, pos.y + rand_offset_y));
        symbol.rotate_rad_by(rand_rot);
        base_canvas.add_strokes(symbol, StrokeStyle {
            color: "darkred".to_string(),
            width: 1.5,
        });
    }
    base_canvas
}

pub fn build_balanced_ternary_canvas(symd: &symboldict::SymbolPathDict) -> Canvas {
    let mut base_canvas = Canvas::new();
    let symbol_radius = 50.0;
    let ring_radius = 600.0;
    let ring_ratio = 4.0;
    let stroke_width = 3.0;
    let symbol_size_ratio = 1.0;
    let symbol_stroke_ratio = 1.5;

    // we're going to draw a circle of 27 balanced ternary symbols,
    // with the value of 0 at the top, increasing clockwise and
    // decreasing counter-clockwise.
    for i in -13..=13 {
        let angle = (i as f32) * (2.0 * std::f32::consts::PI / 27.0) - (std::f32::consts::PI / 2.0);
        let centerpoint = Vec2::new(
            angle.cos() * ring_radius,
            angle.sin() * ring_radius,
        );
        let canvasdrawoptions = StrokeStyle {
            color: "black".to_string(),
            width: stroke_width,
        };
        let symbol_name = format!("balanced_ternary_{}", i);
        let mut new_symbol = symd.new_symbol(&symbol_name).expect("Symbol not found");
        new_symbol.set_position(centerpoint.clone());
        new_symbol.rotate_rad_by(angle + (std::f32::consts::PI / 2.0));
        new_symbol.scale_by(symbol_radius);
        let circle_radius = symbol_radius / 1.5;
        base_canvas.add_strokes(CanvasShape::Circle {
            center: centerpoint,
            radius: circle_radius,
        }, canvasdrawoptions.clone());
        base_canvas.add_strokes(new_symbol.clone(), canvasdrawoptions.clone());
        // draw a line from the radius of the symbol to the radius of the corresponding
        // symbol in the inner ring's radius
        let inner_digit = (i as f32 / 3.0).round() as i32;
        let inner_angle = (inner_digit as f32) * (2.0 * std::f32::consts::PI / 9.0) - (std::f32::consts::PI / 2.0);
        // find the point on the edge of the symbol circle with the same angle as the centerpoint
        // then extend that point to the inner ring radius
        let outter_point = Vec2::new(
            centerpoint.x + circle_radius * (angle - std::f32::consts::PI).cos(),
            centerpoint.y + circle_radius * (angle - std::f32::consts::PI).sin(),
        );

        let inner_point_center = Vec2::new(
            inner_angle.cos() * (ring_radius - symbol_radius * ring_ratio),
            inner_angle.sin() * (ring_radius - symbol_radius * ring_ratio),
        );
        // get the point on the edge of the inner symbol circle with the same angle as the
        let inner_point = Vec2::new(
            inner_point_center.x + circle_radius * (inner_angle).cos(),
            inner_point_center.y + circle_radius * (inner_angle).sin(),
        );

        base_canvas.add_strokes(CanvasShape::LineSegment {
            start: outter_point,
            end: inner_point,
        }, canvasdrawoptions);
    }

    // inside, draw another ring of the 0-value for each ternary digit
    let inner_ring_radius = ring_radius - symbol_radius * ring_ratio;
    let inner_symbol_radius = symbol_radius * symbol_size_ratio;
    let inner_stroke_width = stroke_width * symbol_stroke_ratio;
    for i in -4..=4 {
        let angle = (i as f32) * (2.0 * std::f32::consts::PI / 9.0) - (std::f32::consts::PI / 2.0);
        let centerpoint = Vec2::new(
            angle.cos() * inner_ring_radius,
            angle.sin() * inner_ring_radius,
        );
        let canvasdrawoptions = StrokeStyle {
            color: "black".to_string(),
            width: inner_stroke_width,
        };
        let digit = i * 3;
        let symbol_name = format!("balanced_ternary_{}", digit);
        let mut new_symbol = symd.new_symbol(&symbol_name).expect("Symbol not found");
        new_symbol.set_position(centerpoint.clone());
        new_symbol.rotate_rad_by(angle + (std::f32::consts::PI / 2.0));
        new_symbol.scale_by(inner_symbol_radius);
        base_canvas.add_strokes(CanvasShape::Circle {
            center: centerpoint,
            radius: inner_symbol_radius / 1.5,
        }, canvasdrawoptions.clone());
        base_canvas.add_strokes(new_symbol.clone(), canvasdrawoptions.clone());
        // draw a line from the radius of the symbol to the radius of the corresponding
        // symbol in the inmost ring's radius
        let inmost_digit = (i as f32 / 3.0).round() as i32;
        let inmost_angle = (inmost_digit as f32) * (2.0 * std::f32::consts::PI / 3.0) - (std::f32::consts::PI / 2.0);
        // find the point on the edge of the symbol circle with the same angle as the centerpoint
        // then extend that point to the inner ring radius
        let outter_point = Vec2::new(
            centerpoint.x + (inner_symbol_radius / 1.5) * (angle - std::f32::consts::PI).cos(),
            centerpoint.y + (inner_symbol_radius / 1.5) * (angle - std::f32::consts::PI).sin(),
        );
        let inmost_point_center = Vec2::new(
            inmost_angle.cos() * (inner_ring_radius - inner_symbol_radius * ring_ratio),
            inmost_angle.sin() * (inner_ring_radius - inner_symbol_radius * ring_ratio),
        );
        // get the point on the edge of the inner symbol circle with the same angle as the
        // inmost_angle
        // then extend that point to the inmost ring radius
        let inmost_point = Vec2::new(
            inmost_point_center.x + (inner_symbol_radius / 1.5) * (inmost_angle).cos(),
            inmost_point_center.y + (inner_symbol_radius / 1.5) * (inmost_angle).sin(),
        );  

        base_canvas.add_strokes(CanvasShape::LineSegment {
            start: outter_point,
            end: inmost_point,
        }, canvasdrawoptions);
    }

    let inmost_ring_radius = inner_ring_radius - inner_symbol_radius * ring_ratio;
    let inmost_symbol_radius = inner_symbol_radius * symbol_size_ratio;
    let inmost_stroke_width = inner_stroke_width * symbol_stroke_ratio;
    for i in -1..=1 {
        let angle = (i as f32) * (2.0 * std::f32::consts::PI / 3.0) - (std::f32::consts::PI / 2.0);
        let centerpoint = Vec2::new(
            angle.cos() * inmost_ring_radius,
            angle.sin() * inmost_ring_radius,
        );
        let canvasdrawoptions = StrokeStyle {
            color: "black".to_string(),
            width: inmost_stroke_width,
        };
        let digit = i * 9;
        let symbol_name = format!("balanced_ternary_{}", digit);
        let mut new_symbol = symd.new_symbol(&symbol_name).expect("Symbol not found");
        new_symbol.set_position(centerpoint.clone());
        new_symbol.rotate_rad_by(angle + (std::f32::consts::PI / 2.0));
        new_symbol.scale_by(inmost_symbol_radius);
        base_canvas.add_strokes(CanvasShape::Circle {
            center: centerpoint,
            radius: inmost_symbol_radius / 1.5,
        }, canvasdrawoptions.clone());
        base_canvas.add_strokes(new_symbol, canvasdrawoptions.clone());
    }

    // add a series of concentric circles to the center
    let num_circles = 5;
    let min_rad = 15.0;
    let max_rad = inmost_ring_radius - inmost_symbol_radius / 2.0;
    for i in 0..num_circles {
        let t = (i as f32) / (num_circles as f32);
        let circle_radius = min_rad + t * (max_rad - min_rad);
        base_canvas.add_strokes(CanvasShape::Circle {
            center: Vec2::new(0.0, 0.0),
            radius: circle_radius,
        }, StrokeStyle {
            color: "black".to_string(),
            width: inmost_stroke_width,
        });
    }

    base_canvas
}

pub fn random_bounding_box(canvas: &Canvas, width_bounds: (f32, f32), height_bounds: (f32, f32)) -> parry2d::bounding_volume::Aabb {
    let minx = canvas.bounds().mins.x;
    let miny = canvas.bounds().mins.y;
    let maxx = canvas.bounds().maxs.x;
    let maxy = canvas.bounds().maxs.y;
    let width_range = width_bounds.1 - width_bounds.0;
    let height_range = height_bounds.1 - height_bounds.0;
    let width = rand::random::<f32>() * width_range + width_bounds.0;
    let height = rand::random::<f32>() * height_range + height_bounds.0;
    let tlx = rand::random::<f32>() * (maxx - minx - width) + minx;
    let tly = rand::random::<f32>() * (maxy - miny - height) + miny;
    parry2d::bounding_volume::Aabb::new(
        Vec2::new(tlx, tly),
        Vec2::new(tlx + width, tly + height),
    )
}

pub fn splat_msg_overtop(canvas: &Canvas, symd: &symboldict::SymbolPathDict, msg: &str, width_bounds: (f32, f32), height_bounds: (f32, f32), symbol_scale: f32, width: f32, color: &str, max_attempts: usize) -> Option<Canvas> {
    let mut builder = styles::lowercase_builder(&symd, "loche_upper");
    builder.add_filter(|s| {
        let rand_rot_var = std::f32::consts::PI / 8.0;
        let rand_rot = rand::random::<f32>() * rand_rot_var - (rand_rot_var / 2.0);
        let rand_scale_var = 0.3;
        let rand_scale = 1.0 + (rand::random::<f32>() * rand_scale_var - (rand_scale_var / 2.0));
        s.scale_by(rand_scale);
        s.rotate_rad_by(rand_rot);
    });
    let mut symbols = builder.build(msg);
    let mut current_attempt = 0;
    let avoid_scale = symbol_scale / 2.0;
    let avoid_tree = canvas.get_avoid_tree(avoid_scale);
    while current_attempt <= max_attempts {
        let msg_bounds = random_bounding_box(canvas, width_bounds, height_bounds);
        current_attempt += 1;
        let textbox = TextBox::new(TextBoxOptions {
            jagged_edge: 0.1,
            layout: TextBoxLineMode::Flat,
            bounds: msg_bounds,
            symbol_scale,
            avoid_tree: Some(&avoid_tree),
        });
        let (mut placed, unplaced) = textbox.fill_bounds_with(symbols);
        if unplaced.len() > 0 {
            for symbol in &mut placed {
                symbol.scale_by( 1.0 / symbol.scale());
            }
            placed.extend(unplaced);
            symbols = placed;
            continue; // try again
        }
        let mut canvas = Canvas::new();
        for symbol in placed {
            canvas.add_strokes(symbol, StrokeStyle {
                color: color.to_string(),
                width,
            });
        }
        return Some(canvas);
    }
    None
}

pub fn draw_msgs_overtop(canvas: &mut Canvas, symd: &symboldict::SymbolPathDict, msgs: &Vec<String>, width_bounds: (f32, f32), height_bounds: (f32, f32), symbol_scale: f32, width: f32, color: &str) {
    // this is good for a hack, but it would be much better
    // to subdivide the canvas into N boxes and try to fit messages into them
    if msgs.len() == 0 {
        return;
    }
    let max_attempts_per_msg = 100 as usize;
    let mut attempted_placements = 0;
    let mut placed_canvases = Vec::new();
    let mut placed_canvas = canvas.clone();
    while placed_canvases.len() < msgs.len() {
        let next_msg = &msgs[placed_canvases.len()];
        let textcanvas = splat_msg_overtop(&placed_canvas, symd, next_msg, width_bounds, height_bounds, symbol_scale, width, color, max_attempts_per_msg);
        if let Some(textcanvas) = textcanvas {
            placed_canvas.draw_canvas(&textcanvas);
            placed_canvases.push(textcanvas);
            continue;
        } else {
            attempted_placements += 1;
            if placed_canvases.len() == 0 {
                eprintln!("Warning: Failed to splat message '{}' onto canvas even with no other messages placed", next_msg);
                return;
            }
            let todrop = attempted_placements / 3 + 1;
            placed_canvases.truncate(placed_canvases.len() - todrop);
            placed_canvas = canvas.clone();
            for placed in placed_canvases.iter() {

                placed_canvas.draw_canvas(placed);
            }
        }
    }
    for placed in placed_canvases.into_iter() {
        canvas.merge_canvas(placed);
    }
}

#[derive(Clone, Debug)]
pub struct SymbolCircleStyleOptions {
    pub stroke_style: StrokeStyle,
    pub fill_style: Option<StrokeStyle>,
}

#[derive(Clone, Debug)]
pub struct SymbolCircleParams {
    pub rotation: f32,
    pub center: Vec2,
    pub radius: f32,
    pub style_options: Option<SymbolCircleStyleOptions>,
    pub symbol_scale: f32,
}

pub fn symbols_in_circle(symbols: Vec<Symbol>, params: &SymbolCircleParams) -> Canvas {
    let stroke_style = if let Some(style_opts) = &params.style_options {
        style_opts.stroke_style.clone()
    } else {
        StrokeStyle {
            color: "black".to_string(),
            width: 3.0,
        }
    };
    let mut canvas = Canvas::new();
    let SymbolCircleParams {
        rotation,
        center, radius,
        symbol_scale,
        style_options: _ } = params;
    let num_symbols = symbols.len();
    for (i, mut symbol) in symbols.into_iter().enumerate() {
        let angle = (i as f32) * (2.0 * std::f32::consts::PI / (num_symbols as f32)) - (std::f32::consts::PI / 2.0);
        let angle = angle + rotation;
        let x = center.x + angle.cos() * radius;
        let y = center.y + angle.sin() * radius;
        if let Some(style_opts) = &params.style_options {
            if let Some(fill_style) = &style_opts.fill_style {
                let background_circle : StrokePath = CanvasShape::Circle {
                    center: Vec2::new(x, y),
                    radius: symbol_scale * 0.75,
                }.into();
                let background_circle_ss = background_circle.to_substrokes();
                for substroke in background_circle_ss {
                    // a small random offset for the ray stroke fill
                    for _ in 0..3 {
                        let rand_rad_offset = rand::random::<f32>() * std::f32::consts::TAU;
                        let stroke = substroke.ray_stroke_fill(3.0, rand_rad_offset, 3.0);
                        canvas.add_strokes(stroke, fill_style.clone());
                    }
                }
            }
        }
        symbol.scale_by(*symbol_scale);
        symbol.set_position(Vec2::new(x, y));
        symbol.rotate_rad_by(angle + (std::f32::consts::PI / 2.0));
        canvas.add_strokes(symbol, stroke_style.clone());
    }
    canvas
}

pub type MazePosition = (i32, i32);

#[derive(Clone, Debug)]
pub struct BuiltMaze<T: Clone + Fill> {
    cells: Vec<MazeLockCell<T>>,
    pos_neighbors: HashMap<MazePosition, Vec<usize>>,
    pos_cells: HashMap<MazePosition, usize>,
}

impl<T> BuiltMaze<T> where
    T: Clone + Fill,
    T: Default,
{
    pub fn new(seed: u64, maze_width: u32, maze_height: u32) -> BuiltMaze<T> {
        let maze_width = maze_width as i32;
        let maze_height = maze_height as i32;
        // seed the random number generator
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let mut pos_index: HashMap<MazePosition, NodeIndex> = HashMap::new();
        let mut graph = UnGraph::new_undirected();
        for y in 0..maze_height {
            for x in 0..maze_width {
                let current = (x * 2, y * 2);
                pos_index.insert(current, graph.add_node(current));
                if x > 0 {
                    let left = ( (x - 1) * 2, y * 2);
                    let left_index = pos_index.get(&left).unwrap();
                    let current_index = pos_index.get(&current).unwrap();
                    let edge = MazeLockEdge {
                        position: (left.0 + 1, left.1),
                        weight: rng.random_range(1..100),
                    };
                    graph.add_edge(*left_index, *current_index, edge);
                }
                if y > 0 {
                    let up = (x * 2, (y - 1) * 2);
                    let up_index = pos_index.get(&up).unwrap();
                    let current_index = pos_index.get(&current).unwrap();
                    let edge = MazeLockEdge {
                        position: (up.0, up.1 + 1),
                        weight: rng.random_range(1..100),
                    };
                    graph.add_edge(*up_index, *current_index, edge);
                }
            }
        }
        let mst = min_spanning_tree(&graph);

        let mut cells = Vec::new();
        let mut pos_neighbors = HashMap::new();
        let mut pos_cells = HashMap::new();

        for elem in mst {
            let position = match elem {
                petgraph::data::Element::Node{weight} => {
                    weight.clone()
                },
                petgraph::data::Element::Edge{weight, ..} => {
                    weight.position
                }
            };
            let mut cell = MazeLockCell::<T>::default();
            cell.position = position;
            cell.inner.fill(&mut rng);
            cells.push(cell);
        }
        for (index, cell) in cells.iter().enumerate() {
            pos_cells.insert(cell.position, index);
        }

        for cell in cells.iter() {
            let mut neighbors = Vec::new();
            let directions = vec![
                (0, 1),
                (1, 0),
                (0, -1),
                (-1, 0),
            ];
            for (dx, dy) in directions.iter() {
                let neighbor = (cell.position.0 + dx, cell.position.1 + dy);
                if pos_cells.contains_key(&neighbor) {
                    let neighbor_cell = *pos_cells.get(&neighbor).expect("Neighbor cell should exist");
                    neighbors.push(neighbor_cell);
                }
            }
            pos_neighbors.insert(cell.position, neighbors);
        }
        BuiltMaze {
            cells,
            pos_neighbors,
            pos_cells,
        }
    }
    pub fn cell_at(&self, position: &MazePosition) -> Option<&MazeLockCell<T>> {
        if let Some(&index) = self.pos_cells.get(position) {
            self.cells.get(index)
        } else {
            None
        }
    }
    pub fn cell_at_mut(&mut self, position: &MazePosition) -> Option<&mut MazeLockCell<T>> {
        if let Some(&index) = self.pos_cells.get(position) {
            self.cells.get_mut(index)
        } else {
            None
        }
    }
    pub fn neighbors_of(&self, position: &MazePosition) -> Option<Vec<MazePosition>> {
        if let Some(neighbor_indices) = self.pos_neighbors.get(position) {
            let mut neighbors = Vec::new();
            for &index in neighbor_indices.iter() {
                if let Some(cell) = self.cells.get(index) {
                    neighbors.push(cell.position);
                }
            }
            Some(neighbors)
        } else {
            None
        }
    }
    pub fn are_neighbors(&self, position_a: &MazePosition, position_b: &MazePosition) -> bool {
        let neighbors = self.neighbors_of(position_a);
        if neighbors.is_none() {
            return false;
        }
        let neighbors = neighbors.unwrap();
        neighbors.iter().find(|x| x.0 == position_b.0 && x.1 == position_b.1).is_some()

    }
    pub fn iter_inners<'a>(&'a self, traversal: MazeTraversalStrategy) -> MazeTraversalIterator<'a, T> {
        let maze_traversal = MazeTraversal::new((0, 0), traversal);
        maze_traversal.into_iter(self)
    }
    pub fn map_inners(&mut self, traversal: MazeTraversalStrategy, mut fn_update: impl FnMut(&mut T)) {
        let mut traversal = MazeTraversal::new((0, 0), traversal);
        while let Some((pos, _)) = traversal.next(&self) {
            if let Some(cell) = self.cell_at_mut(&pos) {
                fn_update(&mut cell.inner);
            }
        }
    }
    pub fn all_cells(&self) -> &Vec<MazeLockCell<T>> {
        &self.cells
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MazeOptions {
    pub strategy: MazeTraversalStrategy,
    pub input_base: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MazeLockCell<T: Clone> {
    pub position: MazePosition,
    pub inner: T,
}

impl<T: Clone + Default> Default for MazeLockCell<T> {
    fn default() -> Self {
        MazeLockCell {
            position: (0, 0),
            inner: T::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MazeLockEdge {
    pub position: MazePosition,
    pub weight: u32,
}

impl PartialOrd for MazeLockEdge {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.weight.cmp(&other.weight))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MazeTraversalStrategy {
    LeftHanded,
    RightHanded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MazeDirection {
    North,
    East,
    South,
    West,
}

fn turn_left(direction: &MazeDirection) -> MazeDirection {
    match direction {
        MazeDirection::North => MazeDirection::West,
        MazeDirection::West => MazeDirection::South,
        MazeDirection::South => MazeDirection::East,
        MazeDirection::East => MazeDirection::North,
    }
}

fn turn_right(direction: &MazeDirection) -> MazeDirection {
    match direction {
        MazeDirection::North => MazeDirection::East,
        MazeDirection::East => MazeDirection::South,
        MazeDirection::South => MazeDirection::West,
        MazeDirection::West => MazeDirection::North,
    }
}

fn dir_between(from: &MazePosition, to: &MazePosition) -> Option<MazeDirection> {
    let x_delta = to.0 - from.0;
    let y_delta = to.1 - from.1;
    match (x_delta, y_delta) {
        (0, 1) => Some(MazeDirection::South),
        (1, 0) => Some(MazeDirection::East),
        (0, -1) => Some(MazeDirection::North),
        (-1, 0) => Some(MazeDirection::West),
        _ => None,
    }
}

fn sort_neighbors_by_dir_lh(current_dir: &MazeDirection, neighbors: &Vec<MazePosition>, current_pos: &MazePosition) -> Vec<MazePosition> {
    let mut dir_map: HashMap<MazeDirection, MazePosition> = HashMap::new();
    for neighbor in neighbors.iter() {
        if let Some(dir) = dir_between(current_pos, neighbor) {
            dir_map.insert(dir, *neighbor);
        }
    }
    let mut sorted = Vec::new();
    let mut check_dir = turn_left(current_dir);
    for _ in 0..4 {
        if let Some(pos) = dir_map.get(&check_dir) {
            sorted.push(*pos);
        }
        check_dir = turn_right(&check_dir);
    }
    sorted
}

pub struct MazeTraversal {
    strategy: MazeTraversalStrategy,
    traversal_dir: MazeDirection,
    visited: HashSet<MazePosition>,
    fringe: Vec<MazePosition>,
}

impl MazeTraversal {
    pub fn new(start: MazePosition, strategy: MazeTraversalStrategy) -> MazeTraversal {
        MazeTraversal {
            strategy,
            traversal_dir: MazeDirection::North,
            visited: HashSet::new(),
            fringe: vec![start],
        }
    }
    pub fn next<T: Clone + Fill + Default>(&mut self, maze: &BuiltMaze<T>) -> Option<(MazePosition, T)> {
        if self.fringe.is_empty() {
            return None;
        }
        let current_pos = self.fringe.pop()?;
        let cell = maze.cell_at(&current_pos)?;
        self.visited.insert(current_pos);
        let neighbors = maze.neighbors_of(&current_pos).unwrap_or(Vec::new());
        let unvisited_neighbors: Vec<MazePosition> = neighbors.into_iter()
            .filter(|pos| !self.visited.contains(pos))
            .collect();
        let sorted_neighbors = sort_neighbors_by_dir_lh(&self.traversal_dir, &unvisited_neighbors, &current_pos);
        let neighbors = match self.strategy {
            MazeTraversalStrategy::LeftHanded => sorted_neighbors,
            MazeTraversalStrategy::RightHanded => sorted_neighbors.into_iter().rev().collect(),
        };
        let next_neighbor = neighbors.first();
        if let Some(&next_pos) = next_neighbor {
            if let Some(dir) = dir_between(&current_pos, &next_pos) {
                self.traversal_dir = dir;
            }
            for &pos in neighbors.iter().rev() {
                self.fringe.push(pos);
            }
        }
        Some((current_pos, cell.inner.clone()))
    }
    pub fn into_iter<'a, T: Clone + Fill + Default>(self, maze: &'a BuiltMaze<T>) -> MazeTraversalIterator<'a, T> {
        MazeTraversalIterator {
            maze,
            traversal: self,
        }
    }
}

pub struct MazeTraversalIterator<'a, T: Clone + Fill + Default> {
    maze: &'a BuiltMaze<T>,
    traversal: MazeTraversal,
}

impl<'a, T: Clone + Fill + Default> Iterator for MazeTraversalIterator<'a, T> {
    type Item = (MazePosition, T);
    fn next(&mut self) -> Option<Self::Item> {
        self.traversal.next(self.maze)
    }
}
