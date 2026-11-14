//! Canvases for rhai scripts: strokes drawn with styles, added together
//! and moved as one.

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2 as PVec;
use rand::RngCore;
use rhai::{Dynamic, Engine, EvalAltResult, Map, NativeCallContext};
use super::RhaiResult;

use super::options::{apply_props, at_least_zero, check_keys, float};
use super::random::SharedRng;
use super::vec::scale_factors;
use super::symbolic::canvas_of;
use super::{check_finite, check_name, Canvas, Page, Path, Style, Vec2};
use std::collections::HashMap;

use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::canvas::{ColoredStroke, InkRun};
use super::edits::CommitMark;
use std::sync::Arc;
use crate::journal::canvas::Frame;
use crate::journal::paths::StrokePath;
use crate::journal::symbol::Symbol;

#[derive(Debug, Default, Clone)]
pub struct CanvasData {
    pub strokes: Vec<Stroke>,
    /// Its output files are named after this, or its index on the page.
    pub(super) name: Option<String>,
    /// For a tile of a split page, the part of the page it renders.
    pub(super) frame: Option<Frame>,
    /// Where the canvases committed to the visual editor that it holds
    /// are (see edits.rs).
    pub(super) commits: Vec<CommitMark>,
}

impl CanvasData {
    /// The canvas's name in output files, given its index on its page.
    pub fn label(&self, index: usize) -> String {
        self.name.clone().unwrap_or_else(|| index.to_string())
    }
    /// Add a path drawn in `style`, gone over as its handwriting says,
    /// its jitter drawn from `rng`.
    pub fn add_path(&mut self, path: StrokePath, style: &Style, rng: &mut dyn RngCore) {
        self.add_text_path(path, TextRun::default(), style, rng);
    }
    /// Add a path that writes `text`, as `add_path` does.
    pub fn add_text_path(&mut self, path: StrokePath, text: TextRun, style: &Style, rng: &mut dyn RngCore) {
        let path = style.retrace(path, rng);
        // Only a jittered colour draws its seed, so other strokes leave the
        // generator as it was.
        let color_seed = if style.color_jitter() > 0.0 { rng.next_u64() } else { 0 };
        self.strokes.push(Stroke { path, style: style.clone(), seed: None, color_seed, text, hidden: false, commits: None });
    }
    /// `f` applied to every stroke's path (and the committed canvases' marks).
    pub(super) fn transform_with(&mut self, f: impl Fn(&mut StrokePath)) {
        self.transform(f)
    }
    fn transform(&mut self, f: impl Fn(&mut StrokePath)) {
        for stroke in &mut self.strokes {
            f(&mut stroke.path);
        }
        for mark in &mut self.commits {
            mark.transform(&f);
        }
    }
    /// Whether the visual editor hid any of its strokes.
    pub(super) fn has_hidden(&self) -> bool {
        self.strokes.iter().any(|s| s.hidden)
    }
}

