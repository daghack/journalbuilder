use rhai::{Engine, EvalAltResult, ImmutableString, Array, Map, Dynamic, CustomType, TypeBuilder, Scope};
use std::io::{Error, ErrorKind};
use super::paths::StrokePath;
use super::symboldict::SymbolPathDict;
use super::canvas::Canvas as JournalCanvas;
use super::canvas::StrokeStyle as JournalStrokeStyle;
use super::canvas::CanvasStyleOptions;
use super::canvas::Brush;
use super::symbol::{place_along_path, SymbolPlacementArgs};
use super::styles;
use super::canvas::artistic::{BuiltMaze, MazeTraversal, MazeTraversalStrategy};
use super::darkly;
use ::darkly::engine::DarklyEngine;
use crate::composite_from_locktypes;
use crate::lockbuilder::{Lock, LockConfig, InfoVarType, CompositeLock};
use crate::lockbuilder::locks::doublering::{DoubleRingLock, DoubleRingOptions, KeyedDoubleRingLock, KeyedDoubleRingOptions};
use crate::lockbuilder::locks::util::mergealpha::{MergeAlphaInputs, MergeAlpha26Options};
use crate::lockbuilder::locks::util::splitsymbols::{SplitSymbolsLock, SplitSymbolsOptions};
use crate::lockbuilder::locks::shift::{ShiftCipherLock, ShiftCipherOptions, VignCipherLock, VignCipherOptions};
use crate::lockbuilder::locks::huffman::{HuffmanEncodingLock, HuffmanEncodingOptions};
use crate::lockbuilder::locks::substitution::{SubstitutionCipherLock, SubstitutionCipherOptions};
use parry2d::math::Vec2 as PVec;

use std::sync::Arc;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug, Default, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Entry {
    pub metadata: EntryMetadata,
    pages: Vec<Page>,
}

impl Entry {
    fn add_page(&mut self, page: Page) {
        self.pages.push(page);
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder.with_fn("add_page", Self::add_page);
    }
    pub fn darkly_publish_pngs(self, engine: &mut DarklyEngine, dirname: &str, scale: f32) {
        for (i, page) in self.pages.into_iter().enumerate() {
            let canvases = page.canvases();
            for (canvas_i, canvas) in canvases.into_iter().enumerate() {
                let canvas : JournalCanvas = canvas.into();
                let canvas_dim = canvas.bounds().extents();
                darkly::resize_canvas(engine, (canvas_dim.x * scale) as u32, (canvas_dim.y * scale) as u32);
                // Every rhai stroke style carries a brush, so no default is needed.
                let buff = darkly::render_canvas(&canvas, engine, None);
                let filename = format!("{}/darkly_page_{}_{}.png", dirname, i, canvas_i);
                buff.save(filename).expect("Failed to save png");
            }
        }
    }
    pub fn publish_pngs(self, dirname: &str) {
        println!("Publishing Svgs:");
        for (i, page) in self.pages.into_iter().enumerate() {
            let page_canvas: JournalCanvas = page.into();
            let canvas_style = CanvasStyleOptions {
                roughness: 1.0,
                point_sample_distance: 1.0,
                trail_off_percent: 0.15,
                noise_amplitude: 0.3,
            };
            let svg = page_canvas.to_png(2.0, Some(&canvas_style));
            let filename = format!("{}/page_{}.png", dirname, i);
            println!("\t - {}", filename);
            let failmsg = format!("Failed to write page: {}", i);
            std::fs::write(filename, svg).expect(&failmsg);
        }
    }
}

#[derive(Debug, Default, Clone, CustomType)]
pub struct EntryMetadata {
}

#[derive(Debug, Default, Clone)]
pub enum LayoutStyle {
    #[default]
    Layer
}

#[derive(Debug, Default, Clone, CustomType)]
pub struct BrushSettings {
    brush_style: ImmutableString,
    min_pressure: f64,
    max_pressure: f64,
    size: f64,
}

impl BrushSettings {
    /// `new_brush` is registered in `JournalEngine::new`, which checks the
    /// name against Darkly's brush library.
    pub fn new_brush(brush_style: &str, min_pressure: f64, max_pressure: f64, size: f64) -> Self {
        Self {
            brush_style: brush_style.into(),
            min_pressure,
            max_pressure,
            size,
        }
    }
}

