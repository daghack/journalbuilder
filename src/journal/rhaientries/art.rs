//! Procedural art for rhai scripts, in the `art` module.
//!
//! ```rhai
//! // A snake seen from above: its head centred at (0, 0) facing up the page
//! // (angles are radians; 0 faces +x and PI() / 2.0 faces down the page;
//! // by default it faces away from the tail), the tip of its tail at
//! // (40, 400), 900 units long along its spine and 36 across at its widest
//! // (by default a twelfth of its length). The seed picks its winding (0 by
//! // default).
//! let snake = art::snake(vec2(0.0, 0.0), vec2(40.0, 400.0), 900.0, #{ head_angle: -PI() / 2.0, max_width: 36.0, seed: 7 });
//! canvas.add(snake.outline, ink);
//! ```
//!
//! `outline` is the snake's outline. Where the body crosses itself,
//! the outline of the strand underneath stops at the edge of the one on top
//! (the head always goes over), so the path is several strokes. The same
//! seed gives the same snake; it doesn't use or change the script's
//! generator (`seed`, `random`).
//!
//! `spine` is the same snake's centre line from the back of its head to
//! the tip of its tail, to write along:
//!
//! ```rhai
//! canvas.add(snake.spine.fill_with_text("a snake", #{ font: font, size: 30.0, style: ink }));
//! ```
//!
//! `bramble` grows a blackberry bramble and gives its parts as separate
//! paths, to stroke in different colours: a cane from (0, 0) growing up the
//! page (the default angle), 1400 long, with flowers about 200 across
//! (which everything else is sized by; by default a fifth of the length).
//!
//! ```rhai
//! let plant = art::bramble(vec2(0.0, 0.0), 1400.0, #{ angle: -PI() / 2.0, flower_size: 200.0, seed: 7 });
//! canvas.add(plant.vines, bark);     // canes, thorns and stalks
//! canvas.add(plant.leaves, green);   // leaflets and their veins
//! canvas.add(plant.flowers, white);  // flowers, buds and sepals
//! canvas.add(plant, #{ vines: bark, leaves: green, flowers: white });   // or all three
//! ```
//!
//! Flowers, from closed buds to full blooms, and leaves are modelled in 3D
//! and seen from all sides. Flowers lie over leaves and leaves over the
//! vines, and each part's lines stop where something in front covers them.
//!
//! `bramble_along` grows one along a path instead: a maze's, say. Vines
//! follow its lines, wandering a little off them, carrying straight on
//! through junctions with branches up the other ways, and flower at its dead
//! ends; leaves and flowers keep clear of the other vines. Lines that don't
//! touch grow separate vines.
//!
//! ```rhai
//! let path = new_maze(9, 7, #{ seed: 7 }).path().scale_by(70.0);
//! let plant = art::bramble_along(path, 38.0, #{ seed: 3 });   // flowers 38 across
//! ```
//!
//! Either takes `#{ origin, thickness, reach }` to grow canes thicker
//! toward a growth origin, as old wood is: `thickness` times as wide there
//! (default 3), easing back to their usual width `reach` away (default
//! as far as the canes go).
//!
//! ```rhai
//! let plant = art::bramble_along(path, 38.0, #{ seed: 3, origin: vec2(0, 0), thickness: 4 });
//! ```
//!
//! `snake_along` does the same for a snake, making a many-headed serpent:
//! its body follows the path, slithering a little off it and forking where
//! it forks, with its tail at the dead end nearest where the path starts
//! and a head at every other dead end.
//!
//! ```rhai
//! canvas.add(art::snake_along(path, 30.0, #{ seed: 2 }), ink);   // 30 wide
//! ```
//!
//! `eye` draws an eye as a hand sketches one in a journal, and gives it as
//! a canvas, inked and coloured: lids gone over and running on past the
//! corners, an iris ringed and ringed again and washed with `color`
//! (darkest round its edge), and a line down through the iris, bowed
//! toward where the eye looks. Here it's 300 from corner to corner, looking up
//! and to the right (the gaze angle is in radians, on the page), half way
//! between neat (0) and as messy as it gets (1).
//!
//! ```rhai
//! canvas.add(art::eye(vec2(0.0, 0.0), 300.0, #{ color: "#2a9bb8", gaze_angle: -PI() / 4.0, messiness: 0.5, seed: 7 }));
//! ```
//!
//! Options: `angle`, the eye's tilt (default 0, corners level); `gaze`,
//! how far it looks from 0 (ahead) to 1 (as far as it goes; default 0.5);
//! `openness`, its height over its length (default 0.32); `flat`, lids
//! of straight lines meeting in a point above and below the middle, as
//! the `symbols` font's eye has, rather than arcs (default false); and
//! `ink`, the style of its lines (default a near-black ink pen).
//!
//! ```rhai
//! let e = art::eye(vec2(0.0, 0.0), 300.0, #{ color: "#2a9bb8", gaze_angle: PI(), messiness: 0.8, seed: 7, angle: 1.2, gaze: 1.0, openness: 0.6, flat: true, ink: ink });
//! ```
//!
//! `moon` draws the moon in a phase: 0 new, 0.25 first quarter (right half
//! lit), 0.5 full, 0.75 last quarter, wrapping round at 1. It gives its
//! parts as paths: the disc's outline, the terminator between light and
//! dark, and the lit and dark sides as closed shapes, to hatch or fill.
//!
//! ```rhai
//! let moon = art::moon(vec2(0.0, 0.0), 80.0, 0.3);   // a waxing gibbous
//! canvas.add(moon.outline, ink);
//! canvas.add(moon.terminator, ink);
//! canvas.add(moon.dark.hatch(4.0, #{ angle: 0.8, roughness: 0.5 }), ink);
//! ```

