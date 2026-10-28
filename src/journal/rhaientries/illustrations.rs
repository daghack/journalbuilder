//! Illustration files: rhai scripts that draw a lock's values as canvases.
//! Each illustration type is a file, `illustrations/<type>.rhai` (a
//! journal's over the library's), run in the journal's engine (so it can
//! draw), never in the lock files': locks stay values in, values out, and
//! an illustration turns their values into pictures.
//!
//! An illustration file defines:
//!
//! - `options()` (optional): `#{ NAME: default }`. Every option is
//!   optional, its type its default's.
//! - `inputs(o)`: `#{ NAME: type }`, the values it draws, typed as lock
//!   files type theirs (`"StarChart"`, `"Base27Symbol"`, `"Any"`).
//! - `render(o, i)`: a canvas, or an array of canvases (a page each, say).
//! - `fallbacks(o)` (optional): `#{ NAME: value }`, what inputs nothing's
//!   wired to draw with: the styles and glyphsets it draws in, say, so a
//!   chain can wire its own in or leave them.
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
use super::{text_styles, Canvas};

/// The illustration types: the files in a directory, loaded on first use.
pub struct IllustrationLibrary {
    sources: Sources,
    types: RefCell<HashMap<String, Rc<IllustrationType>>>,
}

/// A loaded illustration file.
pub struct IllustrationType {
    pub name: String,
    pub file: String,
    ast: Rc<AST>,
    /// Its options' defaults, in name order.
    pub defaults: Map,
    /// The file's opening comment, without its "name: ".
    pub description: String,
}

impl IllustrationLibrary {
    pub fn new(sources: Sources) -> IllustrationLibrary {
        IllustrationLibrary { sources, types: RefCell::default() }
    }

    pub fn sources(&self) -> &Sources {
        &self.sources
    }

    /// Every illustration type's name.
    pub fn names(&self) -> Vec<String> {
        self.sources.names().unwrap_or_default()
    }

    /// The illustration type `name`, compiled with `engine` (the journal's).
    pub fn load(&self, engine: &Engine, name: &str) -> Result<Rc<IllustrationType>, String> {
        if let Some(loaded) = self.types.borrow().get(name) {
            return Ok(loaded.clone());
        }
        if name.is_empty() || name.contains(['/', '\\', '.']) || !self.sources.has(name) {
            return Err(format!("no illustration named '{}'; illustrations: {}", name, self.names().join(", ")));
        }
        let file = self.sources.path(name).display().to_string();
        let ast = self.sources.compile(engine, name).map_err(|e| format!("illustration '{}': {}", name, e))?;
        let defines = |f: &str, n: usize| ast.iter_functions().any(|d| d.name == f && d.params.len() == n);
        for (function, params, what) in [("inputs", 1, "inputs(o)"), ("render", 2, "render(o, i)")] {
            if !defines(function, params) {
                return Err(format!("illustration '{}': {} doesn't define {}", name, file, what));
            }
        }
        let defaults = super::options::declared_defaults(engine, &ast, &format!("illustration '{}'", name))?;
        let source = self.sources.read(name).map_err(|e| e.to_string())?;
        let loaded = Rc::new(IllustrationType { name: name.to_string(), file, ast: Rc::new(ast), defaults, description: description(&source, name) });
        self.types.borrow_mut().insert(name.to_string(), loaded.clone());
        Ok(loaded)
    }
}

impl IllustrationType {
    /// `given` over the defaults; an option it doesn't have, or one of
    /// another type than its default, is an error.
    pub fn options(&self, given: &Map) -> Result<Map, String> {
        super::options::over_defaults(&format!("illustration '{}'", self.name), &self.defaults, given)
    }

    /// The values it draws, name -> type, for `o`.
    pub fn inputs(&self, engine: &Engine, o: &Map) -> Result<Vec<(String, String)>, String> {
        let value = engine.call_fn::<Dynamic>(&mut Scope::new(), &self.ast, "inputs", (Dynamic::from_map(o.clone()),))
            .map_err(|e| format!("illustration '{}' inputs(o): {}", self.name, e))?;
        let type_name = value.type_name();
        let map = value.try_cast::<Map>().ok_or_else(|| format!("illustration '{}': inputs(o) should return #{{ NAME: type }}, got {}", self.name, type_name))?;
        map.into_iter().map(|(name, t)| {
            let t = t.into_string().map_err(|got| format!("illustration '{}': input '{}' should have a type name, got {}", self.name, name, got))?;
            Ok((name.to_string(), t))
        }).collect()
    }