impl From<BrushSettings> for Brush {
    fn from(bs: BrushSettings) -> Brush {
        Brush {
            name: bs.brush_style.to_string(),
            size: bs.size as f32,
            min_pressure: bs.min_pressure as f32,
            max_pressure: bs.max_pressure as f32,
        }
    }
}

#[derive(Debug, Default, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Page {
    pub metadata: PageMetadata,
    canvases: Vec<Canvas>,
    layout: LayoutStyle,
}

impl Page {
    pub fn add_canvas(&mut self, canvas: Canvas) {
        self.canvases.push(canvas)
    }
    pub fn canvases(&self) -> &Vec<Canvas> {
        &self.canvases
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("add_canvas", Self::add_canvas)
            .with_fn("new_page", Self::default)
        ;
    }
}

impl Into<JournalCanvas> for Page {
    fn into(self) -> JournalCanvas {
        match self.layout {
            LayoutStyle::Layer => {
                let mut combined = JournalCanvas::new();
                for canvas in self.canvases {
                    combined.merge_canvas(canvas.into());
                }
                combined
            }
        }
    }
}

#[derive(Debug, Default, Clone, CustomType)]
pub struct PageMetadata {
}

#[derive(Debug, Default, Clone)]
#[derive(CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Vec2{ x, y }
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_vec", Self::new)
        ;
    }
}

impl Into<PVec> for Vec2 {
    fn into(self) -> PVec {
        PVec::new(self.x as f32, self.y as f32)
    }
}

#[derive(Debug, Default, Clone)]
#[derive(CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Canvas {
    pub strokes: Vec<Stroke>,
}

impl Canvas {
    pub fn add_stroke(&mut self, stroke: Stroke) {
        self.strokes.push(stroke)
    }
    pub fn add_strokepath(&mut self, path: StrokePath, style: StrokeStyle) {
        self.strokes.push(Stroke{path, style});
    }
    pub fn add_canvas(&mut self, lhs: Canvas) {
        for stroke in lhs.strokes {
            self.strokes.push(stroke);
        }
    }
    pub fn translate(&mut self, v: Vec2) {
        let v = v.into();
        for stroke in &mut self.strokes {
            stroke.path.translate_by(v);
        }
    }
    pub fn scale(&mut self, scale: f64) {
        for stroke in &mut self.strokes {
            stroke.path.scale_by(scale as f32);
        }
    }
    pub fn rotate_around(&mut self, point: Vec2, rad: f64) {
        let point = point.into();
        for stroke in &mut self.strokes {
            stroke.path.rotate_around(point, rad as f32);
        }
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("add_stroke", Self::add_stroke)
            .with_fn("add_strokepath", Self::add_strokepath)
            .with_fn("rotate_around", Self::rotate_around)
            .with_fn("translate", Self::translate)
            .with_fn("scale", Self::scale)
            .with_fn("add_canvas", Self::add_canvas)
            .with_fn("new_canvas", Self::default)
        ;
    }
}

impl Into<JournalCanvas> for Canvas {
    fn into(self) -> JournalCanvas {
        let mut canvas = JournalCanvas::new();
        for stroke in self.strokes {
            let Stroke {
                path,
                style,
            } = stroke;
            canvas.add_strokes(path, style.into());
        }
        canvas
    }
}

impl Into<JournalCanvas> for &Canvas {
    fn into(self) -> JournalCanvas {
        let mut canvas = JournalCanvas::new();
        for stroke in self.strokes.iter() {
            let Stroke {
                path,
                style,
            } = stroke;
            canvas.add_strokes(path.clone(), style.clone().into());
        }
        canvas
    }
}

#[derive(Debug, Clone, CustomType)]
pub struct MarkdownContent {
}

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Stroke {
    path: StrokePath,
    style: StrokeStyle,
}

impl Stroke {
    pub fn new(path: StrokePath, style: StrokeStyle) -> Self {
        Self {
            path,
            style,
        }
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_stroke", Self::new)
        ;
    }
}

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct FontSet {
    upper: Array,
    lower: Array

}

impl FontSet {
    pub fn new(upper: Array, lower: Array) -> Self {
        Self { upper, lower }
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_fontset", Self::new)
        ;
    }
}

