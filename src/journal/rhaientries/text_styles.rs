//! Text style files, `styles/<name>.json` (a journal's over the library's):
//! the look of a kind of text, kept apart from the entries that write it, for
//! `text_style(name)` and `text_style(name, options)`. A style file is a JSON
//! object of every setting a style, a type and a paragraph have, flattened,
//! every key optional:
//!
//! - `description`, and `example`: the text to preview it with.
//! - A style's settings (`Style::KEYS`), `color` a colour or a gradient as data.
//! - A type's settings (`TypeStyle::KEYS`), `glyphset` by its name and the
//!   glyph scale as `size`.
//! - A paragraph's settings (`ParagraphStyle::KEYS`).
//! - `spans`, as a role takes them, written the same way: colours as data,
//!   glyphsets by name.
//!
//! `glyph_position_jitter` and `baseline_wander` are in glyph scales here,
//! wherever they're written, so they follow its size. `size` and `color` are
//! its options.
//!
//! `text_style` gives back the bundle with the style, type and paragraph
//! whole (each a map's settings over the defaults), and the spans if it has
//! any: a role for `typeset`, or the settings map of any text call.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rhai::{Dynamic, Engine, Map, FLOAT};

use super::glyphs;
use super::gradient;
use super::locks;
use super::markdown::Spans;
use super::options::{check_keys, over_defaults, setting_number, unwrap_runtime, Settings};
use super::sources::Sources;
use super::text_settings::{ParagraphStyle, TypeStyle};
use super::{RhaiResult, Style};
use crate::journal::fonts::FontBook;

/// What a style's bundle has.
const KEYS: [&str; 4] = ["style", "type", "paragraph", "spans"];

/// A style file's keys that aren't one of its settings as it is.
const OTHER_KEYS: [&str; 5] = ["description", "example", "size", "glyphset", "spans"];

/// What a style file gives in glyph scales, which a style has in canvas units.
pub(super) const IN_GLYPH_SCALES: [&str; 2] = ["glyph_position_jitter", "baseline_wander"];

/// A type's lines, each of which may have a style of its own.
const RULES: [&str; 2] = ["underline", "strike"];

/// The style files, each read when first used.
pub struct TextStyleLibrary {
    sources: Sources,
    book: Rc<FontBook>,
    loaded: RefCell<HashMap<String, Rc<TextStyleFile>>>,
}

/// A style file, read.
pub struct TextStyleFile {
    pub name: String,
    /// Its settings; its description, example, size and colour taken out.
    settings: Map,
    /// Its options' defaults: its size (unit if it gives none) and colour.
    pub defaults: Map,
    pub description: String,
    /// The text it previews with, if it gives any.
    pub example: Option<String>,
}

impl TextStyleLibrary {
    pub fn new(sources: Sources, book: Rc<FontBook>) -> Self {
        TextStyleLibrary { sources, book, loaded: RefCell::default() }
    }

    /// Every style's name.
    pub fn names(&self) -> Vec<String> {
        self.sources.names().unwrap_or_default()
    }

    /// The glyphsets its files name.
    pub(super) fn book(&self) -> &FontBook {
        &self.book
    }

    /// The style file `name`.
    pub fn load(&self, name: &str) -> Result<Rc<TextStyleFile>, String> {
        if let Some(loaded) = self.loaded.borrow().get(name) {
            return Ok(loaded.clone());
        }
        if name.is_empty() || name.contains(['/', '\\', '.']) || !self.sources.has(name) {
            let names = self.names();
            return Err(format!("no text style named '{}'; styles: {}", name, if names.is_empty() { "none".to_string() } else { names.join(", ") }));
        }
        let source = self.sources.read(name).map_err(|e| e.to_string())?;
        let loaded = Rc::new(TextStyleFile::read(name, &source, &self.sources.path(name).display().to_string())?);
        self.loaded.borrow_mut().insert(name.to_string(), loaded.clone());
        Ok(loaded)
    }
}

