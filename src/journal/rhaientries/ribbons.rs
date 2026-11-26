//! `art::ribbons(trees, width, settings)`: trees (points and the edges
//! joining them, as `spanning_tree` gives them) woven into ribbons that
//! pass over and under each other, each tree's text running along its
//! ribbons wherever nothing passes over them.
//!
//! ```rhai
//! let a = points_a.spanning_tree();
//! let woven = art::ribbons([
//!     #{ points: points_a, edges: a, text: "lorem ipsum dolor sit amet" },
//!     #{ points: points_b, edges: points_b.spanning_tree(), text: "consectetur adipiscing", style: RED },
//! ], 40, #{ type: #{ glyphset: "plain" }, ends: "swallowtail" });
//! for r in woven { page.add(r, #{ lines: PEN, rules: FINE }).add(r.text); }
//! ```

use rand::Rng;
use rhai::{Array, Dynamic, Engine, Map};

use super::markdown::FlowText;
use super::options::{as_number, check_keys};
use super::path::{place_text, PathText, TextSource};
use super::text_settings::TEXT_KEYS;
use super::{Path, RhaiResult, Vec2};
use crate::journal::art::{self, RibbonEnd, RibbonSettings, RibbonTree};
use crate::journal::symbol::{Symbol, SymbolPlacementArgs};

const CALLER: &str = "art::ribbons";
/// What the call takes, besides a style, a type and a paragraph.
const RIBBONS_KEYS: [&str; 10] = ["avoid", "ends", "end_depth", "rule_inset", "repeat", "upright", "flip", "width_variation", "wander", "seed"];
/// How much a ribbon's width varies along it unless told, as a share of it
/// either way.
const WIDTH_VARIATION: f32 = 0.1;
/// What a tree's map holds: its points, edges and text, and what it sets
/// its own way.
const TREE_KEYS: [&str; 8] = ["points", "edges", "text", "width", "style", "type", "paragraph", "spans"];
/// The most glyphs repeated text is set in, all told.
const MOST_GLYPHS: usize = 200_000;
/// How much of the room between the rules, less the clearance, glyphs
/// take, unless the type gives their size: a little less than all of it,
/// so they fit round bends.
const FILL: f32 = 0.96;

/// A whole number at least 0, as an index.
fn index(v: &Dynamic) -> Option<usize> {
    v.as_int().ok().and_then(|i| usize::try_from(i).ok())
}

/// A tree's map, read: the tree to weave, its text, and its settings.
fn read_tree(k: usize, value: &Dynamic, width: f32, settings: &Map) -> RhaiResult<(RibbonTree, FlowText, Map)> {
    let name = format!("{}: tree {}", CALLER, k + 1);
    let map = value.clone().try_cast::<Map>()
        .ok_or_else(|| format!("{}: trees should be maps of points, edges and text; tree {} is a {}", CALLER, k + 1, value.type_name()))?;
    check_keys(&name, &map, &TREE_KEYS)?;
    let get = |key: &str| map.get(key).cloned().ok_or_else(|| format!("{} has no '{}'", name, key));
    let points = get("points")?.try_cast::<Array>().ok_or_else(|| format!("{}'s points should be an array of vec2s", name))?
        .into_iter()
        .map(|p| p.try_cast::<Vec2>().map(parry2d::math::Vec2::from).ok_or_else(|| format!("{}'s points should be vec2s", name).into()))
        .collect::<RhaiResult<Vec<_>>>()?;
    let edges = get("edges")?.try_cast::<Array>().ok_or_else(|| format!("{}'s edges should be an array of [a, b] pairs, as spanning_tree gives", name))?
        .into_iter()
        .map(|e| {
            let pair = e.try_cast::<Array>().filter(|p| p.len() == 2).and_then(|p| Some((index(&p[0])?, index(&p[1])?)));
            pair.ok_or_else(|| format!("{}'s edges should be [a, b] pairs of point indices", name).into())
        })
        .collect::<RhaiResult<Vec<_>>>()?;
    let text = FlowText::from_dynamic(&format!("{}'s text", name), get("text")?)?;
    if matches!(text, FlowText::Symbols(_)) {
        return Err(format!("{}'s text should be a string or markdown(...); symbols aren't set on ribbons yet", name).into());
    }
    let width = match map.get("width") {
        None => width,
        Some(w) => as_number(&name, "width", w)?,
    };
    let mut own = settings.clone();
    for key in TEXT_KEYS {
        if let Some(value) = map.get(key) {
            own.insert(key.into(), value.clone());
        }
    }
    Ok((RibbonTree { points, edges, width }, text, own))
}

