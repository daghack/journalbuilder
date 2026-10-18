//! Evaluates every rhai journal script (without rendering) so API changes
//! that break a journal fail `cargo test` in well under a second, instead of
//! surfacing minutes into a render.

use scribe::journal::rhaientries::JournalEngine;
use scribe::journal::fonts::FontBook;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn rhai_journals(dir: &str) -> Vec<PathBuf> {
    let mut scripts: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", dir, e))
        .map(|entry| entry.expect("Failed to read directory entry").path().join("journal_entry.rhai"))
        .filter(|script| script.is_file())
        .collect();
    scripts.sort();
    scripts
}

fn check_scripts(scripts: &[PathBuf]) {
    let symd = Rc::new(FontBook::from_file(scribe::journal::fonts::DEFAULT_FONTS).expect("Failed to load fonts.json"));
    let failures: Vec<String> = scripts.iter()
        .filter_map(|script| {
            let mut engine = JournalEngine::new(symd.clone());
            match engine.build_entry_file(script.to_str().unwrap()) {
                Ok(entry) if entry.page_count() > 0 => None,
                Ok(_) => Some(format!("{}: produced no pages", display(script))),
                Err(err) => Some(format!("{}: {}", display(script), err)),
            }
        })
        .collect();
    assert!(failures.is_empty(), "journal scripts failed:\n{}", failures.join("\n"));
}

fn display(path: &Path) -> String {
    path.parent().and_then(Path::file_name).map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

#[test]
fn every_rhai_journal_evaluates() {
    let scripts = rhai_journals(concat!(env!("CARGO_MANIFEST_DIR"), "/webapp/journals"));
    assert!(!scripts.is_empty(), "no rhai journals found in webapp/journals/");
    check_scripts(&scripts);
}

#[test]
fn smoke_fixture_evaluates() {
    let scripts = rhai_journals(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/smoke_journals"));
    assert!(!scripts.is_empty(), "smoke fixture missing from tests/smoke_journals/");
    check_scripts(&scripts);
}
