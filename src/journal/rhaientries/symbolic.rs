//! Symbolics: what the text calls give when they're writing symbols of a
//! symbol set (`write_line(set.text(symbols), ...)`). A symbolic is a canvas
//! in every respect (everything that takes a canvas takes one, and moving
//! it, adding to it and committing it work as they do for a canvas), but it
//! remembers which symbols of which set it was drawn from, so they can be
//! changed, and it's drawn again.
//!
//! ```rhai
//! let mark = write_at(marks.text([2]), vec2(100, 100), #{});
//! mark.symbols;                       // [2]
//! mark.symbols = [4];                 // drawn again, as symbol 4
//! mark.symbol_set = symbol_set("moons");
//! let plate = new_canvas().add(frame).add(mark).add(other_mark);
//! page.add(plate.commit_to_editor("plate"));
//! ```
//!
//! The remembering is the canvas's: every canvas keeps track of the
//! symbolics it holds (`CanvasData::symbolics`), a symbolic's canvas of
//! itself first of all, and adding a canvas to another hands them on with
//! its strokes. So the visual editor can change the symbols and set of every
//! symbolic in a canvas committed to it: `commit_to_editor` applies the
//! edit's `symbolics`, each by its id, by drawing them again.
//!
//! A symbolic's id is made from the call that drew it (which call, where,
//! with what settings; not its symbols, which an edit changes), so it's the
//! same every run, and stays an edit's however many other symbolics the
//! script goes on to draw before it or after. Those of a canvas drawn by
//! calls alike in all of that (a plate added twice) are told apart by their
//! order among themselves: the first has the id, the next `ID.2`, and so on.
//!
//! Drawing again runs the same call with the symbols changed, from the
//! generator as it stood before the first time, then what's been done to its
//! strokes since (`translate_by`, `scale_by`, `rotate_around`, `size_to`,
//! an edit in the visual editor), to it or to a canvas it was added to, is
//! done again, so jitters land where they did and a redraw never changes
//! what the rest of the script draws. Other strokes stay as they are.

use std::collections::BTreeMap;
use std::rc::Rc;

use parry2d::math::Vec2 as PVec;
use rand::rngs::StdRng;
use rhai::{Array, Dynamic, Engine, Map, NativeCallContext};
use serde::Serialize;

use super::canvas::{CanvasData, Fit, Stroke};
use super::edits::{Applied, SymbolicEdit};
use super::markdown::FlowText;
use super::random::SharedRng;
use super::symbol_sets::{symbols_of, SymbolSet, SymbolSetLibrary, SymbolText};
use super::{Canvas, RhaiResult};

/// A symbolic: a canvas that holds, first of the symbolics it holds, the
/// one it was drawn as. Copies of the variable are the same, as a canvas's.
#[derive(Clone)]
pub struct Symbolic {
    canvas: Canvas,
}

/// How a symbolic's strokes are drawn again for other symbols.
type Recipe = Rc<dyn Fn(&Engine, &SymbolText) -> RhaiResult<CanvasData>>;

/// A symbolic a canvas holds: the symbols that drew some of its strokes,
/// and how to draw them again.
#[derive(Clone)]
pub struct Held {
    /// Which call drew it (see `site_id`).
    id: Rc<str>,
    text: SymbolText,
    recipe: Recipe,
    /// The generator as it stood before it was first drawn.
    start: StdRng,
    rng: SharedRng,
    /// What's been done to its strokes since they were drawn, to do again
    /// after drawing again.
    moves: Vec<Move>,
    /// Which of the canvas's strokes it drew: `drawn` of them, from `at`.
    /// Those a canvas holds are in the order of their strokes.
    at: usize,
    drawn: usize,
}

impl std::fmt::Debug for Held {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Held {{ {} {:?}, strokes {}..{} }}", self.text.set.inner().name, self.text.symbols, self.at, self.at + self.drawn)
    }
}

/// Something done to a held symbolic's strokes.
#[derive(Clone)]
pub(super) enum Move {
    Translate(PVec),
    Scale(PVec),
    Rotate(PVec, f32),
    /// Sized to a box, alone on its canvas: drawn again, it's fitted to the
    /// box again.
    SizeTo(Fit),
    /// Committed to the visual editor: the edit's placing and ink.
    Edit(Rc<Applied>),
}

/// An id for the call `site` describes (its name and arguments), the same
/// wherever it's made (the browser's edits are the command line's): 128
/// bits of FNV-1a, written as a UUID is.
fn site_id(site: &str) -> Rc<str> {
    let hash = |mut h: u64| {
        for byte in site.bytes() {
            h = (h ^ byte as u64).wrapping_mul(0x100000001b3);
        }
        h
    };
    let (a, b) = (hash(0xcbf29ce484222325), hash(0x84222325cbf29ce4));
    format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", a >> 32, (a >> 16) & 0xffff, a & 0xffff, b >> 48, b & 0xffff_ffff_ffff).into()
}