use rhai::{Dynamic, Engine, EvalAltResult, Map, Module, NativeCallContext};

use super::gradient::colour_or_gradient;
use super::options::{check_keys, float, number};
use super::{BrushSettings, Canvas, CanvasData, Path, StrokeStyle, Vec2};
use crate::journal::art;

use super::RhaiResult;

/// An error at the script line of the call: rhai doesn't add it for
/// functions called through a module, as it does for global ones.
fn error(ctx: &NativeCallContext, message: String) -> Box<EvalAltResult> {
    EvalAltResult::ErrorRuntime(message.into(), ctx.call_position()).into()
}

/// A number argument (whole or decimal), failing at the call's line.
fn number_arg(ctx: &NativeCallContext, caller: &str, what: &str, v: &Dynamic) -> RhaiResult<f32> {
    float(caller, what, v).map(|n| n as f32).map_err(|err| error(ctx, err.to_string()))
}

/// A seed option: which drawing, 0 unless given.
fn seed(ctx: &NativeCallContext, caller: &str, options: &Map) -> RhaiResult<u64> {
    match options.get("seed") {
        None => Ok(0),
        Some(seed) => seed.as_int().map(|s| s as u64)
            .map_err(|t| error(ctx, format!("{}: 'seed' should be a whole number, got {}", caller, t))),
    }
}

/// A number option, `default` unless given.
fn number_option(ctx: &NativeCallContext, caller: &str, options: &Map, key: &str, default: f32) -> RhaiResult<f32> {
    number(caller, options, key).map(|n| n.unwrap_or(default)).map_err(|err| error(ctx, err.to_string()))
}

const THICKENING_KEYS: [&str; 3] = ["origin", "thickness", "reach"];

/// A bramble's `origin`, `thickness` and `reach`: canes thicker toward a
/// growth origin, if there is one.
fn thickening(ctx: &NativeCallContext, caller: &str, options: &Map) -> RhaiResult<Option<art::Thickening>> {
    if !THICKENING_KEYS.iter().any(|k| options.contains_key(*k)) {
        return Ok(None);
    }
    let origin = options.get("origin")
        .ok_or_else(|| error(ctx, format!("{}: 'thickness' and 'reach' need an 'origin', the point canes thicken toward", caller)))?
        .clone().try_cast::<Vec2>()
        .ok_or_else(|| error(ctx, format!("{}: 'origin' should be a vec2", caller)))?;
    let thickness = number_option(ctx, caller, options, "thickness", 3.0)?;
    let reach = number(caller, options, "reach").map_err(|err| error(ctx, err.to_string()))?;
    Ok(Some(art::Thickening { origin: origin.into(), thickness, reach }))
}

