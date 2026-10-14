//! What the web editor's lock editor asks of lock types: what they are and
//! take, their ports for some options, a test run traced through a chain,
//! and a composite's chain read back. Values cross as JSON.

use super::{check_symbols, from_script, same_value, sorted, LockLibrary, OptionKind, OptionSpec, RhaiLock};
use super::super::Maze;
use crate::lockbuilder::{base_n_symbols, symbols_to_string, InfoVar, InfoVarSet, InfoVarType, InfoVarTypeSet, Lock};
use rhai::{Array, Dynamic, Map};
use serde::Serialize;
use serde_json::{json, Value};
use std::rc::Rc;

/// A lock type, as the editor's palette lists it.
#[derive(Serialize)]
pub struct TypeInfo {
    pub name: String,
    pub file: String,
    /// The file's opening comment, without its "name: ".
    pub description: String,
    pub composite: bool,
    pub options: Vec<OptionInfo>,
}

/// An option a lock type declares.
#[derive(Serialize)]
pub struct OptionInfo {
    pub name: String,
    /// "integer", "number", "string", "bool", "array" or "one_of".
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

/// A lock's ports, name -> type, and its options with defaults filled in.
#[derive(Serialize)]
pub struct Ports {
    pub inputs: Vec<(String, String)>,
    pub outputs: Vec<(String, String)>,
    pub options: Value,
}

/// A value as the editor shows it.
#[derive(Serialize)]
pub struct ValueView {
    #[serde(rename = "type")]
    pub type_name: String,
    /// Symbols and strings as text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub json: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maze: Option<MazeView>,
}

/// A maze, for drawing: its size in cells, its cells in walking order, and
/// its corridors as an SVG path in cell units.
#[derive(Serialize)]
pub struct MazeView {
    pub width: i64,
    pub height: i64,
    pub cells: Value,
    pub path: String,
}

/// A test run: the outputs and every value along the way, then the inputs
/// unlocking gave back and whether they're the ones given.
#[derive(Serialize)]
pub struct RunResult {
    pub outputs: Vec<(String, ValueView)>,
    pub trace: Vec<(String, ValueView)>,
    pub recovered: Option<Vec<(String, ValueView)>>,
    #[serde(rename = "roundTrip")]
    pub round_trip: Option<bool>,
    #[serde(rename = "unlockError")]
    pub unlock_error: Option<String>,
}

/// An option's place in `chain(o)`, read back by `read_chain`.
#[derive(Clone)]
pub(super) struct OptionRef(pub String);

impl LockLibrary {
    /// Every lock type, each described or with the error loading it gave.
    pub fn describe_all(&self) -> Result<Vec<(String, Result<TypeInfo, String>)>, String> {
        Ok(self.available()?.into_iter().map(|name| {
            let info = self.describe(&name);
            (name, info)
        }).collect())
    }

    /// What a lock type is and which options it takes.
    pub fn describe(&self, type_name: &str) -> Result<TypeInfo, String> {
        let lock_type = self.lock_type(type_name)?;
        let source = self.sources.read(type_name).map_err(|e| e.to_string())?;
        Ok(TypeInfo {
            name: lock_type.name.clone(),
            file: lock_type.file.clone(),
            description: description(&source, type_name),
            composite: lock_type.composite,
            options: lock_type.options.iter().map(|(name, spec)| option_info(name, spec)).collect(),
        })
    }

    /// A lock's ports with `options` (JSON).
    pub fn ports(self: &Rc<Self>, type_name: &str, options: &Value) -> Result<Ports, String> {
        let lock = self.build(type_name, type_name, map_from_json(options)?)?;
        Ok(Ports {
            inputs: typed(&lock.inputs),
            outputs: typed(&lock.outputs),
            options: to_json(&Dynamic::from_map(lock.options.clone())),
        })
    }

    /// Lock `inputs` (JSON, name -> `{ text }` or `{ json }`) with a lock of
    /// `type_name`, then unlock what it gives. A failure to lock is an error;
    /// a failure to unlock is part of the result.
    pub fn run_traced(self: &Rc<Self>, type_name: &str, options: &Value, inputs: &Value) -> Result<RunResult, String> {
        let lock = self.build(type_name, type_name, map_from_json(options)?)?;
        let given = test_inputs(&lock.inputs, inputs)?;
        let (outputs, trace) = lock.lock_traced(&given)?;
        let (recovered, round_trip, unlock_error) = match lock.unlock_info(&outputs) {
            Ok(recovered) => {
                let same = given.iter().all(|(name, value)| recovered.get(name).is_some_and(|back| same_value(&back.data, &value.data)));
                (Some(views(&recovered)), Some(same), None)
            }
            Err(e) => (None, None, Some(e.to_string())),
        };
        Ok(RunResult { outputs: views(&outputs), trace: views(&trace), recovered, round_trip, unlock_error })
    }

