//! Where a journal's resource files come from: a directory, or copies
//! compiled into the binary (`embedded`, for builds with no files to read,
//! like the web editor's). Either way a file is known by its name in the
//! directory, `<dir>/<name>.rhai` (or another extension, for glyphsets and
//! content), and its errors show that path. An overlay of files held in
//! memory (the web editor's unsaved files) can add to the directory or
//! stand in for files in it, and the directory can stand in front of
//! another (`over`): a journal's own files over the built-in library's.
//!
//! Pictures (`png` and `jpg` files) are read as text too, as data URLs
//! (`data:image/png;base64,...`): that's how the web editor and the
//! compiled-in library hold them. `read_bytes` gives their bytes.

use rhai::{Engine, EvalAltResult, Module, ModuleResolver, Position, Scope, Shared, AST};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

/// A directory of files of one extension, read from disk or compiled in,
/// perhaps in front of another.
#[derive(Clone, Debug)]
pub struct Sources {
    dir: PathBuf,
    /// The files' extension, `rhai` for scripts.
    ext: &'static str,
    /// The files by name (`lib/permutations` for one in a subdirectory),
    /// when they're compiled in; `dir` then only names them.
    embedded: Option<Rc<HashMap<&'static str, &'static str>>>,
    /// Files by name that come before the directory's.
    overlay: Option<Rc<BTreeMap<String, String>>>,
    /// Files these stand in front of: a file there is one here too, unless
    /// one of the same name is here.
    under: Option<Rc<Sources>>,
    /// Where the names of these' own files are noted as they're read.
    used: Option<Rc<RefCell<BTreeSet<String>>>>,
}

impl Sources {
    /// The scripts in `dir`.
    pub fn dir(dir: impl Into<PathBuf>) -> Self {
        Sources { dir: dir.into(), ext: "rhai", embedded: None, overlay: None, under: None, used: None }
    }

    /// Scripts compiled in, by name, shown as if they were in `dir`.
    pub fn embedded(dir: impl Into<PathBuf>, scripts: &[(&'static str, &'static str)]) -> Self {
        Sources { embedded: Some(Rc::new(scripts.iter().copied().collect())), ..Sources::dir(dir) }
    }

    /// Nothing, shown as if in `dir`.
    pub fn none(dir: impl Into<PathBuf>) -> Self {
        Sources::embedded(dir, &[])
    }

    /// The same files, but `<name>.<ext>`: `json` for glyphsets.
    pub fn with_ext(mut self, ext: &'static str) -> Self {
        self.ext = ext;
        self
    }

