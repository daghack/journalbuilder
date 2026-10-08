//! Writing an entry's pages out: Darkly renders (an image per canvas, an
//! image per page, or one PDF of the pages) and SVG renders. Each shows its
//! progress, page by page, on the terminal (see `Progress`).

use ::darkly::engine::DarklyEngine;
use image::RgbaImage;

use super::parchment::{self, Sheet};
use super::canvas::StrokeColor;
use super::compose::{self, Placed};
use super::darkly;
use super::pdf::PdfWriter;
use super::progress::Progress;
use super::rhaientries::{Entry, Page};
use super::stable_hash;

/// How Darkly renders are made.
#[derive(Clone, Copy, Debug)]
pub struct Render {
    /// Output pixels per canvas unit (brushes included: their sizes are in
    /// units too).
    pub scale: f32,
    /// Rendered at this multiple of the size and filtered down (see
    /// `darkly::render`).
    pub supersample: f32,
    /// Whether page images and PDFs are drawn on parchment.
    pub parchment: bool,
}

/// Canvas units to an inch in PDF output: a page 1100 units across is
/// about 7.3 inches.
pub const UNITS_PER_INCH: f32 = 150.0;

/// Paper round a page's strokes, as a fraction of its larger side, and at
/// least this many units.
const PAPER_BORDER: f32 = 0.06;
const PAPER_BORDER_MIN: f32 = 20.0;

fn write_error(path: &str, e: impl std::fmt::Display) -> String {
    format!("couldn't write {}: {}", path, e)
}

/// Progress through the pages that will be written.
fn pages_progress(entry: &Entry, what: &str, only: Option<&[usize]>) -> Progress {
    Progress::new(what, entry.selected_pages(only).count())
}

/// A Darkly render of each canvas, `darkly_{page}_{canvas}.png`, of every
/// page or those in `only`. Empty canvases are skipped (journalbuild warns
/// about them).
pub fn canvas_images(entry: &Entry, engine: &mut DarklyEngine, dir: &str, render: &Render, only: Option<&[usize]>) -> Result<(), String> {
    let mut progress = pages_progress(entry, "Rendering", only);
    for (i, page) in entry.selected_pages(only) {
        for (label, canvas) in page.rendered_canvases() {
            progress.start(&format!("{} {}", page.label(i), label));
            let filename = format!("{}/darkly_{}_{}.png", dir, page.label(i), label);
            darkly::render(&canvas, engine, render.scale, render.supersample)
                .save(&filename).map_err(|e| write_error(&filename, e))?;
        }
        progress.step();
    }
    Ok(())
}

/// A Darkly render of each page, its canvases layered into one image (see
/// `page_image`), `darkly_{page}.png`.
pub fn page_images(entry: &Entry, engine: &mut DarklyEngine, dir: &str, render: &Render, only: Option<&[usize]>) -> Result<(), String> {
    let mut progress = pages_progress(entry, "Rendering", only);
    for (i, page) in entry.selected_pages(only) {
        let label = page.label(i);
        progress.start(&label);
        if let Some(image) = page_image(page, &label, engine, render) {
            let filename = format!("{}/darkly_{}.png", dir, label);
            image.save(&filename).map_err(|e| write_error(&filename, e))?;
        }
        progress.step();
    }
    Ok(())
}

/// The pages' images (see `page_image`) as the pages of one PDF at `path`,
/// titled `title`. A page's size is its size in canvas units at
/// `UNITS_PER_INCH`, whatever the scale.
pub fn pdf(entry: &Entry, engine: &mut DarklyEngine, path: &str, title: &str, render: &Render, only: Option<&[usize]>) -> Result<(), String> {
    let mut pdf = PdfWriter::create(path).map_err(|e| write_error(path, e))?;
    let points = 72.0 / UNITS_PER_INCH / render.scale;
    let mut progress = pages_progress(entry, "Rendering", only);
    for (i, page) in entry.selected_pages(only) {
        progress.start(&page.label(i));
        let image = page_image(page, &page.label(i), engine, render);
        progress.step();
        if let Some(image) = image {
            pdf.add_page(&image, image.width() as f32 * points, image.height() as f32 * points)
                .map_err(|e| write_error(path, e))?;
        }
    }
    if pdf.page_count() == 0 {
        return Err(format!("nothing to put in {}: every page is empty", path));
    }
    pdf.finish(title).map_err(|e| write_error(path, e))
}

