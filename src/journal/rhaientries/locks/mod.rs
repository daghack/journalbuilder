//! Rhai API for building and running lock chains (see `crate::lockbuilder`).
//! Each lock type is a lock file, `locks/<type>.rhai`, a journal's or the
//! library's (see `library` and webapp/library/locks/README.md).
//!
//! ```rhai
//! let lock = new_lock("page_one_lock")
//!     .add("sub", "substitution_cipher", #{ base_n: 27 })
//!     .add("vig", "vigenere_cipher", #{ key_base: 27, plain_base: 27, cipher_base: 27 })
//!     .connect("sub.cipher_text", "vig.key")
//!     .set_input("sub.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS"));
//! let result = lock.run(#{ "sub.plain_text": symbols(27, "KEY"), "vig.plain_text": symbols(27, "SECRET") });
//! result.text("vig.cipher_text");
//! lock.unlock(result).text("vig.plain_text");   // "SECRET"
//! ```
//!
//! `chain(name)` builds the same from a chain file, `chains/<name>.rhai`,
//! with the illustrations it attaches.

mod library;

pub use library::{parse_sources, LockLibrary, SourceKind, SourceSpec};
pub use library::{from_json, to_json, value_from_test, value_view, OptionInfo, Ports, RunResult, TypeInfo, ValueView};

use rhai::{Array, CustomType, Dynamic, Engine, EvalAltResult, Map, TypeBuilder};
use crate::lockbuilder::{
    base_n_symbols, compatible, symbols_to_string, CompositeLock, InfoVar, InfoVarSet, InfoVarType, Lock,
};
use super::Path;
use super::illustrations::IllustrationLibrary;
use std::collections::HashMap;
use std::rc::Rc;

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

/// Register the lock API, with lock types from `library` and illustration
/// types from `illustrations`.
pub fn register(engine: &mut Engine, library: Rc<LockLibrary>, illustrations: Rc<IllustrationLibrary>) {
    engine
        .build_type::<LockBuilder>()
        .build_type::<LockValue>()
        .build_type::<LockResult>()
        .register_fn("new_lock", {
            let (library, illustrations) = (library.clone(), illustrations.clone());
            move |name: &str| LockBuilder::new(library.clone(), illustrations.clone(), name)
        })
        .register_fn("chain", {
            let (library, illustrations) = (library.clone(), illustrations.clone());
            move |ctx: rhai::NativeCallContext, name: &str| LockBuilder::from_chain(&ctx, library.clone(), illustrations.clone(), name, Map::new())
        })
        .register_fn("chain", move |ctx: rhai::NativeCallContext, name: &str, options: Map| LockBuilder::from_chain(&ctx, library.clone(), illustrations.clone(), name, options))
        .register_fn("symbols", |base: i64, text: &str| symbols(base, text, &Map::new()))
        .register_fn("symbols", |base: i64, text: &str, options: Map| symbols(base, text, &options));
    // A maze's corridors as a path: paths are for drawing, so lock files
    // don't get this one.
    engine.register_fn("path", |maze: &mut Map| -> RhaiResult<Path> {
        Ok(Path::new(crate::journal::kinds::maze::built("path", maze)?.corridors()))
    });
    // Every value kind's kernels, as in lock files; journals' huffman::code
    // also takes a symbols(...) value, its base its own.
    crate::journal::kinds::register(engine, |name, module| {
        if name == "huffman" {
            module.set_native_fn("code", |ctx: rhai::NativeCallContext, symbols: LockValue, output_base: i64| -> RhaiResult<Map> {
                huffman_code(&ctx, &symbols, output_base, &Map::new())
            });
            module.set_native_fn("code", |ctx: rhai::NativeCallContext, symbols: LockValue, output_base: i64, options: Map| -> RhaiResult<Map> {
                huffman_code(&ctx, &symbols, output_base, &options)
            });
            // As lock files write it, the base given too: the same text
            // runs in both.
            module.set_native_fn("code", |ctx: rhai::NativeCallContext, symbols: LockValue, input_base: i64, output_base: i64| -> RhaiResult<Map> {
                same_base(&ctx, &symbols, input_base)?;
                huffman_code(&ctx, &symbols, output_base, &Map::new())
            });
            module.set_native_fn("code", |ctx: rhai::NativeCallContext, symbols: LockValue, input_base: i64, output_base: i64, options: Map| -> RhaiResult<Map> {
                same_base(&ctx, &symbols, input_base)?;
                huffman_code(&ctx, &symbols, output_base, &options)
            });
        }
    });
}

/// `huffman::code(symbols(base, text), output_base[, #{ smoothing }])` in
/// journals.
fn huffman_code(ctx: &rhai::NativeCallContext, symbols: &LockValue, output_base: i64, options: &Map) -> RhaiResult<Map> {
    use crate::journal::kinds::{fail, huffman_code::build, Options};
    const CALLER: &str = "huffman::code";
    let err = |m: String| fail(ctx, CALLER, m);
    let (base, data) = symbols.base_and_symbols().map_err(|e| err(e.to_string()))?;
    let smoothing = Options::new(options, &["smoothing"]).and_then(|o| o.index("smoothing", 1)).map_err(err)? as u32;
    let output_base = u32::try_from(output_base).ok().filter(|b| (2..=27).contains(b))
        .ok_or_else(|| err(format!("output_base must be from 2 to 27, got {}", output_base)))?;
    build(&data, base, output_base, smoothing).map_err(err)
}

/// Fails unless `input_base` is the symbols' own.
fn same_base(ctx: &rhai::NativeCallContext, symbols: &LockValue, input_base: i64) -> RhaiResult<()> {
    let (base, _) = symbols.base_and_symbols().map_err(|e| crate::journal::kinds::fail(ctx, "huffman::code", e))?;
    if base as i64 != input_base {
        return Err(crate::journal::kinds::fail(ctx, "huffman::code", format!("the symbols are base {}, not {}", base, input_base)));
    }
    Ok(())
}

/// A value for a lock's port: a lock value as it is, a string as a String,
/// or a value kind's data (a map with a `kind`), checked and typed as that
/// kind.
fn port_value(value: Dynamic, what: &str, name: &str) -> RhaiResult<InfoVar> {
    if let Some(value) = value.clone().try_cast::<LockValue>() {
        return Ok(value.var);
    }
    if value.is_string() {
        return Ok(InfoVar::new(InfoVarType::Named("String".to_string()), value));
    }
    match crate::journal::kinds::kind_of(&value) {
        Some(kind) => {
            crate::journal::kinds::check(kind, &value).map_err(|err| format!("{}: '{}' is a bad {}: {}", what, name, kind.name(), err))?;
            Ok(InfoVar::new(InfoVarType::Named(kind.name().to_string()), value))
        }
        None => Err(format!("{}: '{}' must be a lock value, e.g. symbols(27, \"TEXT\"), or a value kind's data, e.g. huffman::code(...)", what, name).into()),
    }
}

