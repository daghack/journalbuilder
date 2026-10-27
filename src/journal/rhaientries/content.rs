//! Content files, `content/<name>.txt`, `.md` or `.json` (a journal's over
//! the library's), for `content(name)`: text kept apart from the entries
//! that write it. Plain text (`.txt`) and marked-up text (`.md`) are given
//! back as strings, without the newline that ends the file
//! (`markdown(content(name))` styles the latter as it's set); symbols
//! (`.json`) as a value of symbols, as `symbols(base, text)` makes, for a
//! lock. A symbols file holds its text, `{ "base": 27, "text": "Hello,
//! world" }`, made to fit the base as it's read (`HELLO WORLD`), or, with
//! `"strict": true`, written in the base already; or the symbols
//! themselves, `{ "base": 27, "symbols": [7, 4, 11, ...] }`.

use rhai::{Dynamic, Engine, NativeCallContext};

use super::locks::{lenient_text, symbols_value};
use crate::lockbuilder::base_n_symbols;
use super::resources::Resources;
use super::RhaiResult;

pub fn register(engine: &mut Engine, resources: Resources) {
    engine.register_fn("content", move |_: NativeCallContext, name: &str| content(&resources, name));
}

fn content(resources: &Resources, name: &str) -> RhaiResult<Dynamic> {
    let kinds = resources.content_kinds(name);
    let read = |sources: &super::Sources| sources.read(name)
        .map_err(|e| format!("content: couldn't read {}: {}", sources.path(name).display(), e));
    match kinds.as_slice() {
        [] => {
            let names = resources.content_names();
            Err(format!("content: no content named '{}'; content: {}", name, if names.is_empty() { "none".to_string() } else { names.join(", ") }).into())
        }
        ["txt"] => Ok(text(read(&resources.text)?).into()),
        ["md"] => Ok(text(read(&resources.markdown)?).into()),
        ["json"] => {
            let path = resources.symbols.path(name);
            read_symbols(&read(&resources.symbols)?)
                .and_then(|(base, symbols)| symbols_value(base, &symbols))
                .map(Dynamic::from)
                .map_err(|e| format!("content: {}: {}", path.display(), e).into())
        }
        several => Err(format!("content: '{}' is ambiguous: there's {}; rename one", name,
            several.iter().map(|ext| format!("{}.{}", name, ext)).collect::<Vec<_>>().join(" and ")).into()),
    }
}

/// A text file's text without the newline that ends it.
fn text(mut text: String) -> String {
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    text
}

/// A symbols content file: its text, or its symbols.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SymbolsFile {
    base: u32,
    text: Option<String>,
    strict: Option<bool>,
    symbols: Option<Vec<u32>>,
}

/// A symbols content file's base and symbols: its text made to fit the
/// base (or, strict, read as it's written), or the symbols it holds.
pub(crate) fn read_symbols(json: &str) -> Result<(u32, Vec<u32>), String> {
    let file: SymbolsFile = serde_json::from_str(json)
        .map_err(|e| format!("should be {{ \"base\": N, \"text\": \"...\" }} or {{ \"base\": N, \"symbols\": [...] }}: {}", e))?;
    let base = file.base;
    if !(2..=27).contains(&base) {
        return Err(format!("symbols: base must be between 2 and 27, got {}", base));
    }
    match (file.text, file.symbols) {
        (Some(text), None) if file.strict == Some(true) => base_n_symbols(base, &text).map_err(|e| format!("symbols (strict): {}", e)),
        (Some(text), None) => base_n_symbols(base, &lenient_text(base, &text)).map_err(|e| format!("symbols: {}", e)),
        (None, Some(_)) if file.strict.is_some() => Err("strict goes with text, not symbols".to_string()),
        (None, Some(symbols)) => Ok(symbols),
        _ => Err("should have its text or its symbols, not both or neither".to_string()),
    }
    .map(|symbols| (base, symbols))
}

#[cfg(test)]
mod tests {
    use super::super::{JournalEngine, Resources};
    use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        let mut resources = Resources::library();
        let files = |files: &[(&str, &str)]| files.iter().map(|(n, s)| (n.to_string(), s.to_string())).collect::<Vec<_>>();
        resources.text = resources.text.with_overlay(files(&[("note", "a note\n"), ("both", "plain")]));
        resources.markdown = resources.markdown.with_overlay(files(&[("marked", "a *marked* note"), ("both", "marked")]));
        resources.symbols = resources.symbols.with_overlay(files(&[
            ("secret", r#"{ "base": 27, "symbols": [8, 9] }"#),
            ("bad", r#"{ "base": 3, "symbols": [4] }"#),
            ("written", r#"{ "base": 27, "text": "It's — hi!" }"#),
            ("strict", r#"{ "base": 27, "text": "HI THERE", "strict": true }"#),
            ("too_strict", r#"{ "base": 27, "text": "Hi, there", "strict": true }"#),
            ("mixed", r#"{ "base": 27, "text": "HI", "symbols": [8, 9] }"#),
        ]));
        JournalEngine::with_resources(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).unwrap()), resources)
    }

    #[test]
    fn content_is_text_or_symbols() {
        let result = engine().build_entry_str(r#"
            assert(content("note") == "a note", "plain text");
            assert(content("marked") == "a *marked* note", "marked-up text, as written");
            let secret = content("secret");
            assert(secret.base == 27 && secret.text() == "HI", "symbols");
            assert(content("written").text() == "ITS HI", "text made to fit");
            assert(content("strict").text() == "HI THERE", "strict text");
        "#);
        assert!(result.is_ok(), "{:?}", result.err());
        for (script, want) in [
            (r#"content("nope")"#, "content: no content named 'nope'; content: "),
            (r#"content("both")"#, "content: 'both' is ambiguous: there's both.txt and both.md"),
            (r#"content("bad")"#, "bad.json: symbols: symbol 4 (item 0) is outside Base3's 0-2"),
            (r#"content("too_strict")"#, "too_strict.json: symbols (strict): ',' isn't a base-27 symbol"),
            (r#"content("mixed")"#, "mixed.json: should have its text or its symbols, not both or neither"),
        ] {
            let err = engine().build_entry_str(script).expect_err(script);
            assert!(err.contains(want), "{}\n  wanted: {}\n  got: {}", script, want, err);
        }
    }
}