    /// The files' extension.
    pub fn ext(&self) -> &'static str {
        self.ext
    }

    /// These files, with `scripts` (name, source) added, each in place of
    /// any file of the same name.
    pub fn with_overlay(mut self, scripts: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut overlay = self.overlay.as_deref().cloned().unwrap_or_default();
        overlay.extend(scripts);
        self.overlay = Some(Rc::new(overlay));
        self
    }

    /// These files in front of `under`'s: `under`'s files are these' too,
    /// unless these have one of the same name. Whatever these were in front
    /// of before, `under` now is.
    pub fn over(mut self, under: Sources) -> Self {
        self.under = Some(Rc::new(under));
        self
    }

    /// What these stand in front of, if anything.
    pub fn under(&self) -> Option<&Sources> {
        self.under.as_deref()
    }

    /// These files, noting into `used` the name of each of their own read
    /// (not those of what they're in front of): which a journal uses.
    pub fn recording(mut self, used: Rc<RefCell<BTreeSet<String>>>) -> Self {
        self.used = Some(used);
        self
    }

    /// These files, noting nothing as they're read, nor what they're in
    /// front of.
    pub fn unrecorded(&self) -> Sources {
        let mut sources = self.clone();
        sources.used = None;
        sources.under = self.under.as_ref().map(|under| Rc::new(under.unrecorded()));
        sources
    }

    fn note(&self, name: &str) {
        if let Some(used) = &self.used {
            used.borrow_mut().insert(name.to_string());
        }
    }

    fn overlaid(&self, name: &str) -> Option<&str> {
        self.overlay.as_ref().and_then(|scripts| scripts.get(name)).map(String::as_str)
    }

    /// Whether the file called `name` is one of these' own, not one from
    /// what they're in front of.
    pub fn has_own(&self, name: &str) -> bool {
        if self.overlaid(name).is_some() {
            return true;
        }
        match &self.embedded {
            Some(scripts) => scripts.contains_key(name),
            None => self.own_path(name).is_file(),
        }
    }

    /// Whether `name` is one of these' own files standing in for one of
    /// the same name in what they're in front of: a journal's own copy of
    /// a built-in.
    pub fn overrides(&self, name: &str) -> bool {
        self.has_own(name) && self.under.as_ref().is_some_and(|under| under.has(name))
    }

    /// The directory, as errors show it.
    pub fn root(&self) -> &Path {
        &self.dir
    }

    fn own_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{}.{}", name, self.ext))
    }

    /// The file called `name`: its own, or else the one in what these are
    /// in front of.
    pub fn path(&self, name: &str) -> PathBuf {
        match &self.under {
            Some(under) if !self.has_own(name) && under.has(name) => under.path(name),
            _ => self.own_path(name),
        }
    }

    /// `path` from the directory it's in (these', or what these are in
    /// front of), or `path` itself if it's in neither.
    pub fn relative<'a>(&self, path: &'a Path) -> &'a Path {
        if let Ok(relative) = path.strip_prefix(&self.dir) {
            return relative;
        }
        match &self.under {
            Some(under) => under.relative(path),
            None => path,
        }
    }

    /// Whether there's a file called `name`.
    pub fn has(&self, name: &str) -> bool {
        self.has_own(name) || self.under.as_ref().is_some_and(|under| under.has(name))
    }

    /// The names of the files directly in the directory (not its
    /// subdirectories), and what it's in front of, sorted.
    pub fn names(&self) -> std::io::Result<Vec<String>> {
        let mut names = self.own_names()?;
        if let Some(under) = &self.under {
            names.extend(under.names()?);
        }
        names.sort();
        names.dedup();
        Ok(names)
    }

    /// The names of these' own files directly in the directory, sorted: not
    /// those of what they're in front of. A directory that isn't there has
    /// none.
    pub fn own_names(&self) -> std::io::Result<Vec<String>> {
        let mut names: Vec<String> = match &self.embedded {
            Some(scripts) => scripts.keys().filter(|name| !name.contains('/')).map(|name| name.to_string()).collect(),
            None if !self.dir.exists() => Vec::new(),
            None => std::fs::read_dir(&self.dir)?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == self.ext))
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

    /// The text of the file called `name`.
    pub fn read(&self, name: &str) -> std::io::Result<String> {
        if let Some(source) = self.overlaid(name) {
            self.note(name);
            return Ok(source.to_string());
        }
        if let Some(under) = &self.under
            && !self.has_own(name) && under.has(name) {
            return under.read(name);
        }
        if self.has_own(name) {
            self.note(name);
        }
        match &self.embedded {
            Some(scripts) => scripts.get(name).map(|source| source.to_string()).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, format!("no file {}", self.path(name).display()))
            }),
            None if image_type(self.ext).is_some() => {
                let bytes = std::fs::read(self.own_path(name))?;
                Ok(format!("data:{};base64,{}", image_type(self.ext).expect("a picture"), base64(&bytes)))
            }
            None => std::fs::read_to_string(self.own_path(name)),
        }
    }

    /// The bytes of the picture called `name`: its own, or else the one in
    /// what these are in front of. Read from disk as they are, or decoded
    /// from the data URL (or plain base64) held in memory or compiled in.
    pub fn read_bytes(&self, name: &str) -> std::io::Result<Vec<u8>> {
        if let Some(under) = &self.under
            && !self.has_own(name) && under.has(name) {
            return under.read_bytes(name);
        }
        if self.overlaid(name).is_none() && self.embedded.is_none() {
            if self.has_own(name) {
                self.note(name);
            }
            return std::fs::read(self.own_path(name));
        }
        let text = self.read(name)?;
        decode_base64(text.split_once(";base64,").map_or(text.as_str(), |(_, data)| data)).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{} isn't a data URL or base64", self.path(name).display()))
        })
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

    /// Resolves `import "lib/name"` to the script `name` here, as rhai's
    /// `FileModuleResolver` would to `<dir>/name.rhai` were these the
    /// directory `lib`. Every kind of script imports its helpers this way.
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
        let name = match path.strip_prefix("lib/") {
            Some(name) if self.sources.has(name) => name,
            _ => return Err(EvalAltResult::ErrorModuleNotFound(path.to_string(), pos).into()),
        };
        let in_module = |err| Box::new(EvalAltResult::ErrorInModule(path.to_string(), err, pos));
        let mut ast = self.sources.compile(engine, name).map_err(in_module)?;
        ast.set_source(path);
        let module: Shared<Module> = Module::eval_ast_as_new(Scope::new(), &ast, engine).map_err(in_module)?.into();
        self.cache.borrow_mut().insert(path.to_string(), module.clone());
        Ok(module)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPTS: &[(&str, &str)] = &[
        ("double", r#"import "lib/twice" as twice; fn double(x) { twice::twice(x) }"#),
        ("broken", "fn oops( {"),
    ];

    const LIB: &[(&str, &str)] = &[
        ("twice", "fn twice(x) { x * 2 }"),
    ];

    #[test]
    fn embedded_scripts_are_listed_read_and_imported() {
        let sources = Sources::embedded("scripts", SCRIPTS);
        assert_eq!(sources.names().unwrap(), ["broken", "double"]);
        assert!(sources.has("double") && !sources.has("missing"));
        assert_eq!(sources.path("double"), Path::new("scripts/double.rhai"));

        let mut engine = Engine::new();
        engine.set_module_resolver(Sources::embedded("lib", LIB).module_resolver());
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
        ]);
        assert_eq!(sources.names().unwrap(), ["broken", "double", "extra"]);
        assert!(sources.has("extra"));
        let mut engine = Engine::new();
        engine.set_module_resolver(Sources::embedded("lib", LIB).module_resolver());
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

    #[test]
    fn files_stand_in_front_of_others() {
        let library = Sources::embedded("library/locks", &[("shared", "1"), ("replaced", "2")]);
        let journal = Sources::embedded("journal/locks", &[("replaced", "3"), ("own", "4")]).over(library);
        assert_eq!(journal.names().unwrap(), ["own", "replaced", "shared"]);
        assert_eq!(journal.own_names().unwrap(), ["own", "replaced"]);
        assert_eq!(journal.read("shared").unwrap(), "1");
        assert_eq!(journal.read("replaced").unwrap(), "3");
        assert_eq!(journal.path("shared"), Path::new("library/locks/shared.rhai"));
        assert_eq!(journal.path("replaced"), Path::new("journal/locks/replaced.rhai"));
        assert!(journal.overrides("replaced") && !journal.overrides("shared") && !journal.overrides("own"));
        assert_eq!(journal.relative(Path::new("library/locks/shared.rhai")), Path::new("shared.rhai"));
        assert_eq!(journal.relative(Path::new("journal/locks/own.rhai")), Path::new("own.rhai"));

        // A journal with no such directory has only the library's.
        let missing = Sources::dir("/nowhere/at/all").over(Sources::embedded("library", &[("shared", "1")]));
        assert_eq!(missing.names().unwrap(), ["shared"]);
        assert_eq!(missing.read("shared").unwrap(), "1");

        // Reads of a library's files are noted, not the journal's own.
        let used: Rc<RefCell<BTreeSet<String>>> = Rc::default();
        let library = Sources::embedded("library", &[("shared", "1"), ("replaced", "2"), ("unread", "3")]).recording(used.clone());
        let journal = Sources::embedded("journal", &[("replaced", "4")]).over(library);
        journal.read("shared").unwrap();
        journal.read("replaced").unwrap();
        assert_eq!(used.borrow().iter().collect::<Vec<_>>(), ["shared"]);
        journal.unrecorded().read("unread").unwrap();
        assert_eq!(used.borrow().len(), 1);

        // Other extensions.
        let dir = std::env::temp_dir().join(format!("scribe-sources-ext-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lorem.txt"), "lorem").unwrap();
        std::fs::write(dir.join("other.md"), "other").unwrap();
        let text = Sources::dir(&dir).with_ext("txt");
        assert_eq!(text.names().unwrap(), ["lorem"]);
        assert_eq!(text.read("lorem").unwrap(), "lorem");
        assert!(!text.has("other"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The media type of pictures with extension `ext`; `None` for text.
pub fn image_type(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" => Some("image/jpeg"),
        _ => None,
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `bytes` in base64, padded.
pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= chunk.len() { BASE64[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// The bytes `text` holds in base64, padded or not, whitespace aside;
/// `None` if it isn't base64.
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut bits, mut count) = (0u32, 0);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        bits = bits << 6 | BASE64.iter().position(|&b| b == c)? as u32;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod base64_tests {
    use super::*;

    #[test]
    fn base64_goes_there_and_back() {
        for bytes in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar", &[0, 255, 128, 7, 250]] {
            let text = base64(bytes);
            assert_eq!(text.len() % 4, 0, "padded");
            assert_eq!(decode_base64(&text).as_deref(), Some(bytes), "{}", text);
        }
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(decode_base64("Zm9v\nYg"), Some(b"foob".to_vec()), "unpadded, with whitespace");
        assert_eq!(decode_base64("Zm9v!"), None);
    }

    #[test]
    fn pictures_held_as_data_urls_have_bytes() {
        let sources = Sources::none("images").with_ext("png").with_overlay([("dot".to_string(), "data:image/png;base64,AQID".to_string())]);
        assert_eq!(sources.read_bytes("dot").unwrap(), vec![1, 2, 3]);
        assert!(sources.read_bytes("nope").is_err());
    }
}