/// A page rendered with Darkly as one image, `label` being its name in
/// output: its canvases layered in order, each where it is on the page, on
/// parchment if the render is on it. On parchment, a page that isn't a tile (or
/// typeset) gets a border of paper round its strokes; each tile of a split
/// page is a sheet of its own, like a big drawing spread over several
/// pages. On parchment, light inks are drawn dark so they show (see
/// `StrokeColor::as_ink_on_paper`). `None` if nothing is drawn.
pub fn page_image(page: &Page, label: &str, engine: &mut DarklyEngine, render: &Render) -> Option<RgbaImage> {
    let scale = render.scale;
    let mut layers = Vec::new();
    let mut frame = None;
    for (_, mut canvas) in page.rendered_canvases() {
        if render.parchment {
            canvas.recolor(StrokeColor::as_ink_on_paper);
        }
        frame = frame.or(canvas.frame().copied());
        layers.push(Placed { at: canvas.render_origin(scale), image: darkly::render(&canvas, engine, scale, render.supersample) });
    }
    let ink = compose::flatten(layers)?;
    if !render.parchment {
        return Some(ink.image);
    }
    // A tile or typeset page is a sheet as big as its frame; other pages get
    // a border of paper round their strokes.
    let (corner, size) = match frame {
        Some(frame) => {
            let [x0, y0, x1, y1] = frame.pixels(frame.page_pixels(scale)).map(|v| v as i64);
            let page = ((frame.page.mins.x * scale).round() as i64, (frame.page.mins.y * scale).round() as i64);
            ((page.0 + x0, page.1 + y0), ((x1 - x0) as u32, (y1 - y0) as u32))
        }
        None => {
            let (w, h) = ink.image.dimensions();
            let border = (w.max(h) as f32 * PAPER_BORDER).max(PAPER_BORDER_MIN * scale).round() as u32;
            ((ink.at.0 - border as i64, ink.at.1 - border as i64), (w + 2 * border, h + 2 * border))
        }
    };
    let sheet = Sheet { corner, size, scale, seed: stable_hash(label) };
    let mut sheet = Placed { image: parchment::paint(&sheet), at: corner };
    compose::draw(&mut sheet, &ink, compose::ink_on_paper);
    Some(sheet.image)
}

/// SVG renders, one per canvas like the Darkly ones, each written as
/// `{page}_{canvas}.svg` and rasterised to `{page}_{canvas}.png`. Here a
/// style's width is the stroke's geometric width; in Darkly it scales the
/// brush's pressure instead (see `new_style`).
pub fn svgs(entry: &Entry, dir: &str, scale: f32, only: Option<&[usize]>) -> Result<(), String> {
    let mut progress = pages_progress(entry, "SVGs", only);
    for (i, page) in entry.selected_pages(only) {
        progress.start(&page.label(i));
        for (label, canvas) in page.rendered_canvases() {
            let name = format!("{}_{}", page.label(i), label);
            let (svg_file, png_file) = (format!("{}/{}.svg", dir, name), format!("{}/{}.png", dir, name));
            // Seeded by the file's name: the same every run, but each canvas
            // roughened differently. Tiles use the whole page's.
            let seed = canvas.frame().filter(|frame| frame.is_tile()).map_or_else(|| stable_hash(&name), |frame| frame.svg_seed);
            let (svg, png) = canvas.to_svg_and_png(2.0 * scale, page.svg_style(), seed);
            std::fs::write(&svg_file, svg).map_err(|e| write_error(&svg_file, e))?;
            std::fs::write(&png_file, png).map_err(|e| write_error(&png_file, e))?;
        }
        progress.step();
    }
    Ok(())
}
