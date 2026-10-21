//! The shape a value kind's values must have, declared in a few lines and
//! checked with errors that name the field: `pages[2].stars[0].point
//! should be a point [x, y], got a string`.

use rhai::{Array, Dynamic, Map};

#[derive(Clone, Debug)]
pub enum Schema {
    /// The kind's tag: exactly this string.
    Kind(&'static str),
    /// A whole number.
    Int,
    /// A number, whole or not.
    Number,
    Text,
    Bool,
    /// `[x, y]`, two numbers.
    Point,
    /// The schema's value, or `()`.
    Optional(Box<Schema>),
    /// An array, every item of the schema.
    List(Box<Schema>),
    /// An array of exactly these, in order.
    Tuple(Vec<Schema>),
    /// A map with exactly these keys.
    Record(Vec<(&'static str, Schema)>),
}

pub fn optional(s: Schema) -> Schema {
    Schema::Optional(Box::new(s))
}

pub fn list(s: Schema) -> Schema {
    Schema::List(Box::new(s))
}

pub fn record(fields: &[(&'static str, Schema)]) -> Schema {
    Schema::Record(fields.to_vec())
}

impl Schema {
    /// Whether `value` has this shape; the error names where it doesn't.
    pub fn check(&self, value: &Dynamic) -> Result<(), String> {
        self.check_at(value, "")
    }

    fn check_at(&self, value: &Dynamic, at: &str) -> Result<(), String> {
        let wrong = || Err(format!("{} should be {}, got {}", if at.is_empty() { "the value" } else { at }, self.describe(), got(value)));
        match self {
            Schema::Kind(name) => match value.read_lock::<rhai::ImmutableString>() {
                Some(s) if s.as_str() == *name => Ok(()),
                Some(s) => Err(format!("{} should be \"{}\", got \"{}\"", at, name, s.as_str())),
                None => wrong(),
            },
            Schema::Int => if value.is_int() { Ok(()) } else { wrong() },
            Schema::Number => if value.is_int() || value.is_float() { Ok(()) } else { wrong() },
            Schema::Text => if value.is_string() { Ok(()) } else { wrong() },
            Schema::Bool => if value.is_bool() { Ok(()) } else { wrong() },
            Schema::Point => match value.read_lock::<Array>() {
                Some(xy) if xy.len() == 2 && xy.iter().all(|c| c.is_int() || c.is_float()) => Ok(()),
                _ => wrong(),
            },
            Schema::Optional(inner) => if value.is_unit() { Ok(()) } else { inner.check_at(value, at) },
            Schema::List(item) => {
                let Some(items) = value.read_lock::<Array>() else { return wrong() };
                items.iter().enumerate().try_for_each(|(i, v)| item.check_at(v, &format!("{}[{}]", at, i)))
            }
            Schema::Tuple(items) => {
                let Some(values) = value.read_lock::<Array>() else { return wrong() };
                if values.len() != items.len() {
                    return wrong();
                }
                items.iter().zip(values.iter()).enumerate().try_for_each(|(i, (s, v))| s.check_at(v, &format!("{}[{}]", at, i)))
            }
            Schema::Record(fields) => {
                let Some(map) = value.read_lock::<Map>() else { return wrong() };
                let key = |k: &str| if at.is_empty() { k.to_string() } else { format!("{}.{}", at, k) };
                // The declared fields first, in order, so a value of
                // another kind says so before anything else.
                for (name, schema) in fields {
                    match map.get(*name) {
                        Some(v) => schema.check_at(v, &key(name))?,
                        None if matches!(schema, Schema::Optional(_)) => {}
                        None => return Err(format!("{} is missing", key(name))),
                    }
                }
                if let Some(unknown) = map.keys().find(|k| !fields.iter().any(|(name, _)| name == &k.as_str())) {
                    let names: Vec<&str> = fields.iter().map(|(name, _)| *name).collect();
                    return Err(format!("{} isn't a field{}; the fields are {}", key(unknown), if at.is_empty() { String::new() } else { format!(" of {}", at) }, names.join(", ")));
                }
                Ok(())
            }
        }
    }

    /// What the schema takes, for errors.
    pub fn describe(&self) -> String {
        match self {
            Schema::Kind(name) => format!("\"{}\"", name),
            Schema::Int => "a whole number".into(),
            Schema::Number => "a number".into(),
            Schema::Text => "a string".into(),
            Schema::Bool => "true or false".into(),
            Schema::Point => "a point [x, y]".into(),
            Schema::Optional(inner) => format!("{} or ()", inner.describe()),
            Schema::List(item) => format!("an array of {}", plural(&item.describe())),
            Schema::Tuple(items) => format!("[{}]", items.iter().map(|s| s.describe()).collect::<Vec<_>>().join(", ")),
            Schema::Record(fields) => format!("a map of {}", fields.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")),
        }
    }
}

/// "a whole number" as "whole numbers", roughly.
fn plural(one: &str) -> String {
    let bare = one.strip_prefix("a ").or_else(|| one.strip_prefix("an ")).unwrap_or(one);
    match bare.split_once(' ') {
        Some((first, rest)) if first == "map" || first == "point" || first == "array" => format!("{}s {}", first, rest),
        _ if !bare.contains(' ') => format!("{}s", bare),
        _ => bare.replacen("number", "numbers", 1).replacen("string", "strings", 1),
    }
}

/// A value's type, for errors: "an integer", "a string", "a map".
fn got(value: &Dynamic) -> String {
    match value.type_name() {
        "i64" | "i32" => "an integer".into(),
        "f64" | "f32" => "a number".into(),
        "string" | "ImmutableString" | "&str" => "a string".into(),
        "bool" => "a bool".into(),
        "array" => "an array".into(),
        "map" => "a map".into(),
        "()" => "()".into(),
        other => format!("a {}", other.rsplit("::").next().unwrap_or(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart() -> Schema {
        record(&[
            ("kind", Schema::Kind("Chart")),
            ("pages", list(record(&[("name", Schema::Text), ("points", list(Schema::Point)), ("note", optional(Schema::Text))]))),
            ("links", list(Schema::Tuple(vec![Schema::Int, Schema::Int]))),
        ])
    }

    fn eval(script: &str) -> Dynamic {
        rhai::Engine::new().eval::<Dynamic>(script).unwrap()
    }

    #[test]
    fn a_good_value_passes() {
        let v = eval(r#"#{ kind: "Chart", pages: [#{ name: "a", points: [[0, 1.5]], note: () }], links: [[0, 1]] }"#);
        chart().check(&v).unwrap();
    }

    #[test]
    fn errors_name_the_field() {
        for (script, want) in [
            (r#"#{ kind: "Chart", pages: [#{ name: "a", points: [[0, 1], "x"] }], links: [] }"#, "pages[0].points[1] should be a point [x, y], got a string"),
            (r#"#{ kind: "Map", pages: [], links: [] }"#, "kind should be \"Chart\", got \"Map\""),
            (r#"#{ kind: "Chart", pages: [], links: [[0]] }"#, "links[0] should be [a whole number, a whole number], got an array"),
            (r#"#{ kind: "Chart", pages: [] }"#, "links is missing"),
            (r#"#{ kind: "Chart", pages: [#{ name: "a", points: [], id: 3 }], links: [] }"#, "pages[0].id isn't a field of pages[0]; the fields are name, points, note"),
            (r#"3"#, "the value should be a map of kind, pages, links, got an integer"),
        ] {
            let err = chart().check(&eval(script)).unwrap_err();
            assert_eq!(err, want);
        }
    }
}
