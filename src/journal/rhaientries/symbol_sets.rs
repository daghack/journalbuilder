//! Symbol sets: a glyphset's glyphs as the symbols of a lock, and how each
//! is drawn, so lock values reach the page without translating them to
//! strings and glyphs by hand. A set is a file in `symbolsets/` (a
//! journal's over the library's), simple or scripted.
//!
//! A simple set, `symbolsets/NAME.json`, is data:
//!
//! ```json
//! { "description": "...", "glyphset": "thirteen_black_priests", "glyphs": ["0", "1", null], "style": "heavy_glyphs" }
//! ```
//!
//! `glyphs` name the glyphset's own glyphs in symbol order (`null` a blank,
//! set as a space; left out, all of them); `style` is a text style file,
//! whose pen, hand and size draw the glyphs.
//!
//! A scripted set, `symbolsets/NAME.rhai`, defines `symbols(o)`, giving the
//! same map (`glyphset`, `glyphs` or `glyph` and `count`, `style` and `size`,
//! both optional), and `draw(o, symbol, glyph)`, giving a canvas for each
//! symbol: `o` its options (declared by an optional `options()`), `symbol`
//! the whole number and `glyph` its glyph as placed.
//!
//! ```rhai
//! let marks = symbol_set("dread_stars");
//! marks.base;                          // 5
//! let glyphs = marks.glyphs([0, 3]);   // glyphs that remember their symbols
//! for (g, i) in glyphs { g.position = vec2(i * 80, 0); }
//! canvas.add(glyphs, marks);           // each drawn as the set draws it
//! ```

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use rand::Rng;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map, NativeCallContext, Scope, AST};

use super::canvas::CanvasData;
use super::glyphs::{add_glyphs, Glyph, GLYPH_PROPS};
use super::illustrations::description;
use super::locks::{self, LockValue};
use super::options::{apply_props, check_keys, declared_defaults, over_defaults, unwrap_runtime, Settings};
use super::random::SharedRng;
use super::sources::Sources;
use super::text_settings::{ParagraphStyle, TypeStyle};
use super::text_styles::{whole, TextStyleLibrary};
use super::{Canvas, RhaiResult, Style};
use crate::journal::fonts::{Font, FontBook};
use crate::journal::symbol::Symbol;

/// The most symbols a set has.
pub const MOST_SYMBOLS: usize = 1024;

/// How many scripted sets can be drawing inside one another.
const MAX_DEPTH: usize = 32;

/// What a set file's map has.
const KEYS: [&str; 7] = ["description", "glyphset", "glyphs", "glyph", "count", "style", "size"];

/// A symbol set, as the editor lists it: what it is and takes.
#[derive(serde::Serialize)]
pub struct SymbolSetInfo {
    pub name: String,
    pub description: String,
    /// Whether a script draws it.
    pub scripted: bool,
    /// How many symbols it has, with its options' defaults.
    pub base: u32,
    /// The glyph scale it draws at.
    pub size: f32,
    /// Its options' defaults: a scripted set's own, a simple set's (its
    /// style's) size and colour.
    pub options: serde_json::Value,
}

/// A symbol set file, read.
enum SetFile {
    Data { spec: Map },
    Script { ast: Rc<AST>, file: String, defaults: Map },
}

/// The symbol set files, loaded on first use.
pub struct SymbolSetLibrary {
    data: Sources,
    scripts: Sources,
    styles: Rc<TextStyleLibrary>,
    book: Rc<FontBook>,
    loaded: RefCell<HashMap<String, Rc<SetFile>>>,
    /// How deep scripted sets are calling each other now.
    depth: Rc<Cell<usize>>,
}

/// A symbol set made from a file with its options: what a script holds.
#[derive(Clone)]
pub struct SymbolSet(Rc<SetInner>);

/// Symbols of a set, as text for the text calls: `set.text(symbols)`.
#[derive(Clone)]
pub struct SymbolText {
    pub(super) set: SymbolSet,
    pub(super) symbols: Vec<u32>,
}

pub(super) struct SetInner {
    pub name: String,
    pub font: Font,
    /// Each symbol's glyph, `None` for a blank.
    pub names: Vec<Option<String>>,
    /// Its style, type and paragraph, whole: the settings text in it starts from.
    pub bundle: Map,
    /// The glyph scale it draws at.
    pub size: f32,
    script: Option<Scripted>,
}

struct Scripted {
    ast: Rc<AST>,
    o: Map,
    file: String,
    depth: Rc<Cell<usize>>,
}

