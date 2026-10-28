//! What illustrations are given arrives in what they draw: a wired text
//! style drawn whole (its pen, its glyphs, its size), its gaps filled from
//! the illustration's fallback, and the options their fallbacks are made
//! from. Each role is wired in a colour of its own, or told apart by
//! where it's drawn, and its ink measured: a glyph's strokes are as tall
//! as its glyph scale, give or take the pen.

use parry2d::bounding_volume::Aabb;
use rhai::{Dynamic, Map};
use std::rc::Rc;

use super::canvas::Stroke;
use super::options::Settings;
use super::text_settings::{ParagraphStyle, TypeStyle};
use super::{Entry, JournalEngine, Resources, Style};
use crate::journal::canvas::{parse_color, StrokeColor};
use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};

const GREEN: &str = "#1a9e3a";
const BLUE: &str = "#2040c0";
/// The fallbacks' inks.
const RED: &str = "#9b2318";
const INK: &str = "#2b1d12";

fn engine(styles: &[(&str, &str)], illustrations: &[(&str, &str)]) -> JournalEngine {
    engine_with(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).expect("glyphsets load")), styles, illustrations)
}

fn engine_with(book: Rc<FontBook>, styles: &[(&str, &str)], illustrations: &[(&str, &str)]) -> JournalEngine {
    let overlay = |files: &[(&str, &str)]| files.iter().map(|(n, s)| (n.to_string(), s.to_string())).collect::<Vec<_>>();
    let mut resources = Resources::library();
    resources.styles = resources.styles.with_overlay(overlay(styles));
    resources.illustrations = resources.illustrations.with_overlay(overlay(illustrations));
    JournalEngine::with_resources(book, resources)
}

/// A style file of just a pen: no type, no paragraph.
const PEN_ONLY: &str = r#"fn options() { #{ color: "black" } } fn style(o) { #{ style: #{ color: o.color, brush_size: 2.0 } } }"#;
/// A style file of the cursive glyphset, ayan.
const CURSIVE: &str = r#"fn options() { #{ size: 40.0, color: "black" } } fn style(o) { #{ style: #{ color: o.color }, type: new_type_style(glyphset("ayan"), o.size) } }"#;

/// What illustration `name` draws, attached with `options` to `chain` (a
/// lock, its values set), `wires` (input, port) connected: a page a canvas.
/// Every character it writes has a glyph.
fn drawn(chain: &str, name: &str, options: &str, wires: &[(&str, &str)]) -> Entry {
    let connects: String = wires.iter().map(|(input, port)| format!(".connect(\"{}\", \"d.{}\")", port, input)).collect();
    let script = format!(r#"
        let lock = {chain}.attach("d", "{name}", {options}){connects};
        for c in lock.run().canvases("d") {{ journal_entry.add(new_page().add(c)); }}
    "#);
    let book = Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).expect("glyphsets load"));
    let entry = engine_with(book.clone(), &[("pen_only", PEN_ONLY), ("cursive", CURSIVE)], &[]).build_entry_str(&script).unwrap_or_else(|e| panic!("{}\n{}", script, e));
    assert_eq!(book.missing_glyph_report(), None, "{}", script);
    entry
}

/// A style source `id` of `style` (a library style's name) with `options`.
fn source(id: &str, style: &str, options: &str) -> String {
    format!(r#".source("{}", "style", "{}", #{{ options: {} }})"#, id, style, options)
}

fn strokes(entry: &Entry) -> impl Iterator<Item = &Stroke> {
    entry.pages.iter().flat_map(|page| page.canvases.iter()).flat_map(|canvas| canvas.strokes.iter())
}

fn inked(stroke: &Stroke, color: &str) -> bool {
    matches!(stroke.style.color(), StrokeColor::Solid(c) if parse_color(c).unwrap() == parse_color(color).unwrap())
}

fn bounds(stroke: &Stroke) -> Aabb {
    Aabb::from_points(stroke.path.points_within())
}

/// The strokes in `color`, `within` (a box's min and max) if given: at
/// least one.
fn in_color<'a>(entry: &'a Entry, color: &str, within: Option<([f32; 2], [f32; 2])>) -> Vec<&'a Stroke> {
    let inside = |s: &&Stroke| within.is_none_or(|(lo, hi)| {
        let b = bounds(s);
        b.mins.x >= lo[0] && b.mins.y >= lo[1] && b.maxs.x <= hi[0] && b.maxs.y <= hi[1]
    });
    let found: Vec<&Stroke> = strokes(entry).filter(|s| inked(s, color)).filter(inside).collect();
    assert!(!found.is_empty(), "nothing drawn in {} within {:?}", color, within);
    found
}

