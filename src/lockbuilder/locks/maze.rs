use crate::journal::canvas::artistic::{BuiltMaze, MazeTraversalStrategy};
use rand::Fill;

#[derive(Clone, Debug, Default)]
pub struct SymbolInfo {
    pub symbol: u32,
}

impl Fill for SymbolInfo {
    fn fill<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) {
        self.symbol = rng.random_range(0..27);
    }
}


/// A maze, with its traversal: in a maze lock's `generated_maze`, the
/// order the text was written into its cells.
#[derive(Clone, Debug)]
pub struct LockedMaze {
    pub maze: BuiltMaze<SymbolInfo>,
    pub strategy: MazeTraversalStrategy,
}
