//! Text in boxes for rhai scripts: text boxes, and messages scattered
//! over a canvas.
//!
//! ```rhai
//! let b = canvas.bounds();          // #{ min, max, center, width, height }
//!
//! // Flow glyphs into a box, line by line, keeping clear of a canvas.
//! // Text calls take a style, a type and a paragraph (see text_settings.rs).
//! let tb = new_textbox(#{
//!     center: b.center, width: b.width * 1.2, height: b.height * 1.2,
//!     type: #{ glyph_scale: 52.0, glyph_tracking: 0.1 },   // the size glyphs are set at
//!     paragraph: #{
//!         layout: "words",          // wrap at blanks (spaces); default "flat", glyph by glyph
//!         line_start_jitter: 4.0,   // random line indent, in canvas units
//!         clearance: 6.0,           // how far from what's avoided (default 3)
//!         split_words: true,        // "words" only: break words round what's avoided
//!         split_padding: 0.3,       // pieces keep this much further from it, in glyph scales
//!     },
//!     style: #{ glyph_position_jitter: 2.0 },   // each placed glyph off true (the hand's)
//!     avoid: canvas,                // optional: see avoid.rs
//! });
//!
//! // Or fill any closed shape, on lines at the paragraph's angle (radians;
//! // default 0, left to right, each line below the last). Each subpath of
//! // the path is an outline, closed back to its start; one inside another
//! // is a hole. A rectangle shape at angle 0 fills just as center/width/height do.
//! let round = new_textbox(#{ shape: new_circle(b.center, 300.0), type: #{ glyph_scale: 20.0 }, paragraph: #{ angle: 0.3 } });
//!
//! let res = tb.fill(glyphset.glyphs(msg));   // #{ placed: [..], unplaced: [..] }
//! let blocks = tb.place_all(glyphs);         // one array per filled box
//!
//! // Text wrapped to a width from a point, line after line: #{ canvas, bottom },
//! // bottom being where the next block goes. With the hand's baseline_wander,
//! // each line wanders up and down from straight by up to that many units.
//! let block = write_block(msg, vec2(0, 0), 400.0, #{ style: ink, type: body, paragraph: #{ line_height: 1.3 } });
//!
//! // Scatter messages over a canvas in random boxes, clear of its strokes
//! // and of each other, box sizes tried between the ranges. Returns a
//! // canvas with just the messages.
//! let notes = page_canvas.scatter_notes(["first note", "second note"], [200, 500], [300, 600], #{
//!     style: ink, type: body,
//!     paragraph: #{ line_start_jitter: 2.0, layout: "words", clearance: 3.0 },
//!     max_attempts: 100,          // boxes per message before backing off
//!     avoid: other,               // more to keep clear of (see avoid.rs)
//! });
//! ```
//!
//! Placement scales each glyph by the glyph scale and turns it to the line,
//! on top of any scale and rotation it already has. Jagged edges come from
//! the engine's seeded generator. Scattered notes turn and scale by the
//! hand's glyph jitter before they're placed, and stray by its position
//! jitter after.

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2 as PVec;
use rand::Rng;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};
use std::rc::Rc;

use super::glyphs::{add_glyphs, glyph_array, Glyph};
use super::random::SharedRng;
use super::{Canvas, CanvasData, Path, Vec2};
use crate::journal::canvas::artistic::{jitter_symbols, scatter_notes, ScatterOptions};
use crate::journal::fonts::FontBook;
use crate::journal::typeset::Align;
use crate::journal::canvas::textbox::{rectangle, TextBox as CoreTextBox, TextBoxLineMode, TextBoxOptions};
use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::symbol::Symbol;
use super::avoid::{Avoid, AvoidOption};
use super::options::{positive, range};
use super::options::Settings;
use super::text_settings::{Jitter, TextSettings, Wander};

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
    align: Align,
    /// Whether glyphs' ink stays inside the shape: not for write_block's
    /// line boxes, which glyphs overhang.
    contain: bool,
    valign: f32,
    /// How far glyphs' ink may reach past the shape's edge.
    overhang: f32,
    /// The hand's baseline_wander, and the paragraph's line_gap_jitter:
    /// not for write_block's line boxes, as it wanders and spaces its
    /// lines itself.
    wander: f32,
    line_gap_jitter: f32,
    /// The paragraph's line_height, if it's set.
    line_height: Option<f32>,
    jitter: Jitter,
}

/// What `new_textbox` takes, besides a style, a type and a paragraph.
const TEXTBOX_KEYS: [&str; 5] = ["center", "width", "height", "shape", "avoid"];

