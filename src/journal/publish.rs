//! Writing an entry's pages out: Darkly renders (an image per canvas, an
//! image per page, or one PDF of the pages) and SVG renders. Each shows its
//! progress, page by page, on the terminal (see `Progress`).

use ::darkly::engine::DarklyEngine;
use image::RgbaImage;

use super::parchment::{self, Sheet};
use super::canvas::StrokeColor;
use super::compose::{self, Placed};
use super::darkly;
#[cfg(not(target_arch = "wasm32"))]
use super::pdf::PdfWriter;
#[cfg(not(target_arch = "wasm32"))]
use super::progress::Progress;
#[cfg(not(target_arch = "wasm32"))]
use super::rhaientries::Entry;
use super::rhaientries::Page;
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

#[cfg(not(target_arch = "wasm32"))]
fn write_error(path: &str, e: impl std::fmt::Display) -> String {
    format!("couldn't write {}: {}", path, e)
}

/// Progress through the pages that will be written.
#[cfg(not(target_arch = "wasm32"))]
fn pages_progress(entry: &Entry, what: &str, only: Option<&[usize]>) -> Progress {
    Progress::new(what, entry.selected_pages(only).count())
}

/// A Darkly render of each canvas, `darkly_{page}_{canvas}.png`, of every
/// page or those in `only`. Empty canvases are skipped (journalbuild warns
/// about them).
#[cfg(not(target_arch = "wasm32"))]
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
/// `page_image`), `darkly_{page}.png`; and with `stitch`, every `stitch`
/// of them in turn side by side (see `stitched`), named for the pages it
/// holds (see `stitch_label`).
#[cfg(not(target_arch = "wasm32"))]
pub fn page_images(entry: &Entry, engine: &mut DarklyEngine, dir: &str, render: &Render, only: Option<&[usize]>, stitch: Option<usize>) -> Result<(), String> {
    let mut progress = pages_progress(entry, "Rendering", only);
    let mut images = Vec::new();
    for (i, page) in entry.selected_pages(only) {
        let label = page.label(i);
        progress.start(&label);
        if let Some(image) = page_image(page, &label, engine, render) {
            let filename = format!("{}/darkly_{}.png", dir, label);
            image.save(&filename).map_err(|e| write_error(&filename, e))?;
            if stitch.is_some() {
                images.push((label, image));
            }
        }
        progress.step();
    }
    if let Some(n) = stitch {
        for group in images.chunks(n) {
            // A page alone is its own image already.
            if group.len() < 2 {
                continue;
            }
            let (first, last) = (&group[0].0, &group[group.len() - 1].0);
            let filename = format!("{}/darkly_{}.png", dir, stitch_label(first, last));
            let images: Vec<RgbaImage> = group.iter().map(|(_, image)| image.clone()).collect();
            stitched(&images).save(&filename).map_err(|e| write_error(&filename, e))?;
        }
    }
    Ok(())
}

/// What a stitch of the pages from `first` to `last` is called: their
/// names with the numbers they end in as a range, page_0 to page_5 being
/// page_0-5; or, unless they differ only in that number, both names,
/// first-last.
pub fn stitch_label(first: &str, last: &str) -> String {
    let stem = |label: &str| label.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (a, b) = (stem(first), stem(last));
    if a < first.len() && b < last.len() && first[..a] == last[..b] {
        format!("{}-{}", first, &last[b..])
    } else {
        format!("{}-{}", first, last)
    }
}

/// Images side by side, left to right in order, their tops level; where
/// one is shorter than the tallest, it's transparent below.
pub fn stitched(images: &[RgbaImage]) -> RgbaImage {
    let width = images.iter().map(|i| i.width()).sum();
    let height = images.iter().map(|i| i.height()).max().unwrap_or(0);
    let mut out = RgbaImage::new(width, height);
    let mut x = 0;
    for image in images {
        image::imageops::replace(&mut out, image, x as i64, 0);
        x += image.width();
    }
    out
}

