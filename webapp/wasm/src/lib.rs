//! The journal editor's engine, for the browser: runs a journal's entries
//! and shows their pages as SVG (`evaluate`), and renders them with Darkly
//! over WebGPU (`Renderer`), as images, a PDF or PNGs.
//!
//! The built-in library (lock, layout, illustration, chain and style files,
//! helpers, glyphsets and content) is compiled in (see
//! `rhaientries::embedded`). The journal being edited (`setJournal`) gives
//! its own files, which stand in front of the library's as a journal's
//! directory does.
//!
//! The editors use the rest, which answer in JSON: `previewSvg` draws a
//! script's first page without touching what `evaluate` keeps;
//! `libraryFiles` lists the library and `usedLibrary` the library files a
//! journal's entries use; the lock and chain editor has `lockTypes`,
//! `lockPorts`, `lockExamples`, `chainTypes`, `runChain`, `readChain` and
//! `valueKinds`; and the illustration, layout and style editors have
//! `illustrationTypes`, `illustrationPorts`, `illustrationPreview`,
//! `layoutTypes`, `layoutPreview`, `styleTypes` and `stylePreview`.

use scribe::journal::darkly;
use scribe::journal::fonts::FontBook;
use scribe::journal::pdf::PdfWriter;
use scribe::journal::publish::{self, Render, UNITS_PER_INCH};
use scribe::journal::rhaientries::{Entry, JournalEngine, LockLibrary, Resources};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

thread_local! {
    /// The last panic's message, for `lastPanic`.
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[wasm_bindgen(start)]
fn start() {
    std::panic::set_hook(Box::new(|info| {
        console_error_panic_hook::hook(info);
        LAST_PANIC.set(Some(info.to_string()));
    }));
}

/// What the last panic said, if one has happened. After a panic the
/// module can't be trusted, so callers report this and start afresh.
#[wasm_bindgen(js_name = lastPanic)]
pub fn last_panic() -> Option<String> {
    LAST_PANIC.with_borrow(|panic| panic.clone())
}

/// A page as the editor lists it. Its SVG comes from `pageSvg`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PageView {
    index: usize,
    label: String,
    /// The box of canvas units its SVG shows, `[x, y, width, height]`;
    /// `None` if nothing's drawn on it.
    view: Option<[f32; 4]>,
}

thread_local! {
    /// The entry `evaluate` last made, for `pageSvg`.
    static EVALUATED: RefCell<Option<Entry>> = const { RefCell::new(None) };
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScriptError {
    message: String,
    line: Option<usize>,
    column: Option<usize>,
}

/// What running a script made, and what it said along the way.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Evaluation {
    pages: Vec<PageView>,
    warnings: Vec<String>,
    prints: Vec<String>,
    missing_glyphs: Option<String>,
    error: Option<ScriptError>,
    millis: f64,
    /// The journal's files that stand in for the library's, as paths.
    overrides: Vec<String>,
}

/// The journal being edited, from `setJournal`.
#[derive(Default)]
struct Journal {
    /// Its own files, entries aside, by path from its directory:
    /// `locks/x.rhai`, `glyphsets/y.json`, `content/z.md`.
    files: BTreeMap<String, String>,
    /// Counts changes, so renders can tell their cache is stale.
    version: u64,
}

thread_local! {
    static JOURNAL: RefCell<Journal> = RefCell::new(Journal::default());
}

/// Use `files` (path from the journal's directory -> text: `locks/x.rhai`,
/// `glyphsets/y.json`, `content/z.txt`; entries aren't needed) as the
/// journal's own files, in front of the library's, for everything run from
/// now on. Files in no resource directory are ignored.
#[wasm_bindgen(js_name = setJournal)]
pub fn set_journal(files: JsValue) -> Result<(), JsError> {
    let files: BTreeMap<String, String> = if files.is_undefined() || files.is_null() { BTreeMap::new() } else { serde_wasm_bindgen::from_value(files)? };
    JOURNAL.with_borrow_mut(|journal| {
        journal.files = files;
        journal.version += 1;
    });
    Ok(())
}