impl SymbolSetLibrary {
    pub fn new(data: Sources, scripts: Sources, styles: Rc<TextStyleLibrary>, book: Rc<FontBook>) -> Self {
        SymbolSetLibrary { data, scripts, styles, book, loaded: RefCell::default(), depth: Rc::default() }
    }

    /// Every symbol set's name.
    pub fn names(&self) -> Vec<String> {
        let mut names = self.data.names().unwrap_or_default();
        names.extend(self.scripts.names().unwrap_or_default());
        names.sort();
        names.dedup();
        names
    }

    fn file(&self, engine: &Engine, name: &str) -> Result<Rc<SetFile>, String> {
        if let Some(loaded) = self.loaded.borrow().get(name) {
            return Ok(loaded.clone());
        }
        if name.is_empty() || name.contains(['/', '\\', '.']) || !(self.data.has(name) || self.scripts.has(name)) {
            let names = self.names();
            return Err(format!("no symbol set named '{}'; symbol sets: {}", name, if names.is_empty() { "none".to_string() } else { names.join(", ") }));
        }
        if self.data.has(name) && self.scripts.has(name) {
            return Err(format!("symbol set '{}' is both {} and {}: it should be one", name, self.data.path(name).display(), self.scripts.path(name).display()));
        }
        let what = format!("symbol set '{}'", name);
        let file = if self.data.has(name) {
            let path = self.data.path(name).display().to_string();
            let source = self.data.read(name).map_err(|e| e.to_string())?;
            let json: serde_json::Value = serde_json::from_str(&source).map_err(|e| format!("{}: {} isn't JSON: {}", what, path, e))?;
            if !json.is_object() {
                return Err(format!("{}: {} should be a JSON object", what, path));
            }
            SetFile::Data { spec: locks::from_json(&json).cast::<Map>() }
        } else {
            let path = self.scripts.path(name).display().to_string();
            let ast = self.scripts.compile(engine, name).map_err(|e| format!("{}: {}", what, e))?;
            let defines = |f: &str, n: usize| ast.iter_functions().any(|d| d.name == f && d.params.len() == n);
            for (function, params, shown) in [("symbols", 1, "symbols(o)"), ("draw", 3, "draw(o, symbol, glyph)")] {
                if !defines(function, params) {
                    return Err(format!("{}: {} doesn't define {}", what, path, shown));
                }
            }
            let defaults = self.deeper(name, || declared_defaults(engine, &ast, &what))?;
            SetFile::Script { ast: Rc::new(ast), file: path, defaults }
        };
        let file = Rc::new(file);
        self.loaded.borrow_mut().insert(name.to_string(), file.clone());
        Ok(file)
    }

    /// `call`, a call into scripted set `name`'s file, one level deeper.
    fn deeper<T>(&self, name: &str, call: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        deeper(&self.depth, name, call)
    }

    /// The description of set `name`: its file's `description` or opening comment.
    pub fn describe(&self, name: &str) -> String {
        if self.data.has(name) {
            let spec = self.data.read(name).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
            return spec.and_then(|v| v.get("description").and_then(|d| d.as_str().map(str::to_string))).unwrap_or_default();
        }
        self.scripts.read(name).map(|source| description(&source, name)).unwrap_or_default()
    }

    /// What set `name` is and takes, as the editor lists it.
    pub fn info(&self, engine: &Engine, name: &str) -> Result<SymbolSetInfo, String> {
        let file = self.file(engine, name)?;
        let set = self.make(engine, name, &Map::new())?;
        let (scripted, options) = match &*file {
            SetFile::Script { defaults, .. } => (true, defaults.clone()),
            SetFile::Data { spec } => {
                let style = spec.get("style").and_then(|s| s.clone().into_string().ok());
                (false, style.and_then(|s| self.styles.load(&s).ok()).map(|s| s.defaults.clone()).unwrap_or_default())
            }
        };
        Ok(SymbolSetInfo {
            name: name.to_string(),
            description: self.describe(name),
            scripted,
            base: set.base(),
            size: set.inner().size,
            options: super::locks::to_json(&Dynamic::from_map(options)),
        })
    }

