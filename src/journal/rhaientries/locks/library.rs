//! Lock types written in rhai, one file per type: `<locks dir>/<type>.rhai`.
//! Lock files run in an engine of their own and work with plain rhai
//! values; locks/README.md describes what a lock file defines. Helpers
//! they share live in `<locks dir>/lib/`, for `import "lib/<name>"`.

use super::{parse_type, register_maze, Maze};
use crate::lockbuilder::{base_n_symbols, symbols_to_string, InfoVar, InfoVarSet, InfoVarType, InfoVarTypeSet, Lock};
use super::super::sources::Sources;
use rhai::{Array, Dynamic, Engine, EvalAltResult, FuncArgs, Map, Scope, AST};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::rc::Rc;

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

/// The repository's lock files.
pub const DEFAULT_LOCKS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/locks");

/// The functions every lock file defines: name, parameter count, and how
/// errors show it.
const REQUIRED_FNS: &[(&str, usize, &str)] = &[
    ("options", 0, "options()"),
    ("inputs", 1, "inputs(o)"),
    ("outputs", 1, "outputs(o)"),
    ("lock", 2, "lock(o, inputs)"),
    ("unlock", 2, "unlock(o, outputs)"),
];

/// Each lock type's name, with its example count or its first failure.
pub type ExampleResults = Vec<(String, Result<usize, String>)>;

/// The lock types in a directory of lock files, each loaded when first used.
pub struct LockLibrary {
    sources: Sources,
    engine: Engine,
    types: RefCell<HashMap<String, Rc<LockType>>>,
}

/// A loaded lock file.
struct LockType {
    name: String,
    file: String,
    ast: AST,
    /// The options it declares, by name.
    options: BTreeMap<String, OptionSpec>,
}

/// An option as options() declares it, with `integer()`, `string("x")`,
/// `one_of([...])` and so on: its type, and its default if it has one.
#[derive(Clone, Debug)]
pub struct OptionSpec {
    kind: OptionKind,
    default: Option<Dynamic>,
}

#[derive(Clone, Debug)]
enum OptionKind {
    Int,
    Number,
    String,
    Bool,
    Array,
    OneOf(Array),
}

impl OptionKind {
    fn describe(&self) -> String {
        match self {
            OptionKind::Int => "an integer".to_string(),
            OptionKind::Number => "a number".to_string(),
            OptionKind::String => "a string".to_string(),
            OptionKind::Bool => "a bool".to_string(),
            OptionKind::Array => "an array".to_string(),
            OptionKind::OneOf(choices) => {
                let choices: Vec<String> = choices.iter().map(|choice| format!("{:?}", choice)).collect();
                format!("one of {}", choices.join(", "))
            }
        }
    }

    /// `value` as an option of this kind (an integer is fine for a number),
    /// or None if it isn't one.
    fn accept(&self, value: Dynamic) -> Option<Dynamic> {
        let ok = match self {
            OptionKind::Int => value.is_int(),
            OptionKind::Number if value.is_int() => return Some(Dynamic::from_float(value.as_int().ok()? as rhai::FLOAT)),
            OptionKind::Number => value.is_float(),
            OptionKind::String => value.is_string(),
            OptionKind::Bool => value.is_bool(),
            OptionKind::Array => value.is_array(),
            OptionKind::OneOf(choices) => choices.iter().any(|choice| same_value(choice, &value)),
        };
        ok.then_some(value)
    }

    /// What was given instead, for error messages.
    fn describe_value(&self, value: &Dynamic) -> String {
        match self {
            OptionKind::OneOf(_) => format!("{:?}", value),
            _ => describe(value.type_name()),
        }
    }
}

fn option_spec(function: &str, kind: OptionKind, default: Option<Dynamic>) -> RhaiResult<OptionSpec> {
    let default = match default {
        None => None,
        Some(value) => Some(kind.accept(value.clone()).ok_or_else(|| {
            format!("{}(): the default should be {}, got {}", function, kind.describe(), kind.describe_value(&value))
        })?),
    };
    Ok(OptionSpec { kind, default })
}

impl LockType {
    fn defines(&self, function: &str, params: usize) -> bool {
        self.ast.iter_functions().any(|f| f.name == function && f.params.len() == params)
    }
}

/// An error from a lock file's function, with where it happened: in the
/// lock file, or in a helper it imported.
struct ScriptError {
    message: String,
    file: Option<String>,
    line: Option<usize>,
}

impl ScriptError {
    /// "file, line N", the file being `lock_file` unless the error happened
    /// elsewhere.
    fn place(&self, lock_file: &str) -> String {
        let file = self.file.as_deref().unwrap_or(lock_file);
        match self.line {
            Some(line) => format!("{}, line {}", file, line),
            None => file.to_string(),
        }
    }

    /// "file, line N: message".
    fn located(&self, lock_file: &str) -> String {
        format!("{}: {}", self.place(lock_file), self.message)
    }
}

