//! Procedural art for rhai scripts, in the `art` module.
//!
//! ```rhai
//! // A snake seen from above: its head centred at (0, 0) facing up the page
//! // (angles are radians; 0 faces +x and PI() / 2.0 faces down the page),
//! // the tip of its tail at (40, 400), 900 units long along its spine and 36
//! // across at its widest. The last argument is the seed for its winding.
//! let snake = art::snake_path(new_vec(0.0, 0.0), -PI() / 2.0, new_vec(40.0, 400.0), 900.0, 36.0, 7);
//! canvas.add_strokepath(snake, ink);
//! ```
//!
//! `snake_path` gives the snake's outline. Where the body crosses itself,
//! the outline of the strand underneath stops at the edge of the one on top
//! (the head always goes over), so the path is several strokes. The same
//! seed gives the same snake; it doesn't use or change the script's
//! generator (`seed`, `random`).

use rhai::{Engine, EvalAltResult, Module, NativeCallContext};

use super::{Path, Vec2};
use crate::journal::art;

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

/// An error at the script line of the call: rhai doesn't add it for
/// functions called through a module, as it does for global ones.
fn error(ctx: &NativeCallContext, message: String) -> Box<EvalAltResult> {
    EvalAltResult::ErrorRuntime(message.into(), ctx.call_position()).into()
}

pub fn register(engine: &mut Engine) {
    let mut module = Module::new();
    module.set_native_fn("snake_path", |ctx: NativeCallContext, head_center: Vec2, head_angle: f64, tail_end: Vec2, snake_length: f64, max_width: f64, random_seed: i64| -> RhaiResult<Path> {
        art::snake_path(head_center.into(), head_angle as f32, tail_end.into(), snake_length as f32, max_width as f32, random_seed as u64)
            .map(Path::new)
            .map_err(|err| error(&ctx, format!("art::snake_path: {}", err)))
    });
    engine.register_static_module("art", module.into());
}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use crate::journal::symboldict::{SymbolPathDict, DEFAULT_SYMBOLS};
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::new(Rc::new(SymbolPathDict::from_file(DEFAULT_SYMBOLS).expect("Failed to load symbols.json")))
    }

    #[test]
    fn snake_path_draws_a_snake_from_its_seed() {
        let script = r##"
            let draw = |seed| art::snake_path(new_vec(0.0, 0.0), -PI() / 2.0, new_vec(20.0, 300.0), 900.0, 30.0, seed);
            let snake = draw.call(7);
            if snake.to_string() != draw.call(7).to_string() { throw "the same seed should draw the same snake"; }
            if snake.to_string() == draw.call(8).to_string() { throw "another seed should draw another snake"; }
            let c = new_canvas();
            c.add_strokepath(snake, new_strokestyle(1.0, "#f2ead8", new_brush("Ink Pen", 0.3, 0.7, 1.0)));
            // The tail's tip reaches the tail end (the outline's bounds include
            // the stroke's width).
            if c.bounds().max.y < 300.0 { throw "the tail should reach the tail end: " + c.bounds(); }
            // The snake has its own generator: the script's isn't moved on.
            seed(3);
            let a = random();
            seed(3);
            draw.call(9);
            if random() != a { throw "snake_path shouldn't draw from the script's generator"; }
            let p = new_page();
            p.add_canvas(c);
            journal_entry.add_page(p);
        "##;
        engine().build_entry_str(script).expect("script should run");
    }

    #[test]
    fn snake_path_errors_name_the_function_and_line() {
        let err = engine().build_entry_str("let ok = 1;\nart::snake_path(new_vec(0.0, 0.0), 0.0, new_vec(5000.0, 0.0), 900.0, 30.0, 1);")
            .expect_err("a tail end out of reach should fail").to_string();
        assert!(err.contains("art::snake_path: the tail end is") && err.contains("line 2"), "{}", err);
    }
}
