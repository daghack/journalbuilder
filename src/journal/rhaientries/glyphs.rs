//! Fonts and glyphs for rhai scripts.
//!
//! Fonts come from the font file (`config/fonts.json`, see fonts.rs), each
//! with its glyphs and the settings text is set with. A font is a value:
//! changing one changes that copy, not the font everywhere.
//!
//! ```rhai
//! let copper = font("copper");                  // as the font file sets it
//! copper.character_padding = 0.08;              // room between glyphs, in font sizes
//! copper.space_width = 0.4;                     // a space's width, in font sizes
//! copper.fallback = font("loche_upper");        // where missing characters are looked for; () for nowhere
//! let tight = font("loche_lower", #{ character_padding: 0.0 });   // settings as options
//! let ternary = ternary_font("balanced_ternary");   // a..m -> 1..13, n..z -> -13..-1
//! font_names();                                 // every font, sorted
//! copper.name;
//! copper.characters();                          // the characters it has glyphs for
//! copper.variants("a");                         // every variant of a glyph
//!
//! // Glyphs sit at the origin with scale 1 until moved. A glyph's path spans
//! // -1..1, drawn at half its scale, so scale is its size in canvas units.
//! for (g, i) in ternary.glyphs("abc") {
//!     g.scale = 30.0;
//!     g.position = new_vec(i * 40.0, 0.0);
//!     g.rotate_by(0.1);
//!     canvas.add_glyph(g, style);
//! }
//! canvas.add_glyph(font("symbols").glyph("basic_eye").scale_by(50.0), style);
//! ternary.glyph(-13);                           // a glyph by its name (or number)
//!
//! // Glyphs, like paths and canvases, are shared: every copy of one (a
//! // variable, an array element, a loop variable) is the same glyph, so
//! // chains and loops change it. `.copy()` makes an independent one.
//! g.translate_by(new_vec(1.0, 0.0)).scale_by(2.0);
//! let word = copper.glyphs("ink");
//! for (g, i) in word { g.position = new_vec(i * 20.0, 0.0); }
//! canvas.add_glyphs(word, style);   // adds copies of the glyphs' strokes
//! ```
//!
//! A character is looked for as it is in the font and its fallbacks, then in
//! the other case; variant picks come from the engine's seeded generator.

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};
use std::rc::Rc;

use super::options::{check_keys, float};
use super::random::SharedRng;
use super::{Canvas, CanvasData, Path, Shared, StrokeStyle, Vec2};
use crate::journal::fonts::{Font, Mapping};
use crate::journal::symbol::Symbol;
use crate::journal::fonts::FontBook;

use super::RhaiResult;

/// One placed glyph: a symbol with a position, scale and rotation (radians,
/// around its own centre). Shared like paths and canvases (see `Shared`).
pub type Glyph = Shared<Symbol>;

fn register_glyph(engine: &mut Engine) {
    engine.register_type_with_name::<Glyph>("Glyph")
        .register_fn("copy", Glyph::copy)
        .register_get_set("position",
            |g: &mut Glyph| { let p = g.borrow().position(); Vec2::new(p.x as f64, p.y as f64) },
            |g: &mut Glyph, position: Vec2| g.borrow_mut().set_position(position.into()))
        .register_get_set("scale",
            |g: &mut Glyph| Dynamic::from(g.borrow().scale() as f64),
            |g: &mut Glyph, scale: Dynamic| -> RhaiResult<()> {
                g.borrow_mut().set_scale(float("glyph.scale", "the scale", &scale)? as f32);
                Ok(())
            })
        .register_get_set("rotation",
            |g: &mut Glyph| Dynamic::from(g.borrow().rotation_rad() as f64),
            |g: &mut Glyph, rotation: Dynamic| -> RhaiResult<()> {
                g.borrow_mut().set_rotation_rad(float("glyph.rotation", "the rotation", &rotation)? as f32);
                Ok(())
            })
        .register_get("is_blank", |g: &mut Glyph| g.borrow().is_blank())
        // Like the path functions, these change the glyph and return it, so
        // they chain.
        .register_fn("translate_by", |g: &mut Glyph, delta: Vec2| g.update(|s| s.translate_by(delta.into())))
        .register_fn("scale_by", |g: &mut Glyph, factor: Dynamic| -> RhaiResult<Glyph> {
            let factor = float("scale_by", "the scale", &factor)? as f32;
            Ok(g.update(|s| s.scale_by(factor)))
        })
        .register_fn("rotate_by", |g: &mut Glyph, rad: Dynamic| -> RhaiResult<Glyph> {
            let rad = float("rotate_by", "the angle", &rad)? as f32;
            Ok(g.update(|s| s.rotate_rad_by(rad)))
        })
        // The glyph's strokes, placed, as a new path. Empty for blanks.
        .register_fn("path", |g: &mut Glyph| Path::new(g.borrow().placed_strokepath()));
}

