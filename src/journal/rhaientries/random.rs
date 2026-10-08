//! Seeded random numbers for rhai scripts.
//!
//! Each engine owns one generator, starting from a fixed seed, so a journal
//! renders the same way every run unless it reseeds:
//!
//! ```rhai
//! seed(1043);
//! let jitter = random_range(-0.5, 0.5);   // float in [-0.5, 0.5)
//! let pick = random_range(0, 4);           // integer in 0..4
//! let size = random_range(1.5, 6);        // float, if either end is
//! if random() < 0.25 { ... }               // float in [0, 1)
//! ```
//!
//! Native helpers that make random choices (glyph variants, text layout,
//! hatching) draw from the same generator, so `seed` fixes those too.

use rhai::Engine;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::cell::RefCell;
use std::rc::Rc;

/// The seed every engine starts from.
pub const DEFAULT_SEED: u64 = 0;

use super::RhaiResult;

/// An engine's generator, shared between the script functions and the
/// native helpers.
pub type SharedRng = Rc<RefCell<StdRng>>;

/// Register the random functions, returning the generator they share.
pub fn register(engine: &mut Engine) -> SharedRng {
    let rng = Rc::new(RefCell::new(StdRng::seed_from_u64(DEFAULT_SEED)));

    let r = rng.clone();
    engine.register_fn("seed", move |seed: i64| {
        *r.borrow_mut() = StdRng::seed_from_u64(seed as u64);
    });

    let r = rng.clone();
    engine.register_fn("random", move || -> f64 { r.borrow_mut().random::<f64>() });

    let r = rng.clone();
    engine.register_fn("random_range", move |low: i64, high: i64| -> RhaiResult<i64> {
        if low >= high {
            return Err(format!("random_range: empty range {}..{}; the low end must be below the high end", low, high).into());
        }
        Ok(r.borrow_mut().random_range(low..high))
    });

    let r = rng.clone();
    let float_range = move |low: f64, high: f64| -> RhaiResult<f64> {
        if !(low < high) {
            return Err(format!("random_range: empty range {}..{}; the low end must be below the high end", low, high).into());
        }
        Ok(r.borrow_mut().random_range(low..high))
    };
    // A float if either end is.
    let f = float_range.clone();
    engine.register_fn("random_range", move |low: f64, high: i64| f(low, high as f64));
    let f = float_range.clone();
    engine.register_fn("random_range", move |low: i64, high: f64| f(low as f64, high));
    engine.register_fn("random_range", float_range);

    rng
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::Array;

    fn engine() -> Engine {
        let mut engine = Engine::new();
        register(&mut engine);
        engine
    }

    fn sequence(engine: &Engine, script: &str) -> Vec<f64> {
        engine.eval::<Array>(script).expect("script should run")
            .into_iter().map(|v| v.as_float().or_else(|_| v.as_int().map(|i| i as f64)).unwrap()).collect()
    }

    const DRAW: &str = "[random(), random_range(0, 1000), random_range(-1.0, 1.0)]";

    #[test]
    fn same_seed_gives_the_same_sequence() {
        let a = sequence(&engine(), &format!("seed(7); {}", DRAW));
        let b = sequence(&engine(), &format!("seed(7); {}", DRAW));
        assert_eq!(a, b);
        let c = sequence(&engine(), &format!("seed(8); {}", DRAW));
        assert_ne!(a, c);
    }

    #[test]
    fn unseeded_engines_are_reproducible_too() {
        assert_eq!(sequence(&engine(), DRAW), sequence(&engine(), DRAW));
    }

    #[test]
    fn values_stay_in_range() {
        let script = r#"
            seed(1);
            let out = [];
            for i in 0..500 {
                let f = random(); let n = random_range(-3, 3); let x = random_range(2.0, 2.5);
                let m = random_range(1, 1.5); let k = random_range(0.5, 1);
                if f < 0.0 || f >= 1.0 || n < -3 || n >= 3 || x < 2.0 || x >= 2.5 { throw "out of range"; }
                if m < 1.0 || m >= 1.5 || k < 0.5 || k >= 1.0 || type_of(m) != "f64" { throw "mixed out of range"; }
                out.push(n);
            }
            out
        "#;
        let values = sequence(&engine(), script);
        // All six integers in -3..3 should turn up in 500 draws.
        for n in -3..3 {
            assert!(values.contains(&(n as f64)), "{} never drawn", n);
        }
    }

    #[test]
    fn empty_ranges_fail_at_their_line() {
        for script in ["let ok = 1;\nrandom_range(3, 3);", "let ok = 1;\nrandom_range(1.0, 0.5);"] {
            let err = engine().run(script).expect_err("empty range should fail").to_string();
            assert!(err.contains("empty range") && err.contains("line 2"), "{}", err);
        }
    }
}