/// `files` (path -> text) as a journal's own resources, with nothing under
/// them.
fn journal_files<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Resources {
    let mut journal = Resources::none();
    for (path, text) in files {
        let Some((kind, file)) = path.split_once('/') else { continue };
        let Some((name, ext)) = file.rsplit_once('.') else { continue };
        let sources = match (kind, ext) {
            ("locks", "rhai") => &mut journal.locks,
            ("layouts", "rhai") => &mut journal.layouts,
            ("illustrations", "rhai") => &mut journal.illustrations,
            ("chains", "rhai") => &mut journal.chains,
            ("styles", "rhai") => &mut journal.styles,
            ("lib", "rhai") => &mut journal.lib,
            ("glyphsets", "json") => &mut journal.glyphsets,
            ("content", "txt") => &mut journal.text,
            ("content", "md") => &mut journal.markdown,
            ("content", "json") => &mut journal.symbols,
            _ => continue,
        };
        *sources = sources.clone().with_overlay([(name.to_string(), text.to_string())]);
    }
    journal
}

/// The journal's resources, with `extra` files (path -> text) over its own,
/// in front of `library`.
fn resources_over(library: Resources, extra: &[(&str, &str)]) -> Resources {
    JOURNAL.with_borrow(|journal| {
        let files = journal.files.iter().map(|(path, text)| (path.as_str(), text.as_str())).chain(extra.iter().copied());
        journal_files(files).over(library)
    })
}

/// The journal's resources, with `extra` files over its own, in front of
/// the compiled-in library.
fn resources(extra: &[(&str, &str)]) -> Resources {
    resources_over(Resources::embedded(), extra)
}

/// The compiled-in library's glyphsets, loaded.
fn library_glyphsets() -> Rc<FontBook> {
    Rc::new(Resources::embedded().glyphset_book().expect("the compiled-in glyphsets load"))
}

/// `resources`' glyphsets, or the library's, with a warning, if the
/// journal's don't load.
fn glyphsets(resources: &Resources) -> (Rc<FontBook>, Option<String>) {
    match resources.glyphset_book() {
        Ok(book) => (Rc::new(book), None),
        Err(e) => (library_glyphsets(), Some(format!("the journal's glyphsets don't load, so only the library's are used: {}", e))),
    }
}

/// Run a journal entry with `resources`: its entry, if it ran, and what it
/// said.
fn run_with(source: &str, resources: Resources) -> (Result<Entry, ScriptError>, Evaluation) {
    let (book, glyphsets_error) = glyphsets(&resources);
    let overrides = resources.overrides();
    let mut engine = JournalEngine::with_resources(book.clone(), resources);
    let prints: Rc<RefCell<Vec<String>>> = Rc::default();
    let printed = prints.clone();
    engine.on_print(move |text| printed.borrow_mut().push(text.to_string()));
    let entry = engine.build_entry(source).map_err(|err| ScriptError { message: err.message, line: err.line, column: err.column });
    let mut warnings: Vec<String> = glyphsets_error.into_iter().collect();
    warnings.extend(engine.warnings());
    if let Ok(entry) = &entry {
        if entry.page_count() == 0 {
            warnings.push("the script made no pages; add them with journal_entry.add(page)".to_string());
        }
        let empty = entry.empty_canvases();
        if !empty.is_empty() {
            warnings.push(format!("{} empty canvas(es), with nothing drawn on them: {}", empty.len(), empty.join(", ")));
        }
    }
    let evaluation = Evaluation {
        pages: Vec::new(),
        warnings,
        prints: prints.take(),
        missing_glyphs: book.missing_glyph_report(),
        error: None,
        millis: 0.0,
        overrides,
    };
    (entry, evaluation)
}

/// Run a journal entry with the journal's files.
fn run(source: &str) -> (Result<Entry, ScriptError>, Evaluation) {
    run_with(source, resources(&[]))
}

/// Run a journal entry, keeping the entry it makes for `pageSvg`.
/// Returns `{ pages: [{ index, label, view }], warnings, prints,
/// missingGlyphs, error: { message, line, column }, millis, overrides }`.
#[wasm_bindgen]
pub fn evaluate(source: &str) -> Result<JsValue, JsError> {
    let started = js_sys::Date::now();
    let (entry, mut evaluation) = run(source);
    let entry = match entry {
        Ok(entry) => {
            evaluation.pages = entry.selected_pages(None)
                .map(|(index, page)| PageView { index, label: page.label(index), view: publish::page_view(page) })
                .collect();
            Some(entry)
        }
        Err(error) => {
            evaluation.error = Some(error);
            None
        }
    };
    EVALUATED.with_borrow_mut(|evaluated| *evaluated = entry);
    evaluation.millis = js_sys::Date::now() - started;
    Ok(serde_wasm_bindgen::to_value(&evaluation)?)
}

