//! The API's shape (see CLAUDE.md): constructors take their values'
//! properties as options, text calls take a style, a type and a paragraph,
//! and the helpers journals share.

use super::JournalEngine;
use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};
use std::rc::Rc;

fn engine() -> JournalEngine {
    JournalEngine::new(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).expect("fonts.json")))
}

fn eval_bool(script: &str) -> bool {
    engine().engine.eval::<bool>(script).unwrap_or_else(|e| panic!("{}\n{}", e, script))
}

fn fails(script: &str) -> String {
    engine().engine.run(script).expect_err("script should fail").to_string()
}

const SETUP: &str = r#"
    let ink = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
    let plain = glyphset("plain");
"#;

#[test]
fn constructors_take_every_settable_property() {
    // Each constructor, a value for each property, and the same value set
    // after: they read back the same.
    for (made, set, props) in [
        ("new_page(#{OPTS})", "new_page()", r#"name: "p" | size: vec2(10, 20) | margin: 3.0 | svg_style: ()"#),
        ("new_canvas(#{OPTS})", "new_canvas()", r#"name: "c""#),
        ("new_brush_style(\"red\", \"Ink Pen\", 2.0, #{OPTS})", "new_brush_style(\"red\", \"Ink Pen\", 2.0)",
            r#"color: "blue" | brush: "Calligraphy" | brush_size: 3.0 | min_pressure: 0.1 | max_pressure: 0.9 | pressure_profile: "taper" | pressure_wander: 0.2 | pressure_wander_length: 9.0 | pool: 0.4 | pool_depth: 0.5"#),
        ("new_handwriting_style(#{OPTS})", "new_handwriting_style()",
            "passes: 3 | pass_end_jitter: 2.0 | stroke_point_jitter: 1.0 | glyph_position_jitter: [1.0, 2.0] | glyph_rotation_jitter: 0.1 | glyph_scale_jitter: 0.2 | baseline_wander: 4.0"),
        ("new_type_style(plain, 20.0, #{OPTS})", "new_type_style(plain, 20.0)",
            "glyph_scale: 30.0 | glyph_scale_factor: 1.5 | glyph_outline_gap: 0.2 | glyph_tracking: 0.1 | space_width: 0.8 | skew: 0.1 | joined: false | underline: 2 | strike: true"),
        ("new_paragraph_style(#{OPTS})", "new_paragraph_style()",
            r#"line_height: 1.4 | paragraph_gap: 1.0 | indent: 2.0 | align: "justify" | valign: "center" | fit: true | layout: "words" | angle: 0.2 | split_words: 3 | split_padding: 0.5 | clearance: 6.0 | line_gap_jitter: 2.0 | line_start_jitter: 4.0"#),
        ("new_flex_style(#{OPTS})", "new_flex_style()",
            r#"width: 100 | height: "50%" | grow: 1 | margin: [1, 2] | padding: 3 | position: "absolute" | top: 4 | gap: 5 | justify: "center" | wrap: true | text_wrap: "hull" | border: "frame""#),
        ("glyphset(\"plain\", #{OPTS})", "glyphset(\"plain\")", "glyph_outline_gap: 0.2 | space_width: 0.8 | skew: 0.1 | joined: false"),
        ("glyphset(\"ayan\", #{OPTS})", "glyphset(\"ayan\")", "joined: false"),
        ("ternary_glyphset(\"balanced_ternary\", #{OPTS})", "ternary_glyphset(\"balanced_ternary\")", "glyph_outline_gap: 0.2 | space_width: 0.8 | skew: 0.1"),
        ("plain.glyph(\"a\", #{OPTS})", "plain.glyph(\"a\")", "position: vec2(3, 4) | scale: 20.0 | rotation: 0.5 | skew: 0.2"),
        ("new_book(#{OPTS})", "new_book()", "first: 3"),
    ] {
        let mut sets = String::new();
        let mut checks = Vec::new();
        for prop in props.split(" | ") {
            let (key, value) = prop.split_once(": ").unwrap();
            sets.push_str(&format!("b.{} = {};\n", key, value));
            checks.push(format!("`${{a.{k}}}` == `${{b.{k}}}`", k = key));
        }
        let script = format!("{}\nlet a = {};\nlet b = {};\n{}\n{}", SETUP, made.replace("OPTS", &props.replace(" | ", ", ")), set, sets, checks.join(" && "));
        assert!(eval_bool(&script), "{}", script);
    }
}

#[test]
fn constructor_options_check_as_properties_do() {
    for (script, want) in [
        ("new_page(#{ nme: \"p\" })", "new_page: unknown option 'nme'; options: name, size, margin, svg_style"),
        ("new_page(#{ name: \"a b\" })", "new_page: page.name: invalid name 'a b'"),
        ("new_brush_style(\"red\", \"Ink Pen\", 1.0, #{ pool: -1 })", "new_brush_style: 'pool' can't be negative"),
        ("new_handwriting_style(#{ passes: 0 })", "new_handwriting_style: 'passes' should be from 1 to 64"),
        ("new_type_style(glyphset(\"plain\"), 10, #{ glyph_tracking: -1 })", "new_type_style: 'glyph_tracking' must be above -0.5"),
        ("new_paragraph_style(#{ align: \"middle\" })", "new_paragraph_style: unknown align 'middle'"),
        ("new_flex_style(#{ grow: -1 })", "new_flex_style: 'grow' can't be negative"),
        ("glyphset(\"plain\").glyph(\"a\", #{ scale: \"big\" })", "glyph: glyph.scale"),
    ] {
        let err = fails(&format!("let ok = 1;\n{};", script));
        assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
    }
    // Properties check alike, against the style's name.
    let err = fails("let s = default_style();\ns.pool = -1;");
    assert!(err.contains("style: 'pool' can't be negative") && err.contains("line 2"), "{}", err);
}

#[test]
fn text_calls_take_a_style_a_type_and_a_paragraph() {
    assert!(eval_bool(&format!(r#"{}
        // A role's settings go straight in.
        let role = #{{ style: ink, type: new_type_style(plain, 20.0), paragraph: #{{ line_height: 1.4, align: "justify" }} }};
        let line = new_line(vec2(0, 0), vec2(2000, 0));
        let a = line.fill_with_text("lorem ipsum", role)[0];
        let b = write_line("lorem ipsum", vec2(0, 0), role);
        let block = write_block("lorem ipsum dolor sit amet consectetur adipiscing elit sed do", vec2(0, 0), 200, role);
        `${{a.bounds()}}` == `${{b.bounds()}}` && block.bottom > 3.0 * 1.4 * 20.0 - 0.01
    "#, SETUP)));
    for (script, want) in [
        ("write_line(\"a\", vec2(0, 0), #{ type: new_type_style(plain, 10), colour: 1 })", "write_line: unknown option 'colour'"),
        ("write_line(\"a\", vec2(0, 0), #{ style: ink, type: #{ glyphset: plain } })", "write_line: the type has no 'glyph_scale'"),
        ("new_line(vec2(0, 0), vec2(99, 0)).fill_with_text(\"a\", #{ style: ink, type: #{ glyph_scale: 10 } })", "fill_with_text: the type has no 'glyphset'"),
        ("write_line(\"a\", vec2(0, 0), #{ type: new_type_style(plain, 10), style: #{ glyph_scale_jitter: 1.5 } })", "'glyph_scale_jitter' is a fraction either way, below 1"),
        ("write_line(\"a\", vec2(0, 0), #{ type: new_type_style(plain, 10), paragraph: 3 })", "should be a paragraph_style or a map of its settings"),
    ] {
        let err = fails(&format!("{}\n{};", SETUP, script));
        assert!(err.contains(want), "{}: {}", want, err);
    }
    // A map of style settings is over the default style.
    assert!(eval_bool(&format!(r#"{}
        let a = write_line("lorem", vec2(0, 0), #{{ type: new_type_style(plain, 20.0) }});
        let b = write_line("lorem", vec2(0, 0), #{{ type: new_type_style(plain, 20.0), style: #{{ color: "black" }} }});
        `${{a.bounds()}}` == `${{b.bounds()}}`
    "#, SETUP)));
}

#[test]
fn color_jitter_draws_from_the_generator_only_when_on() {
    let next = |jitter: f64| -> f64 {
        engine().engine.eval::<f64>(&format!(r#"
            seed(5);
            let style = new_style(new_brush_style("navy", "Ink Pen", 1.2, #{{ color_jitter: {} }}), new_handwriting_style(#{{ passes: 3 }}));
            new_canvas().add(new_line(vec2(0, 0), vec2(50, 0)), style);
            random_range(0.0, 1.0)
        "#, jitter)).unwrap()
    };
    let plain = engine().engine.eval::<f64>("seed(5); random_range(0.0, 1.0)").unwrap();
    assert_eq!(next(0.0), next(0.0));
    assert_ne!(next(0.0), next(0.1), "a jittered colour draws its seed");
    assert_ne!(next(0.0), plain, "passes draw from the generator as before");
    assert!(engine().engine.eval::<f64>(r#"new_brush_style("navy", "Ink Pen", 1.2, #{ color_jitter: 0.05 }).color_jitter"#).unwrap() == 0.05);
    assert!(engine().engine.eval::<f64>(r#"let s = default_style(); s.color_jitter = 0.2; s.color_jitter"#).unwrap() == 0.2);
    assert!(engine().engine.eval::<rhai::Dynamic>(r#"new_brush_style("navy", "Ink Pen", 1.2, #{ color_jitter: 1.5 })"#).is_err());
}

#[test]
fn jitter_moves_glyphs_from_the_seeded_generator() {
    let xs = |jitter: &str, seed: i64| -> String {
        engine().engine.eval::<String>(&format!(r#"{}
            seed({});
            let c = write_line("lorem ipsum", vec2(0, 0), #{{ style: #{{ {} }}, type: new_type_style(plain, 20.0) }});
            `${{c.bounds()}}`
        "#, SETUP, seed, jitter)).unwrap()
    };
    let jittered = "glyph_position_jitter: [2.0, 4.0], glyph_rotation_jitter: 0.1, glyph_scale_jitter: 0.1";
    assert_eq!(xs("", 1), xs("", 2), "no jitter draws nothing from the generator");
    assert_ne!(xs("", 1), xs(jittered, 1));
    assert_eq!(xs(jittered, 1), xs(jittered, 1));
    assert_ne!(xs(jittered, 1), xs(jittered, 2));
}

#[test]
fn shapes_and_points_along() {
    assert!(eval_bool(r#"
        let ring = new_circle(vec2(0, 0), 10).points_along(4, #{ start: 0.75 });
        let near = |a, b| (a - b).length() < 0.01;
        let open = new_line(vec2(0, 0), vec2(10, 0)).points_along(3);
        near.call(ring[0].position, vec2(0, -10)) && near.call(ring[1].position, vec2(10, 0))
            && (ring[0].angle).abs() < 0.01 && (ring[1].angle - PI() / 2.0).abs() < 0.01
            && near.call(open[2].position, vec2(10, 0)) && near.call(open[1].position, vec2(5, 0))
            && new_polygon(vec2(0, 0), 10, 6).bounds().height > 19.9
            && new_polygon(vec2(0, 0), 10, 6, #{ step: 2 }).bounds().width > 0
            && new_star(vec2(0, 0), 10, 4, 5).bounds().height > 18
            && (new_ellipse(vec2(0, 0), 20, 5).bounds().width - 40.0).abs() < 0.1
            && new_wave(vec2(0, 0), vec2(90, 0), #{ amplitude: 5 }).bounds().height > 5
            && new_polyline([vec2(0, 0), vec2(5, 5)]).bounds().width == 5.0
            && new_path().is_empty && !new_line(vec2(0, 0), vec2(1, 0)).is_empty
    "#));
    for (script, want) in [
        ("new_polygon(vec2(0, 0), 10, 2)", "sides should be from 3"),
        ("new_polygon(vec2(0, 0), 10, 5, #{ step: 5 })", "'step' should be a whole number from 1 to 4"),
        ("new_circle(vec2(0, 0), 10).append(new_circle(vec2(5, 5), 2)).points_along(3)", "one line (one subpath)"),
        ("new_polyline([vec2(0, 0), 3])", "point 1 is a i64"),
    ] {
        let err = fails(&format!("let ok = 1;\n{};", script));
        assert!(err.contains(want), "{}: {}", want, err);
    }
}

#[test]
fn books_number_name_and_list_pages() {
    assert!(eval_bool(r#"
        let book = new_book(#{ first: 3 });
        let p = new_page().add(new_canvas().add(new_line(vec2(0, 0), vec2(1, 0)), new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style())));
        let first = book.next;
        book.add("pens", p);
        book.add("pens", p);
        book.add("at a glance", [p, p]);
        let names = book.pages.map(|p| p.name);
        first == 3 && book.next == 7 && names == ["03_pens", "04_pens", "05_at_a_glance", "06_at_a_glance"]
            && book.contents == [["pens", 3], ["at a glance", 5]]
            && roman(14) == "xiv" && roman(1994) == "mcmxciv"
            && escape_markdown("*a* [b]{c}") == "\\*a\\* \\[b\\]\\{c\\}"
    "#));
    let err = fails("let b = new_book();\nb.add(\"x\", new_page());\nb.first = 2;");
    assert!(err.contains("set it before adding pages"), "{}", err);
    let err = fails("let ok = 1;\nassert(1 > 2, \"one is not more\");");
    assert!(err.contains("assertion failed: one is not more") && err.contains("line 2"), "{}", err);
}

#[test]
fn canvas_add_takes_parts_and_canvases() {
    assert!(eval_bool(r#"
        let ink = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
        let moon = art::moon(vec2(0, 0), 10, 0.3);
        let all = new_canvas().add(moon, #{ outline: ink, terminator: ink });
        let one = new_canvas().add(moon.outline, ink).add(moon.terminator, ink);
        let parts = new_canvas().add([one, one.copy()]);
        `${all.bounds()}` == `${one.bounds()}` && `${parts.bounds()}` == `${one.bounds()}`
    "#));
    let err = fails("let ink = new_style(new_brush_style(\"black\", \"Ink Pen\", 1.0), new_handwriting_style());\nnew_canvas().add(art::moon(vec2(0, 0), 10, 0.3), #{ rim: ink });");
    assert!(err.contains("there's no part 'rim' to style"), "{}", err);
}

#[test]
fn lenient_symbols_keep_what_the_base_writes() {
    assert!(eval_bool(r#"
        symbols(27, "  It's a — test, 2 go!", #{ lenient: true }).text() == "ITS A TEST GO"
            && symbols(10, "a1-2 3", #{ lenient: true }).text() == "123"
    "#));
}
