//! Layout helpers for rhai scripts: canvas bounds, text boxes and hatching.
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
//! });
//! let res = tb.fill(font.glyphs(msg));   // #{ placed: [..], unplaced: [..] }
//! let blocks = tb.place_all(glyphs);     // one array per filled box
//!
//! // Scatter messages over a canvas in random boxes, clear of its strokes
//! // and of each other. Returns a canvas with just the messages.
//! let notes = page_canvas.splat_msgs(["first note", "second note"], font, style, #{
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
//! // Hatch the inside of a closed path: lines `spacing` apart at `angle`
//! // (radians), their ends jittered by up to `roughness`.
//! let shading = new_circle(new_vec(0.0, 0.0), 20.0).hatch(3.0, random_range(0.0, 6.28), 1.0);
//! // Or with options; only `spacing` is needed.
//! let shading = outline.hatch(#{
//!     spacing: 4.0,
//!     angle: [0.8, 2.4],            // a layer per angle: cross-hatching (default pi/4)
//!     roughness: 1.0,               // ends jittered by up to this (default 0)
//!     bow: 1.5,                     // lines bend by up to this from straight (default 0)
//!     inset: 2.0,                   // lines stop this far inside the outline (default 0)
//!     fill_rule: "holes",           // or "evenodd" or "nonzero"; see below
//! });
//! ```
//!
//! Every subpath of a hatched path is a closed shape (open ones close back
//! to their start), and each line is a stroke of its own, from where it
//! enters the shape to where it leaves. With the default fill rule,
//! "holes", shapes inside or across others cut holes in them: two circles
//! one inside the other hatch as a ring, and glyph outlines keep their
//! counters. "evenodd" also leaves out where a single subpath crosses over
//! itself (the middle of a star drawn in one line); "nonzero" merges
//! overlapping shapes, as SVG fills do.
//!
//! Placement scales each glyph by `symbol_scale` and turns it to the line,
//! on top of any scale and rotation it already has. Jagged edges and hatch
//! roughness and bows come from the engine's seeded generator.

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2 as PVec;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};
use std::rc::Rc;

use super::glyphs::{add_glyph, glyph_array, Glyph};
use super::random::SharedRng;
use super::{Canvas, CanvasData, Path, StrokeStyle, Vec2};
use crate::journal::canvas::artistic::{jitter_symbols, splat_msgs, SplatOptions};
use crate::journal::styles::Font;
use crate::journal::symboldict::SymbolPathDict;
use crate::journal::canvas::textbox::{TextBox as CoreTextBox, TextBoxLineMode, TextBoxOptions};
use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::paths::StrokePath;
use crate::journal::hatch::{FillRule, HatchOptions};
use crate::journal::symbol::Symbol;
use super::avoid::{Avoid, AvoidOption};

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

fn vec(v: PVec) -> Dynamic {
    Dynamic::from(Vec2::new(v.x as f64, v.y as f64))
}

fn bounds(canvas: &mut Canvas) -> RhaiResult<Map> {
    let jc: JournalCanvas = (&*canvas).into();
    let b = jc.bounds();
    if !(b.mins.x <= b.maxs.x && b.mins.y <= b.maxs.y) {
        return Err("bounds: the canvas is empty".into());
    }
    let extents = b.extents();
    let mut map = Map::new();
    map.insert("min".into(), vec(b.mins));
    map.insert("max".into(), vec(b.maxs));
    map.insert("center".into(), vec(b.center()));
    map.insert("width".into(), Dynamic::from(extents.x as f64));
    map.insert("height".into(), Dynamic::from(extents.y as f64));
    Ok(map)
}

/// A box that glyphs flow into, line by line.
#[derive(Clone)]
pub struct TextBox {
    bounds: Aabb,
    symbol_scale: f32,
    jagged_edge: f32,
    layout: TextBoxLineMode,
    avoid: Option<Avoid>,
    padding: f32,
    spacing: f32,
}