impl TextBox {
    fn new(options: Map) -> RhaiResult<TextBox> {
        const CALLER: &str = "new_textbox";
        let text = TextSettings::read(CALLER, options, &TEXTBOX_KEYS)?;
        TextBox::from_settings(CALLER, &text)
    }
    fn from_settings(caller: &str, text: &TextSettings) -> RhaiResult<TextBox> {
        let options = &text.options;
        let symbol_scale = text.type_.size(caller)?;
        let shape = match options.get("shape") {
            Some(_) if ["center", "width", "height"].iter().any(|k| options.contains_key(*k)) =>
                return Err(format!("{}: give either a 'shape' or a 'center', 'width' and 'height', not both", caller).into()),
            Some(shape) => {
                let path = shape.clone().try_cast::<Path>()
                    .ok_or_else(|| format!("{}: 'shape' should be a path, got {}", caller, shape.type_name()))?;
                shape_rings(caller, "'shape'", &path, symbol_scale)?
            }
            None => {
                let center: PVec = options.get("center")
                    .ok_or_else(|| format!("{}: missing 'center' (or give a 'shape')", caller))?
                    .clone().try_cast::<Vec2>()
                    .ok_or_else(|| format!("{}: 'center' should be a vec", caller))?
                    .into();
                let half = PVec::new(positive(caller, options, "width")?, positive(caller, options, "height")?) / 2.0;
                rectangle(Aabb::new(center - half, center + half))
            }
        };
        TextBox::with_shape(caller, text, shape)
    }
    /// A box of `shape`'s outlines, set as `text` says.
    fn with_shape(caller: &str, text: &TextSettings, shape: Vec<Vec<PVec>>) -> RhaiResult<TextBox> {
        let options = &text.options;
        let symbol_scale = text.type_.size(caller)?;
        let paragraph = &text.paragraph;
        let avoid = AvoidOption::parse(caller, options, paragraph.clearance())?;
        Ok(TextBox {
            shape: Rc::new(shape),
            angle: paragraph.angle().unwrap_or(0.0),
            symbol_scale,
            // The text box takes it in glyph scales.
            jagged_edge: paragraph.line_start_jitter() / symbol_scale,
            layout: paragraph.line_mode(caller, false)?,
            padding: avoid.padding(),
            avoid: avoid.tree,
            spacing: text.type_.tracking(),
            align: paragraph.align(),
            contain: true,
            valign: paragraph.valign(),
            overhang: paragraph.overhang(),
            wander: text.style.handwriting().baseline_wander(),
            line_gap_jitter: paragraph.line_gap_jitter(),
            line_height: paragraph.set_line_height(),
            jitter: text.jitter(),
        })
    }
    /// The box with its glyphs set at `symbol_scale`, its jagged edge
    /// as far in, in canvas units, as before.
    fn at_scale(&self, symbol_scale: f32) -> TextBox {
        TextBox { symbol_scale, jagged_edge: self.jagged_edge * self.symbol_scale / symbol_scale, ..self.clone() }
    }
    /// Fill the box with what fits, jittering what's placed.
    fn fill(&self, glyphs: Vec<Symbol>, rng: &mut dyn rand::RngCore) -> (Vec<Symbol>, Vec<Symbol>) {
        let (mut placed, unplaced) = self.with_core(|core| core.fill_bounds_with(glyphs, rng));
        self.jitter.apply(&mut placed, rng);
        (placed, unplaced)
    }
    /// Fill the box again and again until every glyph is placed: the
    /// placed glyphs, box by box.
    fn place_all(&self, caller: &str, mut to_place: Vec<Symbol>, rng: &mut dyn rand::RngCore) -> RhaiResult<Vec<Vec<Symbol>>> {
        let mut blocks = Vec::new();
        // A glyph that doesn't fit even in an empty box is an error rather
        // than silently dropped.
        while !to_place.is_empty() {
            let (placed, rest) = self.with_core(|core| core.fill_bounds_with(std::mem::take(&mut to_place), &mut *rng));
            if placed.is_empty() {
                return Err(format!("{}: {} glyph(s) don't fit in the text box even when it's empty", caller, rest.len()).into());
            }
            blocks.push(placed);
            to_place = rest;
        }
        // Jittered once all are placed, so the boxes are set as without.
        for block in &mut blocks {
            self.jitter.apply(block, rng);
        }
        Ok(blocks)
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
            align: self.align,
            contain: self.contain,
            valign: self.valign,
            overhang: self.overhang,
            wander: self.wander,
            line_gap_jitter: self.line_gap_jitter,
            line_height: self.line_height,
        });
        f(&core)
    }
}

/// A path's subpaths as a text box's outlines, curves flattened finely
/// next to glyphs `symbol_scale` big; `what` names the path in errors.
fn shape_rings(caller: &str, what: &str, path: &Path, symbol_scale: f32) -> RhaiResult<Vec<Vec<PVec>>> {
    let rings: Vec<Vec<PVec>> = path.borrow().flattened(symbol_scale * 0.01).into_iter()
        .map(|(points, _)| points)
        .filter(|points| points.len() >= 3)
        .collect();
    if rings.is_empty() {
        return Err(format!("{}: {} encloses nothing: it needs a subpath of at least three points", caller, what).into());
    }
    Ok(rings)
}

fn symbols(caller: &str, glyphs: Array) -> RhaiResult<Vec<Symbol>> {
    Ok(glyph_array(caller, glyphs)?.into_iter().map(|g| g.get()).collect())
}

