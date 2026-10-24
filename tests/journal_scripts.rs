//! Evaluates every entry of every journal (without rendering) so API
//! changes that break a journal fail `cargo test` in well under a second,
//! instead of surfacing minutes into a render.

use scribe::journal::rhaientries::{Journal, Resources};
use std::path::PathBuf;

fn journals(dir: &str) -> Vec<PathBuf> {
    let mut journals: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", dir, e))
        .map(|entry| entry.expect("Failed to read directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    journals.sort();
    journals
}

fn check_journals(dirs: &[PathBuf]) {
    let mut failures = Vec::new();
    for dir in dirs {
        let journal = match Journal::open(dir) {
            Ok(journal) => journal,
            Err(err) => {
                failures.push(err);
                continue;
            }
        };
        for name in &journal.manifest.entries {
            let built = journal.engine(Resources::library()).and_then(|mut engine| journal.build_entry(&mut engine, name));
            match built {
                Ok(entry) if entry.page_count() > 0 => {}
                Ok(_) => failures.push(format!("{}/{}: produced no pages", journal.name(), name)),
                Err(err) => failures.push(format!("{}/{}: {}", journal.name(), name, err)),
            }
        }
    }
    assert!(failures.is_empty(), "journal entries failed:\n{}", failures.join("\n"));
}

#[test]
fn every_journal_entry_evaluates() {
    let journals = journals(scribe::journal::rhaientries::DEFAULT_JOURNALS_DIR);
    assert!(!journals.is_empty(), "no journals found in webapp/journals/");
    check_journals(&journals);
}

#[test]
fn smoke_fixture_evaluates() {
    let journals = journals(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/smoke_journals"));
    assert!(!journals.is_empty(), "smoke fixture missing from tests/smoke_journals/");
    check_journals(&journals);
}
