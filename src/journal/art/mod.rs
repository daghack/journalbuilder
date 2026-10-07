//! Procedural path art: shapes generated from a few parameters and a seed,
//! returned as paths to stroke like any other. Scripts reach these through
//! the rhai `art` module (see `rhaientries/art.rs`).

mod bramble;
mod flower;
mod geom;
mod scene;
mod snake;

pub use bramble::{bramble, Bramble};
pub use snake::{snake_path, snake_spine};
