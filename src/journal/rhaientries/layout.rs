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
//!     avoid: canvas,                // optional
//!     avoid_radius: 26.0,           // default: symbol_scale / 2
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
//! });
//!
//! // Hatch the inside of a closed path.
//! let shading = new_circle(new_vec(0.0, 0.0), 20.0).hatch(3.0, random_range(0.0, 6.28), 3.0);
//! ```
//!
//! Placement scales each glyph by `symbol_scale` and turns it to the line,
//! on top of any scale and rotation it already has. Jagged edges and hatch
//! roughness come from the engine's seeded generator.

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2 as PVec;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};
use std::rc::Rc;

use super::glyphs::{add_glyph, glyph_array, Glyph};
use super::random::SharedRng;
use super::{Canvas, StrokeStyle, Vec2};
use crate::journal::canvas::artistic::{jitter_symbols, splat_msgs, SplatOptions};
use crate::journal::canvas::StrokeStyle as JournalStrokeStyle;
use crate::journal::styles::Font;
use crate::journal::symboldict::SymbolPathDict;
use crate::journal::canvas::textbox::{TextBox as CoreTextBox, TextBoxLineMode, TextBoxOptions};
use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::paths::StrokePath;
use crate::journal::symbol::{AvoidanceKdTree, Symbol};

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
    avoid: Option<Rc<AvoidanceKdTree>>,
}

const TEXTBOX_KEYS: [&str; 8] = ["center", "width", "height", "symbol_scale", "layout", "jagged_edge", "avoid", "avoid_radius"];

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
        let layout = match options.get("layout").map(|l| l.to_string()).as_deref() {
            None | Some("flat") => TextBoxLineMode::Flat,
            Some("words") => TextBoxLineMode::WordsByBlankSymbol,
            Some(other) => return Err(format!("new_textbox: unknown layout '{}'; expected \"flat\" or \"words\"", other).into()),
        };
        let avoid = match options.get("avoid") {
            None => None,
            Some(canvas) => {
                let canvas = canvas.clone().try_cast::<Canvas>().ok_or("new_textbox: 'avoid' should be a canvas")?;
                let radius = match number(CALLER, &options, "avoid_radius")? {
                    Some(r) if r > 0.0 => r,
                    Some(r) => return Err(format!("new_textbox: 'avoid_radius' must be positive, got {}", r).into()),
                    None => symbol_scale / 2.0,
                };
                let jc: JournalCanvas = canvas.into();
                Some(Rc::new(jc.get_avoid_tree(radius)))
            }
        };
        if avoid.is_none() && options.contains_key("avoid_radius") {
            return Err("new_textbox: 'avoid_radius' needs 'avoid'".into());
        }
        Ok(TextBox {
            bounds: Aabb::new(center - half, center + half),
            symbol_scale,
            jagged_edge,
            layout,
            avoid,
        })
    }
    fn core(&self) -> CoreTextBox<'_> {
        CoreTextBox::new(TextBoxOptions {
            jagged_edge: self.jagged_edge,
            layout: self.layout.clone(),
            bounds: self.bounds,
            symbol_scale: self.symbol_scale,
            avoid_tree: self.avoid.as_deref(),
        })
    }
}

fn symbols(caller: &str, glyphs: Array) -> RhaiResult<Vec<Symbol>> {
    Ok(glyph_array(caller, glyphs)?.into_iter().map(|g| g.0).collect())
}

fn glyphs(symbols: Vec<Symbol>) -> Array {
    symbols.into_iter().map(|s| Dynamic::from(Glyph(s))).collect()
}

/// Hatch the inside of each subpath with lines `spacing` apart at `angle`
/// (radians), their ends jittered by up to `roughness`. Returns the hatching
/// as a new path; the path itself is unchanged.
fn hatch(path: &StrokePath, spacing: f64, angle: f64, roughness: f64, rng: &SharedRng) -> RhaiResult<StrokePath> {
    if !(spacing > 0.0) {
        return Err(format!("hatch: spacing must be positive, got {}", spacing).into());
    }
    if !(roughness >= 0.0) {
        return Err(format!("hatch: roughness can't be negative, got {}", roughness).into());
    }
    let mut hatching = StrokePath::new();
    for substroke in path.to_substrokes() {
        // Too short to enclose anything (and ray_stroke_fill needs samples).
        if substroke.get_point_samples(spacing as f32).len() < 3 {
            continue;
        }
        hatching.append(substroke.ray_stroke_fill(spacing as f32, angle as f32, roughness as f32, &mut *rng.borrow_mut()));
    }
    Ok(hatching)
}

const SPLAT_KEYS: [&str; 7] = ["symbol_scale", "width_range", "height_range", "rotation_jitter", "scale_jitter", "jagged_edge", "max_attempts"];