/// The canvas functions scripts call. Like the path functions, transforms
/// change the canvas and return it, so they chain.
pub(super) fn register(engine: &mut Engine, rng: SharedRng) {
    engine.register_fn("bounds", bounds);
    let r = rng.clone();
    engine.register_type_with_name::<Canvas>("Canvas")
        .register_fn("new_canvas", || Canvas::new(CanvasData::default()))
        .register_fn("new_canvas", |ctx: NativeCallContext, options: Map| apply_props(&ctx, "new_canvas", Canvas::new(CanvasData::default()), options, &["name"]))
        .register_fn("copy", Canvas::copy)
        .register_get_set("name",
            |c: &mut Canvas| c.borrow().name.clone().map_or(Dynamic::UNIT, Dynamic::from),
            |c: &mut Canvas, name: Dynamic| -> Result<(), Box<EvalAltResult>> {
                c.borrow_mut().name = Some(check_name("canvas.name", name)?);
                Ok(())
            })
        // canvas.add(...) copies what it adds, so later changes to a path
        // don't reach the canvas, and returns the canvas. Paths with
        // non-finite points are refused (see `check_finite`). Glyphs are
        // added in glyphs.rs.
        .register_fn("add", move |c: &mut Canvas, path: Path, style: Style| -> RhaiResult<Canvas> {
            let path = path.get();
            check_finite("canvas.add", &path)?;
            Ok(c.update(|data| data.add_path(path, &style, &mut *r.borrow_mut())))
        })
        .register_fn("add", |c: &mut Canvas, other: Canvas| -> Canvas {
            // Copy first: `c.add(c)` shares one value.
            let (strokes, commits) = { let other = other.borrow(); (other.strokes.clone(), other.commits.clone()) };
            c.update(|data| {
                data.strokes.extend(strokes);
                data.commits.extend(commits);
            })
        })
        .register_fn("add", |c: &mut Canvas, canvases: rhai::Array| -> RhaiResult<Canvas> {
            // An array of canvases, as fill_with_text gives.
            let (mut strokes, mut commits) = (Vec::new(), Vec::new());
            for (i, item) in canvases.into_iter().enumerate() {
                let type_name = item.type_name();
                let other = super::symbolic::canvas_of(&item)
                    .ok_or_else(|| format!("canvas.add: item {} is a {}, not a canvas (glyphs need a style: canvas.add(glyphs, style))", i, type_name))?;
                strokes.extend(other.borrow().strokes.iter().cloned());
                commits.extend(other.borrow().commits.iter().cloned());
            }
            Ok(c.update(|data| {
                data.strokes.extend(strokes);
                data.commits.extend(commits);
            }))
        })
        .register_fn("add", move |c: &mut Canvas, parts: Map, styles: Map| -> RhaiResult<Canvas> { add_parts(c, parts, styles, &mut *rng.borrow_mut()) })
        .register_fn("translate_by", |c: &mut Canvas, v: Vec2| -> Canvas {
            let v = v.into();
            c.update(|data| data.transform(|p| p.translate_by(v)))
        })
        .register_fn("scale_by", |c: &mut Canvas, scale: Dynamic| -> RhaiResult<Canvas> {
            let scale = scale_factors("canvas.scale_by", scale)?;
            Ok(c.update(|data| data.transform(|p| p.scale_by(scale))))
        })
        .register_fn("size_to", |c: &mut Canvas, target: Dynamic| size_to(c, target, &Map::new()))
        .register_fn("size_to", |c: &mut Canvas, target: Dynamic, options: Map| size_to(c, target, &options))
        .register_fn("rotate_around", |c: &mut Canvas, point: Vec2, rad: Dynamic| -> RhaiResult<Canvas> {
            let (point, rad) = (point.into(), float("rotate_around", "the angle", &rad)? as f32);
            Ok(c.update(|data| data.transform(|p| p.rotate_around(point, rad))))
        });
}

/// How dry each stroke's pen is as it's drawn (see `Style::ink_run`): the
/// strokes in order, each pen (brush and colour) from a fresh dip.
fn ink_runs(strokes: &[Stroke]) -> Vec<Option<InkRun>> {
    let mut used: HashMap<String, f32> = HashMap::new();
    strokes.iter().map(|stroke| {
        let pen = stroke.style.pen()?;
        stroke.style.ink_run(used.entry(pen).or_default(), stroke.text.glyphs, stroke.text.mid_word, &stroke.path)
    }).collect()
}

impl From<CanvasData> for JournalCanvas {
    fn from(val: CanvasData) -> Self {
        let mut canvas = JournalCanvas::new();
        let inks = ink_runs(&val.strokes);
        for (Stroke { path, style, seed, color_seed, commits, .. }, ink) in val.strokes.into_iter().zip(inks) {
            canvas.add_colored_stroke(ColoredStroke { path, style: style.stroke_style(color_seed), seed, ink, commits });
        }
        canvas.set_frame(val.frame);
        canvas
    }
}

impl From<&CanvasData> for JournalCanvas {
    fn from(val: &CanvasData) -> Self {
        let mut canvas = JournalCanvas::new();
        for (Stroke { path, style, seed, color_seed, commits, .. }, ink) in val.strokes.iter().zip(ink_runs(&val.strokes)) {
            canvas.add_colored_stroke(ColoredStroke { path: path.clone(), style: style.stroke_style(*color_seed), seed: *seed, ink, commits: commits.clone() });
        }
        canvas.set_frame(val.frame);
        canvas
    }
}

