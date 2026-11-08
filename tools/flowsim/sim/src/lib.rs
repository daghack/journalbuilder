//! The flow simulator's engine: a 2-D incompressible flow on a MAC grid,
//! the objects that shape it, particles traced through it, and the paths
//! they leave written out as SVG. `wasm` binds it for the page in
//! `tools/flowsim/web` (the `bindings` feature); `field` is what journals
//! read of it.

pub mod colour;
pub mod field;
pub mod geom;
pub mod grid;
pub mod objects;
pub mod render;
pub mod scene;
pub mod sim;
pub mod solver;
pub mod svg;
pub mod tracer;

#[cfg(all(target_arch = "wasm32", feature = "bindings"))]
mod wasm;

pub use geom::{V2, v2};
pub use scene::Scene;
pub use sim::Sim;
