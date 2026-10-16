//! Procedural art for rhai scripts, in the `art` module.
//!
//! ```rhai
//! // A snake seen from above: its head centred at (0, 0) facing up the page
//! // (angles are radians; 0 faces +x and PI() / 2.0 faces down the page),
//! // the tip of its tail at (40, 400), 900 units long along its spine and 36
//! // across at its widest. The last argument is the seed for its winding.
//! let snake = art::snake_path(vec2(0.0, 0.0), -PI() / 2.0, vec2(40.0, 400.0), 900.0, 36.0, 7);
//! canvas.add_path(snake, ink);
//! ```
//!
//! `snake_path` gives the snake's outline. Where the body crosses itself,
//! the outline of the strand underneath stops at the edge of the one on top
//! (the head always goes over), so the path is several strokes. The same
//! seed gives the same snake; it doesn't use or change the script's
//! generator (`seed`, `random`).
//!
//! `snake_spine`, with the same arguments, gives the same snake's centre
//! line from the back of its head to the tip of its tail, to write along:
//!
//! ```rhai
//! let back = art::snake_spine(vec2(0.0, 0.0), -PI() / 2.0, vec2(40.0, 400.0), 900.0, 36.0, 7);
//! canvas.add_canvas(back.fill_with_text("a snake", font, 30.0, ink)[0]);
//! ```
//!
//! `bramble` grows a blackberry bramble and gives its parts as separate
//! paths, to stroke in different colours: a cane from (0, 0) growing up the
//! page, 1400 long, with flowers about 200 across (which everything else is
//! sized by).
//!
//! ```rhai
//! let plant = art::bramble(vec2(0.0, 0.0), -PI() / 2.0, 1400.0, 200.0, 7);
//! canvas.add_path(plant.vines, bark);     // canes, thorns and stalks
//! canvas.add_path(plant.leaves, green);   // leaflets and their veins
//! canvas.add_path(plant.flowers, white);  // flowers, buds and sepals
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
//! let path = new_maze(7, 9, 7).path().scale_by(70.0);
//! let plant = art::bramble_along(path, 38.0, 3);   // flowers 38 across
//! ```
//!
//! Either takes `#{ origin, thickness, reach }` to grow canes thicker
//! toward a growth origin, as old wood is: `thickness` times as wide there
//! (default 3), easing back to their usual width `reach` away (default
//! as far as the canes go).
//!
//! ```rhai
//! let plant = art::bramble_along(path, 38.0, 3, #{ origin: vec2(0, 0), thickness: 4 });
//! ```
//!
//! `snake_along` does the same for a snake, making a many-headed serpent:
//! its body follows the path, slithering a little off it and forking where
//! it forks, with its tail at the dead end nearest where the path starts
//! and a head at every other dead end.
//!
//! ```rhai
//! canvas.add_path(art::snake_along(path, 30.0, 2), ink);   // 30 wide
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
//! canvas.add_canvas(art::eye(vec2(0.0, 0.0), 300.0, "#2a9bb8", -PI() / 4.0, 0.5, 7));
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
//! let e = art::eye(vec2(0.0, 0.0), 300.0, "#2a9bb8", PI(), 0.8, 7, #{ angle: 1.2, gaze: 1.0, openness: 0.6, flat: true, ink: ink });
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

const BRAMBLE_KEYS: [&str; 3] = ["origin", "thickness", "reach"];

/// A bramble's `#{ origin, thickness, reach }`: canes thicker toward a
/// growth origin.
fn thickening(ctx: &NativeCallContext, caller: &str, options: Option<&Map>) -> RhaiResult<Option<art::Thickening>> {
    let Some(options) = options else { return Ok(None) };
    check_keys(caller, options, &BRAMBLE_KEYS).map_err(|err| error(ctx, err.to_string()))?;
    let origin = options.get("origin")
        .ok_or_else(|| error(ctx, format!("{}: missing 'origin', the point canes thicken toward", caller)))?
        .clone().try_cast::<Vec2>()
        .ok_or_else(|| error(ctx, format!("{}: 'origin' should be a vec2", caller)))?;
    let thickness = number(caller, options, "thickness").map_err(|err| error(ctx, err.to_string()))?.unwrap_or(3.0);
    let reach = number(caller, options, "reach").map_err(|err| error(ctx, err.to_string()))?;
    Ok(Some(art::Thickening { origin: origin.into(), thickness, reach }))
}