impl TextStyleFile {
    /// The style file `name` with text `source`, read from `path`.
    pub(super) fn read(name: &str, source: &str, path: &str) -> Result<TextStyleFile, String> {
        let what = format!("text style '{}'", name);
        let json: serde_json::Value = serde_json::from_str(source).map_err(|e| format!("{}: {} isn't JSON: {}", what, path, e))?;
        if !json.is_object() {
            return Err(format!("{}: {} should be a JSON object", what, path));
        }
        let mut settings = locks::from_json(&json).cast::<Map>();
        let type_keys: Vec<&str> = TypeStyle::KEYS.iter().copied().filter(|key| !["glyphset", "glyph_scale"].contains(key)).collect();
        let known = [&OTHER_KEYS[..], Style::KEYS, &type_keys, ParagraphStyle::KEYS].concat();
        check_keys(&what, &settings, &known).map_err(|e| unwrap_runtime(&e))?;
        let mut text = |key: &str| match settings.remove(key) {
            None => Ok(None),
            Some(value) => value.into_string().map(Some).map_err(|t| format!("{}: '{}' should be a string, got {}", what, key, t)),
        };
        let description = text("description")?.unwrap_or_default();
        let example = text("example")?.filter(|example| !example.trim().is_empty());
        let color = settings.remove("color").unwrap_or_else(|| "black".into());
        let color = gradient::colour_option(&color).map_err(|e| format!("{}: 'color': {}", what, unwrap_runtime(&e)))?;
        let mut defaults = Map::new();
        defaults.insert("color".into(), color);
        defaults.insert("size".into(), settings.remove("size").unwrap_or(Dynamic::UNIT));
        Ok(TextStyleFile { name: name.to_string(), settings, defaults, description, example })
    }

    fn what(&self) -> String {
        format!("text style '{}'", self.name)
    }

    /// Those of its settings called `keys`.
    fn settings_of(&self, keys: &[&str]) -> Map {
        keys.iter().filter_map(|&key| self.settings.get(key).map(|value| (key.into(), value.clone()))).collect()
    }

    /// Its bundle, with `given` options over its defaults; `book` has the
    /// glyphsets it names.
    pub fn bundle(&self, book: &FontBook, given: &Map) -> Result<Map, String> {
        let what = self.what();
        let o = over_defaults(&what, &self.defaults, given)?;
        let size = &o["size"];
        let mut style = self.settings_of(Style::KEYS);
        style.insert("color".into(), o["color"].clone());
        let mut type_ = type_settings(&what, book, self.settings_of(TypeStyle::KEYS), size)?;
        if !size.is_unit() {
            type_.insert("glyph_scale".into(), size.clone());
        }
        let mut bundle = Map::new();
        bundle.insert("style".into(), Dynamic::from_map(style_settings(&what, style, size)?));
        bundle.insert("type".into(), Dynamic::from_map(type_));
        bundle.insert("paragraph".into(), Dynamic::from_map(self.settings_of(ParagraphStyle::KEYS)));
        if let Some(spans) = self.settings.get("spans") {
            bundle.insert("spans".into(), span_settings(&what, book, spans, size)?);
        }
        whole(&what, bundle).map_err(|e| unwrap_runtime(&e))
    }
}

/// A map of style settings as a file has them, as a style takes them: its
/// colour's gradient made one, and what's in glyph scales in canvas units.
fn style_settings(what: &str, mut style: Map, size: &Dynamic) -> Result<Map, String> {
    if let Some(color) = style.get("color") {
        let color = gradient::colour_option(color).map_err(|e| format!("{}: 'color': {}", what, unwrap_runtime(&e)))?;
        style.insert("color".into(), color);
    }
    for key in IN_GLYPH_SCALES {
        let Some(fraction) = style.get(key) else { continue };
        if size.is_unit() {
            return Err(format!("{}: '{}' is in glyph scales, so it needs a 'size'", what, key));
        }
        let scaled = |fraction: &Dynamic| -> RhaiResult<Dynamic> {
            Ok(Dynamic::from_float((setting_number(what, key, fraction)? * setting_number(what, "size", size)?) as FLOAT))
        };
        let canvas_units = match fraction.read_lock::<rhai::Array>() {
            Some(pair) => pair.iter().map(scaled).collect::<RhaiResult<rhai::Array>>().map(Dynamic::from_array),
            None => scaled(fraction),
        };
        style.insert(key.into(), canvas_units.map_err(|e| unwrap_runtime(&e))?);
    }
    Ok(style)
}

