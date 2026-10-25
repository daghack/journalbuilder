//! Paths for rhai scripts: building them from lines and curves, moving
//! them, and writing text along them.
//!
//! Every path function changes the path and returns that same (shared)
//! path, so statements, assignments and chains all work:
//! `p.move_to(a).line_to(b);` draws both segments into `p`.
//!
//! ```rhai
//! // Text along a path, each glyph turned to it, or held upright (or at
//! // the paragraph's angle, in radians) and set along it. Text calls take
//! // a style, a type and a paragraph (see text_settings.rs).
//! let text = #{ style: ink, type: new_type_style(glyphset("plain"), 20.0) };
//! let words = arc.fill_with_text("lorem ipsum", text);    // a canvas per copy of the path
//! let upright = arc.fill_with_text("lorem ipsum", text + #{ paragraph: #{ angle: 0.0 } });
//! let fitted = arc.fit_text("lorem", text);               // glyph_scale ignored
//! let line = write_line("lorem", vec2(0, 0), text);       // from a point, to the right
//! let wavy = write_line("lorem", vec2(0, 0), text + #{ style: #{ baseline_wander: 4.0 } });   // the line wanders
//!
//! // Shapes, and points spaced round them.
//! new_polygon(c, 50.0, 5, #{ step: 2 });                  // a star in one line
//! new_star(c, 50.0, 20.0, 6);
//! new_ellipse(c, 80.0, 30.0, #{ angle: 0.3 });
//! new_wave(a, b, #{ amplitude: 10.0, waves: 4 });
//! new_polyline([a, b, c], #{ closed: true });
//! for at in new_circle(c, 50.0).points_along(12, #{ start: 0.75 }) { ... }   // #{ position, angle }
//!
//! // Hatch the inside of a closed path: lines `spacing` apart at `angle`
//! // (radians), their ends jittered by up to `roughness`.
//! let shading = new_circle(vec2(0.0, 0.0), 20.0).hatch(3.0, #{ angle: random_range(0.0, 6.28), roughness: 1.0 });
//! // Every option:
//! let shading = outline.hatch(4.0, #{
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
//!
//! ```rhai
//! // Cut a path where a closed shape covers it: lines behind a figure.
//! background.erase_inside(figure_outline);
//! frame.erase_outside(new_circle(vec2(0, 0), 100), #{ fill_rule: "evenodd" });
//! ```
//!
//! ```rhai
//! // Balanced-ternary digits along a path, as the balanced_ternary font
//! // writes them down its stems: the marks (draw the path itself too).
//! // The first sits at the path's start, and +1 is to the right going
//! // from the path's start to its end.
//! let marks = stem.fit_text(balanced_ternary(42), #{ style: ink });   // spread over the path
//! let copies = stem.fill_with_text(balanced_ternary("+-0+"), #{
//!     style: ink,
//!     type: #{ glyph_scale: 12.0 }, // from one digit to the next
//!     width: 5.0,                   // how far marks reach out (default 0.6 of the glyph scale)
//! });                               // a canvas per copy of the path
//! ```
//!
//! The shape's subpaths are closed shapes as for hatching, with the same
//! fill rules. Lines and curves are cut where they cross its outline and
//! stay lines and curves.

use rhai::{Dynamic, Engine, EvalAltResult, Map};
use super::RhaiResult;
use std::rc::Rc;

use super::options::{as_number, check_keys, float, number, positive};
use super::canvas::bounds_map;
use super::random::SharedRng;
use super::{avoid, random, Canvas, CanvasData, Path, Style, Vec2};
use crate::journal::clip::{clip, Keep};
use crate::journal::hatch::{FillRule, HatchOptions};
use crate::journal::canvas::textbox::TextBoxLineMode;
use crate::journal::paths::StrokePath;
use crate::journal::symbol::{joined_paths, place_along_path, place_words_along_path, ScaleField, Symbol, SymbolPlacementArgs};
use crate::journal::fonts::FontBook;
use crate::journal::ternary::{ternary_along, TernaryOptions};
use super::generate::BalancedTernary;
use super::text_settings::{Jitter, TextSettings, Wander};
use parry2d::math::Vec2 as PVec;
use rhai::{Array, INT};

/// Where text's glyphs come from: the fonts, and the generator
/// that picks glyph variants.
pub(super) type TextSource = (Rc<FontBook>, random::SharedRng);

/// The `angle` option: glyphs held at this angle (radians, 0 upright)
/// along the path, rather than turned to follow it.
fn place_text(symbols: Vec<Symbol>, path: &StrokePath, args: &SymbolPlacementArgs, mode: &TextBoxLineMode) -> (Vec<Symbol>, Vec<Symbol>) {
    match mode {
        TextBoxLineMode::Flat => place_along_path(symbols, path, args),
        TextBoxLineMode::WordsByBlankSymbol(split) => place_words_along_path(symbols, path, args, *split),
    }
}

/// What text along a path takes, besides font, size and style.
/// What the path text calls take, besides a style, a type and a paragraph.
const PATH_TEXT_KEYS: [&str; 1] = ["avoid"];

struct PathText {
    symbols: Vec<Symbol>,
    style: Style,
    mode: TextBoxLineMode,
    avoid: avoid::AvoidOption,
    spacing: f32,
    angle: Option<f32>,
    jitter: Jitter,
    /// The glyph scale, if the type gives one.
    size: Option<f32>,
    /// The paragraph's line height, in glyph scales.
    line_height: f32,
    /// How far it keeps from what it avoids.
    clearance: f32,
    /// The settings' own options, besides a style, a type and a paragraph.
    options: Map,
}

