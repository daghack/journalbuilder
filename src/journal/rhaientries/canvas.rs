//! Canvases for rhai scripts: strokes drawn with styles, added together
//! and moved as one.

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2 as PVec;
use rhai::{CustomType, Dynamic, Engine, EvalAltResult, Map, TypeBuilder};
use super::RhaiResult;

use super::options::float;
use super::{check_finite, check_name, Canvas, Path, StrokeStyle, Vec2};
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
    pub fn add_stroke(&mut self, stroke: Stroke) {
        self.strokes.push(stroke)
    }
    pub fn add_strokepath(&mut self, path: StrokePath, style: StrokeStyle) {
        self.strokes.push(Stroke { path, style, seed: None });
    }
    fn transform(&mut self, f: impl Fn(&mut StrokePath)) {
        for stroke in &mut self.strokes {
            f(&mut stroke.path);
        }
    }
}

/// The canvas functions scripts call. Like the path functions, transforms
/// change the canvas and return it, so they chain.
pub(super) fn register(engine: &mut Engine) {
    engine.build_type::<Stroke>();
    engine.register_fn("bounds", bounds);
    engine.register_type_with_name::<Canvas>("Canvas")
        .register_fn("new_canvas", || Canvas::new(CanvasData::default()))
        .register_fn("copy", Canvas::copy)
        .register_get_set("name",
            |c: &mut Canvas| c.borrow().name.clone().map_or(Dynamic::UNIT, Dynamic::from),
            |c: &mut Canvas, name: Dynamic| -> Result<(), Box<EvalAltResult>> {
                c.borrow_mut().name = Some(check_name("canvas.name", name)?);
                Ok(())
            })
        // Adding copies the stroke, so later changes to the path don't reach
        // the canvas. Paths with non-finite points are refused (see
        // `check_finite`).
        .register_fn("add_stroke", |c: &mut Canvas, stroke: Stroke| -> Result<(), Box<EvalAltResult>> {
            check_finite("add_stroke", &stroke.path)?;
            c.borrow_mut().add_stroke(stroke);
            Ok(())
        })
        .register_fn("add_strokepath", |c: &mut Canvas, path: Path, style: StrokeStyle| -> Result<(), Box<EvalAltResult>> {
            let path = path.get();
            check_finite("add_strokepath", &path)?;
            c.borrow_mut().add_strokepath(path, style);
            Ok(())
        })
        .register_fn("add_canvas", |c: &mut Canvas, other: Canvas| {
            // Copy first: `c.add_canvas(c)` shares one value.
            let strokes = other.borrow().strokes.clone();
            c.borrow_mut().strokes.extend(strokes);
        })
        .register_fn("translate", |c: &mut Canvas, v: Vec2| -> Canvas {
            let v = v.into();
            c.update(|data| data.transform(|p| p.translate_by(v)))
        })
        .register_fn("scale", |c: &mut Canvas, scale: Dynamic| -> RhaiResult<Canvas> {
            let scale = float("scale", "the scale", &scale)? as f32;
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
            canvas.add_seeded_stroke(path, style.into(), seed);
        }
        canvas.set_frame(val.frame);
        canvas
    }
}

impl From<&CanvasData> for JournalCanvas {
    fn from(val: &CanvasData) -> Self {
        let mut canvas = JournalCanvas::new();
        for Stroke { path, style, seed } in val.strokes.iter() {
            canvas.add_seeded_stroke(path.clone(), style.clone().into(), *seed);
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

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Stroke {
    #[rhai_type(skip)]
    pub(super) path: StrokePath,
    pub(super) style: StrokeStyle,
    /// Its texture's seed, kept from the whole page in a tile (see
    /// `ColoredStroke::seed`).
    #[rhai_type(skip)]
    pub(super) seed: Option<u32>,
}

impl Stroke {
    /// `new_stroke(path, style)`, with a copy of the path.
    pub fn new(path: Path, style: StrokeStyle) -> Self {
        Self {
            path: path.get(),
            style,
            seed: None,
        }
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_stroke", Self::new)
        ;
    }
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
    use crate::journal::symboldict::SymbolPathDict;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        let symd = SymbolPathDict::from_file(crate::journal::symboldict::DEFAULT_SYMBOLS).expect("Failed to load symbols.json");
        JournalEngine::new(Rc::new(symd))
    }

    fn eval<T: Clone + 'static>(script: &str) -> T {
        let setup = r#"let style = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));"#;
        engine().engine.eval::<T>(&format!("{}\n{}", setup, script)).expect("script should evaluate")
    }

    fn fails(script: &str) -> String {
        engine().engine.run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn bounds_include_stroke_width() {
        let ok: bool = eval(r#"
            let c = new_canvas();
            c.add_strokepath(new_line(new_vec(0.0, 0.0), new_vec(10.0, 20.0)), style);
            let b = c.bounds();
            b.min.x == -1.0 && b.min.y == -1.0 && b.max.x == 11.0 && b.max.y == 21.0
                && b.center.x == 5.0 && b.center.y == 10.0 && b.width == 12.0 && b.height == 22.0
        "#);
        assert!(ok);
        // A path's bounds are its line's, curves included.
        let ok: bool = eval(r#"
            let b = new_strokepath().move_to(new_vec(0, 0)).quad_to(new_vec(10, 20), new_vec(20, 0)).bounds();
            b.min == new_vec(0, 0) && b.max.x == 20.0 && (b.max.y - 10.0).abs() < 0.01
        "#);
        assert!(ok);
        assert!(fails("new_canvas().bounds()").contains("bounds: the canvas is empty"));
    }
}