/// A map of type settings as a file has them, as a type takes them: its
/// glyphset found by name, and its lines' style settings as a style takes them.
fn type_settings(what: &str, book: &FontBook, mut type_: Map, size: &Dynamic) -> Result<Map, String> {
    if let Some(glyphset) = type_.get("glyphset") {
        let name = glyphset.clone().into_string().map_err(|t| format!("{}: 'glyphset' should be a glyphset's name, got {}", what, t))?;
        let glyphset = glyphs::named(book, "glyphset", &name).map_err(|e| format!("{}: {}", what, unwrap_runtime(&e)))?;
        type_.insert("glyphset".into(), Dynamic::from(glyphset));
    }
    for key in RULES {
        let Some(mut rule) = type_.get(key).and_then(|rule| rule.clone().try_cast::<Map>()) else { continue };
        let Some(style) = rule.get("style").and_then(|style| style.clone().try_cast::<Map>()) else { continue };
        rule.insert("style".into(), Dynamic::from_map(style_settings(what, style, size)?));
        type_.insert(key.into(), Dynamic::from_map(rule));
    }
    Ok(type_)
}

/// A file's `spans`, as a role takes them; what isn't as it should be is left
/// for `whole` to say.
fn span_settings(what: &str, book: &FontBook, spans: &Dynamic, size: &Dynamic) -> Result<Dynamic, String> {
    let Some(mut spans) = spans.clone().try_cast::<Map>() else { return Ok(spans.clone()) };
    for span in spans.values_mut() {
        let Some(mut settings) = span.clone().try_cast::<Map>() else { continue };
        if let Some(style) = settings.get("style").and_then(|style| style.clone().try_cast::<Map>()) {
            settings.insert("style".into(), Dynamic::from_map(style_settings(what, style, size)?));
        }
        if let Some(type_) = settings.get("type").and_then(|type_| type_.clone().try_cast::<Map>()) {
            settings.insert("type".into(), Dynamic::from_map(type_settings(what, book, type_, size)?));
        }
        *span = Dynamic::from_map(settings);
    }
    Ok(Dynamic::from_map(spans))
}

/// A bundle, `given` (a map of some of its keys), with what it doesn't
/// give taken from `under`'s, a bundle's too: the keys it hasn't, and in
/// its type, what `TypeStyle::or` fills in.
pub(super) fn bundle_over(caller: &str, given: Map, under: &Map) -> RhaiResult<Map> {
    let mut out = under.clone();
    for (key, value) in given {
        let value = match (key.as_str(), under.get("type")) {
            ("type", Some(below)) => {
                let what = format!("{} 'type'", caller);
                let over = TypeStyle::given(&what, &value, &TypeStyle::empty())?;
                Dynamic::from(over.or(&TypeStyle::given(&what, below, &TypeStyle::empty())?))
            }
            _ => value,
        };
        out.insert(key, value);
    }
    Ok(out)
}

/// A bundle with its style, type and paragraph made whole.
pub(super) fn whole(caller: &str, mut map: Map) -> RhaiResult<Map> {
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
    engine.register_fn("text_style", move |name: &str| -> RhaiResult<Map> { bundle(&styles, name, &Map::new()) });
    engine.register_fn("text_style", move |name: &str, options: Map| -> RhaiResult<Map> { bundle(&library, name, &options) });
}

fn bundle(library: &TextStyleLibrary, name: &str, options: &Map) -> RhaiResult<Map> {
    let file = library.load(name).map_err(|e| format!("text_style: {}", e))?;
    file.bundle(library.book(), options).map_err(|e| format!("text_style: {}", e).into())
}

