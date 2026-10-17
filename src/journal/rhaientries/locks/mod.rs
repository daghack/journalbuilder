//! Rhai API for building and running lock chains (see `crate::lockbuilder`).
//! Each lock type is a lock file, `locks/<type>.rhai` (see `library` and
//! locks/README.md).
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

mod library;

pub use library::{LockLibrary, DEFAULT_LOCKS_DIR};
pub use library::{from_json, to_json, value_from_test, value_view, OptionInfo, Ports, RunResult, TypeInfo, ValueView};

use rhai::{Array, CustomType, Dynamic, Engine, EvalAltResult, Map, TypeBuilder};
use crate::lockbuilder::{
    base_n_symbols, compatible, symbols_to_string, CompositeLock, InfoVar, InfoVarSet, InfoVarType, Lock,
};
use crate::journal::canvas::artistic::{BuiltMaze, MazeTraversalStrategy};
use rand::Fill;
use super::Path;
use std::collections::HashMap;
use std::rc::Rc;

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

/// Register the lock API, with lock types from `library`.
pub fn register(engine: &mut Engine, library: Rc<LockLibrary>) {
    engine
        .build_type::<LockBuilder>()
        .build_type::<LockValue>()
        .build_type::<LockResult>()
        .register_fn("new_lock", move |name: &str| LockBuilder::new(library.clone(), name))
        .register_fn("symbols", |base: i64, text: &str| symbols(base, text, &Map::new()))
        .register_fn("symbols", |base: i64, text: &str, options: Map| symbols(base, text, &options));
    register_maze(engine);
    // Paths are for drawing, so lock files don't get this one.
    engine.register_fn("path", |maze: &mut Maze| maze.path());
}

/// `new_maze` and the `Maze` type, for journals and lock files alike.
fn register_maze(engine: &mut Engine) {
    engine
        .build_type::<Maze>()
        .register_fn("new_maze", |width: i64, height: i64| new_maze(width, height, &Map::new()))
        .register_fn("new_maze", |width: i64, height: i64, options: Map| new_maze(width, height, &options));
}

/// A lock chain under construction: named locks, connections from one
/// lock's output to another's input, and fixed input values. It is rebuilt
/// after every change, so a mistake fails at the line that made it.
#[derive(Clone)]
pub struct LockBuilder {
    library: Rc<LockLibrary>,
    name: String,
    locks: HashMap<String, Rc<dyn Lock>>,
    connections: Vec<(String, String)>,
    fixed_inputs: Vec<(String, InfoVar)>,
    lock: CompositeLock,
}

impl LockBuilder {
    fn new(library: Rc<LockLibrary>, name: &str) -> RhaiResult<Self> {
        let lock = CompositeLock::new(name.to_string(), HashMap::new(), Vec::new())
            .map_err(|e| format!("new_lock: {}", e))?;
        Ok(Self {
            library,
            name: name.to_string(),
            locks: HashMap::new(),
            connections: Vec::new(),
            fixed_inputs: Vec::new(),
            lock,
        })
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
            let lock = next.library.build(name, type_name, options)?;
            next.locks.insert(name.to_string(), Rc::new(lock));
            Ok(())
        })
    }

    fn connect(&mut self, from: &str, to: &str) -> RhaiResult<Self> {
        // CompositeLock::new checks the ports exist, their types match, the
        // input isn't already wired and nothing loops.
        self.update("connect", |next| {
            next.connections.push((from.to_string(), to.to_string()));
            Ok(())
        })
    }

    fn set_input(&mut self, name: &str, value: LockValue) -> RhaiResult<Self> {
        self.update("set_input", |next| {
            next.fixed_inputs.retain(|(existing, _)| existing != name);
            next.fixed_inputs.push((name.to_string(), value.var));
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
        Ok(LockResult { values, trace })
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
        Ok(LockResult { values, trace })
    }
}

impl CustomType for LockBuilder {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Lock")
            .with_fn("add", Self::add)
            .with_fn("add", |lock: &mut Self, name: &str, type_name: &str| lock.add(name, type_name, Map::new()))
            .with_fn("connect", Self::connect)
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
    /// maze as a `Maze`, and so on.
    fn value(&mut self) -> Dynamic {
        self.var.data.clone()
    }

    fn type_name(&mut self) -> String {
        type_name(&self.var.var_type)
    }

    fn display(&mut self) -> String {
        self.text().unwrap_or_else(|_| format!("<{}>", self.type_name()))
    }

    /// A maze output as a `Maze`, for drawing.
    fn maze(&mut self) -> RhaiResult<Maze> {
        self.var.data.clone().try_cast::<Maze>()
            .ok_or_else(|| format!("maze: a {} value isn't a maze", type_name(&self.var.var_type)).into())
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
}

impl CustomType for LockResult {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("LockResult")
            .with_fn("get", Self::get)
            .with_fn("text", Self::text)
            .with_fn("values", Self::values)
            .with_fn("trace", Self::trace);
    }
}

/// A maze for drawing, from a maze lock or `new_maze`. Cells sit on an
/// integer grid: nodes at even coordinates, passages between connected
/// nodes. In a lock's maze each cell holds one symbol of the locked text.
#[derive(Clone)]
pub struct Maze {
    maze: BuiltMaze<SymbolInfo>,
    /// The order cells are walked in: for a lock's maze, the order the text
    /// was written in.
    strategy: MazeTraversalStrategy,
}

/// What a maze cell holds: a symbol, random in a new maze.
#[derive(Clone, Debug, Default)]
struct SymbolInfo {
    symbol: u32,
}

impl Fill for SymbolInfo {
    fn fill<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) {
        self.symbol = rng.random_range(0..27);
    }
}