fn splat(canvas: &Canvas, msgs: Array, font: &Font, style: StrokeStyle, options: Map, symdict: &SymbolPathDict, rng: &SharedRng) -> RhaiResult<Canvas> {
    const CALLER: &str = "splat_msgs";
    check_keys(CALLER, &options, &SPLAT_KEYS)?;
    let max_attempts = match options.get("max_attempts") {
        None => 100,
        Some(n) => match n.as_int() {
            Ok(n) if n >= 1 => n as usize,
            _ => return Err(format!("{}: 'max_attempts' should be a whole number of at least 1, got {}", CALLER, n).into()),
        },
    };
    let opts = SplatOptions {
        symbol_scale: positive(CALLER, &options, "symbol_scale")?,
        width_range: range(CALLER, &options, "width_range")?,
        height_range: range(CALLER, &options, "height_range")?,
        jagged_edge: number(CALLER, &options, "jagged_edge")?.unwrap_or(0.1),
        max_attempts,
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
    let journal_style: JournalStrokeStyle = style.clone().into();
    let placed = splat_msgs(&journal_canvas, &symbol_msgs, &opts, &journal_style, &mut *rng)
        .map_err(|err| format!("{}: {}", CALLER, err))?;

    let mut splatted = Canvas::default();
    for symbol in placed.into_iter().flatten() {
        add_glyph(&mut splatted, &Glyph(symbol), &style);
    }
    Ok(splatted)
}

pub fn register(engine: &mut Engine, symdict: Rc<SymbolPathDict>, rng: SharedRng) {
    engine.register_type_with_name::<TextBox>("TextBox");
    engine.register_fn("bounds", bounds);
    engine.register_fn("new_textbox", TextBox::new);

    let r = rng.clone();
    engine.register_fn("fill", move |tb: &mut TextBox, glyphs_in: Array| -> RhaiResult<Map> {
        let (placed, unplaced) = tb.core().fill_bounds_with(symbols("fill", glyphs_in)?, &mut *r.borrow_mut());
        let mut result = Map::new();
        result.insert("placed".into(), Dynamic::from_array(glyphs(placed)));
        result.insert("unplaced".into(), Dynamic::from_array(glyphs(unplaced)));
        Ok(result)
    });

    let r = rng.clone();
    engine.register_fn("place_all", move |tb: &mut TextBox, glyphs_in: Array| -> RhaiResult<Array> {
        let mut to_place = symbols("place_all", glyphs_in)?;
        let core = tb.core();
        let mut blocks = Array::new();
        let mut rng = r.borrow_mut();
        // Like TextBox::place_all_symbols, but a glyph that doesn't fit
        // even in an empty box is an error rather than silently dropped.
        while !to_place.is_empty() {
            let (placed, rest) = core.fill_bounds_with(to_place, &mut *rng);
            if placed.is_empty() {
                return Err(format!("place_all: {} glyph(s) don't fit in the text box even when it's empty", rest.len()).into());
            }
            blocks.push(Dynamic::from_array(glyphs(placed)));
            to_place = rest;
        }
        Ok(blocks)
    });

    let r = rng.clone();
    engine.register_fn("splat_msgs", move |canvas: &mut Canvas, msgs: Array, font: Font, style: StrokeStyle, options: Map| -> RhaiResult<Canvas> {
        splat(canvas, msgs, &font, style, options, &symdict, &r)
    });

    engine.register_fn("hatch", move |path: &mut StrokePath, spacing: f64, angle: f64, roughness: f64| -> RhaiResult<StrokePath> {
        hatch(path, spacing, angle, roughness, &rng)
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
            let avoiding = new_textbox(opts).fill(font.glyphs(text)).placed;
            for g in avoiding {
                if g.position.x.abs() < 30.0 && g.position.y.abs() < 30.0 { throw "placed on the blot: " + g.position; }
            }
            [free.len(), avoiding.len()]
        "#);
        let counts: Vec<i64> = counts.into_iter().map(|c| c.as_int().unwrap()).collect();
        assert!(counts[1] < counts[0], "{:?}", counts);
    }

    #[test]
    fn hatch_fills_inside_the_path_and_follows_the_seed() {
        let hatching = |seed: i64| -> String {
            let path: crate::journal::paths::StrokePath = eval(&format!(
                "seed({}); new_circle(new_vec(0.0, 0.0), 20.0).hatch(3.0, 0.7, 1.0)", seed));
            for p in path.points_within() {
                assert!(p.length() < 22.0, "hatch point {:?} outside the circle", p);
            }
            path.to_string()
        };
        assert!(!hatching(1).is_empty());
        assert_eq!(hatching(1), hatching(1));
        assert_ne!(hatching(1), hatching(2));
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
    fn splat_fails_when_a_message_cannot_fit() {
        let err = fails(&format!(r#"{}
            opts.symbol_scale = 400.0;
            opts.max_attempts = 5;
            base.splat_msgs(msgs, lowercase_font("loche_upper"), style, opts);
        "#, SPLAT_SETUP));
        assert!(err.contains("splat_msgs: message 0 doesn't fit in 5 tries"), "{}", err);
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
            (format!("#{{ center: {}, width: 10, height: 10, symbol_scale: 2, avoid_radius: 3 }}", c), "'avoid_radius' needs 'avoid'"),
        ] {
            let err = fails(&format!("let ok = 1;\nnew_textbox({});", opts));
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
        let err = fails("let ok = 1;\nnew_circle(new_vec(0.0, 0.0), 5.0).hatch(0.0, 0.0, 1.0);");
        assert!(err.contains("spacing must be positive") && err.contains("line 2"), "{}", err);
    }
}