/// A lock chain under construction: named locks, connections from one
/// lock's output to another's input, and fixed input values. It is rebuilt
/// after every change, so a mistake fails at the line that made it.
#[derive(Clone)]
pub struct LockBuilder {
    library: Rc<LockLibrary>,
    illustration_library: Rc<IllustrationLibrary>,
    /// Illustrations attached, in the order they were.
    illustrations: Vec<Attachment>,
    /// The resources it draws on, each with its value's type.
    sources: Vec<(SourceSpec, InfoVarType)>,
    name: String,
    locks: HashMap<String, Rc<dyn Lock>>,
    connections: Vec<(String, String)>,
    fixed_inputs: Vec<(String, InfoVar)>,
    lock: CompositeLock,
}

impl LockBuilder {
    fn new(library: Rc<LockLibrary>, illustration_library: Rc<IllustrationLibrary>, name: &str) -> RhaiResult<Self> {
        let lock = CompositeLock::new(name.to_string(), HashMap::new(), Vec::new())
            .map_err(|e| format!("new_lock: {}", e))?;
        Ok(Self {
            library,
            illustration_library,
            name: name.to_string(),
            locks: HashMap::new(),
            connections: Vec::new(),
            fixed_inputs: Vec::new(),
            illustrations: Vec::new(),
            sources: Vec::new(),
            lock,
        })
    }

    /// `chain(name[, options])`: chain file `name`'s locks, wiring, fixed
    /// values and illustrations, built as a journal would build them.
    fn from_chain(ctx: &rhai::NativeCallContext, library: Rc<LockLibrary>, illustration_library: Rc<IllustrationLibrary>, name: &str, options: Map) -> RhaiResult<Self> {
        let spec = library.chain_spec(name, options).map_err(|e| format!("chain: {}", e))?;
        let at = |e: Box<EvalAltResult>| -> Box<EvalAltResult> { format!("chain: {}: {}", spec.file, super::options::unwrap_runtime(&e)).into() };
        let mut lock = Self::new(library.clone(), illustration_library, name)?;
        for source in &spec.sources {
            lock.add_source(source.clone()).map_err(at)?;
        }
        for (id, type_name, options) in &spec.locks {
            lock.add(id, type_name, options.clone()).map_err(at)?;
        }
        for (from, to) in &spec.connect {
            lock.connect(from, to).map_err(at)?;
        }
        for (port, value) in &spec.fixed {
            let var_type = lock.lock.inputs().get(port).cloned()
                .ok_or_else(|| at(format!("fixed: '{}' isn't an open input", port).into()))?;
            let var = library::value_from_script(value.clone(), &var_type).map_err(|e| at(format!("fixed: '{}': {}", port, e).into()))?;
            lock.update("fixed", |next| {
                next.fixed_inputs.push((port.clone(), var));
                Ok(())
            }).map_err(at)?;
        }
        for illustration in spec.illustrations {
            lock.attach(ctx, &illustration.id, &illustration.illustration, illustration.options).map_err(at)?;
            for (input, port) in &illustration.inputs {
                lock.connect_illustration(port, &illustration.id, input).map_err(at)?;
            }
        }
        Ok(lock)
    }

    /// Apply `change` to a copy, keeping it only if the result still builds.
    fn update(&mut self, what: &str, change: impl FnOnce(&mut Self) -> Result<(), String>) -> RhaiResult<Self> {
        let mut next = self.clone();
        change(&mut next).map_err(|e| format!("{}: {}", what, e))?;
        next.lock = next.compile().map_err(|e| format!("{}: {}", what, e))?;
        *self = next;
        Ok(self.clone())
    }

    fn compile(&self) -> Result<CompositeLock, String> {
        compose(&self.name, self.locks.clone(), self.connections.clone(), &self.fixed_inputs)
    }

    fn add(&mut self, name: &str, type_name: &str, options: Map) -> RhaiResult<Self> {
        self.update("add", |next| {
            if name.is_empty() || name.contains('.') {
                return Err(format!("lock name '{}' must be non-empty and contain no '.'", name));
            }
            if next.locks.contains_key(name) {
                return Err(format!("a lock named '{}' was already added", name));
            }
            if next.illustrations.iter().any(|r| r.id == name) || next.sources.iter().any(|(s, _)| s.id == name) {
                return Err(format!("there's an illustration or source named '{}' already", name));
            }
            let lock = next.library.build(name, type_name, options)?;
            next.locks.insert(name.to_string(), Rc::new(lock));
            Ok(())
        })
    }

    /// `lock.attach(id, illustration[, options])`: an illustration, to draw values
    /// of the chain wired to its inputs with `connect`.
    fn attach(&mut self, ctx: &rhai::NativeCallContext, id: &str, illustration: &str, options: Map) -> RhaiResult<Self> {
        let loaded = self.illustration_library.load(ctx.engine(), illustration).map_err(|e| format!("attach: {}", e))?;
        let merged = loaded.options(&options).map_err(|e| format!("attach: {}", e))?;
        let types = loaded.inputs(ctx.engine(), &merged).map_err(|e| format!("attach: {}", e))?;
        if id.is_empty() || id.contains('.') {
            return Err(format!("attach: illustration id '{}' must be non-empty and contain no '.'", id).into());
        }
        if self.locks.contains_key(id) || self.illustrations.iter().any(|r| r.id == id) || self.sources.iter().any(|(s, _)| s.id == id) {
            return Err(format!("attach: there's a lock, illustration or source named '{}' already", id).into());
        }
        self.illustrations.push(Attachment { id: id.to_string(), illustration: illustration.to_string(), options, wires: Vec::new(), types });
        Ok(self.clone())
    }

    /// `lock.source(id, kind, name[, options])`: a resource the chain
    /// draws on, its value at port "id.value" (see `library::sources`).
    fn source(&mut self, id: &str, kind: &str, name: &str, options: Map) -> RhaiResult<Self> {
        super::options::check_keys("source", &options, &["options", "base"])?;
        let mut entry = Map::new();
        entry.insert("type".into(), kind.into());
        entry.insert("name".into(), name.into());
        entry.extend(options);
        let mut sources = Map::new();
        sources.insert(id.into(), Dynamic::from_map(entry));
        let mut spec = Map::new();
        spec.insert("sources".into(), Dynamic::from_map(sources));
        let source = library::parse_sources(&spec).map_err(|e| format!("source: {}", e))?.remove(0);
        self.add_source(source)
    }

    fn add_source(&mut self, source: SourceSpec) -> RhaiResult<Self> {
        if self.locks.contains_key(&source.id) || self.illustrations.iter().any(|r| r.id == source.id) || self.sources.iter().any(|(s, _)| s.id == source.id) {
            return Err(format!("source: there's a lock, illustration or source named '{}' already", source.id).into());
        }
        let var_type = self.library.source_type(&source).map_err(|e| format!("source: {}", e))?;
        self.sources.push((source, var_type));
        Ok(self.clone())
    }

    /// The source whose output `port` ("id.value") is, if it's one.
    fn source_at(&self, port: &str) -> Option<&(SourceSpec, InfoVarType)> {
        let (id, name) = port.split_once('.')?;
        self.sources.iter().find(|(source, _)| source.id == id && name == library::SOURCE_PORT)
    }