    /// Set `name` with `options`: a simple set's are its style's (`size`,
    /// `color`), a scripted set's its own.
    pub fn make(&self, engine: &Engine, name: &str, options: &Map) -> Result<SymbolSet, String> {
        let what = format!("symbol set '{}'", name);
        match &*self.file(engine, name)? {
            SetFile::Data { spec } => {
                let (font, names, bundle, size) = self.build(&what, spec.clone(), options)?;
                Ok(SymbolSet(Rc::new(SetInner { name: name.to_string(), font, names, bundle, size, script: None })))
            }
            SetFile::Script { ast, file, defaults } => {
                let o = over_defaults(&what, defaults, options)?;
                let value = self.deeper(name, || {
                    engine.call_fn::<Dynamic>(&mut Scope::new(), ast, "symbols", (o.clone(),)).map_err(|e| format!("{} symbols(o): {}", what, e))
                })?;
                let type_name = value.type_name();
                let spec = value.try_cast::<Map>().ok_or_else(|| format!("{}: symbols(o) should return #{{ glyphset, glyphs, style, size }}, got {}", what, type_name))?;
                let (font, names, bundle, size) = self.build(&what, spec, &Map::new())?;
                let script = Scripted { ast: ast.clone(), o: o.clone(), file: file.clone(), depth: self.depth.clone() };
                Ok(SymbolSet(Rc::new(SetInner { name: name.to_string(), font, names, bundle, size, script: Some(script) })))
            }
        }
    }

    /// The glyphset, glyph names, bundle and size a set's map describes;
    /// `style_options` go to its style.
    fn build(&self, what: &str, mut spec: Map, style_options: &Map) -> Result<(Font, Vec<Option<String>>, Map, f32), String> {
        check_keys(what, &spec, &KEYS).map_err(|e| unwrap_runtime(&e))?;
        let mut text = |key: &str| match spec.remove(key) {
            None => Ok(None),
            Some(value) => value.into_string().map(Some).map_err(|t| format!("{}: '{}' should be a string, got {}", what, key, t)),
        };
        let glyphset = text("glyphset")?.ok_or_else(|| format!("{}: it needs a 'glyphset'", what))?;
        let style = text("style")?;
        let single = text("glyph")?;
        let font = self.book.font(&glyphset).ok_or_else(|| format!("{}: no glyphset named '{}'; glyphsets: {}", what, glyphset, self.book.names().join(", ")))?;
        let names: Vec<Option<String>> = match (spec.remove("glyphs"), single, spec.remove("count")) {
            (Some(glyphs), None, None) => {
                let glyphs = glyphs.into_array().map_err(|t| format!("{}: 'glyphs' should be an array of glyph names, got {}", what, t))?;
                glyphs.into_iter().enumerate().map(|(i, g)| {
                    if g.is_unit() {
                        return Ok(None);
                    }
                    let type_name = g.type_name();
                    g.into_string().map(Some).map_err(|_| format!("{}: glyph {} should be a name or null, got {}", what, i, type_name))
                }).collect::<Result<_, String>>()?
            }
            (None, Some(glyph), count) => {
                let count = match count {
                    None => return Err(format!("{}: 'glyph' needs a 'count'", what)),
                    Some(c) => c.as_int().ok().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| format!("{}: 'count' should be a whole number, got {}", what, c))?,
                };
                vec![Some(glyph); count]
            }
            (None, None, None) => font.characters().into_iter().map(Some).collect(),
            _ => return Err(format!("{}: give 'glyphs', or 'glyph' and 'count', not both nor 'count' alone", what)),
        };
        if names.is_empty() || names.len() > MOST_SYMBOLS {
            return Err(format!("{}: a symbol set has from 1 to {} symbols, got {}", what, MOST_SYMBOLS, names.len()));
        }
        for (i, name) in names.iter().enumerate() {
            if let Some(name) = name
                && font.variants(name).is_none() {
                return Err(format!("{}: glyphset '{}' has no glyph '{}' (symbol {}); its glyphs: {}", what, glyphset, name, i, font.characters().join(", ")));
            }
        }
        let bundle = match &style {
            Some(style) => self.styles.load(style).map_err(|e| format!("{}: {}", what, e))?.bundle(&self.book, style_options).map_err(|e| format!("{}: {}", what, e))?,
            None => whole(what, Map::new()).map_err(|e| unwrap_runtime(&e))?,
        };
        let given = spec.remove("size");
        let size = match given {
            Some(size) => size.as_float().or_else(|_| size.as_int().map(|n| n as f64)).map(|n| n as f32).ok().filter(|n| *n > 0.0)
                .ok_or_else(|| format!("{}: 'size' should be a positive number, got {}", what, size))?,
            None => bundle.get("type").and_then(|t| t.clone().try_cast::<TypeStyle>()).filter(|t| t.has_size()).map(|t| t.size(what)).transpose()
                .map_err(|e| unwrap_runtime(&e))?
                .ok_or_else(|| format!("{}: it needs a size: a 'size', or a 'style' that has one", what))?,
        };
        Ok((font, names, bundle, size))
    }
}

