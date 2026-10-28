pub mod art;
pub mod avoid;
pub mod canvas;
pub mod clip;
pub mod compose;
pub mod distribute;
pub mod fonts;
pub mod free_regions;
pub mod gradient;
pub mod symbol;
pub mod parchment;
pub mod paths;
pub mod pdf;
pub mod progress;
pub mod publish;
pub mod hatch;
pub mod kinds;
pub mod spanning;
pub mod ternary;
pub mod markup;
pub mod typeset;
pub mod rhaientries;
pub mod darkly;

pub use paths::StrokePath;

/// FNV-1a: a string hash that, unlike std's, is guaranteed stable across
/// Rust versions, for seeds made from names.
pub fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |hash, byte| (hash ^ byte as u64).wrapping_mul(0x100000001b3))
}
