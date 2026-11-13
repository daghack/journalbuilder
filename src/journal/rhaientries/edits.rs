//! The visual editor's edits. A script hands a canvas to the editor by
//! name, `canvas.commit_to_editor("hero")`, and the edit saved for that
//! name in the entry's `edits/ENTRY.json` is applied to it there and then:
//! moved, turned, scaled, skewed, its corners pinned, hidden, or re-inked.
//!
//! ```json
//! { "version": 1, "canvases": { "hero": {
//!     "move": [12, -4], "turn": 15, "scale": [1.2, -1], "skew": 8,
//!     "warp": [[0, 0], [6, -3], [0, 0], [0, 0]],
//!     "hidden": false, "locked": false,
//!     "ink": { "color": "#7a1f1f", "opacity": 0.8, "weight": 1.5 } } } }
//! ```
//!
//! Edits are about the canvas's own box (its bounds when it's committed),
//! so they hold however the script moves it before or after: the corners
//! are pinned first (`warp`, each corner moved by so much: top left, top
//! right, bottom right, bottom left), then each point `p` goes to
//! `C + move + R(turn) K(skew) S(scale) (p - C)`, `C` the box's centre,
//! turns clockwise in degrees, skews sideways in degrees. Paints laid
//! across the canvas (gradients from a point, pictures) move with the
//! strokes, but not with the warp. Then the ink: a colour for every stroke,
//! how opaque, and how wide (a multiple of each brush's size). `locked` is
//! the editor's own; the drawing ignores it.
//!
//! Nothing here draws from the random generator, so the rest of the script
//! draws the same with edits or without.

use parry2d::math::Vec2 as PVec;
use rhai::{Dynamic, Engine, NativeCallContext};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use super::canvas::CanvasData;
use super::entry::check_name;
use super::{Canvas, Entry, RhaiResult};
use crate::journal::affine::Affine;
use crate::journal::canvas::{parse_color, Canvas as JournalCanvas};
use crate::journal::paths::StrokePath;

/// The edits file version this build reads.
pub const EDITS_VERSION: u32 = 1;

/// An entry's edits file: each committed canvas's edit, by name.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edits {
    pub version: u32,
    #[serde(default)]
    pub canvases: BTreeMap<String, CanvasEdit>,
}

/// What the editor did to one canvas; every part may be left out.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CanvasEdit {
    #[serde(rename = "move")]
    pub offset: [f32; 2],
    pub turn: f32,
    pub scale: [f32; 2],
    pub skew: f32,
    pub warp: Option<[[f32; 2]; 4]>,
    pub hidden: bool,
    pub locked: bool,
    pub ink: Option<InkEdit>,
    /// For a symbolic (see symbolic.rs): the symbols it's drawn as, and the
    /// symbol set they're of, in place of the script's.
    pub symbols: Option<Vec<u32>>,
    pub symbol_set: Option<String>,
}

impl Default for CanvasEdit {
    fn default() -> Self {
        CanvasEdit { offset: [0.0; 2], turn: 0.0, scale: [1.0; 2], skew: 0.0, warp: None, hidden: false, locked: false, ink: None, symbols: None, symbol_set: None }
    }
}

/// The ink a committed canvas is re-inked with.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InkEdit {
    /// A CSS colour for every stroke.
    pub color: Option<String>,
    /// How opaque, 0 to 1.
    pub opacity: Option<f32>,
    /// How wide, a multiple of each brush's size.
    pub weight: Option<f32>,
}

impl Edits {
    /// The edits in `text`, checked.
    pub fn parse(text: &str) -> Result<Edits, String> {
        let edits: Edits = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if edits.version != EDITS_VERSION {
            return Err(format!("version {} isn't one this build reads ({})", edits.version, EDITS_VERSION));
        }
        for (name, edit) in &edits.canvases {
            check_name("edits", Dynamic::from(name.clone())).map_err(|e| e.to_string())?;
            edit.check().map_err(|e| format!("'{}': {}", name, e))?;
        }
        Ok(edits)
    }
}

