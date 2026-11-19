//! A journal on disk: a directory holding its manifest, `journal.json`,
//! its entries, `entries/<name>.rhai`, each a script that builds a
//! `journal_entry` of pages, and its own resources (see resources.rs), in
//! front of the built-in library's.
//!
//! ```json
//! {
//!     "version": 1,
//!     "title": "Nivelia's Letters",
//!     "description": "Her letter, and the reply.",
//!     "entries": ["letter_1", "response_1"]
//! }
//! ```
//!
//! `entries` lists the entries in order. `bundled` lists library files an
//! export copied in alongside the journal's own (`locks/huffman.rhai`), so
//! an import can tell them from files the journal changed, and drop them.
//! `linked_content` lists content kept outside the journal, read as it's
//! asked for (see `Resources::with_linked_content`): folders of content
//! files, or content files, each a path from the journal's directory or an
//! absolute one (`~/` for the home directory).

use super::resources::Resources;
use super::sources::Sources;
use super::{Edits, Entry, JournalEngine};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The manifest's file name.
pub const MANIFEST: &str = "journal.json";

/// The manifest version this build writes and reads.
pub const MANIFEST_VERSION: u32 = 1;

/// A journal's `journal.json`.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The entries, in order: `entries/<name>.rhai` each.
    pub entries: Vec<String>,
    /// Library files an export copied in, as paths from the journal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundled: Vec<String>,
    /// Content outside the journal, in front of its own: folders and files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_content: Vec<String>,
}

impl Manifest {
    /// The manifest in `text`, checked.
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let manifest: Manifest = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if manifest.version != MANIFEST_VERSION {
            return Err(format!("version {} isn't one this build reads ({})", manifest.version, MANIFEST_VERSION));
        }
        if manifest.entries.is_empty() {
            return Err("it lists no entries".to_string());
        }
        for (i, name) in manifest.entries.iter().enumerate() {
            if !valid_name(name) {
                return Err(format!("entry '{}' isn't a file name: letters, digits and _ only", name));
            }
            if manifest.entries[..i].contains(name) {
                return Err(format!("entry '{}' is listed twice", name));
            }
        }
        for (i, link) in manifest.linked_content.iter().enumerate() {
            if link.trim().is_empty() {
                return Err("linked_content has an empty path".to_string());
            }
            if manifest.linked_content[..i].contains(link) {
                return Err(format!("linked_content lists '{}' twice", link));
            }
        }
        Ok(manifest)
    }

    /// The manifest as its file holds it.
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("a manifest serializes");
        text.push('\n');
        text
    }
}

/// Whether `name` can name an entry or a resource: letters, digits and _.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A journal's directory, opened.
#[derive(Clone, Debug)]
pub struct Journal {
    pub dir: PathBuf,
    pub manifest: Manifest,
    /// Its entry scripts.
    pub entries: Sources,
}

