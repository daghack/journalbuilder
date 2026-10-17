//! Avoidance trees for rhai scripts: what text keeps clear of.
//!
//! ```rhai
//! let avoid = new_avoidance_tree();
//! avoid.add(border, #{ clearance: 8 });   // keep 8 units from a path's lines
//! avoid.add(drawing, #{ clearance: 4 });  // from a canvas's marks (their edges, not their middles)
//! avoid.add(glyphs, #{ clearance: 2 });   // from glyphs' outlines, insides included
//! avoid.add_region(new_circle(vec2(0.0, 0.0), 80.0));   // out of a closed shape
//! avoid.add_hull(drawing, #{ clearance: 10.0 });   // out of everything inside a drawing's outline
//! avoid.add(other_tree);            // everything another tree avoids
//! avoid.add([border, drawing], #{ clearance: 3 });   // arrays of any of these
//! avoid.blocks(vec2(10.0, 0.0)); // whether a point is too close to something
//! avoid.blocks(glyph);              // whether a placed glyph is
//! ```
//!
//! Every text layout function takes `avoid` and `clearance` options:
//!
//! ```rhai
//! new_textbox(#{ center: c, width: 400, height: 300, size: 20.0, avoid: avoid });
//! path.fill_with_text(msg, #{ font: font, size: 20.0, style: style, avoid: drawing, clearance: 6.0 });
//! path.fit_text(msg, #{ font: font, style: style, avoid: [drawing, border] });
//! canvas.scatter_notes(msgs, [200, 400], [100, 200], #{ font: font, style: style, size: 20.0, avoid: avoid });
//! ```
//!
//! `avoid` is a tree, or a path, canvas or glyph, or an array of any of
//! these. Glyphs keep their whole outline (the hull of their strokes) at
//! least `clearance` from all of it: by default an eighth of the symbol
//! scale, added to any clearance a tree was built with. Glyphs placed by one
//! call keep clear of each other too, so text on a path that loops back
//! doesn't land on itself.
//!
//! A tree is shared like paths and canvases: a text box made with one sees
//! anything added to it later, so `avoid.add(res.placed)` keeps later text
//! off what's just been placed. Canvases, paths and glyphs are taken as
//! they are when passed.

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use super::glyphs::Glyph;
use super::options::number;
use super::{Canvas, Path, Shared, Vec2};
use crate::journal::avoid::AvoidanceTree;
use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::paths::StrokePath;
use parry2d::math::Vec2 as PVec;

use super::RhaiResult;

/// An avoidance tree as scripts see it (see `Shared`).
pub type Avoid = Shared<AvoidanceTree>;

/// The default clearance, as a fraction of the symbol scale.
pub const DEFAULT_CLEARANCE: f32 = 0.125;

/// A tree of what `item` covers, items `clearance` away. Trees given keep
/// their own clearances.
fn tree_of(caller: &str, item: Dynamic, clearance: f32) -> RhaiResult<AvoidanceTree> {
    let mut tree = AvoidanceTree::new();
    add_to(caller, &mut tree, item, clearance)?;
    Ok(tree)
}

fn add_to(caller: &str, tree: &mut AvoidanceTree, item: Dynamic, clearance: f32) -> RhaiResult<()> {
    let type_name = item.type_name();
    if item.is_array() {
        for element in item.cast::<Array>() {
            add_to(caller, tree, element, clearance)?;
        }
    } else if let Some(path) = item.clone().try_cast::<Path>() {
        tree.add_path(&path.borrow(), clearance);
    } else if let Some(canvas) = item.clone().try_cast::<Canvas>() {
        let canvas: JournalCanvas = (&canvas).into();
        tree.add_canvas(&canvas, clearance);
    } else if let Some(glyph) = item.clone().try_cast::<Glyph>() {
        tree.add_symbol(&glyph.borrow(), clearance);
    } else if let Some(other) = item.try_cast::<Avoid>() {
        tree.merge(&other.borrow());
    } else {
        return Err(format!("{}: can't avoid a {}; expected a path, canvas, glyph, avoidance tree, or an array of them", caller, type_name).into());
    }
    Ok(())
}

/// Every point of `item` (curves flattened), for its hull, and how far
/// its canvases' marks reach past their lines.
fn hull_points(caller: &str, item: Dynamic, points: &mut Vec<PVec>, reach: &mut f32) -> RhaiResult<()> {
    let type_name = item.type_name();
    let add_path = |path: &StrokePath, points: &mut Vec<PVec>| {
        for (line, _) in path.flattened(HULL_TOLERANCE) {
            points.extend(line);
        }
    };
    if item.is_array() {
        for element in item.cast::<Array>() {
            hull_points(caller, element, points, reach)?;
        }
    } else if let Some(path) = item.clone().try_cast::<Path>() {
        add_path(&path.borrow(), points);
    } else if let Some(canvas) = item.clone().try_cast::<Canvas>() {
        let canvas: JournalCanvas = (&canvas).into();
        for stroke in canvas.strokes() {
            add_path(&stroke.path, points);
            *reach = reach.max(stroke.style.reach());
        }
    } else if let Some(glyph) = item.clone().try_cast::<Glyph>() {
        if !glyph.borrow().is_blank() {
            points.extend(glyph.borrow().footprint());
        }
    } else if item.is::<Avoid>() {
        return Err(format!("{}: an avoidance tree has no outline to take; pass what it was built from", caller).into());
    } else {
        return Err(format!("{}: can't take the outline of a {}; expected a path, canvas, glyph, or an array of them", caller, type_name).into());
    }
    Ok(())
}