impl PathText {
    fn parse(caller: &str, msg: &str, settings: Map, text: &TextSource) -> RhaiResult<PathText> {
        PathText::parse_with(caller, msg, settings, text, &PATH_TEXT_KEYS)
    }
    /// As `parse`, taking `keys` besides a style, a type and a paragraph.
    fn parse_with(caller: &str, msg: &str, settings: Map, (symdict, rng): &TextSource, keys: &[&str]) -> RhaiResult<PathText> {
        let text = TextSettings::read(caller, settings, keys)?;
        let font = text.type_.glyphset(caller)?;
        Ok(PathText {
            symbols: font.build(msg, symdict, &mut *rng.borrow_mut()),
            mode: text.paragraph.line_mode(caller, false)?,
            avoid: avoid::AvoidOption::parse(caller, &text.options, text.paragraph.clearance())?,
            spacing: text.type_.tracking(),
            angle: text.paragraph.angle(),
            jitter: text.jitter(),
            size: if text.type_.has_size() { Some(text.type_.size(caller)?) } else { None },
            line_height: text.paragraph.line_height(),
            clearance: text.paragraph.clearance(),
            style: text.style,
            options: text.options,
        })
    }
    /// The glyph scale `caller` sets text at.
    fn size(&self, caller: &str) -> RhaiResult<f32> {
        self.size.ok_or_else(|| format!("{}: the type has no 'glyph_scale', the size glyphs are set at", caller).into())
    }
    fn draw(&self, mut placed: Vec<Symbol>, rng: &SharedRng) -> Canvas {
        let mut rng = rng.borrow_mut();
        self.jitter.apply(&mut placed, &mut *rng);
        let mut canvas = CanvasData::default();
        for (_, path) in joined_paths(&placed, |_| false) {
            canvas.add_path(path, &self.style, &mut *rng);
        }
        Canvas::new(canvas)
    }
}

fn fill_with_text(path: &mut Path, msg: &str, settings: Map, text: &TextSource) -> RhaiResult<Vec<Dynamic>> {
    const CALLER: &str = "fill_with_text";
    let mut set = PathText::parse(CALLER, msg, settings, text)?;
    let fontsize = set.size(CALLER)?;
    let tree = set.avoid.tree.as_ref().map(|a| a.borrow());
    let args = SymbolPlacementArgs {
        avoid: tree.as_deref(),
        padding: set.avoid.padding(),
        spacing: set.spacing,
        angle: set.angle,
        ..SymbolPlacementArgs::new(fontsize)
    };

    let mut all_canvases = Vec::new();
    let mut symbols = std::mem::take(&mut set.symbols);
    let path = path.get();
    let total = symbols.len();
    while !symbols.is_empty() {
        let (placed, unplaced) = place_text(std::mem::take(&mut symbols), &path, &args, &set.mode);
        if placed.is_empty() {
            return Err(format!(
                "{}: glyph {} of {} doesn't fit on the path at glyph scale {}; use a longer path or a smaller scale{}",
                CALLER, total - unplaced.len() + 1, total, fontsize,
                if set.avoid.tree.is_some() { ", or avoid less" } else { "" },
            ).into());
        }
        all_canvases.push(Dynamic::from(set.draw(placed, &text.1)));
        symbols = unplaced;
    }
    Ok(all_canvases)
}

fn fit_text(path: &mut Path, msg: &str, settings: Map, text: &TextSource) -> RhaiResult<Canvas> {
    const CALLER: &str = "fit_text";
    let set = PathText::parse(CALLER, msg, settings, text)?;
    let path = &path.get();
    let tree = set.avoid.tree.as_ref().map(|a| a.borrow());
    let mut args = SymbolPlacementArgs {
        avoid: tree.as_deref(),
        padding: set.avoid.padding(),
        spacing: set.spacing,
        angle: set.angle,
        ..SymbolPlacementArgs::new(20.0)
    };
    let mut window_min = 0.1;
    let mut window_max = 1000.0;
    while window_max - window_min > 0.005 {
        let current_scale = (window_max + window_min) / 2.0;
        args.symbol_scale = current_scale;
        let (_, unplaced) = place_text(set.symbols.clone(), path, &args, &set.mode);
        if !unplaced.is_empty() {
            window_max = current_scale;
        } else {
            window_min = current_scale;
        }
    }
    args.symbol_scale = window_min;
    let (placed, unplaced) = place_text(set.symbols.clone(), path, &args, &set.mode);
    if !unplaced.is_empty() {
        return Err(format!(
            "{}: the text doesn't fit on the path even at glyph scale {}: {} of {} glyphs left over",
            CALLER, window_min, unplaced.len(), placed.len() + unplaced.len(),
        ).into());
    }
    Ok(set.draw(placed, &text.1))
}

/// What `art::spiral_text` takes, besides a style, a type and a paragraph.
const SPIRAL_TEXT_KEYS: [&str; 3] = ["avoid", "inner", "minimum_scale"];

