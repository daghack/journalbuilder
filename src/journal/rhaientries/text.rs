//! Text in boxes for rhai scripts: text boxes, and messages scattered
//! over a canvas.
//!
//! ```rhai
//! let b = canvas.bounds();          // #{ min, max, center, width, height }
//!
//! // Flow glyphs into a box, line by line, keeping clear of a canvas.
//! let tb = new_textbox(#{
//!     center: b.center, width: b.width * 1.2, height: b.height * 1.2,
//!     symbol_scale: 52.0,
//!     layout: "flat",               // or "words": wrap at blanks (spaces)
//!     jagged_edge: 0.0,             // random line indent, in symbol scales
//!     avoid: canvas,                // optional: see avoid.rs
//!     clearance: 6.0,               // how far from it; default: symbol_scale / 8
//!     spacing: 0.1,                 // extra space between glyphs, in symbol scales (default 0)
//!     split_words: true,            // "words" only: break words round what's avoided, rather
//!                                   // than moving them past it whole; or the fewest glyphs
//!                                   // a piece may have (true is 2)
//!     split_padding: 0.3,           // pieces keep this much further from it, in symbol scales
//! });
//!
//! // Or fill any closed shape, on lines at an angle (radians; default 0,
//! // left to right, each line below the last). Each subpath of the path is
//! // an outline, closed back to its start; one inside another is a hole.
//! // A rectangle shape at angle 0 fills just as center/width/height do.
//! let round = new_textbox(#{ shape: new_circle(b.center, 300.0), angle: 0.3, symbol_scale: 20.0 });
//!
//! let res = tb.fill(font.glyphs(msg));   // #{ placed: [..], unplaced: [..] }
//! let blocks = tb.place_all(glyphs);     // one array per filled box
//!
//! // Scatter messages over a canvas in random boxes, clear of its strokes
//! // and of each other. Returns a canvas with just the messages.
//! let notes = page_canvas.scatter_notes(["first note", "second note"], font, style, #{
//!     symbol_scale: 23.0,
//!     width_range: [200, 500], height_range: [300, 600],   // box sizes to try
//!     rotation_jitter: 0.196,     // radians either way (default pi/16)
//!     scale_jitter: 0.15,         // fraction either way (default 0.15)
//!     jagged_edge: 0.1,           // default 0.1
//!     max_attempts: 100,          // boxes per message before backing off
//!     spacing: 0.0,               // extra space between glyphs, in symbol scales
//!     avoid: other,               // more to keep clear of (see avoid.rs)
//!     clearance: 3.0,             // from the canvas, `avoid` and other messages; default symbol_scale / 8
//!     layout: "words",            // words whole, each note in one clear patch; default "flat", glyph by glyph
//! });
//!
//! ```
//!
//! Placement scales each glyph by `symbol_scale` and turns it to the line,
//! on top of any scale and rotation it already has. Jagged edges come from
//! the engine's seeded generator.

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2 as PVec;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};
use std::rc::Rc;

use super::glyphs::{add_glyph, glyph_array, Glyph};
use super::random::SharedRng;
use super::{Canvas, CanvasData, Path, StrokeStyle, Vec2};
use crate::journal::canvas::artistic::{jitter_symbols, scatter_notes, ScatterOptions};
use crate::journal::fonts::Font;
use crate::journal::fonts::FontBook;
use crate::journal::canvas::textbox::{rectangle, TextBox as CoreTextBox, TextBoxLineMode, TextBoxOptions};
use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::symbol::{Symbol, WordSplit};
use super::avoid::{Avoid, AvoidOption};
use super::options::{check_keys, number, positive, range};

use super::RhaiResult;

/// A box (or any closed shape) that glyphs flow into, line by line.
#[derive(Clone)]
pub struct TextBox {
    shape: Rc<Vec<Vec<PVec>>>,
    angle: f32,
    symbol_scale: f32,
    jagged_edge: f32,
    layout: TextBoxLineMode,
    avoid: Option<Avoid>,
    padding: f32,
    spacing: f32,
}