const TEXTBOX_KEYS: [&str; 9] = ["center", "width", "height", "symbol_scale", "layout", "jagged_edge", "avoid", "clearance", "spacing"];

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
/// "words" keeps words together, wrapping at blanks.
pub(super) fn line_mode(caller: &str, options: &Map) -> RhaiResult<TextBoxLineMode> {
    match options.get("layout").map(|l| l.to_string()).as_deref() {
        None | Some("flat") => Ok(TextBoxLineMode::Flat),
        Some("words") => Ok(TextBoxLineMode::WordsByBlankSymbol),
        Some(other) => Err(format!("{}: unknown layout '{}'; expected \"flat\" or \"words\"", caller, other).into()),
    }
}

pub(super) fn check_keys(caller: &str, options: &Map, known: &[&str]) -> RhaiResult<()> {
    match options.keys().find(|k| !known.contains(&k.as_str())) {
        Some(key) => Err(format!("{}: unknown option '{}'; options: {}", caller, key, known.join(", ")).into()),
        None => Ok(()),
    }
}

fn as_number(caller: &str, what: &str, v: &Dynamic) -> RhaiResult<f32> {
    v.as_float().map(|f| f as f32)
        .or_else(|_| v.as_int().map(|i| i as f32))
        .map_err(|type_name| format!("{}: {} should be a number, got {}", caller, what, type_name).into())
}

/// A number option, accepting ints as well as floats.
pub(super) fn number(caller: &str, options: &Map, key: &str) -> RhaiResult<Option<f32>> {
    options.get(key).map(|v| as_number(caller, &format!("'{}'", key), v)).transpose()
}

fn positive(caller: &str, options: &Map, key: &str) -> RhaiResult<f32> {
    match number(caller, options, key)? {
        Some(n) if n > 0.0 => Ok(n),
        Some(n) => Err(format!("{}: '{}' must be positive, got {}", caller, key, n).into()),
        None => Err(format!("{}: missing '{}'", caller, key).into()),
    }
}

/// A required `[low, high]` option of positive numbers.
fn range(caller: &str, options: &Map, key: &str) -> RhaiResult<(f32, f32)> {
    let bad = || -> Box<EvalAltResult> { format!("{}: '{}' should be [low, high], two positive numbers with low <= high", caller, key).into() };
    let pair = options.get(key)
        .ok_or_else(|| -> Box<EvalAltResult> { format!("{}: missing '{}'", caller, key).into() })?
        .clone().into_array().map_err(|_| bad())?;
    if pair.len() != 2 {
        return Err(bad());
    }
    let (low, high) = (as_number(caller, &format!("'{}'", key), &pair[0])?, as_number(caller, &format!("'{}'", key), &pair[1])?);
    if !(low > 0.0 && low <= high) {
        return Err(bad());
    }
    Ok((low, high))
}