    /// The type of the chain's port `port` ("lock.name"), input or output,
    /// or a source's value.
    fn port_type(&self, port: &str) -> Option<InfoVarType> {
        if let Some((_, var_type)) = self.source_at(port) {
            return Some(var_type.clone());
        }
        let (part, name) = port.split_once('.')?;
        let lock = self.locks.get(part)?;
        lock.outputs().get(name).cloned().or_else(|| lock.inputs().get(name).cloned())
    }

    /// A wire from a port of the chain to an illustration's input: kept apart
    /// from the chain, whose port stays as it was.
    fn connect_illustration(&mut self, from: &str, id: &str, input: &str) -> RhaiResult<Self> {
        let from_type = self.port_type(from).ok_or_else(|| format!("connect: '{}' isn't a port of the chain", from))?;
        let attachment = self.illustrations.iter_mut().find(|r| r.id == id).ok_or_else(|| format!("connect: no illustration '{}' is attached", id))?;
        let types = &attachment.types;
        let (_, input_type) = types.iter().find(|(name, _)| name == input).ok_or_else(|| {
            format!("connect: illustration '{}' ({}) has no input '{}'; inputs: {}", id, attachment.illustration, input, types.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "))
        })?;
        if !compatible(&from_type, &parse_type(input_type)) {
            return Err(format!("connect: can't connect {} ({}) to {}.{} ({})", from, from_type, id, input, input_type).into());
        }
        if attachment.wires.iter().any(|(name, _)| name == input) {
            return Err(format!("connect: illustration input '{}.{}' is already connected", id, input).into());
        }
        attachment.wires.push((input.to_string(), from.to_string()));
        Ok(self.clone())
    }

    /// Every illustration attached: #{ ID: illustration type }.
    fn illustrations(&mut self) -> Map {
        self.illustrations.iter().map(|r| (r.id.as_str().into(), r.illustration.as_str().into())).collect()
    }

    fn connect(&mut self, from: &str, to: &str) -> RhaiResult<Self> {
        if let Some((id, input)) = to.split_once('.') {
            if self.illustrations.iter().any(|r| r.id == id) {
                return self.connect_illustration(from, id, input);
            }
        }
        // A source feeding a lock: its value is the input's, fixed.
        if let Some((source, var_type)) = self.source_at(from).cloned() {
            let expected = self.lock.inputs().get(to).cloned().ok_or_else(|| format!("connect: '{}' isn't an open input", to))?;
            if !compatible(&var_type, &expected) {
                return Err(format!("connect: can't connect {} ({}) to {} ({})", from, var_type, to, expected).into());
            }
            let value = self.library.source_value(&source).map_err(|e| format!("connect: {}", e))?;
            return self.update("connect", |next| {
                next.fixed_inputs.retain(|(existing, _)| existing != to);
                next.fixed_inputs.push((to.to_string(), value));
                Ok(())
            });
        }
        // CompositeLock::new checks the ports exist, their types match, the
        // input isn't already wired and nothing loops.
        self.update("connect", |next| {
            next.connections.push((from.to_string(), to.to_string()));
            Ok(())
        })
    }

    fn set_input(&mut self, name: &str, value: Dynamic) -> RhaiResult<Self> {
        let var = port_value(value, "set_input", name)?;
        self.update("set_input", |next| {
            next.fixed_inputs.retain(|(existing, _)| existing != name);
            next.fixed_inputs.push((name.to_string(), var));
            Ok(())
        })
    }

    /// Inputs that still need a value when running: name -> type.
    fn inputs(&mut self) -> Map {
        let fixed: Vec<&str> = self.fixed_inputs.iter().map(|(name, _)| name.as_str()).collect();
        type_map(&self.lock.inputs(), |name| !fixed.contains(&name))
    }

    /// Final outputs of the chain: name -> type.
    fn outputs(&mut self) -> Map {
        type_map(&self.lock.outputs(), |_| true)
    }

    fn run(&mut self, inputs: Map) -> RhaiResult<LockResult> {
        let inputs = value_set(inputs, "run")?;
        let open = self.inputs();
        if let Some(unknown) = inputs.keys().find(|name| !open.contains_key(name.as_str())) {
            return Err(format!("run: '{}' isn't an open input; open inputs: {}", unknown, keys(&open)).into());
        }
        let (values, trace) = self.lock.trace_lock_info(&inputs).map_err(|e| format!("run: {}", e))?;
        Ok(self.result(values, trace))
    }

    /// Recover the inputs from a `run` result.
    fn unlock(&mut self, result: LockResult) -> RhaiResult<LockResult> {
        self.unlock_values(result.values)
    }

    /// Recover the inputs from hand-supplied outputs (name -> lock value).
    /// Chains with graph outputs, such as mazes, need a `run` result instead.
    fn unlock_map(&mut self, outputs: Map) -> RhaiResult<LockResult> {
        self.unlock_values(value_set(outputs, "unlock")?)
    }

    fn unlock_values(&mut self, outputs: InfoVarSet) -> RhaiResult<LockResult> {
        let (values, trace) = self.lock.trace_unlock_info(&outputs).map_err(|e| format!("unlock: {}", e))?;
        Ok(self.result(values, trace))
    }

    fn result(&self, values: InfoVarSet, trace: InfoVarSet) -> LockResult {
        LockResult {
            values,
            trace,
            illustrations: Rc::new(self.illustrations.clone()),
            illustration_library: self.illustration_library.clone(),
            sources: Rc::new(self.sources.iter().map(|(source, _)| source.clone()).collect()),
            library: self.library.clone(),
        }
    }
}

/// An illustration attached to a chain, and what's wired to it.
#[derive(Clone)]
struct Attachment {
    id: String,
    illustration: String,
    /// Its options as given, over its defaults when it draws.
    options: Map,
    /// Its input -> the port of the chain wired to it.
    wires: Vec<(String, String)>,
    /// Its inputs' types, as it gave them when attached.
    types: Vec<(String, String)>,
}

impl CustomType for LockBuilder {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Lock")
            .with_fn("add", Self::add)
            .with_fn("add", |lock: &mut Self, name: &str, type_name: &str| lock.add(name, type_name, Map::new()))
            .with_fn("connect", Self::connect)
            .with_fn("attach", |ctx: rhai::NativeCallContext, lock: &mut Self, id: &str, illustration: &str| lock.attach(&ctx, id, illustration, Map::new()))
            .with_fn("attach", |ctx: rhai::NativeCallContext, lock: &mut Self, id: &str, illustration: &str, options: Map| lock.attach(&ctx, id, illustration, options))
            .with_fn("illustrations", Self::illustrations)
            .with_fn("source", |lock: &mut Self, id: &str, kind: &str, name: &str| lock.source(id, kind, name, Map::new()))
            .with_fn("source", Self::source)
            .with_fn("set_input", Self::set_input)
            .with_fn("inputs", Self::inputs)
            .with_fn("outputs", Self::outputs)
            .with_fn("run", Self::run)
            .with_fn("run", |lock: &mut Self| lock.run(Map::new()))
            .with_fn("unlock", Self::unlock)
            .with_fn("unlock", Self::unlock_map);
    }
}

