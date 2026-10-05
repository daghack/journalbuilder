//! Rhai API for building and running lock chains (see `crate::lockbuilder`).
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

use rhai::{Array, CustomType, Dynamic, Engine, EvalAltResult, Map, TypeBuilder};
use crate::lockbuilder::{
    symbols_to_string, CompositeLock, InfoVar, InfoVarGraphType, InfoVarSet, InfoVarType, Lock,
    LockConfig, LockTypeConfig,
};
use crate::lockbuilder::locks::maze::LockedMaze;
use crate::lockbuilder::registry::build_lock;
use crate::journal::paths::StrokePath;
use std::collections::HashMap;

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

pub fn register(engine: &mut Engine) {
    engine
        .build_type::<LockBuilder>()
        .build_type::<LockValue>()
        .build_type::<LockResult>()
        .build_type::<Maze>()
        .register_fn("new_lock", LockBuilder::new)
        .register_fn("symbols", symbols);
}

/// A lock chain under construction: named locks, connections from one
/// lock's output to another's input, and fixed input values. It is rebuilt
/// after every change, so a mistake fails at the line that made it.
#[derive(Clone)]
pub struct LockBuilder {
    config: LockConfig,
    fixed_inputs: Vec<(String, InfoVar)>,
    lock: CompositeLock,
}

impl LockBuilder {
    fn new(name: &str) -> RhaiResult<Self> {
        let config = LockConfig {
            name: name.to_string(),
            locks: HashMap::new(),
            connections: Vec::new(),
            static_inputs: HashMap::new(),
        };
        let lock = compile(&config, &[]).map_err(|e| format!("new_lock: {}", e))?;
        Ok(Self { config, fixed_inputs: Vec::new(), lock })
    }

    /// Apply `change` to a copy, keeping it only if the result still builds.
    fn update(&mut self, what: &str, change: impl FnOnce(&mut Self) -> Result<(), String>) -> RhaiResult<Self> {
        let mut next = self.clone();
        change(&mut next).map_err(|e| format!("{}: {}", what, e))?;
        next.lock = compile(&next.config, &next.fixed_inputs).map_err(|e| format!("{}: {}", what, e))?;
        *self = next;
        Ok(self.clone())
    }

    fn add(&mut self, name: &str, type_name: &str, options: Map) -> RhaiResult<Self> {
        let options: serde_json::Value = rhai::serde::from_dynamic(&Dynamic::from_map(options))
            .map_err(|e| format!("add: options for '{}' aren't valid: {}", name, e))?;
        self.update("add", |next| {
            if name.is_empty() || name.contains('.') {
                return Err(format!("lock name '{}' must be non-empty and contain no '.'", name));
            }
            if next.config.locks.contains_key(name) {
                return Err(format!("a lock named '{}' was already added", name));
            }
            next.config.locks.insert(name.to_string(), LockTypeConfig { type_name: type_name.to_string(), options });
            Ok(())
        })
    }