/// Page `index` of the entry `evaluate` last made, as SVG in canvas units
/// (see `PageView::view`); `undefined` if nothing's drawn on it or there's
/// no such page.
#[wasm_bindgen(js_name = pageSvg)]
pub fn page_svg(index: usize) -> Option<String> {
    EVALUATED.with_borrow(|entry| {
        let (_, page) = entry.as_ref()?.selected_pages(None).nth(index)?;
        publish::page_svg(page, &page.label(index)).map(|(svg, _)| svg)
    })
}

/// `files` (JSON, path -> text) as pairs, or none for null or nothing.
fn file_pairs(files: Option<String>) -> Result<Vec<(String, String)>, String> {
    match files.as_deref().map(str::trim) {
        None | Some("") | Some("null") => Ok(Vec::new()),
        Some(text) => serde_json::from_str::<BTreeMap<String, String>>(text)
            .map(|files| files.into_iter().collect())
            .map_err(|e| format!("files: {}", e)),
    }
}

/// An entry's pages as SVG, as JSON: `{ pages: [{ svg, view }] }`.
fn pages_json(entry: &Entry) -> Value {
    let pages: Vec<Value> = entry.selected_pages(None).map(|(index, page)| {
        json!({ "svg": publish::page_svg(page, &page.label(index)).map(|(svg, _)| svg), "view": publish::page_view(page) })
    }).collect();
    json!({ "pages": pages })
}

/// A script's first page as SVG, for previews, as JSON: `{ svg, view,
/// warnings, missingGlyphs, error }`. `files` (JSON, path -> text, or
/// null) stand in for the journal's files of the same paths, as an editor
/// shows them before they're kept. It leaves what `evaluate` keeps alone.
#[wasm_bindgen(js_name = previewSvg)]
pub fn preview_svg(source: &str, files: Option<String>) -> String {
    let files = match file_pairs(files) {
        Ok(files) => files,
        Err(e) => return json!({ "svg": null, "view": null, "warnings": [], "missingGlyphs": null, "error": { "message": e } }).to_string(),
    };
    let extra: Vec<(&str, &str)> = files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
    let (entry, evaluation) = run_with(source, resources(&extra));
    let (svg, view, error) = match entry {
        Ok(entry) => match entry.selected_pages(None).next() {
            Some((index, page)) => (publish::page_svg(page, &page.label(index)).map(|(svg, _)| svg), publish::page_view(page), None),
            None => (None, None, None),
        },
        Err(error) => (None, None, Some(error)),
    };
    let preview = json!({
        "svg": svg, "view": view, "warnings": evaluation.warnings,
        "missingGlyphs": evaluation.missing_glyphs, "error": error.map(|e| json!({ "message": e.message, "line": e.line, "column": e.column })),
    });
    preview.to_string()
}

/// Every file of the compiled-in library, as JSON: `{ path: text }`, paths
/// as a journal's would be (`locks/huffman.rhai`, `lib/names.rhai`).
#[wasm_bindgen(js_name = libraryFiles)]
pub fn library_files() -> String {
    let library = Resources::embedded();
    let mut files = serde_json::Map::new();
    for (kind, sources) in library.kinds() {
        let names = sources.names().unwrap_or_default();
        for name in names {
            if let Ok(text) = sources.read(&name) {
                files.insert(format!("{}/{}.{}", kind, name, sources.ext()), Value::String(text));
            }
        }
    }
    Value::Object(files).to_string()
}

/// The library files the journal's entries (`entries`, JSON: an array of
/// their scripts) use, run with the journal's files: what an export
/// bundles. JSON: `{ paths: [...] }` (`locks/huffman.rhai`), or `{ error }`
/// naming the first entry that fails.
#[wasm_bindgen(js_name = usedLibrary)]
pub fn used_library(entries: &str) -> String {
    answer((|| {
        let entries: Vec<String> = serde_json::from_str(entries).map_err(|e| format!("entries: {}", e))?;
        let (library, used) = Resources::embedded().recording();
        let resources = resources_over(library, &[]);
        let (book, _) = glyphsets(&resources);
        for (i, source) in entries.iter().enumerate() {
            let mut engine = JournalEngine::with_resources(book.clone(), resources.clone());
            engine.build_entry(source).map_err(|e| format!("entry {}: {}", i, e.message))?;
        }
        let paths = used.paths(Some((&book, &Resources::embedded().glyphsets)));
        Ok(json!({ "paths": paths }))
    })())
}