/// Hulls only need their outlines roughly.
const HULL_TOLERANCE: f32 = 0.5;

fn add_hull(a: &mut Avoid, item: Dynamic, clearance: f32) -> RhaiResult<Avoid> {
    let (mut points, mut reach) = (Vec::new(), 0.0);
    hull_points("add_hull", item, &mut points, &mut reach)?;
    Ok(a.update(|tree| tree.add_hull(&points, clearance + reach)))
}

/// `#{ clearance }`, as tree.add, add_hull and add_region take it: 0 by
/// default.
fn clearance_option(caller: &str, options: &Map) -> RhaiResult<f32> {
    super::options::check_keys(caller, options, &["clearance"])?;
    options.get("clearance").map_or(Ok(0.0), |c| clearance_arg(caller, c.clone()))
}

fn clearance_arg(caller: &str, clearance: Dynamic) -> RhaiResult<f32> {
    let value = clearance.as_float().map(|f| f as f32).or_else(|_| clearance.as_int().map(|i| i as f32))
        .map_err(|t| -> Box<EvalAltResult> { format!("{}: clearance should be a number, got {}", caller, t).into() })?;
    if !(value >= 0.0) || !value.is_finite() {
        return Err(format!("{}: clearance must be a finite number of at least 0, got {}", caller, value).into());
    }
    Ok(value)
}

/// The `avoid` and `clearance` options: what to keep clear of, and how far
/// to keep from it beyond its own clearances (by default
/// `DEFAULT_CLEARANCE` of the symbol scale, so `None` until the scale is
/// known).
pub struct AvoidOption {
    pub tree: Option<Avoid>,
    pub clearance: Option<f32>,
}

impl AvoidOption {
    /// `clearance` alone is an error unless `alone_ok`: for callers that
    /// avoid something anyway.
    pub fn parse(caller: &str, options: &Map, alone_ok: bool) -> RhaiResult<Self> {
        let clearance = match number(caller, options, "clearance")? {
            Some(c) if c >= 0.0 && c.is_finite() => Some(c),
            Some(c) => return Err(format!("{}: 'clearance' must be a finite number of at least 0, got {}", caller, c).into()),
            None => None,
        };
        let tree = match options.get("avoid") {
            None if clearance.is_some() && !alone_ok => return Err(format!("{}: 'clearance' needs 'avoid'", caller).into()),
            None => None,
            // A tree alone is used as it is, so later changes reach it.
            Some(item) => match item.clone().try_cast::<Avoid>() {
                Some(tree) => Some(tree),
                None => Some(Avoid::new(tree_of(&format!("{} 'avoid'", caller), item.clone(), 0.0)?)),
            },
        };
        Ok(AvoidOption { tree, clearance })
    }

    /// How far glyphs at `size` keep from what they avoid.
    pub fn padding(&self, symbol_scale: f32) -> f32 {
        self.clearance.unwrap_or(symbol_scale * DEFAULT_CLEARANCE)
    }
}

