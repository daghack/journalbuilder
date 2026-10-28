//! Avoidance trees for rhai scripts: what text keeps clear of.
//!
//! ```rhai
//! let avoid = new_avoidance_tree();
//! avoid.add(border, #{ margin: 8 });    // keep 8 units from a path's lines
//! avoid.add(drawing, #{ margin: 4 });   // from a canvas's marks (their edges, not their middles)
//! avoid.add(glyphs, #{ margin: 2 });    // from glyphs' outlines, insides included
//! avoid.add_region(new_circle(vec2(0.0, 0.0), 80.0));   // out of a closed shape
//! avoid.add_hull(drawing, #{ margin: 10.0 });   // out of everything inside a drawing's outline
//! avoid.add(other_tree);            // everything another tree avoids
//! avoid.add([border, drawing], #{ margin: 3 });   // arrays of any of these
//! avoid.blocks(vec2(10.0, 0.0)); // whether a point is too close to something
//! avoid.blocks(glyph);              // whether a placed glyph is
//! ```
//!
//! Every text layout function takes an `avoid` option, and keeps the
//! paragraph's `clearance` from it:
//!
//! ```rhai
//! new_textbox(#{ center: c, width: 400, height: 300, type: #{ glyph_scale: 20 }, avoid: avoid });
//! path.fill_with_text(msg, #{ type: body, avoid: drawing, paragraph: #{ clearance: 6.0 } });
//! ```
//!
//! `avoid` is a tree, or a path, canvas or glyph, or an array of any of
//! these. Glyphs keep their whole outline (the hull of their strokes) at
//! least `clearance` from all of it (3 canvas units unless the paragraph
//! says), on top of any margin a tree was built with. Glyphs placed by one
//! call keep clear of each other too, so text on a path that loops back
//! doesn't land on itself.
//!
//! A tree is shared like paths and canvases: a text box made with one sees
//! anything added to it later, so `avoid.add(res.placed)` keeps later text
//! off what's just been placed. Canvases, paths and glyphs are taken as
//! they are when passed.

use rhai::{Array, Dynamic, Engine, Map};

use super::glyphs::Glyph;
use super::{Canvas, Path, Shared, Vec2};
use crate::journal::avoid::AvoidanceTree;
use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::paths::StrokePath;
use parry2d::math::Vec2 as PVec;

use super::RhaiResult;

/// An avoidance tree as scripts see it (see `Shared`).
pub type Avoid = Shared<AvoidanceTree>;

