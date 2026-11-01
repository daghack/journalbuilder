//! Text style scripts, as styles were before they were JSON files
//! (`styles/<name>.rhai`, defining `options()`, `style(o)` and `example()`),
//! made style files, for the journals that still have them.
//!
//! A script is run, and what it gives written down. A file then says less
//! than a script can, so the file is read back and given the same options as
//! the script (its defaults, twice the size, another colour): whatever comes
//! out differently is a problem, as is an option a file can't take.

use std::collections::BTreeMap;

use rhai::{Array, Dynamic, Engine, Map, Scope, AST};
use serde_json::{json, Value};

use super::illustrations;
use super::locks;
use super::options::{declared_defaults, unwrap_runtime, Settings};
use super::text_settings::{ParagraphStyle, TypeStyle, ITALIC_SKEW};
use super::text_styles::{whole, TextStyleFile, IN_GLYPH_SCALES};
use super::Style;
use crate::journal::fonts::{Font, FontBook};

/// What a style file's options are.
const FILE_OPTIONS: [&str; 2] = ["size", "color"];
/// The colour a script is tried with.
const TRIED_COLOUR: &str = "#123456";

/// A style script as a style file.
#[derive(Debug)]
pub struct ConvertedStyle {
    /// The file: what the script gives, as near as a file can say it.
    pub file: String,
    /// What the file doesn't do that the script did.
    pub problems: Vec<String>,
}

/// The script `name`, with text `source`, as a style file. It fails if the
/// script can't be run.
pub(super) fn convert(engine: &Engine, book: &FontBook, name: &str, source: &str) -> Result<ConvertedStyle, String> {
    let what = format!("text style '{}'", name);
    let ast = engine.compile(source).map_err(|e| format!("{}: {}", what, e))?;
    if !ast.iter_functions().any(|f| f.name == "style" && f.params.len() <= 1) {
        return Err(format!("{}: it doesn't define style(o)", what));
    }
    let defaults = declared_defaults(engine, &ast, &what)?;
    let given = run(engine, &ast, &what, &defaults)?;

    let mut problems = Vec::new();
    for (key, value) in &defaults {
        if !FILE_OPTIONS.contains(&key.as_str()) {
            problems.push(format!("its option '{}' (by default {}) would be gone: a style file's options are size and color", key, value));
        }
    }
    let size = given["type"].read_lock::<TypeStyle>().and_then(|t| t.has_size().then(|| number(&t.get("glyph_scale")))).flatten();
    let mut file = serde_json::Map::new();
    let description = illustrations::description(source, name);
    if !description.is_empty() {
        file.insert("description".into(), json!(description));
    }
    if let Some(example) = example_of(engine, &ast, &what)? {
        file.insert("example".into(), json!(example));
    }
    write_style(&mut file, &given["style"], size, &mut problems);
    write_type(&mut file, book, &given["type"], size, &mut problems);
    write_paragraph(&mut file, &given["paragraph"]);
    if let Some(spans) = given.get("spans") {
        file.insert("spans".into(), written_spans(spans, size, &mut problems));
    }
    let text = serde_json::to_string_pretty(&Value::Object(file)).map_err(|e| e.to_string())? + "\n";

    // Script and file, side by side.
    let written = TextStyleFile::read(name, &text, "the file it makes")?;
    let mut tries = vec![(String::new(), defaults.clone())];
    if let Some(size) = defaults.get("size").filter(|size| size.is_int() || size.is_float()).map(number) {
        let mut doubled = defaults.clone();
        doubled.insert("size".into(), Dynamic::from_float(size.unwrap_or(1.0) * 2.0));
        tries.push((format!("at size {}, ", size.unwrap_or(1.0) * 2.0), doubled));
    }
    if defaults.contains_key("color") {
        let mut coloured = defaults.clone();
        coloured.insert("color".into(), TRIED_COLOUR.into());
        tries.push((format!("with color {}, ", TRIED_COLOUR), coloured));
    }
    if let Some(difference) = differences(&given, &run(engine, &ast, &what, &defaults)?).into_iter().next() {
        problems.push(format!("it gives something different each time it runs: {}", difference));
    }
    for (when, options) in tries {
        let script = run(engine, &ast, &what, &options);
        let options: Map = options.into_iter().filter(|(key, _)| FILE_OPTIONS.contains(&key.as_str())).collect();
        match (script, written.bundle(book, &options)) {
            (Ok(script), Ok(file)) => problems.extend(differences(&script, &file).into_iter().map(|d| format!("{}{}", when, d))),
            (Err(e), _) => problems.push(format!("{}the script fails: {}", when, e)),
            (_, Err(e)) => problems.push(format!("{}the file fails: {}", when, e)),
        }
    }
    problems.dedup();
    Ok(ConvertedStyle { file: text, problems })
}

