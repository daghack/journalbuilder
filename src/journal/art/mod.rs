//! Procedural path art: shapes generated from a few parameters and a seed,
//! returned as paths to stroke like any other. Scripts reach these through
//! the rhai `art` module (see `rhaientries/art.rs`).

mod bramble;
mod constellation;
mod eye;
mod flower;
mod geom;
mod hydra;
mod leaf;
mod moon;
mod scene;
mod snake;
mod solid;
mod starchart;
mod trail;

pub use constellation::{constellations, Constellation, ConstellationSettings};
pub use bramble::{bramble, bramble_along, Bramble, Thickening};
pub use eye::{eye, Eye, EyeShape};
pub use hydra::snake_along;
pub use moon::{moon, Moon};
pub use snake::{snake_path, snake_spine};
pub use starchart::{huffman_stars, huffman_tree, lay_out_tree, Star, StarChart, StarChartSettings};