/// A lock library over the journal's lock and chain files, and `chains`
/// over them, in front of the library's.
fn locks(chains: &[(&str, &str)]) -> Rc<LockLibrary> {
    let chains: Vec<(String, &str)> = chains.iter().map(|(name, text)| (format!("chains/{}.rhai", name), *text)).collect();
    let extra: Vec<(&str, &str)> = chains.iter().map(|(path, text)| (path.as_str(), *text)).collect();
    Rc::new(LockLibrary::new(&resources(&extra)))
}

/// `result` as JSON, or `{ error }`.
fn answer<T: Serialize>(result: Result<T, String>) -> String {
    match result {
        Ok(value) => serde_json::to_string(&value).unwrap_or_else(|e| json!({ "error": e.to_string() }).to_string()),
        Err(error) => json!({ "error": error }).to_string(),
    }
}

fn parse_json(text: &str, what: &str) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(text).map_err(|e| format!("{}: {}", what, e))
}

/// Whether the journal has file `path` of its own, and the library one.
fn whose(path: &str) -> Value {
    let own = JOURNAL.with_borrow(|journal| journal.files.contains_key(path));
    let builtin = LIBRARY_PATHS.with(|paths| paths.contains(path));
    json!({ "own": own, "builtin": builtin })
}

thread_local! {
    /// Every compiled-in library file's path.
    static LIBRARY_PATHS: std::collections::BTreeSet<String> = {
        let library = Resources::embedded();
        library.kinds().into_iter()
            .flat_map(|(kind, sources)| sources.names().unwrap_or_default().into_iter().map(move |name| format!("{}/{}.{}", kind, name, sources.ext())))
            .collect()
    };
}

/// `value` (a JSON object) with `own` and `builtin` from `whose(path)`.
fn marked(mut value: Value, path: &str) -> Value {
    if let (Value::Object(map), Value::Object(marks)) = (&mut value, whose(path)) {
        map.extend(marks);
    }
    value
}

/// Every lock type, as JSON: `[{ name, file, description, composite,
/// options: [{ name, kind, choices?, default? }], own, builtin }]`, or
/// `{ name, error, own, builtin }` for one that doesn't load. `own` if the
/// journal has the file, `builtin` if the library does.
#[wasm_bindgen(js_name = lockTypes)]
pub fn lock_types() -> String {
    answer(locks(&[]).describe_all().map(|types| {
        types.into_iter().map(|(name, info)| {
            let path = format!("locks/{}.rhai", name);
            match info {
                Ok(info) => marked(serde_json::to_value(info).unwrap_or(Value::Null), &path),
                Err(error) => marked(json!({ "name": name, "error": error }), &path),
            }
        }).collect::<Vec<Value>>()
    }))
}

/// A lock of `type_name` with `options` (JSON): `{ inputs, outputs,
/// options }`, ports as `[name, type]` pairs, or `{ error }`.
#[wasm_bindgen(js_name = lockPorts)]
pub fn lock_ports(type_name: &str, options: &str) -> String {
    answer(parse_json(options, "options").and_then(|options| locks(&[]).ports(type_name, &options)))
}

/// Run lock type `type_name`'s examples through lock and unlock, with
/// `source` standing in for its file if given: `{ examples }`, how many
/// passed, or `{ error }`, the first failure.
#[wasm_bindgen(js_name = lockExamples)]
pub fn lock_examples(type_name: &str, source: Option<String>) -> String {
    let path = format!("locks/{}.rhai", type_name);
    let extra: Vec<(&str, &str)> = source.as_deref().map(|source| (path.as_str(), source)).into_iter().collect();
    let library = Rc::new(LockLibrary::new(&resources(&extra)));
    answer(library.check_examples(type_name).map(|examples| json!({ "examples": examples })))
}

/// Every chain file, as JSON: `[{ name, description, own, builtin }]`, or
/// `{ name, error, own, builtin }` for one that doesn't load.
#[wasm_bindgen(js_name = chainTypes)]
pub fn chain_types() -> String {
    answer(locks(&[]).describe_chains().map(|chains| {
        chains.into_iter().map(|(name, info)| {
            let path = format!("chains/{}.rhai", name);
            match info {
                Ok(description) => marked(json!({ "name": name, "description": description }), &path),
                Err(error) => marked(json!({ "name": name, "error": error }), &path),
            }
        }).collect::<Vec<Value>>()
    }))
}