/// `call`, a call into scripted set `name`'s file, one level deeper.
fn deeper<T>(depth: &Cell<usize>, name: &str, call: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    if depth.get() >= MAX_DEPTH {
        return Err(format!("symbol set '{}': symbol sets are drawing each other more than {} deep; is one drawing itself?", name, MAX_DEPTH));
    }
    depth.set(depth.get() + 1);
    let result = call();
    depth.set(depth.get() - 1);
    result
}

impl SymbolSet {
    pub(super) fn inner(&self) -> &SetInner {
        &self.0
    }

    /// How many symbols it has.
    pub fn base(&self) -> u32 {
        self.0.names.len() as u32
    }

    /// Whether it's drawn by a script.
    pub fn is_scripted(&self) -> bool {
        self.0.script.is_some()
    }

    /// The set's own style: its pen and hand.
    pub fn style(&self) -> Result<Style, String> {
        self.0.bundle.get("style").and_then(|s| s.clone().try_cast::<Style>()).ok_or_else(|| format!("symbol set '{}' has no style", self.0.name))
    }

    /// The glyph for `symbol`, at the origin, `scale` the set's size: the
    /// glyphset's first variant of it, as `glyphset.glyph` gives, so it
    /// draws nothing from the generator (text, which sets glyphs as
    /// `glyphset.glyphs` does, picks variants).
    pub fn glyph(&self, symbol: u32) -> Result<Symbol, String> {
        let name = self.0.names.get(symbol as usize)
            .ok_or_else(|| format!("symbol {} is outside symbol set '{}' (0 to {})", symbol, self.0.name, self.0.names.len() - 1))?;
        let mut glyph = match name {
            None => self.0.font.blank(1.0),
            Some(name) => self.0.font.variants(name).and_then(|v| v.into_iter().next()).expect("a set's glyphs were checked when it was made"),
        };
        glyph.scale_by(self.0.size);
        Ok(glyph.with_stands_for(symbol))
    }

    /// The glyphs for `symbols` as text sets them: each at scale 1, from
    /// `font` (the set's glyphset as the text's type sets it), a variant
    /// picked with `rng`, a blank where the set has one.
    pub(super) fn text_glyphs(&self, font: &Font, symbols: &[u32], rng: &mut dyn rand::RngCore) -> Vec<Symbol> {
        symbols.iter().map(|&s| {
            let glyph = match &self.0.names[s as usize] {
                None => font.blank(1.0),
                Some(name) => {
                    let mut variants = font.variants(name).or_else(|| self.0.font.variants(name)).expect("a set's glyphs were checked when it was made");
                    let pick = rng.random_range(0..variants.len());
                    variants.swap_remove(pick)
                }
            };
            glyph.with_stands_for(s)
        }).collect()
    }

    /// What the set draws for each of `glyphs` (each standing for one of its
    /// symbols, none blank), in order, as its script's `draw` gives them:
    /// none for a simple set, which text draws itself. `engine` runs the
    /// script, and must not be inside a borrow of the generator.
    pub(super) fn drawn(&self, engine: &Engine, glyphs: &[Symbol]) -> RhaiResult<Vec<CanvasData>> {
        let Some(script) = &self.0.script else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for glyph in glyphs {
            let Some(symbol) = glyph.stands_for() else {
                return Err(format!("symbol set '{}': a glyph it was to draw doesn't stand for a symbol", self.0.name).into());
            };
            if glyph.is_blank() {
                continue;
            }
            let result = deeper(&script.depth, &self.0.name, || {
                engine.call_fn::<Dynamic>(&mut Scope::new(), &script.ast, "draw", (script.o.clone(), symbol as i64, Glyph::new(glyph.clone())))
                    .map_err(|e| format!("symbol set '{}' ({}) draw(o, {}, glyph): {}", self.0.name, script.file, symbol, e))
            })?;
            let type_name = result.type_name();
            let canvas = result.try_cast::<Canvas>()
                .ok_or_else(|| format!("symbol set '{}': draw(o, {}, glyph) should return a canvas, got {}", self.0.name, symbol, type_name))?;
            out.push(canvas.get());
        }
        Ok(out)
    }
}

