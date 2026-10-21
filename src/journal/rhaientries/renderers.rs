//! Renderer files: rhai scripts that draw a lock's values as canvases.
//! Each renderer type is a file, `webapp/renderers/<type>.rhai`, run in the
//! journal's engine (so it can draw), never in the lock files': locks stay
//! values in, values out, and a renderer turns their values into pictures.
//!
//! A renderer file defines:
//!
//! - `options()` (optional): `#{ NAME: default }`. Every option is
//!   optional, its type its default's.
//! - `inputs(o)`: `#{ NAME: type }`, the values it draws, typed as lock
//!   files type theirs (`"StarChart"`, `"Base27Symbol"`, `"Any"`).
//! - `render(o, i)`: a canvas, or an array of canvases (a page each, say).
//! - `example()` (optional): `#{ NAME: value }`, inputs to preview it with.
//!
//! Journals attach one to a lock chain, `lock.attach("pages", "star_pages",
//! #{ digit: 24 })`, wire values to it, `lock.connect("sky.chart",
//! "pages.chart")`, and draw a run's values with `result.canvases("pages")`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rhai::{Array, Dynamic, Engine, Map, Scope, AST};

use super::sources::Sources;
use super::Canvas;

/// The repository's renderer files.
pub const DEFAULT_RENDERERS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/webapp/renderers");

/// The renderer types: the files in a directory, loaded on first use.
pub struct RendererLibrary {
    sources: Sources,
    types: RefCell<HashMap<String, Rc<RendererType>>>,
}

/// A loaded renderer file.
pub struct RendererType {
    pub name: String,
    pub file: String,
    ast: Rc<AST>,
    /// Its options' defaults, in name order.
    pub defaults: Map,
    /// The file's opening comment, without its "name: ".
    pub description: String,
}

impl RendererLibrary {
    pub fn new(sources: Sources) -> RendererLibrary {
        RendererLibrary { sources, types: RefCell::default() }
    }

    pub fn sources(&self) -> &Sources {
        &self.sources
    }

    /// Every renderer type's name (helpers in `lib/` aren't types).
    pub fn names(&self) -> Vec<String> {
        let mut names = self.sources.names().unwrap_or_default();
        names.retain(|n| !n.starts_with("lib/"));
        names.sort();
        names
    }

    /// The renderer type `name`, compiled with `engine` (the journal's).
    pub fn load(&self, engine: &Engine, name: &str) -> Result<Rc<RendererType>, String> {
        if let Some(loaded) = self.types.borrow().get(name) {
            return Ok(loaded.clone());
        }
        if name.is_empty() || name.contains(['/', '\\', '.']) || !self.sources.has(name) {
            return Err(format!("no renderer named '{}'; renderers: {}", name, self.names().join(", ")));
        }
        let file = self.sources.path(name).display().to_string();
        let ast = self.sources.compile(engine, name).map_err(|e| format!("renderer '{}': {}", name, e))?;
        let defines = |f: &str, n: usize| ast.iter_functions().any(|d| d.name == f && d.params.len() == n);
        for (function, params, what) in [("inputs", 1, "inputs(o)"), ("render", 2, "render(o, i)")] {
            if !defines(function, params) {
                return Err(format!("renderer '{}': {} doesn't define {}", name, file, what));
            }
        }
        let defaults = if defines("options", 0) {
            let value = engine.call_fn::<Dynamic>(&mut Scope::new(), &ast, "options", ())
                .map_err(|e| format!("renderer '{}' options(): {}", name, e))?;
            let type_name = value.type_name();
            value.try_cast::<Map>().ok_or_else(|| format!("renderer '{}': options() should return #{{ NAME: default }}, got {}", name, type_name))?
        } else {
            Map::new()
        };
        let source = self.sources.read(name).map_err(|e| e.to_string())?;
        let loaded = Rc::new(RendererType { name: name.to_string(), file, ast: Rc::new(ast), defaults, description: description(&source, name) });
        self.types.borrow_mut().insert(name.to_string(), loaded.clone());
        Ok(loaded)
    }
}