/// Run the chain file `source` with `options` and `inputs` (JSON,
/// "lock.port" -> `{ text }`, `{ json }` or `{ rhai }`), then unlock what
/// it gives: `{ outputs, trace, recovered, roundTrip, unlockError }`, each
/// value `{ type, text?, json, art? }` by port, or `{ error }`.
#[wasm_bindgen(js_name = runChain)]
pub fn run_chain(source: &str, options: &str, inputs: &str) -> String {
    const EDITING: &str = "editing";
    answer((|| {
        let options = parse_json(options, "options")?;
        let inputs = parse_json(inputs, "inputs")?;
        locks(&[(EDITING, source)]).run_chain(EDITING, &options, &inputs)
    })())
}

/// The chain file `name` read back for the editor (see
/// `LockLibrary::read_chain`), from `source` if given, or `{ error }`.
#[wasm_bindgen(js_name = readChain)]
pub fn read_chain(name: &str, source: Option<String>) -> String {
    let extra: Vec<(&str, &str)> = source.as_deref().map(|source| (name, source)).into_iter().collect();
    answer(locks(&extra).read_chain(name))
}

/// A journal engine with the journal's files, and `extra` (path -> text)
/// over them, for the illustration, layout and style editors.
fn engine(extra: &[(&str, &str)]) -> JournalEngine {
    let resources = resources(extra);
    let (book, _) = glyphsets(&resources);
    JournalEngine::with_resources(book, resources)
}

/// Every illustration type, as JSON: `[{ name, file, description, options,
/// inputs, example, own, builtin }]`, options its defaults and inputs
/// `[name, type]` pairs for them, or `{ name, error, own, builtin }` for one
/// that doesn't load.
#[wasm_bindgen(js_name = illustrationTypes)]
pub fn illustration_types() -> String {
    let engine = engine(&[]);
    let types: Vec<Value> = engine.illustration_names().into_iter().map(|name| {
        let path = format!("illustrations/{}.rhai", name);
        match engine.describe_illustration(&name) {
            Ok(info) => marked(serde_json::to_value(info).unwrap_or(Value::Null), &path),
            Err(error) => marked(json!({ "name": name, "error": error }), &path),
        }
    }).collect();
    Value::Array(types).to_string()
}

/// Illustration `type_name`'s inputs with `options` (JSON) over its
/// defaults: `{ inputs }`, `[name, type]` pairs, or `{ error }`.
#[wasm_bindgen(js_name = illustrationPorts)]
pub fn illustration_ports(type_name: &str, options: &str) -> String {
    answer(parse_json(options, "options").and_then(|options| {
        engine(&[]).illustration_inputs(type_name, &options).map(|inputs| json!({ "inputs": inputs }))
    }))
}

/// What illustration `type_name` draws, as SVG: `{ pages: [{ svg, view }] }`,
/// or `{ error }`. It draws `inputs` (JSON, name -> value as a run shows
/// it), or its own example() if null, with `options` (JSON) over its
/// defaults, and with `source` standing in for its file if given (as it's
/// edited).
#[wasm_bindgen(js_name = illustrationPreview)]
pub fn illustration_preview(type_name: &str, source: Option<String>, options: &str, inputs: Option<String>) -> String {
    answer((|| {
        let options = parse_json(options, "options")?;
        let inputs = inputs.map(|inputs| parse_json(&inputs, "inputs")).transpose()?;
        let path = format!("illustrations/{}.rhai", type_name);
        let extra: Vec<(&str, &str)> = source.as_deref().map(|source| (path.as_str(), source)).into_iter().collect();
        let entry = engine(&extra).render(type_name, &options, inputs.as_ref())?;
        Ok(pages_json(&entry))
    })())
}

/// Every layout, as JSON: `[{ name, description, options, example, own,
/// builtin }]`, or `{ name, error, own, builtin }` for one that doesn't
/// load.
#[wasm_bindgen(js_name = layoutTypes)]
pub fn layout_types() -> String {
    let engine = engine(&[]);
    let types: Vec<Value> = engine.layout_names().into_iter().map(|name| {
        let path = format!("layouts/{}.rhai", name);
        match engine.describe_layout(&name) {
            Ok(info) => marked(serde_json::to_value(info).unwrap_or(Value::Null), &path),
            Err(error) => marked(json!({ "name": name, "error": error }), &path),
        }
    }).collect();
    Value::Array(types).to_string()
}