#[cfg(test)]
mod tests {
    use super::super::{json_map, Dynamic, Entry, JournalEngine, Resources, Scope};
    use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};
    use std::rc::Rc;

    const HAND: &str = r##"{
        "description": "a red hand.",
        "example": "Lorem ipsum.",
        "color": "darkred",
        "passes": 2,
        "baseline_wander": 0.1,
        "glyphset": "plain",
        "size": 30,
        "align": "center",
        "spans": { "warn": { "style": { "color": "red" } } }
    }"##;

    /// Text style `name` with `options` (a JSON object) over its defaults,
    /// writing its example text (or else `content("lorem")`): a page.
    fn preview(engine: &mut JournalEngine, name: &str, options: &serde_json::Value) -> Result<Entry, String> {
        let text = engine.describe_text_style(name)?.example.map_or(Dynamic::UNIT, Dynamic::from);
        let mut scope = Scope::new();
        scope.push("journal_entry", Entry::default());
        scope.push_constant("NAME", name.to_string());
        scope.push_constant("OPTIONS", json_map(options, "options")?);
        scope.push_constant("TEXT", text);
        engine.run_preview(scope, r#"
            let style = text_style(NAME, OPTIONS);
            let size = style.type.glyph_scale ?? 30.0;
            let c = new_canvas();
            c.add(write_block(TEXT ?? content("lorem"), vec2(size, size * 2), 900.0 - size * 2, style).canvas);
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "#)
    }

    fn engine(styles: &[(&str, &str)]) -> JournalEngine {
        let mut resources = Resources::library();
        resources.styles = resources.styles.with_overlay(styles.iter().map(|(n, s)| (n.to_string(), s.to_string())));
        JournalEngine::with_resources(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).unwrap()), resources)
    }

    #[test]
    fn a_style_file_gives_a_whole_bundle() {
        let mut engine = engine(&[("hand", HAND), ("bare", "{}")]);
        let entry = engine.build_entry_str(r#"
            let hand = text_style("hand", #{ size: 40 });
            assert(hand.type.glyph_scale == 40.0, "options over the defaults");
            assert(hand.style.passes == 2 && hand.style.brush == "Ink Pen", "the style over default_style()");
            assert(hand.style.baseline_wander == 4.0, "in glyph scales, so it follows the size");
            assert(hand.paragraph.align == "center", "the paragraph");
            assert("warn" in hand.spans, "the spans");
            assert(!("spans" in text_style("bare")), "no spans unless given");
            assert(text_style("bare").style.color == "black", "nothing given, default_style()'s colour");
            assert(text_style("bare", #{ size: 12 }).type.glyph_scale == 12.0, "a size even if the file has none");
            let c = new_canvas();
            c.add(write_line("Lorem", vec2(0, 0), hand));
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "#);
        assert!(entry.is_ok(), "{:?}", entry.err());
        let info = engine.describe_text_style("hand").unwrap();
        assert_eq!(info.example.as_deref(), Some("Lorem ipsum."));
        assert_eq!(info.description, "a red hand.");
        assert_eq!(info.options, serde_json::json!({ "color": "darkred", "size": 30 }));
    }

    #[test]
    fn every_library_style_previews() {
        let mut engine = engine(&[]);
        let names = engine.text_style_names();
        assert!(!names.is_empty(), "the library has text styles");
        for name in names {
            let entry = preview(&mut engine, &name, &serde_json::json!({})).unwrap_or_else(|e| panic!("{}: {}", name, e));
            assert!(entry.page_count() == 1 && entry.empty_canvases().is_empty(), "{} should write its example", name);
            assert!(!engine.describe_text_style(&name).unwrap().description.is_empty(), "{} should say what it's for", name);
        }
    }

    #[test]
    fn a_style_with_no_example_previews_in_lorem_ipsum() {
        let mut engine = engine(&[("plain", r#"{ "glyphset": "plain", "size": 30, "example": " " }"#)]);
        assert_eq!(engine.describe_text_style("plain").unwrap().example, None);
        let entry = preview(&mut engine, "plain", &serde_json::json!({})).unwrap();
        assert!(entry.page_count() == 1 && entry.empty_canvases().is_empty());
    }

    #[test]
    fn colour_options_take_gradients() {
        const INKED: &str = r#"{ "color": { "colors": ["red", "blue"], "vertical": true }, "glyphset": "plain", "size": 30 }"#;
        let mut engine = engine(&[("hand", HAND), ("inked", INKED)]);
        let entry = engine.build_entry_str(r#"
            let whole = text_style("hand", #{ color: new_gradient(["red", "blue"]) });
            assert(type_of(whole.style.color) == "Gradient", "a gradient, whole");
            let data = text_style("hand", #{ color: #{ colors: ["red", "blue"], from: [0, 0], to: [90, 0] } });
            assert(type_of(data.style.color) == "Gradient", "a gradient, as data");
            assert(text_style("hand", #{ color: "gold" }).style.color == "gold", "a colour still");
            assert(type_of(text_style("inked").style.color) == "Gradient", "a gradient by default");
            assert(text_style("inked", #{ color: "gold" }).style.color == "gold", "a colour over a gradient");
            // As a lock's source, the way the chain editor writes it.
            let sourced = new_lock("inks")
                .source("hand", "style", "hand", #{ options: #{ color: #{ colors: ["red", "blue"], vertical: true } } })
                .source("words", "content", "lorem")
                .attach("note", "styled_text", #{ width: 500.0 })
                .connect("words.value", "note.text")
                .connect("hand.value", "note.style");
            assert(sourced.run().canvases("note").len() == 1, "a source's gradient");
            let c = new_canvas();
            c.add(write_line("Lorem", vec2(0, 0), data));
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "#);
        assert!(entry.is_ok(), "{:?}", entry.err());
        // The editor's way: the default as data, and data given back as JSON.
        let info = engine.describe_text_style("inked").unwrap();
        assert_eq!(info.options["color"]["vertical"], serde_json::json!(true));
        assert_eq!(info.options["color"]["colors"][1], serde_json::json!([1.0, "rgba(0, 0, 255, 1)"]));
        let colors = serde_json::json!({ "color": { "colors": ["red", [0.7, "gold"], "blue"], "center": [40, 40], "radius": 50 } });
        let page = preview(&mut engine, "hand", &colors).unwrap();
        assert!(page.page_count() == 1 && page.empty_canvases().is_empty());
        let given_back = serde_json::json!({ "color": info.options["color"] });
        assert!(preview(&mut engine, "inked", &given_back).is_ok());
    }

    #[test]
    fn a_style_file_takes_every_setting() {
        const EVERYTHING: &str = r##"{
            "color": "darkred", "brush": "Pencil", "brush_size": 2, "min_pressure": 0.2, "max_pressure": 0.9, "pressure_profile": "taper",
            "pressure_wander": 0.2, "pressure_wander_length": 8, "pool": 0.4, "pool_depth": 0.25, "color_jitter": 0.05,
            "passes": 2, "pass_end_jitter": 1, "stroke_point_jitter": 0.5, "glyph_position_jitter": [0.1, 0.2], "glyph_rotation_jitter": 0.03, "glyph_scale_jitter": 0.1, "baseline_wander": 0.1,
            "glyphset": "plain", "size": 30, "glyph_scale_factor": 1.5, "glyph_outline_gap": 0.1, "glyph_tracking": 0.05, "space_width": 0.4, "skew": 0.2, "joined": false,
            "underline": { "count": 2, "spacing": 0.3, "style": { "color": { "colors": ["red", "blue"] }, "glyph_position_jitter": 0.1 } },
            "strike": 1,
            "line_height": 1.3, "paragraph_gap": 0.8, "indent": 1.5, "align": "justify", "valign": "center", "fit": true, "layout": "words", "angle": 0.1,
            "split_words": 3, "split_padding": 0.3, "break_words": true, "clearance": 5, "line_gap_jitter": 2, "line_start_jitter": 4, "overhang": 3,
            "spans": {
                "strong": { "type": { "glyphset": "copper", "underline": { "style": { "color": { "colors": ["red", "blue"] } } } }, "style": { "baseline_wander": 0.2 } }
            }
        }"##;
        let mut engine = engine(&[("everything", EVERYTHING)]);
        let entry = engine.build_entry_str(r#"
            let s = text_style("everything");
            assert(s.style.pool_depth == 0.25 && s.style.pressure_wander_length == 8.0, "the style's");
            assert(s.style.glyph_position_jitter == [3.0, 6.0], "in glyph scales, either way");
            assert(s.type.glyph_scale_factor == 1.5 && s.type.space_width == 0.4 && s.type.joined == false, "the type's");
            assert(s.type.underline.count == 2 && type_of(s.type.underline.style.color) == "Gradient", "a rule's style, its gradient made one");
            assert(s.type.underline.style.glyph_position_jitter == 3.0, "a rule's style, in glyph scales");
            assert(s.paragraph.indent == 1.5 && s.paragraph.valign == "center" && s.paragraph.split_words == 3, "the paragraph's");
            assert(type_of(s.spans.strong.type.glyphset) == "Glyphset", "a span's glyphset, by name");
            assert(s.spans.strong.style.baseline_wander == 6.0, "a span's style, in glyph scales");
            let pages = new_layout("flowing", |n, info| flow("body")).typeset(#{ width: 900, height: 900, flows: #{ body: markdown("Lorem **ipsum**.") }, roles: #{ body: s } });
            journal_entry.add(pages);
        "#);
        assert!(entry.is_ok(), "{:?}", entry.err());
    }

    #[test]
    fn style_file_mistakes_are_named() {
        for (script, want) in [
            (r#"text_style("nope")"#, "text_style: no text style named 'nope'"),
            (r#"text_style("hand", #{ width: 3 })"#, "text_style: text style 'hand' has no option 'width'; its options: color, size"),
            (r#"text_style("hand", #{ size: "big" })"#, "text_style: text style 'hand': option 'size' should be like its default"),
            (r#"text_style("hand", #{ size: new_gradient(["red", "blue"]) })"#, "option 'size' should be like its default"),
            (r#"text_style("hand", #{ color: 3 })"#, "option 'color' should be like its default"),
            (r#"text_style("hand", #{ color: #{ to: [1, 1] } })"#, "option 'color': a gradient written as a map needs 'colors'"),
            (r#"text_style("hand", #{ color: #{ colors: "red" } })"#, "a gradient's 'colors' should be an array"),
            (r#"text_style("hand", #{ color: #{ colors: ["red"] } })"#, "option 'color': new_gradient: needs at least two colours"),
            (r#"text_style("keys")"#, "text_style: text style 'keys': unknown option 'font'"),
            (r#"text_style("broken")"#, "isn't JSON"),
            (r#"text_style("list")"#, "should be a JSON object"),
            (r#"text_style("about")"#, "text style 'about': 'description' should be a string"),
            (r#"text_style("wander")"#, "text style 'wander': 'baseline_wander' is in glyph scales, so it needs a 'size'"),
            (r#"text_style("pressure")"#, "text style 'pressure' 'style': 'min_pressure' must be from 0 to 1"),
            (r#"text_style("glyphs")"#, "text style 'glyphs': glyphset: "),
            (r#"text_style("scale")"#, "text style 'scale': unknown option 'glyph_scale'"),
            (r#"text_style("depth")"#, "'pool_depth' is a fraction of the height"),
            (r#"text_style("padding")"#, "'split_padding' needs 'split_words'"),
            (r#"text_style("span_glyphs")"#, "text style 'span_glyphs': glyphset: "),
        ] {
            let mut engine = engine(&[
                ("hand", HAND),
                ("keys", r#"{ "font": 1 }"#),
                ("broken", "fn style(o) { #{} }"),
                ("list", "[]"),
                ("about", r#"{ "description": 3 }"#),
                ("wander", r#"{ "baseline_wander": 0.1 }"#),
                ("pressure", r#"{ "min_pressure": 3 }"#),
                ("glyphs", r#"{ "glyphset": "nope" }"#),
                ("scale", r#"{ "glyph_scale": 3 }"#),
                ("depth", r#"{ "pool_depth": 3 }"#),
                ("padding", r#"{ "split_padding": 3 }"#),
                ("span_glyphs", r#"{ "spans": { "x": { "type": { "glyphset": "nope" } } } }"#),
            ]);
            let err = engine.build_entry_str(script).expect_err(script);
            assert!(err.contains(want), "{}\n  wanted: {}\n  got: {}", script, want, err);
        }
    }
}