/// `symbols`, a lock value or an array of whole numbers, as symbols of a
/// set of `base`: each must be below it.
pub(super) fn symbols_of(caller: &str, symbols: &Dynamic, base: u32) -> RhaiResult<Vec<u32>> {
    let numbers: Vec<u32> = if let Some(value) = symbols.clone().try_cast::<LockValue>() {
        value.base_and_symbols().map_err(|e| format!("{}: {}", caller, e))?.1
    } else {
        let type_name = symbols.type_name();
        let items = symbols.clone().try_cast::<Array>().ok_or_else(|| format!("{}: symbols should be symbols or an array of whole numbers, got {}", caller, type_name))?;
        items.iter().enumerate().map(|(i, item)| {
            item.as_int().ok().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| format!("{}: symbol {} is {}, not a whole number of at least 0", caller, i, item))
        }).collect::<Result<_, String>>()?
    };
    if let Some((i, s)) = numbers.iter().enumerate().find(|(_, s)| **s >= base) {
        return Err(format!("{}: symbol {} (item {}) is outside the symbol set's 0-{}", caller, s, i, base - 1).into());
    }
    Ok(numbers)
}

/// `settings` for a text call writing `set`'s symbols: the set's own style,
/// type and paragraph, with the call's over them. The glyphset is always the
/// set's, and the glyph scale its size unless the call gives one; spans are
/// left out.
pub(super) fn symbol_settings(caller: &str, set: &SymbolSet, mut settings: Map) -> RhaiResult<Map> {
    let bundle = &set.0.bundle;
    let font = Dynamic::from(set.0.font.clone());
    let mut base = bundle.get("type").and_then(|t| t.clone().try_cast::<TypeStyle>()).unwrap_or_else(TypeStyle::empty);
    base.set(caller, "glyphset", &font)?;
    base.set(caller, "glyph_scale", &Dynamic::from(set.0.size as f64))?;
    let mut type_ = match settings.remove("type") {
        None => base,
        Some(t) => TypeStyle::given(&format!("{} 'type'", caller), &t, &base)?,
    };
    type_.set(caller, "glyphset", &font)?;
    if !type_.has_size() {
        type_.set(caller, "glyph_scale", &Dynamic::from(set.0.size as f64))?;
    }
    let style_base = set.style()?;
    let style = match settings.remove("style") {
        None => style_base,
        Some(s) => Style::given(&format!("{} 'style'", caller), &s, &style_base)?,
    };
    let paragraph_base = bundle.get("paragraph").and_then(|p| p.clone().try_cast::<ParagraphStyle>()).unwrap_or_default();
    let paragraph = match settings.remove("paragraph") {
        None => paragraph_base,
        Some(p) => ParagraphStyle::given(&format!("{} 'paragraph'", caller), &p, &paragraph_base)?,
    };
    settings.remove("spans");
    settings.insert("type".into(), Dynamic::from(type_));
    settings.insert("style".into(), Dynamic::from(style));
    settings.insert("paragraph".into(), Dynamic::from(paragraph));
    Ok(settings)
}

/// A symbol (a whole number below the set's base) from a script value.
fn symbol_of(caller: &str, set: &SymbolSet, symbol: &Dynamic) -> RhaiResult<u32> {
    let n = symbol.as_int().ok().and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| format!("{}: the symbol should be a whole number of at least 0, got {}", caller, symbol))?;
    if n >= set.base() {
        return Err(format!("{}: symbol {} is outside symbol set '{}' (0 to {})", caller, n, set.0.name, set.base() - 1).into());
    }
    Ok(n)
}

/// Add what `set` draws for `glyphs` (each standing for one of its symbols)
/// to `canvas`: a simple set's strokes in its style, a scripted set's
/// drawings. The generator is only borrowed after any script has run.
fn add_drawn(ctx: &NativeCallContext, canvas: &mut Canvas, glyphs: Vec<Glyph>, set: &SymbolSet, rng: &SharedRng) -> RhaiResult<Canvas> {
    const CALLER: &str = "canvas.add";
    let mut placed = Vec::with_capacity(glyphs.len());
    for (i, g) in glyphs.iter().enumerate() {
        let g = g.get();
        match g.stands_for() {
            Some(symbol) if symbol < set.base() => placed.push(g),
            Some(symbol) => return Err(format!("{}: glyph {} stands for symbol {}, outside symbol set '{}' (0 to {})", CALLER, i, symbol, set.0.name, set.base() - 1).into()),
            None => return Err(format!("{}: glyph {} doesn't stand for a symbol (make glyphs with set.glyph and set.glyphs)", CALLER, i).into()),
        }
    }
    if set.is_scripted() {
        let drawings = set.drawn(ctx.engine(), &placed)?;
        return Ok(canvas.update(|data| {
            for drawing in drawings {
                data.strokes.extend(drawing.strokes);
                data.commits.extend(drawing.commits);
            }
        }));
    }
    let style = set.style().map_err(|e| format!("{}: {}", CALLER, e))?;
    let mut result = Ok(());
    let canvas = canvas.update(|data| result = add_glyphs(data, &placed, |_| false, &style, &mut *rng.borrow_mut()));
    result.map(|()| canvas)
}