/// The pages' images (see `page_image`) as the pages of one PDF at `path`,
/// titled `title`. A page's size is its size in canvas units at
/// `UNITS_PER_INCH`, whatever the scale.
#[cfg(not(target_arch = "wasm32"))]
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
    pdf.finish(title).map(|_| ()).map_err(|e| write_error(path, e))
}

/// A page rendered with Darkly as one image, `label` being its name in
/// output: its canvases layered in order, each where it is on the page, on
/// parchment if the render is on it. On parchment, a page that isn't a tile (or
/// typeset) gets a border of paper round its strokes; each tile of a split
/// page is a sheet of its own, like a big drawing spread over several
/// pages. On parchment, light inks are drawn dark so they show (see
/// `StrokeColor::as_ink_on_paper`). `None` if nothing is drawn.
#[cfg(not(target_arch = "wasm32"))]
pub fn page_image(page: &Page, label: &str, engine: &mut DarklyEngine, render: &Render) -> Option<RgbaImage> {
    darkly::block_on(page_image_async(page, label, engine, render, &mut |_| {}))
        .unwrap_or_else(|e| panic!("{}", e))
}

/// `page_image`, giving the GPU time as it goes (see
/// `darkly::render_async`), with `progress` called with how far through the
/// page it is, from 0 to 1. Fails if the GPU does.
pub async fn page_image_async(page: &Page, label: &str, engine: &mut DarklyEngine, render: &Render, progress: &mut dyn FnMut(f32)) -> Result<Option<RgbaImage>, String> {
    let scale = render.scale;
    let mut layers = Vec::new();
    let mut frame = None;
    let canvases: Vec<_> = page.rendered_canvases().collect();
    let count = canvases.len();
    for (n, (_, mut canvas)) in canvases.into_iter().enumerate() {
        if render.parchment {
            canvas.recolor(StrokeColor::as_ink_on_paper);
        }
        frame = frame.or(canvas.frame().copied());
        let mut canvas_progress = |done: f32| progress((n as f32 + done) / count as f32);
        let image = darkly::render_async(&canvas, engine, scale, render.supersample, &mut canvas_progress).await?;
        layers.push(Placed { at: canvas.render_origin(scale), image });
    }
    let Some(ink) = compose::flatten(layers) else { return Ok(None) };
    if !render.parchment {
        return Ok(Some(ink.image));
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
    Ok(Some(sheet.image))
}

/// `image` as a PNG file's bytes.
pub fn png_bytes(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(png.into_inner())
}

/// The pixels per unit page SVGs are laid out for, as `svgs` lays out
/// canvases at scale 1 (it matters only to where tiles' edges fall).
const PAGE_SVG_SCALE: f32 = 2.0;

/// A page as one SVG in canvas units, `[x, y, width, height]` of them
/// showing (`page_view`): its canvases' SVGs (as `svgs` makes them)
/// layered in order, each where it is on the page, with numbers to two
/// decimal places to keep it small. `label` is the page's. `None` if
/// nothing is drawn.
pub fn page_svg(page: &Page, label: &str) -> Option<(String, [f32; 4])> {
    let mut layers = String::new();
    let mut page_view = None;
    for (canvas_label, canvas) in page.rendered_canvases() {
        let seed = canvas_svg_seed(&canvas, &format!("{}_{}", label, canvas_label));
        let (group, view) = canvas.to_svg_view(page.svg_style(), seed, PAGE_SVG_SCALE);
        let [x, y, w, h] = view;
        layers.push_str(&format!(
            "<svg x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" viewBox=\"{x} {y} {w} {h}\">{group}</svg>"
        ));
        page_view = Some(union(page_view, view));
    }
    let [x, y, w, h] = page_view?;
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"{x} {y} {w} {h}\">{layers}</svg>"
    );
    Some((trim_decimals(&svg, 2), [x, y, w, h]))
}

/// The box of canvas units `page_svg` shows, without drawing it.
pub fn page_view(page: &Page) -> Option<[f32; 4]> {
    page.rendered_canvases().fold(None, |view, (_, canvas)| Some(union(view, canvas.svg_view_box(PAGE_SVG_SCALE))))
}