fn glyphs(symbols: Vec<Symbol>) -> Array {
    symbols.into_iter().map(|s| Dynamic::from(Glyph::new(s))).collect()
}

/// What `scatter_notes` takes, besides a style, a type and a paragraph.
const SCATTER_KEYS: [&str; 2] = ["max_attempts", "avoid"];

/// `canvas.scatter_notes(messages, width_range, height_range, settings)`.
fn splat(canvas: &Canvas, msgs: Array, width_range: Dynamic, height_range: Dynamic, settings: Map, symdict: &FontBook, rng: &SharedRng) -> RhaiResult<Canvas> {
    const CALLER: &str = "scatter_notes";
    let text = TextSettings::read(CALLER, settings, &SCATTER_KEYS)?;
    let font = text.type_.glyphset(CALLER)?;
    if canvas.borrow().strokes.is_empty() {
        // Messages are placed in boxes within the canvas's bounds.
        return Err(format!("{}: the canvas is empty, so there's nowhere to place messages; draw on it first", CALLER).into());
    }
    let max_attempts = match text.options.get("max_attempts") {
        None => 100,
        Some(n) => match n.as_int() {
            Ok(n) if n >= 1 => n as usize,
            _ => return Err(format!("{}: 'max_attempts' should be a whole number of at least 1, got {}", CALLER, n).into()),
        },
    };
    let symbol_scale = text.type_.size(CALLER)?;
    let paragraph = &text.paragraph;
    let avoid = AvoidOption::parse(CALLER, &text.options, paragraph.clearance())?;
    let ranges = Map::from_iter([("width_range".into(), width_range), ("height_range".into(), height_range)]);
    let opts = ScatterOptions {
        symbol_scale,
        layout: paragraph.line_mode(CALLER, false)?,
        clearance: avoid.padding(),
        width_range: range(CALLER, &ranges, "width_range")?,
        height_range: range(CALLER, &ranges, "height_range")?,
        // In glyph scales.
        jagged_edge: paragraph.line_start_jitter() / symbol_scale,
        max_attempts,
        spacing: text.type_.tracking(),
    };
    // Notes turn and scale before they're placed, as the hand turns and
    // scales glyphs; its position jitter is after.
    let hand = text.style.handwriting();
    let (rotation_jitter, scale_jitter) = (hand.glyph_rotation_jitter() as f32, hand.glyph_scale_jitter() as f32);
    let after = Jitter::of(&text.style).position_only();
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
    // Each note's first glyph, which doesn't join the last note.
    let firsts: Vec<usize> = placed.iter().scan(0, |at, note| { let first = *at; *at += note.len(); Some(first) }).collect();
    let mut placed: Vec<Symbol> = placed.into_iter().flatten().collect();
    after.apply(&mut placed, &mut *rng);
    add_glyphs(&mut splatted, &placed, |k| firsts.contains(&k), &text.style, &mut *rng)?;
    Ok(Canvas::new(splatted))
}

/// What `write_block` takes, besides a style, a type and a paragraph.
const BLOCK_KEYS: [&str; 1] = ["avoid"];

/// `write_block(text, at, width, settings)`: the text wrapped to lines
/// `width` long, the first from `at` (on its centre line, as write_line
/// sets it), each `line_height` glyph scales below the last, give or take
/// up to the paragraph's `line_gap_jitter` at random, each line wandering
/// up and down from straight by up to the hand's `baseline_wander`, words
/// kept whole unless the paragraph's `layout` is "flat". Gives
/// `#{ canvas, bottom }`, `bottom` being where a line after them would go.
fn write_block(msg: &str, at: Vec2, width: Dynamic, settings: Map, symdict: &FontBook, rng: &SharedRng) -> RhaiResult<Map> {
    const CALLER: &str = "write_block";
    let text = TextSettings::read(CALLER, settings, &BLOCK_KEYS)?;
    let font = text.type_.glyphset(CALLER)?;
    let size = text.type_.size(CALLER)?;
    let width = super::options::float(CALLER, "the width", &width)? as f32;
    if !(width > 0.0) {
        return Err(format!("{}: the width must be positive, got {}", CALLER, width).into());
    }
    let paragraph = &text.paragraph;
    let line_gap_jitter = paragraph.line_gap_jitter();
    let wander = text.style.handwriting().baseline_wander();
    let at: PVec = at.into();
    // A box one line high, at the line's own height, so what it avoids is
    // avoided where the line is: a line each. Words kept whole by default.
    let mut line_paragraph = paragraph.clone();
    if paragraph.get("layout").is_unit() {
        line_paragraph.set(CALLER, "layout", &"words".into())?;
    }
    let line_box = |y: f32| -> RhaiResult<TextBox> {
        let mut options = text.options.clone();
        options.insert("center".into(), Dynamic::from(Vec2::new((at.x + width / 2.0) as f64, y as f64)));
        options.insert("width".into(), Dynamic::from(width as f64));
        options.insert("height".into(), Dynamic::from((size * 0.5) as f64));
        let line_text = TextSettings { style: text.style.clone(), type_: text.type_.clone(), paragraph: line_paragraph.clone(), options };
        let mut line = TextBox::from_settings(CALLER, &line_text)?;
        line.contain = false;
        (line.wander, line.line_gap_jitter) = (0.0, 0.0);
        Ok(line)
    };
    let mut r = rng.borrow_mut();
    let mut to_place = font.build(msg, symdict, &mut *r);
    let pitch = paragraph.line_height() * size;
    let mut canvas = CanvasData::default();
    // How far lines have strayed from their even spacing, all told.
    let mut drift = 0.0;
    let mut lines = 0;
    while !to_place.is_empty() {
        if lines > 0 && line_gap_jitter > 0.0 {
            drift += r.random_range(-line_gap_jitter..line_gap_jitter);
        }
        let tb = line_box(at.y + lines as f32 * pitch + drift)?;
        let (mut row, rest) = tb.with_core(|core| core.fill_bounds_with(std::mem::take(&mut to_place), &mut *r));
        if row.is_empty() {
            return Err(format!("{}: {} glyph(s) don't fit a line {} wide", CALLER, rest.len(), width).into());
        }
        to_place = rest;
        lines += 1;
        if wander > 0.0 {
            // A little past the width: glyphs can reach over it.
            Wander::new(wander, size, width + size * 2.0, &mut *r).bend(&mut row, at.x);
        }
        tb.jitter.apply(&mut row, &mut *r);
        add_glyphs(&mut canvas, &row, |_| false, &text.style, &mut *r)?;
    }
    let rows = lines;
    let mut out = Map::new();
    out.insert("canvas".into(), Dynamic::from(Canvas::new(canvas)));
    out.insert("bottom".into(), Dynamic::from((at.y + rows as f32 * pitch + drift) as f64));
    Ok(out)
}