/// What the script gives with options `o`, made whole.
fn run(engine: &Engine, ast: &AST, what: &str, o: &Map) -> Result<Map, String> {
    let takes_options = ast.iter_functions().any(|f| f.name == "style" && f.params.len() == 1);
    let value = if takes_options {
        engine.call_fn::<Dynamic>(&mut Scope::new(), ast, "style", (Dynamic::from_map(o.clone()),))
    } else {
        engine.call_fn::<Dynamic>(&mut Scope::new(), ast, "style", ())
    }.map_err(|e| format!("{} style(o): {}", what, e))?;
    let type_name = value.type_name();
    let map = value.try_cast::<Map>().ok_or_else(|| format!("{}: style(o) should return #{{ style, type, paragraph, spans }}, got {}", what, type_name))?;
    whole(&format!("{} style(o)", what), map).map_err(|e| unwrap_runtime(&e))
}

/// The text the script previews with, if it isn't the one previews use anyway.
fn example_of(engine: &Engine, ast: &AST, what: &str) -> Result<Option<String>, String> {
    if !ast.iter_functions().any(|f| f.name == "example" && f.params.is_empty()) {
        return Ok(None);
    }
    let value = engine.call_fn::<Dynamic>(&mut Scope::new(), ast, "example", ()).map_err(|e| format!("{} example(): {}", what, e))?;
    let type_name = value.type_name();
    let example = value.into_string().map_err(|_| format!("{}: example() should return the text to preview it with, got {}", what, type_name))?;
    let lorem = engine.eval::<String>(r#"content("lorem")"#).ok();
    Ok(Some(example).filter(|example| Some(example) != lorem.as_ref()))
}

fn number(value: &Dynamic) -> Option<f64> {
    value.as_float().ok().or_else(|| value.as_int().ok().map(|n| n as f64))
}

/// A number without the noise of having been a float.
fn tidy(n: f64) -> f64 {
    (n * 1e6).round() / 1e6
}

/// A setting's value as JSON: a whole style or type by its settings, a
/// glyphset by its name, a gradient as data.
fn plain(value: &Dynamic) -> Value {
    if let Some(style) = value.read_lock::<Style>() {
        return Value::Object(Style::KEYS.iter().map(|&key| (key.to_string(), plain(&style.get(key)))).collect());
    }
    if let Some(type_) = value.read_lock::<TypeStyle>() {
        // Its `italic` is its `skew` again; what it leaves unset, it leaves.
        let set = TypeStyle::KEYS.iter().filter(|&&key| key != "italic").map(|&key| (key, type_.get(key))).filter(|(_, value)| !value.is_unit());
        return Value::Object(set.map(|(key, value)| (key.to_string(), plain(&value))).collect());
    }
    if let Some(font) = value.read_lock::<Font>() {
        return json!(font.name());
    }
    if let Some(items) = value.read_lock::<Array>() {
        return Value::Array(items.iter().map(plain).collect());
    }
    if let Some(map) = value.read_lock::<Map>() {
        return Value::Object(map.iter().map(|(key, value)| (key.to_string(), plain(value))).collect());
    }
    match value.as_float() {
        Ok(x) => json!(tidy(x)),
        Err(_) => locks::to_json(value),
    }
}

/// Style settings as a file writes them: what's in glyph scales, in glyph
/// scales.
fn in_file_units(settings: &mut serde_json::Map<String, Value>, size: Option<f64>, problems: &mut Vec<String>) {
    for key in IN_GLYPH_SCALES {
        let Some(canvas_units) = settings.get_mut(key) else { continue };
        let Some(size) = size else {
            problems.push(format!("'{}' would be gone: a style file gives it in glyph scales, and this has no size", key));
            settings.remove(key);
            continue;
        };
        let scale = |v: &Value| v.as_f64().map_or(Value::Null, |v| json!(tidy(v / size)));
        let in_scales = match &*canvas_units {
            Value::Array(pair) => Value::Array(pair.iter().map(scale).collect()),
            single => scale(single),
        };
        *canvas_units = in_scales;
    }
}

fn write_style(file: &mut serde_json::Map<String, Value>, style: &Dynamic, size: Option<f64>, problems: &mut Vec<String>) {
    let style = style.read_lock::<Style>().expect("a whole bundle's style");
    let base = Style::default_style();
    let mut settings = serde_json::Map::new();
    for &key in Style::KEYS {
        let value = plain(&style.get(key));
        // What a file always says, as the style form writes one.
        if ["color", "brush", "brush_size"].contains(&key) || value != plain(&base.get(key)) {
            settings.insert(key.to_string(), value);
        }
    }
    in_file_units(&mut settings, size, problems);
    file.extend(settings);
}

fn write_type(file: &mut serde_json::Map<String, Value>, book: &FontBook, type_: &Dynamic, size: Option<f64>, problems: &mut Vec<String>) {
    let type_ = type_.read_lock::<TypeStyle>().expect("a whole bundle's type");
    let base = TypeStyle::empty();
    for &key in TypeStyle::KEYS {
        if key == "glyphset" || key == "italic" {
            continue;
        }
        let value = plain(&type_.get(key));
        if value == plain(&base.get(key)) {
            continue;
        }
        match key {
            "glyph_scale" => file.insert("size".into(), value),
            "skew" if value == json!(tidy(ITALIC_SKEW)) => file.insert("italic".into(), json!(true)),
            "underline" | "strike" => file.insert(key.into(), written_rule(&type_.get(key), size, problems)),
            _ => file.insert(key.into(), value),
        };
    }
    // A glyphset changed by the script, as the type's settings could change it.
    let Some(font) = type_.get("glyphset").try_cast::<Font>() else { return };
    file.insert("glyphset".into(), json!(font.name()));
    let Some(named) = book.font(font.name()) else {
        problems.push(format!("its glyphset '{}' isn't one a file could name", font.name()));
        return;
    };
    let changed = |key: &str, value: Value, named: Value| (type_.get(key).is_unit() && value != named).then(|| (key.to_string(), value));
    file.extend([
        changed("glyph_outline_gap", json!(tidy(font.character_padding as f64)), json!(tidy(named.character_padding as f64))),
        changed("space_width", json!(tidy(font.space_width as f64)), json!(tidy(named.space_width as f64))),
        changed("skew", json!(tidy(font.skew as f64)), json!(tidy(named.skew as f64))),
        changed("joined", json!(font.joined), json!(named.joined)),
    ].into_iter().flatten());
}

/// Lines under or through text, as a file writes them.
fn written_rule(rule: &Dynamic, size: Option<f64>, problems: &mut Vec<String>) -> Value {
    let Value::Object(mut settings) = plain(rule) else { return plain(rule) };
    if settings.len() == 1 {
        return settings.remove("count").unwrap_or(json!(1));
    }
    if let Some(Value::Object(style)) = settings.get_mut("style") {
        in_file_units(style, size, problems);
    }
    Value::Object(settings)
}

fn write_paragraph(file: &mut serde_json::Map<String, Value>, paragraph: &Dynamic) {
    let paragraph = paragraph.read_lock::<ParagraphStyle>().expect("a whole bundle's paragraph");
    let base = ParagraphStyle::default();
    for &key in ParagraphStyle::KEYS {
        let value = plain(&paragraph.get(key));
        if value != plain(&base.get(key)) {
            file.insert(key.to_string(), value);
        }
    }
    // Its default is as good as unset to `get`, which text boxes tell apart.
    if let (Some(height), None) = (paragraph.set_line_height(), base.set_line_height()) {
        file.insert("line_height".into(), json!(tidy(height as f64)));
    }
}

/// A bundle's spans, as a file writes them.
fn written_spans(spans: &Dynamic, size: Option<f64>, problems: &mut Vec<String>) -> Value {
    let mut spans = plain(spans);
    for span in spans.as_object_mut().into_iter().flat_map(|spans| spans.values_mut()) {
        if let Some(style) = span.get_mut("style").and_then(Value::as_object_mut) {
            in_file_units(style, size, problems);
        }
    }
    spans
}

/// What a bundle's settings come to, named, for telling two apart.
fn facts(bundle: &Map) -> BTreeMap<String, Value> {
    let mut facts = BTreeMap::new();
    let style = bundle["style"].read_lock::<Style>().expect("a whole bundle's style");
    for &key in Style::KEYS {
        facts.insert(key.to_string(), plain(&style.get(key)));
    }
    let type_ = bundle["type"].read_lock::<TypeStyle>().expect("a whole bundle's type");
    for key in ["glyph_scale", "glyph_scale_factor", "glyph_tracking", "underline", "strike"] {
        facts.insert(key.to_string(), plain(&type_.get(key)));
    }
    if let Ok(font) = type_.glyphset("a text style") {
        facts.insert("glyphset".into(), font_facts(&font));
    }
    let paragraph = bundle["paragraph"].read_lock::<ParagraphStyle>().expect("a whole bundle's paragraph");
    for &key in ParagraphStyle::KEYS {
        facts.insert(key.to_string(), plain(&paragraph.get(key)));
    }
    facts.insert("line_height".into(), paragraph.set_line_height().map_or(Value::Null, |h| json!(tidy(h as f64))));
    facts.insert("spans".into(), bundle.get("spans").map_or(Value::Null, plain));
    facts
}

/// What a glyphset is, as a type changes it and otherwise.
fn font_facts(font: &Font) -> Value {
    json!({
        "name": font.name(),
        "glyph_outline_gap": tidy(font.character_padding as f64),
        "space_width": tidy(font.space_width as f64),
        "skew": tidy(font.skew as f64),
        "joined": font.joined,
        "mapping": format!("{:?}", font.mapping),
        "fallback": font.fallback.as_deref().map(font_facts),
    })
}

/// Whether two settings come to the same, numbers but for rounding.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap_or(f64::NAN), b.as_f64().unwrap_or(f64::NAN));
            (a - b).abs() <= 1e-4 * a.abs().max(1.0)
        }
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b)),
        (Value::Object(a), Value::Object(b)) => a.len() == b.len() && a.iter().all(|(key, a)| b.get(key).is_some_and(|b| same(a, b))),
        _ => a == b,
    }
}

