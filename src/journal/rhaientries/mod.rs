mod art;
mod avoid;
mod glyphs;
mod layout;
mod locks;
mod random;

pub use locks::{LockLibrary, DEFAULT_LOCKS_DIR};

use rhai::{Engine, EvalAltResult, ImmutableString, Dynamic, CustomType, TypeBuilder, Scope, Map};
use super::paths::StrokePath;
use super::symboldict::SymbolPathDict;
use super::canvas::Canvas as JournalCanvas;
use super::canvas::StrokeStyle as JournalStrokeStyle;
use super::canvas::CanvasStyleOptions;
use super::canvas::{Brush, Frame, PressureProfile};
use super::symbol::{place_along_path, place_words_along_path, Symbol, SymbolPlacementArgs};
use super::canvas::textbox::TextBoxLineMode;
use super::styles::Font;
use super::darkly;
use ::darkly::engine::DarklyEngine;
use parry2d::math::Vec2 as PVec;

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Default, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Entry {
    pages: Vec<Page>,
}

impl Entry {
    fn add_page(&mut self, page: Page) -> Result<(), Box<EvalAltResult>> {
        let label = page.label(self.pages.len());
        if self.pages.iter().enumerate().any(|(i, p)| p.label(i) == label) {
            return Err(format!("add_page: the entry already has a page named '{}'", label).into());
        }
        self.pages.push(page);
        Ok(())
    }
    /// `journal_entry.add_pages(pages)`, such as a split page's tiles.
    fn add_pages(&mut self, pages: rhai::Array) -> Result<(), Box<EvalAltResult>> {
        for (i, page) in pages.into_iter().enumerate() {
            let type_name = page.type_name();
            let page = page.try_cast::<Page>()
                .ok_or_else(|| format!("add_pages: item {} is a {}, not a page", i, type_name))?;
            self.add_page(page)?;
        }
        Ok(())
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("add_page", Self::add_page)
            .with_fn("add_pages", Self::add_pages);
    }
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
    /// `page/canvas` labels of the canvases that have nothing on them, which
    /// publishing skips.
    pub fn empty_canvases(&self) -> Vec<String> {
        self.pages.iter().enumerate().flat_map(|(i, page)| {
            page.canvases.iter().enumerate()
                .filter(|(_, canvas)| canvas.strokes.is_empty() && canvas.frame.is_none())
                .map(move |(c, canvas)| format!("{}/{}", page.label(i), canvas.label(c)))
        }).collect()
    }
    /// The index of a page given by index or by name (its output label).
    pub fn page_index(&self, page: &str) -> Result<usize, String> {
        let labels: Vec<String> = self.pages.iter().enumerate().map(|(i, p)| p.label(i)).collect();
        let found = match page.parse::<usize>() {
            Ok(index) => Some(index).filter(|&i| i < labels.len()),
            Err(_) => labels.iter().position(|label| label == page),
        };
        found.ok_or_else(|| match labels.len() {
            0 => format!("page '{}' doesn't exist; the journal has no pages", page),
            n => format!("page '{}' doesn't exist; pages are 0-{}: {}", page, n - 1, labels.join(", ")),
        })
    }
    /// Pages to publish with their original indices: all of them, or only
    /// those listed in `only`.
    fn selected_pages<'a>(&'a self, only: Option<&'a [usize]>) -> impl Iterator<Item = (usize, &'a Page)> + 'a {
        self.pages.iter().enumerate()
            .filter(move |(i, _)| only.is_none_or(|pages| pages.contains(i)))
    }
    pub fn darkly_publish_pngs(&self, engine: &mut DarklyEngine, dirname: &str, scale: f32, supersample: f32, only: Option<&[usize]>) -> Result<(), String> {
        for (i, page) in self.selected_pages(only) {
            for (canvas_i, canvas) in page.canvases().iter().enumerate() {
                let filename = format!("{}/darkly_{}_{}.png", dirname, page.label(i), canvas.label(canvas_i));
                let Some(canvas) = page.render_canvas(canvas) else {
                    continue; // journalbuild warns about these (see empty_canvases)
                };
                // Canvas units are pixels at scale 1, so brushes scale with
                // the render.
                let buff = darkly::render(&canvas, engine, scale, supersample);
                buff.save(&filename).map_err(|e| format!("couldn't write {}: {}", filename, e))?;
            }
        }
        Ok(())
    }
    /// SVG renders, one per canvas like the Darkly ones, each written as
    /// `{page}_{canvas}.svg` and rasterised to `{page}_{canvas}.png`. Here a
    /// style's width is the stroke's geometric width; in Darkly it scales
    /// the brush's pressure instead (see `new_strokestyle`).
    pub fn publish_svgs(&self, dirname: &str, scale: f32, only: Option<&[usize]>) -> Result<(), String> {
        println!("Publishing SVGs:");
        for (i, page) in self.selected_pages(only) {
            for (canvas_i, canvas) in page.canvases().iter().enumerate() {
                let name = format!("{}_{}", page.label(i), canvas.label(canvas_i));
                let svg_file = format!("{}/{}.svg", dirname, name);
                let filename = format!("{}/{}.png", dirname, name);
                let Some(canvas) = page.render_canvas(canvas) else {
                    continue;
                };
                // Seeded by the file's name: the same every run, but each
                // canvas roughened differently. Tiles use the whole page's.
                let seed = canvas.frame().map_or_else(|| stable_hash(&name), |frame| frame.svg_seed);
                let (svg, png) = canvas.to_svg_and_png(2.0 * scale, page.svg_style.as_ref(), seed);
                println!("\t - {}", svg_file);
                std::fs::write(&svg_file, svg).map_err(|e| format!("couldn't write {}: {}", svg_file, e))?;
                std::fs::write(&filename, png).map_err(|e| format!("couldn't write {}: {}", filename, e))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct BrushSettings {
    brush_style: ImmutableString,
    min_pressure: f64,
    max_pressure: f64,
    size: f64,
    #[rhai_type(skip)]
    pressure_profile: PressureProfile,
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
            pressure_profile: PressureProfile::default(),
        }
    }
    fn get_pressure_profile(&mut self) -> String {
        self.pressure_profile.name().to_string()
    }
    /// `brush.pressure_profile = "taper"`: "ramp" (min to max, the default),
    /// "taper" (min to max at the middle and back) or "constant" (max).
    fn set_pressure_profile(&mut self, name: String) -> Result<(), Box<EvalAltResult>> {
        self.pressure_profile = PressureProfile::from_name(&name).ok_or_else(|| -> Box<EvalAltResult> {
            format!("brush.pressure_profile: unknown profile '{}'; expected one of {}", name, PressureProfile::NAMES.join(", ")).into()
        })?;
        Ok(())
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder.with_get_set("pressure_profile", Self::get_pressure_profile, Self::set_pressure_profile);
    }
}

impl From<BrushSettings> for Brush {
    fn from(bs: BrushSettings) -> Brush {
        Brush {
            name: bs.brush_style.to_string(),
            size: bs.size as f32,
            min_pressure: bs.min_pressure as f32,
            max_pressure: bs.max_pressure as f32,
            pressure_profile: bs.pressure_profile,
        }
    }
}

/// Fail if a path has an infinite or NaN coordinate. Scaling or dividing
/// by zero in a script makes them, and they'd otherwise break the render's
/// sizing far from the line that caused them.
pub(super) fn check_finite(caller: &str, path: &StrokePath) -> Result<(), Box<EvalAltResult>> {
    if path.points_within().iter().all(|p| p.is_finite()) {
        Ok(())
    } else {
        Err(format!("{}: the path has a non-finite (infinite or NaN) coordinate", caller).into())
    }
}

/// FNV-1a: a string hash that, unlike std's, is guaranteed stable across
/// Rust versions.
fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |hash, byte| (hash ^ byte as u64).wrapping_mul(0x100000001b3))
}

/// Names become file names, so they're limited to letters, digits, `_`,
/// `-` and `.` (not leading).
fn check_name(caller: &str, name: Dynamic) -> Result<String, Box<EvalAltResult>> {
    let name = name.into_immutable_string()
        .map_err(|type_name| -> Box<EvalAltResult> { format!("{}: a name must be a string, got {}", caller, type_name).into() })?;
    let ok = !name.is_empty() && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c));
    if ok {
        Ok(name.to_string())
    } else {
        Err(format!("{}: invalid name '{}'; use letters, digits, '_', '-' and '.' (not first)", caller, name).into())
    }
}

