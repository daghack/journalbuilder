//! Free regions for rhai scripts: the stroke-free space of a page, as
//! paths to write text into.
//!
//! ```rhai
//! let page_area = new_rectangle(vec2(425, 550), 850, 1100);
//! let regions = free_regions(page_area, drawing);   // paths, largest first
//! let regions = free_regions(page_area, [drawing, border], #{
//!     clearance: 12,        // how far outlines keep from what's avoided (default: one resolution)
//!     resolution: 4,        // the grid's cell size (default: the area's longer side / 400)
//!     min_width: 60,        // free space narrower than this is left out, parting regions (default 0)
//!     min_area: 5000,       // smaller regions are left out (default: four cells square)
//!     outline: "smooth",    // or "polygon" (the default), straight edges
//!     tolerance: 2,         // how far a polygon's edges may stray from the grid's (default half a cell)
//!     order: "smallest",    // or "largest" (the default), by the room they enclose
//! });
//! let rest = text;
//! for region in regions {
//!     let out = write_inside_path(rest, region, #{ style: ink, type: body });
//!     canvas.add(out.canvas);
//!     rest = out.rest;
//! }
//! ```
//!
//! Or round a canvas, in its bounds grown (or, negative, shrunk) by a
//! margin on each side, avoiding its marks:
//!
//! ```rhai
//! let regions = free_regions_in_canvas(drawing, #{ margin: vec2(100, 50), clearance: 12 });
//! // A page as it's rendered (its size, or its canvases' bounds), 60 in from its edges.
//! let regions = free_regions_in_page(page, #{ margin: vec2(-60, -60) });
//! ```
//!
//! Each region is one path: its outline, then a subpath for each hole, so
//! it goes straight into `write_inside_path` or `new_textbox`'s `shape`.
//! `avoid` is anything text's `avoid` option takes: paths, canvases (their
//! marks' edges), glyphs, avoidance trees, or arrays of them.

use parry2d::bounding_volume::{Aabb, BoundingVolume};
use parry2d::math::Vec2 as PVec;
use rhai::{Array, Dynamic, Engine, Map};

use super::avoid::tree_of;
use super::options::{check_keys, one_of, positive, at_least_zero};
use super::{Canvas, Page, Path, RhaiResult, Vec2};
use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::art::smooth_closed;
use crate::journal::free_regions::{free_regions, FreeRegionOptions};
use crate::journal::paths::StrokePath;

const OPTION_KEYS: [&str; 7] = ["clearance", "resolution", "min_width", "min_area", "outline", "tolerance", "order"];

/// How many cells across the area's longer side by default.
const DEFAULT_CELLS: f32 = 400.0;

/// How far apart, in cells, a smooth outline's points are along its
/// straight runs, so the curve keeps to them and rounds only the corners.
const SMOOTH_SPACING: f32 = 3.0;

/// A closed outline with points added along each edge, at most `spacing`
/// apart.
fn along_edges(ring: &[parry2d::math::Vec2], spacing: f32) -> Vec<parry2d::math::Vec2> {
    let mut out = Vec::new();
    for (i, &a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        let steps = ((a.distance(b) / spacing).ceil() as usize).max(1);
        out.extend((0..steps).map(|k| a.lerp(b, k as f32 / steps as f32)));
    }
    out
}