/// What each of `held`, a canvas's symbolics, is to an edit: its id, those
/// after the first with an id numbered among themselves.
pub(super) fn keys(held: &[Held]) -> Vec<String> {
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    held.iter().map(|h| {
        let n = seen.entry(&h.id).or_default();
        *n += 1;
        if *n == 1 { h.id.to_string() } else { format!("{}.{}", h.id, n) }
    }).collect()
}

impl Held {
    /// As it is in a canvas whose strokes it's added after `base` of.
    pub(super) fn after(&self, base: usize) -> Held {
        Held { at: self.at + base, ..self.clone() }
    }

    /// Note something done to its strokes.
    pub(super) fn remember(&mut self, step: Move) {
        match (self.moves.last_mut(), &step) {
            // Moves in a row are one, so a canvas moved over and over
            // doesn't remember each.
            (Some(Move::Translate(before)), Move::Translate(v)) => *before += *v,
            _ => self.moves.push(step),
        }
    }

    /// Whether it drew every one of a canvas's `strokes` strokes.
    pub(super) fn is_all(&self, strokes: usize) -> bool {
        self.at == 0 && self.drawn == strokes
    }

    /// The strokes of a canvas's it drew.
    pub(super) fn strokes(&self) -> std::ops::Range<usize> {
        self.at..self.at + self.drawn
    }

    /// Its strokes as `text`: drawn from the generator as it stood, and
    /// what's been done to them since done again.
    fn draw(&self, engine: &Engine, text: &SymbolText) -> RhaiResult<Vec<Stroke>> {
        let saved = self.rng.borrow().clone();
        *self.rng.borrow_mut() = self.start.clone();
        let result = (self.recipe)(engine, text);
        *self.rng.borrow_mut() = saved;
        let mut fresh = result?;
        fresh.symbolics.clear();
        for step in &self.moves {
            match step {
                Move::Translate(v) => fresh.transform_with(|path| path.translate_by(*v)),
                Move::Scale(s) => fresh.transform_with(|path| path.scale_by(*s)),
                Move::Rotate(point, rad) => fresh.transform_with(|path| path.rotate_around(*point, *rad)),
                Move::SizeTo(fit) => fresh.fit(*fit)?,
                Move::Edit(applied) => applied.apply(&mut fresh.strokes),
            }
        }
        Ok(fresh.strokes)
    }
}

impl CanvasData {
    /// Symbolic `index`'s strokes replaced by `fresh`, which draw `text`;
    /// the symbolics after it are of the strokes they were.
    fn redrawn(&mut self, index: usize, fresh: Vec<Stroke>, text: SymbolText) {
        let (range, count) = (self.symbolics[index].strokes(), fresh.len());
        let was = range.len();
        self.strokes.splice(range, fresh);
        for later in &mut self.symbolics[index + 1..] {
            later.at = later.at - was + count;
        }
        let held = &mut self.symbolics[index];
        held.drawn = count;
        held.text = text;
    }
}

/// What the visual editor is told of a symbolic a committed canvas holds:
/// what it is to the canvas's edit (`id`), its symbols and set as drawn (the edit applied), and as the script made
/// it; and where it is, its box's corners (top left, top right, bottom
/// right, bottom left) in the canvas as the canvas's edit left it (none if
/// it drew nothing).
#[derive(Clone, Debug, Serialize)]
pub struct SymbolicReport {
    pub id: String,
    pub symbols: Vec<u32>,
    pub symbol_set: String,
    pub base: u32,
    pub script_symbols: Vec<u32>,
    pub script_symbol_set: String,
    pub corners: Option<[[f32; 2]; 4]>,
}

/// `produce`'s canvas for `text`, as a symbolic that can draw it again for
/// other symbols; `extra` is whatever else the first call gave. `site` is
/// the call, its name and arguments but for the text, for its id.
pub(super) fn make_with<T>(
    engine: &Engine,
    rng: &SharedRng,
    site: &str,
    text: SymbolText,
    produce: impl Fn(&Engine, &FlowText) -> RhaiResult<(Canvas, T)> + 'static,
) -> RhaiResult<(Symbolic, T)> {
    let start = rng.borrow().clone();
    let (mut canvas, extra) = produce(engine, &FlowText::Symbols(text.clone()))?;
    let recipe: Recipe = Rc::new(move |engine, text| produce(engine, &FlowText::Symbols(text.clone())).map(|(canvas, _)| canvas.get()));
    let rng = rng.clone();
    canvas.update(|data| {
        // Every stroke is the call's: it holds this symbolic, and no other.
        let drawn = data.strokes.len();
        data.symbolics = vec![Held { id: site_id(site), text, recipe, start, rng, moves: Vec::new(), at: 0, drawn }];
    });
    Ok((Symbolic { canvas }, extra))
}