    /// A composite lock file's chain, as the editor draws it: its options,
    /// locks, connections, fixed values, ports, layout and examples. Options used
    /// directly by a lock come back as `{"$option": name}`; if `chain(o)`
    /// computes with them, it's read with the defaults instead and
    /// `promotionsLost` is set.
    pub fn read_chain(&self, type_name: &str) -> Result<Value, String> {
        let lock_type = self.lock_type(type_name)?;
        if !lock_type.composite {
            return Err(format!("{} isn't a composite lock file", lock_type.file));
        }
        let refs: Map = lock_type.options.keys().map(|name| (name.as_str().into(), Dynamic::from(OptionRef(name.to_string())))).collect();
        let (spec, promotions_lost) = match self.call(&lock_type, "chain", (refs,)) {
            Ok(spec) => (spec, false),
            Err(_) => {
                let defaults = lock_type.options.iter()
                    .map(|(name, spec)| spec.default.clone().map(|value| (name.as_str().into(), value))
                        .ok_or_else(|| format!("{}: chain(o) computes with option '{}', which has no default to read it with", lock_type.file, name)))
                    .collect::<Result<Map, String>>()?;
                let spec = self.call(&lock_type, "chain", (defaults,)).map_err(|e| e.located(&lock_type.file))?;
                (spec, true)
            }
        };
        let layout = match lock_type.defines("layout", 0) {
            true => to_json(&self.call(&lock_type, "layout", ()).map_err(|e| e.located(&lock_type.file))?),
            false => Value::Null,
        };
        let examples = match lock_type.defines("examples", 0) {
            true => to_json(&self.call(&lock_type, "examples", ()).map_err(|e| e.located(&lock_type.file))?),
            false => Value::Null,
        };
        let options: Vec<OptionInfo> = lock_type.options.iter().map(|(name, spec)| option_info(name, spec)).collect();
        let source = self.sources.read(type_name).map_err(|e| e.to_string())?;
        Ok(json!({
            "name": lock_type.name,
            "description": description(&source, type_name),
            "options": options,
            "chain": to_json(&spec),
            "layout": layout,
            "examples": examples,
            "promotionsLost": promotions_lost,
        }))
    }
}

impl RhaiLock {
    /// Lock `inputs`, returning the outputs and every value along the way:
    /// inside a composite, each by "lock.port"; otherwise the inputs and
    /// outputs themselves.
    fn lock_traced(&self, inputs: &InfoVarSet) -> Result<(InfoVarSet, InfoVarSet), String> {
        if let Some(chain) = &self.chain {
            return chain.lock(inputs).map_err(|e| self.error(e));
        }
        let outputs = self.lock_info(inputs).map_err(|e| e.to_string())?;
        let mut trace = inputs.clone();
        trace.extend(outputs.iter().map(|(name, value)| (name.clone(), value.clone())));
        Ok((outputs, trace))
    }
}

/// The opening `//` comment of a lock file, joined into a line, without a
/// leading "name: ".
fn description(source: &str, type_name: &str) -> String {
    let lines: Vec<&str> = source.lines()
        .map(str::trim)
        .take_while(|line| line.starts_with("//"))
        .map(|line| line.trim_start_matches('/').trim())
        .collect();
    let text = lines.join(" ");
    text.strip_prefix(&format!("{}:", type_name)).map(str::trim).unwrap_or(&text).to_string()
}

fn option_info(name: &str, spec: &OptionSpec) -> OptionInfo {
    let (kind, choices) = match &spec.kind {
        OptionKind::Int => ("integer", None),
        OptionKind::Number => ("number", None),
        OptionKind::String => ("string", None),
        OptionKind::Bool => ("bool", None),
        OptionKind::Array => ("array", None),
        OptionKind::OneOf(choices) => ("one_of", Some(choices.iter().map(to_json).collect())),
    };
    OptionInfo { name: name.to_string(), kind, choices, default: spec.default.as_ref().map(to_json) }
}

fn typed(types: &InfoVarTypeSet) -> Vec<(String, String)> {
    sorted(types.keys()).into_iter().map(|name| (name.clone(), types[name].to_string())).collect()
}

fn views(values: &InfoVarSet) -> Vec<(String, ValueView)> {
    sorted(values.keys()).into_iter().map(|name| (name.clone(), value_view(&values[name]))).collect()
}

/// `inputs` (name -> `{ text }` or `{ json }`) as values of the lock's
/// input types. Every input needs one.
fn test_inputs(types: &InfoVarTypeSet, inputs: &Value) -> Result<InfoVarSet, String> {
    let given = inputs.as_object().ok_or("the inputs should be an object, name -> { text } or { json }")?;
    if let Some(unknown) = given.keys().find(|name| !types.contains_key(*name)) {
        return Err(format!("'{}' isn't an input; inputs: {}", unknown, typed(types).into_iter().map(|(name, _)| name).collect::<Vec<_>>().join(", ")));
    }
    sorted(types.keys()).into_iter()
        .map(|name| {
            let value = given.get(name).ok_or_else(|| format!("no value for input '{}'", name))?;
            let var = value_from_test(value, &types[name]).map_err(|e| format!("input '{}': {}", name, e))?;
            Ok((name.clone(), var))
        })
        .collect()
}

/// A test value, `{ text }` (symbols or a string) or `{ json }`, as a value
/// of `var_type`.
pub fn value_from_test(value: &Value, var_type: &InfoVarType) -> Result<InfoVar, String> {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        let data = match var_type {
            InfoVarType::BaseNSymbol(base) => base_n_symbols(*base, text)?
                .into_iter().map(|s| Dynamic::from_int(s as rhai::INT)).collect::<Array>().into(),
            _ => Dynamic::from(text.to_string()),
        };
        return from_script(data, var_type);
    }
    match value.get("json") {
        Some(json) => from_script(from_json(json), var_type),
        None => Err("give { text } or { json }".to_string()),
    }
}

