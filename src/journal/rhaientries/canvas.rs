//! Canvases for rhai scripts: strokes drawn with styles, added together
//! and moved as one.

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2 as PVec;
use rand::RngCore;
use rhai::{Dynamic, Engine, EvalAltResult, Map, NativeCallContext};
use super::RhaiResult;

use super::options::{apply_props, float};
use super::random::SharedRng;
use super::{check_finite, check_name, Canvas, Path, Style, Vec2};
use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::canvas::Frame;
use crate::journal::paths::StrokePath;

#[derive(Debug, Default, Clone)]
pub struct CanvasData {
    pub strokes: Vec<Stroke>,
    /// Its output files are named after this, or its index on the page.
    pub(super) name: Option<String>,
    /// For a tile of a split page, the part of the page it renders.
    pub(super) frame: Option<Frame>,
}

impl CanvasData {
    /// The canvas's name in output files, given its index on its page.
    pub fn label(&self, index: usize) -> String {
        self.name.clone().unwrap_or_else(|| index.to_string())
    }
    /// Add a path drawn in `style`, gone over as its handwriting says,
    /// its jitter drawn from `rng`.
    pub fn add_path(&mut self, path: StrokePath, style: &Style, rng: &mut dyn RngCore) {
        self.strokes.push(Stroke { path: style.retrace(path, rng), style: style.clone(), seed: None });
    }
    fn transform(&mut self, f: impl Fn(&mut StrokePath)) {
        for stroke in &mut self.strokes {
            f(&mut stroke.path);
        }
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
            let strokes = other.borrow().strokes.clone();
            c.update(|data| data.strokes.extend(strokes))
        })
        .register_fn("add", |c: &mut Canvas, canvases: rhai::Array| -> RhaiResult<Canvas> {
            // An array of canvases, as fill_with_text gives.
            let mut strokes = Vec::new();
            for (i, item) in canvases.into_iter().enumerate() {
                let type_name = item.type_name();
                let other = item.try_cast::<Canvas>()
                    .ok_or_else(|| format!("canvas.add: item {} is a {}, not a canvas (glyphs need a style: canvas.add(glyphs, style))", i, type_name))?;
                strokes.extend(other.borrow().strokes.iter().cloned());
            }
            Ok(c.update(|data| data.strokes.extend(strokes)))
        })
        .register_fn("add", move |c: &mut Canvas, parts: Map, styles: Map| -> RhaiResult<Canvas> { add_parts(c, parts, styles, &mut *rng.borrow_mut()) })
        .register_fn("translate_by", |c: &mut Canvas, v: Vec2| -> Canvas {
            let v = v.into();
            c.update(|data| data.transform(|p| p.translate_by(v)))
        })
        .register_fn("scale_by", |c: &mut Canvas, scale: Dynamic| -> RhaiResult<Canvas> {
            let scale = float("scale_by", "the scale", &scale)? as f32;
            Ok(c.update(|data| data.transform(|p| p.scale_by(scale))))
        })
        .register_fn("rotate_around", |c: &mut Canvas, point: Vec2, rad: Dynamic| -> RhaiResult<Canvas> {
            let (point, rad) = (point.into(), float("rotate_around", "the angle", &rad)? as f32);
            Ok(c.update(|data| data.transform(|p| p.rotate_around(point, rad))))
        });
}

impl From<CanvasData> for JournalCanvas {
    fn from(val: CanvasData) -> Self {
        let mut canvas = JournalCanvas::new();
        for Stroke { path, style, seed } in val.strokes {
            canvas.add_seeded_stroke(path, (&style).into(), seed);
        }
        canvas.set_frame(val.frame);
        canvas
    }
}

impl From<&CanvasData> for JournalCanvas {
    fn from(val: &CanvasData) -> Self {
        let mut canvas = JournalCanvas::new();
        for Stroke { path, style, seed } in val.strokes.iter() {
            canvas.add_seeded_stroke(path.clone(), style.into(), *seed);
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

fn vec(v: PVec) -> Dynamic {
    Dynamic::from(Vec2::new(v.x as f64, v.y as f64))
}

/// `canvas.bounds()`: what its strokes cover, their widths included.
fn bounds(canvas: &mut Canvas) -> RhaiResult<Map> {
    let jc: JournalCanvas = (&*canvas).into();
    bounds_map(jc.bounds()).ok_or_else(|| "bounds: the canvas is empty".into())
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
}
