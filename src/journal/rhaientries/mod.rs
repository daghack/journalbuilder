mod glyphs;
mod layout;
mod locks;
mod random;

use rhai::{Engine, EvalAltResult, ImmutableString, Dynamic, CustomType, TypeBuilder, Scope};
use super::paths::StrokePath;
use super::symboldict::SymbolPathDict;
use super::canvas::Canvas as JournalCanvas;
use super::canvas::StrokeStyle as JournalStrokeStyle;
use super::canvas::CanvasStyleOptions;
use super::canvas::{Brush, PressureProfile};
use super::symbol::{place_along_path, SymbolPlacementArgs};
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
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder.with_fn("add_page", Self::add_page);
    }
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
    /// `page/canvas` labels of the canvases that have nothing on them, which
    /// publishing skips.
    pub fn empty_canvases(&self) -> Vec<String> {
        self.pages.iter().enumerate().flat_map(|(i, page)| {
            page.canvases.iter().enumerate()
                .filter(|(_, canvas)| canvas.strokes.is_empty())
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
            let canvases = page.canvases();
            for (canvas_i, canvas) in canvases.iter().enumerate() {
                let filename = format!("{}/darkly_{}_{}.png", dirname, page.label(i), canvas.label(canvas_i));
                let canvas : JournalCanvas = canvas.into();
                if !canvas.has_strokes() {
                    continue; // journalbuild warns about these (see empty_canvases)
                }
                let canvas_dim = canvas.render_extents();
                // Canvas units are pixels at scale 1, so brushes scale with
                // the render.
                let buff = darkly::render_canvas_scaled(
                    &canvas, engine,
                    ((canvas_dim.x * scale) as u32).max(1), ((canvas_dim.y * scale) as u32).max(1),
                    scale, supersample,
                );
                buff.save(&filename).map_err(|e| format!("couldn't write {}: {}", filename, e))?;
            }
        }
        Ok(())
    }
    /// Plain renders, one per canvas like the Darkly ones:
    /// `{page}_{canvas}.png`.
    pub fn publish_pngs(&self, dirname: &str, scale: f32, only: Option<&[usize]>) -> Result<(), String> {
        println!("Publishing PNGs:");
        for (i, page) in self.selected_pages(only) {
            for (canvas_i, canvas) in page.canvases().iter().enumerate() {
                let name = format!("{}_{}", page.label(i), canvas.label(canvas_i));
                let filename = format!("{}/{}.png", dirname, name);
                let canvas: JournalCanvas = canvas.into();
                if !canvas.has_strokes() {
                    continue;
                }
                // Seeded by the file's name: the same every run, but each
                // canvas roughened differently.
                let png = canvas.to_png(2.0 * scale, page.plain_style.as_ref(), stable_hash(&name));
                println!("\t - {}", filename);
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

/// How a page's plain (non-Darkly) output is drawn, unless the script sets
/// `page.plain_style`.
const DEFAULT_PLAIN_STYLE: CanvasStyleOptions = CanvasStyleOptions {
    roughness: 1.0,
    point_sample_distance: 1.0,
    trail_off_percent: 0.15,
    noise_amplitude: 0.3,
};

const PLAIN_STYLE_KEYS: [&str; 4] = ["roughness", "point_sample_distance", "trail_off_percent", "noise_amplitude"];

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Page {
    canvases: Vec<Canvas>,
    /// Output files are named after this, or `page_{index}` without one.
    #[rhai_type(skip)]
    name: Option<String>,
    /// `None` draws the plain output as simple strokes.
    #[rhai_type(skip)]
    plain_style: Option<CanvasStyleOptions>,
}

impl Default for Page {
    fn default() -> Self {
        Page {
            canvases: Vec::new(),
            name: None,
            plain_style: Some(DEFAULT_PLAIN_STYLE),
        }
    }
}

impl Page {
    pub fn add_canvas(&mut self, canvas: Canvas) -> Result<(), Box<EvalAltResult>> {
        let label = canvas.label(self.canvases.len());
        if self.canvases.iter().enumerate().any(|(i, c)| c.label(i) == label) {
            return Err(format!("add_canvas: the page already has a canvas named '{}'", label).into());
        }
        self.canvases.push(canvas);
        Ok(())
    }
    pub fn canvases(&self) -> &Vec<Canvas> {
        &self.canvases
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
    fn get_plain_style(&mut self) -> Dynamic {
        match &self.plain_style {
            None => Dynamic::UNIT,
            Some(style) => {
                let mut map = rhai::Map::new();
                for (key, value) in PLAIN_STYLE_KEYS.iter().zip([style.roughness, style.point_sample_distance, style.trail_off_percent, style.noise_amplitude]) {
                    // Via the shortest decimal form, so 0.3 reads back as 0.3
                    // rather than 0.30000001192092896.
                    let value: f64 = value.to_string().parse().expect("a float's string form parses");
                    map.insert((*key).into(), Dynamic::from(value));
                }
                Dynamic::from_map(map)
            }
        }
    }
    /// `()` for plain strokes, or a map of options; any left out keep their
    /// defaults.
    fn set_plain_style(&mut self, style: Dynamic) -> Result<(), Box<EvalAltResult>> {
        const CALLER: &str = "page.plain_style";
        if style.is_unit() {
            self.plain_style = None;
            return Ok(());
        }
        let type_name = style.type_name();
        let options = style.try_cast::<rhai::Map>()
            .ok_or_else(|| -> Box<EvalAltResult> { format!("{}: expected a map of options or (), got {}", CALLER, type_name).into() })?;
        layout::check_keys(CALLER, &options, &PLAIN_STYLE_KEYS)?;
        let get = |key: &str, default: f32| -> Result<f32, Box<EvalAltResult>> {
            match layout::number(CALLER, &options, key)? {
                Some(n) if n >= 0.0 => Ok(n),
                Some(n) => Err(format!("{}: '{}' can't be negative, got {}", CALLER, key, n).into()),
                None => Ok(default),
            }
        };
        let d = DEFAULT_PLAIN_STYLE;
        let point_sample_distance = get("point_sample_distance", d.point_sample_distance)?;
        if point_sample_distance == 0.0 {
            return Err(format!("{}: 'point_sample_distance' must be positive", CALLER).into());
        }
        self.plain_style = Some(CanvasStyleOptions {
            roughness: get("roughness", d.roughness)?,
            point_sample_distance,
            trail_off_percent: get("trail_off_percent", d.trail_off_percent)?,
            noise_amplitude: get("noise_amplitude", d.noise_amplitude)?,
        });
        Ok(())
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("add_canvas", Self::add_canvas)
            .with_fn("new_page", Self::default)
            .with_get_set("name", Self::get_name, Self::set_name)
            .with_get_set("plain_style", Self::get_plain_style, Self::set_plain_style)
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
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_vec", Self::new)
        ;
    }
}

impl From<Vec2> for PVec {
    fn from(val: Vec2) -> Self {
        PVec::new(val.x as f32, val.y as f32)
    }
}

#[derive(Debug, Default, Clone)]
#[derive(CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Canvas {
    pub strokes: Vec<Stroke>,
    /// Its Darkly output is named after this, or its index on the page.
    #[rhai_type(skip)]
    name: Option<String>,
}

impl Canvas {
    /// The canvas's name in output files, given its index on its page.
    pub fn label(&self, index: usize) -> String {
        self.name.clone().unwrap_or_else(|| index.to_string())
    }
    fn get_name(&mut self) -> Dynamic {
        self.name.clone().map_or(Dynamic::UNIT, Dynamic::from)
    }
    fn set_name(&mut self, name: Dynamic) -> Result<(), Box<EvalAltResult>> {
        self.name = Some(check_name("canvas.name", name)?);
        Ok(())
    }
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
    // Transforms modify the canvas in place and also return it, matching
    // the StrokePath functions, so they can be chained (with the same
    // caveat: a chain on a variable changes it only by its first call).
    pub fn translate(&mut self, v: Vec2) -> Canvas {
        let v = v.into();
        for stroke in &mut self.strokes {
            stroke.path.translate_by(v);
        }
        self.clone()
    }
    pub fn scale(&mut self, scale: f64) -> Canvas {
        for stroke in &mut self.strokes {
            stroke.path.scale_by(scale as f32);
        }
        self.clone()
    }
    pub fn rotate_around(&mut self, point: Vec2, rad: f64) -> Canvas {
        let point = point.into();
        for stroke in &mut self.strokes {
            stroke.path.rotate_around(point, rad as f32);
        }
        self.clone()
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
            .with_get_set("name", Self::get_name, Self::set_name)
        ;
    }
}

impl From<Canvas> for JournalCanvas {
    fn from(val: Canvas) -> Self {
        let mut canvas = JournalCanvas::new();
        for stroke in val.strokes {
            let Stroke {
                path,
                style,
            } = stroke;
            canvas.add_strokes(path, style.into());
        }
        canvas
    }
}

impl From<&Canvas> for JournalCanvas {
    fn from(val: &Canvas) -> Self {
        let mut canvas = JournalCanvas::new();
        for stroke in val.strokes.iter() {
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
    /// `new_strokestyle(width, color, brush)`. In the plain output `width` is
    /// the stroke's width in canvas units; in Darkly it multiplies the
    /// brush's pressure (which Darkly caps at 1), and the brush's size sets
    /// the mark's width.
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

/// Apply a by-value StrokePath builder method to a path in place, returning
/// the updated path.
fn update_path(path: &mut StrokePath, f: impl FnOnce(StrokePath) -> StrokePath) -> StrokePath {
    *path = f(std::mem::take(path));
    path.clone()
}

pub struct JournalEngine {
    engine: Engine,
    warnings: Rc<RefCell<Vec<String>>>,
}

impl JournalEngine {
    pub fn new(symdict: Rc<SymbolPathDict>) -> Self {
        let mut engine = Engine::new();
        let rng = random::register(&mut engine);
        let fit_msg_symdict = symdict.clone();
        let fill_msg_symdict = symdict.clone();
        let fill_msg_rng = rng.clone();
        let fit_msg_rng = rng.clone();

        engine.register_type::<StrokePath>()
            // Every path function both modifies the path in place and returns
            // it, so `p.line_to(v);`, `p = p.line_to(v);` and chained calls on
            // temporaries all behave the same. A chain on a variable is the
            // exception: `p.line_to(a).line_to(b);` changes `p` only by the
            // first call (rhai passes the variable by reference to the first
            // call only), so write `p = p.line_to(a).line_to(b);` instead.
            .register_fn("move_to", |path: &mut StrokePath, pos: Vec2| -> StrokePath {
                update_path(path, |p| p.move_to(pos.x as f32, pos.y as f32))
            })
            .register_fn("line_to", |path: &mut StrokePath, pos: Vec2| -> StrokePath {
                update_path(path, |p| p.line_to(pos.x as f32, pos.y as f32))
            })
            // The message along as many copies of the path as it takes, one
            // canvas per copy.
            .register_fn("fill_with_msg", move |path: &mut StrokePath, style: StrokeStyle, font: Font, fontsize: f64, msg: &str| -> Result<Vec<Dynamic>, Box<EvalAltResult>> {
                if !(fontsize > 0.0) {
                    return Err(format!("fill_with_msg: font size must be positive, got {}", fontsize).into());
                }
                let builder = font.builder(&fill_msg_symdict);
                let args = SymbolPlacementArgs{
                    avoid_kdtree: None, 
                    symbol_scale: fontsize as f32,
                };

                let mut all_canvases = Vec::new();
                let mut symbols = builder.build(msg, &mut *fill_msg_rng.borrow_mut());
                let total = symbols.len();
                while !symbols.is_empty() {
                    let (placed, unplaced) = place_along_path(std::mem::take(&mut symbols), path, &args);
                    if placed.is_empty() {
                        // Another copy of the path wouldn't fit it either.
                        return Err(format!(
                            "fill_with_msg: glyph {} of {} doesn't fit on the path at font size {}; use a longer path or a smaller font",
                            total - unplaced.len() + 1, total, fontsize,
                        ).into());
                    }
                    let mut canvas = Canvas::default();
                    for symbol in placed {
                        canvas.add_strokepath(symbol.placed_strokepath(), style.clone());
                    }
                    all_canvases.push(Dynamic::from(canvas));
                    symbols = unplaced;
                }
                Ok(all_canvases)
            })
            .register_fn("fit_msg_to",
                move |path: &mut StrokePath, style: StrokeStyle, font: Font, msg: &str| -> Result<Canvas, Box<EvalAltResult>> {
                let mut canvas = Canvas::default();
                let builder = font.builder(&fit_msg_symdict);
                let symbols = builder.build(msg, &mut *fit_msg_rng.borrow_mut());
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
                    if !unplaced.is_empty() {
                        window_max = current_scale;
                    } else {
                        window_min = current_scale;
                    }
                }
                args.symbol_scale = window_min;
                let (placed, unplaced) = place_along_path(symbols, path, &args);
                if !unplaced.is_empty() {
                    return Err(format!(
                        "fit_msg_to: the message doesn't fit on the path even at font size {}: {} of {} glyphs left over",
                        window_min, unplaced.len(), placed.len() + unplaced.len(),
                    ).into());
                }
                for symbol in placed {
                    canvas.add_strokepath(symbol.placed_strokepath(), style.clone());
                }
                Ok(canvas)
            })
            .register_fn("scale_by", |stroke: &mut StrokePath, scale: f64| -> StrokePath {
                stroke.scale_by(scale as f32);
                stroke.clone()
            })
            .register_fn("translate_by", |stroke: &mut StrokePath, translate: Vec2| -> StrokePath {
                stroke.translate_by(translate.into());
                stroke.clone()
            })
            .register_fn("rotate_around",
                |stroke: &mut StrokePath, rot_point: Vec2, rot: f64| -> StrokePath {
                    stroke.rotate_around(rot_point.into(), rot as f32);
                    stroke.clone()
                },
            )
            .register_fn("new_strokepath", StrokePath::new)
            .register_fn("cubic_to", |path: &mut StrokePath, c1: Vec2, c2: Vec2, end: Vec2| -> StrokePath {
                update_path(path, |p| p.cubic_curve_to(
                    c1.x as f32, c1.y as f32, c2.x as f32, c2.y as f32, end.x as f32, end.y as f32,
                ))
            })
            .register_fn("quad_to", |path: &mut StrokePath, control: Vec2, end: Vec2| -> StrokePath {
                update_path(path, |p| p.quadratic_curve_to(
                    control.x as f32, control.y as f32, end.x as f32, end.y as f32,
                ))
            })
            .register_fn("close", |path: &mut StrokePath| -> StrokePath {
                update_path(path, |p| p.close_path())
            })
            .register_fn("new_circle", |center: Vec2, radius: f64| -> StrokePath {
                StrokePath::circle(center.into(), radius as f32)
            })
            .register_fn("new_rectangle", |center: Vec2, width: f64, height: f64| -> StrokePath {
                StrokePath::rectangle(center.into(), width as f32, height as f32)
            })
            .register_fn("new_line", |start: Vec2, end: Vec2| -> StrokePath {
                StrokePath::line(start.into(), end.into())
            })
            .register_fn("new_arc", |center: Vec2, radius: f64, start_rad: f64, end_rad: f64| -> StrokePath {
                StrokePath::arc(center.into(), radius as f32, start_rad as f32, end_rad as f32)
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

        glyphs::register(&mut engine, symdict.clone(), rng.clone());
        layout::register(&mut engine, symdict.clone(), rng.clone());
        locks::register(&mut engine);

        engine
            .build_type::<Vec2>()
            .build_type::<Entry>()
            .build_type::<Page>()
            .build_type::<Canvas>()
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
        let path: StrokePath = engine.engine.eval(entry_script).expect("script should evaluate");
        path.to_string()
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
        assert_eq!(statements, "M 1 1 L 3 1 ");
        assert_eq!(chained, statements);
        assert_eq!(reassigned, statements);
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
        let named = ring;
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
        entry.publish_pngs(dir.to_str().unwrap(), 0.1, Some(&[1])).expect("publish should succeed");
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
        let parts: rhai::Array = engine().engine.eval(&format!("{}\nnew_line(new_vec(0.0, 0.0), new_vec(60.0, 0.0)).fill_with_msg(s, font, 10.0, \"abcdefghij\")", setup)).unwrap();
        assert!(parts.len() > 1);
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
        entry.publish_pngs(dir.to_str().unwrap(), 1.0, None).expect("publish should succeed");
        assert!(dir.join("page_0_0.png").is_file() && !dir.join("page_0_1.png").exists());
        std::fs::remove_dir_all(&dir).unwrap();
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
    fn plain_style_defaults_merges_and_turns_off() {
        let script = r#"
            let p = new_page();
            if p.name != () { throw "unnamed pages have no name"; }
            if p.plain_style.roughness != 1.0 || p.plain_style.noise_amplitude != 0.3 { throw "unexpected default: " + p.plain_style; }
            p.plain_style = #{ roughness: 2, noise_amplitude: 1.5 };
            if p.plain_style.roughness != 2.0 || p.plain_style.trail_off_percent != 0.15 { throw "not merged: " + p.plain_style; }
            journal_entry.add_page(p);
            let plain = new_page();
            plain.plain_style = ();
            journal_entry.add_page(plain);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let style = entry.pages[0].plain_style.as_ref().expect("styled");
        assert_eq!((style.roughness, style.point_sample_distance, style.noise_amplitude), (2.0, 1.0, 1.5));
        assert!(entry.pages[1].plain_style.is_none());
        for (script, want) in [
            ("let p = new_page();\np.plain_style = #{ roughnes: 1 };", "unknown option 'roughnes'"),
            ("let p = new_page();\np.plain_style = #{ roughness: -1 };", "'roughness' can't be negative"),
            ("let p = new_page();\np.plain_style = 2;", "expected a map of options or ()"),
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
