//! The journal editor's renderer, for the browser: runs a journal script
//! and shows its pages as SVG (`evaluate`), and renders them with Darkly
//! over WebGPU (`Renderer`), as images, a PDF or PNGs.
//!
//! Layout and lock files and the fonts are compiled in (see
//! `rhaientries::embedded`), so scripts can use them with nothing to read.
//! The editor's workspace (`setWorkspace`) can stand in for the fonts and
//! add lock files, for edits not yet saved into the repository.
//!
//! The font and lock editors use the rest: `previewSvg` draws a script's
//! first page without touching what `evaluate` keeps, and the lock exports
//! (`lockTypes`, `lockPorts`, `runChain`, `readChain`, `checkLock`) answer
//! in JSON.

use scribe::journal::darkly;
use scribe::journal::fonts::FontBook;
use scribe::journal::pdf::PdfWriter;
use scribe::journal::publish::{self, Render, UNITS_PER_INCH};
use scribe::journal::rhaientries::{embedded, Entry, JournalEngine, LockLibrary, Sources};
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
    workspace: WorkspaceUse,
}

/// Edits not yet in the repository, from `setWorkspace`.
#[derive(Default)]
struct Workspace {
    /// A fonts.json standing in for the compiled-in one, if it loads.
    fonts: Option<Rc<Vec<u8>>>,
    /// Why the fonts given don't load, if they don't.
    fonts_error: Option<String>,
    /// Lock files by type name, added to the compiled-in ones or in place
    /// of them.
    locks: BTreeMap<String, String>,
    /// Counts changes, so renders can tell their cache is stale.
    version: u64,
}

thread_local! {
    static WORKSPACE: RefCell<Workspace> = RefCell::new(Workspace::default());
}

/// Use `fonts` (a fonts.json, or null for the compiled-in one) and `locks`
/// (type name -> lock file source) for every script run from now on.
/// Returns JSON, `{ fontsError }`: why the fonts don't load, if they
/// don't, in which case the compiled-in ones are used.
#[wasm_bindgen(js_name = setWorkspace)]
pub fn set_workspace(fonts: Option<String>, locks: JsValue) -> Result<String, JsError> {
    let locks: BTreeMap<String, String> = if locks.is_undefined() || locks.is_null() {
        BTreeMap::new()
    } else {
        serde_wasm_bindgen::from_value(locks)?
    };
    let error = WORKSPACE.with_borrow_mut(|workspace| {
        let (loaded, error) = match fonts {
            None => (None, None),
            Some(fonts) => match FontBook::from_bytes(fonts.as_bytes()) {
                Ok(_) => (Some(Rc::new(fonts.into_bytes())), None),
                Err(e) => (None, Some(e)),
            },
        };
        workspace.fonts = loaded;
        workspace.fonts_error = error.clone();
        workspace.locks = locks;
        workspace.version += 1;
        error
    });
    Ok(json!({ "fontsError": error }).to_string())
}

/// The workspace's fonts, or the compiled-in ones.
fn font_book() -> Rc<FontBook> {
    let fonts = WORKSPACE.with_borrow(|workspace| workspace.fonts.clone());
    let bytes: &[u8] = fonts.as_deref().map(Vec::as_slice).unwrap_or(embedded::FONTS);
    Rc::new(FontBook::from_bytes(bytes).expect("the workspace's fonts were checked when set"))
}

/// The compiled-in lock files with the workspace's, and `extra`, over them.
fn lock_sources(extra: &[(&str, &str)]) -> Sources {
    let locks = WORKSPACE.with_borrow(|workspace| workspace.locks.clone());
    embedded::locks()
        .with_overlay(locks)
        .with_overlay(extra.iter().map(|(name, source)| (name.to_string(), source.to_string())))
}

/// What the workspace changes, for the editor to say so.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceUse {
    edited_fonts: bool,
    locks: Vec<String>,
}

/// Run a journal script: its entry, if it ran, and what it said.
fn run(source: &str) -> (Result<Entry, ScriptError>, Evaluation) {
    let (entry, mut evaluation) = run_with(source, font_book());
    let fonts_error = WORKSPACE.with_borrow(|workspace| workspace.fonts_error.clone());
    if let Some(error) = fonts_error {
        evaluation.warnings.insert(0, format!("the edited fonts don't load, so the built-in ones are used: {}", error));
    }
    (entry, evaluation)
}

