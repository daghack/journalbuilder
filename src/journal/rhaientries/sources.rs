//! Where layout and lock files come from: a directory, or copies compiled
//! into the binary (`embedded`, for builds with no files to read, like the
//! web editor's). Either way a script is known by its name in the
//! directory, `<dir>/<name>.rhai`, and its errors show that path. An
//! overlay of scripts held in memory (the web editor's unsaved lock files)
//! can add to the directory or stand in for scripts in it.

use rhai::{Engine, EvalAltResult, Module, ModuleResolver, Position, Scope, Shared, AST};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

/// A directory of rhai scripts, read from disk or compiled in.
#[derive(Clone, Debug)]
pub struct Sources {
    dir: PathBuf,
    /// The scripts by name (`lib/permutations` for one in a subdirectory),
    /// when they're compiled in; `dir` then only names them.
    embedded: Option<Rc<HashMap<&'static str, &'static str>>>,
    /// Scripts by name that come before the directory's.
    overlay: Option<Rc<BTreeMap<String, String>>>,
}

impl Sources {
    /// The scripts in `dir`.
    pub fn dir(dir: impl Into<PathBuf>) -> Self {
        Sources { dir: dir.into(), embedded: None, overlay: None }
    }

    /// Scripts compiled in, by name, shown as if they were in `dir`.
    pub fn embedded(dir: impl Into<PathBuf>, scripts: &[(&'static str, &'static str)]) -> Self {
        Sources { dir: dir.into(), embedded: Some(Rc::new(scripts.iter().copied().collect())), overlay: None }
    }

    /// These scripts, with `scripts` (name, source) added, each in place of
    /// any script of the same name.
    pub fn with_overlay(mut self, scripts: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut overlay = self.overlay.as_deref().cloned().unwrap_or_default();
        overlay.extend(scripts);
        self.overlay = Some(Rc::new(overlay));
        self
    }

    fn overlaid(&self, name: &str) -> Option<&str> {
        self.overlay.as_ref().and_then(|scripts| scripts.get(name)).map(String::as_str)
    }

    /// The directory, as errors show it.
    pub fn root(&self) -> &Path {
        &self.dir
    }

    /// The file of the script called `name`.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{}.rhai", name))
    }

    /// Whether there's a script called `name`.
    pub fn has(&self, name: &str) -> bool {
        if self.overlaid(name).is_some() {
            return true;
        }
        match &self.embedded {
            Some(scripts) => scripts.contains_key(name),
            None => self.path(name).is_file(),
        }
    }

    /// The names of the scripts directly in the directory (not its
    /// subdirectories), sorted.
    pub fn names(&self) -> std::io::Result<Vec<String>> {
        let mut names: Vec<String> = match &self.embedded {
            Some(scripts) => scripts.keys().filter(|name| !name.contains('/')).map(|name| name.to_string()).collect(),
            None => std::fs::read_dir(&self.dir)?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "rhai"))
                .filter_map(|path| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
                .collect(),
        };
        if let Some(overlay) = &self.overlay {
            names.extend(overlay.keys().filter(|name| !name.contains('/')).cloned());
        }
        names.sort();
        names.dedup();
        Ok(names)
    }

    /// The source of the script called `name`.
    pub fn read(&self, name: &str) -> std::io::Result<String> {
        if let Some(source) = self.overlaid(name) {
            return Ok(source.to_string());
        }
        match &self.embedded {
            Some(scripts) => scripts.get(name).map(|source| source.to_string()).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, format!("no script {}", self.path(name).display()))
            }),
            None => std::fs::read_to_string(self.path(name)),
        }
    }

    /// The script called `name`, compiled, with its path as its source,
    /// failing as `Engine::compile_file` does.
    pub fn compile(&self, engine: &Engine, name: &str) -> RhaiResult<AST> {
        let path = self.path(name);
        let source = self.read(name).map_err(|err| {
            EvalAltResult::ErrorSystem(format!("Cannot open script file '{}'", path.to_string_lossy()), err.into())
        })?;
        let mut ast = engine.compile(source)?;
        ast.set_source(path.to_string_lossy().as_ref());
        Ok(ast)
    }

    /// Resolves `import "lib/name"` to the script `lib/name`, as rhai's
    /// `FileModuleResolver` would to `<dir>/lib/name.rhai`.
    pub fn module_resolver(&self) -> SourcesResolver {
        SourcesResolver { sources: self.clone(), cache: RefCell::default() }
    }
}

/// See `Sources::module_resolver`. Each module is run once and cached.
pub struct SourcesResolver {
    sources: Sources,
    cache: RefCell<HashMap<String, Shared<Module>>>,
}

impl ModuleResolver for SourcesResolver {
    fn resolve(&self, engine: &Engine, _source: Option<&str>, path: &str, pos: Position) -> RhaiResult<Shared<Module>> {
        if let Some(module) = self.cache.borrow().get(path) {
            return Ok(module.clone());
        }
        if !self.sources.has(path) {
            return Err(EvalAltResult::ErrorModuleNotFound(path.to_string(), pos).into());
        }
        let in_module = |err| Box::new(EvalAltResult::ErrorInModule(path.to_string(), err, pos));
        let mut ast = self.sources.compile(engine, path).map_err(in_module)?;
        ast.set_source(path);
        let module: Shared<Module> = Module::eval_ast_as_new(Scope::new(), &ast, engine).map_err(in_module)?.into();
        self.cache.borrow_mut().insert(path.to_string(), module.clone());
        Ok(module)
    }
}