/// `art::bramble(base, length[, #{ angle, flower_size, seed, origin, thickness, reach }])`.
fn bramble(ctx: &NativeCallContext, base: Vec2, length: &Dynamic, options: &Map) -> RhaiResult<Map> {
    const CALLER: &str = "art::bramble";
    check_keys(CALLER, options, &["angle", "flower_size", "seed", "origin", "thickness", "reach"]).map_err(|err| error(ctx, err.to_string()))?;
    let length = number_arg(ctx, CALLER, "length", length)?;
    let angle = number_option(ctx, CALLER, options, "angle", -std::f32::consts::FRAC_PI_2)?;
    let flower_size = number_option(ctx, CALLER, options, "flower_size", length / 5.0)?;
    let bramble = art::bramble(base.into(), angle, length, flower_size, seed(ctx, CALLER, options)?, thickening(ctx, CALLER, options)?)
        .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    Ok(parts(bramble))
}

/// `art::bramble_along(path, flower_size[, #{ seed, origin, thickness, reach }])`.
fn bramble_along(ctx: &NativeCallContext, path: &Path, flower_size: &Dynamic, options: &Map) -> RhaiResult<Map> {
    const CALLER: &str = "art::bramble_along";
    check_keys(CALLER, options, &["seed", "origin", "thickness", "reach"]).map_err(|err| error(ctx, err.to_string()))?;
    let flower_size = number_arg(ctx, CALLER, "flower_size", flower_size)?;
    let bramble = art::bramble_along(&path.borrow(), flower_size, seed(ctx, CALLER, options)?, thickening(ctx, CALLER, options)?)
        .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    Ok(parts(bramble))
}

/// `art::snake(head_center, tail_end, length[, #{ head_angle, max_width, seed }])`:
/// `#{ outline, spine }`.
fn snake(ctx: &NativeCallContext, head: Vec2, tail: Vec2, length: &Dynamic, options: &Map) -> RhaiResult<Map> {
    const CALLER: &str = "art::snake";
    check_keys(CALLER, options, &["head_angle", "max_width", "seed"]).map_err(|err| error(ctx, err.to_string()))?;
    let length = number_arg(ctx, CALLER, "length", length)?;
    let (h, t): (parry2d::math::Vec2, parry2d::math::Vec2) = (head.into(), tail.into());
    let away = (h - t).y.atan2((h - t).x);
    let head_angle = number_option(ctx, CALLER, options, "head_angle", away)?;
    let max_width = number_option(ctx, CALLER, options, "max_width", length / 12.0)?;
    let seed = seed(ctx, CALLER, options)?;
    let fail = |err: String| error(ctx, format!("{}: {}", CALLER, err));
    let outline = art::snake_path(h, head_angle, t, length, max_width, seed).map_err(fail)?;
    let spine = art::snake_spine(h, head_angle, t, length, max_width, seed).map_err(fail)?;
    let mut parts = Map::new();
    parts.insert("outline".into(), Dynamic::from(Path::new(outline)));
    parts.insert("spine".into(), Dynamic::from(Path::new(spine)));
    Ok(parts)
}

/// A bramble's parts, by name.
fn parts(bramble: art::Bramble) -> Map {
    let mut parts = Map::new();
    for (name, path) in [("vines", bramble.vines), ("leaves", bramble.leaves), ("flowers", bramble.flowers)] {
        parts.insert(name.into(), Dynamic::from(Path::new(path)));
    }
    parts
}

const EYE_KEYS: [&str; 9] = ["color", "gaze_angle", "messiness", "seed", "angle", "gaze", "openness", "flat", "ink"];

/// A style for an eye's strokes: `brush` at `size`, in `color`.
/// Tapered at both ends, as quick strokes are.
fn eye_style(color: &crate::journal::canvas::StrokeColor, brush: &str, min: f64, max: f64, size: f32) -> StrokeStyle {
    let mut brush = BrushSettings::new_brush(brush, min, max, size as f64);
    brush.pressure_profile = crate::journal::canvas::PressureProfile::Taper;
    StrokeStyle { width: 1.0, color: color.clone(), brush, passes: 1, jitter: 0.0 }
}