pub fn register(engine: &mut Engine, library: Rc<SymbolSetLibrary>, rng: SharedRng) {
    engine.register_type_with_name::<SymbolSet>("SymbolSet");
    let lib = library.clone();
    engine.register_fn("symbol_set", move |ctx: NativeCallContext, name: &str| -> RhaiResult<SymbolSet> {
        lib.make(ctx.engine(), name, &Map::new()).map_err(|e| format!("symbol_set: {}", e).into())
    });
    let lib = library.clone();
    engine.register_fn("symbol_set", move |ctx: NativeCallContext, name: &str, options: Map| -> RhaiResult<SymbolSet> {
        lib.make(ctx.engine(), name, &options).map_err(|e| format!("symbol_set: {}", e).into())
    });
    engine.register_fn("symbol_set_names", move || -> Array { library.names().into_iter().map(Dynamic::from).collect() });
    engine.register_get("name", |set: &mut SymbolSet| set.0.name.clone());
    engine.register_get("base", |set: &mut SymbolSet| set.base() as i64);
    engine.register_get("size", |set: &mut SymbolSet| set.0.size as f64);
    engine.register_type_with_name::<SymbolText>("SymbolText");
    engine.register_fn("text", |set: &mut SymbolSet, symbols: Dynamic| -> RhaiResult<SymbolText> {
        let symbols = symbols_of("text", &symbols, set.base())?;
        Ok(SymbolText { set: set.clone(), symbols })
    });
    engine.register_get("symbols", |t: &mut SymbolText| -> Array { t.symbols.iter().map(|s| Dynamic::from(*s as i64)).collect() });
    engine.register_get("symbol_set", |t: &mut SymbolText| t.set.clone());
    engine.register_get("symbol", |g: &mut Glyph| g.borrow().stands_for().map_or(Dynamic::UNIT, |s| Dynamic::from(s as i64)));

    engine.register_fn("glyph", move |set: &mut SymbolSet, symbol: Dynamic| -> RhaiResult<Glyph> {
        let n = symbol_of("glyph", set, &symbol)?;
        set.glyph(n).map(Glyph::new).map_err(|e| format!("glyph: {}", e).into())
    });
    engine.register_fn("glyph", move |ctx: NativeCallContext, set: &mut SymbolSet, symbol: Dynamic, options: Map| -> RhaiResult<Glyph> {
        let n = symbol_of("glyph", set, &symbol)?;
        let glyph = set.glyph(n).map(Glyph::new).map_err(|e| -> Box<EvalAltResult> { format!("glyph: {}", e).into() })?;
        apply_props(&ctx, "glyph", glyph, options, &GLYPH_PROPS)
    });
    engine.register_fn("glyphs", move |set: &mut SymbolSet, symbols: Dynamic| -> RhaiResult<Array> {
        let symbols = symbols_of("glyphs", &symbols, set.base())?;
        symbols.into_iter().map(|s| set.glyph(s).map(|g| Dynamic::from(Glyph::new(g))).map_err(|e| format!("glyphs: {}", e).into())).collect()
    });

    let r = rng.clone();
    engine.register_fn("add", move |ctx: NativeCallContext, canvas: &mut Canvas, glyph: Glyph, set: SymbolSet| -> RhaiResult<Canvas> {
        add_drawn(&ctx, canvas, vec![glyph], &set, &r)
    });
    engine.register_fn("add", move |ctx: NativeCallContext, canvas: &mut Canvas, glyphs: Array, set: SymbolSet| -> RhaiResult<Canvas> {
        let glyphs = super::glyphs::glyph_array("canvas.add", glyphs)?;
        add_drawn(&ctx, canvas, glyphs, &set, &rng)
    });
}