impl Into<styles::FontSet> for FontSet {
    fn into(self) -> styles::FontSet {
        let upper = self.upper.into_iter().map(Dynamic::cast).collect();
        let lower = self.lower.into_iter().map(Dynamic::cast).collect();
        styles::FontSet::new(upper, lower)
    }
}

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct StrokeStyle {
    width: f64,
    color: ImmutableString,
    #[rhai_type(skip)]
    brush: BrushSettings,
}

impl Into<JournalStrokeStyle> for StrokeStyle {
    fn into(self) -> JournalStrokeStyle {
        JournalStrokeStyle {
            width: self.width as f32,
            color: self.color.to_string(),
            brush: Some(self.brush.into()),
        }
    }
}

impl StrokeStyle {
    pub fn new(width: f64, color: ImmutableString, brush: BrushSettings) -> Result<Self, Box<EvalAltResult>> {
        super::canvas::parse_color(&color)?;
        Ok(Self {
            width,
            color,
            brush,
        })
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_strokestyle", Self::new)
        ;
    }
}

pub struct JournalEngine {
    engine: Engine,
}

impl JournalEngine {
    pub fn new(symdict: Arc<SymbolPathDict>) -> Self {
        let mut engine = Engine::new();
        let fit_msg_symdict = symdict.clone();
        let fill_msg_symdict = symdict.clone();
        let lockset = Rc::new(RefCell::new(vec![]));

        engine.register_type::<StrokePath>()
            .register_fn("move_to", |path: StrokePath, pos: Vec2| -> StrokePath {
                path.move_to(pos.x as f32, pos.y as f32)
            })
            .register_fn("line_to", |path: StrokePath, pos: Vec2| -> StrokePath {
                path.line_to(pos.x as f32, pos.y as f32)
            })
            .register_fn("fill_with_msg", move |path: &mut StrokePath, style: StrokeStyle, fonts: FontSet, fontsize: f64, msg: &str| -> Vec<Dynamic> {
                let symdict = &fill_msg_symdict;
                let fontset : styles::FontSet = fonts.into();
                let builder = fontset.into_symbol_string_builder(&symdict);
                let args = SymbolPlacementArgs{
                    avoid_kdtree: None, 
                    symbol_scale: fontsize as f32,
                };

                let mut all_canvases = Vec::new();
                let mut symbols = builder.build(msg);
                while symbols.len() > 0 {
                    let (placed, unplaced) = place_along_path(symbols.drain(..).collect(), path, &args);
                    let mut canvas = Canvas::default();
                    for symbol in placed {
                        canvas.add_strokepath(symbol.placed_strokepath(), style.clone());
                    }
                    all_canvases.push(Dynamic::from(canvas));
                    symbols = unplaced;
                }
                all_canvases
            })
            .register_fn("fit_msg_to",
                move |path: &mut StrokePath, style: StrokeStyle, fonts : FontSet, msg: &str| -> Canvas {
                let mut canvas = Canvas::default();
                let symdict = &fit_msg_symdict;
                let fontset : styles::FontSet = fonts.into();
                let builder = fontset.into_symbol_string_builder(&symdict);
                let symbols = builder.build(msg);
                let mut args = SymbolPlacementArgs{
                    avoid_kdtree: None,
                    symbol_scale: 20.0,
                };
                let mut window_min = 0.1;
                let mut window_max = 1000.0;
                while window_max - window_min > 0.005 {
                    let current_scale = (window_max + window_min) / 2.0;
                    args.symbol_scale = current_scale;
                    let (_, unplaced) = place_along_path(symbols.clone(), path, &args);
                    if unplaced.len() > 0 {
                        window_max = current_scale;
                    } else {
                        window_min = current_scale;
                    }
                }
                args.symbol_scale = window_min;
                let (placed, _) = place_along_path(symbols, path, &args);
                for symbol in placed {
                    canvas.add_strokepath(symbol.placed_strokepath(), style.clone());
                }
                canvas
            })
            .register_fn("scale_by", |stroke: &mut StrokePath, scale: f64 | {
                stroke.scale_by(scale as f32);
            })
            .register_fn("translate_by", |stroke: &mut StrokePath, translate: Vec2 | {
                stroke.translate_by(translate.into());
            })
            .register_fn("rotate_around",
                |stroke: &mut StrokePath, rot_point: Vec2, rot: f64 | {
                    stroke.rotate_around(rot_point.into(), rot as f32);
                },
            )
            .register_fn("left_handed_maze_path", |seed: i64, width: i64, height: i64| -> StrokePath {
                let seed = seed as u64;
                let width = width as u64;
                let height = height as u64;
                let maze = BuiltMaze::<[i64; 1]>::new(seed, width as u32, height as u32);
                let strategy = MazeTraversalStrategy::LeftHanded;
                let traversal = MazeTraversal::new((0,0), strategy);
                let mut path = StrokePath::new();
                let mut current = (0, 0);
                let mut visited = std::collections::HashSet::<(i32, i32)>::new();
                for (step, _) in traversal.into_iter(&maze) {
                    if maze.are_neighbors(&current, &step) {
                        path = path.line_to(step.0 as f32, step.1 as f32);
                    } else {
                        let neighbors = maze.neighbors_of(&step).unwrap();
                        let mut visited_neighbor = None;
                        for neighbor in neighbors {
                            if visited.contains(&neighbor) {
                                visited_neighbor = Some(neighbor);
                                break;
                            }
                        }
                        match visited_neighbor {
                            None => {
                                path = path
                                    .move_to(step.0 as f32, step.1 as f32)
                            }
                            Some(n) => {
                                path = path
                                    .move_to(n.0 as f32, n.1 as f32)
                                    .line_to(step.0 as f32, step.1 as f32);
                            }
                        }
                    }
                    visited.insert(step);
                    current = step;
                }
                path
            })
            .register_fn("new_strokepath", StrokePath::new);

        let load_symdict = symdict.clone();
        engine.register_fn("load_symbol", move |symname: &str| -> Result<StrokePath, Box<EvalAltResult>> {
            load_symdict.get_strokepath(symname)
                .ok_or_else(|| format!("load_symbol: unknown symbol '{}'", symname).into())
        });

        // Validate brush names when the script runs, rather than failing
        // mid-render when Darkly can't find the brush.
        let brush_names: Vec<String> = ::darkly::brush::builtin_brushes::all()
            .into_iter()
            .map(|brush| brush.metadata.name)
            .collect();
        engine.register_fn("new_brush", move |name: &str, min_pressure: f64, max_pressure: f64, size: f64| -> Result<BrushSettings, Box<EvalAltResult>> {
            if !brush_names.iter().any(|known| known == name) {
                return Err(format!("new_brush: unknown brush '{}'; available: {}", name, brush_names.join(", ")).into());
            }
            Ok(BrushSettings::new_brush(name, min_pressure, max_pressure, size))
        });

        let new_lock_lockset = lockset.clone();
        engine.register_fn("lock_inputs", move |lock_index: &i64| -> Map {
            let mut inputs = Map::new();
            if let Some(lock) = new_lock_lockset.borrow().get(*lock_index as usize) {
                Lock::inputs(lock).iter().for_each(|(var_name, var_type)| {
                    let str = match var_type {
                        InfoVarType::String => {
                            "String".into()
                        }
                        InfoVarType::BaseNSymbol(symbol_count) => {
                            format!("Base{}Symbol", symbol_count)
                        }
                        _ => {
                            "Unknown".into()
                        }
                    };
                    inputs.insert(var_name.into(), str.into());
                });
            }
            inputs
        });

        let new_lock_lockset = lockset.clone();
        engine.register_fn("new_lock", move |lock_config: &str| -> i64 {
            let lock_config: LockConfig = serde_json::from_str(lock_config).expect("Failed to parse lock definition");
            let lock = composite_from_locktypes!(lock_config,
                "shift_cipher", ShiftCipherLock, ShiftCipherOptions,
                "vigenere_cipher", VignCipherLock, VignCipherOptions,
                "huffman_encoding", HuffmanEncodingLock, HuffmanEncodingOptions,
                "doublering", DoubleRingLock, DoubleRingOptions,
                "keyed_doublering", KeyedDoubleRingLock, KeyedDoubleRingOptions,
                "merge_alpha", MergeAlphaInputs, MergeAlpha26Options,
                "split_symbols", SplitSymbolsLock, SplitSymbolsOptions,
                "substitution_cipher", SubstitutionCipherLock, SubstitutionCipherOptions,
            ).expect("Failed to create lock from config.");
            let mut lockset = new_lock_lockset.borrow_mut();
            let index = lockset.len();
            lockset.push(lock);
            index as i64
        });
        engine
            .build_type::<FontSet>()
            .build_type::<Vec2>()
            .build_type::<EntryMetadata>()
            .build_type::<Entry>()
            .build_type::<PageMetadata>()
            .build_type::<Page>()
            .build_type::<Canvas>()
            .build_type::<BrushSettings>()
            .build_type::<MarkdownContent>()
            .build_type::<StrokeStyle>()
            .build_type::<Stroke>();

        JournalEngine {
            engine,
        }
    }
    pub fn build_entry_str(&mut self, script_str: &str) -> Result<Entry, Error> {
        let mut scope = Scope::new();
        scope.push("journal_entry", Entry::default());
        self.engine.run_with_scope(&mut scope, script_str)
            .map_err(|err| {
                let msg = format!("Failed to Evaluate Rhai: {}", err);
                Error::new(ErrorKind::Other, msg)
            })?;
        scope.get_value("journal_entry")
            .ok_or(Error::new(ErrorKind::Other, "journal_entry value not found"))

    }
    pub fn build_entry_file(&mut self, rhai_filename: &str) -> Result<Entry, Error> {
        let file_contents = std::fs::read_to_string(rhai_filename)?;
        self.build_entry_str(&file_contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> JournalEngine {
        let symd = SymbolPathDict::from_file("config/symbols.json").expect("Failed to load symbols.json");
        JournalEngine::new(Arc::new(symd))
    }

    const BRUSH: &str = r#"let brush = new_brush("Calligraphy", 0.25, 0.5, 1.0);"#;

    #[test]
    fn new_strokestyle_accepts_css_colors() {
        let script = format!(r#"{}
            let a = new_strokestyle(1.5, "darkred", brush);
            let b = new_strokestyle(1.5, "hsl(160, 50%, 40%, 0.8)", brush);
        "#, BRUSH);
        assert!(engine().build_entry_str(&script).is_ok());
    }

    #[test]
    fn new_strokestyle_rejects_bad_color_with_line() {
        let script = format!("{}\nlet bad = new_strokestyle(1.5, \"darkredd\", brush);", BRUSH);
        let err = engine().build_entry_str(&script).err().expect("bad color should fail").to_string();
        assert!(err.contains("darkredd"), "{}", err);
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn load_symbol_rejects_unknown_name_with_line() {
        let err = engine().build_entry_str("let ok = load_symbol(\"basic_eye\");\nlet bad = load_symbol(\"no_such_glyph\");")
            .err().expect("unknown symbol should fail").to_string();
        assert!(err.contains("unknown symbol 'no_such_glyph'"), "{}", err);
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn new_brush_rejects_unknown_name_with_line() {
        let err = engine().build_entry_str("let ok = new_brush(\"Ink Pen\", 0.2, 0.5, 1.0);\nlet bad = new_brush(\"Crayon\", 0.2, 0.5, 1.0);")
            .err().expect("unknown brush should fail").to_string();
        assert!(err.contains("unknown brush 'Crayon'"), "{}", err);
        assert!(err.contains("Calligraphy"), "error should list available brushes: {}", err);
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn new_strokestyle_requires_brush() {
        let err = engine().build_entry_str(r#"let s = new_strokestyle(1.5, "black");"#)
            .err().expect("brushless style should fail").to_string();
        assert!(err.contains("new_strokestyle"), "{}", err);
    }

    #[test]
    fn per_stroke_brushes_reach_journal_canvas() {
        let script = r#"
            let ink = new_strokestyle(1.5, "black", new_brush("Rough Ink", 0.25, 0.5, 1.0));
            let calli = new_strokestyle(1.5, "darkred", new_brush("Calligraphy", 1.0, 1.0, 0.8));
            let path = new_strokepath().move_to(new_vec(0.0, 0.0)).line_to(new_vec(10.0, 10.0));
            let canvas = new_canvas();
            canvas.add_strokepath(path, ink);
            canvas.add_strokepath(path, calli);
            let page = new_page();
            page.add_canvas(canvas);
            journal_entry.add_page(page);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let jc: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        let brushes: Vec<_> = jc.strokes().iter()
            .map(|s| s.style.brush.clone().expect("rhai strokes always have a brush"))
            .map(|b| (b.name, b.size))
            .collect();
        assert_eq!(brushes, vec![("Rough Ink".to_string(), 1.0), ("Calligraphy".to_string(), 0.8)]);
    }
}
