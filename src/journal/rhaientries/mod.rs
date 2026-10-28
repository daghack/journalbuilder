mod art;
mod avoid;
mod book;
mod canvas;
mod content;
mod entry;
mod generate;
mod glyphs;
mod gradient;
mod layout;
mod locks;
mod illustrations;
mod manifest;
mod markdown;
mod options;
mod path;
mod random;
mod regions;
mod resources;
mod sources;
mod style;
mod text;
mod text_settings;
mod text_styles;
mod vec;

pub use sources::Sources;
pub use resources::{Resources, BUILT_IN_JOURNALS_DIR, DEFAULT_JOURNALS_DIR, DEFAULT_JOURNALS_DIRS, DEFAULT_LIBRARY_DIR, KINDS};
#[cfg(feature = "embedded-assets")]
pub use resources::embedded;
pub use locks::{LockLibrary, SourceKind};
pub use manifest::{valid_name, Journal, Manifest, MANIFEST, MANIFEST_VERSION};
pub use illustrations::IllustrationLibrary;
use text_styles::TextStyleLibrary;
/// What the web editor's lock editor asks of lock types.
pub mod lock_inspection {
    pub use super::locks::{from_json, to_json, value_from_test, value_view, OptionInfo, Ports, RunResult, TypeInfo, ValueView};
}
pub use canvas::{CanvasData, Stroke};
pub use entry::{Entry, Page};
pub use style::{BrushStyle, HandwritingStyle, Style};
pub use vec::Vec2;
use entry::check_name;
use path::TextSource;

use rhai::{Dynamic, Engine, EvalAltResult, Scope};

/// What rhai functions return: a value, or an error for the script.
pub(super) type RhaiResult<T> = Result<T, Box<EvalAltResult>>;
use super::paths::StrokePath;
use super::fonts::FontBook;

use std::cell::RefCell;
use std::rc::Rc;

/// Fail if a path has an infinite or NaN coordinate. Scaling or dividing
/// by zero in a script makes them, and they'd otherwise break the render's
/// sizing far from the line that caused them.
pub(super) fn check_finite(caller: &str, path: &StrokePath) -> Result<(), Box<EvalAltResult>> {
    if path.points_within().iter().all(|p| p.is_finite()) {
        Ok(())
    } else {
        Err(format!("{}: the path has a non-finite (infinite or NaN) coordinate", caller).into())
    }
}

/// A shared, mutable value, as rhai scripts see paths, glyphs and canvases:
/// copying one (assigning it, passing it, a `for` loop's variable) shares
/// the value, so changes through any copy, including chained calls like
/// `p.move_to(a).line_to(b)`, all reach it. `.copy()` makes an independent
/// one.
pub struct Shared<T>(Rc<RefCell<T>>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T: Clone> Shared<T> {
    pub fn new(value: T) -> Self {
        Shared(Rc::new(RefCell::new(value)))
    }
    pub fn borrow(&self) -> std::cell::Ref<'_, T> {
        self.0.borrow()
    }
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, T> {
        self.0.borrow_mut()
    }
    /// The value as it is now.
    pub fn get(&self) -> T {
        self.0.borrow().clone()
    }
    /// An independent copy, for `.copy()`.
    pub fn copy(&mut self) -> Self {
        Shared::new(self.get())
    }
    /// Change the value in place and return this same handle, so the change
    /// reaches the variable even partway through a chain.
    pub fn update(&mut self, f: impl FnOnce(&mut T)) -> Self {
        f(&mut self.0.borrow_mut());
        self.clone()
    }
}

/// A path as scripts see it (see `Shared`).
pub type Path = Shared<StrokePath>;

/// A canvas as scripts see it (see `Shared`). Pages keep copies of the
/// canvases added to them.
pub type Canvas = Shared<CanvasData>;

/// Why a journal script failed: rhai's message, with the line, and where
/// in the script it happened. An error in a function the script defines is
/// placed where it happened in that function; one in a layout or lock file
/// where the script called into it.
#[derive(Clone, Debug)]
pub struct ScriptError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl ScriptError {
    fn new(err: &EvalAltResult) -> Self {
        let mut position = err.position();
        let mut inner = err;
        // Calls to the script's own functions (no source: it's not from a
        // file) say where inside them it went wrong.
        while let EvalAltResult::ErrorInFunctionCall(_, source, next, _) = inner
            && source.is_empty() {
            inner = next;
            if !inner.position().is_none() {
                position = inner.position();
            }
        }
        ScriptError { message: err.to_string(), line: position.line(), column: position.position() }
    }
}

/// A bare rhai engine for journal, lock and layout scripts. rhai halves
/// how deeply expressions may nest in builds with debug assertions (32,
/// and 16 in functions, against 64 and 32), for fear of the bigger stack
/// frames of unoptimized code. rhai is optimized even in dev builds here
/// (Cargo.toml), so this uses release's limits: debug builds and `cargo
/// test` without --release take the scripts release builds do.
pub fn rhai_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_expr_depths(64, 32);
    engine
}

pub struct JournalEngine {
    engine: Engine,
    warnings: Rc<RefCell<Vec<String>>>,
    symdict: Rc<FontBook>,
    resources: Resources,
    illustrations: Rc<IllustrationLibrary>,
    text_styles: Rc<TextStyleLibrary>,
}

/// A JSON object (or null) as a rhai map.
fn json_map(value: &serde_json::Value, what: &str) -> Result<rhai::Map, String> {
    match value {
        serde_json::Value::Null => Ok(rhai::Map::new()),
        serde_json::Value::Object(_) => Ok(locks::from_json(value).cast::<rhai::Map>()),
        _ => Err(format!("the {} should be a JSON object", what)),
    }
}

/// An illustration type, as the lock editor's palette lists it: what it is, its
/// options' defaults, and its inputs with those options.
#[derive(serde::Serialize)]
pub struct IllustrationInfo {
    pub name: String,
    pub file: String,
    pub description: String,
    pub options: serde_json::Value,
    pub inputs: Vec<(String, String)>,
    pub example: bool,
}

/// A text style, as the editor lists it: its options' defaults, and the
/// text it previews with, if its file gives one.
#[derive(serde::Serialize)]
pub struct TextStyleInfo {
    pub name: String,
    pub description: String,
    pub options: serde_json::Value,
    pub example: Option<String>,
}

/// A layout, as the editor lists it: its options' defaults, and whether
/// its file gives an `example()` to preview it with.
#[derive(serde::Serialize)]
pub struct LayoutInfo {
    pub name: String,
    pub description: String,
    pub options: serde_json::Value,
    pub example: bool,
}

/// What a text style without an `example()` previews with.
const PREVIEW_TEXT: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";