/// What differs between `script`'s bundle and `file`'s, said of the file.
fn differences(script: &Map, file: &Map) -> Vec<String> {
    let (script, file) = (facts(script), facts(file));
    let empty = Value::Null;
    let keys: std::collections::BTreeSet<&String> = script.keys().chain(file.keys()).collect();
    keys.into_iter().filter_map(|key| {
        let (wanted, got) = (script.get(key).unwrap_or(&empty), file.get(key).unwrap_or(&empty));
        (!same(wanted, got)).then(|| format!("'{}' would be {}, not {}", key, got, wanted))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::super::{JournalEngine, Resources};
    use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};
    use serde_json::{json, Value};
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::with_resources(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).unwrap()), Resources::library())
    }

    fn file(name: &str, script: &str) -> (Value, Vec<String>) {
        let converted = engine().convert_old_text_style(name, script).unwrap_or_else(|e| panic!("{}", e));
        (serde_json::from_str(&converted.file).unwrap(), converted.problems)
    }

    /// The library's and the showcase's styles, as they were.
    const COPPER_HEADING: &str = r##"// copper_heading: headings in the copper glyphset, centred, with a broad
// calligraphy nib at a constant pressure; strong text gone over twice.

fn options() {
	#{ size: 72.0, color: "#9b2318" }
}