impl LockLibrary {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self::with_sources(Sources::dir(dir))
    }

    /// The lock types in `sources`, with their helpers in `lib/`.
    pub fn with_sources(sources: Sources) -> Self {
        LockLibrary { engine: lock_engine(&sources), sources, types: RefCell::default() }
    }

    /// The lock types in the directory, sorted.
    pub fn available(&self) -> Result<Vec<String>, String> {
        let names = self.sources.names()
            .map_err(|e| format!("couldn't read the lock directory {}: {}", self.sources.root().display(), e))?;
        Ok(names.into_iter().filter(|name| valid_type_name(name)).collect())
    }

    /// Whether the directory has a lock file for `type_name`.
    pub fn has(&self, type_name: &str) -> bool {
        valid_type_name(type_name) && self.sources.has(type_name)
    }

    /// A lock of type `type_name`, called `name` in error messages, with
    /// `options` checked against the ones the type declares.
    pub fn build(self: &Rc<Self>, name: &str, type_name: &str, options: Map) -> Result<RhaiLock, String> {
        let lock_type = self.lock_type(type_name)?;
        let mut lock = RhaiLock {
            library: self.clone(),
            lock_type,
            name: name.to_string(),
            options: Map::new(),
            inputs: InfoVarTypeSet::new(),
            outputs: InfoVarTypeSet::new(),
        };
        lock.options = lock.checked_options(options)?;
        if lock.lock_type.defines("check", 1) {
            let _ = lock.call("check", (lock.options.clone(),))?;
        }
        lock.inputs = lock.types("inputs")?;
        lock.outputs = lock.types("outputs")?;
        Ok(lock)
    }

    /// Run every lock file's examples (see `check_examples`).
    pub fn check_all(self: &Rc<Self>) -> Result<ExampleResults, String> {
        Ok(self.available()?.into_iter()
            .map(|name| {
                let result = self.check_examples(&name);
                (name, result)
            })
            .collect())
    }

    /// Run a lock type's examples through lock and then unlock, returning
    /// how many there are. Each must give its inputs back and produce any
    /// `outputs` it pins, or, if it names an `error`, fail with that.
    pub fn check_examples(self: &Rc<Self>, type_name: &str) -> Result<usize, String> {
        let lock_type = self.lock_type(type_name)?;
        if !lock_type.defines("examples", 0) {
            return Ok(0);
        }
        let examples = self.call(&lock_type, "examples", ())
            .map_err(|e| e.located(&lock_type.file))?
            .try_cast::<Array>()
            .ok_or_else(|| format!("{}: examples() must return an array of examples", lock_type.file))?;
        for (n, example) in examples.iter().enumerate() {
            let example = example.clone().try_cast::<Map>()
                .ok_or_else(|| format!("example {}: must be a map, #{{ options, inputs, outputs, error }}", n))?;
            let expected_error = match example.get("error") {
                None => None,
                Some(error) => Some(error.clone().into_string().map_err(|_| format!("example {}: error must be a string", n))?),
            };
            match (self.run_example(type_name, &example), expected_error) {
                (Ok(()), None) => {}
                (Err(e), Some(expected)) if e.contains(&expected) => {}
                (Err(e), None) => return Err(format!("example {}: {}", n, e)),
                (Err(e), Some(expected)) => {
                    return Err(format!("example {}: expected an error containing '{}', got: {}", n, expected, e));
                }
                (Ok(()), Some(expected)) => {
                    return Err(format!("example {}: expected an error containing '{}', but it worked", n, expected));
                }
            }
        }
        Ok(examples.len())
    }

    fn run_example(self: &Rc<Self>, type_name: &str, example: &Map) -> Result<(), String> {
        const KEYS: [&str; 4] = ["options", "inputs", "outputs", "error"];
        if let Some(unknown) = example.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(format!("unknown key '{}'; examples take {}", unknown, KEYS.join(", ")));
        }
        let map = |key: &str| -> Result<Map, String> {
            match example.get(key) {
                None => Ok(Map::new()),
                Some(value) => value.clone().try_cast::<Map>().ok_or_else(|| format!("{} must be a map", key)),
            }
        };
        let lock = self.build(type_name, type_name, map("options")?)?;

        let given = map("inputs")?;
        if let Some(unknown) = given.keys().find(|name| !lock.inputs.contains_key(name.as_str())) {
            return Err(format!("inputs has '{}', which isn't an input of {}", unknown, type_name));
        }
        let mut inputs = InfoVarSet::new();
        for name in sorted(lock.inputs.keys()) {
            let value = given.get(name.as_str()).ok_or_else(|| format!("inputs lacks '{}'", name))?;
            let var = from_script(value.clone(), &lock.inputs[name])
                .map_err(|e| format!("input '{}': {}", name, e))?;
            inputs.insert(name.clone(), var);
        }

        let outputs = lock.lock_info(&inputs).map_err(|e| e.to_string())?;
        for (name, expected) in map("outputs")? {
            let var = outputs.get(name.as_str())
                .ok_or_else(|| format!("outputs has '{}', which isn't an output of {}", name, type_name))?;
            if !same_value(&var.data, &expected) {
                return Err(format!("output '{}' is {}, not {}", name, show(&var.data, &var.var_type), show(&expected, &var.var_type)));
            }
        }

        let recovered = lock.unlock_info(&outputs).map_err(|e| e.to_string())?;
        for name in sorted(inputs.keys()) {
            let (given, back) = (&inputs[name], &recovered[name]);
            if !same_value(&back.data, &given.data) {
                let var_type = &given.var_type;
                return Err(format!("unlocking gave back '{}' as {}, not {}", name, show(&back.data, var_type), show(&given.data, var_type)));
            }
        }
        Ok(())
    }

    fn lock_type(&self, type_name: &str) -> Result<Rc<LockType>, String> {
        if let Some(lock_type) = self.types.borrow().get(type_name) {
            return Ok(lock_type.clone());
        }
        if !self.has(type_name) {
            return Err(format!("unknown lock type '{}'; available: {}", type_name, self.available()?.join(", ")));
        }
        let lock_type = Rc::new(self.load(type_name)?);
        self.types.borrow_mut().insert(type_name.to_string(), lock_type.clone());
        Ok(lock_type)
    }

    fn load(&self, type_name: &str) -> Result<LockType, String> {
        let file = format!("{}.rhai", type_name);
        let ast = self.sources.compile(&self.engine, type_name).map_err(|e| format!("{}: {}", file, e))?;
        let mut lock_type = LockType { name: type_name.to_string(), file, ast, options: BTreeMap::new() };
        let missing: Vec<&str> = REQUIRED_FNS.iter()
            .filter(|(function, params, _)| !lock_type.defines(function, *params))
            .map(|(_, _, shown)| *shown)
            .collect();
        if !missing.is_empty() {
            return Err(format!("{} doesn't define {}", lock_type.file, missing.join(", ")));
        }
        let declared = self.call(&lock_type, "options", ())
            .map_err(|e| e.located(&lock_type.file))?
            .try_cast::<Map>()
            .ok_or_else(|| format!("{}: options() must return a map of option names to types, e.g. #{{ base: integer() }}", lock_type.file))?;
        for (name, spec) in declared {
            let got = describe(spec.type_name());
            let spec = spec.try_cast::<OptionSpec>().ok_or_else(|| format!(
                "{}: options() should declare '{}' with its type, e.g. integer(), string(\"default\") or one_of([...]), got {}",
                lock_type.file, name, got,
            ))?;
            lock_type.options.insert(name.to_string(), spec);
        }
        Ok(lock_type)
    }

    fn call(&self, lock_type: &LockType, function: &str, args: impl FuncArgs) -> Result<Dynamic, ScriptError> {
        self.engine.call_fn::<Dynamic>(&mut Scope::new(), &lock_type.ast, function, args)
            .map_err(|err| self.script_error(&err))
    }

    /// The error itself, not the calls it came through, with the file and
    /// line it happened at.
    fn script_error(&self, err: &EvalAltResult) -> ScriptError {
        let mut inner = err;
        let mut source = None;
        while let EvalAltResult::ErrorInFunctionCall(_, from, next, _) | EvalAltResult::ErrorInModule(from, next, _) = inner {
            if !from.is_empty() {
                source = Some(from.as_str());
            }
            inner = next;
        }
        let position = inner.position();
        let message = match inner {
            EvalAltResult::ErrorRuntime(thrown, _) => thrown.to_string(),
            other => {
                let text = other.to_string();
                text.strip_suffix(&format!(" ({})", position)).unwrap_or(&text).to_string()
            }
        };
        ScriptError { message, file: source.map(|source| self.shown_path(source)), line: position.line() }
    }

    /// A script's path as errors show it: relative to the lock directory,
    /// with its extension. Lock files are known by their full path, and
    /// imported helpers by their import path.
    fn shown_path(&self, source: &str) -> String {
        let path = Path::new(source);
        let relative = path.strip_prefix(self.sources.root()).unwrap_or(path);
        let mut shown = relative.to_string_lossy().into_owned();
        if relative.extension().is_none() {
            shown.push_str(".rhai");
        }
        shown
    }
}