/// A clockwise spiral on the page about `center`, from the top, out from
/// `inner`, for glyphs reaching `reach` either side of it and `wide` times
/// that along it, turns `gap` apart between their glyphs. Once full-size
/// glyphs would cross the circle of radius `room`, corners and all, each
/// fills the band between it and the glyphs on the turn inside, shrinking
/// as the band narrows, and the spiral ends
/// where they'd reach less than `least`. The path, a polyline fine enough
/// for glyphs `fine` across, and how far along it each point is with the
/// glyphs' reach there as a factor of `reach`.
fn spiral(center: PVec, inner: f32, reach: f32, wide: f32, gap: f32, room: f32, least: f32, fine: f32) -> (StrokePath, Vec<(f32, f32)>) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    // The most a glyph can reach either side of its centre, `base + (k -
    // 1) a` from the centre, its outer corners inside the circle. Glyphs are
    // turned to the spiral where they start, so they lean out by about half
    // the angle they span.
    let fits = |base: f32, k: f32, a: f32| {
        let (r, h) = (base + (k - 1.0) * a, wide * a);
        let lean = h / r.max(1e-3);
        let (out, along) = (r + a * lean.cos() + h * lean.sin(), h * lean.cos() - a * lean.sin());
        out * out + along * along <= room * room
    };
    let most = |base: f32, k: f32| {
        if fits(base, k, reach) { return reach; }
        let (mut low, mut high) = (0.0, reach);
        for _ in 0..24 {
            let mid = (low + high) / 2.0;
            if fits(base, k, mid) { low = mid } else { high = mid }
        }
        low
    };
    let at = |t: f32, r: f32| center + PVec::new((t - FRAC_PI_2).cos(), (t - FRAC_PI_2).sin()) * r;
    // Each point's angle, radius and glyphs' reach.
    let mut turns: Vec<(f32, f32, f32)> = Vec::new();
    let mut sizes = Vec::new();
    let mut path = StrokePath::new();
    let (mut t, mut length) = (0.0, 0.0);
    let mut last = at(0.0, inner);
    loop {
        let (r, a) = if t < TAU {
            // The first turn: nothing inside it.
            let r = inner + (2.0 * reach + gap) * t / TAU;
            (r, most(r, 1.0))
        } else {
            // The glyphs a turn in, where this angle last came round.
            let back = t - TAU;
            let k = turns.partition_point(|&(u, _, _)| u < back).clamp(1, turns.len() - 1);
            let ((u0, r0, a0), (u1, r1, a1)) = (turns[k - 1], turns[k]);
            let f = ((back - u0) / (u1 - u0)).clamp(0.0, 1.0);
            let top = r0 + (r1 - r0) * f + a0 + (a1 - a0) * f + gap;
            let a = most(top, 2.0);
            (top + a, a)
        };
        if a < least || turns.len() > 1_000_000 {
            break;
        }
        let p = at(t, r);
        if turns.is_empty() {
            path = path.move_to(p.x, p.y);
        } else {
            length += (p - last).length();
            path = path.line_to(p.x, p.y);
        }
        last = p;
        turns.push((t, r, a));
        sizes.push((length, a / reach));
        t += (fine * 0.05 / r.max(fine * 0.1)).min(0.1);
    }
    (path, sizes)
}

/// `art::spiral_text(text, center, radius, settings)`: the text written
/// out from the middle in a clockwise spiral, as big as it can be and still
/// fit inside the circle of `radius` about `center`, and no bigger than
/// the type's glyph_scale if it gives one. Glyphs keep one size until
/// they'd come nearer the circle than the paragraph's clearance; from
/// there on each fills the room left between the turn inside it and the
/// circle (less the clearance), shrinking as that narrows, until they'd be
/// smaller than `minimum_scale` of their size (default a third). Turns
/// are the paragraph's line height apart, in the glyphs' own height (the
/// tallest's) rather than glyph scales; the spiral starts `inner` from the
/// centre (default 0.85 of the glyph scale).
pub(super) fn spiral_text(msg: &str, center: Vec2, radius: &Dynamic, settings: Map, text: &TextSource) -> RhaiResult<Canvas> {
    const CALLER: &str = "art::spiral_text";
    let radius = as_number(CALLER, "radius", radius)?;
    if !(radius > 0.0) {
        return Err(format!("{}: radius must be positive, got {}", CALLER, radius).into());
    }
    let set = PathText::parse_with(CALLER, msg, settings, text, &SPIRAL_TEXT_KEYS)?;
    let inner = if set.options.contains_key("inner") { Some(number(CALLER, &set.options, "inner")?.unwrap_or(0.0).max(0.0)) } else { None };
    let minimum_scale = if set.options.contains_key("minimum_scale") { positive(CALLER, &set.options, "minimum_scale")?.min(1.0) } else { 1.0 / 3.0 };
    let center: PVec = center.into();
    // How far the glyphs reach either side of the spiral, and how far
    // apart turns are, in glyph scales.
    let reach = set.symbols.iter().filter(|g| !g.is_blank()).map(|g| g.outline(1.0).reach()).fold(0.0, f32::max).max(0.05);
    let line = reach * 2.0 * set.line_height;
    // How far the widest glyph reaches along the spiral, in reaches.
    let wide = set.symbols.iter().filter(|g| !g.is_blank()).map(|g| g.outline(1.0).width() / 2.0).fold(0.0, f32::max) / reach;
    // The glyphs' ink stays the clearance inside the circle.
    let room = radius - set.clearance;
    if !(room > 0.0) {
        return Err(format!("{}: the circle is no bigger than the clearance, {}", CALLER, set.clearance).into());
    }
    let ring: Vec<PVec> = (0..256).map(|i| {
        let a = i as f32 / 256.0 * std::f32::consts::TAU;
        center + PVec::new(a.cos(), a.sin()) * room
    }).collect();
    let within = crate::journal::canvas::textbox::Region::new(vec![ring]);
    let tree = set.avoid.tree.as_ref().map(|a| a.borrow());
    let args = SymbolPlacementArgs {
        avoid: tree.as_deref(),
        padding: set.avoid.padding(),
        spacing: set.spacing,
        angle: set.angle,
        within: Some(&within),
        ..SymbolPlacementArgs::new(1.0)
    };
    let place = |scale: f32| {
        let start = inner.unwrap_or(scale * 0.85);
        let full = scale * reach;
        let (path, sizes) = spiral(center, start, full, wide, scale * (line - 2.0 * reach), room, full * minimum_scale, scale * minimum_scale);
        if sizes.len() < 2 {
            return (Vec::new(), set.symbols.clone());
        }
        // A glyph's size where its room starts along the spiral.
        let scale_at = |d: f32| {
            let k = sizes.partition_point(|&(l, _)| l < d).clamp(1, sizes.len() - 1);
            let ((l0, f0), (l1, f1)) = (sizes[k - 1], sizes[k]);
            f0 + (f1 - f0) * ((d - l0) / (l1 - l0).max(1e-6)).clamp(0.0, 1.0)
        };
        let args = SymbolPlacementArgs { symbol_scale: scale, scale_at: Some(ScaleField(&scale_at)), ..args.clone() };
        place_text(set.symbols.clone(), &path, &args, &set.mode)
    };
    let largest = set.size.map_or(radius, |size| size.min(radius));
    let (placed, unplaced) = place(largest);
    if unplaced.is_empty() {
        return Ok(set.draw(placed, &text.1));
    }
    let (mut window_min, mut window_max) = (0.0, largest);
    while window_max - window_min > radius * 0.0005 {
        let scale = (window_max + window_min) / 2.0;
        if place(scale).1.is_empty() { window_min = scale } else { window_max = scale }
    }
    let (placed, unplaced) = place(window_min);
    if window_min <= 0.0 || !unplaced.is_empty() {
        return Err(format!("{}: the text doesn't fit in the circle at any glyph scale{}", CALLER,
            if inner.is_some() { "; make 'inner' smaller" } else { "" }).into());
    }
    Ok(set.draw(placed, &text.1))
}

