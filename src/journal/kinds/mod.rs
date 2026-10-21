//! Value kinds: lock-value types whose values are plain data (maps,
//! arrays, numbers and strings) tagged `kind: "<Name>"`, so they cross
//! between lock files, journals and the web editor as themselves.
//!
//! A kind is one file here, registered in `KINDS`, and that's all every
//! part of the system needs to know about it:
//!
//! - lock files' outputs and inputs of its type are checked against its
//!   schema and `validate` (`locks/library.rs`, `from_script`);
//! - its kernels, the pure functions that build and lay out its values, are
//!   a rhai module of the same name in lock files and journals alike
//!   (`stars::layout(code)` works the same in both);
//! - its `sketch`, if it has one, is how it's drawn by default: as a parts
//!   map in journals (`value.sketch()`, `art::sketch(data)`), and as the web
//!   editor's preview.
//!
//! A kind's value holds only what its drawing shows, plus geometry (and its
//! schema rejects anything else), so a lock that unlocks from it unlocks
//! from what a reader sees. Kernels are deterministic: a seed is an option,
//! never the clock.

pub mod schema;
pub mod huffman_code;
pub mod maze;
mod star_chart;

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2;
use rhai::{Array, Dynamic, Engine, Map, Module};

use crate::journal::paths::StrokePath;
pub use schema::Schema;

pub trait ValueKind: Sync {
    /// Its type name, and the tag its values carry: "StarChart".
    fn name(&self) -> &'static str;
    /// The rhai module its kernels are in, if it has any: "stars".
    fn module(&self) -> Option<&'static str> {
        None
    }
    /// The exact shape of its values.
    fn schema(&self) -> Schema;
    /// Checks beyond its shape, on a value that has it: indices in range, a
    /// tree that's one tree.
    fn validate(&self, _value: &Map) -> Result<(), String> {
        Ok(())
    }
    /// Adds its kernels to its module.
    fn kernels(&self, _module: &mut Module) {}
    /// Registers functions it has outside a module: methods on its values,
    /// such as `maze.cells()`, and constructors older than modules.
    fn globals(&self, _engine: &mut Engine) {}
    /// How a value is drawn by default.
    fn sketch(&self, _value: &Map) -> Option<Sketch> {
        None
    }
    /// A rhai expression that makes one, for the lock editor to suggest
    /// as a test value; it runs the same in lock files and journals.
    fn example(&self) -> Option<&'static str> {
        None
    }
}

/// A value drawn: named parts to stroke in their own styles, and text to
/// write at points.
pub struct Sketch {
    pub parts: Vec<(String, StrokePath)>,
    pub labels: Vec<Label>,
    /// How big its labels are written, in its own units.
    pub label_size: f32,
}

pub struct Label {
    pub at: Vec2,
    pub text: String,
}

impl Sketch {
    /// What its parts and labels cover.
    pub fn bounds(&self) -> Option<Aabb> {
        let mut points: Vec<Vec2> = self.parts.iter().flat_map(|(_, p)| p.points_within()).collect();
        points.extend(self.labels.iter().map(|l| l.at));
        let first = *points.first()?;
        Some(points.iter().fold(Aabb::new(first, first), |b, p| Aabb::new(b.mins.min(*p), b.maxs.max(*p))))
    }
}

static KINDS: &[&dyn ValueKind] = &[&huffman_code::HuffmanCode, &maze::Maze, &star_chart::StarChart];

pub fn kinds() -> &'static [&'static dyn ValueKind] {
    KINDS
}

/// The kind of this name.
pub fn get(name: &str) -> Option<&'static dyn ValueKind> {
    KINDS.iter().copied().find(|k| k.name() == name)
}

/// The kind a value is tagged with, if it's a map with a known `kind`.
pub fn kind_of(value: &Dynamic) -> Option<&'static dyn ValueKind> {
    let map = value.read_lock::<Map>()?;
    let tag = map.get("kind")?.clone().into_string().ok()?;
    get(&tag)
}

/// Whether `value` is a good value of `kind`: its shape, then the rest.
pub fn check(kind: &dyn ValueKind, value: &Dynamic) -> Result<(), String> {
    kind.schema().check(value)?;
    kind.validate(&value.read_lock::<Map>().expect("a record is a map"))
}

/// A value's default drawing, if its kind has one.
pub fn sketch(value: &Dynamic) -> Result<Option<Sketch>, String> {
    let kind = kind_of(value).ok_or("the value has no kind with a sketch: it should be a map with a kind, such as a StarChart")?;
    check(kind, value).map_err(|err| format!("a bad {}: {}", kind.name(), err))?;
    Ok(kind.sketch(&value.read_lock::<Map>().expect("checked")))
}

