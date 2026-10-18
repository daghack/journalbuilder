//! Text in boxes for rhai scripts: text boxes, and messages scattered
//! over a canvas.
//!
//! ```rhai
//! let b = canvas.bounds();          // #{ min, max, center, width, height }
//!
//! // Flow glyphs into a box, line by line, keeping clear of a canvas.
//! // Text calls take text settings, the keys layout roles take.
//! let tb = new_textbox(#{
//!     center: b.center, width: b.width * 1.2, height: b.height * 1.2,
//!     size: 52.0,                   // the size glyphs are set at
//!     layout: "flat",               // or "words": wrap at blanks (spaces)
//!     jagged: 0.0,                  // random line indent, in sizes
//!     avoid: canvas,                // optional: see avoid.rs
//!     clearance: 6.0,               // how far from it; default: size / 8
//!     spacing: 0.1,                 // extra space between glyphs, in sizes (default 0)
//!     split_words: true,            // "words" only: break words round what's avoided, rather
//!                                   // than moving them past it whole; or the fewest glyphs
//!                                   // a piece may have (true is 2)
//!     split_padding: 0.3,           // pieces keep this much further from it, in sizes
//!     position_jitter: 0.05,        // each placed glyph off true (see text_settings.rs)
//! });
//!
//! // Or fill any closed shape, on lines at an angle (radians; default 0,
//! // left to right, each line below the last). Each subpath of the path is
//! // an outline, closed back to its start; one inside another is a hole.
//! // A rectangle shape at angle 0 fills just as center/width/height do.
//! let round = new_textbox(#{ shape: new_circle(b.center, 300.0), angle: 0.3, size: 20.0 });
//!
//! let res = tb.fill(font.glyphs(msg));   // #{ placed: [..], unplaced: [..] }
//! let blocks = tb.place_all(glyphs);     // one array per filled box
//!
//! // Text wrapped to a width from a point, line after line: #{ canvas, bottom },
//! // bottom being where the next block goes. With path_jitter, each line
//! // wanders up and down from straight by up to that many sizes.
//! let block = write_block(msg, vec2(0, 0), 400.0, #{ font: font, size: 20.0, style: ink, line_height: 1.3 });
//! let wavy = write_block(msg, vec2(0, 0), 400.0, #{ font: font, size: 20.0, style: ink, path_jitter: 0.2 });
//!
//! // Scatter messages over a canvas in random boxes, clear of its strokes
//! // and of each other, box sizes tried between the ranges. Returns a
//! // canvas with just the messages.
//! let notes = page_canvas.scatter_notes(["first note", "second note"], [200, 500], [300, 600], #{
//!     font: font, style: style, size: 23.0,
//!     rotation_jitter: 0.196,     // radians either way, before placing (default pi/16)
//!     scale_jitter: 0.15,         // fraction either way, before placing (default 0.15)
//!     jagged: 0.1,                // default 0.1
//!     max_attempts: 100,          // boxes per message before backing off
//!     spacing: 0.0,               // extra space between glyphs, in sizes
//!     avoid: other,               // more to keep clear of (see avoid.rs)
//!     clearance: 3.0,             // from the canvas, `avoid` and other messages; default size / 8
//!     layout: "words",            // words whole, each note in one clear patch; default "flat", glyph by glyph
//! });
//!
//! ```
//!
//! Placement scales each glyph by `size` and turns it to the line,
//! on top of any scale and rotation it already has. Jagged edges come from
//! the engine's seeded generator.

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2 as PVec;
use rand::Rng;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};
use std::rc::Rc;

use super::glyphs::{add_glyph, glyph_array, Glyph};
use super::random::SharedRng;
use super::{Canvas, CanvasData, Path, Vec2};
use crate::journal::canvas::artistic::{jitter_symbols, scatter_notes, ScatterOptions};
use crate::journal::fonts::FontBook;
use crate::journal::typeset::Align;
use crate::journal::canvas::textbox::{rectangle, TextBox as CoreTextBox, TextBoxLineMode, TextBoxOptions};
use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::symbol::{Symbol, WordSplit};
use super::avoid::{Avoid, AvoidOption};
use super::options::{number, positive, range};
use super::text_settings::{self, Jitter, Wander};

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
    jitter: Jitter,
}

