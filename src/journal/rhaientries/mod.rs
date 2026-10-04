use rhai::{Engine, ImmutableString, Array, Map, Dynamic, CustomType, TypeBuilder, Scope};
use std::io::{Error, ErrorKind};
use super::paths::{StrokePath, PathCmd};
use super::symboldict::SymbolPathDict;
use super::canvas::Canvas as JournalCanvas;
use super::canvas::StrokeStyle as JournalStrokeStyle;
use super::canvas::CanvasStyleOptions;
use super::symbol::{place_along_path, SymbolPlacementArgs};
use super::styles;
use super::canvas::artistic::{BuiltMaze, MazeTraversal, MazeTraversalStrategy};
use super::darkly;
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
    pub fn publish_svgs(self, dirname: &str) {
        println!("Publishing Svgs:");
        for (i, page) in self.pages.into_iter().enumerate() {
            let page_canvas: JournalCanvas = page.into();
            let svg = page_canvas.to_svg(None);
            let filename = format!("{}/page_{}.svg", dirname, i);
            println!("\t - {}", filename);
            let failmsg = format!("Failed to write page: {}", i);
            std::fs::write(filename, svg).expect(&failmsg);
        }
    }
    pub fn darkly_publish_pngs(self, dirname: &str, scale: f32) {
        for (i, page) in self.pages.into_iter().enumerate() {
            let canvases = page.canvases();
            for (canvas_i, (canvas, brush_settings)) in canvases.into_iter().enumerate() {
                let canvas : JournalCanvas = canvas.into();
                let canvas_dim = canvas.bounds().extents();
                let mut engine = darkly::new_engine((canvas_dim.x * scale) as u32, (canvas_dim.y * scale) as u32);
                let br_size = brush_settings.size as f32;
                let pressure_bounds = (brush_settings.min_pressure as f32, brush_settings.max_pressure as f32);
                let buff = darkly::render_canvas(&canvas, &mut engine, brush_settings.brush_style.as_str(), br_size, pressure_bounds);
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
#[rhai_type(extra = Self::build_extra)]
pub struct BrushSettings {
    brush_style: ImmutableString,
    min_pressure: f64,
    max_pressure: f64,
    size: f64,
}

impl BrushSettings {
    pub fn new_brush(brush_style: &str, min_pressure: f64, max_pressure: f64, size: f64) -> Self {
        Self {
            brush_style: brush_style.into(),
            min_pressure,
            max_pressure,
            size,
        }
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_brush", Self::new_brush)
        ;
    }
}

#[derive(Debug, Default, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Page {
    pub metadata: PageMetadata,
    canvases: Vec<(Canvas, BrushSettings)>,
    layout: LayoutStyle,
}

impl Page {
    pub fn add_canvas(&mut self, canvas: Canvas, brush_settings: BrushSettings) {
        self.canvases.push((canvas, brush_settings))
    }
    pub fn canvases(&self) -> &Vec<(Canvas, BrushSettings)> {
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
                    combined.merge_canvas(canvas.0.into());
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

impl From<JournalCanvas> for Canvas {
    fn from(jc: JournalCanvas) -> Self {
        let mut canvas = Canvas::default();
        for stroke in jc.strokes().into_iter() {
            let path = stroke.path.clone();
            let style = StrokeStyle::from(stroke.style.clone());
            canvas.add_strokepath(path, style);
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
    color: ImmutableString
}

impl Into<JournalStrokeStyle> for StrokeStyle {
    fn into(self) -> JournalStrokeStyle {
        JournalStrokeStyle {
            width: self.width as f32,
            color: self.color.to_string(),
        }
    }
}

impl From<JournalStrokeStyle> for StrokeStyle {
    fn from(jss: JournalStrokeStyle) -> StrokeStyle {
        StrokeStyle {
            width: jss.width as f64,
            color: jss.color.into(),
        }
    }
}

impl StrokeStyle {
    pub fn new(width: f64, color: ImmutableString) -> Self {
        Self {
            width,
            color
        }
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_strokestyle", Self::new)
        ;
    }
}

pub struct JournalEngine {
    engine: Engine,
    lockset: Rc<RefCell<Vec<CompositeLock>>>,
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
                let style: JournalStrokeStyle = style.into();
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
                        let style: StrokeStyle = style.clone().into();
                        canvas.add_strokepath(symbol.placed_strokepath(), style);
                    }
                    all_canvases.push(Dynamic::from(canvas));
                    symbols = unplaced;
                }
                all_canvases
            })
            .register_fn("fit_msg_to",
                move |path: &mut StrokePath, style: StrokeStyle, fonts : FontSet, msg: &str| -> Canvas {
                let style: JournalStrokeStyle = style.into();
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
                    let style: StrokeStyle = style.clone().into();
                    canvas.add_strokepath(symbol.placed_strokepath(), style);
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
        engine.register_fn("load_symbol", move |symname: &str| -> StrokePath {
            load_symdict.get_strokepath(symname).expect("Unable to load Symbol")
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
            lockset,
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