impl CanvasEdit {
    fn check(&self) -> Result<(), String> {
        let numbers = self.offset.iter().chain(&self.scale).chain([&self.turn, &self.skew])
            .chain(self.warp.iter().flatten().flatten());
        if numbers.into_iter().any(|v| !v.is_finite()) {
            return Err("every number should be finite".into());
        }
        if self.scale.contains(&0.0) {
            return Err("a scale can't be 0".into());
        }
        if self.skew.abs() >= 89.0 {
            return Err(format!("the skew should be under 89 degrees either way, got {}", self.skew));
        }
        if let Some(ink) = &self.ink {
            if let Some(color) = &ink.color {
                parse_color(color)?;
            }
            if ink.opacity.is_some_and(|o| !(0.0..=1.0).contains(&o)) {
                return Err("the ink's opacity should be from 0 to 1".into());
            }
            if ink.weight.is_some_and(|w| !(w > 0.0 && w.is_finite())) {
                return Err("the ink's weight should be above 0".into());
            }
        }
        Ok(())
    }

    /// The move, turn, scale and skew, about `centre`.
    pub fn affine(&self, centre: PVec) -> Affine {
        Affine::scale(self.scale[0], self.scale[1])
            .then(&Affine::skew(self.skew))
            .then(&Affine::rotate(self.turn))
            .about(centre)
            .then(&Affine::translate(PVec::new(self.offset[0], self.offset[1])))
    }

    /// The corners pinned: a point of the box from `min` to `max` moved as
    /// its corners are, between them (and on past them, outside it).
    fn warp_point(&self, min: PVec, max: PVec, p: PVec) -> PVec {
        let Some(w) = self.warp else { return p };
        let size = max - min;
        let u = if size.x > 0.0 { (p.x - min.x) / size.x } else { 0.0 };
        let v = if size.y > 0.0 { (p.y - min.y) / size.y } else { 0.0 };
        let [tl, tr, br, bl] = w.map(|[x, y]| PVec::new(x, y));
        p + tl * ((1.0 - u) * (1.0 - v)) + tr * (u * (1.0 - v)) + br * (u * v) + bl * ((1.0 - u) * v)
    }

    fn warps(&self) -> bool {
        self.warp.is_some_and(|w| w.iter().flatten().any(|v| *v != 0.0))
    }
}

/// Where a canvas committed to the editor is: its box's corners (top left,
/// top right, bottom right, bottom left) as the edit left them, moved with
/// the canvas since.
#[derive(Clone, Debug)]
pub struct CommitMark {
    pub name: String,
    pub corners: [PVec; 4],
}

impl CommitMark {
    /// The corners moved as a path is by `f`.
    pub(super) fn transform(&mut self, f: &impl Fn(&mut StrokePath)) {
        let [a, b, c, d] = self.corners;
        let mut path = StrokePath::new().move_to(a.x, a.y).line_to(b.x, b.y).line_to(c.x, c.y).line_to(d.x, d.y);
        f(&mut path);
        let points = path.points_within();
        if let [a, b, c, d] = points[..] {
            self.corners = [a, b, c, d];
        }
    }
}

/// A canvas committed in a run: its name, the script line, its box when
/// committed, `[x, y, width, height]` (none if it was empty), and the
/// box's corners once edited (moved, turned, scaled and skewed, not
/// warped).
#[derive(Clone, Debug, Serialize)]
pub struct Committed {
    pub name: String,
    pub line: Option<usize>,
    #[serde(rename = "box")]
    pub bounds: Option<[f32; 4]>,
    pub local: Option<[[f32; 2]; 4]>,
    /// What it's drawn from, if it's a symbolic.
    pub symbolic: Option<super::symbolic::SymbolicReport>,
}

