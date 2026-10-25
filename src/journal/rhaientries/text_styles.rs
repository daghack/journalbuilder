//! Text style files, `styles/<name>.rhai` (a journal's over the
//! library's): the look of a kind of text, kept apart from the entries that
//! write it, for `text_style(name)` and `text_style(name, options)`. A
//! style file defines:
//!
//! - `options()` (optional): `#{ NAME: default }`. Every option is
//!   optional, its type its default's.
//! - `style(o)`: `#{ style, type, paragraph, spans }`, each whole or a map
//!   of settings, as a layout's role takes them (`style()` if it has no
//!   options).
//! - `example()` (optional): the text to preview it with.
//!
//! `text_style` gives back the bundle with the style, type and paragraph
//! whole (each a map's settings over the defaults), and the spans if it has
//! any: a role for `typeset`, or the settings map of any text call.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rhai::{Dynamic, Engine, Map, NativeCallContext, Scope, AST};

use super::markdown::Spans;
use super::options::{check_keys, Settings};
use super::sources::Sources;
use super::text_settings::{ParagraphStyle, TypeStyle};
use super::{RhaiResult, Style};

/// What a style's bundle has.
const KEYS: [&str; 4] = ["style", "type", "paragraph", "spans"];

/// The style files, each loaded when first used.
pub struct TextStyleLibrary {
    sources: Sources,
    loaded: RefCell<HashMap<String, Rc<TextStyleFile>>>,
}

/// A loaded style file.
pub struct TextStyleFile {
    pub name: String,
    ast: AST,
    /// Its options' defaults.
    pub defaults: Map,
    /// The file's opening comment, without its "name: ".
    pub description: String,
}

impl TextStyleLibrary {
    pub fn new(sources: Sources) -> Self {
        TextStyleLibrary { sources, loaded: RefCell::default() }
    }

    /// Every style's name.
    pub fn names(&self) -> Vec<String> {
        self.sources.names().unwrap_or_default()
    }

    /// The style file `name`, compiled with `engine` (the journal's).
    pub fn load(&self, engine: &Engine, name: &str) -> Result<Rc<TextStyleFile>, String> {
        if let Some(loaded) = self.loaded.borrow().get(name) {
            return Ok(loaded.clone());
        }
        if name.is_empty() || name.contains(['/', '\\', '.']) || !self.sources.has(name) {
            let names = self.names();
            return Err(format!("no text style named '{}'; styles: {}", name, if names.is_empty() { "none".to_string() } else { names.join(", ") }));
        }
        let what = format!("text style '{}'", name);
        let ast = self.sources.compile(engine, name).map_err(|e| format!("{}: {}", what, e))?;
        if !ast.iter_functions().any(|f| f.name == "style" && f.params.len() <= 1) {
            return Err(format!("{}: {} doesn't define style(o)", what, self.sources.path(name).display()));
        }
        let defaults = super::options::declared_defaults(engine, &ast, &what)?;
        let source = self.sources.read(name).map_err(|e| e.to_string())?;
        let description = super::illustrations::description(&source, name);
        let loaded = Rc::new(TextStyleFile { name: name.to_string(), ast, defaults, description });
        self.loaded.borrow_mut().insert(name.to_string(), loaded.clone());
        Ok(loaded)
    }
}

impl TextStyleFile {
    fn what(&self) -> String {
        format!("text style '{}'", self.name)
    }

    /// Its bundle, with `given` options over its defaults.
    pub fn bundle(&self, engine: &Engine, given: &Map) -> Result<Map, String> {
        let o = super::options::over_defaults(&self.what(), &self.defaults, given)?;
        let takes_options = self.ast.iter_functions().any(|f| f.name == "style" && f.params.len() == 1);
        let value = if takes_options {
            engine.call_fn::<Dynamic>(&mut Scope::new(), &self.ast, "style", (Dynamic::from_map(o),))
        } else {
            engine.call_fn::<Dynamic>(&mut Scope::new(), &self.ast, "style", ())
        }.map_err(|e| format!("{} style(o): {}", self.what(), e))?;
        let type_name = value.type_name();
        let map = value.try_cast::<Map>()
            .ok_or_else(|| format!("{}: style(o) should return #{{ style, type, paragraph, spans }}, got {}", self.what(), type_name))?;
        whole(&format!("{} style(o)", self.what()), map).map_err(|e| super::options::unwrap_runtime(&e))
    }

    /// The text its file gives to preview it with, if it gives any.
    pub fn example(&self, engine: &Engine) -> Result<Option<String>, String> {
        if !self.ast.iter_functions().any(|f| f.name == "example" && f.params.is_empty()) {
            return Ok(None);
        }
        let value = engine.call_fn::<Dynamic>(&mut Scope::new(), &self.ast, "example", ())
            .map_err(|e| format!("{} example(): {}", self.what(), e))?;
        let type_name = value.type_name();
        value.into_string().map(Some).map_err(|_| format!("{}: example() should return the text to preview it with, got {}", self.what(), type_name))
    }
}

