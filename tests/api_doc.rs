//! Keeps docs/api.md, the reference for everything journal scripts can
//! call, up to date and its examples true (see CLAUDE.md):
//!
//! - every function scripts can call is named in an entry's heading, and
//!   every file of the library (webapp/library/) somewhere in the doc's code;
//! - every ```` ```rhai render=NAME ```` example runs, after the doc's
//!   ```` ```rhai prelude ```` blocks, is followed by its image,
//!   `![NAME](images/NAME.svg)`, and draws what that image shows: the two
//!   are rasterised and compared with a tolerance, as tests/svg_renders.rs
//!   compares renders. An example draws on `page`, a canvas the prelude
//!   makes.
//!
//! When an example, or what it draws, changes, look at the new images and
//! diffs written to target/api-doc-diffs/, then rewrite the images:
//!
//! ```bash
//! UPDATE_DOC_IMAGES=1 cargo test --release --test api_doc
//! ```

mod common;

use image::{Rgba, RgbaImage};
use resvg::{tiny_skia, usvg};
use scribe::journal::publish::page_svg_renders;
use scribe::journal::rhaientries::{JournalEngine, Resources};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// What the images are drawn on, so they read on light and dark pages alike.
const BACKGROUND: &str = "#faf6ee";

fn doc() -> String {
    std::fs::read_to_string(format!("{}/docs/api.md", ROOT)).expect("docs/api.md")
}

/// A fenced block: its info string, its code, the line it opens on, and the
/// first line after it that isn't blank.
struct Block {
    info: String,
    code: String,
    line: usize,
    next: Option<String>,
}

fn blocks(doc: &str) -> Vec<Block> {
    let lines: Vec<&str> = doc.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(info) = lines[i].strip_prefix("```") else {
            i += 1;
            continue;
        };
        let line = i + 1;
        let mut code = String::new();
        i += 1;
        while i < lines.len() && !lines[i].starts_with("```") {
            code.push_str(lines[i]);
            code.push('\n');
            i += 1;
        }
        i += 1;
        let next = lines.get(i..).unwrap_or_default().iter().find(|l| !l.trim().is_empty()).map(|l| l.to_string());
        blocks.push(Block { info: info.trim().to_string(), code, line, next });
    }
    blocks
}

/// The doc's lines outside fenced blocks.
fn prose(doc: &str) -> Vec<&str> {
    let mut fenced = false;
    doc.lines().filter(|line| {
        if line.starts_with("```") {
            fenced = !fenced;
            return false;
        }
        !fenced
    }).collect()
}

/// The `code` spans of a line.
fn code_spans(line: &str) -> Vec<&str> {
    line.split('`').skip(1).step_by(2).collect()
}