/// A lock whose type is a lock file.
pub struct RhaiLock {
    library: Rc<LockLibrary>,
    lock_type: Rc<LockType>,
    name: String,
    options: Map,
    inputs: InfoVarTypeSet,
    outputs: InfoVarTypeSet,
}

impl RhaiLock {
    fn error(&self, message: impl std::fmt::Display) -> String {
        format!("lock '{}' ({}): {}", self.name, self.lock_type.file, message)
    }

    fn call(&self, function: &str, args: impl FuncArgs) -> Result<Dynamic, String> {
        self.library.call(&self.lock_type, function, args)
            .map_err(|e| format!("lock '{}' ({}): {}", self.name, e.place(&self.lock_type.file), e.message))
    }

    /// The given options, each declared and of its declared type, with
    /// defaults for those not given.
    fn checked_options(&self, given: Map) -> Result<Map, String> {
        let declared = &self.lock_type.options;
        if let Some(unknown) = given.keys().find(|name| !declared.contains_key(name.as_str())) {
            let known = match declared.len() {
                0 => "it takes no options".to_string(),
                _ => {
                    let options: Vec<String> = declared.iter()
                        .map(|(name, spec)| format!("{} ({})", name, spec.kind.describe()))
                        .collect();
                    format!("options are {}", options.join(", "))
                }
            };
            return Err(self.error(format!("unknown option '{}'; {}", unknown, known)));
        }
        let mut options = Map::new();
        for (name, spec) in declared {
            let value = match given.get(name.as_str()) {
                None => spec.default.clone()
                    .ok_or_else(|| self.error(format!("missing option '{}' ({})", name, spec.kind.describe())))?,
                Some(value) => spec.kind.accept(value.clone()).ok_or_else(|| self.error(format!(
                    "option '{}' should be {}, got {}", name, spec.kind.describe(), spec.kind.describe_value(value),
                )))?,
            };
            options.insert(name.into(), value);
        }
        Ok(options)
    }

