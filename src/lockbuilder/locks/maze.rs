use crate::lockbuilder::*;
use crate::journal::canvas::artistic::{BuiltMaze, MazeOptions, MazeTraversal, MazeTraversalStrategy, MazeDirection};
use rand::{Fill};
use std::error::Error;

#[derive(Clone, Debug, Default)]
pub struct SymbolInfo {
    pub symbol: u32,
    pub orientation: u32,
    pub color: u32,
}

impl Fill for SymbolInfo {
    fn fill<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) -> () {
        self.symbol = rng.random_range(0..27);
        self.orientation = rng.random_range(0..4);
        self.color = rng.random_range(0..4);
    }
}


#[derive(Clone, Debug)]
pub struct MazeLock {
    pub input_base: u32,
    pub strategy: MazeTraversalStrategy,
    pub maze: Option<BuiltMaze<SymbolInfo>>,
    pub seed: Option<u64>,
}

impl MazeLock {
    pub fn new(options: MazeOptions) -> Result<MazeLock, Box<dyn Error>> {
        Ok(MazeLock {
            strategy: options.strategy, 
            input_base: options.input_base,
            maze: None,
            seed: None,
        })
    }
    fn get_computed_maze(&mut self, maze_text: &mut Vec<u32>, random_seed: u64) -> &BuiltMaze<SymbolInfo> {
        if self.maze.is_none() || self.seed != Some(random_seed) {
            let built_maze = Self::generate_maze(maze_text, random_seed);
            self.maze = Some(built_maze);
            self.seed = Some(random_seed);
        }
        self.maze.as_ref().expect("Maze should be computed")
    }
    pub fn generate_maze(maze_text: &mut Vec<u32>, random_seed: u64) -> BuiltMaze<SymbolInfo> {
        if maze_text.len() % 2 == 0 {
            maze_text.push(0);
        }
        let edge_count = maze_text.len() / 2;
        let node_count = edge_count + 1;

        let maze_width = (node_count as f64).sqrt().ceil() as u32;
        let to_add = maze_width - (node_count as u32 % maze_width);
        let maze_height = (node_count as u32 + to_add) / maze_width;
        let node_count = maze_width * maze_height;
        let edge_count = node_count - 1;
        let to_add = (node_count + edge_count) - maze_text.len() as u32;
        maze_text.extend(std::iter::repeat(0).take(to_add as usize));
        BuiltMaze::new(random_seed, maze_width, maze_height)
    }
}

impl Lock for MazeLock {
    fn description(&self) -> String {
        format!("maze_lock[{:?}]", self.strategy)
    }
    fn inputs(&self) -> InfoVarTypeSet {
        let mut inputs = InfoVarTypeSet::new();
        inputs.insert(
            "maze_text".to_string(),
            InfoVarType::BaseNSymbol(self.input_base),
        );
        inputs.insert(
            "random_seed".to_string(),
            InfoVarType::BaseNSymbol(10),
        );
        inputs
    }
    fn outputs(&self) -> InfoVarTypeSet {
        let mut outputs = InfoVarTypeSet::new();
        outputs.insert(
            "generated_maze".to_string(),
            InfoVarType::Graph(InfoVarGraphType::MazeLock),
        );
        outputs.insert(
            "random_seed".to_string(),
            InfoVarType::BaseNSymbol(10),
        );
        outputs
    }
    fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        let random_seed = inputs.get("random_seed").ok_or("plain_text not found")?;
        let maze_text = inputs.get("maze_text").ok_or("maze_text not found")?;
        if !maze_text.is(InfoVarType::BaseNSymbol(self.input_base)) {
            return Err(format!("maze_text is not a BaseNSymbol({})", self.input_base).into());
        }
        if !random_seed.is(InfoVarType::BaseNSymbol(10)) {
            return Err("random_seed is not a BaseNSymbol(10)".into());
        }
        let random_seed_data : &Vec<u32> = random_seed.borrow_data()?;
        let mut random_seed_value : u64 = 0;
        for &digit in random_seed_data.iter() {
            random_seed_value = random_seed_value * 10 + (digit as u64);
        }
        let maze_text_data : &Vec<u32> = maze_text.borrow_data()?;
        let mut maze_text_vec = maze_text_data.clone();
        let mut generated_maze = Self::generate_maze(&mut maze_text_vec, random_seed_value);

        generated_maze.map_inners(self.strategy, |inner: &mut SymbolInfo| {
            if maze_text_vec.is_empty() {
                inner.symbol = 0;
            } else {
                inner.symbol = maze_text_vec.remove(0);
            }
            inner.orientation = 0;
            inner.color = 0;
        });

        let mut output_set = InfoVarSet::new();
        output_set.insert("generated_maze".to_string(), InfoVar::new_with(
            InfoVarType::Graph(InfoVarGraphType::MazeLock),
            generated_maze,
        ));
        output_set.insert("random_seed".to_string(), InfoVar::new_with(
            InfoVarType::BaseNSymbol(10),
            random_seed_data.clone(),
        ));
        Ok(output_set)
    }
    fn unlock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        let random_seed = inputs.get("random_seed").ok_or("random_seed not found")?;
        let generated_maze = inputs.get("generated_maze").ok_or("generated_maze not found")?;
        if !generated_maze.is(InfoVarType::Graph(InfoVarGraphType::MazeLock)) {
            return Err("generated_maze is not a Graph(MazeLock)".into());
        }
        if !random_seed.is(InfoVarType::BaseNSymbol(10)) {
            return Err("random_seed is not a BaseNSymbol(10)".into());
        }
        let random_seed_data : &Vec<u32> = random_seed.borrow_data()?;
        let mut random_seed_value : u64 = 0;
        for &digit in random_seed_data.iter() {
            random_seed_value = random_seed_value * 10 + (digit as u64);
        }
        let generated_maze_data : &BuiltMaze<SymbolInfo> = generated_maze.borrow_data()?;
        let mut decoded_maze = Vec::new();
        for (pos, _) in generated_maze_data.iter_inners(self.strategy) {
            let cell = generated_maze_data.cell_at(&pos).expect("Cell should exist");
            decoded_maze.push(cell.inner.symbol);
        }
        let mut output_set = InfoVarSet::new();
        output_set.insert("maze_text".to_string(), InfoVar::new_with(
            InfoVarType::BaseNSymbol(self.input_base),
            decoded_maze,
        ));
        output_set.insert("random_seed".to_string(), InfoVar::new_with(
            InfoVarType::BaseNSymbol(10),
            random_seed_data.clone(),
        ));
        Ok(output_set)
    }
}