/// Layout `name` typesetting its example(), with `options` (JSON) over its
/// defaults and `source` standing in for its file if given: `{ pages: [{
/// svg, view }] }`, or `{ error }`.
#[wasm_bindgen(js_name = layoutPreview)]
pub fn layout_preview(name: &str, source: Option<String>, options: &str) -> String {
    answer((|| {
        let options = parse_json(options, "options")?;
        let path = format!("layouts/{}.rhai", name);
        let extra: Vec<(&str, &str)> = source.as_deref().map(|source| (path.as_str(), source)).into_iter().collect();
        let entry = engine(&extra).preview_layout(name, &options)?;
        Ok(pages_json(&entry))
    })())
}

/// Every text style, as JSON: `[{ name, description, options, example,
/// own, builtin }]`, or `{ name, error, own, builtin }` for one that
/// doesn't load.
#[wasm_bindgen(js_name = styleTypes)]
pub fn style_types() -> String {
    let engine = engine(&[]);
    let types: Vec<Value> = engine.text_style_names().into_iter().map(|name| {
        let path = format!("styles/{}.rhai", name);
        match engine.describe_text_style(&name) {
            Ok(info) => marked(serde_json::to_value(info).unwrap_or(Value::Null), &path),
            Err(error) => marked(json!({ "name": name, "error": error }), &path),
        }
    }).collect();
    Value::Array(types).to_string()
}

/// Text style `name` writing its example() (or lorem ipsum) in a block
/// `width` wide, with `options` (JSON) over its defaults and `source`
/// standing in for its file if given: `{ pages: [{ svg, view }] }`, or
/// `{ error }`.
#[wasm_bindgen(js_name = stylePreview)]
pub fn style_preview(name: &str, source: Option<String>, options: &str, width: f64) -> String {
    answer((|| {
        let options = parse_json(options, "options")?;
        let path = format!("styles/{}.rhai", name);
        let extra: Vec<(&str, &str)> = source.as_deref().map(|source| (path.as_str(), source)).into_iter().collect();
        let entry = engine(&extra).preview_text_style(name, &options, width)?;
        Ok(pages_json(&entry))
    })())
}

/// Every value kind, as JSON: `[{ name, example }]`, example a rhai
/// expression that makes one (or null), for the chain editor to suggest as
/// a test value.
#[wasm_bindgen(js_name = valueKinds)]
pub fn value_kinds() -> String {
    let kinds: Vec<Value> = scribe::journal::kinds::kinds().iter()
        .map(|k| json!({ "name": k.name(), "example": k.example() }))
        .collect();
    Value::Array(kinds).to_string()
}

/// The name of every function scripts can call that journals add to rhai,
/// for the editor to highlight and complete.
#[wasm_bindgen(js_name = apiNames)]
pub fn api_names() -> Vec<String> {
    JournalEngine::embedded(library_glyphsets()).function_names().into_iter()
        .filter(|name| {
            let mut parts = name.split("::");
            parts.all(|part| part.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        })
        .collect()
}

/// How pages are rendered, as `journalbuild`'s flags set it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RenderOptions {
    scale: f32,
    supersample: f32,
    parchment: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { scale: 1.0, supersample: 1.2, parchment: false }
    }
}

impl RenderOptions {
    fn from_js(options: JsValue) -> Result<Render, JsError> {
        let options: RenderOptions = if options.is_undefined() || options.is_null() {
            RenderOptions::default()
        } else {
            serde_wasm_bindgen::from_value(options)?
        };
        if !(options.scale > 0.0 && options.scale <= 4.0) {
            return Err(JsError::new(&format!("scale must be more than 0 and at most 4, got {}", options.scale)));
        }
        Ok(Render { scale: options.scale, supersample: options.supersample.max(1.0), parchment: options.parchment })
    }
}

/// Renders pages with Darkly on one WebGPU device. Renders run one at a
/// time; while one does, the others fail.
#[wasm_bindgen]
pub struct Renderer {
    state: Rc<RefCell<RendererState>>,
}

struct RendererState {
    engine: darkly::DarklyEngine,
    /// The last script run, the journal version it ran with, and its entry, so
    /// rendering its pages one by one runs it once.
    last: Option<(String, u64, Rc<Entry>)>,
}