fn write_line(msg: &str, at: Vec2, settings: Map, text: &TextSource) -> RhaiResult<Canvas> {
    const CALLER: &str = "write_line";
    let set = PathText::parse(CALLER, msg, settings, text)?;
    let size = set.size(CALLER)?;
    let wander = set.style.handwriting().baseline_wander();
    let start: PVec = at.into();
    let length = (set.symbols.len() as f32 + 1.0) * size * 4.0;
    let line = if wander > 0.0 {
        Wander::new(wander, size, length, &mut *text.1.borrow_mut()).path(start)
    } else {
        StrokePath::line(start, start + PVec::new(length, 0.0))
    };
    let tree = set.avoid.tree.as_ref().map(|a| a.borrow());
    let args = SymbolPlacementArgs {
        avoid: tree.as_deref(),
        padding: set.avoid.padding(),
        spacing: set.spacing,
        angle: set.angle,
        ..SymbolPlacementArgs::new(size)
    };
    let (placed, unplaced) = place_text(set.symbols.clone(), &line, &args, &set.mode);
    if !unplaced.is_empty() {
        return Err(format!("{}: {} of {} glyphs didn't fit on the line{}", CALLER, unplaced.len(), placed.len() + unplaced.len(),
            if set.avoid.tree.is_some() { "; avoid less" } else { "" }).into());
    }
    Ok(set.draw(placed, &text.1))
}

/// What balanced-ternary text takes, besides a style and a type.
const TERNARY_KEYS: [&str; 1] = ["width"];

/// A style, the glyph scale if the type gives one, and the marks' width.
fn ternary_settings(caller: &str, settings: Map) -> RhaiResult<(Style, Option<f32>, Option<f32>)> {
    let text = TextSettings::read(caller, settings, &TERNARY_KEYS)?;
    let width = if text.options.contains_key("width") { Some(positive(caller, &text.options, "width")?) } else { None };
    let size = if text.type_.has_size() { Some(text.type_.size(caller)?) } else { None };
    Ok((text.style, size, width))
}

fn marks_canvas(marks: StrokePath, style: &Style, rng: &SharedRng) -> Canvas {
    let mut canvas = CanvasData::default();
    canvas.add_path(marks, style, &mut *rng.borrow_mut());
    Canvas::new(canvas)
}

/// `path.fill_with_text(bt, #{ style, type, width })`: balanced-ternary
/// digits along the path, the glyph scale apart, over as many copies of
/// the path as it takes, one canvas per copy.
fn fill_with_ternary(path: &mut Path, bt: &BalancedTernary, settings: Map, rng: &SharedRng) -> RhaiResult<Vec<Dynamic>> {
    const CALLER: &str = "fill_with_text";
    let (style, size, width) = ternary_settings(CALLER, settings)?;
    let size = size.ok_or_else(|| format!("{}: the type has no 'glyph_scale', how far apart digits are", CALLER))?;
    let path = path.get();
    let length = path_length(CALLER, &path)?;
    let per_copy = (length / size * 1.0001).floor() as usize;
    if per_copy == 0 {
        return Err(format!("{}: a digit {} long doesn't fit on the path, {} long", CALLER, size, length).into());
    }
    let options = TernaryOptions { spacing: Some(size), width };
    bt.digits().chunks(per_copy).map(|digits| {
        ternary_along(&path, digits, &options)
            .map(|marks| Dynamic::from(marks_canvas(marks, &style, rng)))
            .map_err(|e| format!("{}: {}", CALLER, e).into())
    }).collect()
}

/// `path.fit_text(bt, #{ style, width })`: balanced-ternary digits
/// spread evenly over the whole path, as a canvas.
fn fit_ternary(path: &mut Path, bt: &BalancedTernary, settings: Map, rng: &SharedRng) -> RhaiResult<Canvas> {
    const CALLER: &str = "fit_text";
    let (style, _, width) = ternary_settings(CALLER, settings)?;
    let options = TernaryOptions { spacing: None, width };
    ternary_along(&path.borrow(), bt.digits(), &options)
        .map(|marks| marks_canvas(marks, &style, rng))
        .map_err(|e| format!("{}: {}", CALLER, e).into())
}

/// The length of a path of one subpath.
fn path_length(caller: &str, path: &StrokePath) -> RhaiResult<f32> {
    let lines = path.flattened(0.01);
    let [(points, closed)] = lines.as_slice() else {
        return Err(format!("{}: the path should be one line (one subpath), not {}", caller, lines.len()).into());
    };
    Ok(polyline(points, *closed).windows(2).map(|w| (w[1] - w[0]).length()).sum())
}

/// A flattened subpath's points, back to the start if it's closed.
fn polyline(points: &[PVec], closed: bool) -> Vec<PVec> {
    let mut points = points.to_vec();
    if closed && points.len() > 1 && points.first() != points.last() {
        points.push(points[0]);
    }
    points
}