impl TextBox {
    fn new(options: Map) -> RhaiResult<TextBox> {
        const CALLER: &str = "new_textbox";
        check_keys(CALLER, &options, &TEXTBOX_KEYS)?;
        let center: PVec = options.get("center")
            .ok_or("new_textbox: missing 'center'")?
            .clone().try_cast::<Vec2>()
            .ok_or("new_textbox: 'center' should be a vec")?
            .into();
        let half = PVec::new(positive(CALLER, &options, "width")?, positive(CALLER, &options, "height")?) / 2.0;
        let symbol_scale = positive(CALLER, &options, "symbol_scale")?;
        let jagged_edge = number(CALLER, &options, "jagged_edge")?.unwrap_or(0.0);
        let layout = line_mode(CALLER, &options)?;
        let avoid = AvoidOption::parse(CALLER, &options, false)?;
        Ok(TextBox {
            bounds: Aabb::new(center - half, center + half),
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
            bounds: self.bounds,
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

const HATCH_KEYS: [&str; 6] = ["spacing", "angle", "roughness", "bow", "inset", "fill_rule"];

/// `path.hatch(#{ spacing, angle, roughness, bow, inset, fill_rule })`;
/// see the module docs.
fn hatch_options(options: &Map) -> RhaiResult<HatchOptions> {
    const CALLER: &str = "hatch";
    check_keys(CALLER, options, &HATCH_KEYS)?;
    let spacing = positive(CALLER, options, "spacing")?;
    let angles = match options.get("angle") {
        None => vec![std::f32::consts::FRAC_PI_4],
        Some(angle) if angle.is_array() => {
            let angles = angle.clone().into_array().expect("checked").iter()
                .map(|a| as_number(CALLER, "each 'angle'", a))
                .collect::<RhaiResult<Vec<_>>>()?;
            if angles.is_empty() {
                return Err(format!("{}: 'angle' needs at least one angle", CALLER).into());
            }
            angles
        }
        Some(angle) => vec![as_number(CALLER, "'angle'", angle)?],
    };
    let at_least_zero = |key: &str| -> RhaiResult<f32> {
        match number(CALLER, options, key)? {
            None => Ok(0.0),
            Some(n) if n >= 0.0 && n.is_finite() => Ok(n),
            Some(n) => Err(format!("{}: '{}' can't be negative, got {}", CALLER, key, n).into()),
        }
    };
    let fill_rule = match options.get("fill_rule") {
        None => FillRule::default(),
        Some(rule) => {
            let name = rule.to_string();
            FillRule::from_name(&name).ok_or_else(|| -> Box<EvalAltResult> {
                format!("{}: unknown fill_rule '{}'; expected one of {}", CALLER, name, FillRule::NAMES.join(", ")).into()
            })?
        }
    };
    Ok(HatchOptions {
        spacing,
        angles,
        roughness: at_least_zero("roughness")?,
        bow: at_least_zero("bow")?,
        inset: at_least_zero("inset")?,
        fill_rule,
    })
}

/// The hatching of a path's inside, as a new path; the path itself is
/// unchanged.
fn hatch(path: &StrokePath, options: &HatchOptions, rng: &SharedRng) -> RhaiResult<StrokePath> {
    if !options.angles.iter().all(|a| a.is_finite()) {
        return Err("hatch: angles must be finite numbers".into());
    }
    Ok(crate::journal::hatch::hatch(path, options, &mut *rng.borrow_mut()))
}

const SPLAT_KEYS: [&str; 11] = ["symbol_scale", "width_range", "height_range", "rotation_jitter", "scale_jitter", "jagged_edge", "max_attempts", "spacing", "avoid", "clearance", "layout"];

fn splat(canvas: &Canvas, msgs: Array, font: &Font, style: StrokeStyle, options: Map, symdict: &SymbolPathDict, rng: &SharedRng) -> RhaiResult<Canvas> {
    const CALLER: &str = "splat_msgs";
    check_keys(CALLER, &options, &SPLAT_KEYS)?;
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
    let opts = SplatOptions {
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

    let builder = font.builder(symdict);
    let mut rng = rng.borrow_mut();
    let symbol_msgs: Vec<Vec<Symbol>> = msgs.iter().map(|msg| {
        let mut symbols = builder.build(msg, &mut *rng);
        jitter_symbols(&mut symbols, rotation_jitter, scale_jitter, &mut *rng);
        symbols
    }).collect();
    let journal_canvas: JournalCanvas = canvas.into();
    let tree = avoid.tree.as_ref().map(|a| a.borrow());
    let placed = splat_msgs(&journal_canvas, tree.as_deref(), &symbol_msgs, &opts, &mut *rng)
        .map_err(|err| format!("{}: {}", CALLER, err))?;

    let mut splatted = CanvasData::default();
    for symbol in placed.into_iter().flatten() {
        add_glyph(&mut splatted, &Glyph::new(symbol), &style)?;
    }
    Ok(Canvas::new(splatted))
}

pub fn register(engine: &mut Engine, symdict: Rc<SymbolPathDict>, rng: SharedRng) {
    engine.register_type_with_name::<TextBox>("TextBox");
    engine.register_fn("bounds", bounds);
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
    engine.register_fn("splat_msgs", move |canvas: &mut Canvas, msgs: Array, font: Font, style: StrokeStyle, options: Map| -> RhaiResult<Canvas> {
        splat(canvas, msgs, &font, style, options, &symdict, &r)
    });

    let r = rng.clone();
    engine.register_fn("hatch", move |path: &mut Path, spacing: f64, angle: f64, roughness: f64| -> RhaiResult<Path> {
        let mut options = Map::new();
        options.insert("spacing".into(), Dynamic::from(spacing));
        options.insert("angle".into(), Dynamic::from(angle));
        options.insert("roughness".into(), Dynamic::from(roughness));
        hatch(&path.borrow(), &hatch_options(&options)?, &r).map(Path::new)
    });
    engine.register_fn("hatch", move |path: &mut Path, options: Map| -> RhaiResult<Path> {
        hatch(&path.borrow(), &hatch_options(&options)?, &rng).map(Path::new)
    });
}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use crate::journal::symboldict::SymbolPathDict;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        let symd = SymbolPathDict::from_file(crate::journal::symboldict::DEFAULT_SYMBOLS).expect("Failed to load symbols.json");
        JournalEngine::new(Rc::new(symd))
    }

    /// Run `script` after some shared setup, returning its value.
    fn eval<T: Clone + 'static>(script: &str) -> T {
        let setup = r#"
            let style = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = lowercase_font("loche_upper");
            let text = "the quick brown fox jumps over the lazy dog and keeps on running ";
            text = text + text + text + text;
        "#;
        engine().engine.eval::<T>(&format!("{}\n{}", setup, script)).expect("script should evaluate")
    }

    fn fails(script: &str) -> String {
        engine().engine.run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn bounds_include_stroke_width() {
        let ok: bool = eval(r#"
            let c = new_canvas();
            c.add_strokepath(new_line(new_vec(0.0, 0.0), new_vec(10.0, 20.0)), style);
            let b = c.bounds();
            b.min.x == -1.0 && b.min.y == -1.0 && b.max.x == 11.0 && b.max.y == 21.0
                && b.center.x == 5.0 && b.center.y == 10.0 && b.width == 12.0 && b.height == 22.0
        "#);
        assert!(ok);
        assert!(fails("new_canvas().bounds()").contains("bounds: the canvas is empty"));
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
            tb.place_all(lowercase_font("loche_upper").glyphs("ab"));
        "#);
        assert!(err.contains("2 glyph(s) don't fit"), "{}", err);
    }

    #[test]
    fn avoid_keeps_glyphs_off_a_canvas() {
        let counts: rhai::Array = eval(r#"
            let blot = new_canvas();
            for i in 0..10 { blot.add_strokepath(new_circle(new_vec(0.0, 0.0), 4.0 * i), style); }
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
    fn hatch_fills_inside_the_path_and_follows_the_seed() {
        let hatching = |seed: i64| -> String {
            let path: super::Path = eval(&format!(
                "seed({}); new_circle(new_vec(0.0, 0.0), 20.0).hatch(3.0, 0.7, 1.0)", seed));
            let path = path.get();
            for p in path.points_within() {
                assert!(p.length() < 22.0, "hatch point {:?} outside the circle", p);
            }
            path.to_string()
        };
        assert!(!hatching(1).is_empty());
        assert_eq!(hatching(1), hatching(1));
        assert_ne!(hatching(1), hatching(2));
    }

    #[test]
    fn hatch_options_cross_hatch_and_keep_holes() {
        let lines = |options: &str| -> usize {
            let path: super::super::Path = eval(&format!(
                "let ring = new_circle(new_vec(0.0, 0.0), 20.0); ring.append(new_circle(new_vec(0.0, 0.0), 10.0)); ring.hatch({})", options));
            let hatching = path.get();
            for (points, _) in hatching.flattened(0.1) {
                let middle = (points[0] + *points.last().unwrap()) / 2.0;
                assert!(middle.length() > 9.0, "a line crosses the hole");
            }
            hatching.to_substrokes().len()
        };
        let single = lines("#{ spacing: 3.0, angle: 0.3 }");
        assert_eq!(lines("#{ spacing: 3.0, angle: [0.3, 1.9] }"), 2 * single);
        assert_eq!(lines("3.0, 0.3, 0.0"), single);
    }

    const SPLAT_SETUP: &str = r#"
        let style = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
        let base = new_canvas();
        base.add_strokepath(new_rectangle(new_vec(0.0, 0.0), 800.0, 800.0), style);
        base.add_strokepath(new_circle(new_vec(0.0, 0.0), 150.0), style);
        let msgs = ["ash and oak", "a quiet note", "the last word"];
        let opts = #{ symbol_scale: 20.0, width_range: [150, 250], height_range: [100, 200] };
    "#;

    fn splat_glyph_positions(seed: i64) -> Vec<(f32, f32)> {
        let script = format!(r#"{}
            seed({});
            let notes = base.splat_msgs(msgs, lowercase_font("loche_upper"), style, opts);
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
            let notes = base.splat_msgs(msgs, lowercase_font("loche_upper"), style, opts);
            let page = new_page();
            page.add_canvas(notes);
            journal_entry.add_page(page);
        "#, SPLAT_SETUP);
        let entry = engine().build_entry_str(&script).expect("script should run");
        assert_eq!(entry.pages[0].canvases()[0].strokes.len(), 30);
        let err = fails(&format!("{}\nopts.layout = \"zigzag\";\nbase.splat_msgs(msgs, lowercase_font(\"loche_upper\"), style, opts);", SPLAT_SETUP));
        assert!(err.contains("splat_msgs: unknown layout 'zigzag'"), "{}", err);
    }

    #[test]
    fn splat_fails_when_a_message_cannot_fit() {
        let err = fails(&format!(r#"{}
            opts.symbol_scale = 400.0;
            opts.max_attempts = 5;
            base.splat_msgs(msgs, lowercase_font("loche_upper"), style, opts);
        "#, SPLAT_SETUP));
        assert!(err.contains("splat_msgs: message 0 doesn't fit in 5 tries"), "{}", err);
    }

    #[test]
    fn splat_needs_something_drawn() {
        let err = fails(&format!("{}\nnew_canvas().splat_msgs(msgs, lowercase_font(\"loche_upper\"), style, opts);", SPLAT_SETUP));
        assert!(err.contains("splat_msgs: the canvas is empty"), "{}", err);
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
            let err = fails(&format!("{}\n{}\nbase.splat_msgs(msgs, lowercase_font(\"loche_upper\"), style, opts);", SPLAT_SETUP, change));
            assert!(err.contains(want) && err.contains("line 10"), "{}: {}", want, err);
        }
    }

    #[test]
    fn bad_options_fail_at_their_line() {
        let c = "new_vec(0.0, 0.0)";
        for (opts, want) in [
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, colour: 1 }}", c), "unknown option 'colour'"),
            ("#{ width: 10, height: 10, symbol_scale: 2 }".to_string(), "missing 'center'"),
            (format!("#{{ center: {}, width: 0, height: 10, symbol_scale: 2 }}", c), "'width' must be positive"),
            (format!("#{{ center: {}, width: 10, height: 10 }}", c), "missing 'symbol_scale'"),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, layout: \"zigzag\" }}", c), "unknown layout 'zigzag'"),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, clearance: 3 }}", c), "'clearance' needs 'avoid'"),
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, avoid: new_canvas(), clearance: -3 }}", c), "'clearance' must be a finite number"),
        ] {
            let err = fails(&format!("let ok = 1;\nnew_textbox({});", opts));
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
        let err = fails("let ok = 1;\nnew_circle(new_vec(0.0, 0.0), 5.0).hatch(0.0, 0.0, 1.0);");
        assert!(err.contains("'spacing' must be positive") && err.contains("line 2"), "{}", err);
        for (opts, want) in [
            ("#{ angle: 1.0 }", "missing 'spacing'"),
            ("#{ spacing: 2, angle: [] }", "'angle' needs at least one angle"),
            ("#{ spacing: 2, angle: [1.0, \"x\"] }", "each 'angle' should be a number"),
            ("#{ spacing: 2, bow: -1 }", "'bow' can't be negative"),
            ("#{ spacing: 2, fill_rule: \"winding\" }", "unknown fill_rule 'winding'"),
            ("#{ spacing: 2, colour: 1 }", "unknown option 'colour'"),
        ] {
            let err = fails(&format!("let ok = 1;\nnew_circle(new_vec(0.0, 0.0), 5.0).hatch({});", opts));
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }
}