/// As `make_with`, for a call that gives only a canvas.
pub(super) fn make(
    engine: &Engine,
    rng: &SharedRng,
    site: &str,
    text: SymbolText,
    produce: impl Fn(&Engine, &FlowText) -> RhaiResult<Canvas> + 'static,
) -> RhaiResult<Symbolic> {
    make_with(engine, rng, site, text, move |engine, msg| produce(engine, msg).map(|canvas| (canvas, ()))).map(|(symbolic, ())| symbolic)
}

/// The canvas a script value is: a canvas, or a symbolic's.
pub(super) fn canvas_of(value: &Dynamic) -> Option<Canvas> {
    if let Some(canvas) = value.clone().try_cast::<Canvas>() {
        return Some(canvas);
    }
    value.clone().try_cast::<Symbolic>().map(|s| s.canvas)
}

/// `args` with every symbolic in them (and in arrays among them) as its canvas.
fn plain_args(args: Vec<Dynamic>) -> Vec<Dynamic> {
    args.into_iter().map(|arg| {
        if let Some(symbolic) = arg.clone().try_cast::<Symbolic>() {
            Dynamic::from(symbolic.canvas)
        } else if arg.is_array() {
            Dynamic::from_array(plain_args(arg.cast::<Array>()))
        } else {
            arg
        }
    }).collect()
}

/// What `name` of a canvas gives for `args` over the symbolic's canvas, the
/// symbolic itself if that's the canvas it hands back, so chains go on.
fn delegate(ctx: &NativeCallContext, name: &str, symbolic: &mut Symbolic, args: Vec<Dynamic>) -> RhaiResult<Dynamic> {
    let mut all = vec![Dynamic::from(symbolic.canvas.clone())];
    all.extend(plain_args(args));
    let result = ctx.call_native_fn::<Dynamic>(name, all)?;
    match result.clone().try_cast::<Canvas>() {
        Some(out) if out.same(&symbolic.canvas) => Ok(Dynamic::from(symbolic.clone())),
        _ => Ok(result),
    }
}

/// A set from a script value: a symbol set, or the name of one.
fn set_of(ctx: &NativeCallContext, library: &SymbolSetLibrary, value: &Dynamic) -> RhaiResult<SymbolSet> {
    if let Some(set) = value.clone().try_cast::<SymbolSet>() {
        return Ok(set);
    }
    let name = value.clone().into_immutable_string().map_err(|_| format!("symbol_set: should be a symbol set or its name, got {}", value.type_name()))?;
    library.make(ctx.engine(), &name, &Map::new()).map_err(|e| format!("symbol_set: {}", e).into())
}

/// Draw the symbolic `index` of those `canvas` holds again, as `set`'s
/// `symbols`.
fn resymbol(engine: &Engine, canvas: &Canvas, index: usize, set: SymbolSet, symbols: Vec<u32>) -> RhaiResult<()> {
    if let Some(s) = symbols.iter().find(|s| **s >= set.base()) {
        return Err(format!("symbol {} is outside symbol set '{}' (0 to {})", s, set.inner().name, set.base() - 1).into());
    }
    // Drawn with the canvas let go of: a set's script draws on canvases of its own.
    let held = canvas.borrow().symbolics[index].clone();
    let text = SymbolText { set, symbols };
    let fresh = held.draw(engine, &text)?;
    canvas.borrow_mut().redrawn(index, fresh, text);
    Ok(())
}