/// `path.points_along(count[, #{ start }])`: `count` points evenly spaced
/// along a path of one subpath, by length, each `#{ position, angle }`,
/// the angle the way the path runs there. On a closed path (or one that
/// ends where it starts, as circles do) they go all the way round without
/// doubling up at the start; on an open one the first and last are at its
/// ends. `start`, a fraction of the length, moves where the first is.
fn points_along(path: &Path, count: INT, options: &Map) -> RhaiResult<Array> {
    const CALLER: &str = "points_along";
    check_keys(CALLER, options, &["start"])?;
    if !(0..=1_000_000).contains(&count) {
        return Err(format!("{}: count should be from 0 to 1000000, got {}", CALLER, count).into());
    }
    let start = number(CALLER, options, "start")?.unwrap_or(0.0);
    if !(0.0..=1.0).contains(&start) {
        return Err(format!("{}: 'start' is a fraction of the length, from 0 to 1, got {}", CALLER, start).into());
    }
    let lines = path.borrow().flattened(0.01);
    let [(points, closed)] = lines.as_slice() else {
        return Err(format!("{}: the path should be one line (one subpath), not {}", CALLER, lines.len()).into());
    };
    let mut points = polyline(points, *closed);
    // A path that comes back to its start, as circles do (drawn on a little
    // past it), goes round once.
    let back = (2..points.len()).find(|&k| (points[k] - points[0]).length() < 1e-3);
    if let Some(k) = back {
        points.truncate(k + 1);
    }
    let ring = back.is_some();
    let mut at = vec![0.0f32];
    for w in points.windows(2) {
        at.push(at.last().unwrap() + (w[1] - w[0]).length());
    }
    let length = *at.last().unwrap();
    if !(length > 0.0) {
        return Err(format!("{}: the path has no length", CALLER).into());
    }
    let count = count as usize;
    let fraction = |k: usize| -> f32 {
        if ring {
            (start + k as f32 / count as f32).fract()
        } else if count == 1 {
            start
        } else {
            start + (1.0 - start) * k as f32 / (count - 1) as f32
        }
    };
    // The point `d` along, wrapping round a ring.
    let point_at = |d: f32| -> PVec {
        let d = if ring { d.rem_euclid(length) } else { d.clamp(0.0, length) };
        let i = at.partition_point(|&a| a <= d).clamp(1, points.len() - 1);
        let (a, b) = (points[i - 1], points[i]);
        let t = if at[i] > at[i - 1] { (d - at[i - 1]) / (at[i] - at[i - 1]) } else { 0.0 };
        a + (b - a) * t.clamp(0.0, 1.0)
    };
    // The way the path runs, from just before to just after.
    let step = length * 1e-4;
    let mut out = Array::new();
    for k in 0..count {
        let d = fraction(k) * length;
        let p = point_at(d);
        let dir = point_at(d + step) - point_at(d - step);
        let mut map = Map::new();
        map.insert("position".into(), Dynamic::from(Vec2::new(p.x as f64, p.y as f64)));
        map.insert("angle".into(), Dynamic::from(dir.y.atan2(dir.x) as f64));
        out.push(Dynamic::from_map(map));
    }
    Ok(out)
}

/// The `angle` option of the shape constructors, by default `default`.
fn shape_angle(caller: &str, options: &Map, default: f32) -> RhaiResult<f32> {
    match number(caller, options, "angle")? {
        None => Ok(default),
        Some(a) if a.is_finite() => Ok(a),
        Some(a) => Err(format!("{}: 'angle' must be a finite number of radians, got {}", caller, a).into()),
    }
}

/// Points round `center`: `count` of them from `angle`, each at the radius
/// `radius(k)` gives.
fn round(center: PVec, count: usize, angle: f32, radius: impl Fn(usize) -> f32, step: f32) -> Vec<PVec> {
    (0..count).map(|k| {
        let a = angle + k as f32 * step;
        center + PVec::new(a.cos(), a.sin()) * radius(k)
    }).collect()
}

/// A closed path through points.
fn closed_polyline(points: &[PVec]) -> StrokePath {
    let mut path = StrokePath::new();
    for (k, p) in points.iter().enumerate() {
        path = if k == 0 { path.move_to(p.x, p.y) } else { path.line_to(p.x, p.y) };
    }
    path.close_path()
}

fn vecs(caller: &str, points: &Array) -> RhaiResult<Vec<PVec>> {
    points.iter().enumerate().map(|(i, p)| {
        p.clone().try_cast::<Vec2>().map(PVec::from)
            .ok_or_else(|| format!("{}: point {} is a {}, not a vec2", caller, i, p.type_name()).into())
    }).collect()
}

/// `new_polyline(points[, #{ closed }])`: straight lines through the points.
fn new_polyline(points: &Array, options: &Map) -> RhaiResult<Path> {
    const CALLER: &str = "new_polyline";
    check_keys(CALLER, options, &["closed"])?;
    let closed = match options.get("closed") {
        None => false,
        Some(c) => c.as_bool().map_err(|_| format!("{}: 'closed' should be true or false, got {}", CALLER, c))?,
    };
    let points = vecs(CALLER, points)?;
    if closed {
        return Ok(Path::new(closed_polyline(&points)));
    }
    let mut path = StrokePath::new();
    for (k, p) in points.iter().enumerate() {
        path = if k == 0 { path.move_to(p.x, p.y) } else { path.line_to(p.x, p.y) };
    }
    Ok(Path::new(path))
}

/// `new_polygon(center, radius, sides[, #{ angle, step }])`: a regular
/// polygon, its first corner at `angle` (default up the page); with
/// `step`, each corner joined to the `step`-th on, so 5 sides and step 2
/// is a five-pointed star in one line.
fn new_polygon(center: Vec2, radius: &Dynamic, sides: INT, options: &Map) -> RhaiResult<Path> {
    const CALLER: &str = "new_polygon";
    check_keys(CALLER, options, &["angle", "step"])?;
    let radius = float(CALLER, "the radius", radius)? as f32;
    if !(3..=10_000).contains(&sides) {
        return Err(format!("{}: sides should be from 3 to 10000, got {}", CALLER, sides).into());
    }
    let step = match options.get("step") {
        None => 1,
        Some(s) => s.as_int().ok().filter(|s| (1..sides).contains(s))
            .ok_or_else(|| format!("{}: 'step' should be a whole number from 1 to {}, got {}", CALLER, sides - 1, s))?,
    };
    let angle = shape_angle(CALLER, options, -std::f32::consts::FRAC_PI_2)?;
    let corners = round(center.into(), sides as usize, angle, |_| radius, std::f32::consts::TAU / sides as f32);
    // Each corner to the step-th on, round every cycle of them.
    let mut path = StrokePath::new();
    let n = sides as usize;
    let mut seen = vec![false; n];
    for first in 0..n {
        if seen[first] { continue; }
        let mut cycle = Vec::new();
        let mut k = first;
        while !seen[k] {
            seen[k] = true;
            cycle.push(corners[k]);
            k = (k + step as usize) % n;
        }
        path.append(closed_polyline(&cycle));
    }
    Ok(Path::new(path))
}