const TEXTBOX_KEYS: [&str; 13] = ["center", "width", "height", "shape", "angle", "symbol_scale", "layout", "jagged_edge", "avoid", "clearance", "spacing", "split_words", "split_padding"];

/// The `spacing` option: extra space between glyphs, as a fraction of the
/// symbol scale. Negative tightens, down to glyphs touching their
/// neighbours' centres.
pub(super) fn spacing(caller: &str, options: &Map) -> RhaiResult<f32> {
    match number(caller, options, "spacing")? {
        None => Ok(0.0),
        Some(s) if s.is_finite() && s > -0.5 => Ok(s),
        Some(s) => Err(format!("{}: 'spacing' must be a number above -0.5, got {}", caller, s).into()),
    }
}

/// The `layout` option: "flat" (the default) places glyph by glyph, and
/// "words" keeps words together, wrapping at blanks, unless `split_words`
/// lets them break round what they avoid (see `word_split`).
pub(super) fn line_mode(caller: &str, options: &Map) -> RhaiResult<TextBoxLineMode> {
    let split = word_split(caller, options)?;
    match options.get("layout").map(|l| l.to_string()).as_deref() {
        None | Some("flat") if split.is_some() => Err(format!("{}: 'split_words' needs layout: \"words\" (flat text is placed glyph by glyph already)", caller).into()),
        None | Some("flat") => Ok(TextBoxLineMode::Flat),
        Some("words") => Ok(TextBoxLineMode::WordsByBlankSymbol(split)),
        Some(other) => Err(format!("{}: unknown layout '{}'; expected \"flat\" or \"words\"", caller, other).into()),
    }
}

/// The `split_words` and `split_padding` options: whether words may break
/// round what they avoid, rather than moving past it whole (`true`, or
/// the fewest glyphs a piece may have: `true` is 2), and how much further
/// the pieces keep from it than the clearance, in symbol scales (default
/// 0).
pub(super) fn word_split(caller: &str, options: &Map) -> RhaiResult<Option<WordSplit>> {
    let min_piece = match options.get("split_words") {
        None => None,
        Some(v) if v.is_bool() => v.as_bool().expect("a bool").then_some(2),
        Some(v) => match v.as_int() {
            Ok(n) if n >= 1 => Some(n as usize),
            _ => return Err(format!("{}: 'split_words' should be true, false, or the fewest glyphs a piece may have (at least 1), got {}", caller, v).into()),
        },
    };
    let padding = number(caller, options, "split_padding")?;
    match (min_piece, padding) {
        (None, Some(_)) => Err(format!("{}: 'split_padding' needs 'split_words'", caller).into()),
        (None, None) => Ok(None),
        (Some(_), Some(p)) if !(p >= 0.0 && p.is_finite()) => Err(format!("{}: 'split_padding' must be a finite number of at least 0, got {}", caller, p).into()),
        (Some(min_piece), padding) => Ok(Some(WordSplit { min_piece, padding: padding.unwrap_or(0.0) })),
    }
}