/// How tall the tallest stroke in `color` is: a glyph's, its glyph scale
/// give or take the pen.
fn tallest(entry: &Entry, color: &str, within: Option<([f32; 2], [f32; 2])>) -> f32 {
    in_color(entry, color, within).iter().map(|s| bounds(s).extents().y).fold(0.0, f32::max)
}

/// How wide the widest stroke in `color` is, of those that aren't flat
/// (a rule's).
fn widest(entry: &Entry, color: &str) -> f32 {
    in_color(entry, color, None).iter().map(|s| bounds(s).extents()).filter(|e| e.y > 2.0).map(|e| e.x).fold(0.0, f32::max)
}

/// `big` is `times` `small`, give or take a pen's width.
fn scaled(what: &str, small: f32, big: f32, times: f32) {
    let ratio = big / small;
    assert!((ratio - times).abs() < 0.12 * times, "{}: {} then {}, {}x rather than {}x", what, small, big, ratio, times);
}

/// The tallest stroke in `color` drawn with a pen `size` wide.
fn tallest_in_pen(entry: &Entry, color: &str, size: f64, within: Option<([f32; 2], [f32; 2])>) -> f32 {
    in_color(entry, color, within).iter().filter(|s| s.style.get("brush_size").as_float().ok() == Some(size)).map(|s| bounds(s).extents().y).fold(0.0, f32::max)
}

/// How wide all the strokes in `color` that aren't flat reach, together.
fn spread(entry: &Entry, color: &str) -> f32 {
    let xs: Vec<(f32, f32)> = in_color(entry, color, None).iter().map(|s| bounds(s)).filter(|b| b.extents().y > 2.0).map(|b| (b.mins.x, b.maxs.x)).collect();
    xs.iter().map(|x| x.1).fold(f32::MIN, f32::max) - xs.iter().map(|x| x.0).fold(f32::MAX, f32::min)
}

/// The brush strokes in `color` are drawn with.
fn brush(entry: &Entry, color: &str) -> (String, f64) {
    let style = &in_color(entry, color, None)[0].style;
    (style.get("brush").into_string().unwrap(), style.get("brush_size").as_float().unwrap())
}

// ---- the plumbing ----