/// `new_star(center, outer, inner, points[, #{ angle }])`: a star's
/// outline, its points `outer` from the centre and the corners between
/// them `inner`, its first point at `angle` (default up the page).
fn new_star(center: Vec2, outer: &Dynamic, inner: &Dynamic, points: INT, options: &Map) -> RhaiResult<Path> {
    const CALLER: &str = "new_star";
    check_keys(CALLER, options, &["angle"])?;
    let (outer, inner) = (float(CALLER, "the outer radius", outer)? as f32, float(CALLER, "the inner radius", inner)? as f32);
    if !(2..=10_000).contains(&points) {
        return Err(format!("{}: points should be from 2 to 10000, got {}", CALLER, points).into());
    }
    let angle = shape_angle(CALLER, options, -std::f32::consts::FRAC_PI_2)?;
    let n = points as usize * 2;
    let corners = round(center.into(), n, angle, |k| if k % 2 == 0 { outer } else { inner }, std::f32::consts::TAU / n as f32);
    Ok(Path::new(closed_polyline(&corners)))
}

/// `new_ellipse(center, rx, ry[, #{ angle }])`: an ellipse, its x radius
/// turned to `angle` (default 0).
fn new_ellipse(center: Vec2, rx: &Dynamic, ry: &Dynamic, options: &Map) -> RhaiResult<Path> {
    const CALLER: &str = "new_ellipse";
    check_keys(CALLER, options, &["angle"])?;
    let (rx, ry) = (float(CALLER, "the x radius", rx)? as f32, float(CALLER, "the y radius", ry)? as f32);
    let angle = shape_angle(CALLER, options, 0.0)?;
    let c: PVec = center.into();
    let (sin, cos) = angle.sin_cos();
    let at = |x: f32, y: f32| c + PVec::new(x * cos - y * sin, x * sin + y * cos);
    // Four cubic quarters, each as near a quarter ellipse as cubics get.
    const K: f32 = 0.552_284_8;
    let quarters = [
        ((rx, ry * K), (rx * K, ry), (0.0, ry)),
        ((-rx * K, ry), (-rx, ry * K), (-rx, 0.0)),
        ((-rx, -ry * K), (-rx * K, -ry), (0.0, -ry)),
        ((rx * K, -ry), (rx, -ry * K), (rx, 0.0)),
    ];
    let first = at(rx, 0.0);
    let mut path = StrokePath::new().move_to(first.x, first.y);
    for (c1, c2, end) in quarters {
        let (c1, c2, end) = (at(c1.0, c1.1), at(c2.0, c2.1), at(end.0, end.1));
        path = path.cubic_curve_to(c1.x, c1.y, c2.x, c2.y, end.x, end.y);
    }
    Ok(Path::new(path.close_path()))
}

/// `new_wave(start, end[, #{ amplitude, waves }])`: a wavy line from
/// `start` to `end`, `waves` waves (default 3) rising `amplitude` (default
/// a twentieth of its length) to its left and right: up first, going
/// left to right across the page.
fn new_wave(start: Vec2, end: Vec2, options: &Map) -> RhaiResult<Path> {
    const CALLER: &str = "new_wave";
    check_keys(CALLER, options, &["amplitude", "waves"])?;
    let (start, end): (PVec, PVec) = (start.into(), end.into());
    let along = end - start;
    let amplitude = number(CALLER, options, "amplitude")?.unwrap_or(along.length() / 20.0);
    if !amplitude.is_finite() {
        return Err(format!("{}: 'amplitude' must be a finite number, got {}", CALLER, amplitude).into());
    }
    let waves = match options.get("waves") {
        None => 3,
        Some(w) => w.as_int().ok().filter(|w| (1..=10_000).contains(w))
            .ok_or_else(|| format!("{}: 'waves' should be a whole number from 1 to 10000, got {}", CALLER, w))?,
    };
    let step = along / waves as f32;
    // A cubic's controls `c` either side of its chord bend it at most
    // c * sqrt(3) / 6 from it: controls that far out peak at `amplitude`.
    let left = PVec::new(along.y, -along.x).normalize_or_zero() * amplitude * 2.0 * 3f32.sqrt();
    let mut path = StrokePath::new().move_to(start.x, start.y);
    for i in 0..waves {
        let a = start + step * i as f32;
        let (c1, c2, e) = (a + step / 3.0 + left, a + step * 2.0 / 3.0 - left, a + step);
        path = path.cubic_curve_to(c1.x, c1.y, c2.x, c2.y, e.x, e.y);
    }
    Ok(Path::new(path))
}

/// Apply a by-value StrokePath builder method to a path in place, returning
/// the same path.
fn update_path(path: &mut Path, f: impl FnOnce(StrokePath) -> StrokePath) -> Path {
    path.update(|p| *p = f(std::mem::take(p)))
}