/// The free regions in `area`, `caller` naming the call in errors.
fn regions(caller: &str, area: &Path, avoid: Dynamic, options: &Map) -> RhaiResult<Vec<Dynamic>> {
    check_keys(caller, options, &OPTION_KEYS)?;
    let area = area.borrow();
    let mut bounds = parry2d::bounding_volume::Aabb::new_invalid();
    for (points, _) in area.flattened(0.01) {
        for p in points {
            bounds.take_point(p);
        }
    }
    let longer = (bounds.maxs - bounds.mins).max_element();
    if !(longer > 0.0) {
        return Err(format!("{}: the area encloses nothing: it needs a subpath of at least three points", caller).into());
    }
    let resolution = if options.contains_key("resolution") { positive(caller, options, "resolution")? } else { longer / DEFAULT_CELLS };
    let number = |key: &str, default: f32| -> RhaiResult<f32> {
        options.get(key).map_or(Ok(default), |v| Ok(at_least_zero(caller, key, v)? as f32))
    };
    let o = FreeRegionOptions {
        resolution,
        clearance: number("clearance", resolution)?,
        min_width: number("min_width", 0.0)?,
        min_area: number("min_area", (4.0 * resolution).powi(2))?,
        tolerance: number("tolerance", resolution / 2.0)?,
    };
    let smooth = match options.get("outline") {
        None => false,
        Some(v) => one_of(caller, "outline", v, &["polygon", "smooth"])? == "smooth",
    };
    let smallest_first = match options.get("order") {
        None => false,
        Some(v) => one_of(caller, "order", v, &["largest", "smallest"])? == "smallest",
    };
    let rings: Vec<_> = area.flattened(resolution * 0.1).into_iter().map(|(points, _)| points).filter(|p| p.len() >= 3).collect();
    if rings.is_empty() {
        return Err(format!("{}: the area encloses nothing: it needs a subpath of at least three points", caller).into());
    }
    let tree = tree_of(&format!("{} 'avoid'", caller), avoid, 0.0)?;
    let mut found = free_regions(&rings, &tree, &o).map_err(|e| format!("{}: {}", caller, e))?;
    if smallest_first {
        found.reverse();
    }
    Ok(found.into_iter().map(|region| {
        let mut path = StrokePath::new();
        for ring in &region.rings {
            path = if smooth {
                smooth_closed(path, &along_edges(ring, SMOOTH_SPACING * resolution))
            } else {
                let mut p = path.move_to(ring[0].x, ring[0].y);
                for q in &ring[1..] {
                    p = p.line_to(q.x, q.y);
                }
                p.close_path()
            };
        }
        Dynamic::from(Path::new(path))
    }).collect())
}

/// `free_regions` in `bounds` (`what`'s, for errors), grown by
/// `options.margin` on each side (shrunk, if negative), avoiding `avoid`.
fn in_box(caller: &str, what: &str, bounds: Aabb, avoid: Dynamic, mut options: Map) -> RhaiResult<Vec<Dynamic>> {
    check_keys(caller, &options, &[&OPTION_KEYS[..], &["margin"]].concat())?;
    let margin = match options.remove("margin") {
        None => PVec::ZERO,
        Some(v) => v.clone().try_cast::<Vec2>().map(PVec::from)
            .ok_or_else(|| format!("{}: 'margin' should be a vec2, got {}", caller, v.type_name()))?,
    };
    if !(bounds.mins.x <= bounds.maxs.x && bounds.mins.y <= bounds.maxs.y) {
        return Err(format!("{}: the {} is empty", caller, what).into());
    }
    let (min, max) = (bounds.mins - margin, bounds.maxs + margin);
    if !(min.x < max.x && min.y < max.y) {
        return Err(format!("{}: the margin {:?} leaves nothing of the {}'s {} by {}", caller, (margin.x, margin.y), what, bounds.extents().x, bounds.extents().y).into());
    }
    let area = Path::new(StrokePath::new().move_to(min.x, min.y).line_to(max.x, min.y).line_to(max.x, max.y).line_to(min.x, max.y).close_path());
    regions(caller, &area, avoid, &options)
}

/// `free_regions` in the canvas's bounds, avoiding it.
fn in_canvas(canvas: Canvas, options: Map) -> RhaiResult<Vec<Dynamic>> {
    let bounds = JournalCanvas::from(&canvas).bounds();
    in_box("free_regions_in_canvas", "canvas", bounds, Dynamic::from(canvas), options)
}

/// `free_regions` on the page as it's rendered, avoiding its canvases:
/// its set size, or each canvas's bounds and margin; a tile's own part.
fn in_page(page: Page, options: Map) -> RhaiResult<Vec<Dynamic>> {
    let mut bounds = Aabb::new_invalid();
    let mut canvases = Array::new();
    for canvas in page.canvases() {
        let Some(rendered) = page.render_canvas(canvas) else { continue };
        bounds.merge(&match rendered.frame() {
            Some(frame) => {
                let cell = frame.page.extents() / PVec::new(frame.columns as f32, frame.rows as f32);
                let min = frame.page.mins + cell * PVec::new(frame.column as f32, frame.row as f32);
                Aabb::new(min, min + cell)
            }
            None => rendered.bounds(),
        });
        canvases.push(Dynamic::from(Canvas::new(canvas.clone())));
    }
    in_box("free_regions_in_page", "page", bounds, Dynamic::from(canvases), options)
}