/// Add (a copy of) a glyph's strokes to a canvas; blanks add nothing.
pub(super) fn add_glyph(canvas: &mut CanvasData, glyph: &Glyph, style: &StrokeStyle) -> RhaiResult<()> {
    let glyph = glyph.borrow();
    if !glyph.is_blank() {
        let path = glyph.placed_strokepath();
        super::check_finite("add_glyph", &path)?;
        canvas.add_path(path, style.clone());
    }
    Ok(())
}

/// Glyphs from a rhai array, failing on anything that isn't one.
pub(super) fn glyph_array(caller: &str, glyphs: Array) -> RhaiResult<Vec<Glyph>> {
    glyphs.into_iter().enumerate().map(|(i, g)| {
        let type_name = g.type_name();
        g.try_cast::<Glyph>()
            .ok_or_else(|| format!("{}: item {} is a {}, not a Glyph", caller, i, type_name).into())
    }).collect()
}

/// A font from the book, by name.
fn named(book: &FontBook, caller: &str, name: &str) -> RhaiResult<Font> {
    book.font(name).ok_or_else(|| format!("{}: no font named '{}'; fonts: {}", caller, name, book.names().join(", ")).into())
}

/// A font, or a font's name, as `fallback` takes; `()` for none.
fn fallback_font(book: &FontBook, caller: &str, value: Dynamic) -> RhaiResult<Option<Box<Font>>> {
    if value.is_unit() {
        return Ok(None);
    }
    if value.is::<Font>() {
        return Ok(Some(Box::new(value.cast::<Font>())));
    }
    let type_name = value.type_name();
    let name = value.into_immutable_string()
        .map_err(|_| -> Box<EvalAltResult> { format!("{}: a fallback is a font, a font's name or (), got {}", caller, type_name).into() })?;
    Ok(Some(Box::new(named(book, caller, &name)?)))
}

fn set_padding(font: &mut Font, caller: &str, value: &Dynamic) -> RhaiResult<()> {
    let padding = float(caller, "character_padding", value)?;
    if padding < 0.0 {
        return Err(format!("{}: character_padding can't be negative, got {}", caller, padding).into());
    }
    font.character_padding = padding as f32;
    Ok(())
}

fn set_space(font: &mut Font, caller: &str, value: &Dynamic) -> RhaiResult<()> {
    let width = float(caller, "space_width", value)?;
    if !(width > 0.0) {
        return Err(format!("{}: space_width must be positive, got {}", caller, width).into());
    }
    font.space_width = width as f32;
    Ok(())
}

/// A setting as scripts read it: by way of its shortest decimal form, so
/// 0.3 reads back as 0.3 rather than 0.30000001192092896.
fn setting(value: f32) -> Dynamic {
    Dynamic::from(value.to_string().parse::<f64>().expect("a float's string form parses"))
}

/// A glyph's name, from a string or a number (as `ternary.glyph(-13)`).
fn glyph_key(key: &Dynamic) -> String {
    key.to_string()
}