    /// What its inputs nothing's wired to take, with `o`: `fallbacks(o)`,
    /// or nothing if it doesn't define it.
    pub fn fallbacks(&self, engine: &Engine, o: &Map) -> Result<Map, String> {
        if !self.ast.iter_functions().any(|f| f.name == "fallbacks" && f.params.len() == 1) {
            return Ok(Map::new());
        }
        let value = engine.call_fn::<Dynamic>(&mut Scope::new(), &self.ast, "fallbacks", (Dynamic::from_map(o.clone()),))
            .map_err(|e| format!("illustration '{}' fallbacks(o): {}", self.name, e))?;
        let type_name = value.type_name();
        let map = value.try_cast::<Map>().ok_or_else(|| format!("illustration '{}': fallbacks(o) should return #{{ NAME: value }}, got {}", self.name, type_name))?;
        let inputs = self.inputs(engine, o)?;
        if let Some(name) = map.keys().find(|name| !inputs.iter().any(|(input, _)| input == name.as_str())) {
            let names: Vec<&str> = inputs.iter().map(|(n, _)| n.as_str()).collect();
            return Err(format!("illustration '{}': fallbacks(o) gives '{}', which isn't an input; inputs: {}", self.name, name, names.join(", ")));
        }
        Ok(map)
    }

    /// `inputs` with what's missing from them taken from `fallbacks(o)`.
    /// A TextStyle is made a whole bundle, as `text_style` gives (its
    /// style, type and paragraph whole), with what it doesn't give (a
    /// glyphset, a glyph scale, ...) its fallback's (see
    /// `text_styles::bundle_over`).
    pub fn with_fallbacks(&self, engine: &Engine, o: &Map, mut inputs: Map) -> Result<Map, String> {
        let mut fallbacks = self.fallbacks(engine, o)?;
        for (name, t) in self.inputs(engine, o)? {
            let value = match (inputs.remove(name.as_str()), fallbacks.remove(name.as_str())) {
                (None, None) => continue,
                (Some(value), None) | (None, Some(value)) => value,
                (Some(given), Some(fallback)) if t == "TextStyle" => match (given.clone().try_cast::<Map>(), fallback.try_cast::<Map>()) {
                    (Some(given), Some(fallback)) => self.bundle(&name, |what| text_styles::bundle_over(what, given, &fallback))?.into(),
                    _ => given,
                },
                (Some(given), Some(_)) => given,
            };
            let value = match value.clone().try_cast::<Map>() {
                Some(bundle) if t == "TextStyle" => self.bundle(&name, |what| text_styles::whole(what, bundle))?.into(),
                _ => value,
            };
            inputs.insert(name.into(), value);
        }
        Ok(inputs)
    }

    /// `make` a TextStyle input `name`'s bundle, its errors naming it.
    fn bundle(&self, name: &str, make: impl FnOnce(&str) -> super::RhaiResult<Map>) -> Result<Map, String> {
        make(&format!("illustration '{}' input '{}'", self.name, name)).map_err(|e| super::options::unwrap_runtime(&e))
    }

    /// The canvases it draws `inputs` (name -> value) as, with `o`.
    pub fn render(&self, engine: &Engine, o: &Map, inputs: Map) -> Result<Vec<Canvas>, String> {
        let value = engine.call_fn::<Dynamic>(&mut Scope::new(), &self.ast, "render", (Dynamic::from_map(o.clone()), Dynamic::from_map(inputs)))
            .map_err(|e| format!("illustration '{}' render(o, i): {}", self.name, e))?;
        if value.is::<Canvas>() {
            return Ok(vec![value.cast::<Canvas>()]);
        }
        let type_name = value.type_name();
        let items = value.try_cast::<Array>().ok_or_else(|| format!("illustration '{}': render(o, i) should return a canvas or an array of canvases, got {}", self.name, type_name))?;
        items.into_iter().enumerate().map(|(i, item)| {
            let type_name = item.type_name();
            item.try_cast::<Canvas>().ok_or_else(|| format!("illustration '{}': render(o, i) returned an array whose item {} is a {}, not a canvas", self.name, i, type_name))
        }).collect()
    }