fn bramble(ctx: &NativeCallContext, base: Vec2, angle: &Dynamic, length: &Dynamic, flower_size: &Dynamic, seed: i64, options: Option<&Map>) -> RhaiResult<Map> {
    const CALLER: &str = "art::bramble";
    let n = |what: &str, v: &Dynamic| number_arg(ctx, CALLER, what, v);
    let thickening = thickening(ctx, CALLER, options)?;
    let bramble = art::bramble(base.into(), n("angle", angle)?, n("length", length)?, n("flower_size", flower_size)?, seed as u64, thickening)
        .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    Ok(parts(bramble))
}

fn bramble_along(ctx: &NativeCallContext, path: &Path, flower_size: &Dynamic, seed: i64, options: Option<&Map>) -> RhaiResult<Map> {
    const CALLER: &str = "art::bramble_along";
    let flower_size = number_arg(ctx, CALLER, "flower_size", flower_size)?;
    let thickening = thickening(ctx, CALLER, options)?;
    let bramble = art::bramble_along(&path.borrow(), flower_size, seed as u64, thickening)
        .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    Ok(parts(bramble))
}

/// A bramble's parts, by name.
fn parts(bramble: art::Bramble) -> Map {
    let mut parts = Map::new();
    for (name, path) in [("vines", bramble.vines), ("leaves", bramble.leaves), ("flowers", bramble.flowers)] {
        parts.insert(name.into(), Dynamic::from(Path::new(path)));
    }
    parts
}

const EYE_KEYS: [&str; 5] = ["angle", "gaze", "openness", "flat", "ink"];

/// A style for an eye's strokes: `brush` at `size`, in `color`.
/// Tapered at both ends, as quick strokes are.
fn eye_style(color: &crate::journal::canvas::StrokeColor, brush: &str, min: f64, max: f64, size: f32) -> StrokeStyle {
    let mut brush = BrushSettings::new_brush(brush, min, max, size as f64);
    brush.pressure_profile = crate::journal::canvas::PressureProfile::Taper;
    StrokeStyle { width: 1.0, color: color.clone(), brush, passes: 1, jitter: 0.0 }
}