fn style(o) {
	#{
		style: new_style(
			new_brush_style(o.color, "Calligraphy", 1.5, #{ min_pressure: 0.48, max_pressure: 0.96, pressure_profile: "constant" }),
			new_handwriting_style(),
		),
		type: new_type_style(glyphset("copper"), o.size),
		paragraph: #{ align: "center", line_height: 1.1 },
		spans: #{ strong: #{ style: #{ passes: 2, pass_end_jitter: 1.0 } } },
	}
}

fn example() {
	"Dolor Sit Amet"
}
"##;
    const PLAIN_HAND: &str = r##"// plain_hand: easy-reading text in the plain glyphset, written with an
// ink pen by a hand a little unsteady: each glyph a touch off true, each
// line wandering.

fn options() {
	#{ size: 26.0, color: "#2b1d12" }
}

fn style(o) {
	#{
		style: new_style(
			new_brush_style(o.color, "Ink Pen", 1.4, #{ min_pressure: 0.4, max_pressure: 0.8 }),
			new_handwriting_style(#{ glyph_rotation_jitter: 0.03, glyph_position_jitter: o.size * 0.02, baseline_wander: o.size * 0.08 }),
		),
		type: new_type_style(glyphset("plain"), o.size),
		paragraph: #{ line_height: 1.4 },
	}
}

fn example() {
	content("lorem")
}
"##;
    const MARGIN_NOTE: &str = r##"// margin_note: small red notes in the plain glyphset, leant a little, as
// if written in a margin: the showcase's own text style.

fn options() {
	#{ size: 22.0 }
}

fn style(o) {
	#{
		style: new_style(new_brush_style("#9b2318", "Ink Pen", 1.2), new_handwriting_style(#{ glyph_rotation_jitter: 0.04 })),
		type: new_type_style(glyphset("plain"), o.size, #{ italic: true }),
		paragraph: #{ line_height: 1.3 },
	}
}

fn example() {
	"nota bene: lorem ipsum dolor sit amet"
}
"##;

    #[test]
    fn the_styles_there_were_convert_to_the_files_there_are() {
        for (name, script, kept) in [
            ("copper_heading", COPPER_HEADING, include_str!("../../../webapp/library/styles/copper_heading.json")),
            ("plain_hand", PLAIN_HAND, include_str!("../../../webapp/library/styles/plain_hand.json")),
            ("margin_note", MARGIN_NOTE, include_str!("../../../webapp/journals/showcase/styles/margin_note.json")),
        ] {
            let (converted, problems) = file(name, script);
            assert_eq!(problems, Vec::<String>::new());
            let kept: Value = serde_json::from_str(kept).unwrap();
            assert!(super::same(&converted, &kept), "{}\n  against {}", converted, kept);
        }
    }

    #[test]
    fn a_gradient_default_stays_a_colour_option() {
        let (converted, problems) = file("inked", r##"
            fn options() { #{ size: 26.0, color: new_gradient([[0.0, "hsl(16, 57%, 24%)"], [0.9, "#2b1d12"], [1.0, "#9b2318"]], #{ vertical: true }) } }
            fn style(o) { #{ style: new_style(new_brush_style(o.color, "Ink Pen", 1.4), new_handwriting_style()), type: new_type_style(glyphset("plain"), o.size) } }
        "##);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!(converted["color"]["vertical"], json!(true));
        assert_eq!(converted["color"]["colors"][2], json!([1.0, "rgba(155, 35, 24, 1)"]));
    }

    #[test]
    fn every_kind_of_setting_comes_across() {
        let (converted, problems) = file("lined", r##"// lined: a bit of everything.
            fn options() { #{ size: 30.0 } }
            fn style(o) {
                #{
                    style: #{ pool: 0.4, pool_depth: 0.25, pressure_wander: 0.2, pressure_wander_length: 8.0, glyph_position_jitter: [o.size * 0.1, o.size * 0.2] },
                    type: new_type_style(glyphset("plain", #{ space_width: 0.6 }), o.size, #{
                        glyph_scale_factor: 1.5,
                        underline: #{ count: 2, spacing: 0.3, style: #{ color: new_gradient(["red", "blue"]) } },
                        strike: true,
                        joined: false,
                    }),
                    paragraph: #{ indent: 1.5, paragraph_gap: 0.8, valign: "center", fit: true, layout: "words", angle: 0.1, split_words: 3, split_padding: 0.3, break_words: true, clearance: 5, overhang: 2 },
                    spans: #{ warn: #{ style: new_style(new_brush_style("red", "Pencil", 2.0), new_handwriting_style()) } },
                }
            }
        "##);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!(converted["description"], json!("a bit of everything."));
        assert_eq!(converted["pool_depth"], json!(0.25));
        assert_eq!(converted["pressure_wander_length"], json!(8.0));
        assert_eq!(converted["glyph_position_jitter"], json!([0.1, 0.2]), "in glyph scales");
        assert_eq!(converted["space_width"], json!(0.6), "as the glyphset was changed");
        assert_eq!(converted["glyph_scale_factor"], json!(1.5));
        assert_eq!(converted["underline"]["count"], json!(2));
        assert_eq!(converted["underline"]["style"]["color"]["colors"][1], json!([1.0, "rgba(0, 0, 255, 1)"]));
        assert_eq!(converted["strike"], json!(1));
        assert_eq!(converted["joined"], json!(false));
        assert_eq!(converted["indent"], json!(1.5));
        assert_eq!(converted["split_words"], json!(3));
        assert_eq!(converted["break_words"], json!(2));
        assert_eq!(converted["spans"]["warn"]["style"]["brush"], json!("Pencil"));
    }

    #[test]
    fn what_a_file_cant_say_is_named() {
        const TYPE: &str = r#"type: new_type_style(glyphset("plain"), o.size)"#;
        for (script, wanted) in [
            (format!(r#"fn options() {{ #{{ size: 20.0, weight: 2.0 }} }} fn style(o) {{ #{{ style: #{{ passes: 2 }}, {TYPE} }} }}"#), "its option 'weight' (by default 2.0) would be gone"),
            (format!(r#"fn options() {{ #{{ size: 20.0 }} }} fn style(o) {{ #{{ style: #{{ brush_size: o.size * 0.1 }}, {TYPE} }} }}"#), "at size 40, 'brush_size' would be 2.0, not 4.0"),
            (format!(r#"fn options() {{ #{{ size: 20.0 }} }} fn style(o) {{ #{{ style: #{{ baseline_wander: 3.0 }}, {TYPE} }} }}"#), "at size 40, 'baseline_wander' would be 6.0, not 3.0"),
            (format!(r#"fn options() {{ #{{ size: 20.0, color: "red" }} }} fn style(o) {{ #{{ style: #{{ color: "blue" }}, {TYPE} }} }}"#), "with color #123456, 'color' would be \"#123456\", not \"blue\""),
            (r#"fn options() { #{ size: 20.0 } } fn style(o) { #{ type: new_type_style(ternary_glyphset("plain"), o.size) } }"#.to_string(), "'glyphset' would be"),
            (format!(r#"fn options() {{ #{{ size: 20.0 }} }} fn style(o) {{ #{{ style: #{{ passes: 1 + floor(random() * 60).to_int() }}, {TYPE} }} }}"#), "something different each time"),
        ] {
            let got = file("old", &script).1.join("\n");
            assert!(got.contains(wanted), "{}\n  wanted: {}\n  got: {}", script, wanted, got);
        }
    }

    #[test]
    fn a_style_that_wont_run_fails() {
        for (script, wanted) in [
            ("fn style(o) {", "text style 'old'"),
            ("let x = 1;", "doesn't define style(o)"),
            (r#"fn style() { throw "no"; }"#, "style(o)"),
        ] {
            let err = engine().convert_old_text_style("old", script).expect_err(script);
            assert!(err.contains(wanted), "{}\n  wanted: {}\n  got: {}", script, wanted, err);
        }
    }
}