/// The box `[x, y, width, height]` round `a` (if there is one) and `b`.
fn union(a: Option<[f32; 4]>, b: [f32; 4]) -> [f32; 4] {
    match a {
        None => b,
        Some([ax, ay, aw, ah]) => {
            let [bx, by, bw, bh] = b;
            let (x0, y0) = (ax.min(bx), ay.min(by));
            [x0, y0, (ax + aw).max(bx + bw) - x0, (ay + ah).max(by + bh) - y0]
        }
    }
}

/// `text` with every decimal number cut to at most `places` places.
fn trim_decimals(text: &str, places: usize) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        out.push(b);
        i += 1;
        let after_digit = b == b'.' && out.len() >= 2 && out[out.len() - 2].is_ascii_digit();
        if after_digit && bytes.get(i).is_some_and(u8::is_ascii_digit) {
            let start = i;
            while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            out.extend_from_slice(&bytes[start..(start + places).min(i)]);
        }
    }
    String::from_utf8(out).expect("only ASCII digits were removed")
}

/// The seed a canvas's SVG is roughened with, `name` being its file's name
/// (`{page}_{canvas}`): the same every run, but each canvas roughened
/// differently. Tiles use the whole page's.
fn canvas_svg_seed(canvas: &super::canvas::Canvas, name: &str) -> u64 {
    canvas.frame().filter(|frame| frame.is_tile()).map_or_else(|| stable_hash(name), |frame| frame.svg_seed)
}

/// SVG renders, one per canvas like the Darkly ones, each written as
/// `{page}_{canvas}.svg` and rasterised to `{page}_{canvas}.png`. Here a
/// style's width is the stroke's geometric width; in Darkly it scales the
/// brush's pressure instead (see `new_style`).
#[cfg(not(target_arch = "wasm32"))]
pub fn svgs(entry: &Entry, dir: &str, scale: f32, only: Option<&[usize]>) -> Result<(), String> {
    let mut progress = pages_progress(entry, "SVGs", only);
    for (i, page) in entry.selected_pages(only) {
        progress.start(&page.label(i));
        for (label, canvas) in page.rendered_canvases() {
            let name = format!("{}_{}", page.label(i), label);
            let (svg_file, png_file) = (format!("{}/{}.svg", dir, name), format!("{}/{}.png", dir, name));
            let seed = canvas_svg_seed(&canvas, &name);
            let (svg, png) = canvas.to_svg_and_png(2.0 * scale, page.svg_style(), seed);
            std::fs::write(&svg_file, svg).map_err(|e| write_error(&svg_file, e))?;
            std::fs::write(&png_file, png).map_err(|e| write_error(&png_file, e))?;
        }
        progress.step();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stitches_are_named_for_their_pages() {
        assert_eq!(stitch_label("becoming_us_page_0", "becoming_us_page_5"), "becoming_us_page_0-5");
        assert_eq!(stitch_label("page_8", "page_11"), "page_8-11");
        assert_eq!(stitch_label("03_pens_brushes", "05_pens_0"), "03_pens_brushes-05_pens_0");
    }

    #[test]
    fn stitching_lays_images_side_by_side_tops_level() {
        let red = RgbaImage::from_pixel(2, 3, image::Rgba([255, 0, 0, 255]));
        let blue = RgbaImage::from_pixel(4, 1, image::Rgba([0, 0, 255, 255]));
        let both = stitched(&[red, blue]);
        assert_eq!((both.width(), both.height()), (6, 3));
        assert_eq!(both.get_pixel(1, 2).0, [255, 0, 0, 255]);
        assert_eq!(both.get_pixel(2, 0).0, [0, 0, 255, 255]);
        // Below the shorter image, nothing.
        assert_eq!(both.get_pixel(5, 2).0[3], 0);
    }

    #[test]
    fn decimals_are_cut_and_nothing_else() {
        assert_eq!(
            trim_decimals(r#"<path d="M 100.2043 -91.21536 L 3 4.5" stroke-width="1.25" id="gradient_7"/>v1.x"#, 2),
            r#"<path d="M 100.20 -91.21 L 3 4.5" stroke-width="1.25" id="gradient_7"/>v1.x"#,
        );
    }
}