#[cfg(test)]
mod tests {
    use super::super::{Entry, JournalEngine, Resources};
    use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};
    use std::rc::Rc;

    fn engine(data: &[(&str, &str)], scripts: &[(&str, &str)]) -> JournalEngine {
        let mut resources = Resources::library();
        resources.symbol_set_data = resources.symbol_set_data.with_overlay(data.iter().map(|(n, s)| (n.to_string(), s.to_string())));
        resources.symbol_set_scripts = resources.symbol_set_scripts.with_overlay(scripts.iter().map(|(n, s)| (n.to_string(), s.to_string())));
        JournalEngine::with_resources(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).unwrap()), resources)
    }

    const PRIESTS: &str = r#"{ "description": "priests", "glyphset": "thirteen_black_priests", "glyphs": ["0", "1", null, "A"], "style": "plain_hand", "size": 40 }"#;
    const STARS: &str = r#"// two marks, a circle and a cross.
        fn options() { #{ big: 1.0 } }
        fn symbols(o) { #{ glyphset: "plain", glyph: "O", count: 2, size: 30.0 } }
        fn draw(o, symbol, glyph) {
            let c = new_canvas();
            let style = default_style();
            if symbol == 0 { c.add(new_circle(glyph.position, glyph.scale * o.big / 2.0), style); }
            else { c.add(new_line(glyph.position - vec2(5, 5), glyph.position + vec2(5, 5)), style); }
            c
        }
    "#;

    fn build(engine: &mut JournalEngine, script: &str) -> Result<Entry, String> {
        engine.build_entry_str(script).map_err(|e| e.to_string())
    }

    #[test]
    fn both_kinds_load_and_report_what_they_are() {
        let mut e = engine(&[("priests", PRIESTS)], &[("marks", STARS)]);
        let entry = build(&mut e, r#"
            let p = symbol_set("priests");
            assert(p.name == "priests" && p.base == 4 && p.size == 40.0, "a simple set");
            let m = symbol_set("marks", #{ big: 2.0 });
            assert(m.base == 2 && m.size == 30.0, "a scripted set");
            assert("priests" in symbol_set_names() && "marks" in symbol_set_names(), "listed");
            assert(symbol_set_names().len() >= 2, "listed");
        "#);
        entry.unwrap();
    }

    #[test]
    fn glyphs_remember_their_symbols_and_draw_by_their_sets() {
        let mut e = engine(&[("priests", PRIESTS)], &[("marks", STARS)]);
        let entry = build(&mut e, r#"
            let p = symbol_set("priests");
            let gs = p.glyphs([0, 2, 3]);
            assert(gs.len() == 3 && gs[1].symbol == 2 && gs[1].is_blank && gs[0].scale == 40.0, "tagged, scaled, a blank");
            for (g, i) in gs { g.position = vec2(i * 50, 0); }
            let c = new_canvas().add(gs, p);
            assert(c.bounds().width > 40.0, "simple glyphs drawn");
            let m = symbol_set("marks");
            let mg = m.glyphs([1, 0]);
            mg[1].position = vec2(100, 0);
            let d = new_canvas().add(mg, m);
            let b = d.bounds();
            assert(b.max.x > 100.0 && b.min.x < 5.0, "the script drew both");
            assert(p.glyph(3, #{ position: vec2(9, 9), scale: 12.0 }).position == vec2(9, 9), "options place it");
        "#);
        entry.unwrap();
    }

    #[test]
    fn symbols_are_text_in_every_text_call() {
        let mut e = engine(&[("priests", PRIESTS)], &[("marks", STARS)]);
        let entry = build(&mut e, r#"
            let p = symbol_set("priests");
            let m = symbol_set("marks");
            let text = p.text([0, 1, 2, 3, 0, 1]);
            // A line: the set's size and style unless the call says.
            let line = write_line(text, vec2(0, 0), #{});
            assert(line.bounds().width > 100.0, "a line of 6 symbols at size 40");
            let small = write_line(text, vec2(0, 0), #{ type: #{ glyph_scale: 20.0 } });
            assert(small.bounds().width < line.bounds().width * 0.75, "a size given wins");
            // A block wraps, and gives the canvas and where to go on.
            let block = write_block(p.text([0, 1, 2, 3, 0, 1, 3, 3, 1, 0, 0, 2, 1]), vec2(0, 0), 200.0, #{});
            assert(block.bottom > 40.0 && block.canvas.bounds().width <= 215.0, "wrapped at the width");
            // The scripted set, drawn by its script: a circle or a cross each.
            let marks = write_line(m.text([0, 1, 0]), vec2(0, 0), #{});
            assert(marks.bounds().width > 60.0, "three marks");
            let inside = write_inside_path(m.text([0, 1, 0, 1, 1, 0, 0, 1]), new_circle(vec2(100, 100), 90), #{});
            assert(inside.rest.symbols.len() == 0 || inside.rest.symbols.len() < 8, "what's left is symbols");
            let fitted = new_wave(vec2(0, 0), vec2(400, 0), #{ amplitude: 30.0 }).fit_text(p.text([0, 1, 2, 3]), #{});
            assert(fitted.bounds().width > 100.0, "along a path");
            let ring = art::spiral_text(p.text([0, 1, 2, 3, 0, 1, 2, 3, 0, 1]), vec2(150, 150), 140.0, #{});
            assert(ring.bounds().width > 100.0, "in a spiral");
            // A glyphset given is ignored: the set's own is used.
            let same = write_line(text, vec2(0, 0), #{ type: #{ glyphset: glyphset("plain") } });
            assert((same.bounds().width - line.bounds().width).abs() < 0.25 * line.bounds().width, "the set's glyphs, not an error");
        "#);
        entry.unwrap();
    }

    #[test]
    fn write_at_puts_the_middle_of_the_run_on_the_point() {
        let mut e = engine(&[("priests", PRIESTS)], &[("marks", STARS)]);
        let entry = build(&mut e, r#"
            let p = symbol_set("priests");
            let one = write_at(p.text([0]), vec2(300, 200), #{});
            let b = one.bounds();
            assert((b.center.x - 300.0).abs() < 8.0 && (b.center.y - 200.0).abs() < 8.0, "a single glyph centred: " + b.center);
            let run = write_at(p.text([0, 1, 3, 0]), vec2(300, 200), #{});
            assert((run.bounds().center.x - 300.0).abs() < 12.0, "a run centred");
            let turned = write_at(p.text([0, 1, 3, 0]), vec2(300, 200), #{ turn: PI() / 2.0 });
            assert(turned.bounds().height > turned.bounds().width, "turned a quarter, it runs down the page");
            // Strings take it too.
            let word = write_at("MIDDLE", vec2(50, 50), #{ style: default_style(), type: new_type_style(glyphset("plain"), 20.0) });
            assert((word.bounds().center.x - 50.0).abs() < 8.0, "text centred");
            // A scripted set's symbol drawn at the point.
            let m = symbol_set("marks");
            let star = write_at(m.text([0]), vec2(120, 80), #{});
            assert((star.bounds().center.x - 120.0).abs() < 4.0, "a script's drawing is where its glyph was put");
        "#);
        entry.unwrap();
    }

    #[test]
    fn the_editor_lists_and_previews_sets() {
        let mut e = engine(&[("priests", PRIESTS)], &[("marks", STARS)]);
        let simple = e.describe_symbol_set("priests").unwrap();
        assert_eq!((simple.base, simple.scripted, simple.description.as_str()), (4, false, "priests"));
        assert!(simple.options.get("size").is_some() && simple.options.get("color").is_some(), "a simple set's options are its style's");
        let scripted = e.describe_symbol_set("marks").unwrap();
        assert_eq!((scripted.base, scripted.scripted), (2, true));
        assert_eq!(scripted.options["big"], 1.0);
        assert!(scripted.description.contains("two marks"), "{}", scripted.description);
        assert!(e.symbol_set_names().contains(&"dread_stars".to_string()));
        let entry = e.preview_symbol_set("marks", &serde_json::json!({ "big": 2.0 })).unwrap();
        assert_eq!(entry.pages.len(), 1);
        assert!(entry.pages[0].canvases()[0].strokes.len() >= 4, "two marks and their numbers");
        assert!(e.describe_symbol_set("nothing").is_err());
    }

    #[test]
    fn mistakes_are_named() {
        let mut e = engine(&[("priests", PRIESTS), ("bad_glyph", r#"{ "glyphset": "plain", "glyphs": ["A", "~~"], "size": 20 }"#), ("no_size", r#"{ "glyphset": "plain" }"#), ("odd_key", r#"{ "glyphset": "plain", "size": 9, "colour": 1 }"#)], &[("marks", STARS), ("priests", STARS)]);
        let fails = |e: &mut JournalEngine, script: &str| build(e, script).err().unwrap_or_default();
        assert!(fails(&mut e, r#"symbol_set("nothing");"#).contains("no symbol set named 'nothing'; symbol sets:"));
        assert!(fails(&mut e, r#"symbol_set("priests");"#).contains("is both"));
        assert!(fails(&mut e, r#"symbol_set("bad_glyph");"#).contains("has no glyph '~~' (symbol 1)"));
        assert!(fails(&mut e, r#"symbol_set("no_size");"#).contains("it needs a size"));
        assert!(fails(&mut e, r#"symbol_set("odd_key");"#).contains("colour"));
        assert!(fails(&mut e, r#"symbol_set("marks", #{ small: 1 });"#).contains("has no option 'small'"));
        assert!(fails(&mut e, r#"symbol_set("marks").glyphs([0, 2]);"#).contains("symbol 2 (item 1) is outside the symbol set's 0-1"));
        assert!(fails(&mut e, r#"let g = glyphset("plain").glyph("A"); new_canvas().add(g, symbol_set("marks"));"#).contains("doesn't stand for a symbol"));
    }
}
