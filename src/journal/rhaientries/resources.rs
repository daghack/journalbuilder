//! A journal's resources: its lock, layout, illustration, chain, style and
//! helper scripts, glyphsets and content, each kind in a directory of its
//! own, in front of the built-in library's (`webapp/library/`), which has
//! the same directories. A journal's file stands in for a library file of
//! the same name.
//!
//! ```text
//! locks/NAME.rhai          lock types
//! layouts/NAME.rhai        page layouts, for layout(name)
//! illustrations/NAME.rhai  what chains draw their values with
//! chains/NAME.rhai         locks wired together, for chain(name)
//! styles/NAME.rhai         text styles, for text_style(name)
//! lib/NAME.rhai            helpers any script imports: import "lib/NAME"
//! glyphsets/NAME.json      glyphsets, for glyphset(name)
//! content/NAME.txt         plain text, for content(name)
//! content/NAME.md          marked-up text, for content(name)
//! content/NAME.json        symbols, #{ base, symbols }, for content(name)
//! ```

use super::sources::Sources;
use crate::journal::fonts::FontBook;
use std::path::Path;

/// The built-in library's directory. Resolved at build time, so it's found
/// whatever directory journalbuild (or a test) runs from.
pub const DEFAULT_LIBRARY_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/webapp/library");

/// The repository's journals.
pub const DEFAULT_JOURNALS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/webapp/journals");

/// Every kind of resource: its directory, and its files' extension.
pub const KINDS: [(&str, &str); 10] = [
    ("locks", "rhai"),
    ("layouts", "rhai"),
    ("illustrations", "rhai"),
    ("chains", "rhai"),
    ("styles", "rhai"),
    ("lib", "rhai"),
    ("glyphsets", "json"),
    ("content", "txt"),
    ("content", "md"),
    ("content", "json"),
];

/// A journal's resources, each kind in front of the library's.
#[derive(Clone, Debug)]
pub struct Resources {
    pub locks: Sources,
    pub layouts: Sources,
    pub illustrations: Sources,
    pub chains: Sources,
    pub styles: Sources,
    pub lib: Sources,
    pub glyphsets: Sources,
    /// Content: plain text, marked-up text and symbols.
    pub text: Sources,
    pub markdown: Sources,
    pub symbols: Sources,
}

impl Resources {
    /// The resources in the directories under `root`; a directory that
    /// isn't there has none.
    pub fn dir(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self::from_kinds(|kind, ext| Sources::dir(root.join(kind)).with_ext(ext))
    }

    /// The built-in library, from `webapp/library/`.
    pub fn library() -> Self {
        Self::dir(DEFAULT_LIBRARY_DIR)
    }

    /// The built-in library, compiled in, for builds with no files to read.
    #[cfg(feature = "embedded-assets")]
    pub fn embedded() -> Self {
        Self::from_kinds(|kind, ext| embedded::sources(kind, ext))
    }

    /// No resources: a journal with none of its own over nothing.
    pub fn none() -> Self {
        Self::from_kinds(|kind, ext| Sources::none(kind).with_ext(ext))
    }