/// The repository's layout, lock and renderer files, compiled in.
#[cfg(feature = "embedded-assets")]
pub mod embedded {
    use super::Sources;

    include!(concat!(env!("OUT_DIR"), "/embedded_sources.rs"));

    /// The font files in `webapp/fonts/`, as one font file.
    pub const FONTS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts.json"));

    /// The layout files, as if in `webapp/layouts/`.
    pub fn layouts() -> Sources {
        Sources::embedded("layouts", LAYOUTS)
    }

    /// The lock files and their helpers, as if in `webapp/locks/`.
    pub fn locks() -> Sources {
        Sources::embedded("locks", LOCKS)
    }

    /// The renderer files, as if in `webapp/renderers/`.
    pub fn renderers() -> Sources {
        Sources::embedded("renderers", RENDERERS)
    }

    /// The chain files, as if in `webapp/chains/`.
    pub fn chains() -> Sources {
        Sources::embedded("chains", CHAINS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPTS: &[(&str, &str)] = &[
        ("double", r#"import "lib/twice" as twice; fn double(x) { twice::twice(x) }"#),
        ("broken", "fn oops( {"),
        ("lib/twice", "fn twice(x) { x * 2 }"),
    ];

    #[test]
    fn embedded_scripts_are_listed_read_and_imported() {
        let sources = Sources::embedded("scripts", SCRIPTS);
        assert_eq!(sources.names().unwrap(), ["broken", "double"]);
        assert!(sources.has("double") && sources.has("lib/twice") && !sources.has("missing"));
        assert_eq!(sources.path("double"), Path::new("scripts/double.rhai"));

        let mut engine = Engine::new();
        engine.set_module_resolver(sources.module_resolver());
        let ast = sources.compile(&engine, "double").unwrap();
        assert_eq!(ast.source(), Some("scripts/double.rhai"));
        let doubled: i64 = engine.call_fn(&mut Scope::new(), &ast, "double", (21_i64,)).unwrap();
        assert_eq!(doubled, 42);
    }

    #[test]
    fn embedded_scripts_fail_as_files_do() {
        let sources = Sources::embedded("scripts", SCRIPTS);
        let engine = Engine::new();
        let missing = sources.compile(&engine, "missing").unwrap_err().to_string();
        assert!(missing.contains("Cannot open script file 'scripts/missing.rhai'"), "{}", missing);
        assert!(sources.compile(&engine, "broken").is_err());
    }

    #[test]
    fn overlays_add_and_replace_scripts() {
        let sources = Sources::embedded("scripts", SCRIPTS).with_overlay([
            ("broken".to_string(), "fn oops() { 1 }".to_string()),
            ("extra".to_string(), r#"import "lib/twice" as twice; fn four() { twice::twice(2) }"#.to_string()),
            ("lib/more".to_string(), "fn more() { 3 }".to_string()),
        ]);
        assert_eq!(sources.names().unwrap(), ["broken", "double", "extra"]);
        assert!(sources.has("extra") && sources.has("lib/more"));
        let mut engine = Engine::new();
        engine.set_module_resolver(sources.module_resolver());
        assert!(sources.compile(&engine, "broken").is_ok());
        let ast = sources.compile(&engine, "extra").unwrap();
        assert_eq!(ast.source(), Some("scripts/extra.rhai"));
        let four: i64 = engine.call_fn(&mut Scope::new(), &ast, "four", ()).unwrap();
        assert_eq!(four, 4);

        let dir = std::env::temp_dir().join(format!("scribe-sources-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("on_disk.rhai"), "1").unwrap();
        let sources = Sources::dir(&dir).with_overlay([("on_disk".to_string(), "2".to_string()), ("new".to_string(), "3".to_string())]);
        assert_eq!(sources.names().unwrap(), ["new", "on_disk"]);
        assert_eq!(sources.read("on_disk").unwrap(), "2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every journal runs the same with the repository's layout and lock
    /// files compiled in as read from disk.
    #[cfg(feature = "embedded-assets")]
    #[test]
    fn journals_run_the_same_with_embedded_files() {
        use super::super::JournalEngine;
        use crate::journal::fonts::FontBook;
        let journals = Path::new(env!("CARGO_MANIFEST_DIR")).join("webapp/journals");
        let mut checked = 0;
        for journal in std::fs::read_dir(&journals).unwrap().flatten() {
            let script = journal.path().join("journal_entry.rhai");
            let Ok(source) = std::fs::read_to_string(&script) else { continue };
            let pages = |mut engine: JournalEngine| engine.build_entry_str(&source)
                .map(|entry| entry.selected_pages(None).map(|(i, page)| page.label(i)).collect::<Vec<_>>());
            let fonts = || Rc::new(FontBook::from_bytes(embedded::FONTS).unwrap());
            let from_disk = pages(JournalEngine::new(fonts()));
            let compiled_in = pages(JournalEngine::embedded(fonts()));
            assert!(from_disk.is_ok(), "{}: {:?}", script.display(), from_disk);
            assert_eq!(from_disk, compiled_in, "{}", script.display());
            checked += 1;
        }
        assert!(checked > 0);
    }
}