impl From<&Canvas> for JournalCanvas {
    fn from(val: &Canvas) -> Self {
        (&*val.borrow()).into()
    }
}

/// A path on a canvas, as its style's hand went over it, with the style
/// it's drawn in.
#[derive(Debug, Clone)]
pub struct Stroke {
    pub(super) path: StrokePath,
    pub(super) style: Style,
    /// Its texture's seed, kept from the whole page in a tile (see
    /// `ColoredStroke::seed`).
    pub(super) seed: Option<u32>,
    /// Where its colour jitter's drawn from, if its style has any.
    pub(super) color_seed: u64,
    /// What text it writes, for its pen to run dry by.
    pub(super) text: TextRun,
    /// Whether the visual editor hid it: it's still there, in its canvas's
    /// bounds, but not drawn.
    pub(super) hidden: bool,
    /// The canvases committed to the visual editor it was drawn on (see
    /// `ColoredStroke::commits`).
    pub(super) commits: Option<Arc<str>>,
}

/// What a stroke writes: how many glyphs (none, for a path that isn't
/// text), and whether it goes on writing a word, as a glyph after the
/// first of a word drawn on its own does.
#[derive(Debug, Default, Clone, Copy)]
pub struct TextRun {
    pub(super) glyphs: u32,
    pub(super) mid_word: bool,
}

impl TextRun {
    /// What `glyphs[span]` write: those that aren't blank, going on with a
    /// word if the one before isn't blank either.
    pub(super) fn of(glyphs: &[Symbol], span: std::ops::Range<usize>) -> TextRun {
        let mid_word = span.start > 0 && !glyphs[span.start - 1].is_blank();
        TextRun { glyphs: glyphs[span].iter().filter(|g| !g.is_blank()).count() as u32, mid_word }
    }
}

/// `canvas.add(parts, styles)`: each path of `parts` (as `art::bramble` or
/// `art::moon` give them) in the style `styles` has under its name, or
/// not at all without one; every style must name a part. In the parts'
/// order (rhai maps are sorted), so the drawing is the same every run.
fn add_parts(canvas: &mut Canvas, parts: Map, styles: Map, rng: &mut dyn RngCore) -> RhaiResult<Canvas> {
    const CALLER: &str = "canvas.add";
    if let Some(name) = styles.keys().find(|k| !parts.contains_key(*k)) {
        let names: Vec<&str> = parts.keys().map(|k| k.as_str()).collect();
        return Err(format!("{}: there's no part '{}' to style; parts: {}", CALLER, name, names.join(", ")).into());
    }
    let mut added = Vec::new();
    for (name, part) in &parts {
        let Some(style) = styles.get(name) else { continue };
        let style = style.clone().try_cast::<Style>()
            .ok_or_else(|| format!("{}: the style for '{}' should be a style", CALLER, name))?;
        let type_name = part.type_name();
        let path = part.clone().try_cast::<Path>()
            .ok_or_else(|| format!("{}: part '{}' is a {}, not a path", CALLER, name, type_name))?.get();
        check_finite(CALLER, &path)?;
        added.push((path, style));
    }
    Ok(canvas.update(|data| for (path, style) in added { data.add_path(path, &style, rng) }))
}

/// What `canvas.size_to` takes.
const SIZE_TO_OPTIONS: [&str; 2] = ["padding", "keep_proportions"];

/// What `canvas.size_to` was asked: the box to fill, the room to leave
/// inside its edges, and whether to keep the canvas's proportions.
#[derive(Clone, Copy, Debug)]
pub(super) struct Fit {
    target: Aabb,
    padding: f32,
    keep_proportions: bool,
}

const SIZE_TO: &str = "canvas.size_to";