    fn connect(&mut self, from: &str, to: &str) -> RhaiResult<Self> {
        self.update("connect", |next| {
            if next.config.connections.iter().any(|(_, input)| input == to) {
                return Err(format!("input '{}' is already connected", to));
            }
            next.config.connections.push((from.to_string(), to.to_string()));
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
            .with_fn("connect", Self::connect)
            .with_fn("set_input", Self::set_input)
            .with_fn("inputs", Self::inputs)
            .with_fn("outputs", Self::outputs)
            .with_fn("run", Self::run)
            .with_fn("unlock", Self::unlock)
            .with_fn("unlock", Self::unlock_map);
    }
}

/// A value flowing through a lock: symbols in some base, plain text, or a
/// graph (a maze, a Huffman tree).
#[derive(Clone)]
pub struct LockValue {
    pub var: InfoVar,
}

impl LockValue {
    /// The value as text, for symbol and string values.
    fn text(&mut self) -> RhaiResult<String> {
        match &self.var.var_type {
            InfoVarType::BaseNSymbol(base) => Ok(symbols_to_string(self.symbol_data()?, *base)),
            InfoVarType::String => self.var.borrow_data::<String>().map(String::clone).map_err(|e| e.to_string().into()),
            other => Err(format!("text: a {} value has no text form", type_name(other)).into()),
        }
    }

    fn base(&mut self) -> RhaiResult<i64> {
        match self.var.var_type {
            InfoVarType::BaseNSymbol(base) => Ok(base as i64),
            ref other => Err(format!("base: a {} value has no base", type_name(other)).into()),
        }
    }

    fn symbols(&mut self) -> RhaiResult<Array> {
        Ok(self.symbol_data()?.iter().map(|&s| Dynamic::from(s as i64)).collect())
    }

    fn type_name(&mut self) -> String {
        type_name(&self.var.var_type)
    }

    fn to_string(&mut self) -> String {
        self.text().unwrap_or_else(|_| format!("<{}>", self.type_name()))
    }

    /// A maze output as a `Maze`, for drawing.
    fn maze(&mut self) -> RhaiResult<Maze> {
        self.var.borrow_data::<LockedMaze>()
            .map(|locked| Maze { locked: locked.clone() })
            .map_err(|_| format!("maze: a {} value isn't a maze", type_name(&self.var.var_type)).into())
    }

    fn symbol_data(&self) -> RhaiResult<&Vec<u32>> {
        self.var.borrow_data::<Vec<u32>>().map_err(|_| format!("a {} value has no symbols", type_name(&self.var.var_type)).into())
    }
}

impl CustomType for LockValue {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("LockValue")
            .with_fn("text", Self::text)
            .with_fn("base", Self::base)
            .with_fn("symbols", Self::symbols)
            .with_fn("type_name", Self::type_name)
            .with_fn("maze", Self::maze)
            .with_fn("to_string", Self::to_string)
            .with_fn("to_debug", Self::to_string);
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

/// A maze from a maze lock, for drawing. Cells sit on an integer grid:
/// nodes at even coordinates, passages between connected nodes. Each cell
/// holds one symbol of the locked text.
#[derive(Clone)]
pub struct Maze {
    locked: LockedMaze,
}

impl Maze {
    /// Grid width in cells (passages included).
    fn width(&mut self) -> i64 {
        self.locked.maze.all_cells().iter().map(|c| c.position.0).max().map_or(0, |x| x as i64 + 1)
    }

    /// Grid height in cells (passages included).
    fn height(&mut self) -> i64 {
        self.locked.maze.all_cells().iter().map(|c| c.position.1).max().map_or(0, |y| y as i64 + 1)
    }

    /// Every cell, in the order the text was written into the maze:
    /// `#{ x, y, symbol, step }`.
    fn cells(&mut self) -> Array {
        self.locked.maze.iter_inners(self.locked.strategy)
            .enumerate()
            .map(|(step, ((x, y), inner))| {
                let mut cell = Map::new();
                cell.insert("x".into(), Dynamic::from(x as i64));
                cell.insert("y".into(), Dynamic::from(y as i64));
                cell.insert("symbol".into(), Dynamic::from(inner.symbol as i64));
                cell.insert("step".into(), Dynamic::from(step as i64));
                Dynamic::from_map(cell)
            })
            .collect()
    }

    /// A path along the corridors, in grid units, following the same
    /// traversal the text was written in.
    fn path(&mut self) -> StrokePath {
        self.locked.maze.corridor_path(self.locked.strategy)
    }
}

impl CustomType for Maze {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Maze")
            .with_fn("width", Self::width)
            .with_fn("height", Self::height)
            .with_fn("cells", Self::cells)
            .with_fn("path", Self::path);
    }
}

/// `symbols(base, text)`: text as symbols in `base`. Base 10 takes digits,
/// base 27 takes letters and spaces, other bases up to 26 take the first
/// `base` letters (case-insensitive). Anything else is an error.
fn symbols(base: i64, text: &str) -> RhaiResult<LockValue> {
    if !(2..=27).contains(&base) {
        return Err(format!("symbols: base must be between 2 and 27, got {}", base).into());
    }
    let base = base as u32;
    if let Some(bad) = text.chars().find(|&c| !is_symbol(c, base)) {
        return Err(format!("symbols: '{}' isn't a base-{} symbol ({})", bad, base, symbol_help(base)).into());
    }
    Ok(LockValue { var: InfoVar::from(format!("b{}:{}", base, text)) })
}

fn is_symbol(c: char, base: u32) -> bool {
    let upper = c.to_ascii_uppercase();
    match base {
        10 => c.is_ascii_digit(),
        27 => c == ' ' || upper.is_ascii_uppercase(),
        _ => upper.is_ascii_uppercase() && (upper as u32 - 'A' as u32) < base,
    }
}

fn symbol_help(base: u32) -> String {
    match base {
        10 => "digits 0-9".to_string(),
        27 => "letters and spaces".to_string(),
        _ => format!("letters A-{}", (b'A' + base as u8 - 1) as char),
    }
}

fn compile(config: &LockConfig, fixed_inputs: &[(String, InfoVar)]) -> Result<CompositeLock, String> {
    let mut lock = CompositeLock::from_lockconfig(config.clone(), |c| build_lock(&c.type_name, &c.options))
        .map_err(|e| e.to_string())?;
    let open = lock.inputs();
    for (name, value) in fixed_inputs {
        let expected = open.get(name)
            .ok_or_else(|| format!("'{}' isn't an open input; open inputs: {}", name, sorted_names_of(open.keys())))?;
        if *expected != InfoVarType::MatchAll && value.var_type != *expected {
            return Err(format!("'{}' expects {}, got {}", name, type_name(expected), type_name(&value.var_type)));
        }
        lock.add_static_input(name.clone(), value.clone());
    }
    Ok(lock)
}

fn type_name(var_type: &InfoVarType) -> String {
    match var_type {
        InfoVarType::String => "String".to_string(),
        InfoVarType::BaseNSymbol(base) => format!("Base{}Symbol", base),
        InfoVarType::Graph(InfoVarGraphType::MazeLock) => "Maze".to_string(),
        InfoVarType::Graph(InfoVarGraphType::HuffmanEncoding) => "HuffmanTree".to_string(),
        InfoVarType::LockProcessInfo => "LockProcessInfo".to_string(),
        InfoVarType::MatchAll => "Any".to_string(),
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
        register(&mut engine);
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
        engine().run(script).err().expect("script should fail").to_string()
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
            if maze.type_name() != "Maze" {{ throw "expected a maze, got " + maze.type_name(); }}
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
            [v.text(), v.base(), v.symbols(), v.type_name(), `${v}`]
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
            ("let l = new_lock(\"x\");\nl.add(\"a\", \"substitution_cipher\", #{ base_n: \"lots\" });", "invalid options"),
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
            let w = maze.width(); let h = maze.height();
            for c in maze.cells() {{ if c.x >= w || c.y >= h {{ throw "cell outside the grid"; }} }}
            [w, h]
        "#, MAZE));
        let (w, h) = (size[0].clone().cast::<i64>(), size[1].clone().cast::<i64>());
        // 11 symbols need 6 nodes; MazeLock sizes mazes to stay square, so
        // that's 3x3 nodes rather than 3x2: 5x5 grid cells with the
        // passages between them.
        assert_eq!((w, h), (5, 5));
    }

    #[test]
    fn maze_path_follows_the_corridors() {
        let path: StrokePath = eval(&format!("{} maze.path()", MAZE));
        let path = path.to_string();
        assert!(path.starts_with("M 0 0 "), "{}", path);
        // Every coordinate is a grid cell inside the maze.
        let numbers: Vec<f32> = path.split_whitespace().filter_map(|t| t.parse().ok()).collect();
        assert!(numbers.iter().all(|&n| n >= 0.0 && n <= 4.0 && n.fract() == 0.0), "{}", path);
    }

    #[test]
    fn maze_rejects_non_maze_values() {
        let err = error("let ok = 1;\nsymbols(27, \"HI\").maze();");
        assert!(err.contains("isn't a maze") && err.contains("line 2"), "{}", err);
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