    /// `inputs(o)` or `outputs(o)`: value names and their types.
    fn types(&self, function: &str) -> Result<InfoVarTypeSet, String> {
        let types = self.call(function, (self.options.clone(),))?.try_cast::<Map>().ok_or_else(|| {
            self.error(format!("{}() must return a map of names to types, e.g. #{{ plain_text: symbol_type(27) }}", function))
        })?;
        types.into_iter()
            .map(|(name, var_type)| {
                if name.is_empty() || name.contains('.') {
                    return Err(self.error(format!("{}(): '{}' can't be a name; names must be non-empty, without '.'", function, name)));
                }
                let var_type = var_type.into_immutable_string().map_err(|got| {
                    self.error(format!("{}(): the type of '{}' should be a string, e.g. symbol_type(27), got {}", function, name, describe(got)))
                })?;
                Ok((name.to_string(), parse_type(&var_type)))
            })
            .collect()
    }

    /// The named values, for a lock file's function.
    fn script_values(&self, values: &InfoVarSet, names: &InfoVarTypeSet) -> Result<Map, String> {
        names.keys()
            .map(|name| {
                let var = values.get(name).ok_or_else(|| self.error(format!("no value for '{}'", name)))?;
                Ok((name.into(), var.data.clone()))
            })
            .collect()
    }

    /// What `function` returned, each value checked against its declared
    /// type. Missing values are left to the caller.
    fn returned_values(&self, function: &str, returned: Dynamic, declared: &InfoVarTypeSet, what: &str) -> Result<InfoVarSet, String> {
        let returned = returned.try_cast::<Map>()
            .ok_or_else(|| self.error(format!("{}() must return a map of its {}s", function, what)))?;
        returned.into_iter()
            .map(|(name, value)| {
                let var_type = declared.get(name.as_str()).ok_or_else(|| {
                    let names: Vec<&str> = sorted(declared.keys()).into_iter().map(String::as_str).collect();
                    self.error(format!("{}() returned '{}', which isn't one of its {}s ({})", function, name, what, names.join(", ")))
                })?;
                let var = from_script(value, var_type)
                    .map_err(|e| self.error(format!("{}() returned a bad '{}': {}", function, name, e)))?;
                Ok((name.to_string(), var))
            })
            .collect()
    }
}

impl Lock for RhaiLock {
    fn description(&self) -> String {
        self.lock_type.name.clone()
    }
    fn inputs(&self) -> InfoVarTypeSet {
        self.inputs.clone()
    }
    fn outputs(&self) -> InfoVarTypeSet {
        self.outputs.clone()
    }
    fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        let args = self.script_values(inputs, &self.inputs)?;
        let returned = self.call("lock", (self.options.clone(), args))?;
        let outputs = self.returned_values("lock", returned, &self.outputs, "output")?;
        if let Some(missing) = sorted(self.outputs.keys()).into_iter().find(|name| !outputs.contains_key(*name)) {
            return Err(self.error(format!("lock() didn't return '{}'", missing)).into());
        }
        Ok(outputs)
    }
    fn unlock_info(&self, outputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        let args = self.script_values(outputs, &self.outputs)?;
        let returned = self.call("unlock", (self.options.clone(), args))?;
        let mut inputs = self.returned_values("unlock", returned, &self.inputs, "input")?;
        for name in sorted(self.inputs.keys()) {
            if inputs.contains_key(name) {
                continue;
            }
            // An input that's also an output, like a key, is passed through,
            // so unlocking needn't return it.
            match outputs.get(name) {
                Some(value) if self.outputs.get(name) == self.inputs.get(name) => {
                    inputs.insert(name.clone(), value.clone());
                }
                _ => return Err(self.error(format!("unlock() didn't return '{}'", name)).into()),
            }
        }
        Ok(inputs)
    }
}

/// The engine lock files run in: rhai's standard library, option types,
/// symbol helpers and mazes. Nothing random and nothing drawn, so a lock is
/// a pure function of its options and values.
fn lock_engine(sources: &Sources) -> Engine {
    let mut engine = Engine::new();
    // `import "lib/permutations" as perm;` loads <dir>/lib/permutations.rhai.
    engine.set_module_resolver(sources.module_resolver());
    register_maze(&mut engine);
    // Option types for options(): integer() is a required integer,
    // integer(3) one that defaults to 3, and so on.
    engine.register_type_with_name::<OptionSpec>("OptionSpec");
    let kinds = [
        // Not int(): rhai's int(x) is the integral part of a float.
        ("integer", OptionKind::Int),
        ("number", OptionKind::Number),
        ("string", OptionKind::String),
        ("bool", OptionKind::Bool),
        ("array", OptionKind::Array),
    ];
    for (function, kind) in kinds {
        let required = kind.clone();
        engine
            .register_fn(function, move || option_spec(function, required.clone(), None))
            .register_fn(function, move |default: Dynamic| option_spec(function, kind.clone(), Some(default)));
    }
    let one_of = |choices: Array, default: Option<Dynamic>| -> RhaiResult<OptionSpec> {
        if choices.is_empty() {
            return Err("one_of(): there must be at least one choice".into());
        }
        option_spec("one_of", OptionKind::OneOf(choices), default)
    };
    engine
        .register_fn("one_of", move |choices: Array| one_of(choices, None))
        .register_fn("one_of", move |choices: Array, default: Dynamic| one_of(choices, Some(default)));
    engine
        // symbol_type(27) is "Base27Symbol".
        .register_fn("symbol_type", |base: Dynamic| -> RhaiResult<String> {
            Ok(format!("Base{}Symbol", check_base("symbol_type", base)?))
        })
        // symbols(27, "HI") is [8, 9]: see crate::lockbuilder::base_n_symbols.
        .register_fn("symbols", |base: Dynamic, text: &str| -> RhaiResult<Array> {
            let base = check_base("symbols", base)?;
            let symbols = base_n_symbols(base, text).map_err(|e| format!("symbols: {}", e))?;
            Ok(symbols.into_iter().map(|s| Dynamic::from_int(s as rhai::INT)).collect())
        })
        // symbol_text(27, [8, 9]) is "HI".
        .register_fn("symbol_text", |base: Dynamic, symbols: Array| -> RhaiResult<String> {
            let base = check_base("symbol_text", base)?;
            let symbols = check_symbols(Dynamic::from_array(symbols), base)
                .map_err(|e| format!("symbol_text: {}", e))?;
            Ok(symbols_to_string(&symbols, base))
        });
    engine
}