impl JournalEngine {
    /// An engine with the built-in library's resources (`webapp/library/`).
    pub fn new(symdict: Rc<FontBook>) -> Self {
        Self::with_resources(symdict, Resources::library())
    }
    /// An engine with the built-in library compiled in, for builds with no
    /// files to read.
    #[cfg(feature = "embedded-assets")]
    pub fn embedded(symdict: Rc<FontBook>) -> Self {
        Self::with_resources(symdict, Resources::embedded())
    }
    /// An engine with `resources`: its lock, layout, illustration, chain
    /// and style files, the helpers they and entries import from `lib/`,
    /// and its content. The glyphsets are `symdict`'s
    /// (`resources.glyphset_book()`).
    pub fn with_resources(symdict: Rc<FontBook>, resources: Resources) -> Self {
        let mut engine = rhai_engine();
        // Entries, layouts, illustrations and styles `import "lib/name"`.
        engine.set_module_resolver(resources.lib.module_resolver());
        let rng = random::register(&mut engine);
        let text: TextSource = (symdict.clone(), rng.clone());

        canvas::register(&mut engine, rng.clone());
        path::register(&mut engine, text);
        style::register(&mut engine);
        vec::register(&mut engine);
        entry::register(&mut engine);

        // warn("...") reports a problem without stopping the script: it goes
        // to stderr, marked with the script line, like journalbuild's own
        // warnings.
        let warnings: Rc<RefCell<Vec<String>>> = Rc::default();
        let script_warnings = warnings.clone();
        engine.register_fn("warn", move |ctx: rhai::NativeCallContext, message: &str| {
            let warning = match ctx.call_position().line() {
                Some(line) => format!("{} (line {})", message, line),
                None => message.to_string(),
            };
            eprintln!("warning: {}", warning);
            script_warnings.borrow_mut().push(warning);
        });

        // Characters drawn with the fallback glyph so far:
        // #{ "q": #{ count: 2, tried: ["copper_lower_q", ...] }, ... }
        let missing_symdict = symdict.clone();
        engine.register_fn("missing_glyphs", move || -> rhai::Map {
            missing_symdict.missing_glyphs().into_iter().map(|(c, glyph)| {
                let mut info = rhai::Map::new();
                info.insert("count".into(), Dynamic::from(glyph.count as i64));
                info.insert("tried".into(), Dynamic::from_array(glyph.tried.into_iter().map(Dynamic::from).collect()));
                (c.to_string().into(), Dynamic::from_map(info))
            }).collect()
        });

        engine.build_type::<book::Book>();
        engine.register_fn("roman", book::roman);
        engine.register_fn("escape_markdown", book::escape_markdown);
        // assert(condition, message): stops the script with the message,
        // and the line, unless the condition holds.
        engine.register_fn("assert", |ctx: rhai::NativeCallContext, condition: bool, message: &str| -> RhaiResult<()> {
            if condition {
                Ok(())
            } else {
                Err(EvalAltResult::ErrorRuntime(format!("assertion failed: {}", message).into(), ctx.call_position()).into())
            }
        });

        art::register(&mut engine, (symdict.clone(), rng.clone()));
        avoid::register(&mut engine);
        regions::register(&mut engine);
        layout::register(&mut engine, symdict.clone(), rng.clone(), resources.layouts.clone());
        markdown::register(&mut engine);
        text_settings::register(&mut engine);
        glyphs::register(&mut engine, symdict.clone(), rng.clone());
        gradient::register(&mut engine);
        generate::register(&mut engine, rng.clone());
        text::register(&mut engine, symdict.clone(), rng.clone());
        let illustrations = Rc::new(IllustrationLibrary::new(resources.illustrations.clone()));
        locks::register(&mut engine, Rc::new(LockLibrary::new(&resources)), illustrations.clone());
        let text_styles = Rc::new(TextStyleLibrary::new(resources.styles.clone()));
        text_styles::register(&mut engine, text_styles.clone());
        content::register(&mut engine, resources.clone());

        JournalEngine {
            engine,
            warnings,
            symdict,
            resources,
            illustrations,
            text_styles,
        }
    }
    /// The resources scripts use.
    pub fn resources(&self) -> &Resources {
        &self.resources
    }
    /// The glyphsets scripts use, which note the characters they had no
    /// glyph for.
    pub fn glyphsets(&self) -> &Rc<FontBook> {
        &self.symdict
    }
    /// The name of every function scripts can call that this crate
    /// registers (not rhai's standard library), with `module::` for those
    /// in a module like `art`. Properties are `get$name` and `set$name`.
    pub fn function_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.engine.gen_fn_signatures(false).into_iter()
            .map(|signature| signature.split('(').next().unwrap_or_default().to_string())
            .collect();
        names.sort();
        names.dedup();
        names
    }
    /// Everything scripts have passed to `warn` so far.
    pub fn warnings(&self) -> Vec<String> {
        self.warnings.borrow().clone()
    }
    /// Run a journal script, returning the `journal_entry` it built. Errors
    /// are rhai's message, with the script line.
    pub fn build_entry_str(&mut self, script_str: &str) -> Result<Entry, String> {
        self.build_entry(script_str).map_err(|err| err.message)
    }
    /// `build_entry_str`, with where in the script an error happened.
    pub fn build_entry(&mut self, script_str: &str) -> Result<Entry, ScriptError> {
        let mut scope = Scope::new();
        scope.push("journal_entry", Entry::default());
        self.engine.run_with_scope(&mut scope, script_str).map_err(|err| ScriptError::new(&err))?;
        scope.get_value("journal_entry")
            .ok_or_else(|| ScriptError { message: "the script replaced journal_entry with something that isn't an entry".to_string(), line: None, column: None })
    }
    /// The illustration types' names.
    pub fn illustration_names(&self) -> Vec<String> {
        self.illustrations.names()
    }
    /// What illustration `name` is and takes.
    pub fn describe_illustration(&self, name: &str) -> Result<IllustrationInfo, String> {
        let illustration = self.illustrations.load(&self.engine, name)?;
        Ok(IllustrationInfo {
            name: illustration.name.clone(),
            file: illustration.file.clone(),
            description: illustration.description.clone(),
            options: locks::to_json(&Dynamic::from_map(illustration.defaults.clone())),
            inputs: illustration.inputs(&self.engine, &illustration.defaults)?,
            example: illustration.example(&self.engine)?.is_some(),
        })
    }
    /// Illustration `name`'s inputs with `options` (a JSON object) over its
    /// defaults.
    pub fn illustration_inputs(&self, name: &str, options: &serde_json::Value) -> Result<Vec<(String, String)>, String> {
        let illustration = self.illustrations.load(&self.engine, name)?;
        illustration.inputs(&self.engine, &illustration.options(&json_map(options, "options")?)?)
    }
    /// Which of illustration `name`'s inputs, with `options` (a JSON
    /// object) over its defaults, draw with its fallbacks when nothing's
    /// wired to them.
    pub fn illustration_fallbacks(&self, name: &str, options: &serde_json::Value) -> Result<Vec<String>, String> {
        let illustration = self.illustrations.load(&self.engine, name)?;
        let fallbacks = illustration.fallbacks(&self.engine, &illustration.options(&json_map(options, "options")?)?)?;
        Ok(fallbacks.keys().map(|k| k.to_string()).collect())
    }
    /// What illustration `name` draws, `options` (a JSON object) over its
    /// defaults, of `inputs` (JSON, name -> value as lock files see it), or
    /// of its own `example()` if `None`: an entry with a page for each
    /// canvas.
    pub fn render(&self, name: &str, options: &serde_json::Value, inputs: Option<&serde_json::Value>) -> Result<Entry, String> {
        let illustration = self.illustrations.load(&self.engine, name)?;
        let options = illustration.options(&json_map(options, "options")?)?;
        let inputs = match inputs {
            Some(inputs) => self.with_sources(json_map(inputs, "inputs")?)?,
            None => illustration.example(&self.engine)?.ok_or_else(|| format!("illustration '{}' has no example() to draw", name))?,
        };
        let inputs = illustration.with_fallbacks(&self.engine, &options, inputs)?;
        let mut entry = Entry::default();
        for canvas in illustration.render(&self.engine, &options, inputs)? {
            let mut page = Page::default();
            page.add_canvas(canvas).map_err(|e| e.to_string())?;
            entry.add_page(page).map_err(|e| e.to_string())?;
        }
        Ok(entry)
    }
    /// The text styles' names.
    pub fn text_style_names(&self) -> Vec<String> {
        self.text_styles.names()
    }
    /// What text style `name` takes, and the text it previews with.
    pub fn describe_text_style(&self, name: &str) -> Result<TextStyleInfo, String> {
        let file = self.text_styles.load(&self.engine, name)?;
        Ok(TextStyleInfo {
            name: name.to_string(),
            description: file.description.clone(),
            options: locks::to_json(&Dynamic::from_map(file.defaults.clone())),
            example: file.example(&self.engine)?,
        })
    }
    /// Text style `name` with `options` (a JSON object) over its defaults,
    /// writing its `example()` text (or else a line of lorem ipsum) in a
    /// block `width` wide: a page.
    pub fn preview_text_style(&mut self, name: &str, options: &serde_json::Value, width: f64) -> Result<Entry, String> {
        let text = self.describe_text_style(name)?.example.unwrap_or_else(|| PREVIEW_TEXT.to_string());
        let mut scope = Scope::new();
        scope.push("journal_entry", Entry::default());
        scope.push_constant("NAME", name.to_string());
        scope.push_constant("OPTIONS", json_map(options, "options")?);
        scope.push_constant("TEXT", text);
        scope.push_constant("WIDTH", width);
        self.run_preview(scope, r#"
            let style = text_style(NAME, OPTIONS);
            let size = style.type.glyph_scale ?? 30.0;
            let c = new_canvas();
            c.add(write_block(TEXT, vec2(size, size * 2), WIDTH - size * 2, style).canvas);
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "#)
    }
    /// The layouts' names.
    pub fn layout_names(&self) -> Vec<String> {
        self.resources.layouts.names().unwrap_or_default()
    }
    /// What layout `name` takes, and whether it previews.
    pub fn describe_layout(&self, name: &str) -> Result<LayoutInfo, String> {
        let (defaults, description, example) = layout::layout_info(&self.engine, &self.resources.layouts, name)?;
        Ok(LayoutInfo { name: name.to_string(), description, options: locks::to_json(&Dynamic::from_map(defaults)), example })
    }
    /// Layout `name` with `options` (a JSON object) over its defaults,
    /// typesetting what its `example()` gives: its pages.
    pub fn preview_layout(&mut self, name: &str, options: &serde_json::Value) -> Result<Entry, String> {
        let example = layout::layout_example(&self.engine, &self.resources.layouts, name)?
            .ok_or_else(|| format!("layout '{}' has no example() to preview", name))?;
        let mut scope = Scope::new();
        scope.push("journal_entry", Entry::default());
        scope.push_constant("NAME", name.to_string());
        scope.push_constant("OPTIONS", json_map(options, "options")?);
        scope.push_constant("EXAMPLE", example);
        self.run_preview(scope, "journal_entry.add(layout(NAME, OPTIONS).typeset(EXAMPLE));")
    }
    fn run_preview(&mut self, mut scope: Scope, script: &str) -> Result<Entry, String> {
        self.engine.run_with_scope(&mut scope, script).map_err(|err| ScriptError::new(&err).message)?;
        scope.get_value("journal_entry").ok_or_else(|| "the preview lost its entry".to_string())
    }
    /// `inputs` with each `#{ "$source": #{ type, name, ... } }` (a chain
    /// source, as a chain file writes it) made into what it gives an
    /// illustration: as the editor previews an illustration a source feeds.
    fn with_sources(&self, inputs: rhai::Map) -> Result<rhai::Map, String> {
        let library = LockLibrary::new(&self.resources);
        inputs.into_iter().map(|(name, value)| {
            let Some(spec) = value.clone().try_cast::<rhai::Map>().and_then(|map| map.get("$source").cloned()) else {
                return Ok((name, value));
            };
            let mut sources = rhai::Map::new();
            sources.insert(name.clone(), spec);
            let mut chain = rhai::Map::new();
            chain.insert("sources".into(), Dynamic::from_map(sources));
            let source = locks::parse_sources(&chain)?.remove(0);
            let made = match source.kind {
                SourceKind::Content => library.source_value(&source)?.data,
                kind => {
                    let mut scope = Scope::new();
                    scope.push_constant("NAME", source.name.clone());
                    scope.push_constant("OPTIONS", source.options.clone());
                    let script = match kind {
                        SourceKind::Style => "text_style(NAME, OPTIONS)",
                        SourceKind::Glyphset => "glyphset(NAME)",
                        _ => "layout(NAME, OPTIONS)",
                    };
                    self.engine.eval_with_scope::<Dynamic>(&mut scope, script).map_err(|e| format!("input '{}': {}", name, ScriptError::new(&e).message))?
                }
            };
            Ok((name, made))
        }).collect()
    }
    /// Call `f` with what the script passes to `print` and `debug`.
    pub fn on_print(&mut self, f: impl Fn(&str) + 'static) {
        let f = Rc::new(f);
        let debug = f.clone();
        self.engine.on_print(move |text| f(text));
        self.engine.on_debug(move |text, _, _| debug(text));
    }
    /// Call `f` as a script runs, with how many operations it's done so
    /// far: to show that it's working.
    pub fn on_progress(&mut self, f: impl Fn(u64) + 'static) {
        self.engine.on_progress(move |ops| {
            f(ops);
            None
        });
    }
    pub fn build_entry_file(&mut self, rhai_filename: &str) -> Result<Entry, String> {
        let file_contents = std::fs::read_to_string(rhai_filename)
            .map_err(|err| format!("couldn't read {}: {}", rhai_filename, err))?;
        self.build_entry_str(&file_contents).map_err(|err| format!("{}: {}", rhai_filename, err))
    }
}