/// What `write_inside_path` takes, besides a style, a type and a paragraph.
const INSIDE_KEYS: [&str; 1] = ["avoid"];

/// How many halvings close in on the largest glyph scale that fits, for
/// `write_inside_path` with the paragraph's `fit`.
const FIT_STEPS: usize = 20;

/// `write_inside_path(text, path, settings)`: the text written inside a
/// closed path, as a text box of that shape writes it, giving
/// `#{ canvas, rest }`, `rest` being the text that didn't fit (blanks it
/// starts with trimmed). With the paragraph's `fit`, the glyphs are as big
/// as they can be, up to the type's glyph_scale, and still all fit, so
/// `rest` is empty.
fn write_inside_path(msg: &str, path: &Path, settings: Map, symdict: &FontBook, rng: &SharedRng) -> RhaiResult<Map> {
    const CALLER: &str = "write_inside_path";
    let text = TextSettings::read(CALLER, settings, &INSIDE_KEYS)?;
    let fit = text.paragraph.fit();
    let font = text.type_.glyphset(CALLER)?;
    let size = text.type_.size(CALLER)?;
    let shape = shape_rings(CALLER, "the path", path, size)?;
    let mut tb = TextBox::with_shape(CALLER, &text, shape)?;
    let mut r = rng.borrow_mut();
    let symbols = font.build(msg, symdict, &mut *r);
    let set = |tb: &TextBox, r: &mut dyn rand::RngCore| tb.with_core(|core| core.fill_bounds_with(symbols.clone(), r));
    let (mut placed, unplaced) = set(&tb, &mut *r);
    if fit && !unplaced.is_empty() {
        // The largest scale at which everything fits, closed in on by
        // halving.
        let (mut low, mut high, mut best) = (0.0, size, None);
        for _ in 0..FIT_STEPS {
            let mid = (low + high) / 2.0;
            let scaled = tb.at_scale(mid);
            let (at, rest) = set(&scaled, &mut *r);
            if rest.is_empty() {
                (low, best) = (mid, Some((scaled, at)));
            } else {
                high = mid;
            }
        }
        let Some((scaled, at)) = best else {
            return Err(format!("{}: the text doesn't fit inside the path at any glyph scale", CALLER).into());
        };
        tb = scaled;
        placed = at;
    }
    let rest: String = if fit { String::new() } else {
        let chars: Vec<char> = msg.chars().collect();
        chars[chars.len() - unplaced.len().min(chars.len())..].iter().collect::<String>().trim_start().to_string()
    };
    tb.jitter.apply(&mut placed, &mut *r);
    let mut canvas = CanvasData::default();
    add_glyphs(&mut canvas, &placed, |_| false, &text.style, &mut *r)?;
    let mut out = Map::new();
    out.insert("canvas".into(), Dynamic::from(Canvas::new(canvas)));
    out.insert("rest".into(), rest.into());
    Ok(out)
}