impl RendererState {
    fn entry(&mut self, source: &str) -> Result<Rc<Entry>, JsError> {
        let version = JOURNAL.with_borrow(|journal| journal.version);
        if let Some((last_source, last_version, entry)) = &self.last
            && last_source == source && *last_version == version {
            return Ok(entry.clone());
        }
        let entry = match run(source).0 {
            Ok(entry) => Rc::new(entry),
            Err(err) => return Err(JsError::new(&err.message)),
        };
        self.last = Some((source.to_string(), version, entry.clone()));
        Ok(entry)
    }
}

fn busy() -> JsValue {
    JsError::new("the renderer is busy with another render").into()
}

/// Call a JS progress callback, ignoring what it throws.
fn report(progress: &js_sys::Function, args: &[f64]) {
    let args: js_sys::Array = args.iter().map(|&v| JsValue::from_f64(v)).collect();
    let _ = progress.apply(&JsValue::NULL, &args);
}

#[wasm_bindgen]
impl Renderer {
    /// A renderer on the browser's GPU; fails if it has no WebGPU.
    pub async fn create() -> Result<Renderer, JsError> {
        let engine = darkly::new_engine_async(1, 1).await.map_err(|e| JsError::new(&e))?;
        Ok(Renderer { state: Rc::new(RefCell::new(RendererState { engine, last: None })) })
    }

    /// Page `index` of the journal `source` makes, rendered with `options`
    /// (`{ scale, supersample, parchment }`) as a PNG's bytes, or
    /// `undefined` if nothing's drawn on it. `progress(fraction)` is called
    /// as it goes.
    #[wasm_bindgen(js_name = renderPage)]
    pub fn render_page(&self, source: String, index: usize, options: JsValue, progress: js_sys::Function) -> js_sys::Promise {
        let state = self.state.clone();
        let render = RenderOptions::from_js(options);
        wasm_bindgen_futures::future_to_promise(async move {
            let render = render?;
            let mut state = state.try_borrow_mut().map_err(|_| busy())?;
            let entry = state.entry(&source)?;
            let page = entry.selected_pages(None).nth(index).map(|(_, page)| page)
                .ok_or_else(|| JsError::new(&format!("there's no page {}; the journal has {}", index, entry.page_count())))?;
            let image = publish::page_image_async(page, &page.label(index), &mut state.engine, &render, &mut |done| report(&progress, &[done as f64])).await
                .map_err(|e| JsError::new(&e))?;
            match image {
                None => Ok(JsValue::UNDEFINED),
                Some(image) => Ok(js_sys::Uint8Array::from(&publish::png_bytes(&image).map_err(|e| JsError::new(&e))?[..]).into()),
            }
        })
    }

    /// Every page of the journal `source` makes, rendered with `options`,
    /// in one PDF titled `title` (as `journalbuild --pdf` makes it), as its
    /// bytes. `progress(page, pages, fraction)` is called as it goes.
    #[wasm_bindgen(js_name = exportPdf)]
    pub fn export_pdf(&self, source: String, title: String, options: JsValue, progress: js_sys::Function) -> js_sys::Promise {
        let state = self.state.clone();
        let render = RenderOptions::from_js(options);
        wasm_bindgen_futures::future_to_promise(async move {
            let render = render?;
            let mut state = state.try_borrow_mut().map_err(|_| busy())?;
            let entry = state.entry(&source)?;
            let failed = |e: std::io::Error| JsError::new(&format!("couldn't write the PDF: {}", e));
            let mut pdf = PdfWriter::new(std::io::Cursor::new(Vec::new())).map_err(failed)?;
            let points = 72.0 / UNITS_PER_INCH / render.scale;
            let pages = entry.page_count() as f64;
            for (i, page) in entry.selected_pages(None) {
                let mut page_progress = |done: f32| report(&progress, &[i as f64, pages, done as f64]);
                let image = publish::page_image_async(page, &page.label(i), &mut state.engine, &render, &mut page_progress).await
                    .map_err(|e| JsError::new(&e))?;
                if let Some(image) = image {
                    pdf.add_page(&image, image.width() as f32 * points, image.height() as f32 * points).map_err(failed)?;
                }
            }
            if pdf.page_count() == 0 {
                return Err(JsError::new("nothing to put in the PDF: every page is empty").into());
            }
            let bytes = pdf.finish(&title).map_err(failed)?.into_inner();
            Ok(js_sys::Uint8Array::from(&bytes[..]).into())
        })
    }
}
