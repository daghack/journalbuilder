//! Keeps webapp/journals/showcase up to date: it should show everything journals
//! can draw with. These fail when something scripts can call, or a built-in
//! (a lock type, illustration, chain, layout, text style, content or
//! glyphset of the library's) isn't in it (see CLAUDE.md). Brushes and
//! glyphs needn't be listed: the showcase draws every one of
//! `brush_names()`, and every variant of every glyph of every glyphset in
//! `glyphset_names()`.

use scribe::journal::rhaientries::{Journal, JournalEngine, Resources, BUILT_IN_JOURNALS_DIR};
use std::rc::Rc;

/// Functions scripts can call that aren't capabilities to show.
const NOT_SHOWN: &[(&str, &str)] = &[
    ("to_string", "printing"),
    ("to_debug", "printing"),
];

/// Every entry of the showcase, one after another.
fn showcase() -> String {
    let journal = Journal::open(format!("{}/showcase", BUILT_IN_JOURNALS_DIR)).unwrap_or_else(|e| panic!("{}", e));
    journal.manifest.entries.iter().map(|name| journal.entry_source(name).unwrap()).collect::<Vec<_>>().join("\n")
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
    let library = Resources::library();
    let symd = Rc::new(library.glyphset_book().unwrap());
    let script = showcase();
    let missing: Vec<String> = JournalEngine::with_resources(symd, library).function_names().into_iter()
        // Properties: `get$name` and `set$name`.
        .map(|name| name.trim_start_matches("get$").trim_start_matches("set$").to_string())
        .filter(|name| !NOT_SHOWN.iter().any(|(n, _)| n == name))
        .filter(|name| !mentions(&script, name))
        .collect();
    assert!(missing.is_empty(), "add examples of these to webapp/journals/showcase (or, if they aren't capabilities, to NOT_SHOWN in tests/showcase.rs): {}", missing.join(", "));
}

#[test]
fn every_built_in_is_in_the_showcase() {
    let library = Resources::library();
    let script = showcase();
    let mut missing = Vec::new();
    let mut check = |kind: &str, names: Vec<String>, shown: &dyn Fn(&str) -> String, where_: &str| {
        for name in names {
            if !script.contains(&shown(&name)) {
                missing.push(format!("{} {} ({})", kind, name, where_));
            }
        }
    };
    let names = |sources: &scribe::journal::rhaientries::Sources| sources.names().unwrap();
    check("lock type", names(&library.locks), &|n| format!("\"{}\"", n), "a row on the locks page");
    check("illustration", names(&library.illustrations), &|n| format!("\"{}\"", n), "attach it on the illustrations page");
    check("chain", names(&library.chains), &|n| format!("chain(\"{}\"", n), "use it with chain(name)");
    check("layout", names(&library.layouts), &|n| format!("layout(\"{}\"", n), "typeset pages with it");
    check("text style", names(&library.styles), &|n| format!("text_style(\"{}\"", n), "write with it on the text styles page");
    check("content", library.content_names(), &|n| format!("content(\"{}\"", n), "use it with content(name)");
    // Glyphsets of 20 glyphs or more; smaller sets (the symbols) are only
    // in the glyph catalogue, which draws every glyph.
    let book = library.glyphset_book().unwrap();
    let glyphsets = book.names().into_iter()
        .filter(|name| !name.starts_with("debug") && book.font(name).unwrap().characters().len() >= 20)
        .collect();
    check("glyphset", glyphsets, &|n| format!("\"{}\"", n), "a line on the glyphsets pages");
    assert!(missing.is_empty(), "the showcase is missing built-ins:\n  {}", missing.join("\n  "));
}

#[test]
fn the_showcase_draws_every_brush_and_glyph() {
    let script = showcase();
    assert!(script.contains("brush_names()"), "the pens page should draw every brush from brush_names()");
    assert!(script.contains("glyphset_names()") && script.contains(".variants("), "the glyphs page should draw every variant of every glyphset's glyphs, from glyphset_names()");
}