    fn from_kinds(sources: impl Fn(&str, &'static str) -> Sources) -> Self {
        Resources {
            locks: sources("locks", "rhai"),
            layouts: sources("layouts", "rhai"),
            illustrations: sources("illustrations", "rhai"),
            chains: sources("chains", "rhai"),
            styles: sources("styles", "rhai"),
            lib: sources("lib", "rhai"),
            glyphsets: sources("glyphsets", "json"),
            text: sources("content", "txt"),
            markdown: sources("content", "md"),
            symbols: sources("content", "json"),
        }
    }

    /// These resources in front of `under`'s, kind by kind.
    pub fn over(self, under: Resources) -> Self {
        Resources {
            locks: self.locks.over(under.locks),
            layouts: self.layouts.over(under.layouts),
            illustrations: self.illustrations.over(under.illustrations),
            chains: self.chains.over(under.chains),
            styles: self.styles.over(under.styles),
            lib: self.lib.over(under.lib),
            glyphsets: self.glyphsets.over(under.glyphsets),
            text: self.text.over(under.text),
            markdown: self.markdown.over(under.markdown),
            symbols: self.symbols.over(under.symbols),
        }
    }

    /// Every kind's files, in `KINDS`' order, with their directory's name.
    pub fn kinds(&self) -> [(&'static str, &Sources); 10] {
        [
            ("locks", &self.locks),
            ("layouts", &self.layouts),
            ("illustrations", &self.illustrations),
            ("chains", &self.chains),
            ("styles", &self.styles),
            ("lib", &self.lib),
            ("glyphsets", &self.glyphsets),
            ("content", &self.text),
            ("content", &self.markdown),
            ("content", &self.symbols),
        ]
    }

    /// The files of these' own that stand in for library files of the
    /// same name, as paths from the journal: `illustrations/eye.rhai`.
    pub fn overrides(&self) -> Vec<String> {
        let mut found = Vec::new();
        for (kind, sources) in self.kinds() {
            for name in sources.own_names().unwrap_or_default() {
                if sources.overrides(&name) {
                    found.push(format!("{}/{}.{}", kind, name, sources.ext()));
                }
            }
        }
        found
    }

    /// The content called `name`'s files: where there's more than one
    /// (`lorem.txt` and `lorem.md`), content(name) is ambiguous.
    pub fn content_kinds(&self, name: &str) -> Vec<&'static str> {
        [(&self.text, "txt"), (&self.markdown, "md"), (&self.symbols, "json")].into_iter()
            .filter(|(sources, _)| sources.has(name))
            .map(|(_, ext)| ext)
            .collect()
    }

    /// Every content name, sorted.
    pub fn content_names(&self) -> Vec<String> {
        let mut names: Vec<String> = [&self.text, &self.markdown, &self.symbols].into_iter()
            .flat_map(|sources| sources.names().unwrap_or_default())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Every glyphset, the library's and these' own, as one glyphset file:
    /// `{ "NAME": <that file's glyphset>, ... }`.
    pub fn glyphsets_json(&self) -> Result<String, String> {
        let names = self.glyphsets.names()
            .map_err(|e| format!("couldn't read the glyphsets directory {}: {}", self.glyphsets.root().display(), e))?;
        let mut merged = String::from("{");
        for (i, name) in names.iter().enumerate() {
            let text = self.glyphsets.read(name)
                .map_err(|e| format!("{}: {}", self.glyphsets.path(name).display(), e))?;
            merged.push_str(if i == 0 { "\n" } else { ",\n" });
            merged.push_str(&format!("{:?}: {}", name, text.trim()));
        }
        merged.push_str("\n}\n");
        Ok(merged)
    }

    /// Every glyphset, loaded.
    pub fn glyphset_book(&self) -> Result<FontBook, String> {
        let names = self.glyphsets.names()
            .map_err(|e| format!("couldn't read the glyphsets directory {}: {}", self.glyphsets.root().display(), e))?;
        let files = names.into_iter().map(|name| {
            let path = self.glyphsets.path(&name);
            let text = self.glyphsets.read(&name).map_err(|e| format!("{}: {}", path.display(), e))?;
            Ok((name, path.display().to_string(), text))
        }).collect::<Result<Vec<_>, String>>()?;
        FontBook::from_files(files)
    }
}

/// The library's files, compiled in.
#[cfg(feature = "embedded-assets")]
pub mod embedded {
    use super::Sources;

    include!(concat!(env!("OUT_DIR"), "/embedded_library.rs"));

    /// The library's files of kind `kind` with extension `ext`, as if in
    /// `kind/`.
    pub fn sources(kind: &str, ext: &'static str) -> Sources {
        let files = LIBRARY.iter().find(|(k, e, _)| *k == kind && *e == ext).map_or(&[][..], |(_, _, files)| *files);
        Sources::embedded(kind.to_string(), files).with_ext(ext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_journal_stands_in_front_of_the_library() {
        let dir = std::env::temp_dir().join(format!("scribe-resources-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("illustrations")).unwrap();
        std::fs::create_dir_all(dir.join("content")).unwrap();
        std::fs::write(dir.join("illustrations/eye.rhai"), "// mine").unwrap();
        std::fs::write(dir.join("content/note.txt"), "a note").unwrap();
        std::fs::write(dir.join("content/note.md"), "a *note*").unwrap();
        let resources = Resources::dir(&dir).over(Resources::library());
        assert_eq!(resources.overrides(), ["illustrations/eye.rhai"]);
        assert_eq!(resources.illustrations.read("eye").unwrap(), "// mine");
        assert!(resources.locks.has("shift_cipher"));
        assert!(resources.lib.has("permutations"));
        assert_eq!(resources.content_kinds("note"), ["txt", "md"]);
        assert!(resources.content_names().contains(&"note".to_string()));
        let book = resources.glyphset_book().unwrap();
        assert!(book.names().contains(&"plain".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The compiled-in library is the library on disk.
    #[cfg(feature = "embedded-assets")]
    #[test]
    fn the_embedded_library_is_the_library() {
        let (disk, compiled) = (Resources::library(), Resources::embedded());
        for ((kind, disk), (_, compiled)) in disk.kinds().into_iter().zip(compiled.kinds()) {
            let names = disk.names().unwrap();
            assert_eq!(names, compiled.names().unwrap(), "{}", kind);
            for name in names {
                assert_eq!(disk.read(&name).unwrap(), compiled.read(&name).unwrap(), "{}/{}", kind, name);
            }
        }
        assert!(compiled.lib.has("permutations"));
    }
}