/// How a page's SVG output (`--svg`) is drawn, unless the script sets
/// `page.svg_style`.
const DEFAULT_SVG_STYLE: CanvasStyleOptions = CanvasStyleOptions {
    roughness: 1.0,
    point_sample_distance: 1.0,
    trail_off_percent: 0.15,
    noise_amplitude: 0.3,
};

const SVG_STYLE_KEYS: [&str; 4] = ["roughness", "point_sample_distance", "trail_off_percent", "noise_amplitude"];

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Page {
    canvases: Vec<CanvasData>,
    /// Output files are named after this, or `page_{index}` without one.
    #[rhai_type(skip)]
    name: Option<String>,
    /// `None` draws the SVG output as simple strokes.
    #[rhai_type(skip)]
    svg_style: Option<CanvasStyleOptions>,
    /// Space around each canvas's strokes, in canvas units; `None` for the
    /// default (see `margin_for`).
    #[rhai_type(skip)]
    margin: Option<f64>,
}

impl Default for Page {
    fn default() -> Self {
        Page {
            canvases: Vec::new(),
            name: None,
            svg_style: Some(DEFAULT_SVG_STYLE),
            margin: None,
        }
    }
}

impl Page {
    /// `page.add_canvas(canvas)` adds a copy of the canvas as it is now.
    pub fn add_canvas(&mut self, canvas: Canvas) -> Result<(), Box<EvalAltResult>> {
        let canvas = canvas.get();
        let label = canvas.label(self.canvases.len());
        if self.canvases.iter().enumerate().any(|(i, c)| c.label(i) == label) {
            return Err(format!("add_canvas: the page already has a canvas named '{}'", label).into());
        }
        self.canvases.push(canvas);
        Ok(())
    }
    pub fn canvases(&self) -> &Vec<CanvasData> {
        &self.canvases
    }
    /// `canvas` as it's rendered: padded by its margin, or a tile's frame.
    /// `None` if there's nothing to render.
    pub fn render_canvas(&self, canvas: &CanvasData) -> Option<JournalCanvas> {
        let mut rendered: JournalCanvas = canvas.into();
        if rendered.frame().is_none() {
            if !rendered.has_strokes() {
                return None;
            }
            rendered.pad(self.margin_for(canvas));
        }
        Some(rendered)
    }
    /// The margin rendered around `canvas`: the page's `margin`, or by
    /// default half the largest brush size on the canvas, the least that
    /// keeps brush marks at the edges from being cut off. (The bounds
    /// already include the strokes' widths.)
    pub fn margin_for(&self, canvas: &CanvasData) -> f32 {
        match self.margin {
            Some(margin) => margin as f32,
            None => canvas.strokes.iter().map(|s| s.style.brush.size as f32 / 2.0).fold(0.0, f32::max),
        }
    }
    fn get_margin(&mut self) -> Dynamic {
        self.margin.map_or(Dynamic::UNIT, Dynamic::from)
    }
    /// `page.margin = 20.0` (canvas units, 0 for none), or `()` for the
    /// default.
    fn set_margin(&mut self, margin: Dynamic) -> Result<(), Box<EvalAltResult>> {
        if margin.is_unit() {
            self.margin = None;
            return Ok(());
        }
        let value = margin.as_float().or_else(|_| margin.as_int().map(|i| i as f64))
            .map_err(|t| -> Box<EvalAltResult> { format!("page.margin: expected a number or (), got {}", t).into() })?;
        if !(value >= 0.0) || !value.is_finite() {
            return Err(format!("page.margin: must be a finite number of at least 0, got {}", value).into());
        }
        self.margin = Some(value);
        Ok(())
    }
    /// The page's name in output files, given its index in the entry.
    pub fn label(&self, index: usize) -> String {
        self.name.clone().unwrap_or_else(|| format!("page_{}", index))
    }
    fn get_name(&mut self) -> Dynamic {
        self.name.clone().map_or(Dynamic::UNIT, Dynamic::from)
    }
    fn set_name(&mut self, name: Dynamic) -> Result<(), Box<EvalAltResult>> {
        self.name = Some(check_name("page.name", name)?);
        Ok(())
    }
    fn get_svg_style(&mut self) -> Dynamic {
        match &self.svg_style {
            None => Dynamic::UNIT,
            Some(style) => {
                let mut map = rhai::Map::new();
                for (key, value) in SVG_STYLE_KEYS.iter().zip([style.roughness, style.point_sample_distance, style.trail_off_percent, style.noise_amplitude]) {
                    // Via the shortest decimal form, so 0.3 reads back as 0.3
                    // rather than 0.30000001192092896.
                    let value: f64 = value.to_string().parse().expect("a float's string form parses");
                    map.insert((*key).into(), Dynamic::from(value));
                }
                Dynamic::from_map(map)
            }
        }
    }
    /// `()` for simple strokes, or a map of options; any left out keep their
    /// defaults.
    fn set_svg_style(&mut self, style: Dynamic) -> Result<(), Box<EvalAltResult>> {
        const CALLER: &str = "page.svg_style";
        if style.is_unit() {
            self.svg_style = None;
            return Ok(());
        }
        let type_name = style.type_name();
        let options = style.try_cast::<rhai::Map>()
            .ok_or_else(|| -> Box<EvalAltResult> { format!("{}: expected a map of options or (), got {}", CALLER, type_name).into() })?;
        layout::check_keys(CALLER, &options, &SVG_STYLE_KEYS)?;
        let get = |key: &str, default: f32| -> Result<f32, Box<EvalAltResult>> {
            match layout::number(CALLER, &options, key)? {
                Some(n) if n >= 0.0 => Ok(n),
                Some(n) => Err(format!("{}: '{}' can't be negative, got {}", CALLER, key, n).into()),
                None => Ok(default),
            }
        };
        let d = DEFAULT_SVG_STYLE;
        let point_sample_distance = get("point_sample_distance", d.point_sample_distance)?;
        if point_sample_distance == 0.0 {
            return Err(format!("{}: 'point_sample_distance' must be positive", CALLER).into());
        }
        self.svg_style = Some(CanvasStyleOptions {
            roughness: get("roughness", d.roughness)?,
            point_sample_distance,
            trail_off_percent: get("trail_off_percent", d.trail_off_percent)?,
            noise_amplitude: get("noise_amplitude", d.noise_amplitude)?,
        });
        Ok(())
    }
    /// `page.split(columns, rows)`: the page cut into a grid of tiles, each
    /// a page of its own, row by row. See `split_with`.
    fn split(&mut self, columns: i64, rows: i64) -> Result<rhai::Array, Box<EvalAltResult>> {
        let mut options = Map::new();
        options.insert("columns".into(), Dynamic::from(columns));
        options.insert("rows".into(), Dynamic::from(rows));
        self.split_with(options)
    }
    /// `page.split(#{ columns, rows })`, or `#{ tile_width, tile_height }`
    /// for tiles of a given size (in canvas units; the grid grows evenly
    /// round the page to a whole number of them), or a mix: one option
    /// across and one down, each 1 tile by default.
    ///
    /// The page is everything its renders cover, every canvas with its
    /// margin, and the tiles divide it evenly. Each tile page has a part of
    /// each canvas, named as the canvas was, rendering exactly its share of
    /// the pixels the whole canvas's render would have: placed side by side
    /// in order, a canvas's tiles make that render (over the whole page, so
    /// canvases' tiles line up with each other too). Strokes crossing a
    /// seam are drawn whole in each tile they reach and cut off at its
    /// edges, with the same brush texture, so they carry on unbroken.
    ///
    /// Tiles are named `{name}_{row}_{column}`, counting from 0, so the page
    /// needs a name. Their `tile` property gives their place in the grid.
    fn split_with(&mut self, options: Map) -> Result<rhai::Array, Box<EvalAltResult>> {
        const CALLER: &str = "page.split";
        layout::check_keys(CALLER, &options, &["columns", "rows", "tile_width", "tile_height"])?;
        let name = self.name.clone()
            .ok_or_else(|| format!("{}: name the page first; its tiles are named after it ({{name}}_{{row}}_{{column}})", CALLER))?;
        let canvases: Vec<(usize, &CanvasData)> = self.canvases.iter().enumerate()
            .filter(|(_, canvas)| !canvas.strokes.is_empty()).collect();
        if self.canvases.iter().any(|canvas| canvas.frame.is_some()) {
            return Err(format!("{}: the page is already a tile; split the page it came from instead", CALLER).into());
        }
        let mut region = parry2d::bounding_volume::Aabb::new_invalid();
        for (_, canvas) in &canvases {
            let bounds = self.render_canvas(canvas).expect("the canvas has strokes").bounds();
            region.take_point(bounds.mins);
            region.take_point(bounds.maxs);
        }
        if canvases.is_empty() {
            return Err(format!("{}: the page has nothing drawn on it to split", CALLER).into());
        }
        // How many tiles across and down, growing the region to a whole
        // number of tiles where they have a set size.
        let mut grid = |count_key: &str, size_key: &str, axis: usize| -> Result<u32, Box<EvalAltResult>> {
            let extent = region.extents()[axis].max(1.0);
            match (options.get(count_key), layout::number(CALLER, &options, size_key)?) {
                (Some(_), Some(_)) => Err(format!("{}: give '{}' or '{}', not both", CALLER, count_key, size_key).into()),
                (Some(count), None) => match count.as_int() {
                    Ok(n) if (1..=1000).contains(&n) => Ok(n as u32),
                    _ => Err(format!("{}: '{}' should be a whole number from 1 to 1000, got {}", CALLER, count_key, count).into()),
                },
                (None, Some(size)) if size > 0.0 && size.is_finite() => {
                    let count = (extent / size).ceil().clamp(1.0, 1000.0);
                    let grow = (count * size - extent) / 2.0;
                    region.mins[axis] -= grow;
                    region.maxs[axis] = region.mins[axis] + count * size;
                    Ok(count as u32)
                }
                (None, Some(size)) => Err(format!("{}: '{}' must be positive, got {}", CALLER, size_key, size).into()),
                (None, None) => Ok(1),
            }
        };
        let columns = grid("columns", "tile_width", 0)?;
        let rows = grid("rows", "tile_height", 1)?;
        // Every stroke keeps its texture's seed from the whole canvas.
        let seeded: Vec<(usize, CanvasData)> = canvases.iter().map(|(i, canvas)| {
            let seeds = JournalCanvas::from(*canvas).stroke_seeds();
            let mut canvas = (*canvas).clone();
            for (stroke, seed) in canvas.strokes.iter_mut().zip(seeds) {
                stroke.seed = Some(seed);
            }
            (*i, canvas)
        }).collect();
        let mut tiles = rhai::Array::new();
        for row in 0..rows {
            for column in 0..columns {
                let mut tile = Page {
                    canvases: Vec::new(),
                    name: Some(format!("{}_{}_{}", name, row, column)),
                    svg_style: self.svg_style.clone(),
                    margin: self.margin,
                };
                for (i, canvas) in &seeded {
                    let mut part = canvas.clone();
                    part.frame = Some(Frame {
                        page: region,
                        column, columns, row, rows,
                        svg_seed: stable_hash(&format!("{}_{}", name, canvas.label(*i))),
                    });
                    // Named as it was, since its index might change.
                    part.name = Some(canvas.label(*i));
                    tile.canvases.push(part);
                }
                tiles.push(Dynamic::from(tile));
            }
        }
        Ok(tiles)
    }
    /// A tile's place in its page's grid, or `()`.
    fn get_tile(&mut self) -> Dynamic {
        let Some(frame) = self.canvases.iter().find_map(|c| c.frame) else { return Dynamic::UNIT };
        let mut map = Map::new();
        for (key, value) in [("row", frame.row), ("column", frame.column), ("rows", frame.rows), ("columns", frame.columns)] {
            map.insert(key.into(), Dynamic::from(value as i64));
        }
        Dynamic::from_map(map)
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("add_canvas", Self::add_canvas)
            .with_fn("new_page", Self::default)
            .with_get_set("name", Self::get_name, Self::set_name)
            .with_get_set("svg_style", Self::get_svg_style, Self::set_svg_style)
            .with_get_set("margin", Self::get_margin, Self::set_margin)
            .with_fn("split", Self::split)
            .with_fn("split", Self::split_with)
            .with_get("tile", Self::get_tile)
        ;
    }
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
    /// `new_vec(x, y)`, refusing infinite and NaN coordinates (from a
    /// division by zero, say), which would otherwise surface far away.
    fn checked(x: f64, y: f64) -> Result<Self, Box<EvalAltResult>> {
        if !(x.is_finite() && y.is_finite()) {
            return Err(format!("new_vec: coordinates must be finite numbers, got ({}, {})", x, y).into());
        }
        Ok(Vec2 { x, y })
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_vec", Self::checked)
        ;
    }
}