    /// The inputs its file gives to preview it with, if it gives any.
    pub fn example(&self, engine: &Engine) -> Result<Option<Map>, String> {
        if !self.ast.iter_functions().any(|f| f.name == "example" && f.params.is_empty()) {
            return Ok(None);
        }
        let value = engine.call_fn::<Dynamic>(&mut Scope::new(), &self.ast, "example", ())
            .map_err(|e| format!("illustration '{}' example(): {}", self.name, e))?;
        let type_name = value.type_name();
        value.try_cast::<Map>().map(Some).ok_or_else(|| format!("illustration '{}': example() should return #{{ NAME: value }}, got {}", self.name, type_name))
    }
}

/// The opening `//` comment of a file, joined into a line, without a
/// leading "name: ".
pub(super) fn description(source: &str, name: &str) -> String {
    let lines: Vec<&str> = source.lines().map(str::trim).take_while(|l| l.starts_with("//")).map(|l| l.trim_start_matches('/').trim()).collect();
    let text = lines.join(" ");
    text.strip_prefix(&format!("{}:", name)).map(str::trim).unwrap_or(&text).to_string()
}

#[cfg(test)]
mod tests {
    use super::super::{JournalEngine, Resources};
    use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};
    use std::rc::Rc;

    fn engine_with(chains: &[(&str, &str)], illustrations: &[(&str, &str)]) -> JournalEngine {
        let overlay = |files: &[(&str, &str)]| files.iter().map(|(n, s)| (n.to_string(), s.to_string())).collect::<Vec<_>>();
        let mut resources = Resources::library();
        resources.illustrations = resources.illustrations.with_overlay(overlay(illustrations));
        resources.chains = resources.chains.with_overlay(overlay(chains));
        JournalEngine::with_resources(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).expect("glyphsets load")), resources)
    }

    fn engine() -> JournalEngine {
        engine_with(&[], &[])
    }

    #[test]
    fn every_illustration_draws_its_example() {
        let engine = engine();
        let names = engine.illustration_names();
        assert!(names.contains(&"star_pages".to_string()) && names.contains(&"maze_glyphs".to_string()), "{:?}", names);
        for name in names {
            let info = engine.describe_illustration(&name).unwrap_or_else(|e| panic!("{}: {}", name, e));
            assert!(info.example, "{} should have an example() to preview", name);
            assert!(!info.description.is_empty() && !info.inputs.is_empty(), "{}", name);
            let entry = engine.render(&name, &serde_json::json!({}), None).unwrap_or_else(|e| panic!("{}: {}", name, e));
            assert!(entry.page_count() > 0 && entry.empty_canvases().is_empty(), "{} should draw something", name);
        }
    }

    #[test]
    fn a_journal_attaches_illustrations_and_draws_a_run() {
        let script = r#"
            let lock = new_lock("m")
                .add("maze", "maze_lock", #{ input_base: 27 })
                .attach("drawn", "maze_glyphs", #{ cell: 40.0 })
                .connect("maze.generated_maze", "drawn.maze");
            let result = lock.run(#{ "maze.maze_text": symbols(27, "HELLO"), "maze.random_seed": symbols(10, "4") });
            let canvases = result.canvases("drawn");
            if canvases.len() != 1 { throw "one canvas"; }
            // Wired to an illustration, the maze is still the chain's output.
            if !("maze.generated_maze" in lock.outputs()) { throw "the output stays the chain's"; }
            if lock.illustrations()["drawn"] != "maze_glyphs" { throw "illustrations() lists it"; }
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
            (r#"lock.attach("d", "nope");"#, "attach: no illustration named 'nope'; illustrations: "),
            (r#"lock.attach("d", "maze_glyphs", #{ size: 3 });"#, "attach: illustration 'maze_glyphs' has no option 'size'; its options: cell, corridors, glyphs"),
            (r#"lock.attach("d", "maze_glyphs", #{ cell: "big" });"#, "attach: illustration 'maze_glyphs': option 'cell' should be like its default, 70.0, got big"),
            (r#"lock.attach("maze", "maze_glyphs");"#, "attach: there's a lock, illustration or source named 'maze' already"),
            (r#"lock.attach("d", "maze_glyphs").connect("maze.generated_maze", "d.grid");"#, "connect: illustration 'd' (maze_glyphs) has no input 'grid'; inputs: corridor_style, glyph_style, maze"),
            (r#"lock.attach("d", "maze_glyphs").connect("maze.text_length", "d.maze");"#, "connect: can't connect maze.text_length (LockProcessInfo) to d.maze (Maze)"),
            (r#"lock.attach("d", "maze_glyphs").connect("maze.nope", "d.maze");"#, "connect: 'maze.nope' isn't a port of the chain"),
            (r#"lock.attach("d", "maze_glyphs").run(#{ "maze.maze_text": symbols(27, "HI"), "maze.random_seed": symbols(10, "1") }).canvases("d");"#, "canvases: illustration 'd' has nothing connected to its input 'maze'"),
            (r#"lock.run(#{ "maze.maze_text": symbols(27, "HI"), "maze.random_seed": symbols(10, "1") }).canvases("d");"#, "canvases: no illustration 'd' is attached; illustrations: none"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", chain, line)).expect_err(line).to_string();
            assert!(err.contains(want), "{}\n  wanted: {}\n  got: {}", line, want, err);
        }
    }

    #[test]
    fn unwired_inputs_take_fallbacks_and_wired_ones_override_them() {
        let picky = "fn inputs(o) { #{ text: \"String\", hand: \"TextStyle\" } }\nfn fallbacks(o) { #{ hand: text_style(\"plain_hand\") } }\nfn render(o, i) { new_canvas().add(write_line(i.text, vec2(0, 30), i.hand)) }";
        let script = r#"
            let lock = new_lock("m").source("words", "content", "lorem").source("hand", "style", "plain_hand", #{ options: #{ size: 18 } })
                .attach("plain", "picky").connect("words.value", "plain.text")
                .attach("styled", "picky").connect("words.value", "styled.text").connect("hand.value", "styled.hand");
            let result = lock.run();
            journal_entry.add(new_page().add(result.canvases("plain")[0]).add(result.canvases("styled")[0]));
        "#;
        let entry = engine_with(&[], &[("picky", picky)]).build_entry_str(script);
        assert!(entry.is_ok_and(|e| e.empty_canvases().is_empty()), "both draw");
        let wrong = picky.replace("#{ hand:", "#{ pen:");
        let err = engine_with(&[], &[("picky", &wrong)]).build_entry_str(script).expect_err("a fallback for no input").to_string();
        assert!(err.contains("fallbacks(o) gives 'pen', which isn't an input; inputs: hand, text"), "{}", err);
        let engine = engine_with(&[], &[("picky", picky)]);
        assert_eq!(engine.illustration_fallbacks("picky", &serde_json::json!({})), Ok(vec!["hand".to_string()]));
    }

    const DRAWN_MAZE: &str = r#"
        // drawn_maze: a maze lock's maze, drawn.
        fn options() { #{ cell: number(30.0) } }
        fn chain(o) {
            #{
                locks: #{ maze: #{ type: "maze_lock", options: #{ input_base: 27 } } },
                connect: [],
                fixed: #{ "maze.random_seed": symbols(10, "2") },
                illustrations: #{ drawn: #{ type: "maze_glyphs", options: #{ cell: o.cell }, inputs: #{ maze: "maze.generated_maze" } } },
            }
        }
    "#;

    #[test]
    fn a_journal_uses_a_chain_file() {
        let mut engine = engine_with(&[("drawn_maze", DRAWN_MAZE)], &[]);
        let entry = engine.build_entry_str(r#"
            let c = chain("drawn_maze", #{ cell: 40 });
            if c.illustrations()["drawn"] != "maze_glyphs" { throw "the chain's illustration comes with it"; }
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
            (r#"chain("bad");"#, "chain: chains/bad.rhai: attach: illustration 'maze_glyphs' has no option 'size'"),
        ] {
            let mut engine = engine_with(&[("drawn_maze", DRAWN_MAZE), ("bad", &DRAWN_MAZE.replace("cell: o.cell", "size: 2"))], &[]);
            let err = engine.build_entry_str(script).expect_err(script).to_string();
            assert!(err.contains(want), "{}\n  wanted: {}\n  got: {}", script, want, err);
        }
    }

    const SOURCED: &str = r#"
        // sourced: content locked, and written in a style.
        fn chain(o) {
            #{
                sources: #{
                    secret: #{ type: "content", name: "lorem", base: 27 },
                    words: #{ type: "content", name: "lorem" },
                    hand: #{ type: "style", name: "plain_hand", options: #{ size: 20.0 } },
                },
                locks: #{ shift: #{ type: "shift_cipher", options: #{ base: 27, shift: 3 } } },
                connect: [["secret.value", "shift.plain_text"]],
                illustrations: #{ note: #{ type: "styled_text", options: #{ width: 600.0 }, inputs: #{ text: "words.value", style: "hand.value" } } },
            }
        }
    "#;

    #[test]
    fn chains_draw_on_sources() {
        let mut engine = engine_with(&[("sourced", SOURCED)], &[]);
        let entry = engine.build_entry_str(r#"
            let c = chain("sourced");
            assert(c.inputs().len() == 0, "a source's value is the input it feeds");
            let result = c.run();
            assert(result.text("shift.cipher_text").starts_with("ORUHP"), "the content, locked: " + result.text("shift.cipher_text"));
            let p = new_page();
            p.add(result.canvases("note")[0]);
            journal_entry.add(p);
            // A script adds sources too.
            let more = new_lock("m").source("hand", "style", "plain_hand", #{ options: #{ size: 18 } }).source("words", "content", "lorem")
                .attach("note", "styled_text").connect("words.value", "note.text").connect("hand.value", "note.style");
            journal_entry.add(new_page().add(more.run().canvases("note")[0]));
        "#);
        assert!(entry.is_ok(), "{:?}", entry.err());
        assert!(entry.unwrap().empty_canvases().is_empty());
        let library = std::rc::Rc::new(super::super::LockLibrary::new(engine.resources()));
        assert_eq!(library.check_chain("sourced"), Ok(0));
        for (chain, want) in [
            (SOURCED.replace("\"lorem\", base: 27", "\"nope\""), "there's no content named 'nope'"),
            (SOURCED.replace("type: \"style\"", "type: \"font\""), "has type 'font'; a source is content, style, glyphset or layout"),
            (SOURCED.replace("[[\"secret.value\", \"shift.plain_text\"]]", "[[\"hand.value\", \"shift.plain_text\"]]"), "can't connect hand.value (TextStyle) to shift.plain_text (Base27Symbol)"),
            (SOURCED.replace("base: 27 }", "base: 40 }"), "has base 40"),
        ] {
            let mut engine = engine_with(&[("sourced", &chain)], &[]);
            let err = engine.build_entry_str(r#"chain("sourced");"#).expect_err(want);
            assert!(err.contains(want), "wanted: {}\n  got: {}", want, err);
        }
    }

    #[test]
    fn every_chain_file_runs_its_example() {
        let library = std::rc::Rc::new(super::super::LockLibrary::new(&Resources::library()));
        let results = library.check_all_chains().unwrap();
        assert!(!results.is_empty(), "the library has chain files");
        for (name, result) in results {
            assert_eq!(result.unwrap_or_else(|e| panic!("{}", e)), 1, "{} should have an example", name);
        }
    }

    #[test]
    fn illustration_files_must_define_inputs_and_render() {
        let engine = engine_with(&[], &[("half", "fn inputs(o) { #{} }"), ("canvasless", "fn inputs(o) { #{} }\nfn render(o, i) { 3 }")]);
        assert!(engine.render("half", &serde_json::json!({}), Some(&serde_json::json!({}))).unwrap_err().contains("doesn't define render(o, i)"));
        assert!(engine.render("canvasless", &serde_json::json!({}), Some(&serde_json::json!({}))).unwrap_err().contains("render(o, i) should return a canvas or an array of canvases, got i64"));
    }
}