pub fn register(engine: &mut Engine) {
    engine.register_type_with_name::<Avoid>("AvoidanceTree")
        .register_fn("new_avoidance_tree", || Avoid::new(AvoidanceTree::new()))
        .register_fn("copy", Avoid::copy)
        .register_get("is_empty", |a: &mut Avoid| a.borrow().is_empty())
        .register_fn("add", |a: &mut Avoid, item: Dynamic| -> RhaiResult<Avoid> {
            // Built first, so a tree can add itself.
            let more = tree_of("add", item, 0.0)?;
            Ok(a.update(|tree| tree.merge(&more)))
        })
        .register_fn("add", |a: &mut Avoid, item: Dynamic, options: Map| -> RhaiResult<Avoid> {
            let more = tree_of("add", item, clearance_option("add", &options)?)?;
            Ok(a.update(|tree| tree.merge(&more)))
        })
        .register_fn("add_region", |a: &mut Avoid, path: Path| -> Avoid {
            let path = path.get();
            a.update(|tree| tree.add_region(&path, 0.0))
        })
        .register_fn("add_region", |a: &mut Avoid, path: Path, options: Map| -> RhaiResult<Avoid> {
            let clearance = clearance_option("add_region", &options)?;
            let path = path.get();
            Ok(a.update(|tree| tree.add_region(&path, clearance)))
        })
        .register_fn("add_hull", |a: &mut Avoid, item: Dynamic| add_hull(a, item, 0.0))
        .register_fn("add_hull", |a: &mut Avoid, item: Dynamic, options: Map| -> RhaiResult<Avoid> {
            add_hull(a, item, clearance_option("add_hull", &options)?)
        })
        .register_fn("blocks", |a: &mut Avoid, point: Vec2| a.borrow().blocks_point(point.into()))
        .register_fn("blocks", |a: &mut Avoid, glyph: Glyph| {
            let footprint = glyph.borrow().footprint();
            !glyph.borrow().is_blank() && a.borrow().blocks(&footprint, 0.0)
        });
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
        engine().engine.eval::<T>(script).unwrap_or_else(|e| panic!("script should evaluate: {}", e))
    }

    fn fails(script: &str) -> String {
        engine().engine.run(script).expect_err("script should fail").to_string()
    }

    #[test]
    fn trees_block_near_what_was_added() {
        let ok: bool = eval(r#"
            let style = new_style("black", new_brush("Ink Pen", 0.3, 0.7, 2.0));
            let c = new_canvas();
            c.add(new_line(vec2(0.0, 100.0), vec2(100.0, 100.0)), style);
            let avoid = new_avoidance_tree();
            if !avoid.is_empty { throw "should start empty"; }
            avoid.add(new_line(vec2(0.0, 0.0), vec2(100.0, 0.0)), #{ clearance: 5 })
                .add(c, #{ clearance: 3.0 })
                .add_region(new_circle(vec2(500.0, 0.0), 50.0));
            // The canvas's brush reaches 1 either side of its line.
            [avoid.blocks(vec2(50.0, 4.0)), !avoid.blocks(vec2(50.0, 6.0)),
             avoid.blocks(vec2(50.0, 103.5)), !avoid.blocks(vec2(50.0, 104.5)),
             avoid.blocks(vec2(500.0, 0.0)), !avoid.blocks(vec2(600.0, 0.0))]
                .all(|b| b)
        "#);
        assert!(ok);
    }

    #[test]
    fn trees_are_shared_and_take_glyphs_and_arrays() {
        let ok: bool = eval(r#"
            let a = new_avoidance_tree();
            let b = a;
            let g = font("symbols").glyph("basic_eye").scale_by(20.0);
            b.add([g, new_line(vec2(100.0, 0.0), vec2(200.0, 0.0))]);
            let c = a.copy();
            c.add(new_line(vec2(0.0, 500.0), vec2(10.0, 500.0)), #{ clearance: 1.0 });
            a.add(a);   // adding itself doesn't hang or fail
            [a.blocks(vec2(0.0, 0.0)), a.blocks(g), a.blocks(vec2(150.0, 0.0)),
             !a.blocks(vec2(5.0, 500.0)), c.blocks(vec2(5.0, 500.0))].all(|b| b)
        "#);
        assert!(ok);
    }

    #[test]
    fn hulls_cover_the_gaps_inside_a_drawing() {
        let ok: bool = eval(r#"
            let style = new_style("black", new_brush("Ink Pen", 0.3, 0.7, 2.0));
            let c = new_canvas();
            c.add(new_line(vec2(0.0, 0.0), vec2(100.0, 0.0)), style);
            c.add(new_line(vec2(0.0, 100.0), vec2(100.0, 100.0)), style);
            let lines = new_avoidance_tree().add(c);
            let hull = new_avoidance_tree().add_hull(c, #{ clearance: 5.0 });
            // Brush reach (1) plus the clearance.
            [!lines.blocks(vec2(50.0, 50.0)), hull.blocks(vec2(50.0, 50.0)),
             hull.blocks(vec2(50.0, -5.5)), !hull.blocks(vec2(50.0, -6.5))].all(|b| b)
        "#);
        assert!(ok);
        assert!(fails("let a = new_avoidance_tree();\na.add_hull(a);").contains("add_hull: an avoidance tree has no outline"));
    }

    #[test]
    fn bad_items_and_clearances_fail_at_their_line() {
        for (script, want) in [
            ("let a = new_avoidance_tree();\na.add(3);", "add: can't avoid a i64"),
            ("let a = new_avoidance_tree();\na.add(new_line(vec2(0.0, 0.0), vec2(1.0, 0.0)), #{ clearance: -1.0 });", "clearance must be a finite number of at least 0"),
            ("let a = new_avoidance_tree();\na.add(new_line(vec2(0.0, 0.0), vec2(1.0, 0.0)), #{ clearance: \"far\" });", "clearance should be a number"),
            ("let ok = 1;\nnew_textbox(#{ center: vec2(0.0, 0.0), width: 10, height: 10, size: 2, avoid: [1] });", "new_textbox 'avoid': can't avoid a i64"),
            ("let ok = 1;\nnew_textbox(#{ center: vec2(0.0, 0.0), width: 10, height: 10, size: 2, clearance: 3 });", "'clearance' needs 'avoid'"),
        ] {
            let err = fails(script);
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }
}