impl RendererType {
    /// `given` over the defaults; an option it doesn't have, or one of
    /// another type than its default, is an error.
    pub fn options(&self, given: &Map) -> Result<Map, String> {
        let mut options = self.defaults.clone();
        for (key, value) in given {
            let Some(default) = self.defaults.get(key.as_str()) else {
                let known: Vec<&str> = self.defaults.keys().map(|k| k.as_str()).collect();
                return Err(format!("renderer '{}' has no option '{}'; its options: {}", self.name, key, if known.is_empty() { "none".to_string() } else { known.join(", ") }));
            };
            let number = |v: &Dynamic| v.is_int() || v.is_float();
            let same = (number(default) && number(value)) || default.type_name() == value.type_name() || default.is_unit();
            if !same {
                return Err(format!("renderer '{}': option '{}' should be like its default, {}, got {}", self.name, key, default, value));
            }
            // A whole number given for a float option is that float, so render sees the default's type.
            let value = match value.as_int() {
                Ok(n) if default.is_float() => Dynamic::from_float(n as rhai::FLOAT),
                _ => value.clone(),
            };
            options.insert(key.clone(), value);
        }
        Ok(options)
    }

    /// The values it draws, name -> type, for `o`.
    pub fn inputs(&self, engine: &Engine, o: &Map) -> Result<Vec<(String, String)>, String> {
        let value = engine.call_fn::<Dynamic>(&mut Scope::new(), &self.ast, "inputs", (Dynamic::from_map(o.clone()),))
            .map_err(|e| format!("renderer '{}' inputs(o): {}", self.name, e))?;
        let type_name = value.type_name();
        let map = value.try_cast::<Map>().ok_or_else(|| format!("renderer '{}': inputs(o) should return #{{ NAME: type }}, got {}", self.name, type_name))?;
        map.into_iter().map(|(name, t)| {
            let t = t.into_string().map_err(|got| format!("renderer '{}': input '{}' should have a type name, got {}", self.name, name, got))?;
            Ok((name.to_string(), t))
        }).collect()
    }

    /// The canvases it draws `inputs` (name -> value) as, with `o`.
    pub fn render(&self, engine: &Engine, o: &Map, inputs: Map) -> Result<Vec<Canvas>, String> {
        let value = engine.call_fn::<Dynamic>(&mut Scope::new(), &self.ast, "render", (Dynamic::from_map(o.clone()), Dynamic::from_map(inputs)))
            .map_err(|e| format!("renderer '{}' render(o, i): {}", self.name, e))?;
        if value.is::<Canvas>() {
            return Ok(vec![value.cast::<Canvas>()]);
        }
        let type_name = value.type_name();
        let items = value.try_cast::<Array>().ok_or_else(|| format!("renderer '{}': render(o, i) should return a canvas or an array of canvases, got {}", self.name, type_name))?;
        items.into_iter().enumerate().map(|(i, item)| {
            let type_name = item.type_name();
            item.try_cast::<Canvas>().ok_or_else(|| format!("renderer '{}': render(o, i) returned an array whose item {} is a {}, not a canvas", self.name, i, type_name))
        }).collect()
    }

    /// The inputs its file gives to preview it with, if it gives any.
    pub fn example(&self, engine: &Engine) -> Result<Option<Map>, String> {
        if !self.ast.iter_functions().any(|f| f.name == "example" && f.params.is_empty()) {
            return Ok(None);
        }
        let value = engine.call_fn::<Dynamic>(&mut Scope::new(), &self.ast, "example", ())
            .map_err(|e| format!("renderer '{}' example(): {}", self.name, e))?;
        let type_name = value.type_name();
        value.try_cast::<Map>().map(Some).ok_or_else(|| format!("renderer '{}': example() should return #{{ NAME: value }}, got {}", self.name, type_name))
    }
}

/// The opening `//` comment of a file, joined into a line, without a
/// leading "name: ".
fn description(source: &str, name: &str) -> String {
    let lines: Vec<&str> = source.lines().map(str::trim).take_while(|l| l.starts_with("//")).map(|l| l.trim_start_matches('/').trim()).collect();
    let text = lines.join(" ");
    text.strip_prefix(&format!("{}:", name)).map(str::trim).unwrap_or(&text).to_string()
}

