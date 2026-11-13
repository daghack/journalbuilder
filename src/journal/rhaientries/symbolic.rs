//! Symbolics: what the text calls give when they're writing symbols of a
//! symbol set (`write_line(set.text(symbols), ...)`). A symbolic is a canvas
//! in every respect (everything that takes a canvas takes one, and moving
//! it, adding to it and committing it work as they do for a canvas), but it
//! remembers which symbols of which set it was drawn from, so the visual
//! editor can change them: `commit_to_editor` applies an edit's `symbols`
//! and `symbol_set` by drawing it again.
//!
//! ```rhai
//! let mark = write_at(marks.text([2]), vec2(100, 100), #{});
//! mark.symbols;                       // [2]
//! mark.symbols = [4];                 // drawn again, as symbol 4
//! mark.symbol_set = symbol_set("moons");
//! page.add(mark.commit_to_editor("corner"));
//! ```
//!
//! Drawing again runs the same call with the symbols changed, from the
//! generator as it stood before the first time, then the moves made since
//! (`translate_by`, `scale_by`, `rotate_around`) are made again, so jitters
//! land where they did and a redraw never changes what the rest of the
//! script draws. Strokes the script added to it stay as they are.

use std::rc::Rc;

use parry2d::math::Vec2 as PVec;
use rand::rngs::StdRng;
use rhai::{Array, Dynamic, Engine, Map, NativeCallContext};
use serde::Serialize;

use super::canvas::CanvasData;
use super::edits::{commit_as, SharedEditor};
use super::markdown::FlowText;
use super::random::SharedRng;
use super::symbol_sets::{symbols_of, SymbolSet, SymbolSetLibrary, SymbolText};
use super::{Canvas, RhaiResult, Shared, Vec2};

/// A symbolic, shared as canvases are: copies of the variable are the same.
pub type Symbolic = Shared<SymbolicData>;

/// How a symbolic's canvas is drawn again for other symbols.
type Recipe = Rc<dyn Fn(&Engine, &SymbolText) -> RhaiResult<CanvasData>>;

#[derive(Clone)]
pub struct SymbolicData {
    /// What every canvas function sees.
    canvas: Canvas,
    text: SymbolText,
    recipe: Recipe,
    /// The generator as it stood before it was first drawn.
    start: StdRng,
    rng: SharedRng,
    /// Moves made since it was drawn, to make again after drawing again.
    moves: Vec<Move>,
    /// How many of the canvas's strokes the recipe drew: those after were
    /// added by the script.
    drawn: usize,
}

#[derive(Clone, Copy, Debug)]
enum Move {
    Translate(PVec),
    Scale(f32),
    Rotate(PVec, f32),
}

/// What the visual editor is told of a committed symbolic: its symbols and
/// set as drawn (the edit applied), and as the script made it.
#[derive(Clone, Debug, Serialize)]
pub struct SymbolicReport {
    pub symbols: Vec<u32>,
    pub symbol_set: String,
    pub base: u32,
    pub script_symbols: Vec<u32>,
    pub script_symbol_set: String,
}

/// `produce`'s canvas for `text`, and the symbolic that can draw it again
/// for other symbols; `extra` is whatever else the first call gave.
pub(super) fn make_with<T>(
    engine: &Engine,
    rng: &SharedRng,
    text: SymbolText,
    produce: impl Fn(&Engine, &FlowText) -> RhaiResult<(Canvas, T)> + 'static,
) -> RhaiResult<(Symbolic, T)> {
    let start = rng.borrow().clone();
    let (canvas, extra) = produce(engine, &FlowText::Symbols(text.clone()))?;
    let drawn = canvas.borrow().strokes.len();
    let recipe: Recipe = Rc::new(move |engine, text| produce(engine, &FlowText::Symbols(text.clone())).map(|(canvas, _)| canvas.get()));
    Ok((Shared::new(SymbolicData { canvas, text, recipe, start, rng: rng.clone(), moves: Vec::new(), drawn }), extra))
}