impl Journal {
    /// The journal in `dir`; each entry its manifest lists must be there.
    pub fn open(dir: impl AsRef<Path>) -> Result<Journal, String> {
        let dir = dir.as_ref().to_path_buf();
        let path = dir.join(MANIFEST);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("couldn't read {}: {}", path.display(), e))?;
        let manifest = Manifest::parse(&text).map_err(|e| format!("{}: {}", path.display(), e))?;
        let entries = Sources::dir(dir.join("entries"));
        for name in &manifest.entries {
            if !entries.has(name) {
                return Err(format!("{}: entry '{}' has no file {}", path.display(), name, entries.path(name).display()));
            }
        }
        Ok(Journal { dir, manifest, entries })
    }

    /// The journal's name: its directory's.
    pub fn name(&self) -> String {
        self.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// Its resources, in front of `library`'s, with its linked content in
    /// front of its own.
    pub fn resources(&self, library: Resources) -> Resources {
        self.own_resources(library).with_linked_content(&self.links())
    }

    /// Its own resources, in front of `library`'s, without its links.
    pub fn own_resources(&self, library: Resources) -> Resources {
        Resources::dir(&self.dir).over(library)
    }

    /// Where its linked content is: each path from its directory, or
    /// absolute, `~/` the home directory.
    pub fn links(&self) -> Vec<PathBuf> {
        self.manifest.linked_content.iter().map(|link| {
            match (link.strip_prefix("~/"), std::env::var_os("HOME")) {
                (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
                _ => self.dir.join(link),
            }
        }).collect()
    }

    /// Its links that aren't there.
    pub fn missing_links(&self) -> Vec<PathBuf> {
        self.links().into_iter().filter(|link| !link.exists()).collect()
    }

    /// Entry files in `entries/` the manifest doesn't list.
    pub fn unlisted_entries(&self) -> Vec<String> {
        self.entries.own_names().unwrap_or_default().into_iter()
            .filter(|name| !self.manifest.entries.contains(name))
            .collect()
    }

    /// Entry `name`'s script.
    pub fn entry_source(&self, name: &str) -> Result<String, String> {
        self.entries.read(name).map_err(|e| format!("couldn't read {}: {}", self.entries.path(name).display(), e))
    }

    /// Entry `name`'s file.
    pub fn entry_path(&self, name: &str) -> PathBuf {
        self.entries.path(name)
    }

    /// An engine to run an entry in, with the journal's resources in front
    /// of `library`'s, and its glyphsets. Each entry gets one of its own:
    /// entries share resources, not code.
    pub fn engine(&self, library: Resources) -> Result<JournalEngine, String> {
        let resources = self.resources(library);
        let glyphsets = resources.glyphset_book()?;
        Ok(JournalEngine::with_resources(Rc::new(glyphsets), resources))
    }

    /// Entry `name`'s edits file, `edits/NAME.json`.
    pub fn edits_path(&self, name: &str) -> PathBuf {
        self.dir.join("edits").join(format!("{}.json", name))
    }

    /// The visual editor's edits to entry `name`'s canvases; none without
    /// an edits file.
    pub fn edits(&self, name: &str) -> Result<Option<Edits>, String> {
        let path = self.edits_path(name);
        match std::fs::read_to_string(&path) {
            Ok(text) => Edits::parse(&text).map(Some).map_err(|e| format!("{}: {}", path.display(), e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("couldn't read {}: {}", path.display(), e)),
        }
    }

    /// Run entry `name` in `engine`, with its edits; errors name its file.
    /// Edits for canvases it doesn't commit are warned of.
    pub fn build_entry(&self, engine: &mut JournalEngine, name: &str) -> Result<Entry, String> {
        let source = self.entry_source(name)?;
        if let Some(edits) = self.edits(name)? {
            engine.set_edits(edits);
        }
        let entry = engine.build_entry_str(&source).map_err(|err| format!("{}: {}", self.entry_path(name).display(), err))?;
        for unused in engine.unused_edits() {
            engine.warn(format!("{} has an edit for '{}', which the entry doesn't commit to the editor", self.edits_path(name).display(), unused));
        }
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_are_checked() {
        let good = r#"{ "version": 1, "title": "T", "entries": ["a", "b_2"] }"#;
        let manifest = Manifest::parse(good).unwrap();
        assert_eq!(manifest.entries, ["a", "b_2"]);
        assert_eq!(Manifest::parse(&manifest.to_json()).unwrap(), manifest);
        for (text, want) in [
            (r#"{ "version": 2, "title": "T", "entries": ["a"] }"#, "version 2 isn't one this build reads"),
            (r#"{ "version": 1, "title": "T", "entries": [] }"#, "lists no entries"),
            (r#"{ "version": 1, "title": "T", "entries": ["a/b"] }"#, "entry 'a/b' isn't a file name"),
            (r#"{ "version": 1, "title": "T", "entries": ["a", "a"] }"#, "entry 'a' is listed twice"),
            (r#"{ "version": 1, "title": "T", "entries": ["a"], "colour": 1 }"#, "unknown field `colour`"),
        ] {
            let err = Manifest::parse(text).unwrap_err();
            assert!(err.contains(want), "{}\n  wanted: {}\n  got: {}", text, want, err);
        }
    }

    #[test]
    fn every_repository_journal_opens() {
        let mut opened = 0;
        let dirs = super::super::DEFAULT_JOURNALS_DIRS.iter().flat_map(|dir| std::fs::read_dir(dir).unwrap().flatten());
        for dir in dirs.filter(|dir| dir.path().join(MANIFEST).is_file()) {
            let journal = Journal::open(dir.path()).unwrap_or_else(|e| panic!("{}", e));
            assert!(journal.unlisted_entries().is_empty(), "{}: entries not in its manifest: {:?}", journal.name(), journal.unlisted_entries());
            opened += 1;
        }
        assert!(opened > 0);
    }

    #[test]
    fn entries_import_from_lib_and_use_their_journal() {
        let dir = std::env::temp_dir().join(format!("scribe-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["entries", "lib", "content"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        std::fs::write(dir.join(MANIFEST), r#"{ "version": 1, "title": "T", "entries": ["one", "two"] }"#).unwrap();
        std::fs::write(dir.join("lib/helpers.rhai"), "fn page_named(name) { new_page(#{ name: name }) }").unwrap();
        std::fs::write(dir.join("content/greeting.txt"), "hello").unwrap();
        std::fs::write(dir.join("entries/one.rhai"), r#"import "lib/helpers" as h; journal_entry.add(h::page_named(content("greeting")));"#).unwrap();
        std::fs::write(dir.join("entries/two.rhai"), r#"journal_entry.add(new_page());"#).unwrap();
        std::fs::write(dir.join("entries/stray.rhai"), "").unwrap();
        let journal = Journal::open(&dir).unwrap();
        assert_eq!(journal.unlisted_entries(), ["stray"]);
        let mut engine = journal.engine(Resources::library()).unwrap();
        let entry = journal.build_entry(&mut engine, "one").unwrap();
        assert_eq!(entry.page_index("hello"), Ok(0));
        std::fs::remove_file(dir.join("entries/two.rhai")).unwrap();
        assert!(Journal::open(&dir).unwrap_err().contains("entry 'two' has no file"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn linked_content_is_read_as_it_is_asked_for() {
        let root = std::env::temp_dir().join(format!("scribe-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("journal");
        for sub in ["journal/entries", "journal/content", "notes", "handouts"] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        std::fs::write(dir.join(MANIFEST), r#"{ "version": 1, "title": "T", "entries": ["one"],
            "linked_content": ["../notes", "../handouts/letter.md", "../gone"] }"#).unwrap();
        std::fs::write(dir.join("content/note.md"), "the journal's").unwrap();
        std::fs::write(dir.join("content/fallback.txt"), "kept").unwrap();
        std::fs::write(root.join("notes/note.md"), "linked").unwrap();
        std::fs::write(root.join("notes/other.txt"), "other").unwrap();
        std::fs::write(root.join("handouts/letter.md"), "*dear*").unwrap();
        std::fs::write(root.join("handouts/unlinked.md"), "not linked").unwrap();
        std::fs::write(dir.join("entries/one.rhai"), "journal_entry.add(new_page());").unwrap();
        let journal = Journal::open(&dir).unwrap();
        assert_eq!(journal.missing_links(), [dir.join("../gone")]);
        let read = |journal: &Journal, want: &str| {
            let mut engine = journal.engine(Resources::library()).unwrap();
            let script = format!(r#"let got = content("note") + "|" + content("other") + "|" + content("letter") + "|" + content("fallback");
                if got != "{}" {{ throw got; }}"#, want);
            engine.build_entry_str(&script).map(|_| ()).map_err(|e| e.to_string())
        };
        assert_eq!(read(&journal, "linked|other|*dear*|kept"), Ok(()));
        std::fs::write(root.join("notes/note.md"), "changed").unwrap();
        assert_eq!(read(&journal, "changed|other|*dear*|kept"), Ok(()), "a linked file is read anew");
        let mut engine = journal.engine(Resources::library()).unwrap();
        let err = engine.build_entry_str(r#"content("unlinked")"#).unwrap_err();
        assert!(err.contains("no content named 'unlinked'"), "{}", err);
        assert!(journal.own_resources(Resources::library()).overrides().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn linked_content_is_checked() {
        for (text, want) in [
            (r#"{ "version": 1, "title": "T", "entries": ["a"], "linked_content": [""] }"#, "linked_content has an empty path"),
            (r#"{ "version": 1, "title": "T", "entries": ["a"], "linked_content": ["x", "x"] }"#, "linked_content lists 'x' twice"),
        ] {
            let err = Manifest::parse(text).unwrap_err();
            assert!(err.contains(want), "{}\n  wanted: {}\n  got: {}", text, want, err);
        }
        let manifest = Manifest::parse(r#"{ "version": 1, "title": "T", "entries": ["a"], "linked_content": ["../notes"] }"#).unwrap();
        assert_eq!(Manifest::parse(&manifest.to_json()).unwrap(), manifest);
    }
}