pub(super) fn register(engine: &mut Engine, text: TextSource) {
    let rng = text.1.clone();
    let r = rng.clone();
    engine.register_fn("hatch", move |path: &mut Path, spacing: Dynamic| -> RhaiResult<Path> {
        hatch(&path.borrow(), &hatch_options(spacing, Map::new())?, &r).map(Path::new)
    });
    engine.register_fn("hatch", move |path: &mut Path, spacing: Dynamic, options: Map| -> RhaiResult<Path> {
        hatch(&path.borrow(), &hatch_options(spacing, options)?, &rng).map(Path::new)
    });

    let (r1, r2) = (text.1.clone(), text.1.clone());
    let (t1, t2, t3) = (text.clone(), text.clone(), text);
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
        .register_fn("fill_with_text", move |path: &mut Path, text: &str, settings: Map| fill_with_text(path, text, settings, &t1))
        .register_fn("fit_text", move |path: &mut Path, text: &str, settings: Map| fit_text(path, text, settings, &t2))
        .register_fn("fill_with_text", move |path: &mut Path, bt: BalancedTernary, settings: Map| fill_with_ternary(path, &bt, settings, &r1))
        .register_fn("fit_text", move |path: &mut Path, bt: BalancedTernary, settings: Map| fit_ternary(path, &bt, settings, &r2))
        .register_fn("points_along", |path: &mut Path, count: INT| points_along(path, count, &Map::new()))
        .register_fn("points_along", |path: &mut Path, count: INT, options: Map| points_along(path, count, &options))
        .register_get("is_empty", |path: &mut Path| path.borrow().to_substrokes().is_empty())
        // `path.subpaths()`: each subpath as a path of its own.
        .register_fn("subpaths", |path: &mut Path| -> Array {
            path.borrow().subpaths().into_iter().map(|p| Dynamic::from(Path::new(p))).collect()
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
        .register_fn("erase_inside", |path: &mut Path, shape: Path| -> RhaiResult<Path> { erase(path, &shape, &Map::new(), Keep::Outside) })
        .register_fn("erase_inside", |path: &mut Path, shape: Path, options: Map| -> RhaiResult<Path> { erase(path, &shape, &options, Keep::Outside) })
        .register_fn("erase_outside", |path: &mut Path, shape: Path| -> RhaiResult<Path> { erase(path, &shape, &Map::new(), Keep::Inside) })
        .register_fn("erase_outside", |path: &mut Path, shape: Path, options: Map| -> RhaiResult<Path> { erase(path, &shape, &options, Keep::Inside) })
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
        })
        .register_fn("new_polyline", |points: Array| new_polyline(&points, &Map::new()))
        .register_fn("new_polyline", |points: Array, options: Map| new_polyline(&points, &options))
        .register_fn("new_polygon", |center: Vec2, radius: Dynamic, sides: INT| new_polygon(center, &radius, sides, &Map::new()))
        .register_fn("new_polygon", |center: Vec2, radius: Dynamic, sides: INT, options: Map| new_polygon(center, &radius, sides, &options))
        .register_fn("new_star", |center: Vec2, outer: Dynamic, inner: Dynamic, points: INT| new_star(center, &outer, &inner, points, &Map::new()))
        .register_fn("new_star", |center: Vec2, outer: Dynamic, inner: Dynamic, points: INT, options: Map| new_star(center, &outer, &inner, points, &options))
        .register_fn("new_ellipse", |center: Vec2, rx: Dynamic, ry: Dynamic| new_ellipse(center, &rx, &ry, &Map::new()))
        .register_fn("new_ellipse", |center: Vec2, rx: Dynamic, ry: Dynamic, options: Map| new_ellipse(center, &rx, &ry, &options))
        .register_fn("new_wave", |start: Vec2, end: Vec2| new_wave(start, end, &Map::new()))
        .register_fn("new_wave", |start: Vec2, end: Vec2, options: Map| new_wave(start, end, &options));
    let t = t3;
    engine.register_fn("write_line", move |text: &str, at: Vec2, settings: Map| write_line(text, at, settings, &t));
}

const HATCH_KEYS: [&str; 5] = ["angle", "roughness", "bow", "inset", "fill_rule"];

/// `path.hatch(spacing[, #{ angle, roughness, bow, inset, fill_rule }])`;
/// see the module docs.
fn hatch_options(spacing: Dynamic, options: Map) -> RhaiResult<HatchOptions> {
    const CALLER: &str = "hatch";
    check_keys(CALLER, &options, &HATCH_KEYS)?;
    let spacing = float(CALLER, "the spacing", &spacing)? as f32;
    if !(spacing > 0.0) {
        return Err(format!("{}: the spacing must be positive, got {}", CALLER, spacing).into());
    }
    let options = &options;
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
    let fill_rule = fill_rule(CALLER, options)?;
    Ok(HatchOptions {
        spacing,
        angles,
        roughness: at_least_zero("roughness")?,
        bow: at_least_zero("bow")?,
        inset: at_least_zero("inset")?,
        fill_rule,
    })
}

/// The `fill_rule` option, by default "holes".
fn fill_rule(caller: &str, options: &Map) -> RhaiResult<FillRule> {
    match options.get("fill_rule") {
        None => Ok(FillRule::default()),
        Some(rule) => {
            let name = rule.to_string();
            FillRule::from_name(&name).ok_or_else(|| -> Box<EvalAltResult> {
                format!("{}: unknown fill_rule '{}'; expected one of {}", caller, name, FillRule::NAMES.join(", ")).into()
            })
        }
    }
}

