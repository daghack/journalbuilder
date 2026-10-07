//! Procedural path art: shapes generated from a few parameters and a seed,
//! returned as paths to stroke like any other. Scripts reach these through
//! the rhai `art` module (see `rhaientries/art.rs`).

mod snake;

pub use snake::snake_path;
