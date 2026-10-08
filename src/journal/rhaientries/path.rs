//! Paths for rhai scripts: building them from lines and curves, moving
//! them, and writing text along them.
//!
//! Every path function changes the path and returns that same (shared)
//! path, so statements, assignments and chains all work:
//! `p.move_to(a).line_to(b);` draws both segments into `p`.
//!
//! ```rhai
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

use rhai::{Dynamic, Engine, EvalAltResult, Map};
use super::RhaiResult;
use std::rc::Rc;

use super::options::{as_number, check_keys, float, number, positive};
use super::text::{line_mode, spacing};
use super::canvas::bounds_map;
use super::random::SharedRng;
use super::{avoid, random, Canvas, CanvasData, Path, StrokeStyle, Vec2};
use crate::journal::hatch::{FillRule, HatchOptions};
use crate::journal::canvas::textbox::TextBoxLineMode;
use crate::journal::paths::StrokePath;
use crate::journal::fonts::Font;
use crate::journal::symbol::{place_along_path, place_words_along_path, Symbol, SymbolPlacementArgs};
use crate::journal::fonts::FontBook;

/// Where text's glyphs come from: the fonts, and the generator
/// that picks glyph variants.
pub(super) type TextSource = (Rc<FontBook>, random::SharedRng);

/// Options for text along a path.
const PATH_TEXT_KEYS: [&str; 4] = ["spacing", "avoid", "clearance", "layout"];

/// Text along a path: glyph by glyph, or keeping words together on a
/// subpath ("words" layout).
fn place_text(symbols: Vec<Symbol>, path: &StrokePath, args: &SymbolPlacementArgs, mode: &TextBoxLineMode) -> (Vec<Symbol>, Vec<Symbol>) {
    match mode {
        TextBoxLineMode::Flat => place_along_path(symbols, path, args),
        TextBoxLineMode::WordsByBlankSymbol => place_words_along_path(symbols, path, args),
    }
}

/// `path.fill_with_text(text, font, size, style[, #{ spacing, avoid, clearance, layout }])`:
/// the text along as many copies of the path as it takes, one canvas per
/// copy. Glyphs sit at their widths, plus `spacing` (a fraction of the font
/// size), and keep clear of `avoid` (see avoid.rs). With `layout: "words"`,
/// words aren't split around what's avoided or between subpaths.
fn fill_with_text(path: &mut Path, msg: &str, font: Font, fontsize: Dynamic, style: StrokeStyle, options: Map, (symdict, rng): &TextSource) -> RhaiResult<Vec<Dynamic>> {
    let fontsize = float("fill_with_text", "the font size", &fontsize)?;
    check_keys("fill_with_text", &options, &PATH_TEXT_KEYS)?;
    if !(fontsize > 0.0) {
        return Err(format!("fill_with_text: font size must be positive, got {}", fontsize).into());
    }
    let mode = line_mode("fill_with_text", &options)?;
    let avoid = avoid::AvoidOption::parse("fill_with_text", &options, false)?;
    let tree = avoid.tree.as_ref().map(|a| a.borrow());
    let args = SymbolPlacementArgs {
        avoid: tree.as_deref(),
        padding: avoid.padding(fontsize as f32),
        spacing: spacing("fill_with_text", &options)?,
        ..SymbolPlacementArgs::new(fontsize as f32)
    };

    let mut all_canvases = Vec::new();
    let mut symbols = font.build(msg, symdict, &mut *rng.borrow_mut());
    let path = path.get();
    let total = symbols.len();
    while !symbols.is_empty() {
        let (placed, unplaced) = place_text(std::mem::take(&mut symbols), &path, &args, &mode);
        if placed.is_empty() {
            // Another copy of the path wouldn't fit it either.
            return Err(format!(
                "fill_with_text: glyph {} of {} doesn't fit on the path at font size {}; use a longer path or a smaller font{}",
                total - unplaced.len() + 1, total, fontsize,
                if avoid.tree.is_some() { ", or avoid less" } else { "" },
            ).into());
        }
        let mut canvas = CanvasData::default();
        for symbol in placed {
            canvas.add_path(symbol.placed_strokepath(), style.clone());
        }
        all_canvases.push(Dynamic::from(Canvas::new(canvas)));
        symbols = unplaced;
    }
    Ok(all_canvases)
}