fn check_base(caller: &str, base: Dynamic) -> RhaiResult<u32> {
    base.as_int().ok()
        .and_then(|b| u32::try_from(b).ok())
        .filter(|b| (2..=27).contains(b))
        .ok_or_else(|| format!("{}: base must be an integer from 2 to 27, got {:?}", caller, base).into())
}

/// Lock type names are file names, so letters, digits and `_` only.
fn valid_type_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A value a lock file returned, checked against its declared type.
fn from_script(value: Dynamic, var_type: &InfoVarType) -> Result<InfoVar, String> {
    let expected = match var_type {
        InfoVarType::BaseNSymbol(base) => {
            check_symbols(value.clone(), *base)?;
            None
        }
        InfoVarType::Named(name) if name == "Maze" => Some((value.is::<Maze>(), "a Maze")),
        InfoVarType::Named(name) if name == "String" => Some((value.is_string(), "a string")),
        _ => None,
    };
    if let Some((false, expected)) = expected {
        return Err(format!("expected {}, got {}", expected, describe(value.type_name())));
    }
    Ok(InfoVar::new(var_type.clone(), value))
}

/// `value` as symbols in `base`: an array of integers from 0 to base - 1.
fn check_symbols(value: Dynamic, base: u32) -> Result<Vec<u32>, String> {
    let items = value.into_array()
        .map_err(|got| format!("expected an array of Base{} symbols, got {}", base, describe(got)))?;
    items.iter().enumerate()
        .map(|(i, item)| {
            let symbol = item.as_int().map_err(|got| format!("item {} is {}, not a symbol", i, describe(got)))?;
            u32::try_from(symbol).ok().filter(|&s| s < base)
                .ok_or_else(|| format!("symbol {} (item {}) is outside Base{}'s 0-{}", symbol, i, base, base - 1))
        })
        .collect()
}

/// A rhai type name for error messages: "an integer", "a Maze".
fn describe(type_name: &str) -> String {
    match type_name {
        "i64" | "i32" => "an integer".to_string(),
        "f64" | "f32" => "a number".to_string(),
        "string" | "ImmutableString" | "&str" => "a string".to_string(),
        "bool" => "a bool".to_string(),
        "char" => "a character".to_string(),
        "array" => "an array".to_string(),
        "map" => "a map".to_string(),
        "()" => "()".to_string(),
        // A custom type, by its name without the module path.
        other => format!("a {}", other.rsplit("::").next().unwrap_or(other)),
    }
}

/// Whether two plain values are equal: integers, numbers, strings, bools,
/// characters and `()`, and arrays and maps of them.
fn same_value(a: &Dynamic, b: &Dynamic) -> bool {
    if let (Some(a), Some(b)) = (a.read_lock::<Array>(), b.read_lock::<Array>()) {
        return a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| same_value(x, y));
    }
    if let (Some(a), Some(b)) = (a.read_lock::<Map>(), b.read_lock::<Map>()) {
        return a.len() == b.len() && a.iter().all(|(key, x)| b.get(key).is_some_and(|y| same_value(x, y)));
    }
    if let (Ok(a), Ok(b)) = (a.as_int(), b.as_int()) {
        return a == b;
    }
    if let (Ok(a), Ok(b)) = (a.as_float(), b.as_float()) {
        return a == b;
    }
    if let (Ok(a), Ok(b)) = (a.as_bool(), b.as_bool()) {
        return a == b;
    }
    if let (Ok(a), Ok(b)) = (a.as_char(), b.as_char()) {
        return a == b;
    }
    if let (Some(a), Some(b)) = (a.read_lock::<rhai::ImmutableString>(), b.read_lock::<rhai::ImmutableString>()) {
        return *a == *b;
    }
    a.is_unit() && b.is_unit()
}

/// A value for error messages: symbols as their text when they have one.
fn show(value: &Dynamic, var_type: &InfoVarType) -> String {
    if let InfoVarType::BaseNSymbol(base) = var_type
        && *base <= 27
        && let Ok(symbols) = check_symbols(value.clone(), *base)
    {
        return format!("\"{}\"", symbols_to_string(&symbols, *base));
    }
    format!("{:?}", value)
}