pub fn register(engine: &mut Engine, symdict: Rc<FontBook>, rng: SharedRng) {
    engine.register_type_with_name::<TextBox>("TextBox");
    engine.register_fn("new_textbox", TextBox::new);

    let r = rng.clone();
    engine.register_fn("fill", move |tb: &mut TextBox, glyphs_in: Array| -> RhaiResult<Map> {
        let glyphs_in = symbols("fill", glyphs_in)?;
        let (placed, unplaced) = tb.fill(glyphs_in, &mut *r.borrow_mut());
        let mut result = Map::new();
        result.insert("placed".into(), Dynamic::from_array(glyphs(placed)));
        result.insert("unplaced".into(), Dynamic::from_array(glyphs(unplaced)));
        Ok(result)
    });

    let r = rng.clone();
    engine.register_fn("place_all", move |tb: &mut TextBox, glyphs_in: Array| -> RhaiResult<Array> {
        let blocks = tb.place_all("place_all", symbols("place_all", glyphs_in)?, &mut *r.borrow_mut())?;
        Ok(blocks.into_iter().map(|block| Dynamic::from_array(glyphs(block))).collect())
    });

    let r = rng.clone();
    let book = symdict.clone();
    engine.register_fn("scatter_notes", move |canvas: &mut Canvas, msgs: Array, width_range: Dynamic, height_range: Dynamic, settings: Map| -> RhaiResult<Canvas> {
        splat(canvas, msgs, width_range, height_range, settings, &book, &r)
    });
    let r = rng.clone();
    let book = symdict.clone();
    engine.register_fn("write_block", move |text: &str, at: Vec2, width: Dynamic, settings: Map| -> RhaiResult<Map> {
        write_block(text, at, width, settings, &book, &r)
    });
    let r = rng.clone();
    engine.register_fn("write_inside_path", move |text: &str, path: Path, settings: Map| -> RhaiResult<Map> {
        write_inside_path(text, &path, settings, &symdict, &r)
    });

}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use crate::journal::fonts::FontBook;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_GLYPHSETS).expect("Failed to load fonts.json");
        JournalEngine::new(Rc::new(symd))
    }

    /// Run `script` after some shared setup, returning its value.
    fn eval<T: Clone + 'static>(script: &str) -> T {
        let setup = r#"
            let style = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let font = glyphset("loche_upper");
            let text = "the quick brown fox jumps over the lazy dog and keeps on running ";
            text = text + text + text + text;
        "#;
        engine().engine.eval::<T>(&format!("{}\n{}", setup, script)).expect("script should evaluate")
    }

    fn fails(script: &str) -> String {
        engine().engine.run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn write_block_line_gap_jitter_moves_lines_off_even() {
        let ok: bool = eval(r#"
            let hand = #{ style: style, type: new_type_style(font, 10.0) };
            let even = write_block(text, vec2(0, 0), 100.0, hand);
            let lines = (even.bottom / 12.5).round();
            if (even.bottom - lines * 12.5).abs() > 1e-3 { throw "even lines should be line_height apart: " + even.bottom; }
            let jittered = write_block(text, vec2(0, 0), 100.0, hand + #{ paragraph: #{ line_gap_jitter: 3.0 } });
            if (jittered.bottom - even.bottom).abs() < 1e-3 { throw "line_gap_jitter should move the lines"; }
            if (jittered.bottom - even.bottom).abs() > lines * 3.0 { throw "lines strayed too far: " + jittered.bottom; }
            true
        "#);
        assert!(ok);
        let err = fails(r#"write_block("a", vec2(0, 0), 100.0, #{ type: new_type_style(glyphset("loche_upper"), 10.0), paragraph: #{ line_gap_jitter: -1 } })"#);
        assert!(err.contains("'line_gap_jitter' can't be negative"), "{}", err);
    }

    #[test]
    fn baseline_wander_bends_lines_within_bounds() {
        let ok: bool = eval(r#"
            let hand = #{ style: style, type: new_type_style(font, 10.0) };
            let wandering = style;
            wandering.baseline_wander = 3.0;
            seed(3);
            let even = write_block(text, vec2(0, 0), 100.0, hand);
            seed(3);
            let wavy = write_block(text, vec2(0, 0), 100.0, hand + #{ style: wandering });
            if (wavy.bottom - even.bottom).abs() > 1e-3 { throw "baseline_wander shouldn't move where the next block goes"; }
            let e = even.canvas.bounds();
            let w = wavy.canvas.bounds();
            if (e.height - w.height).abs() < 1e-3 { throw "baseline_wander should bend the lines"; }
            if (e.height - w.height).abs() > 2.0 * 3.0 + 3.0 { throw "lines bent too far: " + w.height; }
            seed(3);
            let line = write_line("the quick brown fox", vec2(0, 0), hand);
            seed(3);
            let bent = write_line("the quick brown fox", vec2(0, 0), hand + #{ style: wandering });
            let l = line.bounds();
            let b = bent.bounds();
            if (l.height - b.height).abs() < 1e-3 { throw "write_line's baseline_wander should bend the line"; }
            if b.min.y < l.min.y - 3.5 || b.max.y > l.max.y + 3.5 { throw "the line bent too far"; }
            true
        "#);
        assert!(ok);
        let err = fails(r#"write_line("a", vec2(0, 0), #{ type: new_type_style(glyphset("loche_upper"), 10.0), style: #{ baseline_wander: -1 } })"#);
        assert!(err.contains("'baseline_wander' can't be negative"), "{}", err);
    }

    #[test]
    fn fill_places_glyphs_inside_the_box_and_returns_the_rest() {
        let ok: bool = eval(r#"
            let glyphs = font.glyphs(text);
            let tb = new_textbox(#{ center: vec2(0.0, 0.0), width: 100, height: 40, type: #{ glyph_scale: 10.0 } });
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
            let tb = new_textbox(#{ center: vec2(0.0, 0.0), width: 60, height: 30, type: #{ glyph_scale: 10.0 }, paragraph: #{ layout: "words", line_start_jitter: 5.0 } });
            let blocks = tb.place_all(glyphs);
            let total = 0;
            for b in blocks { total += b.len(); }
            blocks.len() > 1 && total == glyphs.len()
        "#);
        assert!(ok);
        let err = fails(r#"
            let tb = new_textbox(#{ center: vec2(0.0, 0.0), width: 5, height: 5, type: #{ glyph_scale: 50.0 } });
            tb.place_all(glyphset("loche_upper").glyphs("ab"));
        "#);
        assert!(err.contains("2 glyph(s) don't fit"), "{}", err);
    }

    #[test]
    fn avoid_keeps_glyphs_off_a_canvas() {
        let counts: rhai::Array = eval(r#"
            let blot = new_canvas();
            for i in 0..10 { blot.add(new_circle(vec2(0.0, 0.0), 4.0 * i), style); }
            let opts = #{ center: vec2(0.0, 0.0), width: 100, height: 100, type: #{ glyph_scale: 10.0 } };
            let free = new_textbox(opts).fill(font.glyphs(text)).placed;
            opts.avoid = blot;
            opts.paragraph = #{ clearance: 0.0 };
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
            let bar = new_line(vec2(-160.0, -100.0), vec2(-160.0, 100.0));
            // One line high: glyphs' ink keeps inside the box.
            let opts = #{ center: vec2(0.0, 0.0), width: 400, height: 16, type: #{ glyph_scale: 10.0 }, paragraph: #{ layout: "words", clearance: 2.0 }, avoid: bar };
            let whole = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            opts.paragraph.split_words = true;
            let split = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            opts.paragraph.split_padding = 1.0;
            let padded = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            opts.paragraph.split_words = 8;   // the piece before the bar would be too short
            let long_pieces = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            [whole[0].position.x, split[0].position.x, last_before(split, -160.0), last_before(padded, -160.0),
             long_pieces[0].position.x, split.len(), padded.len()]
        "#);
        let xs: Vec<f64> = xs.into_iter().map(|x| x.as_float().unwrap_or_else(|_| x.as_int().unwrap() as f64)).collect();
        assert!(xs[0] > -160.0, "a whole word moves past the bar: {:?}", xs);
        assert!(xs[1] < -160.0, "a split one starts before it: {:?}", xs);
        assert!(xs[3] < xs[2], "padding keeps the first piece further from it: {:?}", xs);
        assert!(xs[4] > -160.0, "pieces too short to split: {:?}", xs);
        assert!(xs[5] == 16.0 && xs[6] == 16.0, "every glyph placed: {:?}", xs);
    }

    #[test]
    fn break_words_cut_words_at_line_ends() {
        let counts: rhai::Array = eval(r#"
            // Lines of about twelve glyphs, words of eight.
            let opts = #{ center: vec2(0.0, 0.0), width: 65, height: 400, type: #{ glyph_scale: 10.0 }, paragraph: #{ layout: "words" } };
            let text = "abcdefgh abcdefgh abcdefgh abcdefgh";
            let whole = new_textbox(opts).fill(font.glyphs(text)).placed;
            opts.paragraph.break_words = true;
            let broken = new_textbox(opts).fill(font.glyphs(text)).placed;
            opts.paragraph.break_words = 5;   // no piece of a word of eight fits
            let long_pieces = new_textbox(opts).fill(font.glyphs(text)).placed;
            fn lines(glyphs) { let ys = []; for g in glyphs { let y = g.position.y.round(); if !ys.contains(y) { ys.push(y); } } ys.len() }
            fn first_line(glyphs) { let y = glyphs[0].position.y; glyphs.filter(|g| (g.position.y - y).abs() < 1.0).len() }
            [lines(whole), lines(broken), lines(long_pieces), first_line(whole), first_line(broken), whole.len(), broken.len()]
        "#);
        let n: Vec<i64> = counts.into_iter().map(|c| c.as_int().unwrap()).collect();
        assert!(n[1] < n[0], "cut words take fewer lines: {:?}", n);
        assert_eq!(n[2], n[0], "pieces too short to cut: {:?}", n);
        assert!(n[4] > n[3], "a cut word's head fills the first line: {:?}", n);
        assert!(n[5] == 35 && n[6] == 35, "every glyph placed: {:?}", n);
        let err = engine().engine.run("new_textbox(#{ center: vec2(0, 0), width: 10, height: 10, type: #{ glyph_scale: 10 }, paragraph: #{ layout: \"flat\", break_words: true } });").expect_err("flat").to_string();
        assert!(err.contains("'break_words' needs layout: \"words\""), "{}", err);
    }

    const SPLAT_SETUP: &str = r#"
        let style = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
        let base = new_canvas();
        base.add(new_rectangle(vec2(0.0, 0.0), 800.0, 800.0), style);
        base.add(new_circle(vec2(0.0, 0.0), 150.0), style);
        let msgs = ["ash and oak", "a quiet note", "the last word"];
        let widths = [150, 250];
        let heights = [100, 200];
        let opts = #{ style: style, type: new_type_style(glyphset("loche_upper"), 20.0) };
    "#;

    fn splat_glyph_positions(seed: i64) -> Vec<(f32, f32)> {
        let script = format!(r#"{}
            seed({});
            let notes = base.scatter_notes(msgs, widths, heights, opts);
            let page = new_page();
            page.add(notes);
            journal_entry.add(page);
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
            opts.paragraph = #{{ layout: "words" }};
            let notes = base.scatter_notes(msgs, widths, heights, opts);
            let page = new_page();
            page.add(notes);
            journal_entry.add(page);
        "#, SPLAT_SETUP);
        let entry = engine().build_entry_str(&script).expect("script should run");
        assert_eq!(entry.pages[0].canvases()[0].strokes.len(), 30);
        let err = fails(&format!("{}\nopts.paragraph = #{{ layout: \"zigzag\" }};\nbase.scatter_notes(msgs, widths, heights, opts);", SPLAT_SETUP));
        assert!(err.contains("scatter_notes 'paragraph': unknown layout 'zigzag'"), "{}", err);
    }

    #[test]
    fn splat_fails_when_a_message_cannot_fit() {
        let err = fails(&format!(r#"{}
            opts.type = new_type_style(glyphset("loche_upper"), 400.0);
            opts.max_attempts = 5;
            base.scatter_notes(msgs, widths, heights, opts);
        "#, SPLAT_SETUP));
        assert!(err.contains("scatter_notes: message 0 doesn't fit in 5 tries"), "{}", err);
    }

    #[test]
    fn splat_needs_something_drawn() {
        let err = fails(&format!("{}\nnew_canvas().scatter_notes(msgs, widths, heights, opts);", SPLAT_SETUP));
        assert!(err.contains("scatter_notes: the canvas is empty"), "{}", err);
    }

    #[test]
    fn bad_splat_options_fail_at_their_line() {
        for (change, want) in [
            ("widths = 3;", "'width_range' should be [low, high]"),
            ("heights = [300, 100];", "'height_range' should be [low, high]"),
            ("opts.type = #{ glyph_scale: 20.0 };", "the type has no 'glyphset'"),
            ("opts.max_attempts = 0;", "'max_attempts' should be a whole number"),
            ("opts.style = #{ glyph_scale_jitter: 1.5 };", "'glyph_scale_jitter' is a fraction either way, below 1"),
            ("opts.colour = 1;", "unknown option 'colour'"),
            ("msgs.push(3);", "message 3 is a i64"),
        ] {
            let err = fails(&format!("{}\n{}\nbase.scatter_notes(msgs, widths, heights, opts);", SPLAT_SETUP, change));
            assert!(err.contains(want) && err.contains("line 12"), "{}: {}", want, err);
        }
    }

    #[test]
    fn bad_options_fail_at_their_line() {
        let c = "vec2(0.0, 0.0)";
        let size = "type: #{ glyph_scale: 2 }";
        for (opts, want) in [
            (format!("#{{ center: {}, width: 10, height: 10, {}, colour: 1 }}", c, size), "unknown option 'colour'"),
            (format!("#{{ center: {}, shape: new_circle({}, 5.0), {} }}", c, c, size), "either a 'shape' or a 'center'"),
            (format!("#{{ shape: 3, {} }}", size), "'shape' should be a path, got i64"),
            (format!("#{{ shape: new_line({}, vec2(1.0, 1.0)), {} }}", c, size), "'shape' encloses nothing"),
            (format!("#{{ center: {}, width: 10, height: 10, {}, paragraph: #{{ angle: \"up\" }} }}", c, size), "'angle'"),
            (format!("#{{ width: 10, height: 10, {} }}", size), "missing 'center'"),
            (format!("#{{ center: {}, width: 0, height: 10, {} }}", c, size), "'width' must be positive"),
            (format!("#{{ center: {}, width: 10, height: 10 }}", c), "the type has no 'glyph_scale'"),
            (format!("#{{ center: {}, width: 10, height: 10, {}, paragraph: #{{ layout: \"zigzag\" }} }}", c, size), "unknown layout 'zigzag'"),
            (format!("#{{ center: {}, width: 10, height: 10, {}, paragraph: #{{ clearance: -3 }} }}", c, size), "'clearance' can't be negative"),
            (format!("#{{ center: {}, width: 10, height: 10, {}, paragraph: #{{ split_words: true }} }}", c, size), "'split_words' needs layout: \"words\""),
            (format!("#{{ center: {}, width: 10, height: 10, {}, paragraph: #{{ layout: \"words\", split_padding: 1 }} }}", c, size), "'split_padding' needs 'split_words'"),
            (format!("#{{ center: {}, width: 10, height: 10, {}, paragraph: #{{ layout: \"words\", split_words: 0 }} }}", c, size), "'split_words' should be true, false"),
            (format!("#{{ center: {}, width: 10, height: 10, {}, paragraph: #{{ layout: \"words\", split_words: true, split_padding: -1 }} }}", c, size), "'split_padding' can't be negative"),
        ] {
            let err = fails(&format!("let ok = 1;\nnew_textbox({});", opts));
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }
    #[test]
    fn a_rectangle_shape_fills_as_a_box_does() {
        let same: bool = eval(r#"
            let blot = new_canvas();
            blot.add(new_circle(vec2(10.0, 5.0), 30.0), style);
            let same = true;
            for layout in ["flat", "words"] {
                let opts = #{ type: #{ glyph_scale: 12.0, glyph_tracking: 0.1 }, paragraph: #{ layout: layout, line_start_jitter: 8.4 }, avoid: blot };
                let positions = [];
                for shaped in [false, true] {
                    seed(3);
                    let o = opts;
                    if shaped { o.shape = new_rectangle(vec2(3.0, -7.0), 230, 150); o.paragraph.angle = 0; }
                    else { o.center = vec2(3.0, -7.0); o.width = 230; o.height = 150; }
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
    fn write_inside_path_gives_back_what_doesnt_fit() {
        let ok: bool = eval(r#"
            let hand = #{ style: style, type: new_type_style(font, 10.0), paragraph: #{ layout: "words" } };
            let star = new_star(vec2(0.0, 0.0), 60.0, 30.0, 5);
            let out = write_inside_path(text, star, hand);
            if out.rest == "" || out.rest.len() >= text.len() { throw "expected some of the text left: " + out.rest.len(); }
            if !text.ends_with(out.rest) { throw "rest should be the end of the text"; }
            let b = out.canvas.bounds();
            if b.min.x < -62.0 || b.max.x > 62.0 || b.min.y < -62.0 || b.max.y > 62.0 { throw "outside the star: " + b.min + " " + b.max; }
            let all = write_inside_path("a short line", new_circle(vec2(0.0, 0.0), 80.0), hand);
            if all.rest != "" { throw "all of it should fit"; }
            true
        "#);
        assert!(ok);
    }

    #[test]
    fn write_inside_path_fit_shrinks_text_to_fit() {
        let ok: bool = eval(r#"
            let hand = #{ style: style, type: new_type_style(font, 10.0), paragraph: #{ fit: true } };
            let circle = new_circle(vec2(0.0, 0.0), 50.0);
            let out = write_inside_path(text, circle, hand);
            if out.rest != "" { throw "fit should leave nothing over"; }
            let b = out.canvas.bounds();
            if b.max.x > 52.0 || b.min.x < -52.0 { throw "outside the circle"; }
            // Text that fits as it is stays its size.
            let line = write_inside_path("ab", circle, hand).canvas.bounds();
            let plain = write_inside_path("ab", circle, hand + #{ paragraph: #{ fit: false } }).canvas.bounds();
            (line.width - plain.width).abs() < 1.0
        "#);
        assert!(ok);
        for (settings, want) in [
            ("#{ type: new_type_style(glyphset(\"loche_upper\"), 10.0), paragraph: #{ fit: 1 } }", "'fit' should be true or false"),
            ("#{ type: new_type_style(glyphset(\"loche_upper\"), 10.0), colour: 1 }", "unknown option 'colour'"),
            ("#{ type: new_type_style(glyphset(\"loche_upper\"), 10.0), paragraph: #{ overhang: -1 } }", "'overhang' can't be negative"),
            ("#{ type: #{ glyph_scale: 10.0 } }", "the type has no 'glyphset'"),
        ] {
            let err = fails(&format!("let ok = 1;\nwrite_inside_path(\"ab\", new_circle(vec2(0.0, 0.0), 50.0), {});", settings));
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
        let err = fails(r#"write_inside_path("ab", new_line(vec2(0.0, 0.0), vec2(1.0, 1.0)), #{ type: new_type_style(glyphset("loche_upper"), 10.0) })"#);
        assert!(err.contains("the path encloses nothing"), "{}", err);
    }

    #[test]
    fn a_shape_keeps_glyphs_inside_it_on_turned_lines() {
        let ok: bool = eval(r#"
            let tb = new_textbox(#{ shape: new_circle(vec2(0.0, 0.0), 80.0), type: #{ glyph_scale: 10.0 }, paragraph: #{ angle: PI() / 6.0 } });
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