const TEXTBOX_KEYS: [&str; 18] = ["center", "width", "height", "shape", "angle", "size", "layout", "jagged", "avoid", "clearance", "spacing", "align", "valign", "split_words", "split_padding", "position_jitter", "rotation_jitter", "scale_jitter"];

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
        let options = text_settings::settings(CALLER, options, &TEXTBOX_KEYS)?;
        let symbol_scale = text_settings::size(CALLER, &options)?;
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
        let jagged_edge = number(CALLER, &options, "jagged")?.unwrap_or(0.0);
        let layout = line_mode(CALLER, &options)?;
        let avoid = AvoidOption::parse(CALLER, &options, false)?;
        let align = match options.get("align") {
            None => Align::Left,
            Some(value) => {
                let name = value.clone().into_string().map_err(|t| format!("{}: 'align' should be a string, got {}", CALLER, t))?;
                Align::from_name(&name).ok_or_else(|| format!("{}: unknown align '{}'; aligns: {}", CALLER, name, Align::NAMES.join(", ")))?
            }
        };
        let valign = match options.get("valign") {
            None => 0.0,
            Some(value) => match value.clone().into_string().as_deref() {
                Ok("top") => 0.0,
                Ok("center") => 0.5,
                Ok("bottom") => 1.0,
                _ => return Err(format!("{}: 'valign' should be top, center or bottom, got {}", CALLER, value).into()),
            },
        };
        Ok(TextBox {
            shape: Rc::new(shape),
            angle,
            symbol_scale,
            jagged_edge,
            layout,
            padding: avoid.padding(symbol_scale),
            avoid: avoid.tree,
            spacing: spacing(CALLER, &options)?,
            align,
            contain: true,
            valign,
            jitter: Jitter::parse(CALLER, &options)?,
        })
    }
    /// Fill the box with what fits, jittering what's placed.
    fn fill(&self, glyphs: Vec<Symbol>, rng: &mut dyn rand::RngCore) -> (Vec<Symbol>, Vec<Symbol>) {
        let (mut placed, unplaced) = self.with_core(|core| core.fill_bounds_with(glyphs, rng));
        self.jitter.apply(&mut placed, self.symbol_scale, rng);
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
            self.jitter.apply(block, self.symbol_scale, rng);
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

const SCATTER_KEYS: [&str; 14] = ["font", "size", "style", "rotation_jitter", "scale_jitter", "position_jitter", "jagged", "max_attempts", "spacing", "avoid", "clearance", "layout", "split_words", "split_padding"];

/// `canvas.scatter_notes(messages, width_range, height_range, settings)`.
fn splat(canvas: &Canvas, msgs: Array, width_range: Dynamic, height_range: Dynamic, settings: Map, symdict: &FontBook, rng: &SharedRng) -> RhaiResult<Canvas> {
    const CALLER: &str = "scatter_notes";
    let options = text_settings::settings(CALLER, settings, &SCATTER_KEYS)?;
    let font = text_settings::font(CALLER, &options)?;
    let style = text_settings::style(CALLER, &options)?;
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
    let symbol_scale = text_settings::size(CALLER, &options)?;
    let avoid = AvoidOption::parse(CALLER, &options, true)?;
    let ranges = Map::from_iter([("width_range".into(), width_range), ("height_range".into(), height_range)]);
    let opts = ScatterOptions {
        symbol_scale,
        layout: line_mode(CALLER, &options)?,
        clearance: avoid.padding(symbol_scale),
        width_range: range(CALLER, &ranges, "width_range")?,
        height_range: range(CALLER, &ranges, "height_range")?,
        jagged_edge: number(CALLER, &options, "jagged")?.unwrap_or(0.1),
        max_attempts,
        spacing: spacing(CALLER, &options)?,
    };
    // Notes turn and scale before they're placed, by default a little; any
    // position jitter is after.
    let rotation_jitter = number(CALLER, &options, "rotation_jitter")?.unwrap_or(std::f32::consts::PI / 16.0);
    let scale_jitter = number(CALLER, &options, "scale_jitter")?.unwrap_or(0.15);
    if !(rotation_jitter >= 0.0) || !(0.0..1.0).contains(&scale_jitter) {
        return Err(format!("{}: 'rotation_jitter' can't be negative and 'scale_jitter' must be in [0, 1)", CALLER).into());
    }
    let mut after = options.clone();
    after.remove("rotation_jitter");
    after.remove("scale_jitter");
    let after = Jitter::parse(CALLER, &after)?;
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
    let mut placed: Vec<Symbol> = placed.into_iter().flatten().collect();
    after.apply(&mut placed, symbol_scale, &mut *rng);
    for symbol in placed {
        add_glyph(&mut splatted, &Glyph::new(symbol), &style)?;
    }
    Ok(Canvas::new(splatted))
}

const BLOCK_KEYS: [&str; 15] = ["font", "size", "style", "line_height", "line_jitter", "path_jitter", "jagged", "spacing", "layout", "avoid", "clearance", "split_words", "split_padding", "position_jitter", "rotation_jitter"];

/// `write_block(text, at, width, settings)`: the text wrapped to lines
/// `width` long, the first from `at` (on its centre line, as write_line
/// sets it), each `line_height` sizes below the last (default 1.25), give
/// or take up to `line_jitter` sizes at random (default 0), each line
/// wandering up and down from straight by up to `path_jitter` sizes
/// (default 0), words kept whole unless `layout` is "flat". Gives `#{ canvas, bottom }`,
/// `bottom` being where a line after them would go.
fn write_block(msg: &str, at: Vec2, width: Dynamic, settings: Map, symdict: &FontBook, rng: &SharedRng) -> RhaiResult<Map> {
    const CALLER: &str = "write_block";
    let mut keys = BLOCK_KEYS.to_vec();
    keys.push("scale_jitter");
    let mut options = text_settings::settings(CALLER, settings, &keys)?;
    let font = text_settings::font(CALLER, &options)?;
    let style = text_settings::style(CALLER, &options)?;
    let size = text_settings::size(CALLER, &options)?;
    let width = super::options::float(CALLER, "the width", &width)? as f32;
    if !(width > 0.0) {
        return Err(format!("{}: the width must be positive, got {}", CALLER, width).into());
    }
    let line_height = match number(CALLER, &options, "line_height")? {
        None => 1.25,
        Some(h) if h > 0.0 && h.is_finite() => h,
        Some(h) => return Err(format!("{}: 'line_height' must be positive, got {}", CALLER, h).into()),
    };
    let line_jitter = match number(CALLER, &options, "line_jitter")? {
        None => 0.0,
        Some(j) if j >= 0.0 && j.is_finite() => j,
        Some(j) => return Err(format!("{}: 'line_jitter' must be a number of at least 0, got {}", CALLER, j).into()),
    };
    let path_jitter = text_settings::path_jitter(CALLER, &options)?;
    options.entry("layout".into()).or_insert_with(|| "words".into());
    let at: PVec = at.into();
    // A box one line high, filled again and again: a line each.
    let mut box_options = Map::new();
    for key in ["jagged", "spacing", "layout", "avoid", "clearance", "split_words", "split_padding", "position_jitter", "rotation_jitter", "scale_jitter"] {
        if let Some(v) = options.get(key) {
            box_options.insert(key.into(), v.clone());
        }
    }
    box_options.insert("size".into(), Dynamic::from(size as f64));
    box_options.insert("width".into(), Dynamic::from(width as f64));
    box_options.insert("height".into(), Dynamic::from((size * 0.5) as f64));
    // A box one line high, at the line's own height, so what it avoids is
    // avoided where the line is: a line each.
    let line_box = |y: f32| -> RhaiResult<TextBox> {
        let mut options = box_options.clone();
        options.insert("center".into(), Dynamic::from(Vec2::new((at.x + width / 2.0) as f64, y as f64)));
        let mut line = TextBox::new(options).map_err(|e| -> Box<EvalAltResult> { format!("{}", e).replace("new_textbox", CALLER).into() })?;
        line.contain = false;
        Ok(line)
    };
    let mut r = rng.borrow_mut();
    let mut to_place = font.build(msg, symdict, &mut *r);
    let pitch = line_height * size;
    let mut canvas = CanvasData::default();
    // How far lines have strayed from their even spacing, all told.
    let mut drift = 0.0;
    let mut lines = 0;
    while !to_place.is_empty() {
        if lines > 0 && line_jitter > 0.0 {
            drift += r.random_range(-line_jitter..line_jitter) * size;
        }
        let tb = line_box(at.y + lines as f32 * pitch + drift)?;
        let (mut row, rest) = tb.with_core(|core| core.fill_bounds_with(std::mem::take(&mut to_place), &mut *r));
        if row.is_empty() {
            return Err(format!("{}: {} glyph(s) don't fit a line {} wide", CALLER, rest.len(), width).into());
        }
        to_place = rest;
        lines += 1;
        if path_jitter > 0.0 {
            // A little past the width: glyphs can reach over it.
            Wander::new(path_jitter, size, width + size * 2.0, &mut *r).bend(&mut row, at.x);
        }
        tb.jitter.apply(&mut row, size, &mut *r);
        for g in row.iter() {
            add_glyph(&mut canvas, &Glyph::new(g.clone()), &style)?;
        }
    }
    let rows = lines;
    let mut out = Map::new();
    out.insert("canvas".into(), Dynamic::from(Canvas::new(canvas)));
    out.insert("bottom".into(), Dynamic::from((at.y + rows as f32 * pitch + drift) as f64));
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
    engine.register_fn("write_block", move |text: &str, at: Vec2, width: Dynamic, settings: Map| -> RhaiResult<Map> {
        write_block(text, at, width, settings, &symdict, &r)
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
            let style = new_style("black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
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
    fn write_block_line_jitter_moves_lines_off_even() {
        let ok: bool = eval(r#"
            let hand = #{ font: font, size: 10.0, style: style };
            let even = write_block(text, vec2(0, 0), 100.0, hand);
            let lines = (even.bottom / 12.5).round();
            if (even.bottom - lines * 12.5).abs() > 1e-3 { throw "even lines should be line_height apart: " + even.bottom; }
            let jittered = write_block(text, vec2(0, 0), 100.0, hand + #{ line_jitter: 0.3 });
            if (jittered.bottom - even.bottom).abs() < 1e-3 { throw "line_jitter should move the lines"; }
            if (jittered.bottom - even.bottom).abs() > lines * 3.0 { throw "lines strayed too far: " + jittered.bottom; }
            true
        "#);
        assert!(ok);
        assert!(fails(r#"write_block("a", vec2(0, 0), 100.0, #{ font: font("loche_upper"), size: 10.0, style: new_style("black", new_brush("Ink Pen", 0.3, 0.7, 1.0)), line_jitter: -1 })"#).contains("line_jitter"));
    }

    #[test]
    fn path_jitter_bends_lines_within_bounds() {
        let ok: bool = eval(r#"
            let hand = #{ font: font, size: 10.0, style: style };
            seed(3);
            let even = write_block(text, vec2(0, 0), 100.0, hand);
            seed(3);
            let wavy = write_block(text, vec2(0, 0), 100.0, hand + #{ path_jitter: 0.3 });
            if (wavy.bottom - even.bottom).abs() > 1e-3 { throw "path_jitter shouldn't move where the next block goes"; }
            let e = even.canvas.bounds();
            let w = wavy.canvas.bounds();
            if (e.height - w.height).abs() < 1e-3 { throw "path_jitter should bend the lines"; }
            if (e.height - w.height).abs() > 2.0 * 3.0 + 3.0 { throw "lines bent too far: " + w.height; }
            seed(3);
            let line = write_line("the quick brown fox", vec2(0, 0), hand);
            seed(3);
            let bent = write_line("the quick brown fox", vec2(0, 0), hand + #{ path_jitter: 0.3 });
            let l = line.bounds();
            let b = bent.bounds();
            if (l.height - b.height).abs() < 1e-3 { throw "write_line's path_jitter should bend the line"; }
            if b.min.y < l.min.y - 3.5 || b.max.y > l.max.y + 3.5 { throw "the line bent too far"; }
            true
        "#);
        assert!(ok);
        for script in [
            r#"write_block("a", vec2(0, 0), 100.0, #{ font: font("loche_upper"), size: 10.0, style: new_style("black", new_brush("Ink Pen", 0.3, 0.7, 1.0)), path_jitter: -1 })"#,
            r#"write_line("a", vec2(0, 0), #{ font: font("loche_upper"), size: 10.0, style: new_style("black", new_brush("Ink Pen", 0.3, 0.7, 1.0)), path_jitter: -1 })"#,
        ] {
            assert!(fails(script).contains("'path_jitter' must be a number of at least 0"), "{}", script);
        }
    }

    #[test]
    fn fill_places_glyphs_inside_the_box_and_returns_the_rest() {
        let ok: bool = eval(r#"
            let glyphs = font.glyphs(text);
            let tb = new_textbox(#{ center: vec2(0.0, 0.0), width: 100, height: 40, size: 10.0 });
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
            let tb = new_textbox(#{ center: vec2(0.0, 0.0), width: 60, height: 30, size: 10.0, layout: "words", jagged: 0.5 });
            let blocks = tb.place_all(glyphs);
            let total = 0;
            for b in blocks { total += b.len(); }
            blocks.len() > 1 && total == glyphs.len()
        "#);
        assert!(ok);
        let err = fails(r#"
            let tb = new_textbox(#{ center: vec2(0.0, 0.0), width: 5, height: 5, size: 50.0 });
            tb.place_all(font("loche_upper").glyphs("ab"));
        "#);
        assert!(err.contains("2 glyph(s) don't fit"), "{}", err);
    }

    #[test]
    fn avoid_keeps_glyphs_off_a_canvas() {
        let counts: rhai::Array = eval(r#"
            let blot = new_canvas();
            for i in 0..10 { blot.add(new_circle(vec2(0.0, 0.0), 4.0 * i), style); }
            let opts = #{ center: vec2(0.0, 0.0), width: 100, height: 100, size: 10.0 };
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
            let bar = new_line(vec2(-160.0, -100.0), vec2(-160.0, 100.0));
            // One line high: glyphs' ink keeps inside the box.
            let opts = #{ center: vec2(0.0, 0.0), width: 400, height: 16, size: 10.0, layout: "words", avoid: bar, clearance: 2.0 };
            let whole = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            opts.split_words = true;
            let split = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            opts.split_padding = 1.0;
            let padded = new_textbox(opts).fill(font.glyphs("abcdefghijklmnop")).placed;
            opts.split_words = 8;   // the piece before the bar would be too short
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

    const SPLAT_SETUP: &str = r#"
        let style = new_style("black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
        let base = new_canvas();
        base.add(new_rectangle(vec2(0.0, 0.0), 800.0, 800.0), style);
        base.add(new_circle(vec2(0.0, 0.0), 150.0), style);
        let msgs = ["ash and oak", "a quiet note", "the last word"];
        let widths = [150, 250];
        let heights = [100, 200];
        let opts = #{ font: font("loche_upper"), style: style, size: 20.0 };
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
            opts.layout = "words";
            let notes = base.scatter_notes(msgs, widths, heights, opts);
            let page = new_page();
            page.add(notes);
            journal_entry.add(page);
        "#, SPLAT_SETUP);
        let entry = engine().build_entry_str(&script).expect("script should run");
        assert_eq!(entry.pages[0].canvases()[0].strokes.len(), 30);
        let err = fails(&format!("{}\nopts.layout = \"zigzag\";\nbase.scatter_notes(msgs, widths, heights, opts);", SPLAT_SETUP));
        assert!(err.contains("scatter_notes: unknown layout 'zigzag'"), "{}", err);
    }

    #[test]
    fn splat_fails_when_a_message_cannot_fit() {
        let err = fails(&format!(r#"{}
            opts.size = 400.0;
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
            ("opts.remove(\"font\");", "missing 'font'"),
            ("opts.max_attempts = 0;", "'max_attempts' should be a whole number"),
            ("opts.scale_jitter = 1.5;", "'scale_jitter' must be in [0, 1)"),
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
        for (opts, want) in [
            (format!("#{{ center: {}, width: 10, height: 10, size: 2, colour: 1 }}", c), "unknown option 'colour'"),
            (format!("#{{ center: {}, shape: new_circle({}, 5.0), size: 2 }}", c, c), "either a 'shape' or a 'center'"),
            ("#{ shape: 3, size: 2 }".to_string(), "'shape' should be a path, got i64"),
            (format!("#{{ shape: new_line({}, vec2(1.0, 1.0)), size: 2 }}", c), "'shape' encloses nothing"),
            (format!("#{{ center: {}, width: 10, height: 10, size: 2, angle: \"up\" }}", c), "'angle'"),
            ("#{ width: 10, height: 10, size: 2 }".to_string(), "missing 'center'"),
            (format!("#{{ center: {}, width: 0, height: 10, size: 2 }}", c), "'width' must be positive"),
            (format!("#{{ center: {}, width: 10, height: 10 }}", c), "missing 'size'"),
            (format!("#{{ center: {}, width: 10, height: 10, size: 2, layout: \"zigzag\" }}", c), "unknown layout 'zigzag'"),
            (format!("#{{ center: {}, width: 10, height: 10, size: 2, clearance: 3 }}", c), "'clearance' needs 'avoid'"),
            (format!("#{{ center: {}, width: 10, height: 10, size: 2, avoid: new_canvas(), clearance: -3 }}", c), "'clearance' must be a finite number"),
            (format!("#{{ center: {}, width: 10, height: 10, size: 2, split_words: true }}", c), "'split_words' needs layout: \"words\""),
            (format!("#{{ center: {}, width: 10, height: 10, size: 2, layout: \"words\", split_padding: 1 }}", c), "'split_padding' needs 'split_words'"),
            (format!("#{{ center: {}, width: 10, height: 10, size: 2, layout: \"words\", split_words: 0 }}", c), "'split_words' should be true, false"),
            (format!("#{{ center: {}, width: 10, height: 10, size: 2, layout: \"words\", split_words: true, split_padding: -1 }}", c), "'split_padding' must be a finite number"),
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
                let opts = #{ size: 12.0, layout: layout, jagged: 0.7, avoid: blot, spacing: 0.1 };
                let positions = [];
                for shaped in [false, true] {
                    seed(3);
                    let o = opts;
                    if shaped { o.shape = new_rectangle(vec2(3.0, -7.0), 230, 150); o.angle = 0; }
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
    fn a_shape_keeps_glyphs_inside_it_on_turned_lines() {
        let ok: bool = eval(r#"
            let tb = new_textbox(#{ shape: new_circle(vec2(0.0, 0.0), 80.0), angle: PI() / 6.0, size: 10.0 });
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
