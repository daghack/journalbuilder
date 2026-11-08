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
//! `libraryFiles` lists the library and `usedLibrary` the library files an
//! entry uses; the lock and chain editor has `lockTypes`,
//! `lockPorts`, `lockExamples`, `chainTypes`, `runChain`, `readChain` and
//! `valueKinds`; and the illustration, layout and style editors have
//! `illustrationTypes`, `illustrationPorts`, `illustrationPreview`,
//! `layoutTypes`, `layoutPreview`, `styleTypes`, `convertStyle` and
//! `styleChoices`.

use scribe::journal::darkly;
use scribe::journal::fluid::{self, Fluid, FluidField};
use scribe::journal::fonts::FontBook;
use scribe::journal::pdf::PdfWriter;
use scribe::journal::publish::{self, Render};
use scribe::journal::rhaientries::{brush_names, Entry, JournalEngine, LockLibrary, Page, Resources};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
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
/// `glyphsets/y.json`, `content/z.txt`, `images/p.png` as a data URL;
/// entries aren't needed) as the
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
            ("helpers", "rhai") => &mut journal.helpers,
            ("layouts", "rhai") => &mut journal.layouts,
            ("illustrations", "rhai") => &mut journal.illustrations,
            ("chains", "rhai") => &mut journal.chains,
            ("styles", "json") => &mut journal.styles,
            ("lib", "rhai") => &mut journal.lib,
            ("glyphsets", "json") => &mut journal.glyphsets,
            ("content", "txt") => &mut journal.text,
            ("content", "md") => &mut journal.markdown,
            ("content", "json") => &mut journal.symbols,
            // Pictures as data URLs.
            ("images", "png") => &mut journal.png_images,
            ("images", "jpg") => &mut journal.jpg_images,
            ("fluids", "json") => &mut journal.fluids,
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

/// The page the Fluid tab works on (`fluidPrepare`).
struct FluidPage {
    /// As the entry makes it, and with the tab's fluid left out.
    page: Page,
    original: Page,
    label: String,
    /// The fluid the tab edits.
    name: String,
    /// The last field `fluidApply` was given, and what's worked out from
    /// it, so changing only the settings keeps that.
    field: Option<(RawField, Arc<FluidField>)>,
}

#[derive(PartialEq)]
struct RawField {
    nx: usize,
    ny: usize,
    h: f32,
    arrays: [Vec<f32>; 4],
}

thread_local! {
    static FLUID_PAGE: RefCell<Option<FluidPage>> = const { RefCell::new(None) };
}

fn error_json(message: impl Into<String>) -> Value {
    json!({ "message": message.into() })
}

/// Run the journal entry `source` and keep its page `index` for the Fluid
/// tab, which edits the fluid `name`. As JSON: `{ pages: [{ index, label
/// }], label, size, canvases: [{ label, bounds }], uses: { page, canvases
/// }, svg, view, warnings, error }`: the page `size` it's set to, if it is;
/// its canvases' `bounds` (`[x, y, width, height]`, with their margins);
/// where the entry adds the fluid (to the page, to which canvases); and
/// the page as SVG with the fluid left out (with its `view`, as `pageSvg`).
#[wasm_bindgen(js_name = fluidPrepare)]
pub fn fluid_prepare(source: &str, index: usize, name: &str) -> String {
    FLUID_PAGE.set(None);
    let (entry, evaluation) = run(source);
    let entry = match entry {
        Ok(entry) => entry,
        Err(e) => return json!({ "pages": [], "warnings": evaluation.warnings, "error": { "message": e.message, "line": e.line, "column": e.column } }).to_string(),
    };
    let pages: Vec<Value> = entry.selected_pages(None).map(|(i, page)| json!({ "index": i, "label": page.label(i) })).collect();
    let Some((index, page)) = entry.selected_pages(None).nth(index) else {
        let message = format!("there's no page {}; the entry has {}", index, entry.page_count());
        return json!({ "pages": pages, "warnings": evaluation.warnings, "error": error_json(message) }).to_string();
    };
    let label = page.label(index);
    let original = page.with_fluid(name, None, None).expect("leaving a fluid out can't fail");
    let canvases: Vec<Value> = original.canvases().iter().enumerate().map(|(i, canvas)| {
        let bounds = original.render_canvas(canvas).map(|rendered| {
            let b = rendered.bounds();
            [b.mins.x, b.mins.y, b.maxs.x - b.mins.x, b.maxs.y - b.mins.y]
        });
        json!({ "label": canvas.label(i), "bounds": bounds })
    }).collect();
    let (on_page, on_canvases) = page.fluid_places(name);
    let svg = publish::page_svg(&original, &label);
    let answer = json!({
        "pages": pages,
        "label": label,
        "size": original.fixed_size().map(|s| [s.x, s.y]),
        "canvases": canvases,
        "uses": { "page": on_page, "canvases": on_canvases },
        "svg": svg.as_ref().map(|(svg, _)| svg),
        "view": svg.as_ref().map(|(_, view)| view),
        "warnings": evaluation.warnings,
        "error": null,
    });
    FLUID_PAGE.set(Some(FluidPage { page: page.clone(), original, label, name: name.to_string(), field: None }));
    answer.to_string()
}

