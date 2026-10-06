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
//!
//! // A for loop's variable is a copy: to change an array's glyphs, index it.
//! let word = copper.glyphs("ink");
//! for i in 0..word.len() { word[i].position = new_vec(i * 20.0, 0.0); }
//! canvas.add_glyphs(word, style);
//! ```
//!
//! Variant picks come from the engine's seeded generator.

use rhai::{Array, CustomType, Dynamic, Engine, EvalAltResult, TypeBuilder};
use std::rc::Rc;

use super::random::SharedRng;
use super::{Canvas, StrokeStyle, Vec2};
use crate::journal::paths::StrokePath;
use crate::journal::styles::{Font, FontSet};
use crate::journal::symbol::Symbol;
use crate::journal::symboldict::SymbolPathDict;

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

/// One placed glyph: a symbol with a position, scale and rotation (radians,
/// around its own centre).
#[derive(Clone, Debug)]
pub struct Glyph(pub Symbol);

impl Glyph {
    fn position(&mut self) -> Vec2 {
        let p = self.0.position();
        Vec2::new(p.x as f64, p.y as f64)
    }
    fn set_position(&mut self, position: Vec2) {
        self.0.set_position(position.into());
    }
    fn scale(&mut self) -> f64 {
        self.0.scale() as f64
    }
    fn set_scale(&mut self, scale: f64) {
        self.0.set_scale(scale as f32);
    }
    fn rotation(&mut self) -> f64 {
        self.0.rotation_rad() as f64
    }
    fn set_rotation(&mut self, rotation: f64) {
        self.0.set_rotation_rad(rotation as f32);
    }
    fn is_blank(&mut self) -> bool {
        self.0.is_blank()
    }
    // Like the path functions, these modify the glyph in place and return
    // it, so they can be chained.
    fn translate_by(&mut self, delta: Vec2) -> Glyph {
        self.0.translate_by(delta.into());
        self.clone()
    }
    fn scale_by(&mut self, factor: f64) -> Glyph {
        self.0.scale_by(factor as f32);
        self.clone()
    }
    fn rotate_by(&mut self, rad: f64) -> Glyph {
        self.0.rotate_rad_by(rad as f32);
        self.clone()
    }
    /// The glyph's strokes, placed. Empty for blanks (spaces).
    fn path(&mut self) -> StrokePath {
        self.0.placed_strokepath()
    }
}

impl CustomType for Glyph {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Glyph")
            .with_get_set("position", Self::position, Self::set_position)
            .with_get_set("scale", Self::scale, Self::set_scale)
            .with_get_set("rotation", Self::rotation, Self::set_rotation)
            .with_get("is_blank", Self::is_blank)
            .with_fn("translate_by", Self::translate_by)
            .with_fn("scale_by", Self::scale_by)
            .with_fn("rotate_by", Self::rotate_by)
            .with_fn("path", Self::path);
    }
}

/// Add a glyph's strokes to a canvas; blanks add nothing.
pub(super) fn add_glyph(canvas: &mut Canvas, glyph: &Glyph, style: &StrokeStyle) {
    if !glyph.0.is_blank() {
        canvas.add_strokepath(glyph.0.placed_strokepath(), style.clone());
    }
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
    engine.build_type::<Glyph>();

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
        checked(&sd, "lowercase_font", Font::Lowercase(prefix.to_string()))
    });

    let sd = symdict.clone();
    engine.register_fn("glyphs", move |font: &mut Font, text: &str| -> Array {
        font.builder(&sd)
            .build(text, &mut *rng.borrow_mut())
            .into_iter()
            .map(|symbol| Dynamic::from(Glyph(symbol)))
            .collect()
    });
    let sd = symdict;
    engine.register_fn("glyph", move |name: &str| -> RhaiResult<Glyph> {
        sd.get_symbol_base(name)
            .map(|base| Glyph(Symbol::new_default_from_base_rc(base)))
            .ok_or_else(|| format!("glyph: unknown symbol '{}'", name).into())
    });

    engine.register_fn("add_glyph", |canvas: &mut Canvas, glyph: Glyph, style: StrokeStyle| {
        add_glyph(canvas, &glyph, &style);
    });
    engine.register_fn("add_glyphs", |canvas: &mut Canvas, glyphs: Array, style: StrokeStyle| -> RhaiResult<()> {
        for glyph in glyph_array("add_glyphs", glyphs)? {
            add_glyph(canvas, &glyph, &style);
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