/// `canvas.size_to(target, options)`'s arguments: the target's box (a
/// page's size from the origin, or another canvas's bounds, a symbolic's
/// included) and the options.
pub(super) fn fit_of(target: &Dynamic, options: &Map) -> RhaiResult<Fit> {
    check_keys(SIZE_TO, options, &SIZE_TO_OPTIONS)?;
    let padding = match options.get("padding") {
        None => 0.0,
        Some(padding) => at_least_zero(SIZE_TO, "padding", padding)? as f32,
    };
    let keep_proportions = match options.get("keep_proportions") {
        None => false,
        Some(keep) => keep.as_bool()
            .map_err(|type_name| format!("{}: 'keep_proportions' should be true or false, got {}", SIZE_TO, type_name))?,
    };
    let target = if let Some(page) = target.clone().try_cast::<Page>() {
        let size = page.size.ok_or_else(|| format!("{}: the page has no size to fill; set page.size", SIZE_TO))?;
        Aabb::new(PVec::ZERO, size)
    } else if let Some(other) = canvas_of(target) {
        stroke_bounds(&other.borrow().strokes).ok_or_else(|| format!("{}: the target canvas is empty", SIZE_TO))?.1
    } else {
        return Err(format!("{}: the target should be a page or a canvas, got {}", SIZE_TO, target.type_name()).into());
    };
    Ok(Fit { target, padding, keep_proportions })
}

/// `canvas.size_to(target, options)`, the target read before the canvas is
/// changed, so a canvas can be sized to itself.
fn size_to(canvas: &mut Canvas, target: Dynamic, options: &Map) -> RhaiResult<Canvas> {
    let fit = fit_of(&target, options)?;
    canvas.borrow_mut().fit(fit)?;
    Ok(canvas.clone())
}

impl CanvasData {
    /// Scaled about the origin and moved so that what its strokes cover,
    /// their widths included (as `canvas.bounds()` gives), fills the fit's
    /// box less its padding all round: stretched across and down
    /// separately, or, keeping its proportions, as big as fits and centred.
    /// Brush sizes stay as they are, so the lines are scaled to leave room
    /// for them; lines with no extent one way (a straight line across)
    /// can't be stretched that way and are centred.
    pub(super) fn fit(&mut self, fit: Fit) -> RhaiResult<()> {
        let pad = PVec::splat(fit.padding);
        let (from, to) = (fit.target.mins + pad, fit.target.maxs - pad);
        // Which stroke's width reaches furthest past the lines can change
        // as they're scaled, so it's fitted again from where it lands until
        // it stays put (at once, when the strokes share a brush size).
        for _ in 0..4 {
            let (lines, covered) = stroke_bounds(&self.strokes).ok_or_else(|| format!("{}: the canvas is empty", SIZE_TO))?;
            // The box the lines go in: the target's, less what the widths take.
            let (from, to) = (from + (lines.mins - covered.mins), to - (covered.maxs - lines.maxs));
            let (room, extents) = (to - from, lines.extents());
            let mut scale = PVec::ONE;
            for axis in 0..2 {
                if extents[axis] > 0.0 {
                    if room[axis] <= 0.0 {
                        return Err(format!("{}: the target less the padding is too small for the strokes' widths", SIZE_TO).into());
                    }
                    scale[axis] = room[axis] / extents[axis];
                }
            }
            if fit.keep_proportions {
                let fits = (0..2).filter(|&axis| extents[axis] > 0.0).map(|axis| scale[axis]).fold(f32::INFINITY, f32::min);
                scale = PVec::splat(if fits.is_finite() { fits } else { 1.0 });
            }
            let offset = (from + to) / 2.0 - lines.center() * scale;
            self.transform(|p| {
                p.scale_by(scale);
                p.translate_by(offset);
            });
            if (scale - PVec::ONE).abs().max_element() < 1e-6 && offset.abs().max_element() < 1e-4 {
                break;
            }
        }
        Ok(())
    }
}

/// What the strokes' lines cover, and what they cover with their widths
/// (as `canvas.bounds()` gives it), or `None` for no strokes.
fn stroke_bounds(strokes: &[Stroke]) -> Option<(Aabb, Aabb)> {
    let (mut lines, mut covered) = (Aabb::new_invalid(), Aabb::new_invalid());
    for stroke in strokes {
        let width = PVec::splat(stroke.style.stroke_width());
        stroke.path.for_each_point(|point| {
            lines.take_point(point);
            covered.take_point(point - width);
            covered.take_point(point + width);
        });
    }
    (lines.mins.x <= lines.maxs.x).then_some((lines, covered))
}

fn vec(v: PVec) -> Dynamic {
    Dynamic::from(Vec2::new(v.x as f64, v.y as f64))
}