/// `art::eye(center, length[, #{ color, gaze_angle, messiness, seed, angle, gaze, openness, flat, ink }])`.
fn eye(ctx: &NativeCallContext, center: Vec2, length: &Dynamic, options: &Map) -> RhaiResult<Canvas> {
    const CALLER: &str = "art::eye";
    let fail = |err: Box<EvalAltResult>| error(ctx, err.to_string());
    check_keys(CALLER, options, &EYE_KEYS).map_err(fail)?;
    let color = colour_or_gradient(CALLER, options.get("color").cloned().unwrap_or_else(|| "#2b1d12".into())).map_err(fail)?;
    let gaze_angle = number_option(ctx, CALLER, options, "gaze_angle", 0.0)?;
    let messiness = number_option(ctx, CALLER, options, "messiness", 0.3)?;
    let seed = seed(ctx, CALLER, options)?;
    let defaults = art::EyeShape::default();
    let option = |key: &str, default: f32| number(CALLER, options, key).map(|n| n.unwrap_or(default)).map_err(fail);
    let flat = match options.get("flat") {
        None => defaults.flat,
        Some(flat) => flat.as_bool()
            .map_err(|type_name| error(ctx, format!("{}: 'flat' should be true or false, got {}", CALLER, type_name)))?,
    };
    let shape = art::EyeShape { angle: option("angle", defaults.angle)?, gaze: option("gaze", defaults.gaze)?, openness: option("openness", defaults.openness)?, flat };
    let ink = match options.get("ink") {
        None => eye_style(&crate::journal::canvas::StrokeColor::Solid("#1e1915".into()), "Ink Pen", 0.25, 0.7, 1.1),
        Some(ink) => ink.clone().try_cast::<StrokeStyle>()
            .ok_or_else(|| error(ctx, format!("{}: 'ink' should be a style (new_style), got {}", CALLER, ink.type_name())))?,
    };
    let eye = art::eye(center.into(), number_arg(ctx, CALLER, "length", length)?, gaze_angle, messiness, shape, seed)
        .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    // The wash first, then its pooled rim, the coloured outline and the
    // ink over them all.
    let r = eye.iris_radius;
    let mut canvas = CanvasData::default();
    canvas.add_path(eye.wash, eye_style(&color, "Smooth Watercolor", 0.2, 0.35, r * 0.3));
    canvas.add_path(eye.rim, eye_style(&color, "Rough Watercolor", 0.6, 0.95, r * 0.12));
    canvas.add_path(eye.accent, eye_style(&color, "Ink Pen", 0.2, 0.6, 1.2));
    canvas.add_path(eye.lines, ink);
    Ok(Canvas::new(canvas))
}

/// `art::moon(center, radius, phase)`: #{ outline, terminator, lit, dark }.
fn moon(ctx: &NativeCallContext, center: Vec2, radius: &Dynamic, phase: &Dynamic) -> RhaiResult<Map> {
    const CALLER: &str = "art::moon";
    let n = |what: &str, v: &Dynamic| number_arg(ctx, CALLER, what, v);
    let moon = art::moon(center.into(), n("radius", radius)?, n("phase", phase)?)
        .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    let mut parts = Map::new();
    for (name, path) in [("outline", moon.outline), ("terminator", moon.terminator), ("lit", moon.lit), ("dark", moon.dark)] {
        parts.insert(name.into(), Dynamic::from(Path::new(path)));
    }
    Ok(parts)
}