#[cfg(test)]
mod tests {
    use super::super::sources::Sources;
    use super::super::{JournalEngine, DEFAULT_CHAINS_DIR, DEFAULT_LAYOUTS_DIR, DEFAULT_LOCKS_DIR};
    use super::DEFAULT_RENDERERS_DIR;
    use crate::journal::fonts::{FontBook, DEFAULT_FONTS};
    use std::rc::Rc;

    fn engine_with(chains: &[(&str, &str)], renderers: &[(&str, &str)]) -> JournalEngine {
        let overlay = |files: &[(&str, &str)]| files.iter().map(|(n, s)| (n.to_string(), s.to_string())).collect::<Vec<_>>();
        JournalEngine::with_sources(
            Rc::new(FontBook::from_file(DEFAULT_FONTS).expect("fonts load")),
            Sources::dir(DEFAULT_LOCKS_DIR),
            Sources::dir(DEFAULT_LAYOUTS_DIR),
            Sources::dir(DEFAULT_RENDERERS_DIR).with_overlay(overlay(renderers)),
            Sources::dir(DEFAULT_CHAINS_DIR).with_overlay(overlay(chains)),
        )
    }

    fn engine() -> JournalEngine {
        engine_with(&[], &[])
    }

    #[test]
    fn every_renderer_draws_its_example() {
        let engine = engine();
        let names = engine.renderer_names();
        assert!(names.contains(&"star_pages".to_string()) && names.contains(&"maze_glyphs".to_string()), "{:?}", names);
        for name in names {
            let info = engine.describe_renderer(&name).unwrap_or_else(|e| panic!("{}: {}", name, e));
            assert!(info.example, "{} should have an example() to preview", name);
            assert!(!info.description.is_empty() && !info.inputs.is_empty(), "{}", name);
            let entry = engine.render(&name, &serde_json::json!({}), None).unwrap_or_else(|e| panic!("{}: {}", name, e));
            assert!(entry.page_count() > 0 && entry.empty_canvases().is_empty(), "{} should draw something", name);
        }
    }

