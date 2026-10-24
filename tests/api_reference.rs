//! Keeps webapp/journals/api_reference up to date: every function scripts can call
//! should have an entry there. This fails when one isn't in any entry's
//! signature (see CLAUDE.md).

use scribe::journal::rhaientries::{JournalEngine, Resources};
use std::rc::Rc;

const REFERENCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/webapp/journals/api_reference/entries/api_reference.rhai");

/// The signature of every `api("signature", "description")` entry.
fn signatures(script: &str) -> Vec<&str> {
    script.match_indices("api(\"")
        .filter_map(|(at, open)| {
            let rest = &script[at + open.len()..];
            rest.find('"').map(|end| &rest[..end])
        })
        .collect()
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
fn every_function_scripts_can_call_has_an_entry() {
    let script = std::fs::read_to_string(REFERENCE).expect("webapp/journals/api_reference/entries/api_reference.rhai");
    let signatures = signatures(&script);
    assert!(signatures.len() > 100, "found only {} api(...) entries in the reference", signatures.len());
    let library = Resources::library();
    let symd = Rc::new(library.glyphset_book().unwrap());
    let missing: Vec<String> = JournalEngine::with_resources(symd, library).function_names().into_iter()
        // Properties: `get$name` and `set$name`.
        .map(|name| name.trim_start_matches("get$").trim_start_matches("set$").to_string())
        .filter(|name| !signatures.iter().any(|signature| mentions(signature, name)))
        .collect();
    assert!(missing.is_empty(), "add entries for these to webapp/journals/api_reference: {}", missing.join(", "));
}
