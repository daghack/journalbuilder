mod locks;
mod random;

use rhai::{Engine, EvalAltResult, ImmutableString, Array, Dynamic, CustomType, TypeBuilder, Scope};
use std::io::{Error, ErrorKind};
use super::paths::StrokePath;
use super::symboldict::SymbolPathDict;
use super::canvas::Canvas as JournalCanvas;
use super::canvas::StrokeStyle as JournalStrokeStyle;
use super::canvas::CanvasStyleOptions;
use super::canvas::Brush;
use super::symbol::{place_along_path, SymbolPlacementArgs};
use super::styles;
use super::canvas::artistic::{BuiltMaze, MazeTraversalStrategy};
use super::darkly;
use ::darkly::engine::DarklyEngine;
use parry2d::math::Vec2 as PVec;

use std::sync::Arc;

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
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
    /// Pages to publish with their original indices: all of them, or only
    /// those listed in `only`.
    fn selected_pages<'a>(&'a self, only: Option<&'a [usize]>) -> impl Iterator<Item = (usize, &'a Page)> + 'a {
        self.pages.iter().enumerate()
            .filter(move |(i, _)| only.map_or(true, |pages| pages.contains(i)))
    }
    pub fn darkly_publish_pngs(&self, engine: &mut DarklyEngine, dirname: &str, scale: f32, supersample: u32, only: Option<&[usize]>) {
        for (i, page) in self.selected_pages(only) {
            let canvases = page.canvases();
            for (canvas_i, canvas) in canvases.into_iter().enumerate() {
                let canvas : JournalCanvas = canvas.into();
                let canvas_dim = canvas.bounds().extents();
                // Canvas units are pixels at scale 1, so brushes scale with
                // the render. Every rhai stroke style carries a brush, so no
                // default is needed.
                let buff = darkly::render_canvas_scaled(
                    &canvas, engine, None,
                    (canvas_dim.x * scale) as u32, (canvas_dim.y * scale) as u32,
                    scale, supersample,
                );
                let filename = format!("{}/darkly_page_{}_{}.png", dirname, i, canvas_i);
                buff.save(filename).expect("Failed to save png");
            }
        }
    }
    pub fn publish_pngs(&self, dirname: &str, scale: f32, only: Option<&[usize]>) {
        println!("Publishing PNGs:");
        for (i, page) in self.selected_pages(only) {
            let page_canvas: JournalCanvas = page.clone().into();
            let canvas_style = CanvasStyleOptions {
                roughness: 1.0,
                point_sample_distance: 1.0,
                trail_off_percent: 0.15,
                noise_amplitude: 0.3,
            };
            let png = page_canvas.to_png(2.0 * scale, Some(&canvas_style));
            let filename = format!("{}/page_{}.png", dirname, i);
            println!("\t - {}", filename);
            let failmsg = format!("Failed to write page: {}", i);
            std::fs::write(filename, png).expect(&failmsg);
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
    // Transforms modify the canvas in place and also return it, matching
    // the StrokePath functions, so they can be chained.
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

/// Apply a by-value StrokePath builder method to a path in place, returning
/// the updated path.
fn update_path(path: &mut StrokePath, f: impl FnOnce(StrokePath) -> StrokePath) -> StrokePath {
    *path = f(std::mem::replace(path, StrokePath::new()));
    path.clone()
}

pub struct JournalEngine {
    engine: Engine,
}

impl JournalEngine {
    pub fn new(symdict: Arc<SymbolPathDict>) -> Self {
        let mut engine = Engine::new();
        let fit_msg_symdict = symdict.clone();
        let fill_msg_symdict = symdict.clone();

        engine.register_type::<StrokePath>()
            // Every path function both modifies the path in place and returns
            // it, so `p.line_to(v);`, `p = p.line_to(v);` and chained calls on
            // temporaries all behave the same.
            .register_fn("move_to", |path: &mut StrokePath, pos: Vec2| -> StrokePath {
                update_path(path, |p| p.move_to(pos.x as f32, pos.y as f32))
            })
            .register_fn("line_to", |path: &mut StrokePath, pos: Vec2| -> StrokePath {
                update_path(path, |p| p.line_to(pos.x as f32, pos.y as f32))
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
            .register_fn("left_handed_maze_path", |seed: i64, width: i64, height: i64| -> StrokePath {
                let seed = seed as u64;
                let width = width as u64;
                let height = height as u64;
                let maze = BuiltMaze::<[i64; 1]>::new(seed, width as u32, height as u32);
                maze.corridor_path(MazeTraversalStrategy::LeftHanded)
            })
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
            Ok(BrushSettings::new_brush(name, min_pressure, max_pressure, size))
        });

        locks::register(&mut engine);
        random::register(&mut engine);

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
            // Non-lowercase characters (punctuation included) use the upper fonts.
            if missing["~"].tried != ["copper_upper_~"] { throw "unexpected tried list: " + missing["~"].tried; }
        "#;
        engine().build_entry_str(script).expect("script should run");
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