/// For `canvas.commit_to_editor(name)`: the symbolics `canvas` holds drawn
/// again as `edits` have them, each by its id (or, as older files have
/// them, by its place among them, from 0). What each is then, and was as
/// the script made it (without its corners, which the commit gives). An
/// edit for one it doesn't hold is left out, with a warning from `warn`.
pub(super) fn apply_edits(
    engine: &Engine,
    library: &SymbolSetLibrary,
    canvas: &Canvas,
    name: &str,
    edits: &BTreeMap<String, SymbolicEdit>,
    warn: impl Fn(String),
) -> RhaiResult<Vec<SymbolicReport>> {
    const CALLER: &str = "commit_to_editor";
    let script: Vec<SymbolText> = canvas.borrow().symbolics.iter().map(|held| held.text.clone()).collect();
    let ids = keys(&canvas.borrow().symbolics);
    for (key, edit) in edits {
        let place = ids.iter().position(|id| id == key).or_else(|| key.parse::<usize>().ok().filter(|n| *n < script.len()));
        let Some(index) = place else {
            warn(format!("{}: '{}' has an edit for a symbolic '{}', which it doesn't hold (it holds {}); the edit is left out", CALLER, name, key, script.len()));
            continue;
        };
        let was = &script[index];
        if edit.symbol_set.is_none() && edit.symbols.is_none() {
            continue;
        }
        let set = match &edit.symbol_set {
            Some(set_name) => library.make(engine, set_name, &Map::new())
                .map_err(|e| format!("{}: '{}': symbolic {} is edited to symbol set '{}': {}", CALLER, name, index, set_name, e))?,
            None => was.set.clone(),
        };
        let symbols = edit.symbols.clone().unwrap_or_else(|| was.symbols.clone());
        if let Some(bad) = symbols.iter().find(|n| **n >= set.base()) {
            return Err(format!("{}: '{}': symbolic {} is edited to symbol {}, but symbol set '{}' has {}", CALLER, name, index, bad, set.inner().name, set.base()).into());
        }
        resymbol(engine, canvas, index, set, symbols).map_err(|e| format!("{}: '{}': symbolic {}: {}", CALLER, name, index, e))?;
    }
    let data = canvas.borrow();
    Ok(data.symbolics.iter().zip(&script).zip(ids).map(|((held, was), id)| SymbolicReport {
        id,
        symbols: held.text.symbols.clone(),
        symbol_set: held.text.set.inner().name.clone(),
        base: held.text.set.base(),
        script_symbols: was.symbols.clone(),
        script_symbol_set: was.set.inner().name.clone(),
        corners: None,
    }).collect())
}

pub(super) fn register(engine: &mut Engine, library: Rc<SymbolSetLibrary>) {
    engine.register_type_with_name::<Symbolic>("Symbolic");
    // What it was drawn as: the first of the symbolics its canvas holds.
    engine.register_get("symbols", |s: &mut Symbolic| -> Array { s.canvas.borrow().symbolics[0].text.symbols.iter().map(|n| Dynamic::from(*n as i64)).collect() });
    engine.register_get("symbol_set", |s: &mut Symbolic| s.canvas.borrow().symbolics[0].text.set.clone());
    engine.register_fn("set$symbols", |ctx: NativeCallContext, s: &mut Symbolic, symbols: Dynamic| -> RhaiResult<()> {
        let set = s.canvas.borrow().symbolics[0].text.set.clone();
        let symbols = symbols_of("symbolic.symbols", &symbols, set.base())?;
        resymbol(ctx.engine(), &s.canvas, 0, set, symbols).map_err(|e| format!("symbolic.symbols: {}", e).into())
    });
    engine.register_fn("set$symbol_set", move |ctx: NativeCallContext, s: &mut Symbolic, set: Dynamic| -> RhaiResult<()> {
        let set = set_of(&ctx, &library, &set)?;
        let symbols = s.canvas.borrow().symbolics[0].text.symbols.clone();
        resymbol(ctx.engine(), &s.canvas, 0, set, symbols).map_err(|e| format!("symbolic.symbol_set: {}", e).into())
    });
    engine.register_get_set("name",
        |s: &mut Symbolic| s.canvas.borrow().name.clone().map_or(Dynamic::UNIT, Dynamic::from),
        |s: &mut Symbolic, name: Dynamic| -> RhaiResult<()> {
            s.canvas.borrow_mut().name = Some(super::check_name("canvas.name", name)?);
            Ok(())
        });
    engine.register_fn("copy", |s: &mut Symbolic| Symbolic { canvas: s.canvas.copy() });
    // Every canvas function, on the symbolic's canvas.
    for name in ["add", "translate_by", "scale_by", "size_to", "rotate_around", "scatter_notes", "bounds", "commit_to_editor"] {
        engine.register_fn(name, move |ctx: NativeCallContext, s: &mut Symbolic| delegate(&ctx, name, s, vec![]));
        engine.register_fn(name, move |ctx: NativeCallContext, s: &mut Symbolic, a: Dynamic| delegate(&ctx, name, s, vec![a]));
        engine.register_fn(name, move |ctx: NativeCallContext, s: &mut Symbolic, a: Dynamic, b: Dynamic| delegate(&ctx, name, s, vec![a, b]));
        engine.register_fn(name, move |ctx: NativeCallContext, s: &mut Symbolic, a: Dynamic, b: Dynamic, c: Dynamic| delegate(&ctx, name, s, vec![a, b, c]));
        engine.register_fn(name, move |ctx: NativeCallContext, s: &mut Symbolic, a: Dynamic, b: Dynamic, c: Dynamic, d: Dynamic| delegate(&ctx, name, s, vec![a, b, c, d]));
    }
    // A symbolic where a canvas goes.
    engine.register_fn("add", |ctx: NativeCallContext, c: &mut Canvas, s: Symbolic| -> RhaiResult<Dynamic> {
        ctx.call_native_fn::<Dynamic>("add", (c.clone(), s.canvas))
    });
    engine.register_fn("add", |ctx: NativeCallContext, page: &mut super::Page, s: Symbolic| -> RhaiResult<Dynamic> {
        ctx.call_native_fn::<Dynamic>("add", (page.clone(), s.canvas))
    });
    engine.register_fn("free_regions_in_canvas", |ctx: NativeCallContext, s: Symbolic| -> RhaiResult<Dynamic> {
        ctx.call_native_fn::<Dynamic>("free_regions_in_canvas", (s.canvas,))
    });
    engine.register_fn("free_regions_in_canvas", |ctx: NativeCallContext, s: Symbolic, options: Map| -> RhaiResult<Dynamic> {
        ctx.call_native_fn::<Dynamic>("free_regions_in_canvas", (s.canvas, options))
    });
}

