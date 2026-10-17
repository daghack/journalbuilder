//! Points, directions and angles for rhai scripts.
//!
//! ```rhai
//! let a = vec2(10, 20);              // whole numbers or decimals
//! let b = a + vec2(5.0, 0.0) * 2;    // + and - between vecs; * and / by numbers
//! let c = -b / 4;
//! a.x += 1;
//! a.length();                        // how long it is
//! a.distance(b);                     // how far from another
//! a.dot(b);
//! a.angle();                         // radians from the x axis (y down: clockwise)
//! a.normalized();                    // as long as 1, the same way
//! a.rotated(degrees(90));            // turned about the origin
//! a == b;
//!
//! let p = vec3(1, 2, 3);             // vec3 and vec4 add, scale, measure the same way
//! let q = vec4(p, 1);                // a vec3 and a w
//! p.cross(vec3(0, 0, 1));            // vec3s only
//! q.xyz(); p.xy();                   // dropping the last coordinates
//!
//! degrees(180) == radians(PI());     // angles, as radians
//! ```
//!
//! Vecs are values: methods give new vecs, leaving the one they're called
//! on as it was. Angles are plain numbers of radians: `degrees(d)` converts,
//! `radians(r)` says which a number is.

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
    /// `vec2(x, y)`, refusing infinite and NaN coordinates (from a
    /// division by zero, say), which would otherwise surface far away.
    fn checked(x: Dynamic, y: Dynamic) -> RhaiResult<Self> {
        Ok(Vec2 { x: float("vec2", "x", &x)?, y: float("vec2", "y", &y)? })
    }
    fn scaled(&self, k: f64) -> Self {
        Vec2::new(self.x * k, self.y * k)
    }
    fn length(&mut self) -> f64 {
        self.x.hypot(self.y)
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("vec2", Self::checked)
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

/// A vec of more than two coordinates (`vec3`, `vec4`): a value type
/// with the arithmetic and measures `Vec2` has, bar the 2D-only `angle`
/// and `rotated`.
macro_rules! vec_n {
    ($name:ident, $ctor:literal, $($field:ident),+) => {
        #[derive(Debug, Default, Clone, PartialEq)]
        pub struct $name {
            $(pub $field: f64,)+
        }

        impl $name {
            pub fn new($($field: f64),+) -> Self {
                $name { $($field),+ }
            }
            pub fn coordinates(&self) -> Vec<f64> {
                vec![$(self.$field),+]
            }
            fn checked($($field: Dynamic),+) -> RhaiResult<Self> {
                Ok($name { $($field: float($ctor, stringify!($field), &$field)?),+ })
            }
            fn map(&self, f: impl Fn(f64) -> f64) -> Self {
                $name { $($field: f(self.$field)),+ }
            }
            fn zip(&self, other: &Self, f: impl Fn(f64, f64) -> f64) -> Self {
                $name { $($field: f(self.$field, other.$field)),+ }
            }
            fn dot(&self, other: &Self) -> f64 {
                0.0 $(+ self.$field * other.$field)+
            }
            fn length(&self) -> f64 {
                self.dot(self).sqrt()
            }
            fn describe(&self) -> String {
                format!("({})", [$(self.$field.to_string()),+].join(", "))
            }
        }

        impl CustomType for $name {
            fn build(mut builder: TypeBuilder<Self>) {
                builder.with_name(stringify!($name)).with_fn($ctor, Self::checked);
                $(
                    builder.with_get_set(stringify!($field), |v: &mut Self| Dynamic::from(v.$field), |v: &mut Self, n: Dynamic| -> RhaiResult<()> {
                        v.$field = float(concat!($ctor, ".", stringify!($field)), stringify!($field), &n)?;
                        Ok(())
                    });
                )+
                builder
                    .with_fn("+", |a: Self, b: Self| a.zip(&b, |p, q| p + q))
                    .with_fn("-", |a: Self, b: Self| a.zip(&b, |p, q| p - q))
                    .with_fn("-", |a: Self| a.map(|p| -p))
                    .with_fn("*", |a: Self, k: Dynamic| -> RhaiResult<Self> {
                        let k = float("*", "a vec's factor", &k)?;
                        Ok(a.map(|p| p * k))
                    })
                    .with_fn("*", |k: f64, a: Self| a.map(|p| p * k))
                    .with_fn("*", |k: INT, a: Self| a.map(|p| p * k as f64))
                    .with_fn("/", |a: Self, k: Dynamic| -> RhaiResult<Self> {
                        let k = float("/", "a vec's divisor", &k)?;
                        if k == 0.0 {
                            return Err("/: can't divide a vec by zero".into());
                        }
                        Ok(a.map(|p| p / k))
                    })
                    .with_fn("==", |a: Self, b: Self| a == b)
                    .with_fn("!=", |a: Self, b: Self| a != b)
                    .with_fn("length", |a: &mut Self| a.length())
                    .with_fn("distance", |a: &mut Self, b: Self| a.zip(&b, |p, q| p - q).length())
                    .with_fn("dot", |a: &mut Self, b: Self| a.dot(&b))
                    .with_fn("normalized", |a: &mut Self| -> RhaiResult<Self> {
                        let length = a.length();
                        if length == 0.0 {
                            return Err("normalized: a vec of length 0 has no direction".into());
                        }
                        Ok(a.map(|p| p / length))
                    })
                    .with_fn("xy", |a: &mut Self| Vec2::new(a.x, a.y))
                    .with_fn("to_string", |a: &mut Self| a.describe())
                    .with_fn("to_debug", |a: &mut Self| a.describe());
            }
        }
    };
}

vec_n!(Vec3, "vec3", x, y, z);
vec_n!(Vec4, "vec4", x, y, z, w);

impl Vec2 {
    pub fn coordinates(&self) -> Vec<f64> {
        vec![self.x, self.y]
    }
}

/// The coordinates of a vec2, vec3 or vec4 (what `distribute_points`
/// gives and `spanning_tree` takes).
pub(super) fn coordinates(caller: &str, v: &Dynamic) -> RhaiResult<Vec<f64>> {
    if let Some(v) = v.read_lock::<Vec2>() {
        return Ok(v.coordinates());
    }
    if let Some(v) = v.read_lock::<Vec3>() {
        return Ok(v.coordinates());
    }
    if let Some(v) = v.read_lock::<Vec4>() {
        return Ok(v.coordinates());
    }
    Err(format!("{}: points should be vec2s, vec3s or vec4s, got {}", caller, v.type_name()).into())
}

/// The vec of these coordinates: a `Vec2`, `Vec3` or `Vec4`.
pub(super) fn from_coordinates(c: &[f64]) -> Dynamic {
    match *c {
        [x, y] => Dynamic::from(Vec2::new(x, y)),
        [x, y, z] => Dynamic::from(Vec3::new(x, y, z)),
        [x, y, z, w] => Dynamic::from(Vec4::new(x, y, z, w)),
        _ => unreachable!("vecs have 2 to 4 coordinates"),
    }
}

pub(super) fn register(engine: &mut Engine) {
    engine.build_type::<Vec2>();
    engine.build_type::<Vec3>();
    engine.build_type::<Vec4>();
    engine
        .register_fn("vec3", |v: Vec2, z: Dynamic| -> RhaiResult<Vec3> { Ok(Vec3::new(v.x, v.y, float("vec3", "z", &z)?)) })
        .register_fn("vec4", |v: Vec3, w: Dynamic| -> RhaiResult<Vec4> { Ok(Vec4::new(v.x, v.y, v.z, float("vec4", "w", &w)?)) })
        .register_fn("xyz", |v: &mut Vec4| Vec3::new(v.x, v.y, v.z))
        .register_fn("cross", |a: &mut Vec3, b: Vec3| Vec3::new(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x))
        // Angles are radians throughout; these say which unit a number is in.
        .register_fn("radians", |r: Dynamic| float("radians", "the angle", &r))
        .register_fn("degrees", |d: Dynamic| -> RhaiResult<f64> { Ok(float("degrees", "the angle", &d)?.to_radians()) });
}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use crate::journal::fonts::FontBook;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::new(Rc::new(FontBook::from_file(crate::journal::fonts::DEFAULT_FONTS).unwrap()))
    }

    #[test]
    fn vecs_add_scale_and_measure() {
        let ok: bool = engine().engine.eval(r#"
            let a = vec2(3, 4);
            let b = a + vec2(1.0, 1) * 2 - vec2(0, 1);
            let c = -b / 2;
            a.x += 1;
            let turned = vec2(1, 0).rotated(PI() / 2.0);
            a.x == 4.0 && b == vec2(5, 5) && c == vec2(-2.5, -2.5) && 2 * a == a * 2
                && vec2(3, 4).length() == 5.0 && vec2(0, 0).distance(vec2(3, 4)) == 5.0
                && vec2(1, 2).dot(vec2(3, 4)) == 11.0 && vec2(0, 2).normalized() == vec2(0, 1)
                && turned.y > 0.999 && turned.x.abs() < 1e-9 && vec2(0, 1).angle() > 1.57
                && `${vec2(1, 2.5)}` == "(1, 2.5)"
        "#).unwrap();
        assert!(ok);
    }

    #[test]
    fn bad_vecs_are_errors() {
        let e = engine();
        for (script, want) in [
            ("vec2(1, 2) / 0", "can't divide a vec by zero"),
            ("vec2(0, 0).normalized()", "has no direction"),
            ("vec2(1, \"2\")", "vec2: y should be a number, got string"),
            ("vec2(1.0, 1.0 / 0.0)", "vec2: y must be finite"),
        ] {
            let err = e.engine.eval::<rhai::Dynamic>(script).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }
}