/// A chain of `locks` wired by `connections`, with `fixed` values for some
/// of its open inputs: what `new_lock` builds, and composite lock files too.
pub(crate) fn compose(name: &str, locks: HashMap<String, Rc<dyn Lock>>, connections: Vec<(String, String)>, fixed: &[(String, InfoVar)]) -> Result<CompositeLock, String> {
    let mut lock = CompositeLock::new(name.to_string(), locks, connections).map_err(|e| e.to_string())?;
    let open = lock.inputs();
    for (name, value) in fixed {
        let expected = open.get(name)
            .ok_or_else(|| format!("'{}' isn't an open input; open inputs: {}", name, sorted_names_of(open.keys())))?;
        if !compatible(&value.var_type, expected) {
            return Err(format!("'{}' expects {}, got {}", name, type_name(expected), type_name(&value.var_type)));
        }
        lock.add_static_input(name.clone(), value.clone());
    }
    Ok(lock)
}

/// A value flowing through a lock: symbols in some base, a maze, or a
/// value of a type a lock file named.
#[derive(Clone)]
pub struct LockValue {
    pub var: InfoVar,
}

impl LockValue {
    /// The value as text, for symbols and strings.
    fn text(&mut self) -> RhaiResult<String> {
        match &self.var.var_type {
            InfoVarType::BaseNSymbol(base) => Ok(symbols_to_string(&self.symbol_data()?, *base)),
            other => self.var.data.clone().into_string()
                .map_err(|_| format!("text: a {} value has no text form", type_name(other)).into()),
        }
    }

    fn base(&mut self) -> RhaiResult<i64> {
        match self.var.var_type {
            InfoVarType::BaseNSymbol(base) => Ok(base as i64),
            ref other => Err(format!("base: a {} value has no base", type_name(other)).into()),
        }
    }

    fn symbols(&mut self) -> RhaiResult<Array> {
        self.symbol_data()?;
        Ok(self.var.data.clone().cast::<Array>())
    }

    /// The value as lock files see it: symbols as an array of integers, a
    /// value kind's data (a maze, a star chart) as its map, and so on.
    fn value(&mut self) -> Dynamic {
        self.var.data.clone()
    }

    fn type_name(&mut self) -> String {
        type_name(&self.var.var_type)
    }

    fn display(&mut self) -> String {
        self.text().unwrap_or_else(|_| format!("<{}>", self.type_name()))
    }

    /// A maze output's data, for drawing: `maze.cells()`, `maze.path()`.
    fn maze(&mut self) -> RhaiResult<Map> {
        match crate::journal::kinds::kind_of(&self.var.data) {
            Some(kind) if kind.name() == "Maze" => Ok(self.var.data.clone().cast::<Map>()),
            _ => Err(format!("maze: a {} value isn't a maze", type_name(&self.var.var_type)).into()),
        }
    }

    /// A value kind's default drawing, as a parts map (see `art::sketch`).
    fn sketch(&mut self) -> RhaiResult<Map> {
        match crate::journal::kinds::sketch(&self.var.data) {
            Ok(Some(sketch)) => Ok(super::art::sketch_parts(sketch)),
            Ok(None) => Err(format!("sketch: a {} value has no sketch", type_name(&self.var.var_type)).into()),
            Err(_) => Err(format!("sketch: a {} value isn't a value kind's, so it has no sketch", type_name(&self.var.var_type)).into()),
        }
    }

    /// The base and symbols of a value of symbols, for drawing from.
    pub(in crate::journal::rhaientries) fn base_and_symbols(&self) -> RhaiResult<(u32, Vec<u32>)> {
        let symbols = self.symbol_data()?;
        match self.var.var_type {
            InfoVarType::BaseNSymbol(base) => Ok((base, symbols)),
            _ => unreachable!("symbol_data fails for values that aren't symbols"),
        }
    }

    fn symbol_data(&self) -> RhaiResult<Vec<u32>> {
        let no_symbols = || -> Box<EvalAltResult> { format!("a {} value has no symbols", type_name(&self.var.var_type)).into() };
        if !matches!(self.var.var_type, InfoVarType::BaseNSymbol(_)) {
            return Err(no_symbols());
        }
        let symbols = self.var.data.read_lock::<Array>().ok_or_else(no_symbols)?;
        symbols.iter().map(|s| s.as_int().ok().and_then(|s| u32::try_from(s).ok()).ok_or_else(no_symbols)).collect()
    }
}

impl CustomType for LockValue {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("LockValue")
            .with_fn("text", Self::text)
            .with_get("base", Self::base)
            .with_fn("symbols", Self::symbols)
            .with_get("type_name", Self::type_name)
            .with_fn("maze", Self::maze)
            .with_fn("sketch", Self::sketch)
            .with_fn("value", Self::value)
            .with_fn("to_string", Self::display)
            .with_fn("to_debug", Self::display);
    }
}

/// The values produced by `run` (the chain's outputs) or `unlock` (the
/// recovered inputs), plus every intermediate value along the way.
#[derive(Clone)]
pub struct LockResult {
    values: InfoVarSet,
    trace: InfoVarSet,
    /// The illustrations attached to the chain, to draw its values.
    illustrations: Rc<Vec<Attachment>>,
    illustration_library: Rc<IllustrationLibrary>,
    /// The resources the chain draws on, for its illustrations.
    sources: Rc<Vec<SourceSpec>>,
    library: Rc<LockLibrary>,
}

impl LockResult {
    fn get(&mut self, name: &str) -> RhaiResult<LockValue> {
        self.values.get(name)
            .map(|var| LockValue { var: var.clone() })
            .ok_or_else(|| format!("'{}' isn't in this result; available: {}", name, sorted_names(&self.values)).into())
    }

    fn text(&mut self, name: &str) -> RhaiResult<String> {
        self.get(name)?.text()
    }

    /// All values: name -> lock value.
    fn values(&mut self) -> Map {
        value_map(&self.values)
    }

    /// Every value computed along the way, including intermediate ones.
    fn trace(&mut self) -> Map {
        value_map(&self.trace)
    }

    /// `result.canvases(id)`: what the illustration `id` draws of this result's
    /// values, an array of canvases.
    fn canvases(&mut self, ctx: &rhai::NativeCallContext, id: &str) -> RhaiResult<Array> {
        let attachment = self.illustrations.iter().find(|r| r.id == id).ok_or_else(|| {
            let ids: Vec<&str> = self.illustrations.iter().map(|r| r.id.as_str()).collect();
            format!("canvases: no illustration '{}' is attached; illustrations: {}", id, if ids.is_empty() { "none".to_string() } else { ids.join(", ") })
        })?;
        let fail = |e: String| -> Box<EvalAltResult> { format!("canvases: {}", e).into() };
        let loaded = self.illustration_library.load(ctx.engine(), &attachment.illustration).map_err(fail)?;
        let options = loaded.options(&attachment.options).map_err(fail)?;
        let mut inputs = Map::new();
        let mut fallbacks = loaded.fallbacks(ctx.engine(), &options).map_err(fail)?;
        for (input, _) in loaded.inputs(ctx.engine(), &options).map_err(fail)? {
            let Some(port) = attachment.wires.iter().find(|(name, _)| *name == input).map(|(_, port)| port) else {
                let fallback = fallbacks.remove(input.as_str())
                    .ok_or_else(|| fail(format!("illustration '{}' has nothing connected to its input '{}'", id, input)))?;
                inputs.insert(input.into(), fallback);
                continue;
            };
            if let Some(source) = port.strip_suffix(".value").and_then(|id| self.sources.iter().find(|s| s.id == id)) {
                inputs.insert(input.into(), source_value(ctx, &self.library, source).map_err(fail)?);
                continue;
            }
            let value = self.trace.get(port).or_else(|| self.values.get(port))
                .ok_or_else(|| fail(format!("illustration '{}' draws '{}', which this result has no value for", id, port)))?;
            inputs.insert(input.into(), value.data.clone());
        }
        let canvases = loaded.render(ctx.engine(), &options, inputs).map_err(fail)?;
        Ok(canvases.into_iter().map(Dynamic::from).collect())
    }
}