#[cfg(test)]
mod api_shape_tests;
#[cfg(test)]
mod illustration_style_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::canvas::Canvas as JournalCanvas;
    use crate::journal::canvas::PressureProfile;
    use crate::journal::publish;

    fn engine() -> JournalEngine {
        let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_GLYPHSETS).expect("Failed to load fonts.json");
        JournalEngine::new(Rc::new(symd))
    }

    const BRUSH: &str = r#"let brush = new_brush_style("black", "Calligraphy", 1.0, #{ min_pressure: 0.25, max_pressure: 0.5 }); let hand = new_handwriting_style();"#;

    #[test]
    fn script_errors_say_where_in_the_script() {
        // In the script's own function: where it went wrong in there.
        let err = engine().build_entry("let a = 1;\nfn f() {\n    let x = [1][5];\n}\nf();").unwrap_err();
        assert_eq!((err.line, err.column), (Some(3), Some(17)), "{}", err.message);
        assert!(err.message.contains("line 5"), "{}", err.message);
        // In a layout file: where the script called it.
        let err = engine().build_entry("let a = 1;\nlayout(\"no_such_layout\");").unwrap_err();
        assert_eq!(err.line, Some(2), "{}", err.message);
        let err = engine().build_entry("let a = ;").unwrap_err();
        assert_eq!(err.line, Some(1), "{}", err.message);
    }

    #[test]
    fn print_and_debug_go_to_on_print() {
        let printed: Rc<RefCell<Vec<String>>> = Rc::default();
        let mut engine = engine();
        let sink = printed.clone();
        engine.on_print(move |text| sink.borrow_mut().push(text.to_string()));
        engine.build_entry("print(\"hello\"); debug(2);").unwrap();
        assert_eq!(*printed.borrow(), ["hello", "2"]);
    }

    #[test]
    fn a_page_svg_lays_its_canvases_together() {
        let entry = engine().build_entry_str(&format!(r#"{}
            brush.color = "red";
            let style = new_style(brush, hand);
            let a = new_canvas();
            a.add(new_path().move_to(vec2(0.0, 0.0)).line_to(vec2(10.0, 0.0)), style);
            let b = new_canvas();
            b.add(new_path().move_to(vec2(0.0, 20.0)).line_to(vec2(30.0, 20.0)), style);
            let page = new_page();
            page.add(a);
            page.add(b);
            journal_entry.add(page);
            journal_entry.add(new_page());
        "#, BRUSH)).unwrap();
        let (_, page) = entry.selected_pages(None).next().unwrap();
        let (svg, [x, y, w, h]) = publish::page_svg(page, "page_0").unwrap();
        assert_eq!(svg.matches("<svg").count(), 3, "{}", svg);
        assert_eq!(svg.matches("<path").count(), 2, "{}", svg);
        assert!(x <= 0.0 && y <= 0.0 && x + w >= 30.0 && y + h >= 20.0, "{:?}", [x, y, w, h]);
        let (_, empty) = entry.selected_pages(None).nth(1).unwrap();
        assert!(publish::page_svg(empty, "page_1").is_none());
    }

    #[test]
    fn brush_styles_take_css_colors_and_gradients() {
        let script = format!(r#"{}
            brush.color = "darkred";
            brush.color = "hsl(160, 50%, 40%, 0.8)";
            let a = new_brush_style(new_gradient(["red", "blue"]), "Ink Pen", 1.0);
        "#, BRUSH);
        assert!(engine().build_entry_str(&script).is_ok());
        let err = engine().build_entry_str(&format!("{}
brush.color = \"darkredd\";", BRUSH)).expect_err("bad color should fail").to_string();
        assert!(err.contains("darkredd") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn brush_styles_check_their_brush_with_line() {
        let err = engine().build_entry_str("let ok = new_brush_style(\"black\", \"Ink Pen\", 1.0);\nlet bad = new_brush_style(\"black\", \"Crayon\", 1.0);")
            .expect_err("unknown brush should fail").to_string();
        assert!(err.contains("unknown brush 'Crayon'"), "{}", err);
        assert!(err.contains("Calligraphy"), "error should list available brushes: {}", err);
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn brush_names_lists_what_brush_styles_take() {
        let names: rhai::Array = engine().engine.eval("brush_names()").unwrap();
        let names: Vec<String> = names.into_iter().map(|n| n.into_string().unwrap()).collect();
        assert!(names.contains(&"Calligraphy".to_string()) && names.contains(&"Ink Pen".to_string()), "{:?}", names);
        assert!(names.windows(2).all(|w| w[0] < w[1]));
        for name in &names {
            engine().engine.run(&format!("new_brush_style(\"black\", {:?}, 1.0);", name)).unwrap();
        }
    }

    #[test]
    fn brush_styles_reject_bad_pressures_and_sizes() {
        for (args, want) in [
            ("1.0, #{ min_pressure: 0.8, max_pressure: 0.2 }", "'min_pressure' (0.8) can't be above 'max_pressure' (0.2)"),
            ("1.0, #{ min_pressure: -0.1 }", "'min_pressure' must be from 0 to 1, got -0.1"),
            ("1.0, #{ max_pressure: 1.5 }", "'max_pressure' must be from 0 to 1, got 1.5"),
            ("0.0", "'brush_size' must be positive, got 0"),
            ("-1.0", "'brush_size' must be positive"),
        ] {
            let err = engine().build_entry_str(&format!("let ok = 1;\nnew_brush_style(\"black\", \"Ink Pen\", {});", args))
                .expect_err(args).to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
        // Equal pressures (a fixed pressure) are fine, and pressures set
        // together are checked together.
        assert!(engine().build_entry_str("new_brush_style(\"black\", \"Calligraphy\", 0.8, #{ min_pressure: 0.9, max_pressure: 1.0 }); journal_entry.add(new_page());").is_ok());
    }

    #[test]
    fn styles_need_a_brush_style_and_a_handwriting_style() {
        let err = engine().build_entry_str(r#"let s = new_style(1.5, "black");"#)
            .expect_err("brushless style should fail").to_string();
        assert!(err.contains("new_style"), "{}", err);
        // Their settings, flattened, read and set as properties.
        let ok: bool = engine().engine.eval(r#"
            let s = default_style();
            s.color = "red";
            s.passes = 3;
            s.brush_size == 1.2 && s.brush == "Ink Pen" && s.passes == 3 && s.color == "red" && s.max_pressure == 0.7
        "#).unwrap();
        assert!(ok);
    }

    fn path_string(entry_script: &str) -> String {
        let engine = engine();
        let path: Path = engine.engine.eval(entry_script).expect("script should evaluate");
        path.get().to_string()
    }

    #[test]
    fn path_functions_mutate_in_place_and_chain() {
        // Statement form mutates the variable...
        let statements = path_string(r#"
            let p = new_path();
            p.move_to(vec2(0.0, 0.0));
            p.line_to(vec2(1.0, 0.0));
            p.scale_by(2.0);
            p.translate_by(vec2(1.0, 1.0));
            p
        "#);
        // ...and chaining (including on temporaries) gives the same result.
        let chained = path_string(r#"
            new_path().move_to(vec2(0.0, 0.0)).line_to(vec2(1.0, 0.0))
                .scale_by(2.0).translate_by(vec2(1.0, 1.0))
        "#);
        let reassigned = path_string(r#"
            let p = new_path();
            p = p.move_to(vec2(0.0, 0.0));
            p = p.line_to(vec2(1.0, 0.0));
            p = p.scale_by(2.0);
            p = p.translate_by(vec2(1.0, 1.0));
            p
        "#);
        // Chains on a variable change the variable by every call.
        let chained_variable = path_string(r#"
            let p = new_path();
            p.move_to(vec2(0.0, 0.0)).line_to(vec2(1.0, 0.0)).scale_by(2.0).translate_by(vec2(1.0, 1.0));
            p
        "#);
        assert_eq!(statements, "M 1 1 L 3 1 ");
        assert_eq!(chained, statements);
        assert_eq!(reassigned, statements);
        assert_eq!(chained_variable, statements);
    }

    #[test]
    fn copies_share_until_copied_and_adding_takes_a_snapshot() {
        let script = r#"
            let a = new_line(vec2(0.0, 0.0), vec2(1.0, 0.0));
            let b = a;
            b.translate_by(vec2(5.0, 0.0));
            if a.to_string() != b.to_string() { throw "assignment should share"; }
            let c = a.copy();
            c.translate_by(vec2(5.0, 0.0));
            if a.to_string() == c.to_string() { throw "copy() should be independent"; }

            let style = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let canvas = new_canvas();
            canvas.add(a, style);
            a.translate_by(vec2(100.0, 0.0));   // after adding: the canvas keeps its copy
            let page = new_page();
            page.add(canvas);
            canvas.add(a, style);        // after adding: the page keeps its copy
            journal_entry.add(page);

            // Loop variables share too, so loops can change array elements.
            let gs = glyphset("loche_upper").glyphs("ab");
            for g in gs { g.scale = 7.0; }
            if gs[0].scale != 7.0 || gs[1].scale != 7.0 { throw "loop should change the glyphs"; }
            // A canvas can add itself.
            canvas.add(canvas);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let strokes = &entry.pages[0].canvases()[0].strokes;
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].path.to_string(), "M 5 0 L 6 0 ");
    }

    #[test]
    fn shape_and_curve_functions_build_paths() {
        assert_eq!(path_string("new_line(vec2(0.0, 0.0), vec2(2.0, 3.0))"), "M 0 0 L 2 3 ");
        assert_eq!(path_string("new_rectangle(vec2(0.0, 0.0), 2.0, 4.0)"), "M -1 -2 L 1 -2 L 1 2 L -1 2 Z ");
        assert_eq!(
            path_string("new_path().move_to(vec2(0.0, 0.0)).quad_to(vec2(1.0, 1.0), vec2(2.0, 0.0)).cubic_to(vec2(3.0, 1.0), vec2(4.0, 1.0), vec2(5.0, 0.0)).close()"),
            "M 0 0 Q 1 1 2 0 C 3 1 4 1 5 0 Z ",
        );
        // A circle is four cubic quarter-arcs.
        assert_eq!(path_string("new_circle(vec2(0.0, 0.0), 1.0)").matches('C').count(), 4);
    }

    #[test]
    fn sized_pages_are_their_size_whatever_is_drawn() {
        let entry = engine().build_entry_str(r#"
            let c = new_canvas();
            c.add(new_line(vec2(10, 10), vec2(500, 10)), new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style()));
            let p = new_page();
            p.name = "sized";
            p.size = vec2(200, 100);
            p.add(c);
            if p.size.x != 200.0 || p.size.y != 100.0 { throw "the size reads back"; }
            journal_entry.add(p);
            journal_entry.add(p.split(#{ columns: 2, rows: 1 }));
            let q = new_page();
            q.size = vec2(10, 10);
            q.size = ();
            if q.size != () { throw "() clears the size"; }
        "#).expect("script should run");
        let whole = parry2d::bounding_volume::Aabb::new(parry2d::math::Vec2::ZERO, parry2d::math::Vec2::new(200.0, 100.0));
        let (_, canvas) = entry.pages[0].rendered_canvases().next().unwrap();
        assert_eq!(canvas.frame().unwrap().page, whole, "the page is its size, though the line runs past it");
        assert!(!canvas.frame().unwrap().is_tile());
        // Its tiles divide the size, not the strokes.
        let (_, tile) = entry.pages[2].rendered_canvases().next().unwrap();
        assert_eq!((tile.frame().unwrap().page, tile.frame().unwrap().column), (whole, 1));
        for (script, want) in [("p.size = 3;", "page.size: expected a vec"), ("p.size = vec2(0, 10);", "at least 1")] {
            let err = engine().build_entry_str(&format!("let p = new_page();\n{}", script)).expect_err(script);
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn missing_glyphs_are_drawn_and_reported() {
        let script = r#"
            let style = new_style(new_brush_style("black", "Ink Pen", 1.0, #{ min_pressure: 0.45, max_pressure: 1 }), new_handwriting_style());
            let font = glyphset("copper");
            let canvas = new_line(vec2(0.0, 0.0), vec2(400.0, 0.0)).fit_text("a~b~", #{ style: style, type: #{ glyphset: font } });
            let missing = missing_glyphs();
            if missing["~"].count != 2 { throw "expected ~ twice, got " + missing; }
            // The font, then its fallback.
            if missing["~"].tried != ["copper '~'", "loche_upper '~'"] { throw "unexpected tried list: " + missing["~"].tried; }
        "#;
        engine().build_entry_str(script).expect("script should run");
    }

    /// The svg of the first page of `script`'s entry, built by a fresh engine.
    fn page_svg(script: &str) -> String {
        let entry = engine().build_entry_str(script).expect("script should run");
        let canvas: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        canvas.to_svg(None, 0)
    }

    #[test]
    fn glyph_variants_follow_the_seed() {
        // copper_lower has several variants of most letters, so the text's
        // glyphs depend on which variant each pick lands on.
        let script = |seed: i64| format!(r#"{}
            seed({});
            let style = new_style(brush, hand);
            let font = glyphset("copper");
            let path = new_path().move_to(vec2(0.0, 0.0)).line_to(vec2(2000.0, 0.0));
            let page = new_page();
            page.add(path.fit_text("a banana and an aardvark", #{{ style: style, type: #{{ glyphset: font }} }}));
            journal_entry.add(page);
        "#, BRUSH, seed);
        // Each engine loads its own dictionary, so this also checks the pick
        // doesn't depend on hash map order.
        assert_eq!(page_svg(&script(3)), page_svg(&script(3)));
        assert_ne!(page_svg(&script(3)), page_svg(&script(4)));
    }

    const NAMED: &str = r#"
        let style = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
        let ring = new_canvas();
        ring.add(new_circle(vec2(0.0, 0.0), 10.0), style);
        let named = ring.copy();
        named.name = "ring";
        let maze = new_page();
        maze.name = "maze";
        maze.add(named);
        maze.add(ring);
        journal_entry.add(new_page());
        journal_entry.add(maze);
    "#;

    #[test]
    fn pages_and_canvases_are_labelled_by_name_or_index() {
        let entry = engine().build_entry_str(NAMED).expect("script should run");
        let labels: Vec<String> = entry.pages.iter().enumerate().map(|(i, p)| p.label(i)).collect();
        assert_eq!(labels, ["page_0", "maze"]);
        let canvases: Vec<String> = entry.pages[1].canvases().iter().enumerate().map(|(i, c)| c.label(i)).collect();
        assert_eq!(canvases, ["ring", "1"]);

        let dir = std::env::temp_dir().join(format!("journal_names_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        publish::svgs(&entry, dir.to_str().unwrap(), 0.1, Some(&[1])).expect("publish should succeed");
        assert!(dir.join("maze_ring.png").is_file() && dir.join("maze_1.png").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn text_that_cannot_fit_its_path_is_an_error() {
        let setup = r#"
            let s = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let font = glyphset("copper");
            let short = new_line(vec2(0.0, 0.0), vec2(10.0, 0.0));
        "#;
        for (script, want) in [
            ("short.fill_with_text(\"abc\", #{ style: s, type: new_type_style(font, 50.0) });", "fill_with_text: glyph 1 of 3 doesn't fit"),
            ("short.fill_with_text(\"abc\", #{ style: s, type: new_type_style(font, 0.0) });", "'glyph_scale' must be positive"),
            ("new_line(vec2(0.0, 0.0), vec2(0.01, 0.0)).fit_text(\"a long message\", #{ style: s, type: #{ glyphset: font } });", "fit_text: the text doesn't fit"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", setup, script)).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
        // A path that fits a few glyphs at a time still gets the whole message.
        let parts: rhai::Array = engine().engine.eval(&format!("{}\nnew_line(vec2(0.0, 0.0), vec2(60.0, 0.0)).fill_with_text(\"abcdefghijklmnopqrstuvwxyz\", #{{ style: s, type: new_type_style(font, 10.0) }})", setup)).unwrap();
        assert!(parts.len() > 1);
    }

    /// Where each glyph of `msg` lands along a straight line, at glyph
    /// scale 20, `tracking` glyph scales apart.
    fn glyph_xs(msg: &str, tracking: f32) -> Vec<f32> {
        let script = format!(r#"
            let s = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let font = glyphset("loche_upper");
            let line = new_line(vec2(0.0, 0.0), vec2(2000.0, 0.0));
            let page = new_page();
            for c in line.fill_with_text("{}", #{{ style: s, type: new_type_style(font, 20.0, #{{ glyph_tracking: {} }}) }}) {{ page.add(c); }}
            journal_entry.add(page);
        "#, msg, tracking);
        let entry = engine().build_entry_str(&script).expect("script should run");
        entry.pages[0].canvases()[0].strokes.iter().map(|stroke| {
            let xs: Vec<f32> = stroke.path.points_within().iter().map(|p| p.x).collect();
            xs.iter().sum::<f32>() / xs.len() as f32
        }).collect()
    }

    #[test]
    fn text_sits_at_glyph_widths_plus_spacing() {
        // Narrow glyphs used to be pushed a whole glyph unit apart, however
        // narrow they were.
        let tight = glyph_xs("iiii", 0.0);
        let gaps: Vec<f32> = tight.windows(2).map(|w| w[1] - w[0]).collect();
        let i_width = {
            let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_GLYPHSETS).unwrap();
            let i = symd.font("loche_upper").unwrap().variants("I").unwrap().remove(0);
            i.hull_aabb().extents().x * 20.0 / 2.0
        };
        assert!(gaps.iter().all(|g| (g - i_width).abs() < 0.5), "gaps {:?}, i is {} wide", gaps, i_width);
        // glyph_tracking adds a fraction of the glyph scale between glyphs.
        let spaced = glyph_xs("iiii", 0.5);
        let spaced_gaps: Vec<f32> = spaced.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(spaced_gaps.iter().zip(&gaps).all(|(s, g)| (s - g - 10.0).abs() < 0.5), "{:?} vs {:?}", spaced_gaps, gaps);
        let err = engine().build_entry_str("new_line(vec2(0.0, 0.0), vec2(9.0, 0.0)).fill_with_text(\"a\", #{ type: new_type_style(glyphset(\"loche_upper\"), 2.0), spacng: 1 });")
            .expect_err("unknown option").to_string();
        assert!(err.contains("unknown option 'spacng'"), "{}", err);
    }

    #[test]
    fn text_on_a_looping_path_still_avoids_itself() {
        // A small circle can't hold the message twice over: glyphs coming
        // round again must not land on the first ones.
        let script = r#"
            let s = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let font = glyphset("loche_upper");
            let ring = new_circle(vec2(0.0, 0.0), 30.0);
            ring.fill_with_text("abcdefghijklmnopqrstuvwxyz", #{ style: s, type: new_type_style(font, 20.0) }).len()
        "#;
        let copies: i64 = engine().engine.eval(script).expect("should run");
        assert!(copies > 1, "the circle should need more than one copy, got {}", copies);
    }

    #[test]
    fn text_along_a_path_keeps_clear_of_what_it_avoids() {
        // A line of text crossed by a post at x = 300: no glyph's outline
        // comes within the clearance (2) of it, and with the words layout no
        // word is split around it.
        let script = |layout: &str| format!(r#"
            let s = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let font = glyphset("loche_upper");
            let post = new_line(vec2(300.0, -100.0), vec2(300.0, 100.0));
            let line = new_line(vec2(0.0, 0.0), vec2(2000.0, 0.0));
            let page = new_page();
            page.add(line.fill_with_text("abcdefghijklmnopqrstuvwxyz the quick brown fox jumps over the lazy dog", #{{ style: s, type: new_type_style(font, 20.0), paragraph: #{{ clearance: 2.0, layout: "{}" }}, avoid: post }})[0]);
            journal_entry.add(page);
        "#, layout);
        for layout in ["flat", "words"] {
            let entry = engine().build_entry_str(&script(layout)).expect("script should run");
            let strokes = &entry.pages[0].canvases()[0].strokes;
            // Strokes are drawn with their hulls inside the glyph's, so check
            // stroke points: the closest may be just under 2 from the hull.
            let mut left = 0;
            for stroke in strokes {
                let xs: Vec<f32> = stroke.path.points_within().iter().map(|p| p.x).collect();
                let (min, max) = xs.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
                assert!(max < 298.5 || min > 301.5, "{}: a glyph spans {}..{}, over the post", layout, min, max);
                if max < 300.0 { left += 1; }
            }
            assert!(left > 0 && left < strokes.len(), "{}: text should be on both sides of the post", layout);
        }
        // "words" moves the whole word that would straddle the post past
        // it, where "flat" sets what fits before it: fewer strokes before.
        let words = engine().build_entry_str(&script("words")).unwrap();
        let flat = engine().build_entry_str(&script("flat")).unwrap();
        let before = |entry: &Entry| entry.pages[0].canvases()[0].strokes.iter()
            .filter(|s| s.path.points_within().iter().all(|p| p.x < 300.0)).count();
        assert!(before(&words) < before(&flat), "{} < {}", before(&words), before(&flat));
    }

    const SPLIT: &str = r##"
        let hand = new_handwriting_style();
        let c = new_canvas();
        for i in 0..8 {
            c.add(new_line(vec2(i * 30.0, 0.0), vec2(300.0 - i * 20.0, 200.0)), new_style(new_brush_style("#203040", "Ink Pen", 3.0, #{ min_pressure: 0.6, max_pressure: 1.0 }), hand));
        }
        c.add(new_circle(vec2(150.0, 100.0), 90.0), new_style(new_brush_style("darkred", "Ink Pen", 3.0, #{ min_pressure: 0.9, max_pressure: 1.0 }), hand));
        let other = new_canvas();
        other.name = "dot";
        other.add(new_circle(vec2(20.0, 20.0), 5.0), new_style(new_brush_style("black", "Ink Pen", 3.0), hand));
        let p = new_page();
        p.name = "big";
        p.svg_style = #{ roughness: 1.5, noise_amplitude: 1.0 };
        p.add(c);
        p.add(other);
        p.add(new_canvas());
        journal_entry.add(p);
    "##;

    fn png_pixels(file: &std::path::Path) -> image::RgbaImage {
        image::open(file).unwrap_or_else(|e| panic!("{}: {}", file.display(), e)).to_rgba8()
    }

    #[test]
    fn split_pages_render_tiles_that_make_the_whole() {
        let script = format!("{}\njournal_entry.add(p.split(#{{ columns: 3, rows: 2 }}));", SPLIT);
        let entry = engine().build_entry_str(&script).expect("script should run");
        let labels: Vec<String> = entry.pages.iter().enumerate().map(|(i, p)| p.label(i)).collect();
        assert_eq!(labels, ["big", "big_0_0", "big_0_1", "big_0_2", "big_1_0", "big_1_1", "big_1_2"]);
        // Tiles keep the canvases' names, and not the empty canvas.
        let names: Vec<String> = entry.pages[1].canvases().iter().enumerate().map(|(i, c)| c.label(i)).collect();
        assert_eq!(names, ["0", "dot"]);
        assert!(entry.empty_canvases() == ["big/2"], "{:?}", entry.empty_canvases());

        let dir = std::env::temp_dir().join(format!("journal_split_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        publish::svgs(&entry, dir.to_str().unwrap(), 0.75, None).expect("publish should succeed");
        let whole = png_pixels(&dir.join("big_0.png"));
        let tiles: Vec<image::RgbaImage> = (0..6).map(|i| png_pixels(&dir.join(format!("big_{}_{}_0.png", i / 3, i % 3)))).collect();
        // Side by side, in order, they make the whole canvas's render.
        let widths: u32 = tiles[..3].iter().map(|t| t.width()).sum();
        let heights: u32 = [&tiles[0], &tiles[3]].iter().map(|t| t.height()).sum();
        assert_eq!((widths, heights), whole.dimensions());
        let mut worst = 0;
        let (mut y0, mut diff_sum) = (0, 0u64);
        for row in 0..2 {
            let mut x0 = 0;
            for column in 0..3 {
                let tile = &tiles[row * 3 + column];
                assert!(tile.height() == tiles[row * 3].height(), "tiles in a row share a height");
                for (x, y, p) in tile.enumerate_pixels() {
                    let q = whole.get_pixel(x0 + x, y0 + y);
                    // Compare premultiplied, so transparent pixels' colors don't count.
                    for c in 0..4 {
                        let (a, b) = (p[c] as i32 * p[3] as i32 / 255, q[c] as i32 * q[3] as i32 / 255);
                        let d = (a - b).abs();
                        worst = worst.max(d);
                        diff_sum += d as u64;
                    }
                }
                x0 += tile.width();
            }
            y0 += tiles[row * 3].height();
        }
        // Anti-aliasing can differ a hair where a stroke's outline crosses
        // a seam; nowhere more.
        assert!(worst <= 12, "a pixel differs by {}", worst);
        assert!((diff_sum as f64) / (whole.len() as f64) < 0.01, "mean difference {}", diff_sum as f64 / whole.len() as f64);
        // Every canvas's tiles cover the whole page, the same pixels each.
        assert_eq!(png_pixels(&dir.join("big_1_2_dot.png")).dimensions(), tiles[5].dimensions());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn split_takes_tile_sizes_and_checks_its_options() {
        let script = format!("{}\nlet tiles = p.split(#{{ tile_width: 100, rows: 2 }});\nif tiles.len() != 8 {{ throw \"expected 4 x 2, got \" + tiles.len(); }}\nif tiles[5].tile.column != 1 || tiles[5].tile.rows != 2 {{ throw \"bad tile: \" + tiles[5].tile; }}\nif p.tile != () {{ throw \"the page isn't a tile\"; }}\njournal_entry.add(tiles);", SPLIT);
        let entry = engine().build_entry_str(&script).expect("script should run");
        // 100-unit tiles: the page's ~310 units grow to 400, evenly.
        let canvas: JournalCanvas = entry.pages[1].canvases()[0].clone().into();
        let frame = canvas.frame().expect("tiles have frames");
        assert!((frame.page.extents().x - 400.0).abs() < 1e-3, "{:?}", frame.page);
        let whole = entry.pages[0].render_canvas(&entry.pages[0].canvases()[0]).unwrap().bounds();
        assert!((frame.page.center().x - whole.center().x).abs() < 1e-3);
        for (script, want) in [
            ("let q = new_page();\nq.split(#{ columns: 2, rows: 2 });", "name the page first"),
            ("let q = new_page(); q.name = \"q\";\nq.split(#{ columns: 2, rows: 2 });", "nothing drawn on it"),
            ("p.split(#{ columns: 0, rows: 2 });", "'columns' should be a whole number from 1 to 1000"),
            ("p.split(#{ columns: 2, tile_width: 10 });", "give 'columns' or 'tile_width', not both"),
            ("p.split(#{ tile_height: -1 });", "'tile_height' must be positive"),
            ("p.split(#{ colums: 2 });", "unknown option 'colums'"),
            ("p.split(#{ columns: 2, rows: 2 })[0].split(#{ columns: 2, rows: 2 });", "already a tile"),
            ("journal_entry.add([p.split(#{ columns: 2, rows: 2 })[0], 3]);", "journal_entry.add: item 1 is a i64"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", SPLIT, script)).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn tiles_keep_their_strokes_seeds_from_the_whole_page() {
        let script = format!("{}\njournal_entry.add(p.split(#{{ columns: 2, rows: 2 }}));", SPLIT);
        let entry = engine().build_entry_str(&script).expect("script should run");
        let whole: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        let tile: JournalCanvas = entry.pages[4].canvases()[0].clone().into();
        assert_eq!(whole.stroke_seeds(), tile.stroke_seeds());
        assert!(tile.strokes().iter().all(|s| s.seed.is_some()));
    }

    #[test]
    fn degenerate_canvases_render_or_are_skipped() {
        let script = r#"
            let s = new_style(new_brush_style("black", "Ink Pen", 1.0, #{ min_pressure: 0, max_pressure: 0 }), new_handwriting_style());
            let line = new_canvas();
            line.add(new_line(vec2(0.0, 0.0), vec2(0.0, 100.0)), s);
            let p = new_page();
            p.add(line);
            p.add(new_canvas());
            journal_entry.add(p);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        assert_eq!(entry.empty_canvases(), vec!["page_0/1".to_string()]);
        let line: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        assert_eq!((line.render_extents().x, line.render_extents().y), (1.0, 100.0));
        let dir = std::env::temp_dir().join(format!("journal_degenerate_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        publish::svgs(&entry, dir.to_str().unwrap(), 1.0, None).expect("publish should succeed");
        assert!(dir.join("page_0_0.png").is_file() && !dir.join("page_0_1.png").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn non_finite_coordinates_fail_where_they_enter() {
        let setup = r#"
            let s = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let c = new_canvas();
        "#;
        for (script, want) in [
            ("vec2(1.0 / 0.0, 0.0);", "vec2: x must be finite"),
            ("vec2(0.0, 0.0 / 0.0);", "vec2: y must be finite"),
            ("new_circle(vec2(0.0, 0.0), 5.0).scale_by(0.0 / 0.0);", "scale_by: the scale must be finite"),
            // Finite, but past what paths hold.
            ("c.add(new_circle(vec2(0.0, 0.0), 5.0).scale_by(1e300), s);", "canvas.add: the path has a non-finite"),
            ("c.add(glyphset(\"symbols\").glyph(\"basic_eye\").scale_by(1e300), s);", "add_glyph: the path has a non-finite"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", setup, script)).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn margins_default_to_half_the_largest_brush() {
        let script = r#"
            let thin = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let wide = new_style(new_brush_style("black", "Ink Pen", 6.0), new_handwriting_style());
            let c = new_canvas();
            c.add(new_line(vec2(0.0, 0.0), vec2(10.0, 10.0)), thin);
            c.add(new_line(vec2(0.0, 0.0), vec2(10.0, 0.0)), wide);
            let auto = new_page();
            if auto.margin != () { throw "default margin should read as ()"; }
            auto.add(c);
            journal_entry.add(auto);
            let set = new_page();
            set.margin = 20;
            set.add(c);
            journal_entry.add(set);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let canvas = &entry.pages[0].canvases()[0];
        assert_eq!(entry.pages[0].margin_for(canvas), 3.0);
        assert_eq!(entry.pages[1].margin_for(canvas), 20.0);
        let mut padded: JournalCanvas = canvas.into();
        let before = padded.render_extents();
        padded.pad(3.0);
        assert!((padded.render_extents() - before - parry2d::math::Vec2::splat(6.0)).length() < 1e-4);
        for (value, want) in [("-1", "at least 0"), ("\"wide\"", "expected a number or ()")] {
            let err = engine().build_entry_str(&format!("let p = new_page();\np.margin = {};", value)).expect_err(value).to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn warn_reports_with_the_line_and_keeps_going() {
        let mut engine = engine();
        engine.build_entry_str("let x = 1;\nwarn(\"too many \" + 3);\njournal_entry.add(new_page());")
            .expect("warn shouldn't stop the script");
        assert_eq!(engine.warnings(), vec!["too many 3 (line 2)".to_string()]);
    }

    #[test]
    fn pages_are_found_by_index_or_name() {
        let entry = engine().build_entry_str(NAMED).expect("script should run");
        assert_eq!(entry.page_index("0"), Ok(0));
        assert_eq!(entry.page_index("maze"), Ok(1));
        assert_eq!(entry.page_index("page_0"), Ok(0));
        let err = entry.page_index("2").unwrap_err();
        assert!(err.contains("pages are 0-1: page_0, maze"), "{}", err);
        assert!(entry.page_index("lock").is_err());
    }

    #[test]
    fn names_must_be_unique_and_file_safe() {
        for (script, want) in [
            ("let p = new_page(); p.name = \"maze\"; journal_entry.add(p);\njournal_entry.add(p);", "already has a page named 'maze'"),
            // An unnamed page's label counts too.
            ("let p = new_page(); p.name = \"page_1\"; journal_entry.add(p);\njournal_entry.add(new_page());", "already has a page named 'page_1'"),
            ("let p = new_page(); let c = new_canvas(); c.name = \"a\"; p.add(c);\np.add(c);", "already has a canvas named 'a'"),
            ("let p = new_page();\np.name = \"../escape\";", "page.name: invalid name '../escape'"),
            ("let c = new_canvas();\nc.name = 3;", "canvas.name: a name must be a string"),
        ] {
            let err = engine().build_entry_str(script).err().unwrap_or_else(|| panic!("should fail: {}", want)).to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn svg_style_defaults_merges_and_turns_off() {
        let script = r#"
            let p = new_page();
            if p.name != () { throw "unnamed pages have no name"; }
            if p.svg_style.roughness != 1.0 || p.svg_style.noise_amplitude != 0.3 { throw "unexpected default: " + p.svg_style; }
            p.svg_style = #{ roughness: 2, noise_amplitude: 1.5 };
            if p.svg_style.roughness != 2.0 || p.svg_style.trail_off_percent != 0.15 { throw "not merged: " + p.svg_style; }
            journal_entry.add(p);
            let plain = new_page();
            plain.svg_style = ();
            journal_entry.add(plain);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let style = entry.pages[0].svg_style.as_ref().expect("styled");
        assert_eq!((style.roughness, style.point_sample_distance, style.noise_amplitude), (2.0, 1.0, 1.5));
        assert!(entry.pages[1].svg_style.is_none());
        for (script, want) in [
            ("let p = new_page();\np.svg_style = #{ roughnes: 1 };", "unknown option 'roughnes'"),
            ("let p = new_page();\np.svg_style = #{ roughness: -1 };", "'roughness' can't be negative"),
            ("let p = new_page();\np.svg_style = 2;", "expected a map of options or ()"),
        ] {
            let err = engine().build_entry_str(script).expect_err("script should fail").to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn brushes_carry_a_pressure_profile() {
        let script = r#"
            let b = new_brush_style("black", "Ink Pen", 1.0, #{ min_pressure: 0.2, max_pressure: 0.8 });
            if b.pressure_profile != "ramp" { throw "default should be ramp"; }
            b.pressure_profile = "taper";
            let c = new_canvas();
            c.add(new_line(vec2(0.0, 0.0), vec2(10.0, 0.0)), new_style(b, new_handwriting_style()));
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let jc: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        assert_eq!(jc.strokes()[0].style.brush.as_ref().unwrap().pressure_profile, PressureProfile::Taper);
        let err = engine().build_entry_str("let b = new_brush_style(\"black\", \"Ink Pen\", 1.0);\nb.pressure_profile = \"wobbly\";")
            .expect_err("unknown profile should fail").to_string();
        assert!(err.contains("unknown pressure_profile 'wobbly'") && err.contains("ramp, taper, constant, pointed") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn brushes_pool_and_wander_in_pressure() {
        let script = r#"
            let b = new_brush_style("black", "Ink Pen", 1.0, #{ min_pressure: 0.2, max_pressure: 0.8 });
            if b.pool != 0.0 || b.pool_depth != 0.3 || b.pressure_wander != 0.0 || b.pressure_wander_length != 12.0 { throw "no pooling or wander by default"; }
            b.pool = 0.4;
            b.pool_depth = 0.5;
            b.pressure_wander = 0.25;
            b.pressure_wander_length = 6;
            let c = new_canvas();
            c.add(new_line(vec2(0.0, 0.0), vec2(10.0, 0.0)), new_style(b, new_handwriting_style()));
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let jc: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        let brush = jc.strokes()[0].style.brush.clone().unwrap();
        assert_eq!((brush.pool, brush.pool_depth, brush.pressure_variation, brush.variation_length), (0.4, 0.5, 0.25, 6.0));
        for (setting, want) in [
            ("b.pool = -0.1;", "'pool' can't be negative"),
            ("b.pool_depth = 0;", "'pool_depth' must be positive"),
            ("b.pool_depth = 1.5;", "'pool_depth' is a fraction"),
            ("b.pressure_wander = 1;", "'pressure_wander' is a fraction"),
            ("b.pressure_wander_length = 0;", "'pressure_wander_length' must be positive"),
        ] {
            let err = engine().build_entry_str(&format!("let b = new_brush_style(\"black\", \"Ink Pen\", 1.0);\n{}", setting))
                .expect_err(setting).to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn text_along_a_path_can_hold_an_angle() {
        // Upright text up a steep line: each glyph's strokes are an upright
        // glyph's, moved.
        let script = r#"
            let s = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let line = new_line(vec2(0.0, 0.0), vec2(300.0, -400.0));
            [line.fill_with_text("ll", #{ style: s, type: new_type_style(glyphset("plain"), 20.0), paragraph: #{ angle: 0.0 } })[0], line.fit_text("ll", #{ style: s, type: #{ glyphset: glyphset("plain") }, paragraph: #{ angle: 0 } })]
        "#;
        let canvases: rhai::Array = engine().engine.eval(script).expect("should run");
        for canvas in canvases {
            let canvas = canvas.cast::<Canvas>();
            for stroke in &canvas.borrow().strokes {
                // An l is a tall stroke: upright, its points barely move across.
                let xs: Vec<f32> = stroke.path.points_within().iter().map(|p| p.x).collect();
                let ys: Vec<f32> = stroke.path.points_within().iter().map(|p| p.y).collect();
                let spread = |v: &[f32]| v.iter().fold(f32::MIN, |m, x| m.max(*x)) - v.iter().fold(f32::MAX, |m, x| m.min(*x));
                assert!(spread(&xs) < spread(&ys) / 3.0, "an upright l: {} across, {} down", spread(&xs), spread(&ys));
            }
        }
        let err = engine().build_entry_str("let ok = 1;\nnew_line(vec2(0.0, 0.0), vec2(90.0, 0.0)).fit_text(\"a\", #{ type: #{ glyphset: glyphset(\"plain\") }, paragraph: #{ angle: \"up\" } });")
            .expect_err("a bad angle").to_string();
        assert!(err.contains("fit_text") && err.contains("angle") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn stroke_point_jitter_reshapes_each_pass() {
        let script = r#"
            let s = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style(#{ stroke_point_jitter: 4 }));
            if s.stroke_point_jitter != 4.0 || s.passes != 1 || s.pass_end_jitter != 0.0 { throw "stroke_point_jitter alone"; }
            let c = new_canvas();
            c.add(new_path().move_to(vec2(0, 0)).cubic_to(vec2(30, -40), vec2(70, -40), vec2(100, 0)), s);
            s.passes = 3;
            c.add(new_line(vec2(0, 50), vec2(100, 50)), s);
            journal_entry.add(new_page().add(c));
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let jc: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        let curve = jc.strokes()[0].path.points_within();
        let p = parry2d::math::Vec2::new;
        let original = [p(0.0, 0.0), p(30.0, -40.0), p(70.0, -40.0), p(100.0, 0.0)];
        let moved: Vec<_> = curve.iter().zip(original).map(|(a, b)| *a - b).collect();
        assert!(moved.iter().all(|d| d.length() <= 4.0 + 1e-4), "{:?}", moved);
        // Control points stray on their own, not along with the ends.
        assert!(moved.windows(2).all(|w| (w[0] - w[1]).length() > 1e-3), "{:?}", moved);
        // Each pass reshaped differently.
        assert_eq!(jc.strokes()[1].path.to_substrokes().len(), 3);
        let ends = jc.strokes()[1].path.points_within();
        assert!((ends[0] - ends[2]).length() > 1e-3 && (ends[1] - ends[3]).length() > 1e-3, "{:?}", ends);
        // Drawn from the journal's generator, so the same every run.
        let again = engine().build_entry_str(script).expect("script should run");
        let again: JournalCanvas = again.pages[0].canvases()[0].clone().into();
        assert_eq!(jc.strokes()[0].path.to_string(), again.strokes()[0].path.to_string());
    }

    #[test]
    fn styles_go_over_their_strokes_in_passes() {
        let script = r#"
            let s = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            if s.passes != 1 || s.pass_end_jitter != 0.0 { throw "one clean pass by default"; }
            let manic = s;
            manic.passes = 5;
            manic.pass_end_jitter = 3;
            if manic.passes != 5 || manic.pass_end_jitter != 3.0 || s.passes != 1 { throw "styles are values"; }
            let c = new_canvas();
            let line = new_line(vec2(0, 0), vec2(100, 0));
            c.add(line, s);
            c.add(line.copy().append(new_circle(vec2(50, 50), 20)), manic);
            // Bounds take in where the passes stray.
            let b = c.bounds();
            if b.max.x <= 101.0 && b.min.x >= -1.0 { throw "bounds miss the passes: " + b; }
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let jc: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        let subpaths: Vec<usize> = jc.strokes().iter().map(|s| s.path.to_substrokes().len()).collect();
        assert_eq!(subpaths, vec![1, 10]);
        for (script, want) in [
            ("s.passes = 0;", "style: 'passes' should be from 1 to 64, got 0"),
            ("s.passes = 1.5;", "style: 'passes' should be a whole number"),
            ("s.pass_end_jitter = -1;", "style: 'pass_end_jitter' can't be negative"),
            ("s.stroke_point_jitter = -1;", "style: 'stroke_point_jitter' can't be negative"),
            ("new_handwriting_style(#{ stroke_point_jitter: -1 });", "new_handwriting_style: 'stroke_point_jitter' can't be negative"),
            ("new_handwriting_style(#{ jitter: 1 });", "new_handwriting_style: unknown option 'jitter'"),
        ] {
            let err = engine().build_entry_str(&format!("let s = default_style();\n{}", script))
                .expect_err("should fail").to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn per_stroke_brushes_reach_journal_canvas() {
        let script = r#"
            let ink = new_style(new_brush_style("black", "Rough Ink", 1.0, #{ min_pressure: 0.375, max_pressure: 0.75 }), new_handwriting_style());
            let calli = new_style(new_brush_style("darkred", "Calligraphy", 0.8, #{ min_pressure: 1, max_pressure: 1 }), new_handwriting_style());
            let path = new_path().move_to(vec2(0.0, 0.0)).line_to(vec2(10.0, 10.0));
            let canvas = new_canvas();
            canvas.add(path, ink);
            canvas.add(path, calli);
            let page = new_page();
            page.add(canvas);
            journal_entry.add(page);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let jc: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        let brushes: Vec<_> = jc.strokes().iter()
            .map(|s| s.style.brush.clone().expect("rhai strokes always have a brush"))
            .map(|b| (b.name, b.size))
            .collect();
        assert_eq!(brushes, vec![("Rough Ink".to_string(), 1.0), ("Calligraphy".to_string(), 0.8)]);
    }
}
