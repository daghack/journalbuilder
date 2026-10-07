//! Fonts and glyphs for rhai scripts.
//!
//! ```rhai
//! let copper = new_fontset(["copper_upper"], ["copper_lower"]);
//! let ternary = ternary_font("balanced_ternary");   // a..m -> 1..13, n..z -> -13..-1
//! let loche = lowercase_font("loche_upper");
//!
//! // Glyphs sit at the origin with scale 1 until moved. A glyph's path spans
//! // -1..1, drawn at half its scale, so scale is its size in canvas units.
//! for (g, i) in ternary.glyphs("abc") {
//!     g.scale = 30.0;
//!     g.position = new_vec(i * 40.0, 0.0);
//!     g.rotate_by(0.1);
//!     canvas.add_glyph(g, style);
//! }
//! canvas.add_glyph(glyph("basic_eye").scale_by(50.0), style);
//! glyph_names("archangel_");    // every glyph whose name starts so, sorted
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
//! Variant picks come from the engine's seeded generator.

use rhai::{Array, Dynamic, Engine};
use std::rc::Rc;

use super::options::float;
use super::random::SharedRng;
use super::{Canvas, CanvasData, Path, Shared, StrokeStyle, Vec2};
use crate::journal::fonts::{Font, FontSet};
use crate::journal::symbol::Symbol;
use crate::journal::symboldict::SymbolPathDict;

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
        canvas.add_strokepath(path, style.clone());
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

fn font_names(caller: &str, names: Array) -> RhaiResult<Vec<String>> {
    names.into_iter().map(|name| {
        name.into_immutable_string()
            .map(|s| s.to_string())
            .map_err(|type_name| format!("{}: font names must be strings, got {}", caller, type_name).into())
    }).collect()
}

/// `font`, if the dictionary has glyphs for every font it names.
fn checked(symdict: &SymbolPathDict, caller: &str, font: Font) -> RhaiResult<Font> {
    for prefix in font.prefixes() {
        if symdict.symbols_with_prefix(&format!("{}_", prefix)).is_empty() {
            return Err(format!("{}: no glyphs named '{}_*'", caller, prefix).into());
        }
    }
    Ok(font)
}

pub fn register(engine: &mut Engine, symdict: Rc<SymbolPathDict>, rng: SharedRng) {
    engine.register_type_with_name::<Font>("Font");
    register_glyph(engine);

    let sd = symdict.clone();
    engine.register_fn("new_fontset", move |upper: Array, lower: Array| -> RhaiResult<Font> {
        let set = FontSet::new(font_names("new_fontset", upper)?, font_names("new_fontset", lower)?);
        checked(&sd, "new_fontset", Font::Set(set))
    });
    let sd = symdict.clone();
    engine.register_fn("ternary_font", move |prefix: &str| -> RhaiResult<Font> {
        checked(&sd, "ternary_font", Font::Ternary(prefix.to_string()))
    });
    let sd = symdict.clone();
    engine.register_fn("lowercase_font", move |prefix: &str| -> RhaiResult<Font> {
        checked(&sd, "lowercase_font", Font::lowercase(prefix))
    });

    let sd = symdict.clone();
    engine.register_fn("glyphs", move |font: &mut Font, text: &str| -> Array {
        font.builder(&sd)
            .build(text, &mut *rng.borrow_mut())
            .into_iter()
            .map(|symbol| Dynamic::from(Glyph::new(symbol)))
            .collect()
    });
    // Every glyph name starting with `prefix`, sorted ("" for all of them).
    let sd = symdict.clone();
    engine.register_fn("glyph_names", move |prefix: &str| -> Array {
        sd.symbols_with_prefix(prefix).into_iter().map(Dynamic::from).collect()
    });
    let sd = symdict;
    engine.register_fn("glyph", move |name: &str| -> RhaiResult<Glyph> {
        sd.get_symbol_base(name)
            .map(|base| Glyph::new(Symbol::new_default_from_base_rc(base)))
            .ok_or_else(|| format!("glyph: unknown symbol '{}'", name).into())
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
        let symd = SymbolPathDict::from_file(crate::journal::symboldict::DEFAULT_SYMBOLS).expect("Failed to load symbols.json");
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
            let g = ternary_font("balanced_ternary").glyphs("aZ ");
            let want = ["balanced_ternary_1", "balanced_ternary_-1", "balanced_ternary_0"];
            let same = true;
            for (name, i) in want {
                if g[i].path().to_string() != glyph(name).path().to_string() { same = false; }
            }
            same
        "#);
        assert!(ok);
    }

    #[test]
    fn glyph_names_lists_the_dictionary() {
        let names: Array = eval(r#"glyph_names("archangel_")"#);
        let names: Vec<String> = names.into_iter().map(|n| n.into_string().unwrap()).collect();
        assert_eq!(names, (0..7).map(|i| format!("archangel_{}", i)).collect::<Vec<_>>());
        let all: Array = eval(r#"glyph_names("")"#);
        assert!(all.len() > 300);
    }

    #[test]
    fn glyph_scale_is_its_size() {
        // Paths span -1..1 and are drawn at half the scale, so a glyph at
        // scale 2 draws its path unchanged.
        let ok: bool = eval(r#"
            glyph("basic_eye").scale_by(2.0).path().to_string() == load_symbol("basic_eye").to_string()
        "#);
        assert!(ok);
    }

    #[test]
    fn glyph_properties_and_chained_transforms() {
        let values: Array = eval(r#"
            let g = glyph("basic_eye");
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
            let style = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let g = lowercase_font("loche_upper").glyphs("a b");
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
    fn font_sets_fall_back_to_the_other_case() {
        // copper_upper has neither '?' nor a capital Q; the lower fonts
        // (loche_upper, copper_lower) do.
        let missing: rhai::Map = eval(r#"
            let g = new_fontset(["copper_upper"], ["loche_upper"]).glyphs("?");
            let h = new_fontset(["copper_upper"], ["copper_lower"]).glyphs("Q");
            missing_glyphs()
        "#);
        assert!(missing.is_empty(), "{:?}", missing);
    }

    #[test]
    fn unknown_fonts_and_glyphs_fail_at_their_line() {
        for (script, want) in [
            ("let ok = 1;\nternary_font(\"no_such_font\");", "ternary_font: no glyphs named 'no_such_font_*'"),
            ("let ok = 1;\nlowercase_font(\"no_such_font\");", "lowercase_font: no glyphs named 'no_such_font_*'"),
            ("let ok = 1;\nnew_fontset([\"copper_upper\"], [\"coper_lower\"]);", "new_fontset: no glyphs named 'coper_lower_*'"),
            ("let ok = 1;\nnew_fontset([\"copper_upper\"], [3]);", "font names must be strings"),
            ("let ok = 1;\nglyph(\"no_such_glyph\");", "glyph: unknown symbol 'no_such_glyph'"),
            ("let ok = 1;\nnew_canvas().add_glyphs([glyph(\"basic_eye\"), 3], new_strokestyle(1.0, \"black\", new_brush(\"Ink Pen\", 0.3, 0.7, 1.0)));", "add_glyphs: item 1"),
        ] {
            let err = fails(script);
            assert!(err.contains(want) && err.contains("line 2"), "{}", err);
        }
    }
}