impl From<Vec2> for PVec {
    fn from(val: Vec2) -> Self {
        PVec::new(val.x as f32, val.y as f32)
    }
}

/// A shared, mutable value, as rhai scripts see paths, glyphs and canvases:
/// copying one (assigning it, passing it, a `for` loop's variable) shares
/// the value, so changes through any copy, including chained calls like
/// `p.move_to(a).line_to(b)`, all reach it. `.copy()` makes an independent
/// one.
pub struct Shared<T>(Rc<RefCell<T>>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T: Clone> Shared<T> {
    pub fn new(value: T) -> Self {
        Shared(Rc::new(RefCell::new(value)))
    }
    pub fn borrow(&self) -> std::cell::Ref<'_, T> {
        self.0.borrow()
    }
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, T> {
        self.0.borrow_mut()
    }
    /// The value as it is now.
    pub fn get(&self) -> T {
        self.0.borrow().clone()
    }
    /// An independent copy, for `.copy()`.
    pub fn copy(&mut self) -> Self {
        Shared::new(self.get())
    }
    /// Change the value in place and return this same handle, so the change
    /// reaches the variable even partway through a chain.
    pub fn update(&mut self, f: impl FnOnce(&mut T)) -> Self {
        f(&mut self.0.borrow_mut());
        self.clone()
    }
}

/// A path as scripts see it (see `Shared`).
pub type Path = Shared<StrokePath>;

/// A canvas as scripts see it (see `Shared`). Pages keep copies of the
/// canvases added to them.
pub type Canvas = Shared<CanvasData>;

