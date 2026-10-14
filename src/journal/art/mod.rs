//! Procedural path art: shapes generated from a few parameters and a seed,
//! returned as paths to stroke like any other. Scripts reach these through
//! the rhai `art` module (see `rhaientries/art.rs`).

mod bramble;
mod eye;
mod flower;
mod geom;
mod hydra;
mod leaf;
mod scene;
mod snake;
mod solid;
mod trail;

pub use bramble::{bramble, bramble_along, Bramble};
pub use eye::{eye, Eye, EyeShape};
pub use hydra::snake_along;
pub use snake::{snake_path, snake_spine};