impl Maze {
    /// Grid width in cells (passages included).
    fn width(&mut self) -> i64 {
        self.maze.all_cells().iter().map(|c| c.position.0).max().map_or(0, |x| x as i64 + 1)
    }

    /// Grid height in cells (passages included).
    fn height(&mut self) -> i64 {
        self.maze.all_cells().iter().map(|c| c.position.1).max().map_or(0, |y| y as i64 + 1)
    }

    /// Every cell, in traversal order (the order a lock writes its text):
    /// `#{ x, y, step, symbol }`. `symbol` is 0-26: the text's symbol in a
    /// lock's maze, random in `new_maze`'s.
    fn cells(&mut self) -> Array {
        self.maze.iter_inners(self.strategy)
            .enumerate()
            .map(|(step, ((x, y), inner))| {
                let mut cell = Map::new();
                cell.insert("x".into(), Dynamic::from(x as i64));
                cell.insert("y".into(), Dynamic::from(y as i64));
                cell.insert("step".into(), Dynamic::from(step as i64));
                cell.insert("symbol".into(), Dynamic::from(inner.symbol as i64));
                Dynamic::from_map(cell)
            })
            .collect()
    }

    /// Write `symbols` into the cells in traversal order, and 0 into any
    /// cells left over.
    fn fill(&mut self, symbols: Array) -> RhaiResult<()> {
        let cells = self.maze.all_cells().len();
        if symbols.len() > cells {
            return Err(format!("fill: {} symbols don't fit in the maze's {} cells", symbols.len(), cells).into());
        }
        let symbols = symbols.iter().enumerate()
            .map(|(i, item)| item.as_int().ok().and_then(|s| u32::try_from(s).ok())
                .ok_or_else(|| -> Box<EvalAltResult> { format!("fill: item {} ({:?}) isn't a symbol", i, item).into() }))
            .collect::<RhaiResult<Vec<u32>>>()?;
        let mut symbols = symbols.into_iter();
        self.maze.map_inners(self.strategy, |inner| inner.symbol = symbols.next().unwrap_or(0));
        Ok(())
    }

    /// A path along the corridors, in grid units, following the same
    /// traversal the text was written in.
    fn path(&mut self) -> Path {
        Path::new(self.maze.corridor_path(self.strategy))
    }
}

impl CustomType for Maze {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Maze")
            .with_get("width", Self::width)
            .with_get("height", Self::height)
            .with_fn("cells", Self::cells)
            .with_fn("fill", Self::fill);
    }
}

/// `new_maze(width, height[, strategy], #{ seed: seed })`: a maze of `width` x `height`
/// nodes (so a grid of 2w-1 x 2h-1 cells), traversed "LeftHanded" (the
/// default) or "RightHanded". Its layout depends only on the seed and size.
/// `new_maze(width, height[, #{ seed, strategy }])`.
fn new_maze(width: i64, height: i64, options: &Map) -> RhaiResult<Maze> {
    const CALLER: &str = "new_maze";
    super::options::check_keys(CALLER, options, &["seed", "strategy"])?;
    let seed = match options.get("seed") {
        None => 0,
        Some(s) => s.as_int().map_err(|t| format!("{}: 'seed' should be a whole number, got {}", CALLER, t))?,
    };
    let strategy = options.get("strategy").map_or("LeftHanded".to_string(), |s| s.to_string());
    let strategy = match strategy.as_str() {
        "LeftHanded" => MazeTraversalStrategy::LeftHanded,
        "RightHanded" => MazeTraversalStrategy::RightHanded,
        other => return Err(format!("new_maze: unknown strategy '{}'; expected \"LeftHanded\" or \"RightHanded\"", other).into()),
    };
    let size = |n: i64, what: &str| u32::try_from(n).ok().filter(|&n| n >= 1)
        .ok_or_else(|| -> Box<EvalAltResult> { format!("new_maze: {} must be at least 1, got {}", what, n).into() });
    let (width, height) = (size(width, "width")?, size(height, "height")?);
    Ok(Maze { maze: BuiltMaze::new(seed as u64, width, height), strategy })
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
        .map(|(name, value)| match value.try_cast::<LockValue>() {
            Some(value) => Ok((name.to_string(), value.var)),
            None => Err(format!("{}: '{}' must be a lock value, e.g. symbols(27, \"TEXT\")", what, name).into()),
        })
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
    use std::collections::HashSet;

    fn engine() -> Engine {
        let mut engine = Engine::new();
        register(&mut engine, Rc::new(LockLibrary::new(DEFAULT_LOCKS_DIR)));
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
            ("let l = new_lock(\"x\").add(\"a\", \"substitution_cipher\", #{ base_n: 27 });\nl.run(#{ \"a.plain_text\": \"HI\" });", "must be a lock value"),
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