/// A bundle with its style, type and paragraph made whole.
fn whole(caller: &str, mut map: Map) -> RhaiResult<Map> {
    check_keys(caller, &map, &KEYS)?;
    let style = match map.remove("style") {
        None => Style::default_style(),
        Some(s) => Style::given(&format!("{} 'style'", caller), &s, &Style::default_style())?,
    };
    let type_ = match map.remove("type") {
        None => TypeStyle::empty(),
        Some(t) => TypeStyle::given(&format!("{} 'type'", caller), &t, &TypeStyle::empty())?,
    };
    let paragraph = match map.remove("paragraph") {
        None => ParagraphStyle::default(),
        Some(p) => ParagraphStyle::given(&format!("{} 'paragraph'", caller), &p, &ParagraphStyle::default())?,
    };
    let mut bundle = Map::new();
    bundle.insert("style".into(), Dynamic::from(style));
    bundle.insert("type".into(), Dynamic::from(type_));
    bundle.insert("paragraph".into(), Dynamic::from(paragraph));
    if let Some(spans) = map.remove("spans") {
        Spans::from_dynamic(&format!("{} 'spans'", caller), &spans)?;
        bundle.insert("spans".into(), spans);
    }
    Ok(bundle)
}

pub fn register(engine: &mut Engine, library: Rc<TextStyleLibrary>) {
    let styles = library.clone();
    engine.register_fn("text_style", move |ctx: NativeCallContext, name: &str| -> RhaiResult<Map> {
        bundle(&ctx, &styles, name, &Map::new())
    });
    engine.register_fn("text_style", move |ctx: NativeCallContext, name: &str, options: Map| -> RhaiResult<Map> {
        bundle(&ctx, &library, name, &options)
    });
}

fn bundle(ctx: &NativeCallContext, library: &TextStyleLibrary, name: &str, options: &Map) -> RhaiResult<Map> {
    let file = library.load(ctx.engine(), name).map_err(|e| format!("text_style: {}", e))?;
    file.bundle(ctx.engine(), options).map_err(|e| format!("text_style: {}", e).into())
}

#[cfg(test)]
mod tests {
    use super::super::{JournalEngine, Resources};
    use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};
    use std::rc::Rc;

    const HAND: &str = r#"
        // hand: a red hand.
        fn options() { #{ size: 30.0, color: "darkred" } }
        fn style(o) {
            #{
                style: #{ color: o.color, passes: 2 },
                type: new_type_style(glyphset("plain"), o.size),
                paragraph: #{ align: "center" },
                spans: #{ warn: #{ style: #{ color: "red" } } },
            }
        }
        fn example() { "Lorem ipsum." }
    "#;

    fn engine(styles: &[(&str, &str)]) -> JournalEngine {
        let mut resources = Resources::library();
        resources.styles = resources.styles.with_overlay(styles.iter().map(|(n, s)| (n.to_string(), s.to_string())));
        JournalEngine::with_resources(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).unwrap()), resources)
    }

    #[test]
    fn a_style_file_gives_a_whole_bundle() {
        let mut engine = engine(&[("hand", HAND), ("bare", "fn style() { #{} }")]);
        let entry = engine.build_entry_str(r#"
            let hand = text_style("hand", #{ size: 40 });
            assert(hand.type.glyph_scale == 40.0, "options over the defaults");
            assert(hand.style.passes == 2 && hand.style.brush == "Ink Pen", "the style over default_style()");
            assert(hand.paragraph.align == "center", "the paragraph");
            assert("warn" in hand.spans, "the spans");
            assert(!("spans" in text_style("bare")), "no spans unless given");
            let c = new_canvas();
            c.add(write_line("Lorem", vec2(0, 0), hand));
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "#);
        assert!(entry.is_ok(), "{:?}", entry.err());
        assert_eq!(engine.describe_text_style("hand").unwrap().example.as_deref(), Some("Lorem ipsum."));
    }

    #[test]
    fn every_library_style_previews() {
        let mut engine = engine(&[]);
        let names = engine.text_style_names();
        assert!(!names.is_empty(), "the library has text styles");
        for name in names {
            let entry = engine.preview_text_style(&name, &serde_json::json!({}), 900.0).unwrap_or_else(|e| panic!("{}: {}", name, e));
            assert!(entry.page_count() == 1 && entry.empty_canvases().is_empty(), "{} should write its example", name);
            assert!(engine.describe_text_style(&name).unwrap().example.is_some(), "{} should have an example()", name);
        }
    }

    #[test]
    fn style_file_mistakes_are_named() {
        for (script, want) in [
            (r#"text_style("nope")"#, "text_style: no text style named 'nope'"),
            (r#"text_style("hand", #{ width: 3 })"#, "text_style: text style 'hand' has no option 'width'; its options: color, size"),
            (r#"text_style("hand", #{ size: "big" })"#, "text_style: text style 'hand': option 'size' should be like its default"),
            (r#"text_style("keys")"#, "text_style: text style 'keys' style(o): unknown option 'font'"),
            (r#"text_style("none")"#, "doesn't define style(o)"),
        ] {
            let mut engine = engine(&[("hand", HAND), ("keys", "fn style(o) { #{ font: 1 } }"), ("none", "fn other() {}")]);
            let err = engine.build_entry_str(script).expect_err(script);
            assert!(err.contains(want), "{}\n  wanted: {}\n  got: {}", script, want, err);
        }
    }
}