impl TextBox {
    fn new(options: Map) -> RhaiResult<TextBox> {
        const CALLER: &str = "new_textbox";
        check_keys(CALLER, &options, &TEXTBOX_KEYS)?;
        let symbol_scale = positive(CALLER, &options, "symbol_scale")?;
        let shape = match options.get("shape") {
            Some(_) if ["center", "width", "height"].iter().any(|k| options.contains_key(*k)) =>
                return Err("new_textbox: give either a 'shape' or a 'center', 'width' and 'height', not both".into()),
            Some(shape) => {
                let path = shape.clone().try_cast::<Path>()
                    .ok_or_else(|| format!("new_textbox: 'shape' should be a path, got {}", shape.type_name()))?;
                // Curves flattened finely next to the glyphs.
                let rings: Vec<Vec<PVec>> = path.borrow().flattened(symbol_scale * 0.01).into_iter()
                    .map(|(points, _)| points)
                    .filter(|points| points.len() >= 3)
                    .collect();
                if rings.is_empty() {
                    return Err("new_textbox: 'shape' encloses nothing: it needs a subpath of at least three points".into());
                }
                rings
            }
            None => {
                let center: PVec = options.get("center")
                    .ok_or("new_textbox: missing 'center' (or give a 'shape')")?
                    .clone().try_cast::<Vec2>()
                    .ok_or("new_textbox: 'center' should be a vec")?
                    .into();
                let half = PVec::new(positive(CALLER, &options, "width")?, positive(CALLER, &options, "height")?) / 2.0;
                rectangle(Aabb::new(center - half, center + half))
            }
        };
        let angle = number(CALLER, &options, "angle")?.unwrap_or(0.0);
        if !angle.is_finite() {
            return Err(format!("new_textbox: 'angle' must be a finite number, got {}", angle).into());
        }
        let jagged_edge = number(CALLER, &options, "jagged_edge")?.unwrap_or(0.0);
        let layout = line_mode(CALLER, &options)?;
        let avoid = AvoidOption::parse(CALLER, &options, false)?;
        Ok(TextBox {
            shape: Rc::new(shape),
            angle,
            symbol_scale,
            jagged_edge,
            layout,
            padding: avoid.padding(symbol_scale),
            avoid: avoid.tree,
            spacing: spacing(CALLER, &options)?,
        })
    }
    /// Run `f` with the text box, avoiding what its tree holds now.
    fn with_core<R>(&self, f: impl FnOnce(&CoreTextBox) -> R) -> R {
        let tree = self.avoid.as_ref().map(|a| a.borrow());
        let core = CoreTextBox::new(TextBoxOptions {
            jagged_edge: self.jagged_edge,
            layout: self.layout.clone(),
            shape: self.shape.to_vec(),
            angle: self.angle,
            symbol_scale: self.symbol_scale,
            avoid: tree.as_deref(),
            padding: self.padding,
            spacing: self.spacing,
        });
        f(&core)
    }
}

fn symbols(caller: &str, glyphs: Array) -> RhaiResult<Vec<Symbol>> {
    Ok(glyph_array(caller, glyphs)?.into_iter().map(|g| g.get()).collect())
}

fn glyphs(symbols: Vec<Symbol>) -> Array {
    symbols.into_iter().map(|s| Dynamic::from(Glyph::new(s))).collect()
}

const SCATTER_KEYS: [&str; 11] = ["symbol_scale", "width_range", "height_range", "rotation_jitter", "scale_jitter", "jagged_edge", "max_attempts", "spacing", "avoid", "clearance", "layout"];

fn splat(canvas: &Canvas, msgs: Array, font: &Font, style: StrokeStyle, options: Map, symdict: &FontBook, rng: &SharedRng) -> RhaiResult<Canvas> {
    const CALLER: &str = "scatter_notes";
    check_keys(CALLER, &options, &SCATTER_KEYS)?;
    if canvas.borrow().strokes.is_empty() {
        // Messages are placed in boxes within the canvas's bounds.
        return Err(format!("{}: the canvas is empty, so there's nowhere to place messages; draw on it first", CALLER).into());
    }
    let max_attempts = match options.get("max_attempts") {
        None => 100,
        Some(n) => match n.as_int() {
            Ok(n) if n >= 1 => n as usize,
            _ => return Err(format!("{}: 'max_attempts' should be a whole number of at least 1, got {}", CALLER, n).into()),
        },
    };
    let symbol_scale = positive(CALLER, &options, "symbol_scale")?;
    let avoid = AvoidOption::parse(CALLER, &options, true)?;
    let opts = ScatterOptions {
        symbol_scale,
        layout: line_mode(CALLER, &options)?,
        clearance: avoid.padding(symbol_scale),
        width_range: range(CALLER, &options, "width_range")?,
        height_range: range(CALLER, &options, "height_range")?,
        jagged_edge: number(CALLER, &options, "jagged_edge")?.unwrap_or(0.1),
        max_attempts,
        spacing: spacing(CALLER, &options)?,
    };
    let rotation_jitter = number(CALLER, &options, "rotation_jitter")?.unwrap_or(std::f32::consts::PI / 16.0);
    let scale_jitter = number(CALLER, &options, "scale_jitter")?.unwrap_or(0.15);
    if !(rotation_jitter >= 0.0) || !(0.0..1.0).contains(&scale_jitter) {
        return Err(format!("{}: 'rotation_jitter' can't be negative and 'scale_jitter' must be in [0, 1)", CALLER).into());
    }
    let msgs = msgs.into_iter().enumerate().map(|(i, msg)| {
        msg.into_immutable_string()
            .map_err(|type_name| -> Box<EvalAltResult> { format!("{}: message {} is a {}, not a string", CALLER, i, type_name).into() })
    }).collect::<RhaiResult<Vec<_>>>()?;

    let mut rng = rng.borrow_mut();
    let symbol_msgs: Vec<Vec<Symbol>> = msgs.iter().map(|msg| {
        let mut symbols = font.build(msg, symdict, &mut *rng);
        jitter_symbols(&mut symbols, rotation_jitter, scale_jitter, &mut *rng);
        symbols
    }).collect();
    let journal_canvas: JournalCanvas = canvas.into();
    let tree = avoid.tree.as_ref().map(|a| a.borrow());
    let placed = scatter_notes(&journal_canvas, tree.as_deref(), &symbol_msgs, &opts, &mut *rng)
        .map_err(|err| format!("{}: {}", CALLER, err))?;

    let mut splatted = CanvasData::default();
    for symbol in placed.into_iter().flatten() {
        add_glyph(&mut splatted, &Glyph::new(symbol), &style)?;
    }
    Ok(Canvas::new(splatted))
}