pub fn register(engine: &mut Engine) {
    let mut module = Module::new();
    module.set_native_fn("moon", |ctx: NativeCallContext, center: Vec2, radius: Dynamic, phase: Dynamic| -> RhaiResult<Map> {
        moon(&ctx, center, &radius, &phase)
    });
    module.set_native_fn("snake", |ctx: NativeCallContext, head_center: Vec2, tail_end: Vec2, length: Dynamic| -> RhaiResult<Map> {
        snake(&ctx, head_center, tail_end, &length, &Map::new())
    });
    module.set_native_fn("snake", |ctx: NativeCallContext, head_center: Vec2, tail_end: Vec2, length: Dynamic, options: Map| -> RhaiResult<Map> {
        snake(&ctx, head_center, tail_end, &length, &options)
    });
    module.set_native_fn("bramble", |ctx: NativeCallContext, base: Vec2, length: Dynamic| -> RhaiResult<Map> {
        bramble(&ctx, base, &length, &Map::new())
    });
    module.set_native_fn("bramble", |ctx: NativeCallContext, base: Vec2, length: Dynamic, options: Map| -> RhaiResult<Map> {
        bramble(&ctx, base, &length, &options)
    });
    module.set_native_fn("bramble_along", |ctx: NativeCallContext, path: Path, flower_size: Dynamic| -> RhaiResult<Map> {
        bramble_along(&ctx, &path, &flower_size, &Map::new())
    });
    module.set_native_fn("bramble_along", |ctx: NativeCallContext, path: Path, flower_size: Dynamic, options: Map| -> RhaiResult<Map> {
        bramble_along(&ctx, &path, &flower_size, &options)
    });
    let snake_along = |ctx: &NativeCallContext, path: &Path, max_width: &Dynamic, options: &Map| -> RhaiResult<Path> {
        const CALLER: &str = "art::snake_along";
        check_keys(CALLER, options, &["seed"]).map_err(|err| error(ctx, err.to_string()))?;
        let max_width = number_arg(ctx, CALLER, "max_width", max_width)?;
        art::snake_along(&path.borrow(), max_width, seed(ctx, CALLER, options)?)
            .map(Path::new)
            .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))
    };
    module.set_native_fn("snake_along", move |ctx: NativeCallContext, path: Path, max_width: Dynamic| -> RhaiResult<Path> {
        snake_along(&ctx, &path, &max_width, &Map::new())
    });
    module.set_native_fn("snake_along", move |ctx: NativeCallContext, path: Path, max_width: Dynamic, options: Map| -> RhaiResult<Path> {
        snake_along(&ctx, &path, &max_width, &options)
    });
    module.set_native_fn("eye", |ctx: NativeCallContext, center: Vec2, length: Dynamic| -> RhaiResult<Canvas> {
        eye(&ctx, center, &length, &Map::new())
    });
    module.set_native_fn("eye", |ctx: NativeCallContext, center: Vec2, length: Dynamic, options: Map| -> RhaiResult<Canvas> {
        eye(&ctx, center, &length, &options)
    });
    engine.register_static_module("art", module.into());
}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use crate::journal::fonts::{FontBook, DEFAULT_FONTS};
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::new(Rc::new(FontBook::from_file(DEFAULT_FONTS).expect("Failed to load fonts.json")))
    }

    #[test]
    fn snake_path_draws_a_snake_from_its_seed() {
        let script = r##"
            let draw = |seed| art::snake(vec2(0.0, 0.0), vec2(20.0, 300.0), 900.0, #{ head_angle: -PI() / 2.0, max_width: 30.0, seed: seed }).outline;
            let snake = draw.call(7);
            if snake.to_string() != draw.call(7).to_string() { throw "the same seed should draw the same snake"; }
            if snake.to_string() == draw.call(8).to_string() { throw "another seed should draw another snake"; }
            let spine = art::snake(vec2(0.0, 0.0), vec2(20.0, 300.0), 900.0, #{ head_angle: -PI() / 2.0, max_width: 30.0, seed: 7 }).spine;
            if spine.to_string() == snake.to_string() { throw "the spine should be another path"; }
            let c = new_canvas();
            c.add(snake, new_style("#f2ead8", new_brush("Ink Pen", 0.3, 0.7, 1.0)));
            // The tail's tip reaches the tail end (the outline's bounds include
            // the stroke's width).
            if c.bounds().max.y < 300.0 { throw "the tail should reach the tail end: " + c.bounds(); }
            // The snake has its own generator: the script's isn't moved on.
            seed(3);
            let a = random();
            seed(3);
            draw.call(9);
            if random() != a { throw "snake_path shouldn't draw from the script's generator"; }
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "##;
        engine().build_entry_str(script).expect("script should run");
    }

    #[test]
    fn bramble_gives_its_three_parts() {
        let script = r##"
            let plant = art::bramble(vec2(0.0, 0.0), 1000.0, #{ angle: -PI() / 2.0, flower_size: 150.0, seed: 5 });
            let pen = new_brush("Ink Pen", 0.3, 0.7, 1.0);
            let c = new_canvas();
            c.add(plant.vines, new_style("#c8a27a", pen));
            c.add(plant.leaves, new_style("#9fd17a", pen));
            c.add(plant.flowers, new_style("#f2ead8", pen));
            if plant.keys().len() != 3 { throw "expected vines, leaves and flowers: " + plant.keys(); }
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "##;
        engine().build_entry_str(script).expect("script should run");
        let err = engine().build_entry_str("let ok = 1;\nart::bramble(vec2(0.0, 0.0), 100.0, #{ angle: 0.0, flower_size: 150.0, seed: 1 });")
            .expect_err("too short a cane should fail").to_string();
        assert!(err.contains("art::bramble: length must be at least") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn bramble_along_follows_a_path() {
        let script = r##"
            let path = new_maze(3, 2, #{ seed: 2 }).path().scale_by(70.0);
            let plant = art::bramble_along(path, 40.0, #{ seed: 1 });
            let pen = new_brush("Ink Pen", 0.3, 0.7, 1.0);
            let c = new_canvas();
            c.add(plant.vines, new_style("#c8a27a", pen));
            c.add(plant.leaves, new_style("#a8d68a", pen));
            c.add(plant.flowers, new_style("#f2ead8", pen));
            // The vines span the maze: 2 nodes across, 140 apart.
            let b = c.bounds();
            if b.width < 140.0 { throw "the vines should cross the maze: " + b; }
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "##;
        engine().build_entry_str(script).expect("script should run");
        let err = engine().build_entry_str("let ok = 1;\nart::bramble_along(new_path(), 40.0, #{ seed: 1 });")
            .expect_err("an empty path should fail").to_string();
        assert!(err.contains("art::bramble_along: the path has no lines to follow") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn brambles_thicken_toward_their_origin() {
        let ok: bool = engine().engine.eval(r#"
            let path = new_maze(3, 2, #{ seed: 2 }).path().scale_by(70.0);
            let plain = art::bramble_along(path, 40.0, #{ seed: 1 }).vines;
            let thick = art::bramble_along(path, 40.0, #{ seed: 1, origin: vec2(0, 0), thickness: 4, reach: 120 }).vines;
            let free = art::bramble(vec2(0, 0), 400.0, #{ angle: -PI() / 2.0, flower_size: 80.0, seed: 2, origin: vec2(0, 0) }).vines;
            `${plain}` != `${thick}` && `${free}` != `${art::bramble(vec2(0, 0), 400.0, #{ angle: -PI() / 2.0, flower_size: 80.0, seed: 2 }).vines}`
        "#).unwrap();
        assert!(ok);
        for (script, want) in [
            ("art::bramble_along(new_maze(3, 2, #{ seed: 2 }).path(), 4.0, #{ seed: 1, thickness: 2 })", "art::bramble_along: 'thickness' and 'reach' need an 'origin'"),
            ("art::bramble(vec2(0, 0), 400.0, #{ angle: 0.0, flower_size: 80.0, seed: 1, origin: vec2(0, 0), thickness: 0.5 })", "thickness must be a number of at least 1"),
            ("art::bramble(vec2(0, 0), 400.0, #{ angle: 0.0, flower_size: 80.0, seed: 1, origin: vec2(0, 0), reach: -1 })", "reach must be a positive number"),
            ("art::bramble(vec2(0, 0), 400.0, #{ angle: 0.0, flower_size: 80.0, seed: 1, origin: 3 })", "'origin' should be a vec2"),
            ("art::bramble(vec2(0, 0), 400.0, #{ angle: 0.0, flower_size: 80.0, seed: 1, origin: vec2(0, 0), girth: 2 })", "unknown option 'girth'"),
        ] {
            let err = engine().engine.eval::<rhai::Dynamic>(script).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn snake_along_follows_a_path() {
        let script = r##"
            let path = new_maze(3, 2, #{ seed: 2 }).path().scale_by(70.0);
            let c = new_canvas();
            c.add(art::snake_along(path, 30.0, #{ seed: 1 }), new_style("#f2ead8", new_brush("Ink Pen", 0.3, 0.7, 1.0)));
            if c.bounds().width < 140.0 { throw "the serpent should cross the maze: " + c.bounds(); }
            let p = new_page();
            p.add(c);
            journal_entry.add(p);
        "##;
        engine().build_entry_str(script).expect("script should run");
        let err = engine().build_entry_str("let ok = 1;\nart::snake_along(new_line(vec2(0.0, 0.0), vec2(100.0, 0.0)), 0.0, #{ seed: 1 });")
            .expect_err("a zero width should fail").to_string();
        assert!(err.contains("art::snake_along: max_width must be a positive number") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn eye_draws_an_eye_from_its_seed() {
        let script = r##"
            let draw = |seed| art::eye(vec2(100.0, 100.0), 200.0, #{ color: "#2a9bb8", gaze_angle: 0.5, messiness: 0.6, seed: seed });
            let e = draw.call(7);
            if e.bounds().to_string() != draw.call(7).bounds().to_string() { throw "the same seed should draw the same eye"; }
            if e.bounds().to_string() == draw.call(8).bounds().to_string() { throw "another seed should draw another eye"; }
            // About as long as asked, round its centre.
            let b = e.bounds();
            if b.width < 190.0 || b.width > 320.0 || (b.center - vec2(100.0, 100.0)).length() > 40.0 { throw "the eye should be where asked: " + b; }
            let tilted = art::eye(vec2(0.0, 0.0), 200.0, #{ color: RED_INK, gaze_angle: PI(), messiness: 0.0, seed: 1, angle: PI() / 2.0, gaze: 1.0, openness: 0.3, ink: new_style("#222", new_brush("Pencil", 0.3, 0.7, 1.0)) });
            if tilted.bounds().height < 190.0 { throw "turned upright, the eye should be tall: " + tilted.bounds(); }
            art::eye(vec2(0.0, 0.0), 200.0, #{ color: "#123", gaze_angle: 0.0, messiness: 0.5, seed: 1, flat: true });
            let p = new_page();
            p.add(e);
            journal_entry.add(p);
        "##.replace("RED_INK", "\"#9b2318\"");
        engine().build_entry_str(&script).expect("script should run");
        for (call, message) in [
            ("art::eye(vec2(0.0, 0.0), 200.0, #{ color: \"#123\", gaze_angle: 0.0, messiness: 1.5, seed: 1 });", "art::eye: messiness runs from 0 to 1"),
            ("art::eye(vec2(0.0, 0.0), 0.0, #{ color: \"#123\", gaze_angle: 0.0, messiness: 0.5, seed: 1 });", "art::eye: length must be a positive number"),
            ("art::eye(vec2(0.0, 0.0), 200.0, #{ color: \"#123\", gaze_angle: 0.0, messiness: 0.5, seed: 1, squint: 1 });", "art::eye: unknown option 'squint'"),
            ("art::eye(vec2(0.0, 0.0), 200.0, #{ color: \"#123\", gaze_angle: 0.0, messiness: 0.5, seed: 1, ink: 3 });", "art::eye: 'ink' should be a style"),
            ("art::eye(vec2(0.0, 0.0), 200.0, #{ color: \"#123\", gaze_angle: 0.0, messiness: 0.5, seed: 1, flat: 1 });", "art::eye: 'flat' should be true or false"),
        ] {
            let err = engine().build_entry_str(&format!("let ok = 1;\n{}", call)).expect_err(call).to_string();
            assert!(err.contains(message) && err.contains("line 2"), "{}", err);
        }
    }

    #[test]
    fn moon_gives_its_parts_by_phase() {
        let ok: rhai::Array = engine().engine.eval(r#"
            let empty = |p| `${p}` == "";
            let quarter = art::moon(vec2(0, 0), 10, 0.25);
            let b = quarter.lit.bounds();
            let wrapped = art::moon(vec2(0, 0), 10, 1.25);
            let dim = art::moon(vec2(0, 0), 10, 0);
            let full = art::moon(vec2(0, 0), 10, 0.5);
            [quarter.keys().len() == 4, b.min.x > -0.01 && b.max.x > 9.9,
                `${wrapped.lit}` == `${quarter.lit}`,
                empty.call(dim.lit) && empty.call(dim.terminator) && !empty.call(dim.dark),
                empty.call(full.dark) && !empty.call(full.lit),
                art::moon(vec2(0, 0), 10, 0.75).lit.bounds().max.x < 0.01]
        "#).unwrap();
        assert!(ok.iter().all(|b| b.as_bool().unwrap()), "{:?}", ok);
        let err = engine().build_entry_str("let ok = 1;\nart::moon(vec2(0.0, 0.0), 0.0, 0.5);")
            .expect_err("a zero radius should fail").to_string();
        assert!(err.contains("art::moon: radius must be a positive number") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn snake_path_errors_name_the_function_and_line() {
        let err = engine().build_entry_str("let ok = 1;\nart::snake(vec2(0.0, 0.0), vec2(5000.0, 0.0), 900.0, #{ head_angle: 0.0, max_width: 30.0, seed: 1 });")
            .expect_err("a tail end out of reach should fail").to_string();
        assert!(err.contains("art::snake: the tail end is") && err.contains("line 2"), "{}", err);
    }
}