/// Every kind's kernels as rhai modules; `extend` adds what a host has of
/// its own to a module before it's registered (journals take a
/// `symbols(...)` value where lock files take an array and its base).
pub fn register(engine: &mut Engine, extend: impl Fn(&str, &mut Module)) {
    for kind in KINDS {
        kind.globals(engine);
        if let Some(name) = kind.module() {
            let mut module = Module::new();
            kind.kernels(&mut module);
            extend(name, &mut module);
            engine.register_static_module(name, module.into());
        }
    }
}

// ---- reading and writing plain data, for kernels ----

/// A point as data: `[x, y]`.
pub fn point(p: Vec2) -> Dynamic {
    Dynamic::from_array(vec![Dynamic::from_float(p.x as f64), Dynamic::from_float(p.y as f64)])
}

/// `[x, y]` as a point.
pub fn read_point(value: &Dynamic, what: &str) -> Result<Vec2, String> {
    let xy = value.read_lock::<Array>().filter(|a| a.len() == 2).ok_or_else(|| format!("{} should be a point [x, y]", what))?;
    let n = |v: &Dynamic| v.as_float().or_else(|_| v.as_int().map(|i| i as f64)).map_err(|_| format!("{} should be a point [x, y]", what));
    Ok(Vec2::new(n(&xy[0])? as f32, n(&xy[1])? as f32))
}

/// A number, whole or not.
pub fn read_number(value: &Dynamic, what: &str) -> Result<f64, String> {
    value.as_float().or_else(|_| value.as_int().map(|i| i as f64)).map_err(|_| format!("{} should be a number", what))
}

/// A whole number of at least 0.
pub fn read_index(value: &Dynamic, what: &str) -> Result<usize, String> {
    value.as_int().ok().and_then(|i| usize::try_from(i).ok()).ok_or_else(|| format!("{} should be a whole number of at least 0", what))
}

/// An array field of a map.
pub fn field<'a>(map: &'a Map, key: &str) -> &'a Dynamic {
    map.get(key).unwrap_or(&Dynamic::UNIT)
}

/// A kernel's error, at the script line of the call (rhai doesn't add it
/// for functions in modules).
pub fn fail(ctx: &rhai::NativeCallContext, caller: &str, message: impl std::fmt::Display) -> Box<rhai::EvalAltResult> {
    rhai::EvalAltResult::ErrorRuntime(format!("{}: {}", caller, message).into(), ctx.call_position()).into()
}

/// A kernel's options map, read with defaults; unknown keys are errors.
pub struct Options<'a> {
    map: &'a Map,
}

impl<'a> Options<'a> {
    pub fn new(map: &'a Map, known: &[&str]) -> Result<Options<'a>, String> {
        if let Some(unknown) = map.keys().find(|k| !known.contains(&k.as_str())) {
            return Err(format!("unknown option '{}'; the options are {}", unknown, known.join(", ")));
        }
        Ok(Options { map })
    }
    pub fn number(&self, key: &str, default: f64) -> Result<f64, String> {
        self.map.get(key).map_or(Ok(default), |v| read_number(v, &format!("'{}'", key)))
    }
    pub fn optional_number(&self, key: &str) -> Result<Option<f64>, String> {
        self.map.get(key).filter(|v| !v.is_unit()).map(|v| read_number(v, &format!("'{}'", key))).transpose()
    }
    pub fn index(&self, key: &str, default: usize) -> Result<usize, String> {
        self.map.get(key).map_or(Ok(default), |v| read_index(v, &format!("'{}'", key)))
    }
    pub fn optional_index(&self, key: &str) -> Result<Option<usize>, String> {
        self.map.get(key).filter(|v| !v.is_unit()).map(|v| read_index(v, &format!("'{}'", key))).transpose()
    }
    pub fn indices(&self, key: &str) -> Result<Vec<usize>, String> {
        match self.map.get(key) {
            None => Ok(Vec::new()),
            Some(v) => v.read_lock::<Array>().ok_or_else(|| format!("'{}' should be an array of indices", key))?
                .iter().map(|i| read_index(i, &format!("'{}'", key))).collect(),
        }
    }
}

/// An optional whole number: `()` as none.
pub fn read_optional_index(value: &Dynamic, what: &str) -> Result<Option<usize>, String> {
    if value.is_unit() { Ok(None) } else { read_index(value, what).map(Some) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_have_distinct_names_and_modules() {
        let names: std::collections::HashSet<&str> = kinds().iter().map(|k| k.name()).collect();
        assert_eq!(names.len(), kinds().len());
        let modules: Vec<&str> = kinds().iter().filter_map(|k| k.module()).collect();
        assert_eq!(modules.iter().collect::<std::collections::HashSet<_>>().len(), modules.len());
    }
}