pub fn register(engine: &mut Engine) {
    engine
        .register_fn("free_regions", |area: Path, avoid: Dynamic| regions("free_regions", &area, avoid, &Map::new()))
        .register_fn("free_regions", |area: Path, avoid: Dynamic, options: Map| regions("free_regions", &area, avoid, &options))
        .register_fn("free_regions_in_canvas", |canvas: Canvas| in_canvas(canvas, Map::new()))
        .register_fn("free_regions_in_canvas", in_canvas)
        .register_fn("free_regions_in_page", |page: Page| in_page(page, Map::new()))
        .register_fn("free_regions_in_page", in_page);
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

    const SETUP: &str = r#"
        let ink = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
        let drawing = new_canvas();
        drawing.add(new_line(vec2(0, -100), vec2(0, 100)), ink);
        drawing.add(new_circle(vec2(-50, 0), 10), ink);
        let area = new_rectangle(vec2(0, 0), 200, 200);
    "#;

    #[test]
    fn regions_part_at_strokes_largest_first() {
        let script = format!(r#"{}
            // Inside the circle is a speck too small to keep.
            let regions = free_regions(area, drawing, #{{ clearance: 4, min_area: 500 }});
            if regions.len() != 2 {{ throw "expected two rooms, got " + regions.len(); }}
            if regions[0].area() < regions[1].area() {{ throw "not largest first"; }}
            // The room with the circle has a hole.
            let holed = regions.filter(|r| r.subpaths().len() == 2);
            if holed.len() != 1 {{ throw "expected one room with a hole"; }}
            let smallest = free_regions(area, drawing, #{{ clearance: 4, min_area: 500, order: "smallest" }});
            if smallest[0].area() != regions[1].area() || smallest[1].area() != regions[0].area() {{ throw "order: smallest should reverse them"; }}
            let smooth = free_regions(area, drawing, #{{ clearance: 4, min_area: 500, outline: "smooth" }});
            if smooth.len() != 2 {{ throw "smooth outlines should find the same rooms"; }}
            // Rounding the corners changes little, and keeps inside the area.
            if (smooth[0].area() - regions[0].area()).abs() > 0.03 * regions[0].area() {{ throw "smoothing moved the outline: " + smooth[0].area() + " " + regions[0].area(); }}
            let sb = smooth[0].bounds();
            if sb.min.x < -101 || sb.max.x > 101 || sb.min.y < -101 || sb.max.y > 101 {{ throw "smoothing left the area"; }}
            true
        "#, SETUP);
        assert!(engine().engine.eval::<bool>(&script).expect("script should run"));
    }

    #[test]
    fn regions_in_a_canvas_are_in_its_bounds_and_margin() {
        let script = format!(r#"{}
            // The drawing's bounds are the line's and circle's, strokes included.
            let b = drawing.bounds();
            let inside = free_regions_in_canvas(drawing, #{{ clearance: 4, min_area: 100 }});
            let same = free_regions(new_rectangle(b.center, b.width, b.height), drawing, #{{ clearance: 4, min_area: 100 }});
            if inside.len() != same.len() || inside[0].area() != same[0].area() {{ throw "not free_regions in the bounds"; }}
            let grown = free_regions_in_canvas(drawing, #{{ margin: vec2(40, 10), clearance: 4 }});
            let gb = grown[0].bounds();
            if gb.min.x > b.min.x - 35 || gb.max.y < b.max.y + 5 || gb.max.y > b.max.y + 11 {{ throw "the margin should grow the area: " + gb; }}
            let shrunk = free_regions_in_canvas(drawing, #{{ margin: vec2(-20, -20), clearance: 4, order: "smallest" }});
            for r in shrunk {{ let rb = r.bounds(); if rb.min.x < b.min.x + 19 || rb.max.y > b.max.y - 19 {{ throw "a negative margin should shrink it: " + rb; }} }}
            if shrunk[0].area() > shrunk[shrunk.len() - 1].area() {{ throw "options pass through: smallest first"; }}
            true
        "#, SETUP);
        assert!(engine().engine.eval::<bool>(&script).expect("script should run"));
        for (call, want) in [
            ("free_regions_in_canvas(drawing, #{ margin: 3 })", "'margin' should be a vec2"),
            ("free_regions_in_canvas(drawing, #{ margin: vec2(-200, 0) })", "leaves nothing of the canvas"),
            ("free_regions_in_canvas(new_canvas())", "the canvas is empty"),
            ("free_regions_in_canvas(drawing, #{ colour: 1 })", "free_regions_in_canvas: unknown option 'colour'"),
            ("free_regions_in_canvas(drawing, #{ outline: \"wavy\" })", "free_regions_in_canvas: unknown outline"),
        ] {
            let err = engine().engine.run(&format!("{}\n{};", SETUP, call)).expect_err(call).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn regions_in_a_page_are_in_it_as_rendered() {
        let script = format!(r#"{}
            fn within(regions, min, max) {{
                for r in regions {{ let b = r.bounds(); if b.min.x < min.x - 0.01 || b.min.y < min.y - 0.01 || b.max.x > max.x + 0.01 || b.max.y > max.y + 0.01 {{ return false; }} }}
                true
            }}
            // Without a size, the page is its canvases' bounds and margins.
            let other = new_canvas();
            other.add(new_circle(vec2(200, 0), 20), ink);
            let page = new_page(#{{ margin: 10 }}).add(drawing).add(other);
            let regions = free_regions_in_page(page, #{{ clearance: 4 }});
            let pb = #{{ min: vec2(-111, -111), max: vec2(231, 111) }};
            if !within(regions, pb.min, pb.max) {{ throw "outside the page"; }}
            if regions[0].bounds().max.x < 200 {{ throw "the page spans both canvases"; }}
            let circled = regions.filter(|r| r.bounds().max.x > 150 && r.subpaths().len() > 1);
            if circled.len() != 1 {{ throw "the second canvas is avoided too"; }}
            // A set size is the page, from the origin.
            page.size = vec2(80, 60);
            let sized = free_regions_in_page(page, #{{ clearance: 4 }});
            if !within(sized, vec2(0, 0), vec2(80, 60)) || sized.len() != 1 {{ throw "not the page's size: " + sized.len(); }}
            // In from its edges.
            let inset = free_regions_in_page(page, #{{ clearance: 4, margin: vec2(-10, -5) }});
            if !within(inset, vec2(10, 5), vec2(70, 55)) {{ throw "the margin should keep it in"; }}
            // A tile is its own part of the page.
            page.size = vec2(400, 200);
            page.name = "p";
            let tile = page.split(#{{ columns: 2, rows: 1 }})[1];
            let right = free_regions_in_page(tile, #{{ clearance: 4 }});
            if !within(right, vec2(200, 0), vec2(400, 200)) {{ throw "not the tile's part"; }}
            true
        "#, SETUP);
        assert!(engine().engine.eval::<bool>(&script).expect("script should run"));
        for (call, want) in [
            ("free_regions_in_page(new_page())", "free_regions_in_page: the page is empty"),
            ("free_regions_in_page(new_page().add(drawing), #{ margin: vec2(0, -200) })", "leaves nothing of the page"),
            ("free_regions_in_page(new_page().add(drawing), #{ colour: 1 })", "free_regions_in_page: unknown option 'colour'"),
        ] {
            let err = engine().engine.run(&format!("{}\n{};", SETUP, call)).expect_err(call).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn bad_options_fail_at_their_line() {
        for (call, want) in [
            ("free_regions(area, drawing, #{ colour: 1 })", "unknown option 'colour'"),
            ("free_regions(area, drawing, #{ resolution: 0 })", "'resolution' must be positive"),
            ("free_regions(area, drawing, #{ clearance: -1 })", "'clearance' can't be negative"),
            ("free_regions(area, drawing, #{ outline: \"wavy\" })", "unknown outline 'wavy'"),
            ("free_regions(area, drawing, #{ order: \"tallest\" })", "unknown order 'tallest'"),
            ("free_regions(area, 3)", "can't avoid a i64"),
            ("free_regions(new_line(vec2(0, 0), vec2(1, 1)), drawing)", "the area encloses nothing"),
            ("free_regions(area, drawing, #{ resolution: 0.01 })", "make it coarser"),
        ] {
            let err = engine().engine.run(&format!("{}\n{};", SETUP, call)).expect_err(call).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }
}