const PROBE: &str = r#"
    // probe: three text styles and a value, with fallbacks.
    fn options() { #{ size: 30.0 } }
    fn inputs(o) { #{ text: "TextStyle", pen: "TextStyle", bare: "TextStyle", plain: "TextStyle", other: "Any" } }
    fn fallbacks(o) {
        #{
            text: #{ style: #{ color: "red" }, type: new_type_style(glyphset("loche_upper"), o.size), paragraph: #{ align: "center" } },
            pen: #{ style: #{ color: "red" }, type: new_type_style(glyphset("plain"), 12, #{ glyph_outline_gap: 0.3, skew: 0.1 }), paragraph: #{ line_height: 2.0 } },
            bare: #{ style: #{ color: "red" }, type: #{ glyph_scale: o.size } },
            other: #{ type: 3 },
        }
    }
    fn render(o, i) { new_canvas() }
"#;

fn type_of(bundle: &Map) -> TypeStyle {
    bundle["type"].clone().try_cast::<TypeStyle>().expect("a whole type")
}

#[test]
fn a_wired_text_style_arrives_whole_its_gaps_its_fallbacks() {
    let engine = engine(&[("pen_only", PEN_ONLY)], &[("probe", PROBE)]);
    let probe = engine.illustrations.load(&engine.engine, "probe").unwrap();
    let o = probe.options(&[("size".into(), Dynamic::from(40_i64))].into_iter().collect()).unwrap();
    let given: Map = engine.engine.eval(r#"#{
        text: text_style("plain_hand", #{ size: 18, color: "green" }),
        pen: text_style("pen_only", #{ color: "green" }),
        plain: #{ type: #{ glyph_scale: 9 } },
        other: #{ type: 4 },
    }"#).unwrap();
    let inputs = probe.with_fallbacks(&engine.engine, &o, given).unwrap();
    let bundle = |name: &str| inputs[name].clone().try_cast::<Map>().unwrap();

    // Given whole, it's drawn as given: plain_hand's pen, glyphs, size and
    // paragraph, not the fallback's.
    let text = bundle("text");
    let t = type_of(&text);
    assert_eq!(t.glyphset("t").unwrap().name(), "plain");
    assert_eq!(t.size("t").unwrap(), 18.0);
    let paragraph = text["paragraph"].clone().try_cast::<ParagraphStyle>().unwrap();
    assert_eq!((paragraph.line_height(), paragraph.get("align").into_string().unwrap()), (1.4, "left".to_string()));
    assert_eq!(text["style"].clone().try_cast::<Style>().unwrap().get("color").into_string().unwrap(), "green");

    // A pen alone takes the fallback's glyphs, at its size, shaped as it
    // shapes them; its pen and paragraph are its own.
    let pen = bundle("pen");
    let t = type_of(&pen);
    assert_eq!((t.glyphset("t").unwrap().name(), t.size("t").unwrap()), ("plain", 12.0));
    assert!(t.outline_gap() == Some(0.3) && (t.get("skew").as_float().unwrap() - 0.1).abs() < 1e-6, "the fallback's shaping");
    assert_eq!(pen["style"].clone().try_cast::<Style>().unwrap().get("color").into_string().unwrap(), "green");
    assert_eq!(pen["paragraph"].clone().try_cast::<ParagraphStyle>().unwrap().line_height(), 1.25, "a style file's paragraph is whole");

    // Unwired, the fallback, made from the options and made whole.
    let bare = bundle("bare");
    assert_eq!(type_of(&bare).size("t").unwrap(), 40.0);
    assert!(bare["paragraph"].is::<ParagraphStyle>() && bare["style"].is::<Style>());

    // A map of some of a bundle, with no fallback: whole, nothing filled.
    let plain = bundle("plain");
    assert!(type_of(&plain).get("glyphset").is_unit() && type_of(&plain).size("t").unwrap() == 9.0);
    assert!(plain["style"].is::<Style>());

    // Anything else is as given.
    assert_eq!(bundle("other")["type"].as_int().unwrap(), 4);
}

#[test]
fn a_text_style_input_given_a_wrong_bundle_is_named() {
    let engine = engine(&[], &[("probe", PROBE)]);
    let probe = engine.illustrations.load(&engine.engine, "probe").unwrap();
    let o = probe.options(&Map::new()).unwrap();
    for (given, want) in [
        (r#"#{ text: #{ font: 3 } }"#, "illustration 'probe' input 'text': unknown option 'font'; options: style, type, paragraph, spans"),
        (r#"#{ pen: #{ type: #{ glyph_scale: -2 } } }"#, "illustration 'probe' input 'pen' 'type': 'glyph_scale' must be positive, got -2"),
    ] {
        let given: Map = engine.engine.eval(given).unwrap();
        let err = probe.with_fallbacks(&engine.engine, &o, given).unwrap_err();
        assert!(err.contains(want), "wanted: {}\n  got: {}", want, err);
    }
}

// ---- the library's illustrations ----

const TRITS: &str = r#"new_lock("t").add("shift", "shift_cipher", #{ base: 3, shift: 1 }).set_input("shift.plain_text", symbols(3, "ABCABCCBA"))"#;

fn trits(styles: &str, options: &str, wires: &[(&str, &str)]) -> Entry {
    let mut wires = wires.to_vec();
    wires.push(("message", "shift.cipher_text"));
    drawn(&format!("{}{}", TRITS, styles), "trit_message", options, &wires)
}

#[test]
fn trit_message_writes_in_the_styles_and_sizes_it_is_given() {
    let styled = |id: &str, style: &str, size: f32, color: &str| source(id, style, &format!("#{{ size: {}, color: \"{}\" }}", size, color));
    // The heading in a wired style: its pen, at its size, in its glyphs
    // (ayan joins a word into one stroke; plain sets glyph by glyph).
    let heading = |style: &str, size: f32, title: &str| trits(&styled("h", style, size, GREEN), &format!("#{{ title: \"{}\" }}", title), &[("heading_style", "h.value")]);
    scaled("the wired heading", tallest(&heading("plain_hand", 30.0, "MESSAGE"), GREEN, None), tallest(&heading("plain_hand", 60.0, "MESSAGE"), GREEN, None), 2.0);
    assert_eq!(brush(&heading("plain_hand", 30.0, "MESSAGE"), GREEN), ("Ink Pen".to_string(), 1.4));
    let plain = heading("plain_hand", 40.0, "message");
    assert!(widest(&plain, GREEN) < 0.3 * spread(&plain, GREEN), "plain sets glyph by glyph");
    let cursive = heading("cursive", 40.0, "message");
    assert!(widest(&cursive, GREEN) > 0.9 * spread(&cursive, GREEN), "ayan joins the word");

    // Unwired, at heading_size, in the fallback's red (the rule's too,
    // but it's flat).
    let option = |options: &str| trits("", options, &[]);
    scaled("heading_size", tallest(&option("#{ heading_size: 30 }"), RED, None), tallest(&option("#{ heading_size: 60 }"), RED, None), 2.0);

    // The marks reach mark_style's size from their lines.
    let marks = |size: f32| trits(&styled("m", "plain_hand", size, GREEN), "#{}", &[("mark_style", "m.value")]);
    scaled("the wired marks", tallest(&marks(15.0), GREEN, None), tallest(&marks(30.0), GREEN, None), 2.0);
    scaled("mark_size", tallest(&option("#{ mark_size: 15 }"), INK, None), tallest(&option("#{ mark_size: 30 }"), INK, None), 2.0);

    // A pen alone draws the lines; for the heading, it writes the
    // fallback's glyphs at heading_size.
    let pens = trits(&format!("{}{}", source("l", "pen_only", &format!("#{{ color: \"{}\" }}", BLUE)), source("p", "pen_only", &format!("#{{ color: \"{}\" }}", GREEN))),
        "#{ heading_size: 50 }", &[("line_style", "l.value"), ("heading_style", "p.value")]);
    assert_eq!(brush(&pens, BLUE), ("Ink Pen".to_string(), 2.0));
    scaled("a pen-only heading, at heading_size", tallest(&option("#{ heading_size: 50 }"), RED, None), tallest(&pens, GREEN, None), 1.0);
}

const SKY: &str = r#"new_lock("t").add("sky", "star_chart", #{ input_base: 27, output_base: 3, most_stars: 9 }).set_input("sky.code", huffman::code(symbols(27, "THE QUICK BROWN FOX"), 3))"#;

fn sky(styles: &str, options: &str, wires: &[(&str, &str)]) -> Entry {
    let mut wires = wires.to_vec();
    wires.push(("chart", "sky.chart"));
    drawn(&format!("{}{}", SKY, styles), "star_pages", options, &wires)
}

#[test]
fn star_pages_writes_each_kind_of_text_in_its_own_style() {
    let styled = |id: &str, style: &str, size: f32, color: &str| source(id, style, &format!("#{{ size: {}, color: \"{}\" }}", size, color));
    for role in ["heading_style", "name_style", "leaf_name_style", "digit_style"] {
        let at = |size: f32| sky(&styled("s", "plain_hand", size, GREEN), "#{}", &[(role, "s.value")]);
        let (small, big) = (at(16.0), at(32.0));
        scaled(role, tallest(&small, GREEN, None), tallest(&big, GREEN, None), 2.0);
        assert_eq!(brush(&small, GREEN), ("Ink Pen".to_string(), 1.4), "{}'s pen", role);
    }
    // In balanced-ternary digits, the numbers are digit glyphs; in plain,
    // decimal, as tall as plain's digits.
    let digits = |options: &str| sky("", options, &[]);
    let index = Some(([600.0, 1110.0], [1200.0, 1600.0]));
    scaled("digit_size", tallest(&digits("#{ digit_size: 20 }"), INK, index), tallest(&digits("#{ digit_size: 40 }"), INK, index), 2.0);
    scaled("name_size", tallest(&digits("#{ name_size: 20 }"), RED, index), tallest(&digits("#{ name_size: 40 }"), RED, index), 2.0);
    let above = Some(([0.0, 0.0], [1200.0, 160.0]));
    scaled("heading_size", tallest(&digits("#{ heading_size: 30 }"), RED, above), tallest(&digits("#{ heading_size: 60 }"), RED, above), 2.0);

    // The links' marks reach line_style's size; a pen alone, the
    // fallback's.
    let lines = sky(&source("l", "pen_only", &format!("#{{ color: \"{}\" }}", BLUE)), "#{}", &[("line_style", "l.value"), ("leaf_style", "l.value")]);
    assert_eq!(brush(&lines, BLUE), ("Ink Pen".to_string(), 2.0));
}

#[test]
fn the_raven_sky_chain_writes_its_stars_numbers_at_its_digit_size() {
    let digit = |size: f32| {
        let script = format!(r#"
            let lock = chain("raven_sky", #{{ digit_size: {} }});
            let result = lock.run(#{{ "enc.plain_text": symbols(27, "ONCE UPON A MIDNIGHT"), "msg.plain_text": symbols(27, "NEVERMORE") }});
            for c in result.canvases("pages") {{ journal_entry.add(new_page().add(c)); }}
        "#, size);
        engine(&[], &[]).build_entry_str(&script).unwrap()
    };
    // The numbers at the stars (in the digits' thinner pen), not the index's.
    let sky = Some(([0.0, 160.0], [1200.0, 1100.0]));
    scaled("raven_sky's digit_size", tallest_in_pen(&digit(18.0), INK, 1.2, sky), tallest_in_pen(&digit(36.0), INK, 1.2, sky), 2.0);
}

const EYE: &str = r#"new_lock("t").add("sub", "substitution_cipher", #{ base_n: 27 }).set_input("sub.ordering", symbols(27, "MQ ZEHWBRKYDTXOAJGNLUFCVISP")).set_input("sub.plain_text", symbols(27, "EVERY LOCK"))"#;

fn eye(styles: &str, options: &str, wires: &[(&str, &str)]) -> Entry {
    let mut wires = wires.to_vec();
    wires.extend([("outer", "sub.ordering"), ("inner", "sub.cipher_text")]);
    drawn(&format!("{}{}", EYE, styles), "eye", options, &wires)
}

#[test]
fn the_eye_writes_its_iris_and_pupil_as_large_as_its_styles_say_and_room_allows() {
    let styled = |id: &str, size: f32, color: &str| source(id, "plain_hand", &format!("#{{ size: {}, color: \"{}\" }}", size, color));
    // The iris's glyphs at the style's size, up to the ring's room.
    let iris = |size: f32| eye(&styled("s", size, GREEN), "#{}", &[("iris_style", "s.value")]);
    scaled("the iris", tallest(&iris(30.0), GREEN, None), tallest(&iris(60.0), GREEN, None), 2.0);
    let ring = (717.2 / 2.0 - 6.0) * 0.44 * 0.8;
    scaled("the iris, held to the ring", tallest(&iris(ring), GREEN, None), tallest(&iris(ring * 3.0), GREEN, None), 1.0);
    // The pupil's text at the style's size, all of it fitting.
    for pupil in ["lines", "spiral"] {
        let at = |size: f32| eye(&styled("s", size, GREEN), &format!("#{{ pupil: \"{}\" }}", pupil), &[("pupil_style", "s.value")]);
        scaled(pupil, tallest(&at(20.0), GREEN, None), tallest(&at(40.0), GREEN, None), 2.0);
        // Unwired, at pupil_size, in inner_ink.
        let option = |size: f32| eye("", &format!("#{{ pupil: \"{}\", pupil_size: {}, inner_ink: \"{}\" }}", pupil, size, BLUE), &[]);
        scaled(&format!("{}, pupil_size", pupil), tallest(&option(20.0), BLUE, None), tallest(&option(40.0), BLUE, None), 2.0);
    }
    // The lids and rings in the styles' pens.
    let pens = eye(&format!("{}{}", source("l", "pen_only", &format!("#{{ color: \"{}\" }}", GREEN)), source("r", "pen_only", &format!("#{{ color: \"{}\" }}", BLUE))), "#{}", &[("lid_style", "l.value"), ("ring_style", "r.value")]);
    assert_eq!((brush(&pens, GREEN).1, brush(&pens, BLUE).1), (2.0, 2.0));
}

const MAZE: &str = r#"new_lock("t").add("maze", "maze_lock", #{ input_base: 27 }).set_input("maze.maze_text", symbols(27, "HELLO WORLD")).set_input("maze.random_seed", symbols(10, "4"))"#;

#[test]
fn maze_glyphs_writes_its_cells_in_any_glyphs_up_to_a_cell() {
    let maze = |styles: &str, options: &str, wires: &[(&str, &str)]| {
        let mut wires = wires.to_vec();
        wires.push(("maze", "maze.generated_maze"));
        drawn(&format!("{}{}", MAZE, styles), "maze_glyphs", options, &wires)
    };
    let glyphs = |style: &str, size: f32| maze(&source("s", style, &format!("#{{ size: {}, color: \"{}\" }}", size, GREEN)), "#{ cell: 80 }", &[("glyph_style", "s.value")]);
    // Letters, in plain or the cursive, at the style's size, up to 0.8 of
    // a cell.
    scaled("plain cells", tallest(&glyphs("plain_hand", 20.0), GREEN, None), tallest(&glyphs("plain_hand", 40.0), GREEN, None), 2.0);
    scaled("plain cells, held to a cell", tallest(&glyphs("plain_hand", 64.0), GREEN, None), tallest(&glyphs("plain_hand", 200.0), GREEN, None), 1.0);
    scaled("cursive cells", tallest(&glyphs("cursive", 20.0), GREEN, None), tallest(&glyphs("cursive", 40.0), GREEN, None), 2.0);
    // Unwired, ternary digits as big as a cell holds.
    let cell = |size: f32| maze("", &format!("#{{ cell: {}, glyphs: \"{}\" }}", size, BLUE), &[]);
    scaled("cell", tallest(&cell(40.0), BLUE, None), tallest(&cell(80.0), BLUE, None), 2.0);
}

#[test]
fn styled_text_writes_in_its_style_whole() {
    let styled = |style: &str, size: f32| drawn(&format!(r#"new_lock("t"){}.source("w", "content", "lorem")"#, source("s", style, &format!("#{{ size: {}, color: \"{}\" }}", size, GREEN))),
        "styled_text", "#{ width: 900 }", &[("text", "w.value"), ("style", "s.value")]);
    scaled("styled_text", tallest(&styled("plain_hand", 20.0), GREEN, None), tallest(&styled("plain_hand", 40.0), GREEN, None), 2.0);
    scaled("styled_text in the cursive", tallest(&styled("cursive", 30.0), GREEN, None), tallest(&styled("cursive", 60.0), GREEN, None), 2.0);
    // Its first line a glyph scale down, its lines line_height apart.
    let lines = styled("plain_hand", 30.0);
    let top = in_color(&lines, GREEN, None).iter().map(|s| bounds(s).mins.y).fold(f32::MAX, f32::min);
    assert!(top > 30.0 * 0.2 && top < 30.0, "the first line a glyph scale down: its ink from {}", top);
}

#[test]
fn every_illustration_writes_in_the_cursive_wherever_it_writes() {
    let maze = |styles: &str, wires: &[(&str, &str)]| {
        let mut wires = wires.to_vec();
        wires.push(("maze", "maze.generated_maze"));
        drawn(&format!("{}{}", MAZE, styles), "maze_glyphs", "#{ cell: 120 }", &wires)
    };
    let styled = |styles: &str, wires: &[(&str, &str)]| {
        let mut wires = wires.to_vec();
        wires.push(("text", "w.value"));
        drawn(&format!(r#"new_lock("t"){}.source("w", "content", "lorem")"#, styles), "styled_text", "#{}", &wires)
    };
    type Draw<'a> = Box<dyn Fn(&str, &[(&str, &str)]) -> Entry + 'a>;
    let illustrations: Vec<(&str, Draw, &[&str])> = vec![
        ("trit_message", Box::new(|s: &str, w: &[(&str, &str)]| trits(s, "#{}", w)), &["heading_style", "mark_style"]),
        ("star_pages", Box::new(|s: &str, w: &[(&str, &str)]| sky(s, "#{}", w)), &["heading_style", "digit_style", "name_style", "leaf_name_style"]),
        ("eye", Box::new(|s: &str, w: &[(&str, &str)]| eye(s, "#{}", w)), &["iris_style", "pupil_style"]),
        ("maze_glyphs", Box::new(maze), &["glyph_style"]),
        ("styled_text", Box::new(styled), &["style"]),
    ];
    for (name, draw, inputs) in illustrations {
        for input in inputs {
            let at = |size: f32| draw(&source("s", "cursive", &format!("#{{ size: {}, color: \"{}\" }}", size, GREEN)), &[(input, "s.value")]);
            scaled(&format!("{}'s {} in the cursive", name, input), tallest(&at(20.0), GREEN, None), tallest(&at(40.0), GREEN, None), 2.0);
        }
    }
}
