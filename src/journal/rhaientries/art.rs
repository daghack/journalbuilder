//! Procedural art for rhai scripts, in the `art` module.
//!
//! ```rhai
//! // A snake seen from above: its head centred at (0, 0) facing up the page
//! // (angles are radians; 0 faces +x and PI() / 2.0 faces down the page;
//! // by default it faces away from the tail), the tip of its tail at
//! // (40, 400), 900 units long along its spine and 36 across at its widest
//! // (by default a twelfth of its length). The seed picks its winding (one
//! // from the journal's generator unless given).
//! let snake = art::snake(vec2(0.0, 0.0), vec2(40.0, 400.0), 900.0, #{ head_angle: -PI() / 2.0, max_width: 36.0, seed: 7 });
//! canvas.add(snake.outline, ink);
//! ```
//!
//! `outline` is the snake's outline. Where the body crosses itself,
//! the outline of the strand underneath stops at the edge of the one on top
//! (the head always goes over), so the path is several strokes. The same
//! seed gives the same snake; given one, it doesn't use or change the
//! script's generator (`seed`, `random`), and left out, it's drawn from it.
//!
//! `spine` is the same snake's centre line from the back of its head to
//! the tip of its tail, to write along:
//!
//! ```rhai
//! canvas.add(snake.spine.fill_with_text("a snake", #{ style: ink, type: new_type_style(font, 30.0) }));
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
//! `eye` draws an eye as a hand sketches one in a journal, and gives its
//! parts as paths to draw in different inks: `lines`, the lids gone over
//! and running on past the corners, the iris ringed and ringed again, and
//! a line down through the iris, bowed toward where the eye looks;
//! `accent`, a second outline of the lids to go round in the iris's
//! colour; `wash`, a spiral filling the iris; and `rim`, rings round its
//! edge where the wash pools. `iris_radius` is the iris's radius, to size
//! the wash's brushes by. Here it's 300 from corner to corner, looking up
//! and to the right (the gaze angle is in radians, on the page), half way
//! between neat (0) and as messy as it gets (1).
//!
//! ```rhai
//! let eye = art::eye(vec2(0.0, 0.0), 300.0, #{ gaze_angle: -PI() / 4.0, messiness: 0.5, seed: 7 });
//! let r = eye.iris_radius;
//! canvas.add(eye, #{
//!     wash: new_style(new_brush_style(BLUE, "Smooth Watercolor", r * 0.3, #{ min_pressure: 0.2, max_pressure: 0.35 }), new_handwriting_style()),
//!     rim: new_style(new_brush_style(BLUE, "Rough Watercolor", r * 0.12, #{ min_pressure: 0.6, max_pressure: 0.95 }), new_handwriting_style()),
//!     accent: new_style(BLUE, pen), lines: ink,
//! });
//! ```
//!
//! Options: `angle`, the eye's tilt (default 0, corners level); `gaze`,
//! how far it looks from 0 (ahead) to 1 (as far as it goes; default 0.5);
//! `openness`, its height over its length (default 0.32); and `flat`, lids
//! of straight lines meeting in a point above and below the middle, as
//! the `symbols` glyphset's eye has, rather than arcs (default false).
//!
//! ```rhai
//! let e = art::eye(vec2(0.0, 0.0), 300.0, #{ gaze_angle: PI(), messiness: 0.8, seed: 7, angle: 1.2, gaze: 1.0, openness: 0.6, flat: true });
//! ```
//!
//! `spiral_text` writes text in a spiral out from the middle of a circle,
//! clockwise from the top, as large as all of it fits: glyphs keep one
//! size until they'd come nearer the circle than the paragraph's
//! clearance, then each fills the room left between the circle and the
//! turn inside it, shrinking as that narrows, until they'd be smaller than
//! `minimum_scale` of the rest (default a third). It takes the usual text
//! settings, a type's glyph_scale capping the size; turns are the
//! paragraph's line height apart in the glyphs' own height, and the
//! spiral starts `inner` from the centre (default 0.85 of the glyph scale).
//!
//! ```rhai
//! canvas.add(art::spiral_text("lorem ipsum dolor sit amet", vec2(0, 0), 160.0, #{
//!     style: ink, type: #{ glyphset: glyphset("plain") }, minimum_scale: 0.1,
//! }));
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
//!
//! `huffman_stars` draws a star chart whose constellation is a Huffman
//! tree: the code the `huffman` lock would make from the symbols, rewriting
//! them in another base (here binary), its nodes stars and its branches
//! the links between them. Stars are bigger the more often their symbols
//! (or a branch's symbols together) come up, the root biggest. The tree is
//! grown out from the root at random angles and lengths, then tidied by
//! forces, each star with a space and a pull of its own and each link a
//! length of its own, into a scatter that looks random, filling the box
//! 900 by 600 about (0, 0), whose links never cross.
//!
//! ```rhai
//! let chart = art::huffman_stars(symbols(27, "THE QUICK BROWN FOX"), 2, vec2(0, 0), 900, 600, #{ seed: 3 });
//! canvas.add(chart, #{ stars: ink, links: ink });
//! for node in chart.nodes {
//!     if node.symbol != () { canvas.add(write_line(node.text, node.position + vec2(node.radius + 8, 0), label)); }
//! }
//! ```
//!
//! It gives `stars`, a circle for every star, and `links`, a line for
//! every link stopping short of its stars, as paths; `points` and
//! `edges`, the stars' centres and the links as [parent, child] index
//! pairs into them, as `new_edges` takes; and `nodes`, each star's
//! `#{ position, radius, weight, depth, code, symbol, text, parent }`.
//! Options: `smoothing` (as the lock's, default 1: every symbol of the
//! base is a star), `seed`, `variation` (how much stars and links differ,
//! 0 to 1, default 0.6), `iterations` (how long the forces tidy, default
//! 150), `star_size` (the biggest star's radius), `gap` (how far short of
//! stars links stop) and `round` (stars kept in the ellipse in the box).