/// A value as the editor shows it: its type, its text if it has one, the
/// value as JSON, and a maze drawn out.
pub fn value_view(var: &InfoVar) -> ValueView {
    let text = match &var.var_type {
        InfoVarType::BaseNSymbol(base) if *base <= 27 => check_symbols(var.data.clone(), *base).ok().map(|s| symbols_to_string(&s, *base)),
        _ => var.data.clone().into_string().ok(),
    };
    let maze = var.data.clone().try_cast::<Maze>().map(|mut maze| MazeView {
        width: maze.width(),
        height: maze.height(),
        cells: to_json(&Dynamic::from_array(maze.cells())),
        path: maze.maze.corridor_path(maze.strategy).to_string(),
    });
    ValueView { type_name: var.var_type.to_string(), text, json: to_json(&var.data), maze }
}

/// A rhai value as JSON: maps, arrays and plain values as themselves, an
/// option's place in `chain(o)` as `{"$option": name}`, a maze as
/// `{"$type": "Maze"}` and any other value as `{"$type": its type}`.
pub fn to_json(value: &Dynamic) -> Value {
    if let Some(option) = value.read_lock::<OptionRef>() {
        return json!({ "$option": option.0 });
    }
    if let Some(items) = value.read_lock::<Array>() {
        return Value::Array(items.iter().map(to_json).collect());
    }
    if let Some(map) = value.read_lock::<Map>() {
        return Value::Object(map.iter().map(|(key, value)| (key.to_string(), to_json(value))).collect());
    }
    if let Ok(n) = value.as_int() {
        return json!(n);
    }
    if let Ok(x) = value.as_float() {
        return json!(x);
    }
    if let Ok(b) = value.as_bool() {
        return json!(b);
    }
    if let Ok(c) = value.as_char() {
        return json!(c.to_string());
    }
    if let Some(s) = value.read_lock::<rhai::ImmutableString>() {
        return json!(s.as_str());
    }
    if value.is_unit() {
        return Value::Null;
    }
    let type_name = value.type_name();
    json!({ "$type": type_name.rsplit("::").next().unwrap_or(type_name) })
}

