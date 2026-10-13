//! The journal editor's renderer, for the browser: runs a journal script
//! and shows its pages as SVG (`evaluate`), and renders them with Darkly
//! over WebGPU (`Renderer`), as images, a PDF or PNGs.
//!
//! Layout and lock files and the fonts are compiled in (see
//! `rhaientries::embedded`), so scripts can use them with nothing to read.

use scribe::journal::darkly;
use scribe::journal::fonts::FontBook;
use scribe::journal::pdf::PdfWriter;
use scribe::journal::publish::{self, Render, UNITS_PER_INCH};
use scribe::journal::rhaientries::{embedded, Entry, JournalEngine};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
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
}

/// Run a journal script: its entry, if it ran, and what it said.
fn run(source: &str) -> (Result<Entry, ScriptError>, Evaluation) {
    let fonts = Rc::new(FontBook::from_bytes(embedded::FONTS).expect("the compiled-in fonts.json loads"));
    let mut engine = JournalEngine::embedded(fonts.clone());
    let prints: Rc<RefCell<Vec<String>>> = Rc::default();
    let printed = prints.clone();
    engine.on_print(move |text| printed.borrow_mut().push(text.to_string()));
    let entry = engine.build_entry(source).map_err(|err| ScriptError { message: err.message, line: err.line, column: err.column });
    let mut warnings = engine.warnings();
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
    /// The last script run, and its entry, so rendering its pages one by
    /// one runs it once.
    last: Option<(String, Rc<Entry>)>,
}

impl RendererState {
    fn entry(&mut self, source: &str) -> Result<Rc<Entry>, JsError> {
        if let Some((last_source, entry)) = &self.last
            && last_source == source {
            return Ok(entry.clone());
        }
        let entry = match run(source).0 {
            Ok(entry) => Rc::new(entry),
            Err(err) => return Err(JsError::new(&err.message)),
        };
        self.last = Some((source.to_string(), entry.clone()));
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