#[cfg(test)]
mod tests {
    use super::super::{Entry, JournalEngine, Resources};
    use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};
    use std::rc::Rc;

    const PRIESTS: &str = r#"{ "glyphset": "thirteen_black_priests", "glyphs": ["0", "1", "2", "3"], "style": "plain_hand", "size": 40 }"#;
    const OTHER: &str = r#"{ "glyphset": "plain", "glyphs": ["A", "B", "C"], "style": "plain_hand", "size": 40 }"#;

    fn engine(edits: &str) -> JournalEngine {
        let mut resources = Resources::library();
        resources.symbol_set_data = resources.symbol_set_data.with_overlay([("priests".to_string(), PRIESTS.to_string()), ("letters3".to_string(), OTHER.to_string())]);
        let mut engine = JournalEngine::with_resources(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).unwrap()), resources);
        engine.set_edits(super::super::Edits::parse(edits).unwrap());
        engine
    }

    fn build(engine: &mut JournalEngine, script: &str) -> Result<Entry, String> {
        engine.build_entry_str(script).map_err(|e| e.to_string())
    }

    #[test]
    fn a_symbolic_is_a_canvas_wherever_a_canvas_goes() {
        let mut e = engine(r#"{ "version": 1 }"#);
        build(&mut e, r#"
            let p = symbol_set("priests");
            let s = write_at(p.text([1]), vec2(100, 100), #{});
            assert(type_of(s) == "Symbolic", "its own type");
            assert(s.symbols == [1] && s.symbol_set.name == "priests", "what it remembers");
            let b = s.bounds();
            assert((b.center.x - 100.0).abs() < 8.0, "bounds");
            s.translate_by(vec2(50, 0));
            assert((s.bounds().center.x - 150.0).abs() < 8.0, "moved");
            let c = new_canvas().add(s).add([s]);
            assert(c.bounds().width > 0.0, "added to a canvas, singly and in an array");
            let page = new_page().add(s);
            let copy = s.copy();
            copy.translate_by(vec2(10, 0));
            assert((s.bounds().center.x - 150.0).abs() < 8.0, "the copy is its own");
            let tree = new_avoidance_tree().add(s);
            assert(!tree.is_empty, "avoided");
            s.name = "mark";
            assert(s.name == "mark", "named");
            s.add(new_line(vec2(0, 0), vec2(5, 5)), default_style());
            assert(free_regions_in_canvas(s).len() >= 0, "regions");
        "#).unwrap();
    }

    #[test]
    fn symbols_and_set_can_be_changed_and_it_draws_again() {
        let mut e = engine(r#"{ "version": 1 }"#);
        build(&mut e, r#"
            let p = symbol_set("priests");
            let s = write_at(p.text([0]), vec2(100, 100), #{});
            s.translate_by(vec2(200, 0));
            let before = s.bounds();
            s.symbols = [1, 2, 3];
            assert(s.symbols == [1, 2, 3], "symbols");
            assert(s.bounds().width > before.width, "three symbols are wider than one");
            assert((s.bounds().center.x - 300.0).abs() < 30.0, "the move was made again: " + s.bounds().center);
            // Strokes the script added stay through a redraw.
            s.add(new_line(vec2(0, 0), vec2(5, 5)), default_style());
            s.symbols = [0];
            assert(s.bounds().min.x < 10.0, "the added stroke is still there");
            s.symbol_set = "letters3";
            assert(s.symbol_set.name == "letters3", "set by name");
            s.symbol_set = symbol_set("priests");
            let bad = s;
        "#).unwrap();
        let err = build(&mut e, r#"let s = write_at(symbol_set("priests").text([0]), vec2(0, 0), #{}); s.symbols = [4];"#).err().unwrap_or_default();
        assert!(err.contains("outside the symbol set's 0-3"), "{}", err);
        let err = build(&mut e, r#"let s = write_at(symbol_set("priests").text([3]), vec2(0, 0), #{}); s.symbol_set = symbol_set("letters3");"#).err().unwrap_or_default();
        assert!(err.contains("outside symbol set 'letters3'"), "{}", err);
    }

    #[test]
    fn scaling_and_sizing_are_made_again_when_it_draws_again() {
        let mut e = engine(r#"{ "version": 1 }"#);
        build(&mut e, r#"
            fn near(a, b) { (a - b).abs() < 0.01 }
            let p = symbol_set("priests");
            // Scaled by a vec: across and down apart, made again.
            let s = write_at(p.text([0]), vec2(0, 0), #{}).scale_by(vec2(3, 1));
            assert(type_of(s) == "Symbolic", "scale_by gives the symbolic back");
            s.symbols = [1];
            let wide = write_at(p.text([1]), vec2(0, 0), #{}).bounds();
            assert(s.bounds().width > wide.width * 2.0, "stretched again: " + s.bounds().width + " vs " + wide.width);
            // Sized to a page, or a canvas: the new drawing fills the same box.
            let page = new_page(#{ size: vec2(300, 200) });
            let t = write_at(p.text([0]), vec2(500, 500), #{}).size_to(page, #{ padding: 10 });
            assert(type_of(t) == "Symbolic", "size_to gives the symbolic back");
            t.symbols = [1, 2, 3];
            let b = t.bounds();
            assert(near(b.min.x, 10) && near(b.min.y, 10) && near(b.max.x, 290) && near(b.max.y, 190), "refitted: " + b.min + " " + b.max);
            let frame = new_canvas().add(new_rectangle(vec2(50, 50), 80, 40), default_style());
            let u = write_at(p.text([2]), vec2(0, 0), #{}).size_to(frame, #{ keep_proportions: true });
            u.symbols = [3, 0];
            assert(u.bounds().height <= frame.bounds().height + 0.01 && u.bounds().width <= frame.bounds().width + 0.01, "kept within the frame");
            // A symbolic as the target, as a canvas.
            let v = new_canvas().add(new_circle(vec2(0, 0), 5), default_style()).size_to(t);
            assert(near(v.bounds().min.x, t.bounds().min.x) && near(v.bounds().max.y, t.bounds().max.y), "sized to a symbolic");
        "#).unwrap();
    }

    #[test]
    fn drawing_again_leaves_the_rest_of_the_script_alone() {
        let mut e = engine(r#"{ "version": 1 }"#);
        build(&mut e, r#"
            seed(7);
            let p = symbol_set("priests");
            let s = write_at(p.text([0, 1]), vec2(0, 0), #{});
            let first = random();
            seed(7);
            let t = write_at(p.text([0, 1]), vec2(0, 0), #{});
            t.symbols = [2, 3, 2];
            assert(random() == first, "a redraw leaves the generator where it was");
            let again = write_at(p.text([0, 1]), vec2(0, 0), #{});
            assert(s.bounds().width > 0.0 && again.bounds().width > 0.0, "ok");
        "#).unwrap();
    }

    #[test]
    fn the_editor_re_symbols_a_committed_symbolic() {
        let script = r#"
            let p = symbol_set("priests");
            let s = write_at(p.text([0]), vec2(100, 100), #{}).commit_to_editor("corner");
            journal_entry.add(new_page().add(s));
        "#;
        let mut plain = engine(r#"{ "version": 1 }"#);
        let before = build(&mut plain, script).unwrap();
        let mut edited = engine(r#"{ "version": 1, "canvases": { "corner": { "symbols": [1, 2], "symbol_set": "priests", "turn": 20 } } }"#);
        let after = build(&mut edited, script).unwrap();
        let width = |entry: &Entry| entry.pages[0].canvases()[0].strokes.iter().flat_map(|s| s.path.points_within()).fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
        let (a, b) = (width(&before), width(&after));
        assert!(b.1 - b.0 > (a.1 - a.0) * 1.3, "two symbols are wider than one");
        let report = super::super::commit_report(&after, &edited.committed());
        let symbolic = &report[0].symbolics[0];
        assert!(symbolic.corners.is_some(), "where it is");
        assert_eq!((symbolic.symbols.clone(), symbolic.symbol_set.as_str(), symbolic.base), (vec![1, 2], "priests", 4));
        assert_eq!(symbolic.script_symbols, vec![0]);
        let mut wrong = engine(r#"{ "version": 1, "canvases": { "corner": { "symbols": [9] } } }"#);
        let err = build(&mut wrong, script).err().unwrap_or_default();
        assert!(err.contains("is edited to symbol 9, but symbol set 'priests' has 4"), "{}", err);
        let mut nothing = engine(r#"{ "version": 1, "canvases": { "corner": { "symbol_set": "nowhere" } } }"#);
        assert!(build(&mut nothing, script).err().unwrap_or_default().contains("is edited to symbol set 'nowhere'"));
    }

    /// The x extent of the first canvas on the first page.
    fn span(entry: &Entry) -> (f32, f32) {
        entry.pages[0].canvases()[0].strokes.iter().flat_map(|s| s.path.points_within()).fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)))
    }

    // Three marks on a plate with a frame, the plate moved and committed.
    const PLATE: &str = r#"
        let p = symbol_set("priests");
        let pen = default_style();
        let plate = new_canvas().add(new_rectangle(vec2(200, 100), 400, 200), pen);
        plate.add(write_at(p.text([0]), vec2(100, 100), #{}));
        plate.add([write_at(p.text([1]), vec2(200, 100), #{}), write_at(p.text([2]), vec2(300, 100), #{}).translate_by(vec2(0, 20))]);
        plate.translate_by(vec2(1000, 0)).commit_to_editor("plate");
        journal_entry.add(new_page().add(plate));
    "#;

    #[test]
    fn a_canvas_holds_the_symbolics_added_to_it() {
        let mut plain = engine(r#"{ "version": 1 }"#);
        let before = build(&mut plain, PLATE).unwrap();
        let report = super::super::commit_report(&before, &plain.committed());
        let held = &report[0].symbolics;
        assert_eq!(held.iter().map(|s| s.symbols.clone()).collect::<Vec<_>>(), vec![vec![0], vec![1], vec![2]]);
        // Each where it is on the plate, moved with it.
        let centre = |s: &super::SymbolicReport| { let c = s.corners.unwrap(); ((c[0][0] + c[2][0]) / 2.0, (c[0][1] + c[2][1]) / 2.0) };
        for (s, at) in held.iter().zip([(1100.0, 100.0), (1200.0, 100.0), (1300.0, 120.0)]) {
            let c = centre(s);
            assert!((c.0 - at.0).abs() < 12.0 && (c.1 - at.1).abs() < 12.0, "{:?} near {:?}", c, at);
        }
        // The middle one edited: drawn again where it was, the rest as they were.
        let mut edited = engine(r#"{ "version": 1, "canvases": { "plate": { "symbolics": { "1": { "symbols": [3, 3], "symbol_set": "letters3" }, "2": {} } } } }"#);
        let err = build(&mut edited, PLATE).err().unwrap_or_default();
        assert!(err.contains("'plate': symbolic 1 is edited to symbol 3, but symbol set 'letters3' has 3"), "{}", err);
        let mut edited = engine(r#"{ "version": 1, "canvases": { "plate": { "symbolics": { "1": { "symbols": [2, 2], "symbol_set": "letters3" } } } } }"#);
        let after = build(&mut edited, PLATE).unwrap();
        let report = super::super::commit_report(&after, &edited.committed());
        let held = &report[0].symbolics;
        assert_eq!((held[1].symbols.clone(), held[1].symbol_set.as_str(), held[1].script_symbols.clone(), held[1].script_symbol_set.as_str()), (vec![2, 2], "letters3", vec![1], "priests"));
        let c = centre(&held[1]);
        assert!((c.0 - 1200.0).abs() < 12.0 && (c.1 - 100.0).abs() < 12.0, "drawn again where it was: {:?}", c);
        assert_eq!(centre(&held[0]), centre(&report[0].symbolics[0]));
        let (a, b) = (&before.pages[0].canvases()[0].strokes, &after.pages[0].canvases()[0].strokes);
        assert_ne!(a.len(), b.len(), "two letters aren't one priest");
        assert_eq!(span(&before), span(&after), "the frame, and the box, are as they were");
        assert!(edited.warnings().is_empty());
    }

    #[test]
    fn an_edit_for_a_symbolic_it_doesnt_hold_is_left_out() {
        let mut e = engine(r#"{ "version": 1, "canvases": { "plate": { "symbolics": { "7": { "symbols": [1] } } } } }"#);
        build(&mut e, PLATE).unwrap();
        assert!(e.warnings()[0].contains("'plate' has an edit for a symbolic '7', which it doesn't hold (it holds 3)"), "{:?}", e.warnings());
    }

    #[test]
    fn older_edits_are_symbolic_0s() {
        let edits = super::super::Edits::parse(r#"{ "version": 1, "canvases": { "a": { "symbols": [1], "symbol_set": "moons" } } }"#).unwrap();
        let first = &edits.canvases["a"].symbolics["0"];
        assert_eq!((first.symbols.clone(), first.symbol_set.as_deref()), (Some(vec![1]), Some("moons")));
    }

    #[test]
    fn held_symbolics_are_edited_again_through_canvases_and_commits() {
        // A mark committed and turned, on a card committed and moved, on a
        // sheet scaled and committed: the sheet's edit draws it again, and
        // every move and edit since is made again.
        let script = r#"
            let p = symbol_set("priests");
            let mark = write_at(p.text([0]), vec2(100, 100), #{}).commit_to_editor("mark");
            let card = new_canvas().add(new_rectangle(vec2(100, 100), 150, 150), default_style()).add(mark).commit_to_editor("card");
            let sheet = new_canvas().add(card).add(card.copy().translate_by(vec2(0, 300))).scale_by(vec2(2, 2)).commit_to_editor("sheet");
            journal_entry.add(new_page().add(sheet));
        "#;
        let placing = r#""mark": { "move": [10, 0] }, "card": { "move": [0, 40], "ink": { "color": "red" } }"#;
        let mut plain = engine(&format!(r#"{{ "version": 1, "canvases": {{ {} }} }}"#, placing));
        let before = build(&mut plain, script).unwrap();
        let report = super::super::commit_report(&before, &plain.committed());
        let sheet = report.iter().find(|r| r.name == "sheet").unwrap();
        assert_eq!(sheet.symbolics.len(), 2, "the card's, and its copy's");
        let was = sheet.symbolics[1].corners.unwrap();
        // The same symbol, by an edit: drawn again, it lands where it was.
        let mut same = engine(&format!(r#"{{ "version": 1, "canvases": {{ {}, "sheet": {{ "symbolics": {{ "1": {{ "symbols": [0], "symbol_set": "priests" }} }} }} }} }}"#, placing));
        let after = build(&mut same, script).unwrap();
        let report = super::super::commit_report(&after, &same.committed());
        let now = report.iter().find(|r| r.name == "sheet").unwrap().symbolics[1].corners.unwrap();
        for (a, b) in was.iter().zip(&now) {
            assert!((a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01, "{:?} vs {:?}", was, now);
        }
        let (a, b) = (&before.pages[0].canvases()[0], &after.pages[0].canvases()[0]);
        assert_eq!(a.strokes.len(), b.strokes.len());
        let svg = crate::journal::publish::page_svg(&after.pages[0], "page_0").unwrap().0;
        assert_eq!(svg, crate::journal::publish::page_svg(&before.pages[0], "page_0").unwrap().0, "its ink and marks too");
    }

    #[test]
    fn edits_follow_their_symbolics_by_id() {
        let mut plain = engine(r#"{ "version": 1 }"#);
        let before = build(&mut plain, PLATE).unwrap();
        let held = super::super::commit_report(&before, &plain.committed())[0].symbolics.clone();
        let ids: Vec<&str> = held.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.iter().all(|id| id.len() == 36) && ids[0] != ids[1] && ids[1] != ids[2], "{:?}", ids);
        // The script draws another before them, and one of them twice: the
        // edit is still the same one's.
        let more = PLATE.replace("plate.add(write_at(p.text([0])", "plate.add(write_at(p.text([3]), vec2(50, 50), #{}));\nplate.add(write_at(p.text([0])")
            .replace("plate.translate_by", "plate.add(write_at(p.text([1]), vec2(200, 100), #{}));\nplate.translate_by");
        let mut edited = engine(&format!(r#"{{ "version": 1, "canvases": {{ "plate": {{ "symbolics": {{ "{}": {{ "symbols": [2, 2] }}, "{}.2": {{ "symbols": [0] }} }} }} }} }}"#, ids[1], ids[1]));
        let after = build(&mut edited, &more).unwrap();
        let held = super::super::commit_report(&after, &edited.committed())[0].symbolics.clone();
        assert_eq!(held.iter().map(|s| s.symbols.clone()).collect::<Vec<_>>(), vec![vec![3], vec![0], vec![2, 2], vec![2], vec![0]]);
        assert_eq!((held[2].id.as_str(), held[4].id.clone()), (ids[1], format!("{}.2", ids[1])), "the same call's are numbered");
        assert!(edited.warnings().is_empty(), "{:?}", edited.warnings());
    }
}