/// JSON as a rhai value: whole numbers as integers.
pub fn from_json(value: &Value) -> Dynamic {
    match value {
        Value::Null => Dynamic::UNIT,
        Value::Bool(b) => Dynamic::from_bool(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Dynamic::from_int(i),
            None => Dynamic::from_float(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => Dynamic::from(s.clone()),
        Value::Array(items) => Dynamic::from_array(items.iter().map(from_json).collect()),
        Value::Object(map) => Dynamic::from_map(map.iter().map(|(key, value)| (key.into(), from_json(value))).collect()),
    }
}

fn map_from_json(value: &Value) -> Result<Map, String> {
    match value {
        Value::Null => Ok(Map::new()),
        Value::Object(_) => Ok(from_json(value).cast::<Map>()),
        _ => Err("the options should be an object".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::DEFAULT_LOCKS_DIR;
    use crate::journal::rhaientries::Sources;

    fn library(extra: &[(&str, &str)]) -> Rc<LockLibrary> {
        let overlay = extra.iter().map(|(name, source)| (name.to_string(), source.to_string()));
        Rc::new(LockLibrary::with_sources(Sources::dir(DEFAULT_LOCKS_DIR).with_overlay(overlay)))
    }

    fn view<'a>(values: &'a [(String, ValueView)], name: &str) -> &'a ValueView {
        &values.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no {}", name)).1
    }

    #[test]
    fn describes_lock_types_and_their_options() {
        let library = library(&[]);
        let shift = serde_json::to_value(library.describe("shift_cipher").unwrap()).unwrap();
        assert_eq!(shift["composite"], json!(false));
        assert!(shift["description"].as_str().unwrap().starts_with("every symbol moved `shift` places"), "{}", shift);
        assert_eq!(shift["options"], json!([{ "name": "base", "kind": "integer" }, { "name": "shift", "kind": "integer" }]));
        let maze = serde_json::to_value(library.describe("maze_lock").unwrap()).unwrap();
        assert_eq!(maze["options"][1], json!({ "name": "strategy", "kind": "one_of", "choices": ["LeftHanded", "RightHanded"], "default": "LeftHanded" }));
        let composite = serde_json::to_value(library.describe("shift_vigenere").unwrap()).unwrap();
        assert_eq!(composite["composite"], json!(true));
        assert_eq!(composite["options"], json!([{ "name": "shift", "kind": "integer", "default": 3 }]));
        let all = library.describe_all().unwrap();
        assert!(all.iter().all(|(_, info)| info.is_ok()) && all.len() >= 9);
    }

    #[test]
    fn ports_follow_the_options() {
        let ports = library(&[]).ports("merge_alpha", &json!({ "base_n": 27, "input_count": 3 })).unwrap();
        let inputs: Vec<&str> = ports.inputs.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(inputs, ["input_0", "input_1", "input_2"]);
        assert_eq!(ports.outputs, [("location_info".to_string(), "LockProcessInfo".to_string()), ("merged_output".to_string(), "Base27Symbol".to_string())]);
        let err = library(&[]).ports("merge_alpha", &json!({ "base_n": 27 })).err().unwrap();
        assert!(err.contains("missing option 'input_count'"), "{}", err);
    }

    #[test]
    fn runs_trace_a_chain_and_unlock_it() {
        let inputs = json!({ "key": { "text": "KEY" }, "plain_text": { "text": "attack at dawn" } });
        let run = library(&[]).run_traced("shift_vigenere", &json!({}), &inputs).unwrap();
        assert_eq!(view(&run.outputs, "cipher_text").text.as_deref(), Some("OAUOKLNIUNLBJV"));
        assert_eq!(view(&run.trace, "shift.cipher_text").text.as_deref(), Some("NHA"));
        assert_eq!(run.round_trip, Some(true));
        assert_eq!(view(run.recovered.as_ref().unwrap(), "plain_text").text.as_deref(), Some("ATTACK AT DAWN"));

        let maze = library(&[]).run_traced("maze_lock", &json!({ "input_base": 27 }), &json!({
            "maze_text": { "text": "HELLO" }, "random_seed": { "json": [4, 2] },
        })).unwrap();
        let maze_view = view(&maze.outputs, "generated_maze");
        assert_eq!(maze_view.type_name, "Maze");
        let drawn = maze_view.maze.as_ref().unwrap();
        assert_eq!((drawn.width, drawn.height), (3, 3));
        assert!(drawn.path.starts_with('M'), "{}", drawn.path);
        assert_eq!(view(&maze.outputs, "text_length").json, json!(5));

        let err = library(&[]).run_traced("shift_cipher", &json!({ "base": 27, "shift": 1 }), &json!({ "plain_text": { "text": "a,b" } })).err().unwrap();
        assert_eq!(err, "input 'plain_text': ',' isn't a base-27 symbol (letters and spaces)");
    }

    #[test]
    fn reads_composite_lock_files_back() {
        let chain = library(&[]).read_chain("shift_vigenere").unwrap();
        assert_eq!(chain["promotionsLost"], json!(false));
        assert_eq!(chain["chain"]["locks"]["shift"]["options"], json!({ "base": 27, "shift": { "$option": "shift" } }));
        assert_eq!(chain["chain"]["connect"], json!([["shift.cipher_text", "vig.key"]]));
        assert_eq!(chain["layout"], json!({ "shift": [120, 60], "vig": [480, 60] }));
        assert_eq!(chain["examples"][0]["inputs"]["key"], json!([11, 5, 25]));

        let computed = r#"
            fn options() { #{ shift: integer(2) } }
            fn chain(o) {
                #{ locks: #{ s: #{ type: "shift_cipher", options: #{ base: 27, shift: o.shift + 1 } } },
                   inputs: #{ p: "s.plain_text" }, outputs: #{ c: "s.cipher_text" } }
            }
        "#;
        let chain = library(&[("computed", computed)]).read_chain("computed").unwrap();
        assert_eq!(chain["promotionsLost"], json!(true));
        assert_eq!(chain["chain"]["locks"]["s"]["options"]["shift"], json!(3));
        assert_eq!(chain["layout"], Value::Null);
        assert!(library(&[]).read_chain("shift_cipher").is_err());
    }
}
