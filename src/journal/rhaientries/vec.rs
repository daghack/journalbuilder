//! Points and directions for rhai scripts.
//!
//! ```rhai
//! let a = new_vec(10, 20);              // whole numbers or decimals
//! let b = a + new_vec(5.0, 0.0) * 2;    // + and - between vecs; * and / by numbers
//! let c = -b / 4;
//! a.x += 1;
//! a.length();                           // how long it is
//! a.distance(b);                        // how far from another
//! a.dot(b);
//! a.angle();                            // radians from the x axis (y down: clockwise)
//! a.normalized();                       // as long as 1, the same way
//! a.rotated(PI() / 2.0);                // turned about the origin
//! a == b;
//! ```
//!
//! Vecs are values: methods give new vecs, leaving the one they're called
//! on as it was.

use parry2d::math::Vec2 as PVec;
use rhai::{CustomType, Dynamic, Engine, TypeBuilder, INT};

use super::options::float;
use super::RhaiResult;

#[derive(Debug, Default, Clone, PartialEq)]
#[derive(CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Vec2 {
    #[rhai_type(skip)]
    pub x: f64,
    #[rhai_type(skip)]
    pub y: f64,
}

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Vec2 { x, y }
    }
    /// `new_vec(x, y)`, refusing infinite and NaN coordinates (from a
    /// division by zero, say), which would otherwise surface far away.
    fn checked(x: Dynamic, y: Dynamic) -> RhaiResult<Self> {
        Ok(Vec2 { x: float("new_vec", "x", &x)?, y: float("new_vec", "y", &y)? })
    }
    fn scaled(&self, k: f64) -> Self {
        Vec2::new(self.x * k, self.y * k)
    }
    fn length(&mut self) -> f64 {
        self.x.hypot(self.y)
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_vec", Self::checked)
            .with_get_set("x", |v: &mut Self| Dynamic::from(v.x), |v: &mut Self, x: Dynamic| -> RhaiResult<()> {
                v.x = float("vec.x", "x", &x)?;
                Ok(())
            })
            .with_get_set("y", |v: &mut Self| Dynamic::from(v.y), |v: &mut Self, y: Dynamic| -> RhaiResult<()> {
                v.y = float("vec.y", "y", &y)?;
                Ok(())
            })
            .with_fn("+", |a: Self, b: Self| Vec2::new(a.x + b.x, a.y + b.y))
            .with_fn("-", |a: Self, b: Self| Vec2::new(a.x - b.x, a.y - b.y))
            .with_fn("-", |a: Self| a.scaled(-1.0))
            .with_fn("*", |a: Self, k: Dynamic| -> RhaiResult<Self> { Ok(a.scaled(float("*", "a vec's factor", &k)?)) })
            .with_fn("*", |k: f64, a: Self| a.scaled(k))
            .with_fn("*", |k: INT, a: Self| a.scaled(k as f64))
            .with_fn("/", |a: Self, k: Dynamic| -> RhaiResult<Self> {
                let k = float("/", "a vec's divisor", &k)?;
                if k == 0.0 {
                    return Err("/: can't divide a vec by zero".into());
                }
                Ok(a.scaled(1.0 / k))
            })
            .with_fn("==", |a: Self, b: Self| a == b)
            .with_fn("!=", |a: Self, b: Self| a != b)
            .with_fn("length", Self::length)
            .with_fn("distance", |a: &mut Self, b: Self| (a.x - b.x).hypot(a.y - b.y))
            .with_fn("dot", |a: &mut Self, b: Self| a.x * b.x + a.y * b.y)
            .with_fn("angle", |a: &mut Self| a.y.atan2(a.x))
            .with_fn("normalized", |a: &mut Self| -> RhaiResult<Self> {
                let length = a.length();
                if length == 0.0 {
                    return Err("normalized: a vec of length 0 has no direction".into());
                }
                Ok(a.scaled(1.0 / length))
            })
            .with_fn("rotated", |a: &mut Self, angle: Dynamic| -> RhaiResult<Self> {
                let (sin, cos) = float("rotated", "angle", &angle)?.sin_cos();
                Ok(Vec2::new(a.x * cos - a.y * sin, a.x * sin + a.y * cos))
            })
            .with_fn("to_string", |a: &mut Self| format!("({}, {})", a.x, a.y))
            .with_fn("to_debug", |a: &mut Self| format!("({}, {})", a.x, a.y))
        ;
    }
}

impl From<Vec2> for PVec {
    fn from(val: Vec2) -> Self {
        PVec::new(val.x as f32, val.y as f32)
    }
}

pub(super) fn register(engine: &mut Engine) {
    engine.build_type::<Vec2>();
}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use crate::journal::symboldict::SymbolPathDict;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::new(Rc::new(SymbolPathDict::from_file(crate::journal::symboldict::DEFAULT_SYMBOLS).unwrap()))
    }

    #[test]
    fn vecs_add_scale_and_measure() {
        let ok: bool = engine().engine.eval(r#"
            let a = new_vec(3, 4);
            let b = a + new_vec(1.0, 1) * 2 - new_vec(0, 1);
            let c = -b / 2;
            a.x += 1;
            let turned = new_vec(1, 0).rotated(PI() / 2.0);
            a.x == 4.0 && b == new_vec(5, 5) && c == new_vec(-2.5, -2.5) && 2 * a == a * 2
                && new_vec(3, 4).length() == 5.0 && new_vec(0, 0).distance(new_vec(3, 4)) == 5.0
                && new_vec(1, 2).dot(new_vec(3, 4)) == 11.0 && new_vec(0, 2).normalized() == new_vec(0, 1)
                && turned.y > 0.999 && turned.x.abs() < 1e-9 && new_vec(0, 1).angle() > 1.57
                && `${new_vec(1, 2.5)}` == "(1, 2.5)"
        "#).unwrap();
        assert!(ok);
    }

    #[test]
    fn bad_vecs_are_errors() {
        let e = engine();
        for (script, want) in [
            ("new_vec(1, 2) / 0", "can't divide a vec by zero"),
            ("new_vec(0, 0).normalized()", "has no direction"),
            ("new_vec(1, \"2\")", "new_vec: y should be a number, got string"),
            ("new_vec(1.0, 1.0 / 0.0)", "new_vec: y must be finite"),
        ] {
            let err = e.engine.eval::<rhai::Dynamic>(script).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }
}