/// A committed canvas, as `Committed` has it, and where it ended up: each
/// page it's on, with its box's corners there. (Its fields are spelled
/// out, not flattened: serde_wasm_bindgen gives a flattened struct to
/// JavaScript as a Map.)
#[derive(Clone, Debug, Serialize)]
pub struct CommitReport {
    pub name: String,
    pub line: Option<usize>,
    #[serde(rename = "box")]
    pub bounds: Option<[f32; 4]>,
    pub local: Option<[[f32; 2]; 4]>,
    pub symbolic: Option<super::symbolic::SymbolicReport>,
    pub shown: Vec<Shown>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Shown {
    pub page: usize,
    pub corners: [[f32; 2]; 4],
}

/// What a run has of the editor's: the edits saved, and the canvases
/// committed so far.
#[derive(Debug, Default)]
pub struct EditorState {
    pub saved: Edits,
    pub committed: Vec<Committed>,
}

pub(super) type SharedEditor = Rc<RefCell<EditorState>>;

/// `canvas.commit_to_editor(name)`.
pub(super) fn register(engine: &mut Engine) -> SharedEditor {
    let editor: SharedEditor = Rc::default();
    let state = editor.clone();
    engine.register_fn("commit_to_editor", move |ctx: NativeCallContext, canvas: &mut Canvas, name: Dynamic| -> RhaiResult<Canvas> {
        let name = check_name("commit_to_editor", name)?;
        commit_as(&ctx, &state, canvas, name, None)
    });
    editor
}

/// Commit `canvas` as `name`: the saved edit applied, and noted for the
/// editor, with what it's drawn from if it's a symbolic's.
pub(super) fn commit_as(ctx: &NativeCallContext, editor: &SharedEditor, canvas: &mut Canvas, name: String, symbolic: Option<super::symbolic::SymbolicReport>) -> RhaiResult<Canvas> {
    const CALLER: &str = "commit_to_editor";
    let mut state = editor.borrow_mut();
    if state.committed.iter().any(|c| c.name == name) {
        return Err(format!("{}: '{}' is committed already; give each canvas a name of its own", CALLER, name).into());
    }
    let edit = state.saved.canvases.get(&name).cloned().unwrap_or_default();
    let mut committed = None;
    let canvas = canvas.update(|data| committed = Some(commit(data, &name, &edit)));
    let (bounds, local) = committed.expect("the canvas was committed");
    state.committed.push(Committed { name, line: ctx.call_position().line(), bounds, local, symbolic });
    Ok(canvas)
}

fn pair(p: PVec) -> [f32; 2] {
    [p.x, p.y]
}

/// Commit `data` as `name`, applying `edit`: its box when committed, and
/// the corners the edit took it to.
fn commit(data: &mut CanvasData, name: &str, edit: &CanvasEdit) -> (Option<[f32; 4]>, Option<[[f32; 2]; 4]>) {
    let bounds = JournalCanvas::from(&*data).bounds();
    let has_box = bounds.mins.x <= bounds.maxs.x && bounds.mins.y <= bounds.maxs.y;
    let (min, max) = (bounds.mins, bounds.maxs);
    let corners = [min, PVec::new(max.x, min.y), max, PVec::new(min.x, max.y)];
    let affine = edit.affine(bounds.center());
    if has_box {
        if edit.warps() {
            let warp = |p: PVec| edit.warp_point(min, max, p);
            for stroke in &mut data.strokes {
                stroke.path.map_points(warp);
            }
            for mark in &mut data.commits {
                mark.corners = mark.corners.map(warp);
            }
        }
        if !affine.is_identity() {
            for stroke in &mut data.strokes {
                stroke.path.map_points(|p| affine.apply(p));
                stroke.style.move_paint(&affine);
            }
            for mark in &mut data.commits {
                mark.corners = mark.corners.map(|p| affine.apply(p));
            }
        }
    }
    if let Some(ink) = &edit.ink {
        for stroke in &mut data.strokes {
            if let Some(color) = &ink.color {
                stroke.style.recolor(color);
            }
            if let Some(opacity) = ink.opacity {
                stroke.style.fade(opacity);
            }
            if let Some(weight) = ink.weight {
                stroke.style.weigh(weight as f64);
            }
        }
    }
    for stroke in &mut data.strokes {
        stroke.hidden |= edit.hidden;
        stroke.commits = Some(match &stroke.commits {
            Some(inner) => Arc::from(format!("{} {}", inner, name)),
            None => Arc::from(name),
        });
    }
    if !has_box {
        return (None, None);
    }
    let edited = corners.map(|p| affine.apply(p));
    data.commits.push(CommitMark { name: name.to_string(), corners: edited });
    let extents = bounds.extents();
    (Some([min.x, min.y, extents.x, extents.y]), Some(edited.map(pair)))
}

/// Where each canvas committed in a run that made `entry` ended up.
pub fn report(entry: &Entry, committed: &[Committed]) -> Vec<CommitReport> {
    committed.iter().map(|c| {
        let shown = entry.pages.iter().enumerate().flat_map(|(page, p)| {
            p.canvases.iter().flat_map(|canvas| &canvas.commits)
                .filter(|mark| mark.name == c.name)
                .map(move |mark| Shown { page, corners: mark.corners.map(pair) })
        }).collect();
        CommitReport { name: c.name.clone(), line: c.line, bounds: c.bounds, local: c.local, symbolic: c.symbolic.clone(), shown }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::canvas::StrokeColor;
    use crate::journal::fonts::FontBook;
    use crate::journal::rhaientries::JournalEngine;

    fn engine(edits: &str) -> JournalEngine {
        let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_GLYPHSETS).expect("the glyphsets load");
        let mut engine = JournalEngine::new(Rc::new(symd));
        engine.set_edits(Edits::parse(edits).expect("the edits parse"));
        engine
    }

    const NONE: &str = r#"{ "version": 1 }"#;

    fn run(edits: &str, script: &str) -> (Entry, JournalEngine) {
        let mut engine = engine(edits);
        let entry = engine.build_entry_str(script).expect("the script runs");
        (entry, engine)
    }

    /// Every point of the first canvas on the first page, as it's drawn.
    fn points(entry: &Entry) -> Vec<PVec> {
        entry.pages[0].canvases()[0].strokes.iter().flat_map(|s| s.path.points_within()).collect()
    }

    fn close(a: PVec, b: PVec) -> bool {
        (a - b).length() < 1e-3
    }

    // A 20 by 10 box from (0, 0), drawn without width.
    const BOX: &str = r#"
        let pen = new_style(new_brush_style("black", "Ink Pen", 0.0001), new_handwriting_style());
        let c = new_canvas().add(new_rectangle(vec2(10, 5), 20, 10), pen);
    "#;

    #[test]
    fn edits_parse_and_are_checked() {
        let edits = Edits::parse(r#"{ "version": 1, "canvases": { "a": { "move": [1, 2], "ink": { "color": "red" } } } }"#).unwrap();
        assert_eq!(edits.canvases["a"].offset, [1.0, 2.0]);
        assert_eq!(edits.canvases["a"].scale, [1.0, 1.0], "parts left out are as they were");
        assert!(Edits::parse(r#"{ "version": 2 }"#).unwrap_err().contains("version 2"));
        assert!(Edits::parse(r#"{ "version": 1, "canvases": { "a": { "spin": 3 } } }"#).is_err());
        assert!(Edits::parse(r#"{ "version": 1, "canvases": { "a": { "scale": [0, 1] } } }"#).unwrap_err().contains("can't be 0"));
        assert!(Edits::parse(r#"{ "version": 1, "canvases": { "a": { "ink": { "color": "nope" } } } }"#).is_err());
        assert!(Edits::parse(r#"{ "version": 1, "canvases": { "a b": {} } }"#).is_err());
    }

    #[test]
    fn without_an_edit_a_commit_changes_nothing() {
        let script = format!("{}\nc.commit_to_editor(\"box\"); journal_entry.add(new_page().add(c));", BOX);
        let plain = format!("{}\njournal_entry.add(new_page().add(c));", BOX);
        assert_eq!(points(&run(NONE, &script).0), points(&run(NONE, &plain).0));
    }

    #[test]
    fn edits_move_turn_and_scale_about_the_box_centre() {
        let script = format!("{}\nc.commit_to_editor(\"box\"); journal_entry.add(new_page().add(c));", BOX);
        let (entry, _) = run(r#"{ "version": 1, "canvases": { "box": { "move": [100, 0], "turn": 90, "scale": [2, 1] } } }"#, &script);
        let pts = points(&entry);
        // (0, 0) is 10 left and 5 up of the centre (10, 5): twice as far
        // left, then turned a quarter clockwise, then moved 100 right.
        let centre = PVec::new(10.0, 5.0);
        let expected = Affine::rotate(90.0).apply(PVec::new(-20.0, -5.0)) + centre + PVec::new(100.0, 0.0);
        assert!(pts.iter().any(|p| close(*p, expected)), "{:?} in {:?}", expected, pts);
    }

    #[test]
    fn a_warp_moves_each_corner_by_its_own() {
        let script = format!("{}\nc.commit_to_editor(\"box\"); journal_entry.add(new_page().add(c));", BOX);
        let (entry, _) = run(r#"{ "version": 1, "canvases": { "box": { "warp": [[0, 0], [5, -5], [0, 0], [0, 0]] } } }"#, &script);
        let pts = points(&entry);
        // The box's bounds are a hair wider than its line.
        assert!(pts.iter().any(|p| (p.x - 25.0).abs() < 0.01 && (p.y + 5.0).abs() < 0.01), "{:?}", pts);
        assert!(pts.iter().any(|p| p.x.abs() < 0.01 && (p.y - 10.0).abs() < 0.01), "the bottom left stays");
    }

    #[test]
    fn hidden_canvases_keep_their_bounds_but_draw_nothing() {
        let script = format!(r#"{}
            c.commit_to_editor("box");
            let b = c.bounds();
            assert(b.width > 19.0, "still as wide");
            journal_entry.add(new_page().add(c));"#, BOX);
        let (entry, _) = run(r#"{ "version": 1, "canvases": { "box": { "hidden": true } } }"#, &script);
        assert!(entry.pages[0].rendered_canvases().next().is_none(), "nothing to render");
    }

    #[test]
    fn ink_edits_recolour_fade_and_weigh() {
        let script = format!("{}\nc.commit_to_editor(\"box\"); journal_entry.add(new_page().add(c));", BOX);
        let (entry, _) = run(r##"{ "version": 1, "canvases": { "box": { "ink": { "color": "#ff0000", "opacity": 0.5, "weight": 2 } } } }"##, &script);
        let canvas = &entry.pages[0].canvases()[0];
        let style = &canvas.strokes[0].style;
        let StrokeColor::Solid(color) = style.color() else { panic!("a solid colour") };
        let [r, g, b, a] = parse_color(color).unwrap();
        assert!([r, g, b] == [1.0, 0.0, 0.0] && (a - 0.5).abs() < 0.01, "{}", color);
        assert!((style.brush_size() - 0.0002).abs() < 1e-7);
    }

    #[test]
    fn paints_across_the_canvas_move_with_it() {
        // A gradient from left to right across the box, and a picture on it.
        let script = r##"
            let g = new_gradient(["#ff0000", "#0000ff"], #{ from: vec2(0, 0), to: vec2(20, 0) });
            let pen = new_style(new_brush_style(g, "Ink Pen", 0.0001), new_handwriting_style());
            let c = new_canvas().add(new_line(vec2(0, 5), vec2(20, 5)), pen);
            c.commit_to_editor("box");
            journal_entry.add(new_page().add(c));"##;
        let colour_at = |entry: &Entry, p: PVec| {
            let StrokeColor::Gradient(g) = entry.pages[0].canvases()[0].strokes[0].style.color().clone() else { panic!("a gradient") };
            g.at_point(p).unwrap()
        };
        let (plain, _) = run(NONE, script);
        for edit in [r#"{ "move": [50, 30] }"#, r#"{ "turn": 90 }"#, r#"{ "scale": [3, -1] }"#, r#"{ "skew": 30, "turn": 20 }"#] {
            let edits = format!(r#"{{ "version": 1, "canvases": {{ "box": {} }} }}"#, edit);
            let (moved, _) = run(&edits, script);
            let e = Edits::parse(&edits).unwrap().canvases["box"].affine(PVec::new(10.0, 5.0));
            for x in [0.0, 5.0, 15.0, 20.0] {
                let p = PVec::new(x, 5.0);
                let (a, b) = (colour_at(&plain, p), colour_at(&moved, e.apply(p)));
                assert!(a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-3), "{}: {:?} vs {:?} at {}", edit, a, b, x);
            }
        }
        // Script transforms leave the paint where it is, as they always have.
        let (shifted, _) = run(NONE, &script.replace("c.commit_to_editor(\"box\");", "c.translate_by(vec2(10, 0));"));
        let StrokeColor::Gradient(g) = shifted.pages[0].canvases()[0].strokes[0].style.color().clone() else { panic!() };
        assert!(g.transform.is_identity());
    }

    #[test]
    fn radial_gradients_and_pictures_move_too() {
        use crate::journal::gradient::{Gradient, GradientKind};
        use crate::journal::image_paint::ImagePaint;
        let edit = CanvasEdit { offset: [5.0, -3.0], turn: 30.0, scale: [2.0, 0.5], skew: 10.0, ..Default::default() };
        let m = edit.affine(PVec::new(4.0, 4.0));
        let radial = Gradient { stops: vec![(0.0, [1.0, 0.0, 0.0, 1.0]), (1.0, [0.0, 0.0, 1.0, 1.0])], kind: GradientKind::Radial { center: PVec::new(3.0, 3.0), radius: 5.0 }, transform: Affine::IDENTITY };
        let moved = radial.moved(&m);
        let pixels = image::RgbaImage::from_fn(4, 4, |x, y| image::Rgba([x as u8 * 60, y as u8 * 60, 0, 255]));
        let picture = ImagePaint::new("p".into(), Arc::new(pixels), PVec::ZERO, PVec::new(8.0, 8.0));
        let moved_picture = picture.moved(&m);
        for p in [PVec::new(1.0, 2.0), PVec::new(6.0, 3.5), PVec::new(4.0, 7.0)] {
            assert!((radial.t_at(p).unwrap() - moved.t_at(m.apply(p)).unwrap()).abs() < 1e-4);
            let (a, b) = (picture.at_point(p), moved_picture.at_point(m.apply(p)));
            assert!(a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-3), "{:?} vs {:?}", a, b);
        }
    }

    #[test]
    fn a_name_can_be_committed_once_a_run() {
        let mut engine = engine(NONE);
        let err = engine.build_entry_str(&format!("{}\nc.commit_to_editor(\"box\"); c.copy().commit_to_editor(\"box\");", BOX)).unwrap_err();
        assert!(err.contains("committed already"), "{}", err);
        assert!(engine.build_entry_str("new_canvas().commit_to_editor(\"a b\");").unwrap_err().contains("invalid name"));
    }

    #[test]
    fn marks_follow_the_canvas_onto_its_pages() {
        let script = format!(r#"{}
            c.commit_to_editor("box");
            let outer = new_canvas().add(c).translate_by(vec2(100, 0));
            outer.commit_to_editor("outer");
            journal_entry.add(new_page());
            journal_entry.add(new_page().add(outer).add(c.copy().scale_by(2)));"#, BOX);
        let (entry, engine) = run(NONE, &script);
        let reports = report(&entry, &engine.committed());
        let boxed = reports.iter().find(|r| r.name == "box").unwrap();
        let line = script.lines().position(|l| l.contains("c.commit_to_editor(\"box\")")).unwrap() + 1;
        assert_eq!(boxed.line, Some(line));
        assert_eq!(boxed.shown.len(), 2, "in the outer canvas, and copied and scaled");
        assert!(boxed.shown.iter().all(|s| s.page == 1));
        let [x, y, ..] = boxed.bounds.unwrap();
        let moved = boxed.shown[0].corners[0];
        assert!((moved[0] - (x + 100.0)).abs() < 1e-3 && (moved[1] - y).abs() < 1e-3);
        let scaled = boxed.shown[1].corners[0];
        assert!((scaled[0] - x * 2.0).abs() < 1e-3);
        let outer = reports.iter().find(|r| r.name == "outer").unwrap();
        assert_eq!(outer.shown.len(), 1);
        assert!(engine.unused_edits().is_empty());
    }

    #[test]
    fn edits_draw_nothing_from_the_generator() {
        let script = format!(r#"{}
            c.commit_to_editor("box");
            journal_entry.add(new_page().add(c));
            let r = random();
            journal_entry.add(new_page().add(new_canvas().add(new_line(vec2(0, 0), vec2(r * 100, 0)), pen)));"#, BOX);
        let last = |entry: &Entry| entry.pages[1].canvases()[0].strokes[0].path.points_within();
        let (plain, _) = run(NONE, &script);
        let (edited, _) = run(r#"{ "version": 1, "canvases": { "box": { "move": [5, 5], "turn": 10, "ink": { "opacity": 0.5 } } } }"#, &script);
        assert_eq!(last(&plain), last(&edited));
    }

    #[test]
    fn committed_strokes_are_grouped_in_svg() {
        let script = format!(r#"{}
            c.commit_to_editor("box");
            journal_entry.add(new_page().add(new_canvas().add(new_line(vec2(0, 0), vec2(5, 5)), pen).add(c)));"#, BOX);
        let (entry, _) = run(NONE, &script);
        let (svg, _) = crate::journal::publish::page_svg(&entry.pages[0], "page_0").unwrap();
        assert_eq!(svg.matches("<g data-commit=\"box\">").count(), 1, "{}", svg);
        let (plain, _) = run(NONE, &script.replace("c.commit_to_editor(\"box\");", ""));
        assert!(!crate::journal::publish::page_svg(&plain.pages[0], "page_0").unwrap().0.contains("data-commit"));
    }

    #[test]
    fn unused_edits_are_named() {
        let (_, engine) = run(r#"{ "version": 1, "canvases": { "gone": { "move": [1, 1] } } }"#, "journal_entry.add(new_page());");
        assert_eq!(engine.unused_edits(), vec!["gone".to_string()]);
    }
}