/// `canvas.bounds()`: what its strokes cover, their widths included.
fn bounds(canvas: &mut Canvas) -> RhaiResult<Map> {
    stroke_bounds(&canvas.borrow().strokes).and_then(|(_, covered)| bounds_map(covered))
        .ok_or_else(|| "bounds: the canvas is empty".into())
}

/// A box as scripts see it: `#{ min, max, center, width, height }`, or
/// `None` for an empty (invalid) one.
pub(super) fn bounds_map(b: Aabb) -> Option<Map> {
    if !(b.mins.x <= b.maxs.x && b.mins.y <= b.maxs.y) {
        return None;
    }
    let extents = b.extents();
    let mut map = Map::new();
    map.insert("min".into(), vec(b.mins));
    map.insert("max".into(), vec(b.maxs));
    map.insert("center".into(), vec(b.center()));
    map.insert("width".into(), Dynamic::from(extents.x as f64));
    map.insert("height".into(), Dynamic::from(extents.y as f64));
    Some(map)
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
        let setup = r#"let style = new_style(new_brush_style("black", "Ink Pen", 1.0, #{ max_pressure: 1.0 }), new_handwriting_style());"#;
        engine().engine.eval::<T>(&format!("{}\n{}", setup, script)).expect("script should evaluate")
    }

    fn fails(script: &str) -> String {
        engine().engine.run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn bounds_include_stroke_width() {
        let ok: bool = eval(r#"
            let c = new_canvas();
            c.add(new_line(vec2(0.0, 0.0), vec2(10.0, 20.0)), style);
            let b = c.bounds();
            b.min.x == -1.0 && b.min.y == -1.0 && b.max.x == 11.0 && b.max.y == 21.0
                && b.center.x == 5.0 && b.center.y == 10.0 && b.width == 12.0 && b.height == 22.0
        "#);
        assert!(ok);
        // A path's bounds are its line's, curves included.
        let ok: bool = eval(r#"
            let b = new_path().move_to(vec2(0, 0)).quad_to(vec2(10, 20), vec2(20, 0)).bounds();
            b.min == vec2(0, 0) && b.max.x == 20.0 && (b.max.y - 10.0).abs() < 0.01
        "#);
        assert!(ok);
        assert!(fails("new_canvas().bounds()").contains("bounds: the canvas is empty"));
    }

    /// Whether a canvas's bounds are `min`..`max`, near enough.
    const NEAR: &str = r#"
        fn near(b, min, max) { (b.min - min).length() < 0.001 && (b.max - max).length() < 0.001 }
    "#;

    #[test]
    fn scale_by_takes_a_vec() {
        let ok: bool = eval(r#"
            let p = new_line(vec2(1, 1), vec2(2, 3)).scale_by(vec2(2, -3));
            let c = new_canvas().add(new_line(vec2(1, 1), vec2(2, 3)), style).scale_by(vec2(3, 2));
            let b = c.bounds();
            p.bounds().min == vec2(2, -9) && p.bounds().max == vec2(4, -3)
                && b.min == vec2(2, 1) && b.max == vec2(7, 7)
        "#);
        assert!(ok);
        let err = fails("new_line(vec2(0, 0), vec2(1, 1)).scale_by(2)");
        assert!(err.contains("path.scale_by: the scale should be a vec, vec2(across, down): vec2(2, 2) for 2 both ways"), "{}", err);
        let err = fails("new_canvas().scale_by(0.5)");
        assert!(err.contains("canvas.scale_by: the scale should be a vec, vec2(across, down): vec2(0.5, 0.5) for 0.5 both ways"), "{}", err);
        assert!(fails(r#"new_canvas().scale_by("big")"#).contains("canvas.scale_by: the scale should be a vec, vec2(across, down), got string"));
    }

    #[test]
    fn size_to_fills_a_page_or_a_canvas() {
        // Stretched: what the strokes cover, widths and all, fills the
        // page less the padding.
        let ok: bool = eval(&format!("{}{}", NEAR, r#"
            let c = new_canvas().add(new_rectangle(vec2(-40, 7), 10, 20), style);
            let page = new_page(#{ size: vec2(200, 100) });
            c.size_to(page, #{ padding: 10 });
            let filled = near(c.bounds(), vec2(10, 10), vec2(190, 90));
            c.size_to(page);
            filled && near(c.bounds(), vec2(0, 0), vec2(200, 100))
        "#));
        assert!(ok);
        // Its proportions kept: as big as fits, centred. The lines go
        // from 10 by 20 to 39 by 78, a width either side.
        let ok: bool = eval(&format!("{}{}", NEAR, r#"
            let c = new_canvas().add(new_rectangle(vec2(0, 0), 10, 20), style)
                .size_to(new_page(#{ size: vec2(200, 100) }), #{ padding: 10, keep_proportions: true });
            near(c.bounds(), vec2(79.5, 10), vec2(120.5, 90))
        "#));
        assert!(ok);
        // Another canvas's bounds, its widths included.
        let ok: bool = eval(&format!("{}{}", NEAR, r#"
            let frame = new_canvas().add(new_rectangle(vec2(50, 50), 100, 60), style);
            let c = new_canvas().add(new_circle(vec2(0, 0), 3), style).size_to(frame, #{ padding: 5 });
            near(c.bounds(), vec2(4, 24), vec2(96, 76))
        "#));
        assert!(ok);
    }

    #[test]
    fn size_to_fits_strokes_of_different_widths() {
        // The thick stroke's width reaches past the thin one's, and which
        // reaches furthest changes as they're scaled.
        let ok: bool = eval(&format!("{}{}", NEAR, r#"
            let thick = new_style(new_brush_style("black", "Ink Pen", 8.0, #{ max_pressure: 1.0 }), new_handwriting_style());
            let c = new_canvas()
                .add(new_line(vec2(0, 0), vec2(10, 10)), style)
                .add(new_line(vec2(4, 4), vec2(6, 6)), thick)
                .size_to(new_page(#{ size: vec2(300, 200) }));
            near(c.bounds(), vec2(0, 0), vec2(300, 200))
        "#));
        assert!(ok);
    }

    #[test]
    fn size_to_centres_lines_it_cant_stretch() {
        // A line straight across has no height to stretch: it's stretched
        // across and centred down.
        let ok: bool = eval(&format!("{}{}", NEAR, r#"
            let c = new_canvas().add(new_line(vec2(0, 5), vec2(10, 5)), style)
                .size_to(new_page(#{ size: vec2(100, 50) }), #{ padding: 4 });
            let across = near(c.bounds(), vec2(4, 24), vec2(96, 26));
            let kept = new_canvas().add(new_line(vec2(0, 5), vec2(10, 5)), style)
                .size_to(new_page(#{ size: vec2(100, 50) }), #{ padding: 4, keep_proportions: true });
            across && near(kept.bounds(), vec2(4, 24), vec2(96, 26))
        "#));
        assert!(ok);
    }

    #[test]
    fn size_to_refuses_what_it_cant_fill() {
        let setup = r#"let style = new_style(new_brush_style("black", "Ink Pen", 1.0, #{ max_pressure: 1.0 }), new_handwriting_style());
            let c = new_canvas().add(new_line(vec2(0, 0), vec2(10, 10)), style);
            let page = new_page(#{ size: vec2(100, 100) });"#;
        for (script, want) in [
            ("c.size_to(new_page())", "canvas.size_to: the page has no size to fill; set page.size"),
            ("new_canvas().size_to(page)", "canvas.size_to: the canvas is empty"),
            ("c.size_to(new_canvas())", "canvas.size_to: the target canvas is empty"),
            ("c.size_to(vec2(10, 10))", "canvas.size_to: the target should be a page or a canvas, got"),
            ("c.size_to(page, #{ padding: 49.5 })", "canvas.size_to: the target less the padding is too small for the strokes' widths"),
            ("c.size_to(page, #{ padding: -1 })", "canvas.size_to: 'padding' can't be negative, got -1"),
            ("c.size_to(page, #{ keep_proportions: 1 })", "canvas.size_to: 'keep_proportions' should be true or false"),
            ("c.size_to(page, #{ margin: 1 })", "canvas.size_to: unknown option 'margin'; options: padding, keep_proportions"),
        ] {
            let err = fails(&format!("{}\n{}", setup, script));
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }
}
