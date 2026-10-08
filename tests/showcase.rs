//! Keeps journals/showcase up to date: it should show everything journals
//! can draw with. These fail when something scripts can call, a font, or a
//! lock type isn't in it (see CLAUDE.md). Brushes and glyphs needn't be
//! listed: the showcase draws every one of `brush_names()`, and every
//! variant of every glyph of every font in `font_names()`.

use scribe::journal::rhaientries::JournalEngine;
use scribe::journal::fonts::{FontBook, DEFAULT_FONTS};
use std::rc::Rc;

const SHOWCASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/journals/showcase/journal_entry.rhai");

/// Functions scripts can call that aren't capabilities to show.
const NOT_SHOWN: &[(&str, &str)] = &[
    ("to_string", "printing"),
    ("to_debug", "printing"),
];

fn showcase() -> String {
    std::fs::read_to_string(SHOWCASE).expect("journals/showcase/journal_entry.rhai")
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

#[test]
fn every_function_scripts_can_call_is_in_the_showcase() {
    let symd = Rc::new(FontBook::from_file(DEFAULT_FONTS).unwrap());
    let script = showcase();
    let missing: Vec<String> = JournalEngine::new(symd).function_names().into_iter()
        // Properties: `get$name` and `set$name`.
        .map(|name| name.trim_start_matches("get$").trim_start_matches("set$").to_string())
        .filter(|name| !NOT_SHOWN.iter().any(|(n, _)| n == name))
        .filter(|name| !mentions(&script, name))
        .collect();
    assert!(missing.is_empty(), "add examples of these to journals/showcase (or, if they aren't capabilities, to NOT_SHOWN in tests/showcase.rs): {}", missing.join(", "));
}

#[test]
fn every_font_is_in_the_showcase() {
    // Fonts of 20 glyphs or more; smaller sets (archangel's seven, the
    // symbols) are only in the glyph catalogue, which draws every glyph.
    let book = FontBook::from_file(DEFAULT_FONTS).unwrap();
    let script = showcase();
    let missing: Vec<String> = book.names().into_iter()
        .filter(|name| !name.starts_with("debug") && book.font(name).unwrap().characters().len() >= 20)
        .filter(|name| !script.contains(&format!("\"{}\"", name)))
        .collect();
    assert!(missing.is_empty(), "add these fonts to the showcase's fonts page: {:?}", missing);
}

#[test]
fn every_lock_type_is_in_the_showcase() {
    let script = showcase();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/locks");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|x| x == "rhai") {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            if !script.contains(&format!("\"{}\"", name)) {
                missing.push(name);
            }
        }
    }
    assert!(missing.is_empty(), "add these lock types to the showcase's locks page: {:?}", missing);
}

#[test]
fn the_showcase_draws_every_brush_and_glyph() {
    let script = showcase();
    assert!(script.contains("brush_names()"), "the pens page should draw every brush from brush_names()");
    assert!(script.contains("font_names()") && script.contains(".variants("), "the glyphs page should draw every variant of every font's glyphs, from font_names()");
}