/// As `make_with`, for a call that gives only a canvas.
pub(super) fn make(
    engine: &Engine,
    rng: &SharedRng,
    text: SymbolText,
    produce: impl Fn(&Engine, &FlowText) -> RhaiResult<Canvas> + 'static,
) -> RhaiResult<Symbolic> {
    make_with(engine, rng, text, move |engine, msg| produce(engine, msg).map(|canvas| (canvas, ()))).map(|(symbolic, ())| symbolic)
}

/// The canvas a script value is: a canvas, or a symbolic's.
pub(super) fn canvas_of(value: &Dynamic) -> Option<Canvas> {
    if let Some(canvas) = value.clone().try_cast::<Canvas>() {
        return Some(canvas);
    }
    value.clone().try_cast::<Symbolic>().map(|s| s.borrow().canvas.clone())
}

impl SymbolicData {
    /// Draw it again as `text`: from the generator as it stood, the moves
    /// made again, in place of what the recipe drew before.
    pub(super) fn redraw(&mut self, engine: &Engine, text: SymbolText) -> RhaiResult<()> {
        let saved = self.rng.borrow().clone();
        *self.rng.borrow_mut() = self.start.clone();
        let result = (self.recipe)(engine, &text);
        *self.rng.borrow_mut() = saved;
        let mut fresh = result?;
        for step in &self.moves {
            let step = *step;
            fresh.transform_with(|path| match step {
                Move::Translate(v) => path.translate_by(v),
                Move::Scale(s) => path.scale_by(s),
                Move::Rotate(point, rad) => path.rotate_around(point, rad),
            });
        }
        let kept = self.drawn;
        let count = fresh.strokes.len();
        self.canvas.update(|data| {
            let after = data.strokes.split_off(kept.min(data.strokes.len()));
            data.strokes = fresh.strokes;
            data.strokes.extend(after);
        });
        self.drawn = count;
        self.text = text;
        Ok(())
    }
}

/// `symbolic`'s set and symbols as a report.
fn report(set: &SymbolSet, symbols: &[u32], script_set: &SymbolSet, script_symbols: &[u32]) -> SymbolicReport {
    SymbolicReport {
        symbols: symbols.to_vec(),
        symbol_set: set.inner().name.clone(),
        base: set.base(),
        script_symbols: script_symbols.to_vec(),
        script_symbol_set: script_set.inner().name.clone(),
    }
}