/// `path.erase_inside(shape[, #{ fill_rule }])` and `erase_outside`: the
/// path cut to the parts on the `keep` side of the shape, in place.
fn erase(path: &mut Path, shape: &Path, options: &Map, keep: Keep) -> RhaiResult<Path> {
    let caller = if keep == Keep::Outside { "erase_inside" } else { "erase_outside" };
    check_keys(caller, options, &["fill_rule"])?;
    let rule = fill_rule(caller, options)?;
    // Copy first: `p.erase_inside(p)` shares one value.
    let shape = shape.get();
    if !shape.points_within().iter().chain(path.borrow().points_within().iter()).all(|p| p.is_finite()) {
        return Err(format!("{}: the paths have a non-finite (infinite or NaN) coordinate", caller).into());
    }
    Ok(path.update(|p| *p = clip(p, &shape, rule, keep)))
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
        let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_GLYPHSETS).expect("Failed to load fonts.json");
        JournalEngine::new(Rc::new(symd))
    }

    fn eval<T: Clone + 'static>(script: &str) -> T {
        let setup = r#"let style = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());"#;
        engine().engine.eval::<T>(&format!("{}\n{}", setup, script)).expect("script should evaluate")
    }

    fn fails(script: &str) -> String {
        engine().engine.run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn subpaths_split_a_path_at_its_moves() {
        let ok: bool = eval(r#"
            let p = new_circle(vec2(0, 0), 5).append(new_line(vec2(20, 0), vec2(30, 0)));
            p.move_to(vec2(50, 50));
            let parts = p.subpaths();
            parts.len() == 2 && parts[1].bounds().width == 10.0 && parts[0].bounds().center.x.abs() < 0.01
                && new_path().subpaths().len() == 0
        "#);
        assert!(ok);
    }

    #[test]
    fn erasing_cuts_paths_by_a_shape() {
        let ok: bool = eval(r#"
            let square = new_rectangle(vec2(0, 0), 10, 10);
            let line = new_line(vec2(-20, 0), vec2(20, 0));
            let same = line.erase_inside(square);
            let outside = line.bounds();
            let inner = new_line(vec2(-20, 0), vec2(20, 0)).erase_outside(square, #{ fill_rule: "nonzero" }).bounds();
            same.bounds().width == 40.0 && outside.width == 40.0 && inner.width > 9.99 && inner.width < 10.01
                && `${line}`.contains("M 5 0")
        "#);
        assert!(ok);
        let e = engine();
        for (script, want) in [
            ("new_line(vec2(0, 0), vec2(1, 0)).erase_inside(new_circle(vec2(0, 0), 1), #{ rule: 1 })", "erase_inside: unknown option 'rule'"),
            ("new_line(vec2(0, 0), vec2(1, 0)).erase_outside(new_circle(vec2(0, 0), 1), #{ fill_rule: \"odd\" })", "erase_outside: unknown fill_rule 'odd'"),
        ] {
            let err = e.engine.eval::<rhai::Dynamic>(script).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn ternary_marks_digits_along_a_path() {
        let ok: bool = eval(r#"
            let ink = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let stem = new_line(vec2(0, 0), vec2(0, 30));
            let hooks = stem.fit_text(balanced_ternary("+0-"), #{ style: ink });
            let spaced = stem.fill_with_text(balanced_ternary("+0-+0-+"), #{ style: ink, type: #{ glyph_scale: 10 }, width: 4 });
            // Role settings it doesn't use are ignored.
            let reused = stem.fit_text(balanced_ternary(5), #{ style: ink, type: #{ glyph_scale: 9 }, paragraph: #{ line_height: 2 } });
            hooks.bounds().height > 20.0 && spaced.len() == 3 && reused.bounds().height > 0.0 && stem.bounds().width == 0.0
        "#);
        assert!(ok);
        let e = engine();
        let ink = r#"let ink = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style()); let stem = new_line(vec2(0, 0), vec2(0, 9));"#;
        for (script, want) in [
            ("stem.fit_text(balanced_ternary(5), #{ style: ink, marks: \"thorns\" })", "unknown option 'marks'"),
            ("stem.fill_with_text(balanced_ternary(5), #{ style: ink, type: #{ glyph_scale: 10 } })", "a digit 10 long doesn't fit"),
            ("stem.fit_text(balanced_ternary(5), #{ style: ink, width: 0 })", "'width' must be positive"),
            ("stem.fill_with_text(balanced_ternary(5), #{ style: ink })", "the type has no 'glyph_scale'"),
        ] {
            let script = format!("{}\n{}", ink, script);
            let script = script.as_str();
            let err = e.engine.eval::<rhai::Dynamic>(script).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn hatch_fills_inside_the_path_and_follows_the_seed() {
        let hatching = |seed: i64| -> String {
            let path: super::super::Path = eval(&format!(
                "seed({}); new_circle(vec2(0.0, 0.0), 20.0).hatch(3.0, #{{ angle: 0.7, roughness: 1.0 }})", seed));
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
                "let ring = new_circle(vec2(0.0, 0.0), 20.0); ring.append(new_circle(vec2(0.0, 0.0), 10.0)); ring.hatch({})", options));
            let hatching = path.get();
            for (points, _) in hatching.flattened(0.1) {
                let middle = (points[0] + *points.last().unwrap()) / 2.0;
                assert!(middle.length() > 9.0, "a line crosses the hole");
            }
            hatching.to_substrokes().len()
        };
        let single = lines("3.0, #{ angle: 0.3 }");
        assert_eq!(lines("3.0, #{ angle: [0.3, 1.9] }"), 2 * single);
        assert_eq!(lines("3.0, #{ angle: 0.3, roughness: 0.0 }"), single);
    }

    #[test]
    fn bad_hatch_options_fail_at_their_line() {
        let err = fails("let ok = 1;\nnew_circle(vec2(0.0, 0.0), 5.0).hatch(0.0, #{ angle: 0.0, roughness: 1.0 });");
        assert!(err.contains("the spacing must be positive") && err.contains("line 2"), "{}", err);
        for (opts, want) in [
            ("#{ angle: 1.0 }", "the spacing should be a number"),
            ("2, #{ angle: [] }", "'angle' needs at least one angle"),
            ("2, #{ angle: [1.0, \"x\"] }", "each 'angle' should be a number"),
            ("2, #{ bow: -1 }", "'bow' can't be negative"),
            ("2, #{ fill_rule: \"winding\" }", "unknown fill_rule 'winding'"),
            ("2, #{ spacing: 2 }", "unknown option 'spacing'"),
        ] {
            let err = fails(&format!("let ok = 1;\nnew_circle(vec2(0.0, 0.0), 5.0).hatch({});", opts));
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }
}