/// Run a journal script with `fonts`.
fn run_with(source: &str, fonts: Rc<FontBook>) -> (Result<Entry, ScriptError>, Evaluation) {
    let mut engine = JournalEngine::with_sources(fonts.clone(), lock_sources(&[]), embedded::layouts());
    let prints: Rc<RefCell<Vec<String>>> = Rc::default();
    let printed = prints.clone();
    engine.on_print(move |text| printed.borrow_mut().push(text.to_string()));
    let entry = engine.build_entry(source).map_err(|err| ScriptError { message: err.message, line: err.line, column: err.column });
    let mut warnings = engine.warnings();
    let workspace = WORKSPACE.with_borrow(|workspace| {
        WorkspaceUse { edited_fonts: workspace.fonts.is_some(), locks: workspace.locks.keys().cloned().collect() }
    });
    if let Ok(entry) = &entry {
        if entry.page_count() == 0 {
            warnings.push("the script made no pages; add them with journal_entry.add_page(page)".to_string());
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
        missing_glyphs: fonts.missing_glyph_report(),
        error: None,
        millis: 0.0,
        workspace,
    };
    (entry, evaluation)
}

/// Run a journal script, keeping the entry it makes for `pageSvg`.
/// Returns `{ pages: [{ index, label, view }], warnings, prints,
/// missingGlyphs, error: { message, line, column }, millis }`.
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

/// A script's first page as SVG, for the font editor's text preview, as
/// JSON: `{ svg, view, warnings, missingGlyphs, error }`. It runs with `fonts`
/// (a fonts.json) if given, else the workspace's, and leaves what
/// `evaluate` keeps alone.
#[wasm_bindgen(js_name = previewSvg)]
pub fn preview_svg(source: &str, fonts: Option<String>) -> String {
    let book = match fonts {
        None => font_book(),
        Some(fonts) => match FontBook::from_bytes(fonts.as_bytes()) {
            Ok(book) => Rc::new(book),
            Err(e) => {
                let preview = json!({ "svg": null, "view": null, "warnings": [], "missingGlyphs": null, "error": { "message": format!("the fonts don't load: {}", e) } });
                return preview.to_string();
            }
        },
    };
    let (entry, evaluation) = run_with(source, book);
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

/// The compiled-in fonts.json.
#[wasm_bindgen(js_name = builtinFonts)]
pub fn builtin_fonts() -> String {
    String::from_utf8_lossy(embedded::FONTS).into_owned()
}

/// A lock library over the compiled-in lock files, the workspace's and
/// `extra`.
fn locks(extra: &[(&str, &str)]) -> Rc<LockLibrary> {
    Rc::new(LockLibrary::with_sources(lock_sources(extra)))
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

/// Every lock type, as JSON: `[{ name, file, description, composite,
/// options: [{ name, kind, choices?, default? }], workspace }]`, or
/// `{ name, error }` for one that doesn't load.
#[wasm_bindgen(js_name = lockTypes)]
pub fn lock_types() -> String {
    let library = locks(&[]);
    let workspace: Vec<String> = WORKSPACE.with_borrow(|workspace| workspace.locks.keys().cloned().collect());
    answer(library.describe_all().map(|types| {
        types.into_iter().map(|(name, info)| {
            let in_workspace = workspace.contains(&name);
            match info {
                Ok(info) => {
                    let mut value = serde_json::to_value(info).unwrap_or(Value::Null);
                    value["workspace"] = json!(in_workspace);
                    value
                }
                Err(error) => json!({ "name": name, "error": error, "workspace": in_workspace }),
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

/// Run the composite lock file `source` with `options` and `inputs`
/// (JSON, name -> `{ text }` or `{ json }`), then unlock what it gives:
/// `{ outputs, trace, recovered, roundTrip, unlockError }` with each value
/// `{ type, text?, json, maze? }`, or `{ error }`.
#[wasm_bindgen(js_name = runChain)]
pub fn run_chain(source: &str, options: &str, inputs: &str) -> String {
    const EDITING: &str = "__editing";
    answer((|| {
        let options = parse_json(options, "options")?;
        let inputs = parse_json(inputs, "inputs")?;
        locks(&[(EDITING, source)]).run_traced(EDITING, &options, &inputs)
    })())
}

/// The composite lock type `type_name` read back for the editor (see
/// `LockLibrary::read_chain`), from `source` if given, or `{ error }`.
#[wasm_bindgen(js_name = readChain)]
pub fn read_chain(type_name: &str, source: Option<String>) -> String {
    let extra: Vec<(&str, &str)> = source.as_deref().map(|source| (type_name, source)).into_iter().collect();
    answer(locks(&extra).read_chain(type_name))
}

/// Run the examples of the lock file `source`, as the type `type_name`:
/// `{ examples }`, how many passed, or `{ error }`.
#[wasm_bindgen(js_name = checkLock)]
pub fn check_lock(type_name: &str, source: &str) -> String {
    answer(locks(&[(type_name, source)]).check_examples(type_name).map(|count| json!({ "examples": count })))
}

/// The name of every function scripts can call that journals add to rhai,
/// for the editor to highlight and complete.
#[wasm_bindgen(js_name = apiNames)]
pub fn api_names() -> Vec<String> {
    let fonts = Rc::new(FontBook::from_bytes(embedded::FONTS).expect("the compiled-in fonts.json loads"));
    JournalEngine::embedded(fonts).function_names().into_iter()
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
    /// The last script run, the workspace it ran with, and its entry, so
    /// rendering its pages one by one runs it once.
    last: Option<(String, u64, Rc<Entry>)>,
}

impl RendererState {
    fn entry(&mut self, source: &str) -> Result<Rc<Entry>, JsError> {
        let version = WORKSPACE.with_borrow(|workspace| workspace.version);
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