#[derive(Debug, Default, Clone)]
pub struct CanvasData {
    pub strokes: Vec<Stroke>,
    /// Its output files are named after this, or its index on the page.
    name: Option<String>,
    /// For a tile of a split page, the part of the page it renders.
    frame: Option<Frame>,
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
fn register_canvas(engine: &mut Engine) {
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
        .register_fn("scale", |c: &mut Canvas, scale: f64| -> Canvas {
            c.update(|data| data.transform(|p| p.scale_by(scale as f32)))
        })
        .register_fn("rotate_around", |c: &mut Canvas, point: Vec2, rad: f64| -> Canvas {
            let point = point.into();
            c.update(|data| data.transform(|p| p.rotate_around(point, rad as f32)))
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
    path: StrokePath,
    style: StrokeStyle,
    /// Its texture's seed, kept from the whole page in a tile (see
    /// `ColoredStroke::seed`).
    #[rhai_type(skip)]
    seed: Option<u32>,
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

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct StrokeStyle {
    width: f64,
    color: ImmutableString,
    #[rhai_type(skip)]
    brush: BrushSettings,
}

impl From<StrokeStyle> for JournalStrokeStyle {
    fn from(val: StrokeStyle) -> Self {
        JournalStrokeStyle {
            width: val.width as f32,
            color: val.color.to_string(),
            brush: Some(val.brush.into()),
        }
    }
}

impl StrokeStyle {
    /// `new_strokestyle(width, color, brush)`.
    ///
    /// `width` means different things in the two outputs:
    /// - Darkly (the default render): it multiplies the brush's pressure,
    ///   so pressure runs from `width * min_pressure` to
    ///   `width * max_pressure`. The brush's `size` sets how wide its marks
    ///   are; pressure makes them heavier (how much depends on the brush).
    ///   Darkly caps pressure at 1, so past `width = 1 / max_pressure` a
    ///   wider stroke looks no heavier, and the render warns.
    /// - SVG (`--svg`): it's the stroke's width in canvas units.
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

/// Where text's glyphs come from: the symbol dictionary, and the generator
/// that picks glyph variants.
type TextSource = (Rc<SymbolPathDict>, random::SharedRng);

/// Options for text along a path.
const PATH_TEXT_KEYS: [&str; 4] = ["spacing", "avoid", "clearance", "layout"];

/// Text along a path: glyph by glyph, or keeping words together on a
/// subpath ("words" layout).
fn place_text(symbols: Vec<Symbol>, path: &StrokePath, args: &SymbolPlacementArgs, mode: &TextBoxLineMode) -> (Vec<Symbol>, Vec<Symbol>) {
    match mode {
        TextBoxLineMode::Flat => place_along_path(symbols, path, args),
        TextBoxLineMode::WordsByBlankSymbol => place_words_along_path(symbols, path, args),
    }
}

/// `path.fill_with_msg(style, font, size, msg[, #{ spacing, avoid, clearance, layout }])`:
/// the message along as many copies of the path as it takes, one canvas per
/// copy. Glyphs sit at their widths, plus `spacing` (a fraction of the font
/// size), and keep clear of `avoid` (see avoid.rs). With `layout: "words"`,
/// words aren't split around what's avoided or between subpaths.
fn fill_with_msg(path: &mut Path, style: StrokeStyle, font: Font, fontsize: f64, msg: &str, options: Map, (symdict, rng): &TextSource) -> Result<Vec<Dynamic>, Box<EvalAltResult>> {
    layout::check_keys("fill_with_msg", &options, &PATH_TEXT_KEYS)?;
    if !(fontsize > 0.0) {
        return Err(format!("fill_with_msg: font size must be positive, got {}", fontsize).into());
    }
    let builder = font.builder(symdict);
    let mode = layout::line_mode("fill_with_msg", &options)?;
    let avoid = avoid::AvoidOption::parse("fill_with_msg", &options, false)?;
    let tree = avoid.tree.as_ref().map(|a| a.borrow());
    let args = SymbolPlacementArgs {
        avoid: tree.as_deref(),
        padding: avoid.padding(fontsize as f32),
        spacing: layout::spacing("fill_with_msg", &options)?,
        ..SymbolPlacementArgs::new(fontsize as f32)
    };

    let mut all_canvases = Vec::new();
    let mut symbols = builder.build(msg, &mut *rng.borrow_mut());
    let path = path.get();
    let total = symbols.len();
    while !symbols.is_empty() {
        let (placed, unplaced) = place_text(std::mem::take(&mut symbols), &path, &args, &mode);
        if placed.is_empty() {
            // Another copy of the path wouldn't fit it either.
            return Err(format!(
                "fill_with_msg: glyph {} of {} doesn't fit on the path at font size {}; use a longer path or a smaller font{}",
                total - unplaced.len() + 1, total, fontsize,
                if avoid.tree.is_some() { ", or avoid less" } else { "" },
            ).into());
        }
        let mut canvas = CanvasData::default();
        for symbol in placed {
            canvas.add_strokepath(symbol.placed_strokepath(), style.clone());
        }
        all_canvases.push(Dynamic::from(Canvas::new(canvas)));
        symbols = unplaced;
    }
    Ok(all_canvases)
}

/// `path.fit_msg_to(style, font, msg[, #{ spacing, avoid, clearance, layout }])`:
/// the message along the path at the largest font size that fits it.
fn fit_msg_to(path: &mut Path, style: StrokeStyle, font: Font, msg: &str, options: Map, (symdict, rng): &TextSource) -> Result<Canvas, Box<EvalAltResult>> {
    layout::check_keys("fit_msg_to", &options, &PATH_TEXT_KEYS)?;
    let path = &path.get();
    let mut canvas = CanvasData::default();
    let builder = font.builder(symdict);
    let symbols = builder.build(msg, &mut *rng.borrow_mut());
    let mode = layout::line_mode("fit_msg_to", &options)?;
    let avoid = avoid::AvoidOption::parse("fit_msg_to", &options, false)?;
    let tree = avoid.tree.as_ref().map(|a| a.borrow());
    let mut args = SymbolPlacementArgs {
        avoid: tree.as_deref(),
        spacing: layout::spacing("fit_msg_to", &options)?,
        ..SymbolPlacementArgs::new(20.0)
    };
    let mut window_min = 0.1;
    let mut window_max = 1000.0;
    while window_max - window_min > 0.005 {
        let current_scale = (window_max + window_min) / 2.0;
        args.symbol_scale = current_scale;
        args.padding = avoid.padding(current_scale);
        let (_, unplaced) = place_text(symbols.clone(), path, &args, &mode);
        if !unplaced.is_empty() {
            window_max = current_scale;
        } else {
            window_min = current_scale;
        }
    }
    args.symbol_scale = window_min;
    args.padding = avoid.padding(window_min);
    let (placed, unplaced) = place_text(symbols, path, &args, &mode);
    if !unplaced.is_empty() {
        return Err(format!(
            "fit_msg_to: the message doesn't fit on the path even at font size {}: {} of {} glyphs left over",
            window_min, unplaced.len(), placed.len() + unplaced.len(),
        ).into());
    }
    for symbol in placed {
        canvas.add_strokepath(symbol.placed_strokepath(), style.clone());
    }
    Ok(Canvas::new(canvas))
}

/// Apply a by-value StrokePath builder method to a path in place, returning
/// the same path.
fn update_path(path: &mut Path, f: impl FnOnce(StrokePath) -> StrokePath) -> Path {
    path.update(|p| *p = f(std::mem::take(p)))
}

pub struct JournalEngine {
    engine: Engine,
    warnings: Rc<RefCell<Vec<String>>>,
}

impl JournalEngine {
    /// An engine with the repository's lock types (DEFAULT_LOCKS_DIR).
    pub fn new(symdict: Rc<SymbolPathDict>) -> Self {
        Self::with_locks_dir(symdict, DEFAULT_LOCKS_DIR)
    }
    /// An engine whose lock types are the lock files in `locks_dir`.
    pub fn with_locks_dir(symdict: Rc<SymbolPathDict>, locks_dir: impl Into<std::path::PathBuf>) -> Self {
        let mut engine = Engine::new();
        let rng = random::register(&mut engine);
        let text: TextSource = (symdict.clone(), rng.clone());
        let (t1, t2, t3, t4) = (text.clone(), text.clone(), text.clone(), text);

        register_canvas(&mut engine);
        engine.register_type_with_name::<Path>("StrokePath")
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
            .register_fn("fill_with_msg", move |path: &mut Path, style: StrokeStyle, font: Font, fontsize: f64, msg: &str| {
                fill_with_msg(path, style, font, fontsize, msg, Map::new(), &t1)
            })
            .register_fn("fill_with_msg", move |path: &mut Path, style: StrokeStyle, font: Font, fontsize: f64, msg: &str, options: Map| {
                fill_with_msg(path, style, font, fontsize, msg, options, &t2)
            })
            .register_fn("fit_msg_to", move |path: &mut Path, style: StrokeStyle, font: Font, msg: &str| {
                fit_msg_to(path, style, font, msg, Map::new(), &t3)
            })
            .register_fn("fit_msg_to", move |path: &mut Path, style: StrokeStyle, font: Font, msg: &str, options: Map| {
                fit_msg_to(path, style, font, msg, options, &t4)
            })
            .register_fn("scale_by", |path: &mut Path, scale: f64| -> Path {
                path.update(|p| p.scale_by(scale as f32))
            })
            .register_fn("translate_by", |path: &mut Path, translate: Vec2| -> Path {
                path.update(|p| p.translate_by(translate.into()))
            })
            .register_fn("rotate_around", |path: &mut Path, rot_point: Vec2, rot: f64| -> Path {
                path.update(|p| p.rotate_around(rot_point.into(), rot as f32))
            })
            .register_fn("new_strokepath", || Path::new(StrokePath::new()))
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
            .register_fn("close", |path: &mut Path| -> Path {
                update_path(path, |p| p.close_path())
            })
            .register_fn("new_circle", |center: Vec2, radius: f64| -> Path {
                Path::new(StrokePath::circle(center.into(), radius as f32))
            })
            .register_fn("new_rectangle", |center: Vec2, width: f64, height: f64| -> Path {
                Path::new(StrokePath::rectangle(center.into(), width as f32, height as f32))
            })
            .register_fn("new_line", |start: Vec2, end: Vec2| -> Path {
                Path::new(StrokePath::line(start.into(), end.into()))
            })
            .register_fn("new_arc", |center: Vec2, radius: f64, start_rad: f64, end_rad: f64| -> Path {
                Path::new(StrokePath::arc(center.into(), radius as f32, start_rad as f32, end_rad as f32))
            });

        // warn("...") reports a problem without stopping the script: it goes
        // to stderr, marked with the script line, like journalbuild's own
        // warnings.
        let warnings: Rc<RefCell<Vec<String>>> = Rc::default();
        let script_warnings = warnings.clone();
        engine.register_fn("warn", move |ctx: rhai::NativeCallContext, message: &str| {
            let warning = match ctx.call_position().line() {
                Some(line) => format!("{} (line {})", message, line),
                None => message.to_string(),
            };
            eprintln!("warning: {}", warning);
            script_warnings.borrow_mut().push(warning);
        });

        // Characters drawn with the fallback glyph so far:
        // #{ "q": #{ count: 2, tried: ["copper_lower_q", ...] }, ... }
        let missing_symdict = symdict.clone();
        engine.register_fn("missing_glyphs", move || -> rhai::Map {
            missing_symdict.missing_glyphs().into_iter().map(|(c, glyph)| {
                let mut info = rhai::Map::new();
                info.insert("count".into(), Dynamic::from(glyph.count as i64));
                info.insert("tried".into(), Dynamic::from_array(glyph.tried.into_iter().map(Dynamic::from).collect()));
                (c.to_string().into(), Dynamic::from_map(info))
            }).collect()
        });

        let load_symdict = symdict.clone();
        engine.register_fn("load_symbol", move |symname: &str| -> Result<Path, Box<EvalAltResult>> {
            load_symdict.get_strokepath(symname).map(Path::new)
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
            // Darkly pressure runs from 0 to 1 (see also the width warning
            // in darkly.rs).
            if !(0.0..=1.0).contains(&min_pressure) || !(0.0..=1.0).contains(&max_pressure) || min_pressure > max_pressure {
                return Err(format!(
                    "new_brush: pressures must satisfy 0 <= min <= max <= 1, got min {} and max {}",
                    min_pressure, max_pressure,
                ).into());
            }
            if !(size > 0.0) {
                return Err(format!("new_brush: size must be positive, got {}", size).into());
            }
            Ok(BrushSettings::new_brush(name, min_pressure, max_pressure, size))
        });

        art::register(&mut engine);
        avoid::register(&mut engine);
        glyphs::register(&mut engine, symdict.clone(), rng.clone());
        layout::register(&mut engine, symdict.clone(), rng.clone());
        locks::register(&mut engine, Rc::new(LockLibrary::new(locks_dir)));

        engine
            .build_type::<Vec2>()
            .build_type::<Entry>()
            .build_type::<Page>()
            .build_type::<BrushSettings>()
            .build_type::<StrokeStyle>()
            .build_type::<Stroke>();

        JournalEngine {
            engine,
            warnings,
        }
    }
    /// Everything scripts have passed to `warn` so far.
    pub fn warnings(&self) -> Vec<String> {
        self.warnings.borrow().clone()
    }
    /// Run a journal script, returning the `journal_entry` it built. Errors
    /// are rhai's message, with the script line.
    pub fn build_entry_str(&mut self, script_str: &str) -> Result<Entry, String> {
        let mut scope = Scope::new();
        scope.push("journal_entry", Entry::default());
        self.engine.run_with_scope(&mut scope, script_str).map_err(|err| err.to_string())?;
        scope.get_value("journal_entry")
            .ok_or_else(|| "the script replaced journal_entry with something that isn't an entry".to_string())
    }
    pub fn build_entry_file(&mut self, rhai_filename: &str) -> Result<Entry, String> {
        let file_contents = std::fs::read_to_string(rhai_filename)
            .map_err(|err| format!("couldn't read {}: {}", rhai_filename, err))?;
        self.build_entry_str(&file_contents).map_err(|err| format!("{}: {}", rhai_filename, err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> JournalEngine {
        let symd = SymbolPathDict::from_file(crate::journal::symboldict::DEFAULT_SYMBOLS).expect("Failed to load symbols.json");
        JournalEngine::new(Rc::new(symd))
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
        let err = engine().build_entry_str(&script).expect_err("bad color should fail").to_string();
        assert!(err.contains("darkredd"), "{}", err);
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn load_symbol_rejects_unknown_name_with_line() {
        let err = engine().build_entry_str("let ok = load_symbol(\"basic_eye\");\nlet bad = load_symbol(\"no_such_glyph\");")
            .expect_err("unknown symbol should fail").to_string();
        assert!(err.contains("unknown symbol 'no_such_glyph'"), "{}", err);
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn new_brush_rejects_unknown_name_with_line() {
        let err = engine().build_entry_str("let ok = new_brush(\"Ink Pen\", 0.2, 0.5, 1.0);\nlet bad = new_brush(\"Crayon\", 0.2, 0.5, 1.0);")
            .expect_err("unknown brush should fail").to_string();
        assert!(err.contains("unknown brush 'Crayon'"), "{}", err);
        assert!(err.contains("Calligraphy"), "error should list available brushes: {}", err);
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn new_brush_rejects_bad_pressures_and_sizes() {
        for (args, want) in [
            ("0.8, 0.2, 1.0", "0 <= min <= max <= 1, got min 0.8 and max 0.2"),
            ("-0.1, 0.5, 1.0", "got min -0.1"),
            ("0.2, 1.5, 1.0", "max 1.5"),
            ("0.2, 0.5, 0.0", "size must be positive, got 0"),
            ("0.2, 0.5, -1.0", "size must be positive"),
        ] {
            let err = engine().build_entry_str(&format!("let ok = 1;\nnew_brush(\"Ink Pen\", {});", args))
                .expect_err(args).to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
        // Equal pressures (a fixed pressure) are fine.
        assert!(engine().build_entry_str("new_brush(\"Calligraphy\", 1.0, 1.0, 0.8); journal_entry.add_page(new_page());").is_ok());
    }

    #[test]
    fn new_strokestyle_requires_brush() {
        let err = engine().build_entry_str(r#"let s = new_strokestyle(1.5, "black");"#)
            .expect_err("brushless style should fail").to_string();
        assert!(err.contains("new_strokestyle"), "{}", err);
    }

    fn path_string(entry_script: &str) -> String {
        let engine = engine();
        let path: Path = engine.engine.eval(entry_script).expect("script should evaluate");
        path.get().to_string()
    }

    #[test]
    fn path_functions_mutate_in_place_and_chain() {
        // Statement form mutates the variable...
        let statements = path_string(r#"
            let p = new_strokepath();
            p.move_to(new_vec(0.0, 0.0));
            p.line_to(new_vec(1.0, 0.0));
            p.scale_by(2.0);
            p.translate_by(new_vec(1.0, 1.0));
            p
        "#);
        // ...and chaining (including on temporaries) gives the same result.
        let chained = path_string(r#"
            new_strokepath().move_to(new_vec(0.0, 0.0)).line_to(new_vec(1.0, 0.0))
                .scale_by(2.0).translate_by(new_vec(1.0, 1.0))
        "#);
        let reassigned = path_string(r#"
            let p = new_strokepath();
            p = p.move_to(new_vec(0.0, 0.0));
            p = p.line_to(new_vec(1.0, 0.0));
            p = p.scale_by(2.0);
            p = p.translate_by(new_vec(1.0, 1.0));
            p
        "#);
        // Chains on a variable change the variable by every call.
        let chained_variable = path_string(r#"
            let p = new_strokepath();
            p.move_to(new_vec(0.0, 0.0)).line_to(new_vec(1.0, 0.0)).scale_by(2.0).translate_by(new_vec(1.0, 1.0));
            p
        "#);
        assert_eq!(statements, "M 1 1 L 3 1 ");
        assert_eq!(chained, statements);
        assert_eq!(reassigned, statements);
        assert_eq!(chained_variable, statements);
    }

    #[test]
    fn copies_share_until_copied_and_adding_takes_a_snapshot() {
        let script = r#"
            let a = new_line(new_vec(0.0, 0.0), new_vec(1.0, 0.0));
            let b = a;
            b.translate_by(new_vec(5.0, 0.0));
            if a.to_string() != b.to_string() { throw "assignment should share"; }
            let c = a.copy();
            c.translate_by(new_vec(5.0, 0.0));
            if a.to_string() == c.to_string() { throw "copy() should be independent"; }

            let style = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let canvas = new_canvas();
            canvas.add_strokepath(a, style);
            a.translate_by(new_vec(100.0, 0.0));   // after adding: the canvas keeps its copy
            let page = new_page();
            page.add_canvas(canvas);
            canvas.add_strokepath(a, style);        // after adding: the page keeps its copy
            journal_entry.add_page(page);

            // Loop variables share too, so loops can change array elements.
            let gs = lowercase_font("loche_upper").glyphs("ab");
            for g in gs { g.scale = 7.0; }
            if gs[0].scale != 7.0 || gs[1].scale != 7.0 { throw "loop should change the glyphs"; }
            // A canvas can add itself.
            canvas.add_canvas(canvas);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let strokes = &entry.pages[0].canvases()[0].strokes;
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].path.to_string(), "M 5 0 L 6 0 ");
    }

    #[test]
    fn shape_and_curve_functions_build_paths() {
        assert_eq!(path_string("new_line(new_vec(0.0, 0.0), new_vec(2.0, 3.0))"), "M 0 0 L 2 3 ");
        assert_eq!(path_string("new_rectangle(new_vec(0.0, 0.0), 2.0, 4.0)"), "M -1 -2 L 1 -2 L 1 2 L -1 2 Z ");
        assert_eq!(
            path_string("new_strokepath().move_to(new_vec(0.0, 0.0)).quad_to(new_vec(1.0, 1.0), new_vec(2.0, 0.0)).cubic_to(new_vec(3.0, 1.0), new_vec(4.0, 1.0), new_vec(5.0, 0.0)).close()"),
            "M 0 0 Q 1 1 2 0 C 3 1 4 1 5 0 Z ",
        );
        // A circle is four cubic quarter-arcs.
        assert_eq!(path_string("new_circle(new_vec(0.0, 0.0), 1.0)").matches('C').count(), 4);
    }

    #[test]
    fn missing_glyphs_are_drawn_and_reported() {
        let script = r#"
            let style = new_strokestyle(1.5, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = new_fontset(["copper_upper"], ["copper_lower"]);
            let canvas = new_line(new_vec(0.0, 0.0), new_vec(400.0, 0.0)).fit_msg_to(style, font, "a~b~");
            let missing = missing_glyphs();
            if missing["~"].count != 2 { throw "expected ~ twice, got " + missing; }
            // Non-lowercase characters (punctuation included) try the upper
            // fonts first, then the lower ones.
            if missing["~"].tried != ["copper_upper_~", "copper_lower_~"] { throw "unexpected tried list: " + missing["~"].tried; }
        "#;
        engine().build_entry_str(script).expect("script should run");
    }

    /// The svg of the first page of `script`'s entry, built by a fresh engine.
    fn page_svg(script: &str) -> String {
        let entry = engine().build_entry_str(script).expect("script should run");
        let canvas: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        canvas.to_svg(None, 0)
    }

    #[test]
    fn glyph_variants_follow_the_seed() {
        // copper_lower has several variants of most letters, so the text's
        // glyphs depend on which variant each pick lands on.
        let script = |seed: i64| format!(r#"{}
            seed({});
            let style = new_strokestyle(1.5, "black", brush);
            let font = new_fontset(["copper_upper"], ["copper_lower"]);
            let path = new_strokepath().move_to(new_vec(0.0, 0.0)).line_to(new_vec(2000.0, 0.0));
            let page = new_page();
            page.add_canvas(path.fit_msg_to(style, font, "a banana and an aardvark"));
            journal_entry.add_page(page);
        "#, BRUSH, seed);
        // Each engine loads its own dictionary, so this also checks the pick
        // doesn't depend on hash map order.
        assert_eq!(page_svg(&script(3)), page_svg(&script(3)));
        assert_ne!(page_svg(&script(3)), page_svg(&script(4)));
    }

    const NAMED: &str = r#"
        let style = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
        let ring = new_canvas();
        ring.add_strokepath(new_circle(new_vec(0.0, 0.0), 10.0), style);
        let named = ring.copy();
        named.name = "ring";
        let maze = new_page();
        maze.name = "maze";
        maze.add_canvas(named);
        maze.add_canvas(ring);
        journal_entry.add_page(new_page());
        journal_entry.add_page(maze);
    "#;

    #[test]
    fn pages_and_canvases_are_labelled_by_name_or_index() {
        let entry = engine().build_entry_str(NAMED).expect("script should run");
        let labels: Vec<String> = entry.pages.iter().enumerate().map(|(i, p)| p.label(i)).collect();
        assert_eq!(labels, ["page_0", "maze"]);
        let canvases: Vec<String> = entry.pages[1].canvases().iter().enumerate().map(|(i, c)| c.label(i)).collect();
        assert_eq!(canvases, ["ring", "1"]);

        let dir = std::env::temp_dir().join(format!("journal_names_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        entry.publish_svgs(dir.to_str().unwrap(), 0.1, Some(&[1])).expect("publish should succeed");
        assert!(dir.join("maze_ring.png").is_file() && dir.join("maze_1.png").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn text_that_cannot_fit_its_path_is_an_error() {
        let setup = r#"
            let s = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = new_fontset(["copper_upper"], ["copper_lower"]);
            let short = new_line(new_vec(0.0, 0.0), new_vec(10.0, 0.0));
        "#;
        for (script, want) in [
            ("short.fill_with_msg(s, font, 50.0, \"abc\");", "fill_with_msg: glyph 1 of 3 doesn't fit"),
            ("short.fill_with_msg(s, font, 0.0, \"abc\");", "font size must be positive"),
            ("new_line(new_vec(0.0, 0.0), new_vec(0.01, 0.0)).fit_msg_to(s, font, \"a long message\");", "fit_msg_to: the message doesn't fit"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", setup, script)).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
        // A path that fits a few glyphs at a time still gets the whole message.
        let parts: rhai::Array = engine().engine.eval(&format!("{}\nnew_line(new_vec(0.0, 0.0), new_vec(60.0, 0.0)).fill_with_msg(s, font, 10.0, \"abcdefghijklmnopqrstuvwxyz\")", setup)).unwrap();
        assert!(parts.len() > 1);
    }

    /// Where each glyph of `msg` lands along a straight line, at font size
    /// 20, with `options`.
    fn glyph_xs(msg: &str, options: &str) -> Vec<f32> {
        let script = format!(r#"
            let s = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = lowercase_font("loche_upper");
            let line = new_line(new_vec(0.0, 0.0), new_vec(2000.0, 0.0));
            let page = new_page();
            for c in line.fill_with_msg(s, font, 20.0, "{}"{}) {{ page.add_canvas(c); }}
            journal_entry.add_page(page);
        "#, msg, options);
        let entry = engine().build_entry_str(&script).expect("script should run");
        entry.pages[0].canvases()[0].strokes.iter().map(|stroke| {
            let xs: Vec<f32> = stroke.path.points_within().iter().map(|p| p.x).collect();
            xs.iter().sum::<f32>() / xs.len() as f32
        }).collect()
    }

    #[test]
    fn text_sits_at_glyph_widths_plus_spacing() {
        // Narrow glyphs used to be pushed a whole glyph unit apart, however
        // narrow they were.
        let tight = glyph_xs("iiii", "");
        let gaps: Vec<f32> = tight.windows(2).map(|w| w[1] - w[0]).collect();
        let i_width = {
            let symd = SymbolPathDict::from_file(crate::journal::symboldict::DEFAULT_SYMBOLS).unwrap();
            let sym = symd.get_symbol_base("loche_upper_i").unwrap();
            crate::journal::symbol::Symbol::new_default_from_base_rc(sym).hull_aabb().extents().x * 20.0 / 2.0
        };
        assert!(gaps.iter().all(|g| (g - i_width).abs() < 0.5), "gaps {:?}, i is {} wide", gaps, i_width);
        // spacing adds a fraction of the font size between glyphs.
        let spaced = glyph_xs("iiii", ", #{ spacing: 0.5 }");
        let spaced_gaps: Vec<f32> = spaced.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(spaced_gaps.iter().zip(&gaps).all(|(s, g)| (s - g - 10.0).abs() < 0.5), "{:?} vs {:?}", spaced_gaps, gaps);
        let err = engine().build_entry_str("new_line(new_vec(0.0, 0.0), new_vec(9.0, 0.0)).fill_with_msg(new_strokestyle(1.0, \"black\", new_brush(\"Ink Pen\", 0.3, 0.7, 1.0)), lowercase_font(\"loche_upper\"), 2.0, \"a\", #{ spacng: 1 });")
            .expect_err("unknown option").to_string();
        assert!(err.contains("unknown option 'spacng'"), "{}", err);
    }

    #[test]
    fn text_on_a_looping_path_still_avoids_itself() {
        // A small circle can't hold the message twice over: glyphs coming
        // round again must not land on the first ones.
        let script = r#"
            let s = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = lowercase_font("loche_upper");
            let ring = new_circle(new_vec(0.0, 0.0), 30.0);
            ring.fill_with_msg(s, font, 20.0, "abcdefghijklmnopqrstuvwxyz").len()
        "#;
        let copies: i64 = engine().engine.eval(script).expect("should run");
        assert!(copies > 1, "the circle should need more than one copy, got {}", copies);
    }

    #[test]
    fn text_along_a_path_keeps_clear_of_what_it_avoids() {
        // A line of text crossed by a post at x = 300: no glyph's outline
        // comes within the clearance (2) of it, and with the words layout no
        // word is split around it.
        let script = |layout: &str| format!(r#"
            let s = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = lowercase_font("loche_upper");
            let post = new_line(new_vec(300.0, -100.0), new_vec(300.0, 100.0));
            let line = new_line(new_vec(0.0, 0.0), new_vec(2000.0, 0.0));
            let page = new_page();
            page.add_canvas(line.fill_with_msg(s, font, 20.0, "abcdefghijklmnopqrstuvwxyz the quick brown fox jumps over the lazy dog",
                #{{ avoid: post, clearance: 2.0, layout: "{}" }})[0]);
            journal_entry.add_page(page);
        "#, layout);
        for layout in ["flat", "words"] {
            let entry = engine().build_entry_str(&script(layout)).expect("script should run");
            let strokes = &entry.pages[0].canvases()[0].strokes;
            // Strokes are drawn with their hulls inside the glyph's, so check
            // stroke points: the closest may be just under 2 from the hull.
            let mut left = 0;
            for stroke in strokes {
                let xs: Vec<f32> = stroke.path.points_within().iter().map(|p| p.x).collect();
                let (min, max) = xs.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
                assert!(max < 298.5 || min > 301.5, "{}: a glyph spans {}..{}, over the post", layout, min, max);
                if max < 300.0 { left += 1; }
            }
            assert!(left > 0 && left < strokes.len(), "{}: text should be on both sides of the post", layout);
        }
        // "words" moves the whole word that would straddle the post.
        let words = engine().build_entry_str(&script("words")).unwrap();
        let flat = engine().build_entry_str(&script("flat")).unwrap();
        let first_after = |entry: &Entry| entry.pages[0].canvases()[0].strokes.iter()
            .map(|s| s.path.points_within().iter().map(|p| p.x).fold(f32::MAX, f32::min))
            .find(|x| *x > 300.0).unwrap();
        assert!(first_after(&words) >= first_after(&flat));
    }

    const SPLIT: &str = r##"
        let pen = new_brush("Ink Pen", 0.3, 0.7, 3.0);
        let c = new_canvas();
        for i in 0..8 {
            c.add_strokepath(new_line(new_vec(i * 30.0, 0.0), new_vec(300.0 - i * 20.0, 200.0)), new_strokestyle(2.0, "#203040", pen));
        }
        c.add_strokepath(new_circle(new_vec(150.0, 100.0), 90.0), new_strokestyle(3.0, "darkred", pen));
        let other = new_canvas();
        other.name = "dot";
        other.add_strokepath(new_circle(new_vec(20.0, 20.0), 5.0), new_strokestyle(1.0, "black", pen));
        let p = new_page();
        p.name = "big";
        p.svg_style = #{ roughness: 1.5, noise_amplitude: 1.0 };
        p.add_canvas(c);
        p.add_canvas(other);
        p.add_canvas(new_canvas());
        journal_entry.add_page(p);
    "##;

    fn png_pixels(file: &std::path::Path) -> image::RgbaImage {
        image::open(file).unwrap_or_else(|e| panic!("{}: {}", file.display(), e)).to_rgba8()
    }

    #[test]
    fn split_pages_render_tiles_that_make_the_whole() {
        let script = format!("{}\njournal_entry.add_pages(p.split(3, 2));", SPLIT);
        let entry = engine().build_entry_str(&script).expect("script should run");
        let labels: Vec<String> = entry.pages.iter().enumerate().map(|(i, p)| p.label(i)).collect();
        assert_eq!(labels, ["big", "big_0_0", "big_0_1", "big_0_2", "big_1_0", "big_1_1", "big_1_2"]);
        // Tiles keep the canvases' names, and not the empty canvas.
        let names: Vec<String> = entry.pages[1].canvases().iter().enumerate().map(|(i, c)| c.label(i)).collect();
        assert_eq!(names, ["0", "dot"]);
        assert!(entry.empty_canvases() == ["big/2"], "{:?}", entry.empty_canvases());

        let dir = std::env::temp_dir().join(format!("journal_split_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        entry.publish_svgs(dir.to_str().unwrap(), 0.75, None).expect("publish should succeed");
        let whole = png_pixels(&dir.join("big_0.png"));
        let tiles: Vec<image::RgbaImage> = (0..6).map(|i| png_pixels(&dir.join(format!("big_{}_{}_0.png", i / 3, i % 3)))).collect();
        // Side by side, in order, they make the whole canvas's render.
        let widths: u32 = tiles[..3].iter().map(|t| t.width()).sum();
        let heights: u32 = [&tiles[0], &tiles[3]].iter().map(|t| t.height()).sum();
        assert_eq!((widths, heights), whole.dimensions());
        let mut worst = 0;
        let (mut y0, mut diff_sum) = (0, 0u64);
        for row in 0..2 {
            let mut x0 = 0;
            for column in 0..3 {
                let tile = &tiles[row * 3 + column];
                assert!(tile.height() == tiles[row * 3].height(), "tiles in a row share a height");
                for (x, y, p) in tile.enumerate_pixels() {
                    let q = whole.get_pixel(x0 + x, y0 + y);
                    // Compare premultiplied, so transparent pixels' colors don't count.
                    for c in 0..4 {
                        let (a, b) = (p[c] as i32 * p[3] as i32 / 255, q[c] as i32 * q[3] as i32 / 255);
                        let d = (a - b).abs();
                        worst = worst.max(d);
                        diff_sum += d as u64;
                    }
                }
                x0 += tile.width();
            }
            y0 += tiles[row * 3].height();
        }
        // Anti-aliasing can differ a hair where a stroke's outline crosses
        // a seam; nowhere more.
        assert!(worst <= 12, "a pixel differs by {}", worst);
        assert!((diff_sum as f64) / (whole.len() as f64) < 0.01, "mean difference {}", diff_sum as f64 / whole.len() as f64);
        // Every canvas's tiles cover the whole page, the same pixels each.
        assert_eq!(png_pixels(&dir.join("big_1_2_dot.png")).dimensions(), tiles[5].dimensions());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn split_takes_tile_sizes_and_checks_its_options() {
        let script = format!("{}\nlet tiles = p.split(#{{ tile_width: 100, rows: 2 }});\nif tiles.len() != 8 {{ throw \"expected 4 x 2, got \" + tiles.len(); }}\nif tiles[5].tile.column != 1 || tiles[5].tile.rows != 2 {{ throw \"bad tile: \" + tiles[5].tile; }}\nif p.tile != () {{ throw \"the page isn't a tile\"; }}\njournal_entry.add_pages(tiles);", SPLIT);
        let entry = engine().build_entry_str(&script).expect("script should run");
        // 100-unit tiles: the page's ~310 units grow to 400, evenly.
        let canvas: JournalCanvas = entry.pages[1].canvases()[0].clone().into();
        let frame = canvas.frame().expect("tiles have frames");
        assert!((frame.page.extents().x - 400.0).abs() < 1e-3, "{:?}", frame.page);
        let whole = entry.pages[0].render_canvas(&entry.pages[0].canvases()[0]).unwrap().bounds();
        assert!((frame.page.center().x - whole.center().x).abs() < 1e-3);
        for (script, want) in [
            ("let q = new_page();\nq.split(2, 2);", "name the page first"),
            ("let q = new_page(); q.name = \"q\";\nq.split(2, 2);", "nothing drawn on it"),
            ("p.split(0, 2);", "'columns' should be a whole number from 1 to 1000"),
            ("p.split(#{ columns: 2, tile_width: 10 });", "give 'columns' or 'tile_width', not both"),
            ("p.split(#{ tile_height: -1 });", "'tile_height' must be positive"),
            ("p.split(#{ colums: 2 });", "unknown option 'colums'"),
            ("p.split(2, 2)[0].split(2, 2);", "already a tile"),
            ("journal_entry.add_pages([p.split(2, 2)[0], 3]);", "add_pages: item 1 is a i64"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", SPLIT, script)).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn tiles_keep_their_strokes_seeds_from_the_whole_page() {
        let script = format!("{}\njournal_entry.add_pages(p.split(2, 2));", SPLIT);
        let entry = engine().build_entry_str(&script).expect("script should run");
        let whole: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        let tile: JournalCanvas = entry.pages[4].canvases()[0].clone().into();
        assert_eq!(whole.stroke_seeds(), tile.stroke_seeds());
        assert!(tile.strokes().iter().all(|s| s.seed.is_some()));
    }

    #[test]
    fn degenerate_canvases_render_or_are_skipped() {
        let script = r#"
            let s = new_strokestyle(0.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let line = new_canvas();
            line.add_strokepath(new_line(new_vec(0.0, 0.0), new_vec(0.0, 100.0)), s);
            let p = new_page();
            p.add_canvas(line);
            p.add_canvas(new_canvas());
            journal_entry.add_page(p);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        assert_eq!(entry.empty_canvases(), vec!["page_0/1".to_string()]);
        let line: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        assert_eq!((line.render_extents().x, line.render_extents().y), (1.0, 100.0));
        let dir = std::env::temp_dir().join(format!("journal_degenerate_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        entry.publish_svgs(dir.to_str().unwrap(), 1.0, None).expect("publish should succeed");
        assert!(dir.join("page_0_0.png").is_file() && !dir.join("page_0_1.png").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn non_finite_coordinates_fail_where_they_enter() {
        let setup = r#"
            let s = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let c = new_canvas();
        "#;
        for (script, want) in [
            ("new_vec(1.0 / 0.0, 0.0);", "new_vec: coordinates must be finite"),
            ("new_vec(0.0, 0.0 / 0.0);", "new_vec: coordinates must be finite"),
            ("c.add_strokepath(new_circle(new_vec(0.0, 0.0), 5.0).scale_by(0.0 / 0.0), s);", "add_strokepath: the path has a non-finite"),
            ("c.add_glyph(glyph(\"basic_eye\").scale_by(1.0 / 0.0), s);", "add_glyph: the path has a non-finite"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", setup, script)).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn margins_default_to_half_the_largest_brush() {
        let script = r#"
            let thin = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let wide = new_strokestyle(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 6.0));
            let c = new_canvas();
            c.add_strokepath(new_line(new_vec(0.0, 0.0), new_vec(10.0, 10.0)), thin);
            c.add_strokepath(new_line(new_vec(0.0, 0.0), new_vec(10.0, 0.0)), wide);
            let auto = new_page();
            if auto.margin != () { throw "default margin should read as ()"; }
            auto.add_canvas(c);
            journal_entry.add_page(auto);
            let set = new_page();
            set.margin = 20;
            set.add_canvas(c);
            journal_entry.add_page(set);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let canvas = &entry.pages[0].canvases()[0];
        assert_eq!(entry.pages[0].margin_for(canvas), 3.0);
        assert_eq!(entry.pages[1].margin_for(canvas), 20.0);
        let mut padded: JournalCanvas = canvas.into();
        let before = padded.render_extents();
        padded.pad(3.0);
        assert_eq!(padded.render_extents(), before + parry2d::math::Vec2::splat(6.0));
        for (value, want) in [("-1", "at least 0"), ("\"wide\"", "expected a number or ()")] {
            let err = engine().build_entry_str(&format!("let p = new_page();\np.margin = {};", value)).expect_err(value).to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn warn_reports_with_the_line_and_keeps_going() {
        let mut engine = engine();
        engine.build_entry_str("let x = 1;\nwarn(\"too many \" + 3);\njournal_entry.add_page(new_page());")
            .expect("warn shouldn't stop the script");
        assert_eq!(engine.warnings(), vec!["too many 3 (line 2)".to_string()]);
    }

    #[test]
    fn pages_are_found_by_index_or_name() {
        let entry = engine().build_entry_str(NAMED).expect("script should run");
        assert_eq!(entry.page_index("0"), Ok(0));
        assert_eq!(entry.page_index("maze"), Ok(1));
        assert_eq!(entry.page_index("page_0"), Ok(0));
        let err = entry.page_index("2").unwrap_err();
        assert!(err.contains("pages are 0-1: page_0, maze"), "{}", err);
        assert!(entry.page_index("lock").is_err());
    }

    #[test]
    fn names_must_be_unique_and_file_safe() {
        for (script, want) in [
            ("let p = new_page(); p.name = \"maze\"; journal_entry.add_page(p);\njournal_entry.add_page(p);", "already has a page named 'maze'"),
            // An unnamed page's label counts too.
            ("let p = new_page(); p.name = \"page_1\"; journal_entry.add_page(p);\njournal_entry.add_page(new_page());", "already has a page named 'page_1'"),
            ("let p = new_page(); let c = new_canvas(); c.name = \"a\"; p.add_canvas(c);\np.add_canvas(c);", "already has a canvas named 'a'"),
            ("let p = new_page();\np.name = \"../escape\";", "page.name: invalid name '../escape'"),
            ("let c = new_canvas();\nc.name = 3;", "canvas.name: a name must be a string"),
        ] {
            let err = engine().build_entry_str(script).err().unwrap_or_else(|| panic!("should fail: {}", want)).to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn svg_style_defaults_merges_and_turns_off() {
        let script = r#"
            let p = new_page();
            if p.name != () { throw "unnamed pages have no name"; }
            if p.svg_style.roughness != 1.0 || p.svg_style.noise_amplitude != 0.3 { throw "unexpected default: " + p.svg_style; }
            p.svg_style = #{ roughness: 2, noise_amplitude: 1.5 };
            if p.svg_style.roughness != 2.0 || p.svg_style.trail_off_percent != 0.15 { throw "not merged: " + p.svg_style; }
            journal_entry.add_page(p);
            let plain = new_page();
            plain.svg_style = ();
            journal_entry.add_page(plain);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let style = entry.pages[0].svg_style.as_ref().expect("styled");
        assert_eq!((style.roughness, style.point_sample_distance, style.noise_amplitude), (2.0, 1.0, 1.5));
        assert!(entry.pages[1].svg_style.is_none());
        for (script, want) in [
            ("let p = new_page();\np.svg_style = #{ roughnes: 1 };", "unknown option 'roughnes'"),
            ("let p = new_page();\np.svg_style = #{ roughness: -1 };", "'roughness' can't be negative"),
            ("let p = new_page();\np.svg_style = 2;", "expected a map of options or ()"),
        ] {
            let err = engine().build_entry_str(script).expect_err("script should fail").to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn brushes_carry_a_pressure_profile() {
        let script = r#"
            let b = new_brush("Ink Pen", 0.2, 0.8, 1.0);
            if b.pressure_profile != "ramp" { throw "default should be ramp"; }
            b.pressure_profile = "taper";
            let c = new_canvas();
            c.add_strokepath(new_line(new_vec(0.0, 0.0), new_vec(10.0, 0.0)), new_strokestyle(1.0, "black", b));
            let p = new_page();
            p.add_canvas(c);
            journal_entry.add_page(p);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let jc: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        assert_eq!(jc.strokes()[0].style.brush.as_ref().unwrap().pressure_profile, PressureProfile::Taper);
        let err = engine().build_entry_str("let b = new_brush(\"Ink Pen\", 0.2, 0.8, 1.0);\nb.pressure_profile = \"wobbly\";")
            .expect_err("unknown profile should fail").to_string();
        assert!(err.contains("unknown profile 'wobbly'") && err.contains("ramp, taper, constant") && err.contains("line 2"), "{}", err);
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