fn sorted<'a>(names: impl Iterator<Item = &'a String>) -> Vec<&'a String> {
    let mut names: Vec<&String> = names.collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lock directory holding `files` (type name, source), removed again
    /// when dropped.
    struct TempLocks {
        dir: PathBuf,
    }

    impl TempLocks {
        fn new(test: &str, files: &[(&str, &str)]) -> Self {
            let dir = std::env::temp_dir().join(format!("scribe-locks-{}-{}", std::process::id(), test));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            for (name, source) in files {
                let path = dir.join(format!("{}.rhai", name));
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, source).unwrap();
            }
            TempLocks { dir }
        }
        fn library(&self) -> Rc<LockLibrary> {
            Rc::new(LockLibrary::new(&self.dir))
        }
    }

    impl Drop for TempLocks {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Passes `text` through as `out`, and `key` through as itself.
    const ECHO: &str = r#"
        fn options() { #{ base: integer(), label: string("x"), scale: number(1.0) } }
        fn inputs(o) { #{ text: symbol_type(o.base), key: symbol_type(o.base) } }
        fn outputs(o) { #{ out: symbol_type(o.base), key: symbol_type(o.base) } }
        fn lock(o, i) { #{ out: i.text, key: i.key } }
        fn unlock(o, out) { #{ text: out.out } }
    "#;

    fn options(pairs: &[(&str, Dynamic)]) -> Map {
        pairs.iter().map(|(k, v)| ((*k).into(), v.clone())).collect()
    }

    fn base(n: i64) -> Map {
        options(&[("base", Dynamic::from_int(n))])
    }

    fn syms(base: u32, text: &str) -> InfoVar {
        let symbols: Array = base_n_symbols(base, text).unwrap().into_iter().map(|s| Dynamic::from_int(s as i64)).collect();
        InfoVar::new(InfoVarType::BaseNSymbol(base), symbols.into())
    }

    fn text(var: &InfoVar) -> String {
        match var.var_type {
            InfoVarType::BaseNSymbol(base) => symbols_to_string(&check_symbols(var.data.clone(), base).unwrap(), base),
            _ => panic!("not symbols: {:?}", var),
        }
    }

    fn values(pairs: &[(&str, InfoVar)]) -> InfoVarSet {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    /// A lock file whose lock() is `body`, with the given inputs and outputs.
    fn lock_file(body: &str) -> String {
        format!(r#"
fn options() {{ #{{}} }}
fn inputs(o) {{ #{{ text: symbol_type(27) }} }}
fn outputs(o) {{ #{{ out: symbol_type(27), extra: symbol_type(27) }} }}
fn lock(o, i) {{
    {}
}}
fn unlock(o, out) {{ #{{ text: out.out }} }}
"#, body)
    }

    #[test]
    fn checks_options_and_fills_defaults() {
        let dir = TempLocks::new("options", &[("echo", ECHO)]);
        let library = dir.library();
        let error = |o: Map| library.build("e", "echo", o).err().expect("should fail");
        assert_eq!(
            error(options(&[("bse", Dynamic::from_int(3))])),
            "lock 'e' (echo.rhai): unknown option 'bse'; options are base (an integer), label (a string), scale (a number)",
        );
        assert_eq!(error(Map::new()), "lock 'e' (echo.rhai): missing option 'base' (an integer)");
        let mut bad = base(27);
        bad.insert("label".into(), Dynamic::from_int(3));
        assert_eq!(error(bad), "lock 'e' (echo.rhai): option 'label' should be a string, got an integer");
        assert_eq!(
            error(options(&[("base", "lots".into())])),
            "lock 'e' (echo.rhai): option 'base' should be an integer, got a string",
        );
        let mut out_of_range = base(40);
        out_of_range.insert("label".into(), "y".into());
        assert!(error(out_of_range).contains("symbol_type: base must be an integer from 2 to 27, got 40"));

        let mut whole = base(27);
        whole.insert("scale".into(), Dynamic::from_int(2));
        let lock = library.build("e", "echo", whole).expect("an integer is fine for a number");
        assert_eq!(lock.options["label"].clone().into_string().unwrap(), "x");
        assert_eq!(lock.options["scale"].as_float().unwrap(), 2.0);
        assert_eq!(lock.inputs["text"], InfoVarType::BaseNSymbol(27));
    }

    #[test]
    fn options_declare_their_types() {
        let declaring = |options: &str| format!(
            "fn options() {{ {} }}\nfn inputs(o) {{ #{{}} }}\nfn outputs(o) {{ #{{}} }}\nfn lock(o, i) {{ #{{}} }}\nfn unlock(o, out) {{ #{{}} }}",
            options,
        );
        let dir = TempLocks::new("option_types", &[
            ("kinds", &declaring(r#"#{ n: integer(3), x: number(), flag: bool(false), list: array([1]), way: one_of(["up", "down"], "up") }"#)),
            ("untyped", &declaring("#{ base: () }")),
            ("bad_default", &declaring("#{ n: integer(3.5) }")),
            ("bad_choice", &declaring(r#"#{ way: one_of(["up", "down"], "sideways") }"#)),
            ("no_choices", &declaring("#{ way: one_of([]) }")),
        ]);
        let library = dir.library();
        let build = |type_name: &str, options: Map| library.build("l", type_name, options);

        let lock = build("kinds", options(&[("x", Dynamic::from_int(2))])).unwrap();
        assert_eq!(lock.options["n"].as_int().unwrap(), 3);
        assert_eq!(lock.options["x"].as_float().unwrap(), 2.0);
        assert_eq!(lock.options["way"].clone().into_string().unwrap(), "up");
        let way = |way: &str| build("kinds", options(&[("x", Dynamic::from_float(1.0)), ("way", way.into())]));
        assert!(way("down").is_ok());
        assert_eq!(
            way("left").err().unwrap(),
            "lock 'l' (kinds.rhai): option 'way' should be one of \"up\", \"down\", got \"left\"",
        );
        let error = |type_name: &str| build(type_name, Map::new()).err().expect("should fail");
        assert_eq!(
            error("untyped"),
            "untyped.rhai: options() should declare 'base' with its type, e.g. integer(), string(\"default\") or one_of([...]), got ()",
        );
        let bad_default = error("bad_default");
        assert!(bad_default.contains("integer(): the default should be an integer, got a number"), "{}", bad_default);
        assert!(error("bad_choice").contains("one_of(): the default should be one of \"up\", \"down\", got \"sideways\""));
        assert!(error("no_choices").contains("one_of(): there must be at least one choice"));
    }

    #[test]
    fn passes_values_through_and_fills_pass_throughs_on_unlock() {
        let dir = TempLocks::new("echo", &[("echo", ECHO)]);
        let lock = dir.library().build("e", "echo", base(27)).unwrap();
        let outputs = lock.lock_info(&values(&[("text", syms(27, "HELLO")), ("key", syms(27, "KEY"))])).unwrap();
        assert_eq!(text(&outputs["out"]), "HELLO");
        // unlock() returns only text; key is both an input and an output.
        let inputs = lock.unlock_info(&outputs).unwrap();
        assert_eq!((text(&inputs["text"]), text(&inputs["key"])), ("HELLO".to_string(), "KEY".to_string()));
    }

    #[test]
    fn reports_script_errors_with_file_and_line() {
        let dir = TempLocks::new("errors", &[
            ("thrower", &lock_file("throw \"boom\";")),
            ("indexer", &lock_file("let a = [];\n    a[3]")),
        ]);
        let library = dir.library();
        let run = |type_name: &str| -> String {
            let lock = library.build("l", type_name, Map::new()).unwrap();
            lock.lock_info(&values(&[("text", syms(27, "HI"))])).expect_err("should fail").to_string()
        };
        assert_eq!(run("thrower"), "lock 'l' (thrower.rhai, line 6): boom");
        let indexed = run("indexer");
        assert!(indexed.starts_with("lock 'l' (indexer.rhai, line 7): "), "{}", indexed);
        assert!(!indexed.contains("position"), "the position should only appear once: {}", indexed);
    }

    #[test]
    fn checks_returned_values() {
        let dir = TempLocks::new("returns", &[
            ("out_of_range", &lock_file("#{ out: [30], extra: [] }")),
            ("not_symbols", &lock_file("#{ out: \"HI\", extra: [] }")),
            ("missing", &lock_file("#{ out: [] }")),
            ("unknown", &lock_file("#{ out: [], extra: [], bogus: [] }")),
            ("no_map", &lock_file("[]")),
            ("not_a_maze", "fn options() { #{} }\nfn inputs(o) { #{} }\nfn outputs(o) { #{ maze: \"Maze\" } }\nfn lock(o, i) { #{ maze: 3 } }\nfn unlock(o, out) { #{} }"),
            ("missing_input", "fn options() { #{} }\nfn inputs(o) { #{ text: symbol_type(27) } }\nfn outputs(o) { #{ out: symbol_type(27) } }\nfn lock(o, i) { #{ out: i.text } }\nfn unlock(o, out) { #{} }"),
        ]);
        let library = dir.library();
        let run = |type_name: &str| -> String {
            let lock = library.build("l", type_name, Map::new()).unwrap();
            let result = lock.lock_info(&values(&[("text", syms(27, "HI"))]))
                .and_then(|outputs| lock.unlock_info(&outputs));
            result.expect_err("should fail").to_string()
        };
        assert_eq!(run("out_of_range"), "lock 'l' (out_of_range.rhai): lock() returned a bad 'out': symbol 30 (item 0) is outside Base27's 0-26");
        assert_eq!(run("not_symbols"), "lock 'l' (not_symbols.rhai): lock() returned a bad 'out': expected an array of Base27 symbols, got a string");
        assert_eq!(run("missing"), "lock 'l' (missing.rhai): lock() didn't return 'extra'");
        assert_eq!(run("unknown"), "lock 'l' (unknown.rhai): lock() returned 'bogus', which isn't one of its outputs (extra, out)");
        assert_eq!(run("no_map"), "lock 'l' (no_map.rhai): lock() must return a map of its outputs");
        assert_eq!(run("not_a_maze"), "lock 'l' (not_a_maze.rhai): lock() returned a bad 'maze': expected a Maze, got an integer");
        assert_eq!(run("missing_input"), "lock 'l' (missing_input.rhai): unlock() didn't return 'text'");
    }

    #[test]
    fn rejects_unknown_incomplete_and_broken_files() {
        let dir = TempLocks::new("files", &[
            ("echo", ECHO),
            ("partial", "fn options() { #{} }\nfn inputs(o) { #{} }\nfn outputs(o) { #{} }"),
            ("broken", "fn options() {\n  #{ base: ( }\n}"),
        ]);
        let library = dir.library();
        let error = |type_name: &str| library.build("l", type_name, Map::new()).err().expect("should fail");
        assert_eq!(error("nope"), "unknown lock type 'nope'; available: broken, echo, partial");
        assert_eq!(error("../echo"), "unknown lock type '../echo'; available: broken, echo, partial");
        assert_eq!(error("partial"), "partial.rhai doesn't define lock(o, inputs), unlock(o, outputs)");
        let broken = error("broken");
        assert!(broken.starts_with("broken.rhai: ") && broken.contains("line 2"), "{}", broken);
    }

    #[test]
    fn lock_files_import_shared_helpers() {
        // A lock file whose lock() runs `body` with `import_line` above it.
        let importing = |import_line: &str, body: &str| format!(
            "{}\nfn options() {{ #{{}} }}\nfn inputs(o) {{ #{{ text: symbol_type(27) }} }}\nfn outputs(o) {{ #{{ out: symbol_type(27) }} }}\nfn lock(o, i) {{ {} }}\nfn unlock(o, out) {{ #{{ text: out.out }} }}",
            import_line, body,
        );
        let dir = TempLocks::new("imports", &[
            ("lib/helpers", "fn doubled(text) { inner(text) + inner(text) }\nfn inner(text) { text }\nfn fail() {\n  throw \"from the helper\";\n}"),
            ("lib/broken", "fn oops( {"),
            ("doubler", &importing("import \"lib/helpers\" as h;", "#{ out: h::doubled(i.text) }")),
            ("failing", &importing("import \"lib/helpers\" as h;", "h::fail()")),
            ("broken_import", &importing("import \"lib/broken\" as b;", "#{ out: [] }")),
            ("missing_import", &importing("import \"lib/nope\" as n;", "#{ out: [] }")),
        ]);
        let library = dir.library();
        assert_eq!(library.available().unwrap(), vec!["broken_import", "doubler", "failing", "missing_import"]);

        let lock = library.build("d", "doubler", Map::new()).unwrap();
        let outputs = lock.lock_info(&values(&[("text", syms(27, "HI"))])).unwrap();
        assert_eq!(text(&outputs["out"]), "HIHI");

        let lock = library.build("f", "failing", Map::new()).unwrap();
        let err = lock.lock_info(&values(&[("text", syms(27, "HI"))])).expect_err("should fail").to_string();
        assert_eq!(err, "lock 'f' (lib/helpers.rhai, line 4): from the helper");

        let error = |type_name: &str| library.build("l", type_name, Map::new()).err().expect("should fail");
        let broken = error("broken_import");
        assert!(broken.starts_with("lib/broken.rhai, line 1: ") && broken.contains("Syntax error"), "{}", broken);
        let missing = error("missing_import");
        assert!(missing.starts_with("missing_import.rhai, line 1: ") && missing.contains("lib/nope"), "{}", missing);
    }

    #[test]
    fn examples_check_round_trips_pins_and_errors() {
        let with_examples = |examples: &str| format!("{}\nfn examples() {{ {} }}", ECHO, examples);
        let good = with_examples(r#"[
            #{ options: #{ base: 27 }, inputs: #{ text: symbols(27, "HI"), key: symbols(27, "K") }, outputs: #{ out: symbols(27, "HI") } },
            #{ options: #{}, inputs: #{}, error: "missing option 'base'" },
        ]"#);
        let wrong_pin = with_examples(r#"[#{ options: #{ base: 27 }, inputs: #{ text: symbols(27, "HI"), key: [] }, outputs: #{ out: symbols(27, "HO") } }]"#);
        let wrong_error = with_examples(r#"[#{ options: #{ base: 27 }, inputs: #{ text: [], key: [] }, error: "nope" }]"#);
        let bad_input = with_examples(r#"[#{ options: #{ base: 27 }, inputs: #{ text: [27], key: [] } }]"#);
        let dir = TempLocks::new("examples", &[
            ("good", &good), ("wrong_pin", &wrong_pin), ("wrong_error", &wrong_error), ("bad_input", &bad_input), ("none", ECHO),
        ]);
        let library = dir.library();
        assert_eq!(library.check_examples("good"), Ok(2));
        assert_eq!(library.check_examples("none"), Ok(0));
        assert_eq!(library.check_examples("wrong_pin"), Err("example 0: output 'out' is \"HI\", not \"HO\"".to_string()));
        assert_eq!(library.check_examples("wrong_error"), Err("example 0: expected an error containing 'nope', but it worked".to_string()));
        assert_eq!(library.check_examples("bad_input"), Err("example 0: input 'text': symbol 27 (item 0) is outside Base27's 0-26".to_string()));
        let all = library.check_all().unwrap();
        let names: Vec<&str> = all.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, vec!["bad_input", "good", "none", "wrong_error", "wrong_pin"]);
    }

    #[test]
    fn every_lock_file_passes_its_examples() {
        let library = Rc::new(LockLibrary::new(DEFAULT_LOCKS_DIR));
        let results = library.check_all().expect("the repository's locks/ should be readable");
        assert!(!results.is_empty(), "no lock files in {}", DEFAULT_LOCKS_DIR);
        let failures: Vec<String> = results.iter()
            .filter_map(|(name, result)| match result {
                Ok(0) => Some(format!("{}: has no examples", name)),
                Ok(_) => None,
                Err(e) => Some(format!("{}: {}", name, e)),
            })
            .collect();
        assert!(failures.is_empty(), "lock files failed their examples:\n{}", failures.join("\n"));
    }
}