/// `path.fit_text(text, font, style[, #{ spacing, avoid, clearance, layout }])`:
/// the text along the path at the largest font size that fits it.
fn fit_text(path: &mut Path, msg: &str, font: Font, style: StrokeStyle, options: Map, (symdict, rng): &TextSource) -> RhaiResult<Canvas> {
    check_keys("fit_text", &options, &PATH_TEXT_KEYS)?;
    let path = &path.get();
    let mut canvas = CanvasData::default();
    let symbols = font.build(msg, symdict, &mut *rng.borrow_mut());
    let mode = line_mode("fit_text", &options)?;
    let avoid = avoid::AvoidOption::parse("fit_text", &options, false)?;
    let tree = avoid.tree.as_ref().map(|a| a.borrow());
    let mut args = SymbolPlacementArgs {
        avoid: tree.as_deref(),
        spacing: spacing("fit_text", &options)?,
        ..SymbolPlacementArgs::new(20.0)
    };
    let mut window_min = 0.1;
    let mut window_max = 1000.0;
    while window_max - window_min > 0.005 {
        let current_scale = (window_max + window_min) / 2.0;
        args.symbol_scale = current_scale;
        args.padding = avoid.padding(current_scale);
        let (_, unplaced) = place_text(symbols.clone(), path, &args, &mode);
        if !unplaced.is_empty() {
            window_max = current_scale;
        } else {
            window_min = current_scale;
        }
    }
    args.symbol_scale = window_min;
    args.padding = avoid.padding(window_min);
    let (placed, unplaced) = place_text(symbols, path, &args, &mode);
    if !unplaced.is_empty() {
        return Err(format!(
            "fit_text: the text doesn't fit on the path even at font size {}: {} of {} glyphs left over",
            window_min, unplaced.len(), placed.len() + unplaced.len(),
        ).into());
    }
    for symbol in placed {
        canvas.add_path(symbol.placed_strokepath(), style.clone());
    }
    Ok(Canvas::new(canvas))
}

/// Apply a by-value StrokePath builder method to a path in place, returning
/// the same path.
fn update_path(path: &mut Path, f: impl FnOnce(StrokePath) -> StrokePath) -> Path {
    path.update(|p| *p = f(std::mem::take(p)))
}