pub fn register(engine: &mut Engine, symdict: Rc<FontBook>, rng: SharedRng) {
    engine.register_type_with_name::<TextBox>("TextBox");
    engine.register_fn("new_textbox", TextBox::new);

    let r = rng.clone();
    engine.register_fn("fill", move |tb: &mut TextBox, glyphs_in: Array| -> RhaiResult<Map> {
        let glyphs_in = symbols("fill", glyphs_in)?;
        let (placed, unplaced) = tb.with_core(|core| core.fill_bounds_with(glyphs_in, &mut *r.borrow_mut()));
        let mut result = Map::new();
        result.insert("placed".into(), Dynamic::from_array(glyphs(placed)));
        result.insert("unplaced".into(), Dynamic::from_array(glyphs(unplaced)));
        Ok(result)
    });

    let r = rng.clone();
    engine.register_fn("place_all", move |tb: &mut TextBox, glyphs_in: Array| -> RhaiResult<Array> {
        let mut to_place = symbols("place_all", glyphs_in)?;
        let mut blocks = Array::new();
        let mut rng = r.borrow_mut();
        // A glyph that doesn't fit even in an empty box is an error rather
        // than silently dropped.
        tb.with_core(|core| {
            while !to_place.is_empty() {
                let (placed, rest) = core.fill_bounds_with(std::mem::take(&mut to_place), &mut *rng);
                if placed.is_empty() {
                    return Err(format!("place_all: {} glyph(s) don't fit in the text box even when it's empty", rest.len()).into());
                }
                blocks.push(Dynamic::from_array(glyphs(placed)));
                to_place = rest;
            }
            Ok(blocks)
        })
    });

    let r = rng.clone();
    engine.register_fn("scatter_notes", move |canvas: &mut Canvas, msgs: Array, font: Font, style: StrokeStyle, options: Map| -> RhaiResult<Canvas> {
        splat(canvas, msgs, &font, style, options, &symdict, &r)
    });

}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use crate::journal::fonts::FontBook;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_FONTS).expect("Failed to load fonts.json");
        JournalEngine::new(Rc::new(symd))
    }

    /// Run `script` after some shared setup, returning its value.
    fn eval<T: Clone + 'static>(script: &str) -> T {
        let setup = r#"
            let style = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = font("loche_upper");
            let text = "the quick brown fox jumps over the lazy dog and keeps on running ";
            text = text + text + text + text;
        "#;
        engine().engine.eval::<T>(&format!("{}\n{}", setup, script)).expect("script should evaluate")
    }

    fn fails(script: &str) -> String {
        engine().engine.run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn fill_places_glyphs_inside_the_box_and_returns_the_rest() {
        let ok: bool = eval(r#"
            let glyphs = font.glyphs(text);
            let tb = new_textbox(#{ center: new_vec(0.0, 0.0), width: 100, height: 40, symbol_scale: 10.0 });
            let res = tb.fill(glyphs);
            if res.placed.len() == 0 || res.unplaced.len() == 0 { throw "expected a partial fill"; }
            if res.placed.len() + res.unplaced.len() != glyphs.len() { throw "glyphs went missing"; }
            for g in res.placed {
                if g.scale != 10.0 { throw "placed glyph not scaled: " + g.scale; }
                if g.position.x.abs() > 60.0 || g.position.y.abs() > 30.0 { throw "outside box: " + g.position; }
            }
            true
        "#);
        assert!(ok);
    }

    #[test]
    fn place_all_spreads_glyphs_over_boxes() {
        let ok: bool = eval(r#"
            let glyphs = font.glyphs(text);
            let tb = new_textbox(#{ center: new_vec(0.0, 0.0), width: 60, height: 30, symbol_scale: 10.0, layout: "words", jagged_edge: 0.5 });
            let blocks = tb.place_all(glyphs);
            let total = 0;
            for b in blocks { total += b.len(); }
            blocks.len() > 1 && total == glyphs.len()
        "#);
        assert!(ok);
        let err = fails(r#"
            let tb = new_textbox(#{ center: new_vec(0.0, 0.0), width: 5, height: 5, symbol_scale: 50.0 });
            tb.place_all(font("loche_upper").glyphs("ab"));
        "#);
        assert!(err.contains("2 glyph(s) don't fit"), "{}", err);
    }

    #[test]
    fn avoid_keeps_glyphs_off_a_canvas() {
        let counts: rhai::Array = eval(r#"
            let blot = new_canvas();
            for i in 0..10 { blot.add_path(new_circle(new_vec(0.0, 0.0), 4.0 * i), style); }
            let opts = #{ center: new_vec(0.0, 0.0), width: 100, height: 100, symbol_scale: 10.0 };
            let free = new_textbox(opts).fill(font.glyphs(text)).placed;
            opts.avoid = blot;
            opts.clearance = 0.0;
            let avoiding = new_textbox(opts).fill(font.glyphs(text)).placed;
            for g in avoiding {
                // Spaces take no room, so they may land anywhere.
                let p = g.position;
                if !g.is_blank && p.x * p.x + p.y * p.y < 36.0 * 36.0 { throw "placed on the blot: " + p.x + ", " + p.y; }
            }
            [free.len(), avoiding.len()]
        "#);
        let counts: Vec<i64> = counts.into_iter().map(|c| c.as_int().unwrap()).collect();
        assert!(counts[1] < counts[0], "{:?}", counts);
    }

    #[test]
    fn split_words_break_round_what_they_avoid() {
        let xs: rhai::Array = eval(r#"
            fn last_before(glyphs, x) {
                let m = -1000.0;
                for g in glyphs { if g.position.x < x && g.position.x > m { m = g.position.x; } }
                m
            }
            let bar = new_line(new_vec(-150.0, -100.0), new_vec(-150.0, 100.0));
            let opts = #{ center: new_vec(0.0, 0.0), width: 400, height: 10, symbol_scale: 10.0, layout: "words", avoid: bar, clearance: 2.0 };
            let whole = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            opts.split_words = true;
            let split = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            opts.split_padding = 1.0;
            let padded = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            opts.split_words = 8;   // the piece before the bar would be too short
            let long_pieces = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            [whole[0].position.x, split[0].position.x, last_before(split, -150.0), last_before(padded, -150.0),
             long_pieces[0].position.x, split.len(), padded.len()]
        "#);
        let xs: Vec<f64> = xs.into_iter().map(|x| x.as_float().unwrap_or_else(|_| x.as_int().unwrap() as f64)).collect();
        assert!(xs[0] > -150.0, "a whole word moves past the bar: {:?}", xs);
        assert!(xs[1] < -150.0, "a split one starts before it: {:?}", xs);
        assert!(xs[3] < xs[2], "padding keeps the first piece further from it: {:?}", xs);
        assert!(xs[4] > -150.0, "pieces too short to split: {:?}", xs);
        assert!(xs[5] == 16.0 && xs[6] == 16.0, "every glyph placed: {:?}", xs);
    }

    const SPLAT_SETUP: &str = r#"
        let style = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
        let base = new_canvas();
        base.add_path(new_rectangle(new_vec(0.0, 0.0), 800.0, 800.0), style);
        base.add_path(new_circle(new_vec(0.0, 0.0), 150.0), style);
        let msgs = ["ash and oak", "a quiet note", "the last word"];
        let opts = #{ symbol_scale: 20.0, width_range: [150, 250], height_range: [100, 200] };
    "#;

    fn splat_glyph_positions(seed: i64) -> Vec<(f32, f32)> {
        let script = format!(r#"{}
            seed({});
            let notes = base.scatter_notes(msgs, font("loche_upper"), style, opts);
            let page = new_page();
            page.add_canvas(notes);
            journal_entry.add_page(page);
        "#, SPLAT_SETUP, seed);
        let entry = engine().build_entry_str(&script).expect("script should run");
        entry.pages[0].canvases()[0].strokes.iter().map(|stroke| {
            let points = stroke.path.points_within();
            let sum = points.iter().fold(parry2d::math::Vec2::ZERO, |acc, p| acc + *p);
            let c = sum / points.len() as f32;
            (c.x, c.y)
        }).collect()
    }

    #[test]
    fn splat_places_every_message_clear_of_the_canvas() {
        let glyphs = splat_glyph_positions(5);
        // One stroke per non-blank glyph: 9 + 10 + 11 letters.
        assert_eq!(glyphs.len(), 30);
        for (x, y) in &glyphs {
            let r = (x * x + y * y).sqrt();
            assert!((r - 150.0).abs() > 5.0, "glyph at ({}, {}) sits on the circle", x, y);
            assert!(x.abs() < 400.0 && y.abs() < 400.0, "glyph at ({}, {}) is off the canvas", x, y);
        }
        assert_eq!(glyphs, splat_glyph_positions(5));
        assert_ne!(glyphs, splat_glyph_positions(6));
    }

    #[test]
    fn splat_can_keep_words_whole() {
        let script = format!(r#"{}
            seed(5);
            opts.layout = "words";
            let notes = base.scatter_notes(msgs, font("loche_upper"), style, opts);
            let page = new_page();
            page.add_canvas(notes);
            journal_entry.add_page(page);
        "#, SPLAT_SETUP);
        let entry = engine().build_entry_str(&script).expect("script should run");
        assert_eq!(entry.pages[0].canvases()[0].strokes.len(), 30);
        let err = fails(&format!("{}\nopts.layout = \"zigzag\";\nbase.scatter_notes(msgs, font(\"loche_upper\"), style, opts);", SPLAT_SETUP));
        assert!(err.contains("scatter_notes: unknown layout 'zigzag'"), "{}", err);
    }

    #[test]
    fn splat_fails_when_a_message_cannot_fit() {
        let err = fails(&format!(r#"{}
            opts.symbol_scale = 400.0;
            opts.max_attempts = 5;
            base.scatter_notes(msgs, font("loche_upper"), style, opts);
        "#, SPLAT_SETUP));
        assert!(err.contains("scatter_notes: message 0 doesn't fit in 5 tries"), "{}", err);
    }

    #[test]
    fn splat_needs_something_drawn() {
        let err = fails(&format!("{}\nnew_canvas().scatter_notes(msgs, font(\"loche_upper\"), style, opts);", SPLAT_SETUP));
        assert!(err.contains("scatter_notes: the canvas is empty"), "{}", err);
    }

    #[test]
    fn bad_splat_options_fail_at_their_line() {
        for (change, want) in [
            ("opts.remove(\"width_range\");", "missing 'width_range'"),
            ("opts.height_range = [300, 100];", "'height_range' should be [low, high]"),
            ("opts.max_attempts = 0;", "'max_attempts' should be a whole number"),
            ("opts.scale_jitter = 1.5;", "'scale_jitter' must be in [0, 1)"),
            ("opts.colour = 1;", "unknown option 'colour'"),
            ("msgs.push(3);", "message 3 is a i64"),
        ] {
            let err = fails(&format!("{}\n{}\nbase.scatter_notes(msgs, font(\"loche_upper\"), style, opts);", SPLAT_SETUP, change));
            assert!(err.contains(want) && err.contains("line 10"), "{}: {}", want, err);
        }
    }

    #[test]
    fn bad_options_fail_at_their_line() {
        let c = "new_vec(0.0, 0.0)";
        for (opts, want) in [
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, colour: 1 }}", c), "unknown option 'colour'"),
            (format!("#{{ center: {}, shape: new_circle({}, 5.0), symbol_scale: 2 }}", c, c), "either a 'shape' or a 'center'"),
            ("#{ shape: 3, symbol_scale: 2 }".to_string(), "'shape' should be a path, got i64"),
            (format!("#{{ shape: new_line({}, new_vec(1.0, 1.0)), symbol_scale: 2 }}", c), "'shape' encloses nothing"),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, angle: \"up\" }}", c), "'angle'"),
            ("#{ width: 10, height: 10, symbol_scale: 2 }".to_string(), "missing 'center'"),
            (format!("#{{ center: {}, width: 0, height: 10, symbol_scale: 2 }}", c), "'width' must be positive"),
            (format!("#{{ center: {}, width: 10, height: 10 }}", c), "missing 'symbol_scale'"),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, layout: \"zigzag\" }}", c), "unknown layout 'zigzag'"),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, clearance: 3 }}", c), "'clearance' needs 'avoid'"),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, avoid: new_canvas(), clearance: -3 }}", c), "'clearance' must be a finite number"),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, split_words: true }}", c), "'split_words' needs layout: \"words\""),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, layout: \"words\", split_padding: 1 }}", c), "'split_padding' needs 'split_words'"),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, layout: \"words\", split_words: 0 }}", c), "'split_words' should be true, false"),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, layout: \"words\", split_words: true, split_padding: -1 }}", c), "'split_padding' must be a finite number"),
        ] {
            let err = fails(&format!("let ok = 1;\nnew_textbox({});", opts));
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }
    #[test]
    fn a_rectangle_shape_fills_as_a_box_does() {
        let same: bool = eval(r#"
            let blot = new_canvas();
            blot.add_path(new_circle(new_vec(10.0, 5.0), 30.0), style);
            let same = true;
            for layout in ["flat", "words"] {
                let opts = #{ symbol_scale: 12.0, layout: layout, jagged_edge: 0.7, avoid: blot, spacing: 0.1 };
                let positions = [];
                for shaped in [false, true] {
                    seed(3);
                    let o = opts;
                    if shaped { o.shape = new_rectangle(new_vec(3.0, -7.0), 230, 150); o.angle = 0; }
                    else { o.center = new_vec(3.0, -7.0); o.width = 230; o.height = 150; }
                    let out = [];
                    for g in new_textbox(o).fill(font.glyphs(text)).placed { out.push([g.position.x, g.position.y, g.rotation]); }
                    positions.push(out);
                }
                if positions[0].len() < 20 || positions[0] != positions[1] { same = false; }
            }
            same
        "#);
        assert!(same);
    }

    #[test]
    fn a_shape_keeps_glyphs_inside_it_on_turned_lines() {
        let ok: bool = eval(r#"
            let tb = new_textbox(#{ shape: new_circle(new_vec(0.0, 0.0), 80.0), angle: PI() / 6.0, symbol_scale: 10.0 });
            let placed = tb.fill(font.glyphs(text)).placed;
            if placed.len() < 40 { throw "too few placed: " + placed.len(); }
            for g in placed {
                let p = g.position;
                if p.x * p.x + p.y * p.y > 82.0 * 82.0 { throw "outside the circle: " + p; }
                if (g.rotation - PI() / 6.0).abs() > 0.3 { throw "not turned to the lines: " + g.rotation; }
            }
            true
        "#);
        assert!(ok);
    }
}
