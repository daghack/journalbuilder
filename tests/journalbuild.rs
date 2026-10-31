//! `journalbuild` as it's run: an entry that fails doesn't stop the others
//! rendering, and the run says which failed and why, and exits non-zero.

use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_journalbuild");

const DRAWS: &str = r#"
let canvas = new_canvas();
canvas.add(new_circle(vec2(0.0, 0.0), 60.0), new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style()));
journal_entry.add(new_page().add(canvas));
"#;

fn write_journal(dir: &Path, entries: &[(&str, &str)]) {
    std::fs::create_dir_all(dir.join("entries")).unwrap();
    let names: Vec<String> = entries.iter().map(|(name, _)| format!("{:?}", name)).collect();
    std::fs::write(dir.join("journal.json"), format!(r#"{{ "version": 1, "title": "T", "entries": [{}] }}"#, names.join(", "))).unwrap();
    for (name, source) in entries {
        std::fs::write(dir.join("entries").join(format!("{}.rhai", name)), source).unwrap();
    }
}

#[test]
fn an_entry_that_fails_doesnt_stop_the_others() {
    // It renders with Darkly, so without a GPU there's nothing to check.
    if !Command::new(BIN).arg("--check-gpu").output().unwrap().status.success() {
        eprintln!("skipping: no GPU for Darkly");
        return;
    }
    let dir = std::env::temp_dir().join(format!("scribe-journalbuild-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let journal = dir.join("partly_broken");
    let out = dir.join("out");
    write_journal(&journal, &[
        ("first", DRAWS),
        ("throws", "throw \"deliberately failing\";"),
        ("syntax", "let oops = ;"),
        ("last", DRAWS),
    ]);

    let run = Command::new(BIN).arg(&journal).args(["--scale", "0.1", "--supersample", "1", "--output-dir"]).arg(&out).output().unwrap();
    let stderr = String::from_utf8_lossy(&run.stderr);

    assert!(!run.status.success(), "should fail: {}", stderr);
    for (name, rendered) in [("first", true), ("throws", false), ("syntax", false), ("last", true)] {
        assert_eq!(out.join(name).join("darkly_page_0_0.png").is_file(), rendered, "{}'s render: {}", name, stderr);
    }
    let summary = stderr.split("2 of 4 entries failed").nth(1).unwrap_or_else(|| panic!("no summary: {}", stderr));
    assert!(summary.contains("throws") && summary.contains("deliberately failing"), "{}", summary);
    assert!(summary.contains("syntax") && summary.contains("Syntax error"), "{}", summary);
    assert!(stderr.contains("Rendered 2 pages") && stderr.contains("(first, last)"), "{}", stderr);

    let _ = std::fs::remove_dir_all(&dir);
}