/// A true or false option.
fn flag(settings: &Map, key: &str, default: bool) -> RhaiResult<bool> {
    settings.get(key).map_or(Ok(default), |v| v.as_bool().map_err(|t| format!("{}: '{}' should be true or false, got {}", CALLER, key, t).into()))
}

/// A number option, if given.
fn optional(settings: &Map, key: &str) -> RhaiResult<Option<f32>> {
    settings.get(key).map(|v| as_number(CALLER, &format!("'{}'", key), v)).transpose()
}

pub(super) fn ribbons(trees: &Array, width: &Dynamic, settings: Map, source: &TextSource, engine: &Engine) -> RhaiResult<Array> {
    let width = as_number(CALLER, "width", width)?;
    if !(width > 0.0 && width.is_finite()) {
        return Err(format!("{}: width must be positive, got {}", CALLER, width).into());
    }
    let keys: Vec<&str> = TEXT_KEYS.iter().chain(RIBBONS_KEYS.iter()).copied().collect();
    check_keys(CALLER, &settings, &keys)?;
    let ends = match settings.get("ends") {
        None => RibbonEnd::Swallowtail,
        Some(v) => v.clone().into_string().ok().and_then(|name| RibbonEnd::named(&name))
            .ok_or_else(|| format!("{}: 'ends' should be one of {}, got {}", CALLER, RibbonEnd::NAMES.map(|n| format!("\"{}\"", n)).join(", "), v))?,
    };
    let woven_settings = RibbonSettings {
        ends,
        end_depth: optional(&settings, "end_depth")?,
        rule_inset: optional(&settings, "rule_inset")?,
        flip: flag(&settings, "flip", false)?,
        upright: flag(&settings, "upright", true)?,
        width_variation: optional(&settings, "width_variation")?.unwrap_or(WIDTH_VARIATION),
        wander: optional(&settings, "wander")?.unwrap_or(0.0),
        seed: match settings.get("seed") {
            None => source.1.borrow_mut().random(),
            Some(seed) => seed.as_int().map(|s| s as u64).map_err(|t| format!("{}: 'seed' should be a whole number, got {}", CALLER, t))?,
        },
    };
    let repeat = flag(&settings, "repeat", false)?;
    let read: Vec<(RibbonTree, FlowText, Map)> = trees.iter().enumerate().map(|(k, tree)| read_tree(k, tree, width, &settings)).collect::<RhaiResult<_>>()?;
    let woven = art::weave(&read.iter().map(|(tree, _, _)| tree.clone()).collect::<Vec<_>>(), &woven_settings)
        .map_err(|err| format!("{}: {}", CALLER, err))?;

    let mut out = Array::new();
    for (t, (_, text, own)) in read.into_iter().enumerate() {
        let set = PathText::parse_words(CALLER, &text, own, source, &RIBBONS_KEYS)?;
        let (text_canvas, unplaced) = write(&woven, t, &set, repeat, engine, source)?;
        let mut parts = Map::new();
        parts.insert("lines".into(), Dynamic::from(Path::new(woven.lines(t))));
        parts.insert("rules".into(), Dynamic::from(Path::new(woven.rules(t))));
        parts.insert("outline".into(), Dynamic::from(Path::new(woven.outline(t))));
        parts.insert("text".into(), Dynamic::from(text_canvas));
        parts.insert("unplaced".into(), Dynamic::from(unplaced as i64));
        out.push(Dynamic::from_map(parts));
    }
    Ok(out)
}