/// The ink of the canvases labelled `canvases` on the page `fluidPrepare`
/// kept (with its fluid left out), as an alpha raster `nx` by `ny`, row by
/// row, over the box at (`x`, `y`) of `width` by `height` canvas units: for
/// the flow simulator's `set_mask`.
#[wasm_bindgen(js_name = fluidMask)]
pub fn fluid_mask(canvases: Vec<String>, x: f32, y: f32, width: f32, height: f32, nx: usize, ny: usize) -> Result<Vec<u8>, JsError> {
    FLUID_PAGE.with_borrow(|kept| {
        let kept = kept.as_ref().ok_or_else(|| JsError::new("no page to mask: call fluidPrepare first"))?;
        let page = &kept.original;
        let rendered: Vec<_> = page.canvases().iter().enumerate()
            .filter(|(i, canvas)| canvases.contains(&canvas.label(*i)))
            .filter_map(|(_, canvas)| page.render_canvas(canvas))
            .collect();
        fluid::ink_mask(&rendered, [x, y], [width, height], nx, ny).map_err(|e| JsError::new(&e))
    })
}

/// The page `fluidPrepare` kept, distorted by its fluid: its field from the
/// flow simulator's raw arrays (`nx` by `ny` cells `h` across: `u`, `v`,
/// `pressure`, `dye`, as `Field::from_raw` takes them) and its box and
/// effects from `settings`, a fluid file's text. Where the entry adds the
/// fluid, this stands in for it; where it doesn't, it's added to the
/// canvas labelled `target`, or the page with none. `live` draws it
/// quickly, coarser, for a flow that's running. As JSON: `{ svg, view,
/// error }`.
#[wasm_bindgen(js_name = fluidApply)]
#[allow(clippy::too_many_arguments)]
pub fn fluid_apply(nx: usize, ny: usize, h: f32, u: Vec<f32>, v: Vec<f32>, pressure: Vec<f32>, dye: Vec<f32>, settings: &str, target: Option<String>, live: bool) -> String {
    let answer = FLUID_PAGE.with_borrow_mut(|kept| -> Result<Value, String> {
        let kept = kept.as_mut().ok_or("no page to distort: call fluidPrepare first")?;
        let raw = RawField { nx, ny, h, arrays: [u, v, pressure, dye] };
        let field = match &kept.field {
            Some((last, field)) if *last == raw => field.clone(),
            _ => {
                let [u, v, pressure, dye] = &raw.arrays;
                let field = Arc::new(FluidField::from_raw(nx, ny, h, u, v, pressure, dye)?);
                kept.field = Some((raw, field.clone()));
                field
            }
        };
        let fluid = Fluid { live, ..Fluid::with_settings(&kept.name, field, settings)? };
        let page = kept.page.with_fluid(&kept.name, Some(Arc::new(fluid)), target.as_deref())?;
        let svg = publish::page_svg(&page, &kept.label);
        Ok(json!({ "svg": svg.as_ref().map(|(svg, _)| svg), "view": svg.as_ref().map(|(_, view)| view), "error": null }))
    });
    answer.unwrap_or_else(|e| json!({ "svg": null, "view": null, "error": error_json(e) })).to_string()
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

/// The library files the entry script `source` uses, run with the journal's
/// files: what an export bundles. JSON: `{ paths: [...], error }`
/// (`locks/huffman.rhai`); `error` is `{ message, line, column }` if the
/// script failed, and `paths` are then those it read before it did.
#[wasm_bindgen(js_name = usedLibrary)]
pub fn used_library(source: &str) -> String {
    let (library, used) = Resources::embedded().recording();
    let resources = resources_over(library, &[]);
    let (book, _) = glyphsets(&resources);
    let mut engine = JournalEngine::with_resources(book.clone(), resources);
    let error = engine.build_entry(source).err().map(|err| ScriptError { message: err.message, line: err.line, column: err.column });
    let paths = used.paths(Some((&book, &Resources::embedded().glyphsets)));
    json!({ "paths": paths, "error": error }).to_string()
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

/// Every lock and helper type, as JSON: `[{ name, file, description,
/// composite, helper, options: [{ name, kind, choices?, default? }], own,
/// builtin }]`, or `{ name, error, helper, own, builtin }` for one that
/// doesn't load. `own` if the journal has the file, `builtin` if the
/// library does.
#[wasm_bindgen(js_name = lockTypes)]
pub fn lock_types() -> String {
    let library = locks(&[]);
    answer(library.describe_all().map(|types| {
        types.into_iter().map(|(name, info)| {
            let helper = library.is_helper(&name);
            let path = format!("{}/{}.rhai", if helper { "helpers" } else { "locks" }, name);
            match info {
                Ok(info) => marked(serde_json::to_value(info).unwrap_or(Value::Null), &path),
                Err(error) => marked(json!({ "name": name, "error": error, "helper": helper }), &path),
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

/// Run lock or helper type `type_name`'s examples through lock and unlock,
/// with `source` standing in for its file if given: `{ examples }`, how
/// many passed, or `{ error }`, the first failure.
#[wasm_bindgen(js_name = lockExamples)]
pub fn lock_examples(type_name: &str, source: Option<String>) -> String {
    let helper = locks(&[]).is_helper(type_name);
    let path = format!("{}/{}.rhai", if helper { "helpers" } else { "locks" }, type_name);
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
/// defaults: `{ inputs, fallbacks }`, `[name, type]` pairs and the names of
/// those that draw with the illustration's own when nothing's wired to
/// them, or `{ error }`.
#[wasm_bindgen(js_name = illustrationPorts)]
pub fn illustration_ports(type_name: &str, options: &str) -> String {
    answer(parse_json(options, "options").and_then(|options| {
        let engine = engine(&[]);
        let inputs = engine.illustration_inputs(type_name, &options)?;
        let fallbacks = engine.illustration_fallbacks(type_name, &options)?;
        Ok(json!({ "inputs": inputs, "fallbacks": fallbacks }))
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
        let path = format!("styles/{}.json", name);
        match engine.describe_text_style(&name) {
            Ok(info) => marked(serde_json::to_value(info).unwrap_or(Value::Null), &path),
            Err(error) => marked(json!({ "name": name, "error": error }), &path),
        }
    }).collect();
    Value::Array(types).to_string()
}

/// A style script, from when styles were scripts, as a style file `name`:
/// `{ file, problems }`, what the file wouldn't do that the script did, or
/// `{ error }` if the script can't be run.
#[wasm_bindgen(js_name = convertStyle)]
pub fn convert_style(name: &str, source: &str) -> String {
    answer((|| {
        let converted = engine(&[]).convert_old_text_style(name, source)?;
        Ok(json!({ "file": converted.file, "problems": converted.problems }))
    })())
}

/// What a style can be made of, for the style builder: `{ brushes,
/// glyphsets }`, every Darkly brush's name and every glyphset's (the
/// journal's and the library's), sorted.
#[wasm_bindgen(js_name = styleChoices)]
pub fn style_choices() -> String {
    let (book, _) = glyphsets(&resources(&[]));
    json!({ "brushes": brush_names(), "glyphsets": book.names() }).to_string()
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
    /// For PDFs, how many pages side by side to a PDF page (as
    /// `journalbuild --pdf --stitch`); none for a page each.
    stitch: Option<usize>,
    /// For a page, a fluid to add where the entry doesn't add it already,
    /// as the Fluid tab previews it.
    fluid: Option<FluidTarget>,
}

/// The fluid `name` (from the journal's `fluids/`), added to the canvas
/// labelled `target`, or the page with none.
#[derive(Deserialize)]
struct FluidTarget {
    name: String,
    target: Option<String>,
}

impl FluidTarget {
    /// `page` with the fluid, or as it is if the entry adds it already.
    fn on(&self, page: &Page) -> Result<Option<Page>, JsError> {
        let (on_page, on_canvases) = page.fluid_places(&self.name);
        if on_page || !on_canvases.is_empty() {
            return Ok(None);
        }
        let fluids = resources(&[]).fluids;
        if !fluids.has(&self.name) {
            return Err(JsError::new(&format!("the journal has no fluid named '{}'", self.name)));
        }
        let text = fluids.read(&self.name).map_err(|e| JsError::new(&format!("fluids/{}.json: {}", self.name, e)))?;
        let fluid = Fluid::from_file(&self.name, &text).map_err(|e| JsError::new(&e))?;
        page.with_fluid(&self.name, Some(Arc::new(fluid)), self.target.as_deref()).map(Some).map_err(|e| JsError::new(&e))
    }
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { scale: 1.0, supersample: 3.0, parchment: false, stitch: None, fluid: None }
    }
}

impl RenderOptions {
    fn from_js(options: JsValue) -> Result<(Render, Option<FluidTarget>), JsError> {
        let options = Self::parse(options)?;
        Ok((options.render(), options.fluid))
    }
    /// How to render, and how many pages to a stitch.
    fn with_stitch(options: JsValue) -> Result<(Render, Option<usize>), JsError> {
        let options = Self::parse(options)?;
        Ok((options.render(), options.stitch))
    }
    fn render(&self) -> Render {
        Render { scale: self.scale, supersample: self.supersample.max(1.0), parchment: self.parchment }
    }
    fn parse(options: JsValue) -> Result<RenderOptions, JsError> {
        let options: RenderOptions = if options.is_undefined() || options.is_null() {
            RenderOptions::default()
        } else {
            serde_wasm_bindgen::from_value(options)?
        };
        if !(options.scale > 0.0 && options.scale <= 4.0) {
            return Err(JsError::new(&format!("scale must be more than 0 and at most 4, got {}", options.scale)));
        }
        if options.stitch == Some(0) {
            return Err(JsError::new("stitch must be at least 1"));
        }
        Ok(options)
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
    /// (`{ scale, supersample, parchment, fluid }`; `fluid: { name, target
    /// }` adds the journal's fluid `name` to the canvas labelled `target`,
    /// or the page with none, unless the entry adds it) as a PNG's bytes, or
    /// `undefined` if nothing's drawn on it. `progress(fraction)` is called
    /// as it goes.
    #[wasm_bindgen(js_name = renderPage)]
    pub fn render_page(&self, source: String, index: usize, options: JsValue, progress: js_sys::Function) -> js_sys::Promise {
        let state = self.state.clone();
        let render = RenderOptions::from_js(options);
        wasm_bindgen_futures::future_to_promise(async move {
            let (render, fluid) = render?;
            let mut state = state.try_borrow_mut().map_err(|_| busy())?;
            let entry = state.entry(&source)?;
            let page = entry.selected_pages(None).nth(index).map(|(_, page)| page)
                .ok_or_else(|| JsError::new(&format!("there's no page {}; the journal has {}", index, entry.page_count())))?;
            let with_fluid = match &fluid {
                Some(fluid) => fluid.on(page)?,
                None => None,
            };
            let page = with_fluid.as_ref().unwrap_or(page);
            let image = publish::page_image_async(page, &page.label(index), &mut state.engine, &render, &mut |done| report(&progress, &[done as f64])).await
                .map_err(|e| JsError::new(&e))?;
            match image {
                None => Ok(JsValue::UNDEFINED),
                Some(image) => Ok(js_sys::Uint8Array::from(&publish::png_bytes(&image).map_err(|e| JsError::new(&e))?[..]).into()),
            }
        })
    }

    /// Every page of the journal `source` makes, rendered with `options`
    /// (`{ scale, supersample, parchment, stitch }`), in one PDF titled
    /// `title` (as `journalbuild --pdf` makes it, with `--stitch`), as its
    /// bytes. `progress(page, pages, fraction)` is called as it goes.
    #[wasm_bindgen(js_name = exportPdf)]
    pub fn export_pdf(&self, source: String, title: String, options: JsValue, progress: js_sys::Function) -> js_sys::Promise {
        let state = self.state.clone();
        let render = RenderOptions::with_stitch(options);
        wasm_bindgen_futures::future_to_promise(async move {
            let (render, stitch) = render?;
            let mut state = state.try_borrow_mut().map_err(|_| busy())?;
            let entry = state.entry(&source)?;
            let failed = |e: std::io::Error| JsError::new(&format!("couldn't write the PDF: {}", e));
            let mut pdf = PdfWriter::new(std::io::Cursor::new(Vec::new())).map_err(failed)?;
            let mut stitcher = publish::Stitcher::new(stitch);
            let pages = entry.page_count() as f64;
            for (i, page) in entry.selected_pages(None) {
                let mut page_progress = |done: f32| report(&progress, &[i as f64, pages, done as f64]);
                let image = publish::page_image_async(page, &page.label(i), &mut state.engine, &render, &mut page_progress).await
                    .map_err(|e| JsError::new(&e))?;
                if let Some(sheet) = image.and_then(|image| stitcher.push(image)) {
                    publish::add_pdf_page(&mut pdf, &sheet, render.scale).map_err(failed)?;
                }
            }
            if let Some(sheet) = stitcher.finish() {
                publish::add_pdf_page(&mut pdf, &sheet, render.scale).map_err(failed)?;
            }
            if pdf.page_count() == 0 {
                return Err(JsError::new("nothing to put in the PDF: every page is empty").into());
            }
            let bytes = pdf.finish(&title).map_err(failed)?.into_inner();
            Ok(js_sys::Uint8Array::from(&bytes[..]).into())
        })
    }
}