/// Whether `word` is in `text` as a whole word (or `module::name`).
fn mentions(text: &str, word: &str) -> bool {
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

fn engine() -> JournalEngine {
    let library = Resources::library();
    let symd = Rc::new(library.glyphset_book().unwrap());
    JournalEngine::with_resources(symd, library)
}

#[test]
fn every_function_scripts_can_call_has_an_entry() {
    let doc = doc();
    let headings: Vec<&str> = prose(&doc).into_iter().filter(|line| line.starts_with('#')).flat_map(code_spans).collect();
    assert!(headings.len() > 100, "found only {} headings' code in docs/api.md", headings.len());
    let missing: Vec<String> = engine().function_names().into_iter()
        // Properties: `get$name` and `set$name`.
        .map(|name| name.trim_start_matches("get$").trim_start_matches("set$").to_string())
        .filter(|name| !headings.iter().any(|heading| mentions(heading, name)))
        .collect::<BTreeSet<_>>().into_iter().collect();
    assert!(missing.is_empty(), "add entries for these to docs/api.md, each named in its heading: {}", missing.join(", "));
}

#[test]
fn every_library_file_is_named() {
    let doc = doc();
    let spans: Vec<&str> = prose(&doc).into_iter().flat_map(code_spans).collect();
    let mut missing = Vec::new();
    let library = PathBuf::from(format!("{}/webapp/library", ROOT));
    for dir in std::fs::read_dir(&library).unwrap().flatten().filter(|entry| entry.path().is_dir()) {
        let kind = dir.file_name().into_string().unwrap();
        for file in std::fs::read_dir(dir.path()).unwrap().flatten() {
            let path = file.path();
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else { continue };
            if stem == "README" || path.is_dir() {
                continue;
            }
            if !spans.iter().any(|span| mentions(span, stem)) {
                missing.push(format!("{}/{}", kind, file.file_name().to_string_lossy()));
            }
        }
    }
    missing.sort();
    assert!(missing.is_empty(), "name these library files in docs/api.md (in code, as `name`): {}", missing.join(", "));
}

#[test]
fn examples_draw_what_their_images_show() {
    let doc = doc();
    let blocks = blocks(&doc);
    let prelude: String = blocks.iter().filter(|block| block.info == "rhai prelude").map(|block| block.code.as_str()).collect();
    assert!(!prelude.is_empty(), "docs/api.md has no ```rhai prelude block");
    let images = PathBuf::from(format!("{}/docs/images", ROOT));
    let diffs = PathBuf::from(format!("{}/target/api-doc-diffs", ROOT));
    let _ = std::fs::remove_dir_all(&diffs);
    let update = std::env::var_os("UPDATE_DOC_IMAGES").is_some();
    if update {
        std::fs::create_dir_all(&images).unwrap();
    }

    let mut names = BTreeSet::new();
    let mut failures = Vec::new();
    for block in &blocks {
        let Some(name) = block.info.strip_prefix("rhai render=") else { continue };
        let at = format!("docs/api.md line {} ({})", block.line, name);
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            failures.push(format!("{}: name examples with lowercase letters, digits and underscores", at));
            continue;
        }
        if !names.insert(name.to_string()) {
            failures.push(format!("{}: another example has this name", at));
            continue;
        }
        let link = format!("![{}](images/{}.svg)", name, name);
        if block.next.as_deref() != Some(link.as_str()) {
            failures.push(format!("{}: follow it with its image, {}", at, link));
        }
        match render(&prelude, &block.code, name) {
            Err(err) => failures.push(format!("{}: {}", at, err)),
            Ok(svg) => {
                let file = images.join(format!("{}.svg", name));
                if update {
                    std::fs::write(&file, &svg).unwrap();
                    continue;
                }
                let problem = match std::fs::read_to_string(&file) {
                    Err(_) => Some("no image in docs/images".to_string()),
                    Ok(committed) => common::compare(&rasterise(&committed), &rasterise(&svg), &diffs, name),
                };
                if let Some(problem) = problem {
                    std::fs::create_dir_all(&diffs).unwrap();
                    std::fs::write(diffs.join(format!("{}.svg", name)), &svg).unwrap();
                    failures.push(format!("{}: {}", at, problem));
                }
            }
        }
    }

    // Images no example draws.
    let drawn: BTreeSet<String> = names.iter().map(|name| format!("{}.svg", name)).collect();
    let images_there: BTreeSet<String> = std::fs::read_dir(&images).map(|entries| entries.flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()).unwrap_or_default();
    for file in images_there.difference(&drawn) {
        if update {
            std::fs::remove_file(images.join(file)).unwrap();
        } else {
            failures.push(format!("docs/images/{}: no example draws it", file));
        }
    }
    assert!(!names.is_empty(), "docs/api.md has no ```rhai render=NAME examples");
    assert!(
        failures.is_empty(),
        "{}\nNew images and diffs are in {}. If the change is meant, update them with \
         UPDATE_DOC_IMAGES=1 cargo test --release --test api_doc",
        failures.join("\n"),
        diffs.display()
    );
}

/// The SVG an example draws: the prelude, then the example, then `page`
/// put on a page of its own, in plain strokes (no svg_style), which keeps
/// the images small.
fn render(prelude: &str, code: &str, name: &str) -> Result<String, String> {
    let script = format!("{}\n{}\njournal_entry.add(new_page(#{{ name: \"{}\", margin: 16, svg_style: () }}).add(page));\n", prelude, code, name);
    let entry = engine().build_entry_str(&script)?;
    let (_, page) = entry.selected_pages(None).next().ok_or("it made no page")?;
    let mut renders = page_svg_renders(page, 0, 1.0);
    if renders.len() != 1 {
        return Err(format!("it should draw one canvas, page, but drew {}", renders.len()));
    }
    let (_, svg, _) = renders.remove(0);
    Ok(backed(&shortened(&svg)))
}

/// The SVG with its paths' coordinates to a tenth of a unit, a fraction of
/// a pixel, so the images are smaller.
fn shortened(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(at) = rest.find(" d=\"") {
        out.push_str(&rest[..at + 4]);
        rest = &rest[at + 4..];
        let end = rest.find('"').unwrap_or(rest.len());
        let numbers: Vec<String> = rest[..end].split(' ').map(|token| match token.parse::<f64>() {
            Ok(n) if token.contains('.') => {
                let short = format!("{:.1}", n);
                short.strip_suffix(".0").map(str::to_string).unwrap_or(short)
            }
            _ => token.to_string(),
        }).collect();
        out.push_str(&numbers.join(" "));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The SVG on a background of its own.
fn backed(svg: &str) -> String {
    let view = svg.split("viewBox=\"").nth(1).and_then(|rest| rest.split('"').next()).unwrap_or("0 0 0 0");
    let [x, y, width, height]: [&str; 4] = view.split(' ').collect::<Vec<_>>().try_into().unwrap_or(["0", "0", "100%", "100%"]);
    let Some(open) = svg.find('>') else { return svg.to_string() };
    format!("{}<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" />{}", &svg[..=open], x, y, width, height, BACKGROUND, &svg[open + 1..])
}

/// The SVG rasterised at a pixel a unit.
fn rasterise(svg: &str) -> RgbaImage {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("an SVG");
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).expect("an SVG with an area");
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    let mut image = RgbaImage::new(size.width(), size.height());
    for (from, to) in pixmap.pixels().iter().zip(image.pixels_mut()) {
        let c = from.demultiply();
        *to = Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    image
}