/// Tree `t`'s text along its ribbons, once or over and over: the canvas,
/// and how many of its glyphs didn't fit.
fn write(woven: &art::Ribbons, t: usize, set: &PathText, repeat: bool, engine: &Engine, source: &TextSource) -> RhaiResult<(super::Canvas, usize)> {
    let tree = format!("tree {}", t + 1);
    let room = woven.room(t) as f32;
    if !room.is_finite() {
        // No ribbons: nothing to write on.
        return Ok((set.draw_runs(engine, Vec::new(), &source.1)?, if repeat { 0 } else { set.symbols.len() }));
    }
    // How far the glyphs' ink reaches up and down from the line they're
    // set on, at glyph scale 1: half of all it spans, the line being moved
    // to put the middle of that on the ribbon's.
    let (top, bottom) = set.symbols.iter().filter(|g| !g.is_blank())
        .flat_map(|g| {
            let at = g.position();
            g.footprint().into_iter().map(move |p| p.y - at.y)
        })
        .fold((0.0f32, 0.0f32), |(top, bottom), y| (top.min(y), bottom.max(y)));
    let reach = ((bottom - top) / 2.0).max(0.05);
    let below = (top + bottom) / 2.0;
    // The glyphs keep the clearance from the rules, and from ribbons over
    // theirs, however the hand sets them off true.
    let clear = |size: f32| set.clearance + set.jitter.stray(size);
    let size = match set.size {
        Some(size) => size,
        None => {
            let first = FILL * (room - set.clearance) / reach;
            FILL * (room - clear(first)) / reach
        }
    };
    let margin = clear(size);
    let half = room - margin;
    if !(size > 0.0) || !(half > 0.0) || size * reach > half + 1e-4 {
        return Err(format!(
            "{}: {}'s glyphs at glyph scale {} reach {} either side of the line, but its ribbons have room for {} between their rules, less the clearance; make the ribbons wider or the glyphs smaller",
            CALLER, tree, size, size * reach, half.max(0.0),
        ).into());
    }
    let pad = margin as f64 + woven.step(t) / 2.0;
    let lines = woven.text_lines(t, half as f64, pad, (below * size) as f64);
    let longest = lines.iter().map(|l| l.longest).fold(0.0, f32::max);
    // Repeated, copies of the text with a space between, enough to more
    // than fill every stretch.
    let count = set.symbols.len();
    let copies = if repeat && count > 0 {
        let along: f32 = lines.iter().flat_map(|l| &l.stretches).map(|(path, _)| path.length()).sum();
        let each: f32 = set.symbols.iter().map(|g| g.outline(size).width()).sum::<f32>().max(size * 0.1);
        (((along / each).ceil() as usize + 2).min(MOST_GLYPHS / count.max(1))).max(1)
    } else {
        1
    };
    let gap = set.symbols.iter().find(|g| g.is_blank()).cloned();
    let mut symbols: Vec<Symbol> = Vec::with_capacity(copies * (count + 1));
    for k in 0..copies {
        if k > 0 {
            if let Some(gap) = &gap {
                symbols.push(gap.clone());
            }
        }
        symbols.extend(set.symbols.iter().cloned());
    }
    let tree_avoid = set.avoid.now();
    let mut rest = symbols;
    let mut placed: Vec<Symbol> = Vec::new();
    for line in &lines {
        let args = SymbolPlacementArgs {
            avoid: tree_avoid.as_deref(),
            padding: set.avoid.padding(),
            spacing: set.spacing,
            angle: set.angle,
            within: Some(&line.region),
            longest_line: Some(longest),
            apart_on_bends: true,
            ..SymbolPlacementArgs::new(size)
        };
        for (path, flipped) in &line.stretches {
            if rest.is_empty() {
                break;
            }
            // Turned right round to set along a stretch read leftward, and
            // back for what's left.
            let turn = |glyphs: &mut Vec<Symbol>| {
                if *flipped && set.angle.is_none() {
                    glyphs.iter_mut().for_each(|g| g.rotate_rad_by(std::f32::consts::PI));
                }
            };
            turn(&mut rest);
            let (here, mut left) = place_text(rest, path, &args, &set.mode);
            turn(&mut left);
            placed.extend(here);
            rest = left;
        }
    }
    // Each copy drawn from the text's start; the spaces between them
    // aren't the text's.
    let period = count + usize::from(gap.is_some() && copies > 1);
    let mut runs: Vec<(Vec<Symbol>, usize)> = Vec::new();
    let mut glyphs = placed.into_iter().enumerate().peekable();
    while glyphs.peek().is_some() {
        let mut run = Vec::new();
        while let Some((i, glyph)) = glyphs.next_if(|(i, _)| run.is_empty() || i % period.max(1) != 0) {
            if i % period.max(1) < count {
                run.push(glyph);
            }
        }
        runs.push((run, 0));
    }
    let unplaced = if repeat { 0 } else { rest.len() };
    Ok((set.draw_runs(engine, runs, &source.1)?, unplaced))
}