    #[test]
    fn a_journal_attaches_renderers_and_draws_a_run() {
        let script = r#"
            let lock = new_lock("m")
                .add("maze", "maze_lock", #{ input_base: 27 })
                .attach("drawn", "maze_glyphs", #{ cell: 40.0 })
                .connect("maze.generated_maze", "drawn.maze");
            let result = lock.run(#{ "maze.maze_text": symbols(27, "HELLO"), "maze.random_seed": symbols(10, "4") });
            let canvases = result.canvases("drawn");
            if canvases.len() != 1 { throw "one canvas"; }
            // Wired to a renderer, the maze is still the chain's output.
            if !("maze.generated_maze" in lock.outputs()) { throw "the output stays the chain's"; }
            if lock.renderers()["drawn"] != "maze_glyphs" { throw "renderers() lists it"; }
            let p = new_page();
            p.add(canvases[0]);
            journal_entry.add(p);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        assert_eq!(entry.page_count(), 1);
    }

    #[test]
    fn attaching_and_wiring_mistakes_are_named() {
        let chain = r#"let lock = new_lock("m").add("maze", "maze_lock", #{ input_base: 27 });"#;
        for (line, want) in [
            (r#"lock.attach("d", "nope");"#, "attach: no renderer named 'nope'; renderers: "),
            (r#"lock.attach("d", "maze_glyphs", #{ size: 3 });"#, "attach: renderer 'maze_glyphs' has no option 'size'; its options: cell, corridors, glyphs"),
            (r#"lock.attach("d", "maze_glyphs", #{ cell: "big" });"#, "attach: renderer 'maze_glyphs': option 'cell' should be like its default, 70.0, got big"),
            (r#"lock.attach("maze", "maze_glyphs");"#, "attach: there's a lock or renderer named 'maze' already"),
            (r#"lock.attach("d", "maze_glyphs").connect("maze.generated_maze", "d.grid");"#, "connect: renderer 'd' (maze_glyphs) has no input 'grid'; inputs: maze"),
            (r#"lock.attach("d", "maze_glyphs").connect("maze.text_length", "d.maze");"#, "connect: can't connect maze.text_length (LockProcessInfo) to d.maze (Maze)"),
            (r#"lock.attach("d", "maze_glyphs").connect("maze.nope", "d.maze");"#, "connect: 'maze.nope' isn't a port of the chain"),
            (r#"lock.attach("d", "maze_glyphs").run(#{ "maze.maze_text": symbols(27, "HI"), "maze.random_seed": symbols(10, "1") }).canvases("d");"#, "canvases: renderer 'd' has nothing connected to its input 'maze'"),
            (r#"lock.run(#{ "maze.maze_text": symbols(27, "HI"), "maze.random_seed": symbols(10, "1") }).canvases("d");"#, "canvases: no renderer 'd' is attached; renderers: none"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", chain, line)).expect_err(line).to_string();
            assert!(err.contains(want), "{}\n  wanted: {}\n  got: {}", line, want, err);
        }
    }

    const DRAWN_MAZE: &str = r#"
        // drawn_maze: a maze lock's maze, drawn.
        fn options() { #{ cell: number(30.0) } }
        fn chain(o) {
            #{
                locks: #{ maze: #{ type: "maze_lock", options: #{ input_base: 27 } } },
                connect: [],
                fixed: #{ "maze.random_seed": symbols(10, "2") },
                renderers: #{ drawn: #{ type: "maze_glyphs", options: #{ cell: o.cell }, inputs: #{ maze: "maze.generated_maze" } } },
            }
        }
    "#;

    #[test]
    fn a_journal_uses_a_chain_file() {
        let mut engine = engine_with(&[("drawn_maze", DRAWN_MAZE)], &[]);
        let entry = engine.build_entry_str(r#"
            let c = chain("drawn_maze", #{ cell: 40 });
            if c.renderers()["drawn"] != "maze_glyphs" { throw "the chain's renderer comes with it"; }
            if "maze.random_seed" in c.inputs() { throw "a fixed input needs no value"; }
            let result = c.run(#{ "maze.maze_text": symbols(27, "HI") });
            if c.unlock(result).text("maze.maze_text") != "HI" { throw "it unlocks"; }
            // It's a lock like new_lock's: a journal can attach more.
            let more = c.attach("again", "maze_glyphs").connect("maze.generated_maze", "again.maze").run(#{ "maze.maze_text": symbols(27, "HI") });
            let p = new_page();
            p.add(result.canvases("drawn")[0]);
            p.add(more.canvases("again")[0]);
            journal_entry.add(p);
        "#).expect("script should run");
        assert_eq!(entry.page_count(), 1);
        for (script, want) in [
            (r#"chain("nope");"#, "chain: no chain 'nope'; chains: "),
            (r#"chain("drawn_maze", #{ size: 2 });"#, "chain: chains/drawn_maze.rhai: unknown option 'size'; options are cell (a number)"),
            (r#"chain("bad");"#, "chain: chains/bad.rhai: attach: renderer 'maze_glyphs' has no option 'size'"),
        ] {
            let mut engine = engine_with(&[("drawn_maze", DRAWN_MAZE), ("bad", &DRAWN_MAZE.replace("cell: o.cell", "size: 2"))], &[]);
            let err = engine.build_entry_str(script).expect_err(script).to_string();
            assert!(err.contains(want), "{}\n  wanted: {}\n  got: {}", script, want, err);
        }
    }

    #[test]
    fn every_chain_file_runs_its_example() {
        let library = std::rc::Rc::new(super::super::LockLibrary::new(DEFAULT_LOCKS_DIR).with_chains(Sources::dir(DEFAULT_CHAINS_DIR)));
        let results = library.check_all_chains().unwrap();
        assert!(!results.is_empty(), "webapp/chains has chain files");
        for (name, result) in results {
            assert_eq!(result.unwrap_or_else(|e| panic!("{}", e)), 1, "{} should have an example", name);
        }
    }

    #[test]
    fn renderer_files_must_define_inputs_and_render() {
        let engine = engine_with(&[], &[("half", "fn inputs(o) { #{} }"), ("canvasless", "fn inputs(o) { #{} }\nfn render(o, i) { 3 }")]);
        assert!(engine.render("half", &serde_json::json!({}), Some(&serde_json::json!({}))).unwrap_err().contains("doesn't define render(o, i)"));
        assert!(engine.render("canvasless", &serde_json::json!({}), Some(&serde_json::json!({}))).unwrap_err().contains("render(o, i) should return a canvas or an array of canvases, got i64"));
    }
}
