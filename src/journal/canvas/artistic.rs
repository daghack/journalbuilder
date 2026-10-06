use parry2d::math::{Vec2};

use crate::journal::canvas::{Canvas, StrokeStyle, textbox::{TextBox, TextBoxOptions, TextBoxLineMode}};
use crate::journal::symboldict;
use crate::journal::symbol::Symbol;
use crate::journal::paths::StrokePath;
use crate::journal::styles;

use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};
use petgraph::graph::{UnGraph, NodeIndex};
use petgraph::algo::min_spanning_tree;
use rand::{Rng, RngCore, SeedableRng, Fill};
use std::default::Default;

pub fn random_bounding_box(canvas: &Canvas, width_bounds: (f32, f32), height_bounds: (f32, f32), rng: &mut dyn RngCore) -> parry2d::bounding_volume::Aabb {
    let minx = canvas.bounds().mins.x;
    let miny = canvas.bounds().mins.y;
    let maxx = canvas.bounds().maxs.x;
    let maxy = canvas.bounds().maxs.y;
    let width_range = width_bounds.1 - width_bounds.0;
    let height_range = height_bounds.1 - height_bounds.0;
    let width = rng.random::<f32>() * width_range + width_bounds.0;
    let height = rng.random::<f32>() * height_range + height_bounds.0;
    let tlx = rng.random::<f32>() * (maxx - minx - width) + minx;
    let tly = rng.random::<f32>() * (maxy - miny - height) + miny;
    parry2d::bounding_volume::Aabb::new(
        Vec2::new(tlx, tly),
        Vec2::new(tlx + width, tly + height),
    )
}

/// Turn and resize each symbol at random: by up to `rotation` radians and
/// `scale` (a fraction of its size) either way.
pub fn jitter_symbols(symbols: &mut [Symbol], rotation: f32, scale: f32, rng: &mut dyn RngCore) {
    for symbol in symbols {
        let rand_rot = rng.random::<f32>() * 2.0 * rotation - rotation;
        let rand_scale = 1.0 + rng.random::<f32>() * 2.0 * scale - scale;
        symbol.scale_by(rand_scale);
        symbol.rotate_rad_by(rand_rot);
    }
}

#[derive(Clone, Debug)]
pub struct SplatOptions {
    pub symbol_scale: f32,
    /// Range of message box widths and heights to try.
    pub width_range: (f32, f32),
    pub height_range: (f32, f32),
    pub jagged_edge: f32,
    /// Boxes to try for each message before backing off.
    pub max_attempts: usize,
}

/// Give up after backing off this many times in one splat.
const MAX_SPLAT_BACKOFFS: usize = 100;

/// Try random boxes over `working` until `symbols` fit in one, clear of
/// everything already drawn.
fn splat_one(working: &Canvas, symbols: &[Symbol], opts: &SplatOptions, rng: &mut dyn RngCore) -> Option<Vec<Symbol>> {
    let avoid_tree = working.get_avoid_tree(opts.symbol_scale / 2.0);
    for _ in 0..opts.max_attempts {
        let textbox = TextBox::new(TextBoxOptions {
            jagged_edge: opts.jagged_edge,
            layout: TextBoxLineMode::Flat,
            bounds: random_bounding_box(working, opts.width_range, opts.height_range, rng),
            symbol_scale: opts.symbol_scale,
            avoid_tree: Some(&avoid_tree),
        });
        // Place copies, so a failed attempt leaves the symbols untouched.
        let (placed, unplaced) = textbox.fill_bounds_with(symbols.to_vec(), rng);
        if unplaced.is_empty() {
            return Some(placed);
        }
    }
    None
}

/// Scatter messages over `canvas` in randomly sized boxes, each clear of the
/// canvas and of the messages before it, returning each message's placed
/// symbols in order. `style` is only used to account for stroke width while
/// placing. When a message won't fit, the most recent messages are taken
/// back (more of them the more often it happens) and placed again.
pub fn splat_msgs(canvas: &Canvas, msgs: &[Vec<Symbol>], opts: &SplatOptions, style: &StrokeStyle, rng: &mut dyn RngCore) -> Result<Vec<Vec<Symbol>>, String> {
    let mut placed: Vec<Vec<Symbol>> = Vec::new();
    let mut working = canvas.clone();
    let mut backoffs = 0;
    while placed.len() < msgs.len() {
        let next = placed.len();
        if let Some(symbols) = splat_one(&working, &msgs[next], opts, rng) {
            working.add_strokes(symbols.clone(), style.clone());
            placed.push(symbols);
            continue;
        }
        if placed.is_empty() {
            return Err(format!("message {} doesn't fit in {} tries even with no other messages placed", next, opts.max_attempts));
        }
        backoffs += 1;
        if backoffs > MAX_SPLAT_BACKOFFS {
            return Err(format!("gave up after taking messages back {} times; message {} still doesn't fit", MAX_SPLAT_BACKOFFS, next));
        }
        placed.truncate(placed.len().saturating_sub(backoffs / 3 + 1));
        working = canvas.clone();
        for symbols in &placed {
            working.add_strokes(symbols.clone(), style.clone());
        }
    }
    Ok(placed)
}

pub fn draw_msgs_overtop(canvas: &mut Canvas, symd: &symboldict::SymbolPathDict, msgs: &Vec<String>, width_bounds: (f32, f32), height_bounds: (f32, f32), symbol_scale: f32, width: f32, color: &str, rng: &mut dyn RngCore) {
    let builder = styles::lowercase_builder(&symd, "loche_upper");
    let msgs: Vec<Vec<Symbol>> = msgs.iter().map(|msg| {
        let mut symbols = builder.build(msg, rng);
        jitter_symbols(&mut symbols, std::f32::consts::PI / 16.0, 0.15, rng);
        symbols
    }).collect();
    let opts = SplatOptions {
        symbol_scale,
        width_range: width_bounds,
        height_range: height_bounds,
        jagged_edge: 0.1,
        max_attempts: 101,
    };
    let style = StrokeStyle { color: color.to_string(), width, brush: None };
    match splat_msgs(canvas, &msgs, &opts, &style, rng) {
        Ok(placed) => for symbols in placed {
            for symbol in symbols {
                canvas.add_strokes(symbol, style.clone());
            }
        },
        Err(err) => eprintln!("Warning: couldn't splat messages: {}", err),
    }
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
    /// A path along the maze's corridors, in grid units, following the
    /// traversal order: a line between consecutive connected cells, and a
    /// jump back to the branch point whenever the traversal backtracks.
    pub fn corridor_path(&self, strategy: MazeTraversalStrategy) -> StrokePath {
        let mut path = StrokePath::new();
        let mut current = (0, 0);
        let mut visited = HashSet::<MazePosition>::new();
        for (step, _) in self.iter_inners(strategy) {
            if self.are_neighbors(&current, &step) {
                path = path.line_to(step.0 as f32, step.1 as f32);
            } else {
                let neighbors = self.neighbors_of(&step).unwrap_or_default();
                match neighbors.into_iter().find(|n| visited.contains(n)) {
                    None => path = path.move_to(step.0 as f32, step.1 as f32),
                    Some(n) => {
                        path = path
                            .move_to(n.0 as f32, n.1 as f32)
                            .line_to(step.0 as f32, step.1 as f32);
                    }
                }
            }
            visited.insert(step);
            current = step;
        }
        path
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