use rand::Rng;
use rhai::{Dynamic, Engine, EvalAltResult, Map, Module, NativeCallContext};

use super::locks::LockValue;
use super::options::{check_keys, float, number};
use super::random::SharedRng;
use super::{Canvas, Path, Vec2};
use crate::lockbuilder::symbols_to_string;
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

/// A seed option: which drawing; unless given, one from the journal's
/// generator.
fn seed(ctx: &NativeCallContext, rng: &SharedRng, caller: &str, options: &Map) -> RhaiResult<u64> {
    match options.get("seed") {
        None => Ok(rng.borrow_mut().random()),
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
fn bramble(ctx: &NativeCallContext, rng: &SharedRng, base: Vec2, length: &Dynamic, options: &Map) -> RhaiResult<Map> {
    const CALLER: &str = "art::bramble";
    check_keys(CALLER, options, &["angle", "flower_size", "seed", "origin", "thickness", "reach"]).map_err(|err| error(ctx, err.to_string()))?;
    let length = number_arg(ctx, CALLER, "length", length)?;
    let angle = number_option(ctx, CALLER, options, "angle", -std::f32::consts::FRAC_PI_2)?;
    let flower_size = number_option(ctx, CALLER, options, "flower_size", length / 5.0)?;
    let bramble = art::bramble(base.into(), angle, length, flower_size, seed(ctx, rng, CALLER, options)?, thickening(ctx, CALLER, options)?)
        .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    Ok(parts(bramble))
}

/// `art::bramble_along(path, flower_size[, #{ seed, origin, thickness, reach }])`.
fn bramble_along(ctx: &NativeCallContext, rng: &SharedRng, path: &Path, flower_size: &Dynamic, options: &Map) -> RhaiResult<Map> {
    const CALLER: &str = "art::bramble_along";
    check_keys(CALLER, options, &["seed", "origin", "thickness", "reach"]).map_err(|err| error(ctx, err.to_string()))?;
    let flower_size = number_arg(ctx, CALLER, "flower_size", flower_size)?;
    let bramble = art::bramble_along(&path.borrow(), flower_size, seed(ctx, rng, CALLER, options)?, thickening(ctx, CALLER, options)?)
        .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    Ok(parts(bramble))
}

/// `art::snake(head_center, tail_end, length[, #{ head_angle, max_width, seed }])`:
/// `#{ outline, spine }`.
fn snake(ctx: &NativeCallContext, rng: &SharedRng, head: Vec2, tail: Vec2, length: &Dynamic, options: &Map) -> RhaiResult<Map> {
    const CALLER: &str = "art::snake";
    check_keys(CALLER, options, &["head_angle", "max_width", "seed"]).map_err(|err| error(ctx, err.to_string()))?;
    let length = number_arg(ctx, CALLER, "length", length)?;
    let (h, t): (parry2d::math::Vec2, parry2d::math::Vec2) = (head.into(), tail.into());
    let away = (h - t).y.atan2((h - t).x);
    let head_angle = number_option(ctx, CALLER, options, "head_angle", away)?;
    let max_width = number_option(ctx, CALLER, options, "max_width", length / 12.0)?;
    let seed = seed(ctx, rng, CALLER, options)?;
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

const EYE_KEYS: [&str; 7] = ["gaze_angle", "messiness", "seed", "angle", "gaze", "openness", "flat"];

/// `art::eye(center, length[, #{ gaze_angle, messiness, seed, angle, gaze, openness, flat }])`:
/// #{ lines, accent, wash, rim, iris_radius }.
fn eye(ctx: &NativeCallContext, rng: &SharedRng, center: Vec2, length: &Dynamic, options: &Map) -> RhaiResult<Map> {
    const CALLER: &str = "art::eye";
    let fail = |err: Box<EvalAltResult>| error(ctx, err.to_string());
    check_keys(CALLER, options, &EYE_KEYS).map_err(fail)?;
    let gaze_angle = number_option(ctx, CALLER, options, "gaze_angle", 0.0)?;
    let messiness = number_option(ctx, CALLER, options, "messiness", 0.3)?;
    let seed = seed(ctx, rng, CALLER, options)?;
    let defaults = art::EyeShape::default();
    let option = |key: &str, default: f32| number(CALLER, options, key).map(|n| n.unwrap_or(default)).map_err(fail);
    let flat = match options.get("flat") {
        None => defaults.flat,
        Some(flat) => flat.as_bool()
            .map_err(|type_name| error(ctx, format!("{}: 'flat' should be true or false, got {}", CALLER, type_name)))?,
    };
    let shape = art::EyeShape { angle: option("angle", defaults.angle)?, gaze: option("gaze", defaults.gaze)?, openness: option("openness", defaults.openness)?, flat };
    let eye = art::eye(center.into(), number_arg(ctx, CALLER, "length", length)?, gaze_angle, messiness, shape, seed)
        .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    let mut parts = Map::new();
    for (name, path) in [("lines", eye.lines), ("accent", eye.accent), ("wash", eye.wash), ("rim", eye.rim)] {
        parts.insert(name.into(), Dynamic::from(Path::new(path)));
    }
    parts.insert("iris_radius".into(), Dynamic::from(eye.iris_radius as f64));
    Ok(parts)
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

const HUFFMAN_STARS_KEYS: [&str; 7] = ["smoothing", "seed", "variation", "iterations", "star_size", "gap", "round"];

/// A whole-number option of at least 0, `default` unless given.
fn count_option(ctx: &NativeCallContext, caller: &str, options: &Map, key: &str, default: u64) -> RhaiResult<u64> {
    match options.get(key) {
        None => Ok(default),
        Some(v) => v.as_int().ok().and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| error(ctx, format!("{}: '{}' should be a whole number of at least 0, got {}", caller, key, v))),
    }
}

/// `art::huffman_stars(symbols, output_base, center, width, height[, #{ smoothing, seed, variation, iterations, star_size, gap, round }])`:
/// #{ stars, links, points, edges, nodes }.
fn huffman_stars(ctx: &NativeCallContext, rng: &SharedRng, symbols: &Dynamic, output_base: i64, center: Vec2, width: &Dynamic, height: &Dynamic, options: &Map) -> RhaiResult<Map> {
    const CALLER: &str = "art::huffman_stars";
    let fail = |err: Box<EvalAltResult>| error(ctx, err.to_string());
    check_keys(CALLER, options, &HUFFMAN_STARS_KEYS).map_err(fail)?;
    let value = symbols.clone().try_cast::<LockValue>()
        .ok_or_else(|| error(ctx, format!("{}: symbols should be symbols, as symbols(27, TEXT) gives, got {}", CALLER, symbols.type_name())))?;
    let (input_base, data) = value.base_and_symbols().map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    let output_base = u32::try_from(output_base).map_err(|_| error(ctx, format!("{}: output_base must be from 2 to 27, got {}", CALLER, output_base)))?;
    let optional = |key: &str| number(CALLER, options, key).map(|n| n.map(|n| n as f64)).map_err(fail);
    let round = match options.get("round") {
        None => false,
        Some(round) => round.as_bool()
            .map_err(|type_name| error(ctx, format!("{}: 'round' should be true or false, got {}", CALLER, type_name)))?,
    };
    let smoothing = u32::try_from(count_option(ctx, CALLER, options, "smoothing", 1)?)
        .map_err(|_| error(ctx, format!("{}: 'smoothing' is too big", CALLER)))?;
    let settings = art::StarChartSettings {
        center: center.into(),
        width: number_arg(ctx, CALLER, "width", width)? as f64,
        height: number_arg(ctx, CALLER, "height", height)? as f64,
        round,
        smoothing,
        variation: number_option(ctx, CALLER, options, "variation", 0.6)? as f64,
        iterations: count_option(ctx, CALLER, options, "iterations", 150)? as usize,
        star_size: optional("star_size")?,
        gap: optional("gap")?,
        seed: seed(ctx, rng, CALLER, options)?,
    };
    let chart = art::huffman_stars(&data, input_base, output_base, &settings).map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))?;
    let point = |p: parry2d::math::Vec2| Dynamic::from(Vec2::new(p.x as f64, p.y as f64));
    let mut nodes = rhai::Array::new();
    for star in &chart.stars {
        let mut node = Map::new();
        node.insert("position".into(), point(star.position));
        node.insert("radius".into(), Dynamic::from(star.radius));
        node.insert("weight".into(), Dynamic::from(star.weight as i64));
        node.insert("depth".into(), Dynamic::from(star.depth as i64));
        node.insert("code".into(), Dynamic::from(star.code.iter().map(|&d| Dynamic::from(d as i64)).collect::<rhai::Array>()));
        node.insert("symbol".into(), star.symbol.map_or(Dynamic::UNIT, |s| Dynamic::from(s as i64)));
        node.insert("text".into(), star.symbol.map_or(Dynamic::UNIT, |s| Dynamic::from(symbols_to_string(&[s], input_base))));
        node.insert("parent".into(), star.parent.map_or(Dynamic::UNIT, |p| Dynamic::from(p as i64)));
        nodes.push(Dynamic::from(node));
    }
    let mut parts = Map::new();
    parts.insert("stars".into(), Dynamic::from(Path::new(chart.outlines)));
    parts.insert("links".into(), Dynamic::from(Path::new(chart.links)));
    parts.insert("points".into(), Dynamic::from(chart.stars.iter().map(|s| point(s.position)).collect::<rhai::Array>()));
    parts.insert("edges".into(), Dynamic::from(chart.edges.iter()
        .map(|&(a, b)| Dynamic::from(vec![Dynamic::from(a as i64), Dynamic::from(b as i64)])).collect::<rhai::Array>()));
    parts.insert("nodes".into(), Dynamic::from(nodes));
    Ok(parts)
}

const CONSTELLATIONS_KEYS: [&str; 5] = ["label_width", "label_height", "label_gap", "split_at", "most_stars"];

/// `art::constellations(chart, count, center, width, height[, #{ label_width, label_height, label_gap, split_at, most_stars }])`:
/// an array of #{ stars, points, pairs, labels, scale }.
fn constellations(ctx: &NativeCallContext, chart: &Map, count: i64, center: Vec2, width: &Dynamic, height: &Dynamic, options: &Map) -> RhaiResult<rhai::Array> {
    const CALLER: &str = "art::constellations";
    let fail = |message: String| error(ctx, format!("{}: {}", CALLER, message));
    check_keys(CALLER, options, &CONSTELLATIONS_KEYS).map_err(|err| error(ctx, err.to_string()))?;
    let part = |key: &str| chart.get(key).and_then(|v| v.clone().try_cast::<rhai::Array>())
        .ok_or_else(|| fail(format!("the chart should have '{}', an array, as art::huffman_stars gives", key)));
    let points = part("points")?.into_iter()
        .map(|p| p.try_cast::<Vec2>().map(parry2d::math::Vec2::from).ok_or_else(|| fail("the chart's points should be vec2s".into())))
        .collect::<RhaiResult<Vec<_>>>()?;
    let index = |v: &Dynamic| v.as_int().ok().and_then(|i| usize::try_from(i).ok());
    let edges = part("edges")?.into_iter().map(|e| {
        let pair = e.try_cast::<rhai::Array>().filter(|p| p.len() == 2).and_then(|p| Some((index(&p[0])?, index(&p[1])?)));
        pair.ok_or_else(|| fail("the chart's edges should be [parent, child] pairs of indices".into()))
    }).collect::<RhaiResult<Vec<_>>>()?;
    let radii = part("nodes")?.into_iter().map(|node| {
        node.try_cast::<Map>().and_then(|m| m.get("radius").and_then(|r| r.as_float().ok()))
            .ok_or_else(|| fail("the chart's nodes should each have a radius".into()))
    }).collect::<RhaiResult<Vec<_>>>()?;
    let count = usize::try_from(count).map_err(|_| fail(format!("count must be at least 1, got {}", count)))?;
    let settings = art::ConstellationSettings {
        center: center.into(),
        width: number_arg(ctx, CALLER, "width", width)? as f64,
        height: number_arg(ctx, CALLER, "height", height)? as f64,
        label_width: number_option(ctx, CALLER, options, "label_width", 30.0)? as f64,
        label_height: number_option(ctx, CALLER, options, "label_height", 24.0)? as f64,
        label_gap: number_option(ctx, CALLER, options, "label_gap", 6.0)? as f64,
        split_at: match options.get("split_at") {
            None => Vec::new(),
            Some(stars) => stars.clone().try_cast::<rhai::Array>().and_then(|a| a.iter().map(index).collect::<Option<Vec<_>>>())
                .ok_or_else(|| fail("'split_at' should be an array of star indices".into()))?,
        },
        most_stars: match options.get("most_stars") {
            None => None,
            Some(most) => Some(index(most).ok_or_else(|| fail(format!("'most_stars' should be a whole number, got {}", most)))?),
        },
    };
    let pieces = art::constellations(&points, &radii, &edges, count, &settings).map_err(fail)?;
    let vecs = |ps: &[parry2d::math::Vec2]| ps.iter().map(|p| Dynamic::from(Vec2::new(p.x as f64, p.y as f64))).collect::<rhai::Array>();
    Ok(pieces.into_iter().map(|piece| {
        let mut m = Map::new();
        m.insert("stars".into(), Dynamic::from(piece.stars.iter().map(|&s| Dynamic::from(s as i64)).collect::<rhai::Array>()));
        m.insert("pairs".into(), Dynamic::from(piece.pairs.iter()
            .map(|&(a, b)| Dynamic::from(vec![Dynamic::from(a as i64), Dynamic::from(b as i64)])).collect::<rhai::Array>()));
        m.insert("points".into(), Dynamic::from(vecs(&piece.points)));
        m.insert("labels".into(), Dynamic::from(vecs(&piece.labels)));
        m.insert("scale".into(), Dynamic::from(Vec2::new(piece.scale.x as f64, piece.scale.y as f64)));
        Dynamic::from(m)
    }).collect())
}

/// A value kind's sketch as a parts map, for `canvas.add(parts, styles)`:
/// each part a path, plus `labels`, an array of #{ at, text }, and
/// `label_size`, how big to write them.
pub(super) fn sketch_parts(sketch: crate::journal::kinds::Sketch) -> Map {
    let mut parts = Map::new();
    for (name, path) in sketch.parts {
        parts.insert(name.into(), Dynamic::from(Path::new(path)));
    }
    let labels: rhai::Array = sketch.labels.into_iter().map(|l| {
        let mut label = Map::new();
        label.insert("at".into(), Dynamic::from(Vec2::new(l.at.x as f64, l.at.y as f64)));
        label.insert("text".into(), l.text.into());
        Dynamic::from_map(label)
    }).collect();
    parts.insert("labels".into(), Dynamic::from_array(labels));
    parts.insert("label_size".into(), Dynamic::from_float(sketch.label_size as f64));
    parts
}

pub fn register(engine: &mut Engine, text: super::path::TextSource) {
    let rng = text.1.clone();
    let mut module = Module::new();
    module.set_native_fn("spiral_text", move |ctx: NativeCallContext, msg: &str, center: Vec2, radius: Dynamic, settings: Map| -> RhaiResult<Canvas> {
        super::path::spiral_text(msg, center, &radius, settings, &text).map_err(|err| error(&ctx, err.to_string()))
    });
    module.set_native_fn("moon", |ctx: NativeCallContext, center: Vec2, radius: Dynamic, phase: Dynamic| -> RhaiResult<Map> {
        moon(&ctx, center, &radius, &phase)
    });
    let r = rng.clone();
    module.set_native_fn("snake", move |ctx: NativeCallContext, head_center: Vec2, tail_end: Vec2, length: Dynamic| -> RhaiResult<Map> {
        snake(&ctx, &r, head_center, tail_end, &length, &Map::new())
    });
    let r = rng.clone();
    module.set_native_fn("snake", move |ctx: NativeCallContext, head_center: Vec2, tail_end: Vec2, length: Dynamic, options: Map| -> RhaiResult<Map> {
        snake(&ctx, &r, head_center, tail_end, &length, &options)
    });
    let r = rng.clone();
    module.set_native_fn("bramble", move |ctx: NativeCallContext, base: Vec2, length: Dynamic| -> RhaiResult<Map> {
        bramble(&ctx, &r, base, &length, &Map::new())
    });
    let r = rng.clone();
    module.set_native_fn("bramble", move |ctx: NativeCallContext, base: Vec2, length: Dynamic, options: Map| -> RhaiResult<Map> {
        bramble(&ctx, &r, base, &length, &options)
    });
    let r = rng.clone();
    module.set_native_fn("bramble_along", move |ctx: NativeCallContext, path: Path, flower_size: Dynamic| -> RhaiResult<Map> {
        bramble_along(&ctx, &r, &path, &flower_size, &Map::new())
    });
    let r = rng.clone();
    module.set_native_fn("bramble_along", move |ctx: NativeCallContext, path: Path, flower_size: Dynamic, options: Map| -> RhaiResult<Map> {
        bramble_along(&ctx, &r, &path, &flower_size, &options)
    });
    let r = rng.clone();
    let snake_along = move |ctx: &NativeCallContext, path: &Path, max_width: &Dynamic, options: &Map| -> RhaiResult<Path> {
        let rng = &r;
        const CALLER: &str = "art::snake_along";
        check_keys(CALLER, options, &["seed"]).map_err(|err| error(ctx, err.to_string()))?;
        let max_width = number_arg(ctx, CALLER, "max_width", max_width)?;
        art::snake_along(&path.borrow(), max_width, seed(ctx, rng, CALLER, options)?)
            .map(Path::new)
            .map_err(|err| error(ctx, format!("{}: {}", CALLER, err)))
    };
    let along = snake_along.clone();
    module.set_native_fn("snake_along", move |ctx: NativeCallContext, path: Path, max_width: Dynamic| -> RhaiResult<Path> {
        along(&ctx, &path, &max_width, &Map::new())
    });
    module.set_native_fn("snake_along", move |ctx: NativeCallContext, path: Path, max_width: Dynamic, options: Map| -> RhaiResult<Path> {
        snake_along(&ctx, &path, &max_width, &options)
    });
    let r = rng.clone();
    module.set_native_fn("eye", move |ctx: NativeCallContext, center: Vec2, length: Dynamic| -> RhaiResult<Map> {
        eye(&ctx, &r, center, &length, &Map::new())
    });
    let r = rng.clone();
    module.set_native_fn("eye", move |ctx: NativeCallContext, center: Vec2, length: Dynamic, options: Map| -> RhaiResult<Map> {
        eye(&ctx, &r, center, &length, &options)
    });
    let r = rng.clone();
    module.set_native_fn("huffman_stars", move |ctx: NativeCallContext, symbols: Dynamic, output_base: i64, center: Vec2, width: Dynamic, height: Dynamic| -> RhaiResult<Map> {
        huffman_stars(&ctx, &r, &symbols, output_base, center, &width, &height, &Map::new())
    });
    let r = rng.clone();
    module.set_native_fn("huffman_stars", move |ctx: NativeCallContext, symbols: Dynamic, output_base: i64, center: Vec2, width: Dynamic, height: Dynamic, options: Map| -> RhaiResult<Map> {
        huffman_stars(&ctx, &r, &symbols, output_base, center, &width, &height, &options)
    });
    module.set_native_fn("constellations", |ctx: NativeCallContext, chart: Map, count: i64, center: Vec2, width: Dynamic, height: Dynamic| -> RhaiResult<rhai::Array> {
        constellations(&ctx, &chart, count, center, &width, &height, &Map::new())
    });
    module.set_native_fn("constellations", |ctx: NativeCallContext, chart: Map, count: i64, center: Vec2, width: Dynamic, height: Dynamic, options: Map| -> RhaiResult<rhai::Array> {
        constellations(&ctx, &chart, count, center, &width, &height, &options)
    });
    // art::sketch(value): a value kind's default drawing, as parts.
    module.set_native_fn("sketch", |ctx: NativeCallContext, value: Dynamic| -> RhaiResult<Map> {
        match crate::journal::kinds::sketch(&value) {
            Ok(Some(sketch)) => Ok(sketch_parts(sketch)),
            Ok(None) => Err(error(&ctx, format!("art::sketch: a {} has no sketch", crate::journal::kinds::kind_of(&value).map_or("value", |k| k.name())))),
            Err(err) => Err(error(&ctx, format!("art::sketch: {}", err))),
        }
    });
    engine.register_static_module("art", module.into());
}

#[cfg(test)]
mod tests {
    use super::super::JournalEngine;
    use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::new(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).expect("Failed to load fonts.json")))
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
            c.add(snake, new_style(new_brush_style("#f2ead8", "Ink Pen", 1.0), new_handwriting_style()));
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
            let ink = |colour| new_style(new_brush_style(colour, "Ink Pen", 1.0), new_handwriting_style());
            let c = new_canvas();
            c.add(plant.vines, ink.call("#c8a27a"));
            c.add(plant.leaves, ink.call("#9fd17a"));
            c.add(plant.flowers, ink.call("#f2ead8"));
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
            let ink = |colour| new_style(new_brush_style(colour, "Ink Pen", 1.0), new_handwriting_style());
            let c = new_canvas();
            c.add(plant.vines, ink.call("#c8a27a"));
            c.add(plant.leaves, ink.call("#a8d68a"));
            c.add(plant.flowers, ink.call("#f2ead8"));
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
            c.add(art::snake_along(path, 30.0, #{ seed: 1 }), new_style(new_brush_style("#f2ead8", "Ink Pen", 1.0), new_handwriting_style()));
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
            let draw = |seed| art::eye(vec2(100.0, 100.0), 200.0, #{ gaze_angle: 0.5, messiness: 0.6, seed: seed });
            let e = draw.call(7);
            if e.lines.to_string() != draw.call(7).lines.to_string() { throw "the same seed should draw the same eye"; }
            if e.lines.to_string() == draw.call(8).lines.to_string() { throw "another seed should draw another eye"; }
            for part in ["lines", "accent", "wash", "rim"] { if e[part].is_empty { throw "the eye should have " + part; } }
            if e.iris_radius <= 0.0 || e.iris_radius > 100.0 { throw "the iris should be inside the eye: " + e.iris_radius; }
            // About as long as asked, round its centre.
            let b = e.lines.bounds();
            if b.width < 190.0 || b.width > 320.0 || (b.center - vec2(100.0, 100.0)).length() > 40.0 { throw "the eye should be where asked: " + b; }
            let tilted = art::eye(vec2(0.0, 0.0), 200.0, #{ gaze_angle: PI(), messiness: 0.0, seed: 1, angle: PI() / 2.0, gaze: 1.0, openness: 0.3 });
            if tilted.lines.bounds().height < 190.0 { throw "turned upright, the eye should be tall: " + tilted.lines.bounds(); }
            let flat = art::eye(vec2(0.0, 0.0), 200.0, #{ gaze_angle: 0.0, messiness: 0.5, seed: 1, flat: true });
            let ink = new_style(new_brush_style("#123", "Ink Pen", 1.2, #{ min_pressure: 0.2, max_pressure: 0.6 }), new_handwriting_style());
            let p = new_page();
            p.add(new_canvas().add(e, #{ lines: ink, accent: ink, wash: ink, rim: ink }).add(flat.lines, ink));
            journal_entry.add(p);
        "##;
        engine().build_entry_str(script).expect("script should run");
        for (call, message) in [
            ("art::eye(vec2(0.0, 0.0), 200.0, #{ gaze_angle: 0.0, messiness: 1.5, seed: 1 });", "art::eye: messiness runs from 0 to 1"),
            ("art::eye(vec2(0.0, 0.0), 0.0, #{ gaze_angle: 0.0, messiness: 0.5, seed: 1 });", "art::eye: length must be a positive number"),
            ("art::eye(vec2(0.0, 0.0), 200.0, #{ gaze_angle: 0.0, messiness: 0.5, seed: 1, squint: 1 });", "art::eye: unknown option 'squint'"),
            ("art::eye(vec2(0.0, 0.0), 200.0, #{ gaze_angle: 0.0, messiness: 0.5, seed: 1, color: \"#123\" });", "art::eye: unknown option 'color'"),
            ("art::eye(vec2(0.0, 0.0), 200.0, #{ gaze_angle: 0.0, messiness: 0.5, seed: 1, flat: 1 });", "art::eye: 'flat' should be true or false"),
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
    fn huffman_stars_charts_the_locks_code() {
        let ok: rhai::Array = engine().engine.eval(r#"
            let chart = art::huffman_stars(symbols(27, "SEE THE SEA"), 2, vec2(0, 0), 900, 600, #{ seed: 3, iterations: 40 });
            let leaves = chart.nodes.filter(|n| n.symbol != ());
            let e = leaves.filter(|n| n.text == "E")[0];
            let z = leaves.filter(|n| n.text == "Z")[0];
            let again = art::huffman_stars(symbols(27, "SEE THE SEA"), 2, vec2(0, 0), 900, 600, #{ seed: 3, iterations: 40 });
            let other = art::huffman_stars(symbols(27, "SEE THE SEA"), 2, vec2(0, 0), 900, 600, #{ seed: 4, iterations: 40 });
            // Without smoothing, only the six symbols used are stars, and
            // base 3 joins them under three more.
            let tight = art::huffman_stars(symbols(27, "SEE THE SEA"), 3, vec2(100, 100), 400, 400, #{ smoothing: 0, round: true, star_size: 10, gap: 0, variation: 0, iterations: 0 });
            let b = new_canvas().add(tight.stars, new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style())).bounds();
            [chart.nodes.len() == 53, chart.points.len() == 53, chart.edges.len() == 52, leaves.len() == 27,
             e.code.len() < z.code.len(), e.weight == 5 && z.weight == 1, chart.nodes[0].parent == () && chart.nodes[0].depth == 0,
             chart.nodes[0].weight == 38 && chart.nodes[0].text == (), chart.edges[0][0] == 0,
             !new_edges(chart.points, chart.edges).is_empty && !chart.stars.is_empty && !chart.links.is_empty,
             `${again.links}` == `${chart.links}` && `${other.links}` != `${chart.links}`,
             tight.nodes.len() == 9, b.min.x >= -100.5 && b.max.x <= 300.5 && b.min.y >= -100.5 && b.max.y <= 300.5]
        "#).unwrap();
        assert!(ok.iter().all(|b| b.as_bool().unwrap()), "{:?}", ok);
        for (call, message) in [
            ("art::huffman_stars(\"SEE\", 2, vec2(0, 0), 900, 600);", "art::huffman_stars: symbols should be symbols, as symbols(27, TEXT) gives, got string"),
            ("art::huffman_stars(symbols(27, \"SEE\"), 1, vec2(0, 0), 900, 600);", "art::huffman_stars: output_base must be from 2 to 27"),
            ("art::huffman_stars(symbols(27, \"SEE\"), 2, vec2(0, 0), 0, 600);", "art::huffman_stars: width and height must be positive numbers"),
            ("art::huffman_stars(symbols(27, \"SEE\"), 2, vec2(0, 0), 900, 600, #{ colour: 1 });", "art::huffman_stars: unknown option 'colour'"),
            ("art::huffman_stars(symbols(27, \"SEE\"), 2, vec2(0, 0), 900, 600, #{ round: 1 });", "art::huffman_stars: 'round' should be true or false"),
            ("art::huffman_stars(symbols(27, \"SEE\"), 2, vec2(0, 0), 900, 600, #{ smoothing: -1 });", "art::huffman_stars: 'smoothing' should be a whole number of at least 0"),
            ("art::huffman_stars(symbols(27, \"SEE\"), 2, vec2(0, 0), 900, 600, #{ variation: 2 });", "art::huffman_stars: variation runs from 0 to 1"),
            ("art::huffman_stars(symbols(27, \"SEE\"), 2, vec2(0, 0), 900, 600, #{ star_size: 400 });", "art::huffman_stars: the chart, 900 by 600, is too small for stars 800 across"),
            ("art::huffman_stars(symbols(10, \"\"), 2, vec2(0, 0), 900, 600, #{ smoothing: 0 });", "art::huffman_stars: there are no symbols and smoothing is 0"),
        ] {
            let err = engine().build_entry_str(&format!("let ok = 1;\n{}", call)).expect_err(call).to_string();
            assert!(err.contains(message) && err.contains("line 2"), "{}", err);
        }
    }

    #[test]
    fn constellations_split_a_chart_into_pages() {
        let ok: rhai::Array = engine().engine.eval(r#"
            let chart = art::huffman_stars(symbols(27, "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG"), 3, vec2(0, 0), 2000, 1500, #{ seed: 2, iterations: 40 });
            let pieces = art::constellations(chart, 4, vec2(600, 600), 1000, 900, #{ label_width: 40, label_height: 30, label_gap: 8 });
            let links = 0;
            for p in pieces { links += p.pairs.len(); }
            let p = pieces[0];
            [pieces.len() == 4, links == chart.edges.len(), p.points.len() == p.stars.len() && p.labels.len() == p.stars.len(),
             p.scale.x > 0 && p.scale.y > 0, type_of(p.stars[0]) == "i64", art::constellations(chart, 1, vec2(0, 0), 500, 500).len() == 1,
             art::constellations(chart, 4, vec2(0, 0), 500, 500, #{ split_at: [0] }).len() == 5]
        "#).unwrap();
        assert!(ok.iter().all(|b| b.as_bool().unwrap()), "{:?}", ok);
        for (call, message) in [
            ("art::constellations(#{ points: [] }, 2, vec2(0, 0), 900, 600);", "art::constellations: the chart should have 'edges'"),
            ("art::constellations(#{ points: [vec2(0, 0)], edges: [], nodes: [#{ radius: 1.0 }] }, 0, vec2(0, 0), 900, 600);", "art::constellations: count must be at least 1"),
            ("art::constellations(#{ points: [vec2(0, 0), vec2(1, 0)], edges: [], nodes: [#{ radius: 1.0 }, #{ radius: 1.0 }] }, 2, vec2(0, 0), 900, 600);", "art::constellations: 2 stars need 1 links"),
            ("art::constellations(#{ points: [vec2(0, 0)], edges: [], nodes: [#{ radius: 1.0 }] }, 2, vec2(0, 0), 900, 600, #{ label: 3 });", "art::constellations: unknown option 'label'"),
            ("art::constellations(#{ points: [vec2(0, 0)], edges: [], nodes: [#{ radius: 1.0 }] }, 2, vec2(0, 0), 900, 600, #{ split_at: 0 });", "art::constellations: 'split_at' should be an array of star indices"),
        ] {
            let err = engine().build_entry_str(&format!("let ok = 1;\n{}", call)).expect_err(call).to_string();
            assert!(err.contains(message) && err.contains("line 2"), "{}", err);
        }
    }

    #[test]
    fn snake_path_errors_name_the_function_and_line() {
        let err = engine().build_entry_str("let ok = 1;\nart::snake(vec2(0.0, 0.0), vec2(5000.0, 0.0), 900.0, #{ head_angle: 0.0, max_width: 30.0, seed: 1 });")
            .expect_err("a tail end out of reach should fail").to_string();
        assert!(err.contains("art::snake: the tail end is") && err.contains("line 2"), "{}", err);
    }
}