/// `args` with every symbolic in them (and in arrays among them) as its canvas.
fn plain_args(args: Vec<Dynamic>) -> Vec<Dynamic> {
    args.into_iter().map(|arg| {
        if let Some(canvas) = arg.clone().try_cast::<Symbolic>().map(|s| s.borrow().canvas.clone()) {
            Dynamic::from(canvas)
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
    let canvas = symbolic.borrow().canvas.clone();
    let mut all = vec![Dynamic::from(canvas.clone())];
    all.extend(plain_args(args.clone()));
    let result = ctx.call_native_fn::<Dynamic>(name, all)?;
    let point = |v: &Dynamic| v.clone().try_cast::<Vec2>().map(PVec::from);
    let number = |v: &Dynamic| v.as_float().or_else(|_| v.as_int().map(|n| n as f64)).ok().map(|n| n as f32);
    match (name, args.as_slice()) {
        ("translate_by", [v]) => if let Some(v) = point(v) { symbolic.borrow_mut().moves.push(Move::Translate(v)) },
        ("scale_by", [s]) => if let Some(s) = number(s) { symbolic.borrow_mut().moves.push(Move::Scale(s)) },
        ("rotate_around", [p, r]) => if let (Some(p), Some(r)) = (point(p), number(r)) { symbolic.borrow_mut().moves.push(Move::Rotate(p, r)) },
        _ => {}
    }
    match result.clone().try_cast::<Canvas>() {
        Some(out) if out.same(&canvas) => Ok(Dynamic::from(symbolic.clone())),
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

/// Draw `symbolic` again as `set`'s `symbols`.
pub(super) fn resymbol(engine: &Engine, symbolic: &mut Symbolic, set: SymbolSet, symbols: Vec<u32>) -> RhaiResult<()> {
    if let Some(s) = symbols.iter().find(|s| **s >= set.base()) {
        return Err(format!("symbol {} is outside symbol set '{}' (0 to {})", s, set.inner().name, set.base() - 1).into());
    }
    let mut data = symbolic.borrow().clone();
    data.redraw(engine, SymbolText { set, symbols })?;
    let (text, drawn, canvas) = (data.text.clone(), data.drawn, data.canvas.clone());
    let _ = canvas;
    let mut inner = symbolic.borrow_mut();
    inner.text = text;
    inner.drawn = drawn;
    Ok(())
}

pub(super) fn register(engine: &mut Engine, editor: SharedEditor, library: Rc<SymbolSetLibrary>, rng: SharedRng) {
    let _ = rng;
    engine.register_type_with_name::<Symbolic>("Symbolic");
    engine.register_get("symbols", |s: &mut Symbolic| -> Array { s.borrow().text.symbols.iter().map(|n| Dynamic::from(*n as i64)).collect() });
    engine.register_get("symbol_set", |s: &mut Symbolic| s.borrow().text.set.clone());
    engine.register_fn("set$symbols", |ctx: NativeCallContext, s: &mut Symbolic, symbols: Dynamic| -> RhaiResult<()> {
        let set = s.borrow().text.set.clone();
        let symbols = symbols_of("symbolic.symbols", &symbols, set.base())?;
        resymbol(ctx.engine(), s, set, symbols).map_err(|e| format!("symbolic.symbols: {}", e).into())
    });
    let lib = library.clone();
    engine.register_fn("set$symbol_set", move |ctx: NativeCallContext, s: &mut Symbolic, set: Dynamic| -> RhaiResult<()> {
        let set = set_of(&ctx, &lib, &set)?;
        let symbols = s.borrow().text.symbols.clone();
        resymbol(ctx.engine(), s, set, symbols).map_err(|e| format!("symbolic.symbol_set: {}", e).into())
    });
    engine.register_get_set("name",
        |s: &mut Symbolic| s.borrow().canvas.borrow().name.clone().map_or(Dynamic::UNIT, Dynamic::from),
        |s: &mut Symbolic, name: Dynamic| -> RhaiResult<()> {
            let canvas = s.borrow().canvas.clone();
            canvas.borrow_mut().name = Some(super::check_name("canvas.name", name)?);
            Ok(())
        });
    engine.register_fn("copy", |s: &mut Symbolic| -> Symbolic {
        let mut data = s.get();
        data.canvas = s.borrow().canvas.clone().copy();
        Shared::new(data)
    });
    // Every canvas function, on the symbolic's canvas.
    for name in ["add", "translate_by", "scale_by", "rotate_around", "scatter_notes", "bounds"] {
        engine.register_fn(name, move |ctx: NativeCallContext, s: &mut Symbolic| delegate(&ctx, name, s, vec![]));
        engine.register_fn(name, move |ctx: NativeCallContext, s: &mut Symbolic, a: Dynamic| delegate(&ctx, name, s, vec![a]));
        engine.register_fn(name, move |ctx: NativeCallContext, s: &mut Symbolic, a: Dynamic, b: Dynamic| delegate(&ctx, name, s, vec![a, b]));
        engine.register_fn(name, move |ctx: NativeCallContext, s: &mut Symbolic, a: Dynamic, b: Dynamic, c: Dynamic| delegate(&ctx, name, s, vec![a, b, c]));
        engine.register_fn(name, move |ctx: NativeCallContext, s: &mut Symbolic, a: Dynamic, b: Dynamic, c: Dynamic, d: Dynamic| delegate(&ctx, name, s, vec![a, b, c, d]));
    }
    // A symbolic where a canvas goes.
    engine.register_fn("add", |ctx: NativeCallContext, c: &mut Canvas, s: Symbolic| -> RhaiResult<Dynamic> {
        let other = s.borrow().canvas.clone();
        ctx.call_native_fn::<Dynamic>("add", (c.clone(), other))
    });
    engine.register_fn("add", |ctx: NativeCallContext, page: &mut super::Page, s: Symbolic| -> RhaiResult<Dynamic> {
        let other = s.borrow().canvas.clone();
        ctx.call_native_fn::<Dynamic>("add", (page.clone(), other))
    });
    engine.register_fn("free_regions_in_canvas", |ctx: NativeCallContext, s: Symbolic| -> RhaiResult<Dynamic> {
        let other = s.borrow().canvas.clone();
        ctx.call_native_fn::<Dynamic>("free_regions_in_canvas", (other,))
    });
    engine.register_fn("free_regions_in_canvas", |ctx: NativeCallContext, s: Symbolic, options: Map| -> RhaiResult<Dynamic> {
        let other = s.borrow().canvas.clone();
        ctx.call_native_fn::<Dynamic>("free_regions_in_canvas", (other, options))
    });

    // commit_to_editor: the edit's symbols and set first, then as a canvas.
    let lib = library;
    engine.register_fn("commit_to_editor", move |ctx: NativeCallContext, s: &mut Symbolic, name: Dynamic| -> RhaiResult<Symbolic> {
        const CALLER: &str = "commit_to_editor";
        let name = super::check_name(CALLER, name)?;
        let (script_set, script_symbols) = { let d = s.borrow(); (d.text.set.clone(), d.text.symbols.clone()) };
        let edit = editor.borrow().saved.canvases.get(&name).cloned().unwrap_or_default();
        let set = match &edit.symbol_set {
            Some(set_name) => lib.make(ctx.engine(), set_name, &Map::new()).map_err(|e| format!("{}: '{}' is edited to symbol set '{}': {}", CALLER, name, set_name, e))?,
            None => script_set.clone(),
        };
        let symbols = edit.symbols.clone().unwrap_or_else(|| script_symbols.clone());
        if let Some(bad) = symbols.iter().find(|n| **n >= set.base()) {
            return Err(format!("{}: '{}' is edited to symbol {}, but symbol set '{}' has {}", CALLER, name, bad, set.inner().name, set.base()).into());
        }
        if edit.symbol_set.is_some() || edit.symbols.is_some() {
            resymbol(ctx.engine(), s, set.clone(), symbols.clone())?;
        }
        let canvas = s.borrow().canvas.clone();
        commit_as(&ctx, &editor, &mut canvas.clone(), name, Some(report(&set, &symbols, &script_set, &script_symbols)))?;
        Ok(s.clone())
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
        let symbolic = report[0].symbolic.as_ref().expect("reported as a symbolic");
        assert_eq!((symbolic.symbols.clone(), symbolic.symbol_set.as_str(), symbolic.base), (vec![1, 2], "priests", 4));
        assert_eq!(symbolic.script_symbols, vec![0]);
        let mut wrong = engine(r#"{ "version": 1, "canvases": { "corner": { "symbols": [9] } } }"#);
        let err = build(&mut wrong, script).err().unwrap_or_default();
        assert!(err.contains("is edited to symbol 9, but symbol set 'priests' has 4"), "{}", err);
        let mut nothing = engine(r#"{ "version": 1, "canvases": { "corner": { "symbol_set": "nowhere" } } }"#);
        assert!(build(&mut nothing, script).err().unwrap_or_default().contains("is edited to symbol set 'nowhere'"));
    }
}