impl CustomType for LockResult {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("LockResult")
            .with_fn("get", Self::get)
            .with_fn("text", Self::text)
            .with_fn("values", Self::values)
            .with_fn("trace", Self::trace)
            .with_fn("canvases", |ctx: rhai::NativeCallContext, result: &mut Self, id: &str| result.canvases(&ctx, id));
    }
}

/// What a source gives an illustration: content as lock files see it, a
/// style's bundle, a glyphset, or a layout, made in the journal's engine
/// (`ctx`'s).
pub(super) fn source_value(ctx: &rhai::NativeCallContext, library: &LockLibrary, source: &SourceSpec) -> Result<Dynamic, String> {
    let made = match source.kind {
        SourceKind::Content => return library.source_value(source).map(|var| var.data),
        SourceKind::Style => ctx.call_native_fn::<Dynamic>("text_style", (source.name.clone(), source.options.clone())),
        SourceKind::Glyphset => ctx.call_native_fn::<Dynamic>("glyphset", (source.name.clone(),)),
        SourceKind::Layout => ctx.call_native_fn::<Dynamic>("layout", (source.name.clone(), source.options.clone())),
    };
    made.map_err(|e| format!("source '{}': {}", source.id, super::options::unwrap_runtime(&e)))
}

/// `symbols(base, text)`: text as symbols in `base`. Base 10 takes digits,
/// base 27 takes letters and spaces, other bases up to 26 take the first
/// `base` letters (case-insensitive). Anything else is an error.
fn symbols(base: i64, text: &str, options: &Map) -> RhaiResult<LockValue> {
    super::options::check_keys("symbols", options, &["lenient"])?;
    let base = u32::try_from(base).map_err(|_| format!("symbols: base must be between 2 and 27, got {}", base))?;
    let text = if lenient("symbols", options)? { lenient_text(base, text) } else { text.to_string() };
    let data: Array = base_n_symbols(base, &text).map_err(|e| format!("symbols: {}", e))?
        .into_iter().map(|s| Dynamic::from_int(s as i64)).collect();
    Ok(LockValue { var: InfoVar::new(InfoVarType::BaseNSymbol(base), data.into()) })
}

/// `symbols` in `base` as a value of symbols, as `symbols(base, text)`
/// makes: for symbols read from a content file.
pub(super) fn symbols_value(base: u32, symbols: &[u32]) -> Result<LockValue, String> {
    if !(2..=27).contains(&base) {
        return Err(format!("symbols: base must be between 2 and 27, got {}", base));
    }
    if let Some((i, symbol)) = symbols.iter().enumerate().find(|(_, s)| **s >= base) {
        return Err(format!("symbols: symbol {} (item {}) is outside Base{}'s 0-{}", symbol, i, base, base - 1));
    }
    let data: Array = symbols.iter().map(|s| Dynamic::from_int(*s as i64)).collect();
    Ok(LockValue { var: InfoVar::new(InfoVarType::BaseNSymbol(base), data.into()) })
}

/// The `lenient` option of `symbols`, false unless given.
pub(super) fn lenient(caller: &str, options: &Map) -> RhaiResult<bool> {
    match options.get("lenient") {
        None => Ok(false),
        Some(l) => l.as_bool().map_err(|_| format!("{}: 'lenient' should be true or false, got {}", caller, l).into()),
    }
}

/// Text made fit for `base`: in capitals, characters the base hasn't
/// dropped, and runs of spaces and dashes one space where the base has
/// one, not at either end.
pub(super) fn lenient_text(base: u32, text: &str) -> String {
    let fits = |c: char| base_n_symbols(base, &c.to_string()).is_ok();
    let space = fits(' ');
    let mut out = String::new();
    let mut gap = false;
    for c in text.to_uppercase().chars() {
        if c != ' ' && fits(c) {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(c);
        } else if space && (c.is_whitespace() || c == '-' || c == '—' || c == '–') {
            gap = true;
        }
    }
    out
}

fn type_name(var_type: &InfoVarType) -> String {
    var_type.to_string()
}

/// The type a lock file names, the reverse of `type_name`: "Base27Symbol",
/// "Any", or any other name, such as "Maze".
fn parse_type(name: &str) -> InfoVarType {
    let base = name.strip_prefix("Base")
        .and_then(|rest| rest.strip_suffix("Symbol"))
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|&base| base >= 2);
    match (name, base) {
        (_, Some(base)) => InfoVarType::BaseNSymbol(base),
        ("Any", _) => InfoVarType::MatchAll,
        _ => InfoVarType::Named(name.to_string()),
    }
}

fn type_map(types: &HashMap<String, InfoVarType>, include: impl Fn(&str) -> bool) -> Map {
    types.iter()
        .filter(|(name, _)| include(name))
        .map(|(name, t)| (name.into(), Dynamic::from(type_name(t))))
        .collect()
}

fn value_map(values: &InfoVarSet) -> Map {
    values.iter()
        .map(|(name, var)| (name.into(), Dynamic::from(LockValue { var: var.clone() })))
        .collect()
}

fn value_set(map: Map, what: &str) -> RhaiResult<InfoVarSet> {
    map.into_iter()
        .map(|(name, value)| Ok((name.to_string(), port_value(value, what, &name)?)))
        .collect()
}

fn keys(map: &Map) -> String {
    map.keys().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
}

fn sorted_names(values: &InfoVarSet) -> String {
    sorted_names_of(values.keys())
}