#[allow(clippy::too_many_arguments)]
fn eye(ctx: &NativeCallContext, center: Vec2, length: &Dynamic, color: Dynamic, gaze_angle: &Dynamic, messiness: &Dynamic, seed: i64, options: &Map) -> RhaiResult<Canvas> {
    const CALLER: &str = "art::eye";
    let fail = |err: Box<EvalAltResult>| error(ctx, err.to_string());
    check_keys(CALLER, options, &EYE_KEYS).map_err(fail)?;
    let color = colour_or_gradient(CALLER, color).map_err(fail)?;
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
    let n = |what: &str, v: &Dynamic| number_arg(ctx, CALLER, what, v);
    let eye = art::eye(center.into(), n("length", length)?, n("gaze_angle", gaze_angle)?, n("messiness", messiness)?, shape, seed as u64)
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

pub fn register(engine: &mut Engine) {
    let mut module = Module::new();
    module.set_native_fn("snake_path", |ctx: NativeCallContext, head_center: Vec2, head_angle: Dynamic, tail_end: Vec2, snake_length: Dynamic, max_width: Dynamic, random_seed: i64| -> RhaiResult<Path> {
        let n = |what: &str, v: &Dynamic| number_arg(&ctx, "art::snake_path", what, v);
        art::snake_path(head_center.into(), n("head_angle", &head_angle)?, tail_end.into(), n("snake_length", &snake_length)?, n("max_width", &max_width)?, random_seed as u64)
            .map(Path::new)
            .map_err(|err| error(&ctx, format!("art::snake_path: {}", err)))
    });
    module.set_native_fn("snake_spine", |ctx: NativeCallContext, head_center: Vec2, head_angle: Dynamic, tail_end: Vec2, snake_length: Dynamic, max_width: Dynamic, random_seed: i64| -> RhaiResult<Path> {
        let n = |what: &str, v: &Dynamic| number_arg(&ctx, "art::snake_spine", what, v);
        art::snake_spine(head_center.into(), n("head_angle", &head_angle)?, tail_end.into(), n("snake_length", &snake_length)?, n("max_width", &max_width)?, random_seed as u64)
            .map(Path::new)
            .map_err(|err| error(&ctx, format!("art::snake_spine: {}", err)))
    });
    module.set_native_fn("bramble", |ctx: NativeCallContext, base: Vec2, angle: Dynamic, length: Dynamic, flower_size: Dynamic, random_seed: i64| -> RhaiResult<Map> {
        bramble(&ctx, base, &angle, &length, &flower_size, random_seed, None)
    });
    module.set_native_fn("bramble", |ctx: NativeCallContext, base: Vec2, angle: Dynamic, length: Dynamic, flower_size: Dynamic, random_seed: i64, options: Map| -> RhaiResult<Map> {
        bramble(&ctx, base, &angle, &length, &flower_size, random_seed, Some(&options))
    });
    module.set_native_fn("bramble_along", |ctx: NativeCallContext, path: Path, flower_size: Dynamic, random_seed: i64| -> RhaiResult<Map> {
        bramble_along(&ctx, &path, &flower_size, random_seed, None)
    });
    module.set_native_fn("bramble_along", |ctx: NativeCallContext, path: Path, flower_size: Dynamic, random_seed: i64, options: Map| -> RhaiResult<Map> {
        bramble_along(&ctx, &path, &flower_size, random_seed, Some(&options))
    });
    module.set_native_fn("snake_along", |ctx: NativeCallContext, path: Path, max_width: Dynamic, random_seed: i64| -> RhaiResult<Path> {
        let max_width = number_arg(&ctx, "art::snake_along", "max_width", &max_width)?;
        art::snake_along(&path.borrow(), max_width, random_seed as u64)
            .map(Path::new)
            .map_err(|err| error(&ctx, format!("art::snake_along: {}", err)))
    });
    module.set_native_fn("eye", |ctx: NativeCallContext, center: Vec2, length: Dynamic, color: Dynamic, gaze_angle: Dynamic, messiness: Dynamic, random_seed: i64| -> RhaiResult<Canvas> {
        eye(&ctx, center, &length, color, &gaze_angle, &messiness, random_seed, &Map::new())
    });
    module.set_native_fn("eye", |ctx: NativeCallContext, center: Vec2, length: Dynamic, color: Dynamic, gaze_angle: Dynamic, messiness: Dynamic, random_seed: i64, options: Map| -> RhaiResult<Canvas> {
        eye(&ctx, center, &length, color, &gaze_angle, &messiness, random_seed, &options)
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
            let draw = |seed| art::snake_path(vec2(0.0, 0.0), -PI() / 2.0, vec2(20.0, 300.0), 900.0, 30.0, seed);
            let snake = draw.call(7);
            if snake.to_string() != draw.call(7).to_string() { throw "the same seed should draw the same snake"; }
            if snake.to_string() == draw.call(8).to_string() { throw "another seed should draw another snake"; }
            let spine = art::snake_spine(vec2(0.0, 0.0), -PI() / 2.0, vec2(20.0, 300.0), 900.0, 30.0, 7);
            if spine.to_string() == snake.to_string() { throw "the spine should be another path"; }
            let c = new_canvas();
            c.add_path(snake, new_style(1.0, "#f2ead8", new_brush("Ink Pen", 0.3, 0.7, 1.0)));
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
            p.add_canvas(c);
            journal_entry.add_page(p);
        "##;
        engine().build_entry_str(script).expect("script should run");
    }

    #[test]
    fn bramble_gives_its_three_parts() {
        let script = r##"
            let plant = art::bramble(vec2(0.0, 0.0), -PI() / 2.0, 1000.0, 150.0, 5);
            let pen = new_brush("Ink Pen", 0.3, 0.7, 1.0);
            let c = new_canvas();
            c.add_path(plant.vines, new_style(1.0, "#c8a27a", pen));
            c.add_path(plant.leaves, new_style(1.0, "#9fd17a", pen));
            c.add_path(plant.flowers, new_style(1.0, "#f2ead8", pen));
            if plant.keys().len() != 3 { throw "expected vines, leaves and flowers: " + plant.keys(); }
            let p = new_page();
            p.add_canvas(c);
            journal_entry.add_page(p);
        "##;
        engine().build_entry_str(script).expect("script should run");
        let err = engine().build_entry_str("let ok = 1;\nart::bramble(vec2(0.0, 0.0), 0.0, 100.0, 150.0, 1);")
            .expect_err("too short a cane should fail").to_string();
        assert!(err.contains("art::bramble: length must be at least") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn bramble_along_follows_a_path() {
        let script = r##"
            let path = new_maze(2, 3, 2).path().scale_by(70.0);
            let plant = art::bramble_along(path, 40.0, 1);
            let pen = new_brush("Ink Pen", 0.3, 0.7, 1.0);
            let c = new_canvas();
            c.add_path(plant.vines, new_style(1.0, "#c8a27a", pen));
            c.add_path(plant.leaves, new_style(1.0, "#a8d68a", pen));
            c.add_path(plant.flowers, new_style(1.0, "#f2ead8", pen));
            // The vines span the maze: 2 nodes across, 140 apart.
            let b = c.bounds();
            if b.width < 140.0 { throw "the vines should cross the maze: " + b; }
            let p = new_page();
            p.add_canvas(c);
            journal_entry.add_page(p);
        "##;
        engine().build_entry_str(script).expect("script should run");
        let err = engine().build_entry_str("let ok = 1;\nart::bramble_along(new_path(), 40.0, 1);")
            .expect_err("an empty path should fail").to_string();
        assert!(err.contains("art::bramble_along: the path has no lines to follow") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn brambles_thicken_toward_their_origin() {
        let ok: bool = engine().engine.eval(r#"
            let path = new_maze(2, 3, 2).path().scale_by(70.0);
            let plain = art::bramble_along(path, 40.0, 1).vines;
            let thick = art::bramble_along(path, 40.0, 1, #{ origin: vec2(0, 0), thickness: 4, reach: 120 }).vines;
            let free = art::bramble(vec2(0, 0), -PI() / 2.0, 400.0, 80.0, 2, #{ origin: vec2(0, 0) }).vines;
            `${plain}` != `${thick}` && `${free}` != `${art::bramble(vec2(0, 0), -PI() / 2.0, 400.0, 80.0, 2).vines}`
        "#).unwrap();
        assert!(ok);
        for (script, want) in [
            ("art::bramble_along(new_maze(2, 3, 2).path(), 4.0, 1, #{ thickness: 2 })", "art::bramble_along: missing 'origin'"),
            ("art::bramble(vec2(0, 0), 0.0, 400.0, 80.0, 1, #{ origin: vec2(0, 0), thickness: 0.5 })", "thickness must be a number of at least 1"),
            ("art::bramble(vec2(0, 0), 0.0, 400.0, 80.0, 1, #{ origin: vec2(0, 0), reach: -1 })", "reach must be a positive number"),
            ("art::bramble(vec2(0, 0), 0.0, 400.0, 80.0, 1, #{ origin: 3 })", "'origin' should be a vec2"),
            ("art::bramble(vec2(0, 0), 0.0, 400.0, 80.0, 1, #{ origin: vec2(0, 0), girth: 2 })", "unknown option 'girth'"),
        ] {
            let err = engine().engine.eval::<rhai::Dynamic>(script).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn snake_along_follows_a_path() {
        let script = r##"
            let path = new_maze(2, 3, 2).path().scale_by(70.0);
            let c = new_canvas();
            c.add_path(art::snake_along(path, 30.0, 1), new_style(1.0, "#f2ead8", new_brush("Ink Pen", 0.3, 0.7, 1.0)));
            if c.bounds().width < 140.0 { throw "the serpent should cross the maze: " + c.bounds(); }
            let p = new_page();
            p.add_canvas(c);
            journal_entry.add_page(p);
        "##;
        engine().build_entry_str(script).expect("script should run");
        let err = engine().build_entry_str("let ok = 1;\nart::snake_along(new_line(vec2(0.0, 0.0), vec2(100.0, 0.0)), 0.0, 1);")
            .expect_err("a zero width should fail").to_string();
        assert!(err.contains("art::snake_along: max_width must be a positive number") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn eye_draws_an_eye_from_its_seed() {
        let script = r##"
            let draw = |seed| art::eye(vec2(100.0, 100.0), 200.0, "#2a9bb8", 0.5, 0.6, seed);
            let e = draw.call(7);
            if e.bounds().to_string() != draw.call(7).bounds().to_string() { throw "the same seed should draw the same eye"; }
            if e.bounds().to_string() == draw.call(8).bounds().to_string() { throw "another seed should draw another eye"; }
            // About as long as asked, round its centre.
            let b = e.bounds();
            if b.width < 190.0 || b.width > 320.0 || (b.center - vec2(100.0, 100.0)).length() > 40.0 { throw "the eye should be where asked: " + b; }
            let tilted = art::eye(vec2(0.0, 0.0), 200.0, RED_INK, PI(), 0.0, 1, #{ angle: PI() / 2.0, gaze: 1.0, openness: 0.3, ink: new_style(1.0, "#222", new_brush("Pencil", 0.3, 0.7, 1.0)) });
            if tilted.bounds().height < 190.0 { throw "turned upright, the eye should be tall: " + tilted.bounds(); }
            art::eye(vec2(0.0, 0.0), 200.0, "#123", 0.0, 0.5, 1, #{ flat: true });
            let p = new_page();
            p.add_canvas(e);
            journal_entry.add_page(p);
        "##.replace("RED_INK", "\"#9b2318\"");
        engine().build_entry_str(&script).expect("script should run");
        for (call, message) in [
            ("art::eye(vec2(0.0, 0.0), 200.0, \"#123\", 0.0, 1.5, 1);", "art::eye: messiness runs from 0 to 1"),
            ("art::eye(vec2(0.0, 0.0), 0.0, \"#123\", 0.0, 0.5, 1);", "art::eye: length must be a positive number"),
            ("art::eye(vec2(0.0, 0.0), 200.0, \"#123\", 0.0, 0.5, 1, #{ squint: 1 });", "art::eye: unknown option 'squint'"),
            ("art::eye(vec2(0.0, 0.0), 200.0, \"#123\", 0.0, 0.5, 1, #{ ink: 3 });", "art::eye: 'ink' should be a style"),
            ("art::eye(vec2(0.0, 0.0), 200.0, \"#123\", 0.0, 0.5, 1, #{ flat: 1 });", "art::eye: 'flat' should be true or false"),
        ] {
            let err = engine().build_entry_str(&format!("let ok = 1;\n{}", call)).expect_err(call).to_string();
            assert!(err.contains(message) && err.contains("line 2"), "{}", err);
        }
    }

    #[test]
    fn snake_path_errors_name_the_function_and_line() {
        let err = engine().build_entry_str("let ok = 1;\nart::snake_path(vec2(0.0, 0.0), 0.0, vec2(5000.0, 0.0), 900.0, 30.0, 1);")
            .expect_err("a tail end out of reach should fail").to_string();
        assert!(err.contains("art::snake_path: the tail end is") && err.contains("line 2"), "{}", err);
    }
}