pub fn register(engine: &mut Engine, symdict: Rc<FontBook>, rng: SharedRng) {
    engine.register_type_with_name::<Font>("Font");
    register_glyph(engine);

    let book = symdict;
    let b = book.clone();
    engine.register_fn("font", move |name: &str| named(&b, "font", name));
    let b = book.clone();
    engine.register_fn("font", move |name: &str, options: Map| -> RhaiResult<Font> {
        const CALLER: &str = "font";
        check_keys(CALLER, &options, &["character_padding", "space_width", "fallback"])?;
        let mut font = named(&b, CALLER, name)?;
        if let Some(value) = options.get("character_padding") {
            set_padding(&mut font, CALLER, value)?;
        }
        if let Some(value) = options.get("space_width") {
            set_space(&mut font, CALLER, value)?;
        }
        if let Some(value) = options.get("fallback") {
            font.fallback = fallback_font(&b, CALLER, value.clone())?;
        }
        Ok(font)
    });
    let b = book.clone();
    engine.register_fn("ternary_font", move |name: &str| -> RhaiResult<Font> {
        let mut font = named(&b, "ternary_font", name)?;
        font.mapping = Mapping::BalancedTernary;
        Ok(font)
    });
    let b = book.clone();
    engine.register_fn("font_names", move || -> Array { b.names().into_iter().map(Dynamic::from).collect() });

    engine.register_get("name", |f: &mut Font| f.name().to_string());
    engine.register_get_set("character_padding",
        |f: &mut Font| setting(f.character_padding),
        |f: &mut Font, value: Dynamic| set_padding(f, "font.character_padding", &value));
    engine.register_get_set("space_width",
        |f: &mut Font| setting(f.space_width),
        |f: &mut Font, value: Dynamic| set_space(f, "font.space_width", &value));
    let b = book.clone();
    engine.register_get_set("fallback",
        |f: &mut Font| f.fallback.as_deref().map_or(Dynamic::UNIT, |fallback| Dynamic::from(fallback.clone())),
        move |f: &mut Font, value: Dynamic| -> RhaiResult<()> {
            f.fallback = fallback_font(&b, "font.fallback", value)?;
            Ok(())
        });
    engine.register_fn("characters", |f: &mut Font| -> Array { f.characters().into_iter().map(Dynamic::from).collect() });
    engine.register_fn("variants", |f: &mut Font, key: Dynamic| -> RhaiResult<Array> {
        let key = glyph_key(&key);
        let variants = f.variants(&key)
            .ok_or_else(|| format!("variants: font '{}' has no glyph '{}'", f.name(), key))?;
        Ok(variants.into_iter().map(|symbol| Dynamic::from(Glyph::new(symbol))).collect())
    });
    // The font's own glyph for a character or name: its first variant. (Its
    // fallbacks aren't looked in; `glyphs` sets text as text is set.)
    engine.register_fn("glyph", |f: &mut Font, key: Dynamic| -> RhaiResult<Glyph> {
        let key = glyph_key(&key);
        f.variants(&key).and_then(|v| v.into_iter().next()).map(Glyph::new)
            .ok_or_else(|| format!("glyph: font '{}' has no glyph '{}'", f.name(), key).into())
    });
    let b = book;
    engine.register_fn("glyphs", move |font: &mut Font, text: &str| -> Array {
        font.build(text, &b, &mut *rng.borrow_mut())
            .into_iter()
            .map(|symbol| Dynamic::from(Glyph::new(symbol)))
            .collect()
    });

    engine.register_fn("add_glyph", |canvas: &mut Canvas, glyph: Glyph, style: StrokeStyle| -> RhaiResult<()> {
        add_glyph(&mut canvas.borrow_mut(), &glyph, &style)
    });
    engine.register_fn("add_glyphs", |canvas: &mut Canvas, glyphs: Array, style: StrokeStyle| -> RhaiResult<()> {
        let mut canvas = canvas.borrow_mut();
        for glyph in glyph_array("add_glyphs", glyphs)? {
            add_glyph(&mut canvas, &glyph, &style)?;
        }
        Ok(())
    });
}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use super::*;

    fn engine() -> JournalEngine {
        let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_FONTS).expect("Failed to load fonts.json");
        JournalEngine::new(Rc::new(symd))
    }

    fn eval<T: Clone + 'static>(script: &str) -> T {
        engine().engine.eval::<T>(script).expect("script should evaluate")
    }

    fn fails(script: &str) -> String {
        engine().engine.run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn ternary_font_maps_letters_to_balanced_symbols() {
        let ok: bool = eval(r#"
            let t = ternary_font("balanced_ternary");
            let g = t.glyphs("aZ ");
            let want = [1, -1, 0];
            let same = true;
            for (key, i) in want {
                if g[i].path().to_string() != t.glyph(key).path().to_string() { same = false; }
            }
            same
        "#);
        assert!(ok);
    }

    #[test]
    fn fonts_list_their_glyphs() {
        let names: Array = eval(r#"font("archangel").characters()"#);
        let names: Vec<String> = names.into_iter().map(|n| n.into_string().unwrap()).collect();
        assert_eq!(names, (0..7).map(|i| i.to_string()).collect::<Vec<_>>());
        let ok: bool = eval(r#"
            font_names().contains("copper") && font("copper").name == "copper"
                && font("copper").variants("a").len() == 4 && font("archangel").glyph(3).scale == 1.0
        "#);
        assert!(ok);
    }

    #[test]
    fn font_settings_can_be_changed() {
        let values: Array = eval(r#"
            let f = font("loche_lower");
            let file = f.character_padding;
            f.character_padding = 0.3;
            f.space_width = 1;
            let g = font("loche_lower", #{ character_padding: 0, space_width: 0.2, fallback: "loche_upper" });
            let wide = f.glyphs("a")[0].path();
            [file, f.character_padding, f.space_width, g.character_padding, g.space_width, g.fallback.name, font("loche_lower").character_padding]
        "#);
        assert_eq!(values[1].as_float().unwrap(), 0.3);
        assert_eq!(values[2].as_float().unwrap(), 1.0);
        assert_eq!(values[3].as_float().unwrap(), 0.0);
        assert!((values[4].as_float().unwrap() - 0.2).abs() < 1e-6);
        assert_eq!(values[5].clone().into_string().unwrap(), "loche_upper");
        assert_eq!(values[0].as_float().unwrap(), values[6].as_float().unwrap(), "changing a copy leaves the font file's alone");
        // A fallback can be dropped.
        let ok: bool = eval(r#"let c = font("copper"); c.fallback = (); c.fallback == ()"#);
        assert!(ok);
    }

    #[test]
    fn glyph_scale_is_its_size() {
        // Paths span -1..1 and are drawn at half the scale: a glyph at scale
        // 2 is its path as stored, 2 units across for a 2-unit wide path.
        let width: f64 = eval(r#"
            let p = font("symbols").glyph("X").scale_by(2.0).path();
            p.bounds().width
        "#);
        assert!((width - 2.0).abs() < 1e-3, "{}", width);
    }

    #[test]
    fn glyph_properties_and_chained_transforms() {
        let values: Array = eval(r#"
            let g = font("symbols").glyph("basic_eye");
            g.scale = 10.0;
            g.position = new_vec(1.0, 2.0);
            g.rotation = 0.5;
            g = g.translate_by(new_vec(1.0, 1.0)).scale_by(3.0).rotate_by(0.25);
            [g.scale, g.position.x, g.position.y, g.rotation]
        "#);
        let values: Vec<f64> = values.into_iter().map(|v| v.as_float().unwrap()).collect();
        assert_eq!(values, vec![30.0, 2.0, 3.0, 0.75]);
    }

    #[test]
    fn spaces_are_blank_glyphs_and_add_nothing() {
        let script = r#"
            let style = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let g = font("loche_upper").glyphs("a b");
            if !g[1].is_blank || g[0].is_blank { throw "expected only the space to be blank"; }
            let canvas = new_canvas();
            canvas.add_glyphs(g, style);
            let page = new_page();
            page.add_canvas(canvas);
            journal_entry.add_page(page);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        assert_eq!(entry.pages[0].canvases()[0].strokes.len(), 2);
    }

    #[test]
    fn fonts_fall_back_and_to_the_other_case() {
        // copper has no '?' but falls back to loche_upper, which has; it has
        // no capital Q, but has a q; loche_upper's capitals set lowercase.
        let missing: rhai::Map = eval(r#"
            font("copper").glyphs("?Q");
            font("loche_upper").glyphs("abc");
            missing_glyphs()
        "#);
        assert!(missing.is_empty(), "{:?}", missing);
        let missing: rhai::Map = eval(r#"
            let c = font("copper");
            c.fallback = ();
            c.glyphs("?");
            missing_glyphs()
        "#);
        assert!(missing.contains_key("?"), "without its fallback copper has no '?'");
    }

    #[test]
    fn unknown_fonts_and_glyphs_fail_at_their_line() {
        for (script, want) in [
            ("let ok = 1;\nternary_font(\"no_such_font\");", "ternary_font: no font named 'no_such_font'"),
            ("let ok = 1;\nfont(\"coper\");", "font: no font named 'coper'; fonts: "),
            ("let ok = 1;\nfont(\"copper\", #{ padding: 1 });", "font: unknown option 'padding'"),
            ("let ok = 1;\nfont(\"copper\", #{ character_padding: -1 });", "character_padding can't be negative"),
            ("let ok = 1;\nlet f = font(\"copper\"); f.space_width = 0;", "space_width must be positive"),
            ("let ok = 1;\nlet f = font(\"copper\"); f.fallback = 3;", "a fallback is a font, a font's name or ()"),
            ("let ok = 1;\nfont(\"symbols\").glyph(\"no_such_glyph\");", "glyph: font 'symbols' has no glyph 'no_such_glyph'"),
            ("let ok = 1;\nnew_canvas().add_glyphs([font(\"symbols\").glyph(\"X\"), 3], new_style(1.0, \"black\", new_brush(\"Ink Pen\", 0.3, 0.7, 1.0)));", "add_glyphs: item 1"),
        ] {
            let err = fails(script);
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }
}