fn sorted_names_of<'a>(names: impl Iterator<Item = &'a String>) -> String {
    let mut names: Vec<&str> = names.map(String::as_str).collect();
    names.sort();
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::canvas::artistic::{BuiltMaze, MazeTraversalStrategy};
    use std::collections::HashSet;

    fn engine() -> Engine {
        let mut engine = crate::journal::rhaientries::rhai_engine();
        let library = super::super::Resources::library();
        register(&mut engine, Rc::new(LockLibrary::new(&library)), Rc::new(IllustrationLibrary::new(library.illustrations)));
        engine
    }

    /// A three-lock chain modelled on journal_entry_17's page_one_lock.
    const CHAIN: &str = r#"
        let lock = new_lock("page_one_lock")
            .add("prime_sub", "substitution_cipher", #{ base_n: 27 })
            .add("maze_content", "vigenere_cipher", #{ key_base: 27, plain_base: 27, cipher_base: 27 })
            .add("maze_encode", "maze_lock", #{ input_base: 27, strategy: "LeftHanded" })
            .connect("prime_sub.cipher_text", "maze_content.key")
            .connect("maze_content.cipher_text", "maze_encode.maze_text")
            .set_input("prime_sub.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS"))
            .set_input("maze_encode.random_seed", symbols(10, "10843"));
    "#;

    fn eval<T: Clone + 'static>(script: &str) -> T {
        engine().eval::<T>(script).unwrap_or_else(|e| panic!("script failed: {}", e))
    }

    fn error(script: &str) -> String {
        engine().run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn inputs_lists_only_open_inputs() {
        let inputs: Map = eval(&format!("{} lock.inputs()", CHAIN));
        let mut names: Vec<String> = inputs.keys().map(|k| k.to_string()).collect();
        names.sort();
        assert_eq!(names, vec!["maze_content.plain_text", "prime_sub.plain_text"]);
        assert_eq!(inputs["prime_sub.plain_text"].clone().cast::<String>(), "Base27Symbol");
    }

    #[test]
    fn run_then_unlock_round_trips() {
        let script = format!(r#"{}
            let result = lock.run(#{{
                "prime_sub.plain_text": symbols(27, "THE KEY"),
                "maze_content.plain_text": symbols(27, "the conversion"),
            }});
            let maze = result.get("maze_encode.generated_maze");
            if maze.type_name != "Maze" {{ throw "expected a maze, got " + maze.type_name; }}
            if !result.trace().contains("maze_content.cipher_text") {{ throw "trace is missing intermediates"; }}
            let back = lock.unlock(result);
            [back.text("maze_content.plain_text"), back.text("prime_sub.plain_text")]
        "#, CHAIN);
        let recovered: Array = eval(&script);
        let recovered: Vec<String> = recovered.into_iter().map(|v| v.cast::<String>()).collect();
        assert_eq!(recovered, vec!["THE CONVERSION", "THE KEY"]);
    }

    #[test]
    fn run_then_unlock_round_trips_without_a_maze() {
        let script = r#"
            let lock = new_lock("pair")
                .add("sub", "substitution_cipher", #{ base_n: 27 })
                .add("vig", "vigenere_cipher", #{ key_base: 27, plain_base: 27, cipher_base: 27 })
                .connect("sub.cipher_text", "vig.key")
                .set_input("sub.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS"));
            let result = lock.run(#{ "sub.plain_text": symbols(27, "KEY"), "vig.plain_text": symbols(27, "attack at dawn") });
            if result.text("vig.cipher_text") == "ATTACK AT DAWN" { throw "cipher text wasn't enciphered"; }
            lock.unlock(result).text("vig.plain_text")
        "#;
        assert_eq!(eval::<String>(script), "ATTACK AT DAWN");
    }

    #[test]
    fn lock_file_types_chain_with_the_others() {
        let script = r#"
            let lock = new_lock("mixed")
                .add("shift", "shift_cipher", #{ base: 27, shift: 5 })
                .add("vig", "vigenere_cipher", #{ key_base: 27, plain_base: 27, cipher_base: 27 })
                .connect("shift.cipher_text", "vig.key");
            let result = lock.run(#{ "shift.plain_text": symbols(27, "KEY"), "vig.plain_text": symbols(27, "attack at dawn") });
            if result.trace()["vig.key"].text() != "PJC" { throw "the shifted key should be PJC"; }
            let back = lock.unlock(result);
            [back.text("vig.plain_text"), back.text("shift.plain_text")]
        "#;
        let recovered: Vec<String> = eval::<Array>(script).into_iter().map(|v| v.cast::<String>()).collect();
        assert_eq!(recovered, vec!["ATTACK AT DAWN", "KEY"]);
    }

    #[test]
    fn lock_file_errors_point_at_the_file_and_the_script() {
        let err = error("let ok = 1;\nnew_lock(\"x\").add(\"v\", \"vigenere_cipher\", #{ plain_base: 27, key_base: 27, cipher_base: 10 });");
        assert!(err.contains("add: lock 'v' (vigenere_cipher.rhai, line 11): cipher_base (10) must be at least plain_base (27)"), "{}", err);
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn unlock_from_hand_supplied_outputs() {
        let script = r#"
            let lock = new_lock("shift").add("s", "substitution_cipher", #{ base_n: 27 })
                .set_input("s.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS"));
            let cipher = lock.run(#{ "s.plain_text": symbols(27, "HELLO") }).get("s.cipher_text");
            let ordering = symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS");
            lock.unlock(#{ "s.cipher_text": cipher, "s.ordering": ordering }).text("s.plain_text")
        "#;
        assert_eq!(eval::<String>(script), "HELLO");
    }

    #[test]
    fn values_expose_text_base_and_symbols() {
        let script = r#"
            let v = symbols(27, "ab c");
            [v.text(), v.base, v.symbols(), v.type_name, `${v}`]
        "#;
        let parts: Array = eval(script);
        assert_eq!(parts[0].clone().cast::<String>(), "AB C");
        assert_eq!(parts[1].clone().cast::<i64>(), 27);
        let symbols: Vec<i64> = parts[2].clone().cast::<Array>().into_iter().map(|v| v.cast::<i64>()).collect();
        assert_eq!(symbols, vec![1, 2, 0, 3]);
        assert_eq!(parts[3].clone().cast::<String>(), "Base27Symbol");
        assert_eq!(parts[4].clone().cast::<String>(), "AB C");
    }

    #[test]
    fn mistakes_fail_at_their_line() {
        let cases = [
            ("let l = new_lock(\"x\");\nl.add(\"a\", \"rot13\", #{});", "unknown lock type 'rot13'"),
            ("let l = new_lock(\"x\");\nl.add(\"a\", \"substitution_cipher\", #{ base_n: \"lots\" });", "option 'base_n' should be an integer, got a string"),
            ("let l = new_lock(\"x\").add(\"a\", \"substitution_cipher\", #{ base_n: 27 });\nl.add(\"a\", \"substitution_cipher\", #{ base_n: 27 });", "already added"),
            ("let l = new_lock(\"x\").add(\"a\", \"substitution_cipher\", #{ base_n: 27 });\nl.connect(\"a.nope\", \"a.plain_text\");", "a.nope"),
            ("let l = new_lock(\"x\").add(\"a\", \"substitution_cipher\", #{ base_n: 27 });\nl.set_input(\"a.ordering\", symbols(10, \"123\"));", "expects Base27Symbol, got Base10Symbol"),
            ("let l = new_lock(\"x\").add(\"a\", \"substitution_cipher\", #{ base_n: 27 });\nl.set_input(\"a.missing\", symbols(27, \"A\"));", "isn't an open input"),
            ("let l = new_lock(\"x\").add(\"a\", \"substitution_cipher\", #{ base_n: 27 });\nl.run(#{ \"a.plain_text\": symbols(27, \"HI\") });", "missing"),
            ("let l = new_lock(\"x\").add(\"a\", \"substitution_cipher\", #{ base_n: 27 });\nl.run(#{ \"a.plain_text\": #{ hi: 1 } });", "must be a lock value"),
            ("let ok = 1;\nlet v = symbols(27, \"hello, world\");", "',' isn't a base-27 symbol"),
            ("let ok = 1;\nlet v = symbols(40, \"A\");", "between 2 and 27"),
        ];
        for (script, expected) in cases {
            let err = error(script);
            assert!(err.contains(expected), "expected {:?} in: {}", expected, err);
            assert!(err.contains("line 2"), "expected line 2 in: {}", err);
        }
    }

    const MAZE: &str = r#"
        let lock = new_lock("m").add("m", "maze_lock", #{ input_base: 27, strategy: "LeftHanded" })
            .set_input("m.random_seed", symbols(10, "42"));
        let maze = lock.run(#{ "m.maze_text": symbols(27, "HELLO WORLD") }).get("m.generated_maze").maze();
    "#;

    #[test]
    fn maze_cells_hold_the_text_in_write_order() {
        let cells: Array = eval(&format!("{} maze.cells()", MAZE));
        let cells: Vec<Map> = cells.into_iter().map(|c| c.cast::<Map>()).collect();
        let field = |c: &Map, k: &str| c[k].clone().cast::<i64>();
        let steps: Vec<i64> = cells.iter().map(|c| field(c, "step")).collect();
        assert_eq!(steps, (0..cells.len() as i64).collect::<Vec<_>>());
        // "HELLO WORLD" in base 27, then zero padding to fill the maze.
        let symbols: Vec<i64> = cells.iter().map(|c| field(c, "symbol")).collect();
        assert_eq!(&symbols[..11], &[8, 5, 12, 12, 15, 0, 23, 15, 18, 12, 4]);
        assert!(symbols[11..].iter().all(|&s| s == 0), "{:?}", symbols);
        // Every cell has a distinct grid position, starting at the origin.
        let positions: HashSet<(i64, i64)> = cells.iter().map(|c| (field(c, "x"), field(c, "y"))).collect();
        assert_eq!(positions.len(), cells.len());
        assert_eq!((field(&cells[0], "x"), field(&cells[0], "y")), (0, 0));
    }

    #[test]
    fn maze_size_covers_its_cells() {
        let size: Array = eval(&format!(r#"{}
            let w = maze.width; let h = maze.height;
            for c in maze.cells() {{ if c.x >= w || c.y >= h {{ throw "cell outside the grid"; }} }}
            [w, h]
        "#, MAZE));
        let (w, h) = (size[0].clone().cast::<i64>(), size[1].clone().cast::<i64>());
        // 11 symbols need 6 nodes; maze_lock sizes mazes to stay square, so
        // that's 3x3 nodes rather than 3x2: 5x5 grid cells with the
        // passages between them.
        assert_eq!((w, h), (5, 5));
    }

    #[test]
    fn maze_lock_uses_the_smallest_square_maze() {
        // A w x w maze holds w*w nodes and w*w - 1 passages, so 7 symbols
        // need 2x2 nodes, 11 and 17 need 3x3, and 19 need 4x4.
        for (len, side) in [(0, 1), (1, 1), (7, 2), (8, 3), (11, 3), (17, 3), (19, 4)] {
            let width: i64 = eval(&format!(r#"
                let lock = new_lock("m").add("m", "maze_lock", #{{ input_base: 27 }})
                    .set_input("m.random_seed", symbols(10, "42"));
                let text = "{}";
                let result = lock.run(#{{ "m.maze_text": symbols(27, text) }});
                if lock.unlock(result).text("m.maze_text") != text {{ throw "didn't round-trip"; }}
                result.get("m.generated_maze").maze().width
            "#, "A".repeat(len)));
            assert_eq!(width, 2 * side - 1, "{} symbols", len);
        }
    }

    #[test]
    fn values_of_any_type_are_reachable() {
        let parts: Array = eval(&format!(r#"{}
            let length = lock.run(#{{ "m.maze_text": symbols(27, "HELLO WORLD") }}).get("m.text_length");
            [length.type_name, length.value(), symbols(27, "AB").value()]
        "#, MAZE));
        assert_eq!(parts[0].clone().cast::<String>(), "LockProcessInfo");
        assert_eq!(parts[1].clone().cast::<i64>(), 11);
        let symbols: Vec<i64> = parts[2].clone().cast::<Array>().into_iter().map(|v| v.cast::<i64>()).collect();
        assert_eq!(symbols, vec![1, 2]);
    }

    #[test]
    fn maze_fill_writes_in_traversal_order() {
        let symbols: Array = eval(r#"
            let maze = new_maze(2, 2, #{ seed: 3 });
            maze.fill([5, 6, 7]);
            maze.cells().map(|c| c.symbol)
        "#);
        let symbols: Vec<i64> = symbols.into_iter().map(|s| s.cast::<i64>()).collect();
        assert_eq!(symbols, vec![5, 6, 7, 0, 0, 0, 0]);
        let err = error("let m = new_maze(2, 2, #{ seed: 3 });\nm.fill([1, 2, 3, 4, 5, 6, 7, 8]);");
        assert!(err.contains("8 symbols don't fit in the maze's 7 cells") && err.contains("line 2"), "{}", err);
        let err = error("let m = new_maze(2, 2, #{ seed: 3 });\nm.fill([1, -2]);");
        assert!(err.contains("item 1 (-2) isn't a symbol"), "{}", err);
    }

    #[test]
    fn maze_path_follows_the_corridors() {
        let path: Path = eval(&format!("{} maze.path()", MAZE));
        let path = path.get();
        let path = path.to_string();
        assert!(path.starts_with("M 0 0 "), "{}", path);
        // Every coordinate is a grid cell inside the maze.
        let numbers: Vec<f32> = path.split_whitespace().filter_map(|t| t.parse().ok()).collect();
        assert!(numbers.iter().all(|&n| (0.0..=4.0).contains(&n) && n.fract() == 0.0), "{}", path);
    }

    #[test]
    fn new_maze_layout_depends_only_on_seed_and_size() {
        let layout = |script: &str| -> Vec<(i64, i64)> {
            let cells: Array = eval(&format!("{}.cells()", script));
            cells.into_iter().map(|c| { let c = c.cast::<Map>(); (c["x"].clone().cast(), c["y"].clone().cast()) }).collect()
        };
        let maze = layout("new_maze(4, 3, #{ seed: 1043 })");
        // 4x3 nodes: 7x5 grid cells, and a spanning tree reaches every node
        // through 11 passages.
        assert_eq!(maze.len(), 12 + 11);
        assert!(maze.iter().all(|&(x, y)| (0..7).contains(&x) && (0..5).contains(&y)), "{:?}", maze);
        assert_eq!(maze, layout("new_maze(4, 3, #{ seed: 1043, strategy: \"LeftHanded\" })"));
        assert_ne!(maze, layout("new_maze(4, 3, #{ seed: 1044 })"));
        let mut right = layout("new_maze(4, 3, #{ seed: 1043, strategy: \"RightHanded\" })");
        assert_ne!(maze, right);
        // The same cells, walked the other way round.
        let mut left = maze.clone();
        left.sort();
        right.sort();
        assert_eq!(left, right);
        // The legacy journals' mazes have the same layout for a seed.
        let legacy = BuiltMaze::<[i64; 1]>::new(1043, 4, 3);
        let legacy: Vec<(i64, i64)> = legacy.iter_inners(MazeTraversalStrategy::LeftHanded)
            .map(|((x, y), _)| (x as i64, y as i64)).collect();
        assert_eq!(maze, legacy);
    }

    #[test]
    fn new_maze_rejects_bad_arguments() {
        for (script, want) in [
            ("let ok = 1;\nnew_maze(0, 3, #{ seed: 1 });", "width must be at least 1"),
            ("let ok = 1;\nnew_maze(3, -2, #{ seed: 1 });", "height must be at least 1"),
            ("let ok = 1;\nnew_maze(3, 3, #{ seed: 1, strategy: \"Sideways\" });", "unknown strategy 'Sideways'"),
        ] {
            let err = error(script);
            assert!(err.contains(want) && err.contains("line 2"), "{}", err);
        }
    }

    #[test]
    fn value_kinds_go_in_and_come_out_of_chains() {
        // A HuffmanCode is the same as journals and lock files write it; a
        // chain's star chart comes out as plain data, and as a drawing.
        let ok: Array = eval(r#"
            let a = huffman::code(symbols(27, "THE QUICK BROWN FOX"), 3);
            let b = huffman::code(symbols(27, "THE QUICK BROWN FOX"), 27, 3);
            let lock = new_lock("sky")
                .add("enc", "huffman_encode", #{ input_base: 27, output_base: 3 })
                .add("sky", "star_chart", #{ input_base: 27, output_base: 3 })
                .connect("enc.code", "sky.code");
            let result = lock.run(#{ "enc.plain_text": symbols(27, "FOX") });
            let chart = result.get("sky.chart");
            let by_hand = lock.unlock(#{ "enc.cipher_text": result.get("enc.cipher_text"), "sky.chart": chart.value() });
            [`${a}` == `${b}`, chart.type_name == "StarChart", chart.value().kind == "StarChart",
             "stars" in chart.sketch(), by_hand.text("enc.plain_text") == "FOX"]
        "#);
        assert!(ok.iter().all(|b| b.as_bool().unwrap()), "{:?}", ok);
        let err = error("let ok = 1;\nhuffman::code(symbols(27, \"AB\"), 10, 3);");
        assert!(err.contains("huffman::code: the symbols are base 27, not 10") && err.contains("line 2"), "{}", err);
        let err = error("let ok = 1;\nnew_lock(\"l\").add(\"s\", \"star_chart\", #{ input_base: 27, output_base: 3 }).set_input(\"s.code\", #{ kind: \"HuffmanCode\", input_base: 27, output_base: 3, nodes: [] });");
        assert!(err.contains("set_input: 's.code' is a bad HuffmanCode: a code needs a root and at least one leaf"), "{}", err);
    }

    #[test]
    fn maze_rejects_non_maze_values() {
        let err = error("let ok = 1;\nsymbols(27, \"HI\").maze();");
        assert!(err.contains("isn't a maze") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn one_output_feeds_several_inputs_of_one_lock() {
        let script = r#"
            let lock = new_lock("fan")
                .add("s", "shift_cipher", #{ base: 27, shift: 1 })
                .add("m", "merge_alpha", #{ base_n: 27, input_count: 2 })
                .connect("s.cipher_text", "m.input_0")
                .connect("s.cipher_text", "m.input_1");
            let result = lock.run(#{ "s.plain_text": symbols(27, "AB") });
            [result.text("m.merged_output"), lock.unlock(result).text("s.plain_text")]
        "#;
        let parts: Vec<String> = eval::<Array>(script).into_iter().map(|v| v.cast::<String>()).collect();
        assert_eq!(parts, vec!["BCBC", "AB"]);
    }

    #[test]
    fn diamonds_run_and_unlock() {
        let script = r#"
            let lock = new_lock("diamond")
                .add("a", "shift_cipher", #{ base: 27, shift: 1 })
                .add("b", "shift_cipher", #{ base: 27, shift: 1 })
                .add("c", "shift_cipher", #{ base: 27, shift: 2 })
                .add("m", "merge_alpha", #{ base_n: 27, input_count: 2 })
                .connect("a.cipher_text", "b.plain_text")
                .connect("a.cipher_text", "c.plain_text")
                .connect("b.cipher_text", "m.input_0")
                .connect("c.cipher_text", "m.input_1");
            let result = lock.run(#{ "a.plain_text": symbols(27, "AB") });
            [result.text("m.merged_output"), lock.unlock(result).text("a.plain_text")]
        "#;
        let parts: Vec<String> = eval::<Array>(script).into_iter().map(|v| v.cast::<String>()).collect();
        assert_eq!(parts, vec!["CDDE", "AB"]);
    }

    #[test]
    fn connections_need_matching_types_one_wire_per_input_and_no_loops() {
        let cases = [
            (
                "let l = new_lock(\"x\").add(\"d\", \"doublering\", #{ canonical_order: \"ABCDEFGHIJKLMNOPQRSTUVWXYZ\" })\n    .add(\"v\", \"vigenere_cipher\", #{ plain_base: 27, key_base: 27, cipher_base: 27 });\nl.connect(\"d.cipher_text\", \"v.key\");",
                "can't connect d.cipher_text (Base26Symbol) to v.key (Base27Symbol)",
            ),
            (
                "let l = new_lock(\"x\").add(\"a\", \"shift_cipher\", #{ base: 27, shift: 1 }).add(\"b\", \"shift_cipher\", #{ base: 27, shift: 1 })\n    .connect(\"a.cipher_text\", \"b.plain_text\");\nl.connect(\"b.cipher_text\", \"a.plain_text\");",
                "the connections make a loop through lock",
            ),
            (
                "let ok = 1;\nlet l = new_lock(\"x\").add(\"a\", \"shift_cipher\", #{ base: 27, shift: 1 });\nl.connect(\"a.cipher_text\", \"a.plain_text\");",
                "the connections make a loop through lock 'a'",
            ),
            (
                "let l = new_lock(\"x\").add(\"a\", \"shift_cipher\", #{ base: 27, shift: 1 }).add(\"b\", \"shift_cipher\", #{ base: 27, shift: 1 })\n    .connect(\"a.cipher_text\", \"b.plain_text\");\nl.connect(\"a.cipher_text\", \"b.plain_text\");",
                "input 'b.plain_text' is already connected",
            ),
        ];
        for (script, expected) in cases {
            let err = error(script);
            assert!(err.contains(expected), "expected {:?} in: {}", expected, err);
            assert!(err.contains("line 3"), "expected line 3 in: {}", err);
        }
    }

    #[test]
    fn failed_changes_leave_the_lock_unchanged() {
        let script = r#"
            let l = new_lock("x").add("a", "substitution_cipher", #{ base_n: 27 });
            try { l.add("b", "rot13", #{}); } catch {}
            l.inputs().keys().len()
        "#;
        assert_eq!(eval::<i64>(script), 2);
    }
}