/// A tree of what `item` covers, items `clearance` away. Trees given keep
/// their own clearances.
pub(super) fn tree_of(caller: &str, item: Dynamic, clearance: f32) -> RhaiResult<AvoidanceTree> {
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

/// `#{ margin }`, as tree.add, add_hull and add_region take it: how far
/// to keep from the item, 0 by default.
fn margin_option(caller: &str, options: &Map) -> RhaiResult<f32> {
    super::options::check_keys(caller, options, &["margin"])?;
    options.get("margin").map_or(Ok(0.0), |m| Ok(super::options::at_least_zero(caller, "margin", m)? as f32))
}

/// The `avoid` option, what to keep clear of, and how far to keep from it
/// beyond its own margins: the paragraph's `clearance`.
pub struct AvoidOption {
    pub tree: Option<Avoid>,
    clearance: f32,
}

impl AvoidOption {
    pub fn parse(caller: &str, options: &Map, clearance: f32) -> RhaiResult<Self> {
        let tree = match options.get("avoid") {
            None => None,
            // A tree alone is used as it is, so later changes reach it.
            Some(item) => match item.clone().try_cast::<Avoid>() {
                Some(tree) => Some(tree),
                None => Some(Avoid::new(tree_of(&format!("{} 'avoid'", caller), item.clone(), 0.0)?)),
            },
        };
        Ok(AvoidOption { tree, clearance })
    }

    /// How far glyphs keep from what they avoid.
    pub fn padding(&self) -> f32 {
        self.clearance
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
            let more = tree_of("add", item, margin_option("add", &options)?)?;
            Ok(a.update(|tree| tree.merge(&more)))
        })
        .register_fn("add_region", |a: &mut Avoid, path: Path| -> Avoid {
            let path = path.get();
            a.update(|tree| tree.add_region(&path, 0.0))
        })
        .register_fn("add_region", |a: &mut Avoid, path: Path, options: Map| -> RhaiResult<Avoid> {
            let clearance = margin_option("add_region", &options)?;
            let path = path.get();
            Ok(a.update(|tree| tree.add_region(&path, clearance)))
        })
        .register_fn("add_hull", |a: &mut Avoid, item: Dynamic| add_hull(a, item, 0.0))
        .register_fn("add_hull", |a: &mut Avoid, item: Dynamic, options: Map| -> RhaiResult<Avoid> {
            add_hull(a, item, margin_option("add_hull", &options)?)
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
        let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_GLYPHSETS).expect("Failed to load fonts.json");
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
            let style = new_style(new_brush_style("black", "Ink Pen", 2.0), new_handwriting_style());
            let c = new_canvas();
            c.add(new_line(vec2(0.0, 100.0), vec2(100.0, 100.0)), style);
            let avoid = new_avoidance_tree();
            if !avoid.is_empty { throw "should start empty"; }
            avoid.add(new_line(vec2(0.0, 0.0), vec2(100.0, 0.0)), #{ margin: 5 })
                .add(c, #{ margin: 3.0 })
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
            let g = glyphset("symbols").glyph("basic_eye").scale_by(20.0);
            b.add([g, new_line(vec2(100.0, 0.0), vec2(200.0, 0.0))]);
            let c = a.copy();
            c.add(new_line(vec2(0.0, 500.0), vec2(10.0, 500.0)), #{ margin: 1.0 });
            a.add(a);   // adding itself doesn't hang or fail
            [a.blocks(vec2(0.0, 0.0)), a.blocks(g), a.blocks(vec2(150.0, 0.0)),
             !a.blocks(vec2(5.0, 500.0)), c.blocks(vec2(5.0, 500.0))].all(|b| b)
        "#);
        assert!(ok);
    }

    #[test]
    fn hulls_cover_the_gaps_inside_a_drawing() {
        let ok: bool = eval(r#"
            let style = new_style(new_brush_style("black", "Ink Pen", 2.0), new_handwriting_style());
            let c = new_canvas();
            c.add(new_line(vec2(0.0, 0.0), vec2(100.0, 0.0)), style);
            c.add(new_line(vec2(0.0, 100.0), vec2(100.0, 100.0)), style);
            let lines = new_avoidance_tree().add(c);
            let hull = new_avoidance_tree().add_hull(c, #{ margin: 5.0 });
            // Brush reach (1) plus the margin.
            [!lines.blocks(vec2(50.0, 50.0)), hull.blocks(vec2(50.0, 50.0)),
             hull.blocks(vec2(50.0, -5.5)), !hull.blocks(vec2(50.0, -6.5))].all(|b| b)
        "#);
        assert!(ok);
        assert!(fails("let a = new_avoidance_tree();\na.add_hull(a);").contains("add_hull: an avoidance tree has no outline"));
    }

    #[test]
    fn bad_items_and_margins_fail_at_their_line() {
        for (script, want) in [
            ("let a = new_avoidance_tree();\na.add(3);", "add: can't avoid a i64"),
            ("let a = new_avoidance_tree();\na.add(new_line(vec2(0.0, 0.0), vec2(1.0, 0.0)), #{ margin: -1.0 });", "add: 'margin' can't be negative"),
            ("let a = new_avoidance_tree();\na.add(new_line(vec2(0.0, 0.0), vec2(1.0, 0.0)), #{ margin: \"far\" });", "add: 'margin' should be a number"),
            ("let a = new_avoidance_tree();\na.add(new_line(vec2(0.0, 0.0), vec2(1.0, 0.0)), #{ clearance: 1 });", "add: unknown option 'clearance'; options: margin"),
            ("let ok = 1;\nnew_textbox(#{ center: vec2(0.0, 0.0), width: 10, height: 10, type: #{ glyph_scale: 2 }, avoid: [1] });", "new_textbox 'avoid': can't avoid a i64"),
        ] {
            let err = fails(script);
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }
}
