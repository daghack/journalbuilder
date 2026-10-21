//! Renders journals as SVG (rasterised on the CPU by resvg, as
//! `journalbuild --svg` does) and compares each canvas's image with the one
//! committed in tests/svg_renders/<journal>/. These need no GPU, so they
//! check what journals draw where Darkly can't run (a cloud sandbox, CI).
//!
//! Images are compared with a tolerance, so float and rasteriser differences
//! between machines pass while moved, missing or restyled strokes don't. On
//! a failure the new render and a diff (changed pixels in red) are written
//! to target/svg-render-diffs/<journal>/ to look at.
//!
//! When a change means to alter what's drawn, look at the diffs, then
//! update the committed images and commit them with the change:
//!
//! ```bash
//! UPDATE_SVG_RENDERS=1 cargo test --release --test svg_renders
//! ```

use image::{Rgba, RgbaImage};
use scribe::journal::fonts::{FontBook, DEFAULT_FONTS};
use scribe::journal::publish::page_svg_renders;
use scribe::journal::rhaientries::JournalEngine;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Passed to `page_svg_renders` like `journalbuild --scale`: a tenth of full
/// size keeps the committed images small but strokes still visible.
const SCALE: f32 = 0.1;

/// A pixel matches another when no channel (composited on white) is off
/// by more than this...
const PIXEL_TOLERANCE: u8 = 32;
/// ...and differs when it matches nothing this many pixels or fewer away
/// in the other image, either way round. Floats come out a last bit
/// different on other machines and builds (their sin and cos differ), and
/// a few scripts' choices turn that into a shift of a few units, a pixel
/// or less here; strokes moved further, missing or restyled still differ.
const SHIFT_PX: i64 = 1;
/// An image differs when more than this fraction of its pixels do.
const MAX_DIFFERING: f64 = 0.0005;

/// Which pages of a journal to render: all, or only some (big journals
/// would make slow tests and large images).
enum Pages {
    All,
    Only(&'static [usize]),
}

fn check(journals_dir: &str, journal: &str, pages: Pages) {
    let symd = Rc::new(FontBook::from_file(DEFAULT_FONTS).expect("Failed to load the fonts"));
    let mut engine = JournalEngine::new(symd);
    let script = format!("{}/{}/{}/journal_entry.rhai", ROOT, journals_dir, journal);
    let entry = engine.build_entry_file(&script).unwrap_or_else(|e| panic!("{}: {}", journal, e));
    let only = match pages {
        Pages::All => None,
        Pages::Only(pages) => Some(pages),
    };

    let expected_dir = PathBuf::from(format!("{}/tests/svg_renders/{}", ROOT, journal));
    let diff_dir = PathBuf::from(format!("{}/target/svg-render-diffs/{}", ROOT, journal));
    let _ = std::fs::remove_dir_all(&diff_dir);
    let update = std::env::var_os("UPDATE_SVG_RENDERS").is_some();
    if update {
        std::fs::create_dir_all(&expected_dir).unwrap();
    }

    let mut rendered = BTreeSet::new();
    let mut failures = Vec::new();
    for (i, page) in entry.selected_pages(only) {
        for (name, _svg, png) in page_svg_renders(page, i, SCALE) {
            let file = format!("{}.png", name);
            let expected_path = expected_dir.join(&file);
            rendered.insert(file.clone());
            if update {
                std::fs::write(&expected_path, &png).unwrap();
                continue;
            }
            let actual = image::load_from_memory(&png).expect("the render is a PNG").to_rgba8();
            let problem = match std::fs::read(&expected_path) {
                Err(_) => Some("no committed image".to_string()),
                Ok(bytes) => compare(&image::load_from_memory(&bytes).expect("a PNG").to_rgba8(), &actual, &diff_dir, &name),
            };
            if let Some(problem) = problem {
                std::fs::create_dir_all(&diff_dir).unwrap();
                std::fs::write(diff_dir.join(&file), &png).unwrap();
                failures.push(format!("{}: {}", file, problem));
            }
        }
    }

    // Committed images of canvases no longer drawn.
    for file in committed(&expected_dir).difference(&rendered) {
        if update {
            std::fs::remove_file(expected_dir.join(file)).unwrap();
        } else {
            failures.push(format!("{}: committed, but the canvas is no longer drawn", file));
        }
    }
    assert!(!rendered.is_empty(), "{} drew no canvases", journal);
    assert!(
        failures.is_empty(),
        "{}'s SVG renders differ from tests/svg_renders/{}:\n  {}\nNew renders and diffs are in {}. If the change is meant, \
         update them with UPDATE_SVG_RENDERS=1 cargo test --release --test svg_renders",
        journal, journal, failures.join("\n  "), diff_dir.display()
    );
}

fn committed(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir).map(|entries| {
        entries.filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| name.ends_with(".png"))
            .collect()
    }).unwrap_or_default()
}