pub(super) fn register(engine: &mut Engine, text: TextSource) {
    let rng = text.1.clone();
    let r = rng.clone();
    engine.register_fn("hatch", move |path: &mut Path, spacing: Dynamic, angle: Dynamic, roughness: Dynamic| -> RhaiResult<Path> {
        let mut options = Map::new();
        options.insert("spacing".into(), spacing);
        options.insert("angle".into(), angle);
        options.insert("roughness".into(), roughness);
        hatch(&path.borrow(), &hatch_options(&options)?, &r).map(Path::new)
    });
    engine.register_fn("hatch", move |path: &mut Path, options: Map| -> RhaiResult<Path> {
        hatch(&path.borrow(), &hatch_options(&options)?, &rng).map(Path::new)
    });

    let (t1, t2, t3, t4) = (text.clone(), text.clone(), text.clone(), text);
    engine.register_type_with_name::<Path>("Path")
        // Every path function changes the path and returns that same
        // (shared) path, so statements, assignments and chains all work:
        // `p.move_to(a).line_to(b);` draws both segments into `p`.
        .register_fn("copy", Path::copy)
        .register_fn("to_string", |p: &mut Path| p.borrow().to_string())
        .register_fn("to_debug", |p: &mut Path| p.borrow().to_string())
        .register_fn("move_to", |path: &mut Path, pos: Vec2| -> Path {
            update_path(path, |p| p.move_to(pos.x as f32, pos.y as f32))
        })
        .register_fn("line_to", |path: &mut Path, pos: Vec2| -> Path {
            update_path(path, |p| p.line_to(pos.x as f32, pos.y as f32))
        })
        .register_fn("fill_with_text", move |path: &mut Path, text: &str, font: Font, size: Dynamic, style: StrokeStyle| {
            fill_with_text(path, text, font, size, style, Map::new(), &t1)
        })
        .register_fn("fill_with_text", move |path: &mut Path, text: &str, font: Font, size: Dynamic, style: StrokeStyle, options: Map| {
            fill_with_text(path, text, font, size, style, options, &t2)
        })
        .register_fn("fit_text", move |path: &mut Path, text: &str, font: Font, style: StrokeStyle| {
            fit_text(path, text, font, style, Map::new(), &t3)
        })
        .register_fn("fit_text", move |path: &mut Path, text: &str, font: Font, style: StrokeStyle, options: Map| {
            fit_text(path, text, font, style, options, &t4)
        })
        .register_fn("scale_by", |path: &mut Path, scale: Dynamic| -> RhaiResult<Path> {
            let scale = float("scale_by", "the scale", &scale)?;
            Ok(path.update(|p| p.scale_by(scale as f32)))
        })
        .register_fn("translate_by", |path: &mut Path, translate: Vec2| -> Path {
            path.update(|p| p.translate_by(translate.into()))
        })
        .register_fn("rotate_around", |path: &mut Path, rot_point: Vec2, rot: Dynamic| -> RhaiResult<Path> {
            let rot = float("rotate_around", "the angle", &rot)?;
            Ok(path.update(|p| p.rotate_around(rot_point.into(), rot as f32)))
        })
        .register_fn("new_path", || Path::new(StrokePath::new()))
        .register_fn("cubic_to", |path: &mut Path, c1: Vec2, c2: Vec2, end: Vec2| -> Path {
            update_path(path, |p| p.cubic_curve_to(
                c1.x as f32, c1.y as f32, c2.x as f32, c2.y as f32, end.x as f32, end.y as f32,
            ))
        })
        .register_fn("quad_to", |path: &mut Path, control: Vec2, end: Vec2| -> Path {
            update_path(path, |p| p.quadratic_curve_to(
                control.x as f32, control.y as f32, end.x as f32, end.y as f32,
            ))
        })
        // `path.bounds()`: #{ min, max, center, width, height } of its line.
        .register_fn("bounds", |path: &mut Path| -> RhaiResult<Map> {
            let mut b = parry2d::bounding_volume::Aabb::new_invalid();
            for (points, _) in path.borrow().flattened(0.01) {
                for p in points {
                    b.take_point(p);
                }
            }
            bounds_map(b).ok_or_else(|| "bounds: the path is empty".into())
        })
        .register_fn("close", |path: &mut Path| -> Path {
            update_path(path, |p| p.close_path())
        })
        // `path.append(other)`: other's subpaths added to this path (a
        // copy of them), so several shapes are one path, e.g. to hatch
        // a shape with holes.
        .register_fn("append", |path: &mut Path, other: Path| -> Path {
            // Copy first: `p.append(p)` shares one value.
            let other = other.get();
            path.update(|p| p.append(other))
        })
        .register_fn("new_circle", |center: Vec2, radius: Dynamic| -> RhaiResult<Path> {
            let radius = float("new_circle", "the radius", &radius)?;
            Ok(Path::new(StrokePath::circle(center.into(), radius as f32)))
        })
        .register_fn("new_rectangle", |center: Vec2, width: Dynamic, height: Dynamic| -> RhaiResult<Path> {
            let (width, height) = (float("new_rectangle", "the width", &width)?, float("new_rectangle", "the height", &height)?);
            Ok(Path::new(StrokePath::rectangle(center.into(), width as f32, height as f32)))
        })
        .register_fn("new_line", |start: Vec2, end: Vec2| -> Path {
            Path::new(StrokePath::line(start.into(), end.into()))
        })
        .register_fn("new_arc", |center: Vec2, radius: Dynamic, start_rad: Dynamic, end_rad: Dynamic| -> RhaiResult<Path> {
            let number = |what: &str, v: &Dynamic| float("new_arc", what, v).map(|n| n as f32);
            let (radius, start, end) = (number("the radius", &radius)?, number("the start angle", &start_rad)?, number("the end angle", &end_rad)?);
            Ok(Path::new(StrokePath::arc(center.into(), radius, start, end)))
        });
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

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use crate::journal::fonts::FontBook;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_FONTS).expect("Failed to load fonts.json");
        JournalEngine::new(Rc::new(symd))
    }

    fn eval<T: Clone + 'static>(script: &str) -> T {
        let setup = r#"let style = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));"#;
        engine().engine.eval::<T>(&format!("{}\n{}", setup, script)).expect("script should evaluate")
    }

    fn fails(script: &str) -> String {
        engine().engine.run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn hatch_fills_inside_the_path_and_follows_the_seed() {
        let hatching = |seed: i64| -> String {
            let path: super::super::Path = eval(&format!(
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

    #[test]
    fn bad_hatch_options_fail_at_their_line() {
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