/// Why `actual` doesn't match `expected`, if it doesn't; then a diff image
/// (`{name}.diff.png`) goes in `diff_dir`.
fn compare(expected: &RgbaImage, actual: &RgbaImage, diff_dir: &Path, name: &str) -> Option<String> {
    if expected.dimensions() != actual.dimensions() {
        return Some(format!("size changed from {:?} to {:?}", expected.dimensions(), actual.dimensions()));
    }
    let (width, height) = actual.dimensions();
    let (expected, actual) = (white_backed(expected), white_backed(actual));
    let at = |image: &[[u8; 3]], x: i64, y: i64| image[(y * width as i64 + x) as usize];
    let matches = |p: [u8; 3], q: [u8; 3]| (0..3).all(|c| p[c].abs_diff(q[c]) <= PIXEL_TOLERANCE);
    // Whether pixel (x, y) of `one` matches a pixel near it in `other`.
    let near = |one: &[[u8; 3]], other: &[[u8; 3]], x: i64, y: i64| {
        let p = at(one, x, y);
        (-SHIFT_PX..=SHIFT_PX).any(|dy| (-SHIFT_PX..=SHIFT_PX).any(|dx| {
            let (nx, ny) = (x + dx, y + dy);
            nx >= 0 && ny >= 0 && nx < width as i64 && ny < height as i64 && matches(p, at(other, nx, ny))
        }))
    };
    let mut diff = RgbaImage::new(width, height);
    let mut differing = 0usize;
    for (x, y, d) in diff.enumerate_pixels_mut() {
        let (x, y) = (x as i64, y as i64);
        let a = at(&actual, x, y);
        if !matches(at(&expected, x, y), a) && !(near(&actual, &expected, x, y) && near(&expected, &actual, x, y)) {
            differing += 1;
            *d = Rgba([255, 0, 0, 255]);
        } else {
            // The render, faded, to show where the changes are.
            let grey = ((a[0] as u32 + a[1] as u32 + a[2] as u32) / 3 / 4 + 191) as u8;
            *d = Rgba([grey, grey, grey, 255]);
        }
    }
    let fraction = differing as f64 / (width * height) as f64;
    if fraction <= MAX_DIFFERING {
        return None;
    }
    std::fs::create_dir_all(diff_dir).unwrap();
    diff.save(diff_dir.join(format!("{}.diff.png", name))).unwrap();
    Some(format!("{:.2}% of pixels differ (at most {:.2}% may)", fraction * 100.0, MAX_DIFFERING * 100.0))
}

/// The image's pixels composited on white, row by row.
fn white_backed(image: &RgbaImage) -> Vec<[u8; 3]> {
    image.pixels().map(|p| {
        let alpha = p[3] as u32;
        let blend = |c: u8| ((c as u32 * alpha + 255 * (255 - alpha)) / 255) as u8;
        [blend(p[0]), blend(p[1]), blend(p[2])]
    }).collect()
}

#[test]
fn smoke_fixture() {
    check("tests/smoke_journals", "smoke", Pages::All);
}

#[test]
fn showcase() {
    check("webapp/journals", "showcase", Pages::All);
}

/// Page 0 of the other journals, as scripts/smoke.sh renders them.
macro_rules! first_pages {
    ($($journal:ident),* $(,)?) => {
        const CHECKED_JOURNALS: &[&str] = &[$(stringify!($journal)),*];
        $(
            #[test]
            fn $journal() {
                check("webapp/journals", stringify!($journal), Pages::Only(&[0]));
            }
        )*
    };
}

first_pages!(
    api_reference,
    becoming_us,
    black_priests,
    copper_letter_1,
    first_codex,
    journal_entry_17,
    journal_entry_18,
    lang_font,
    loche_rules,
    matter_of_pyrius,
    nivelia_letter,
    nivelia_response,
    raven_constellations,
    welcome_home,
);

#[test]
fn every_journal_is_checked() {
    let checked = [CHECKED_JOURNALS, &["showcase"]].concat();
    let missing: Vec<String> = std::fs::read_dir(format!("{}/webapp/journals", ROOT)).unwrap()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| Path::new(&format!("{}/webapp/journals/{}/journal_entry.rhai", ROOT, name)).is_file())
        .filter(|name| !checked.contains(&name.as_str()))
        .collect();
    assert!(missing.is_empty(), "add these journals to first_pages! in tests/svg_renders.rs: {}", missing.join(", "));
}
