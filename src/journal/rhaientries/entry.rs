//! The journal entry and its pages, as rhai scripts build them.

use rhai::{CustomType, Dynamic, Engine, EvalAltResult, Map, NativeCallContext, TypeBuilder};

use super::options::{apply_props, check_keys, number};
use super::{Canvas, CanvasData, RhaiResult, Vec2};
use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2 as PVec;
use crate::journal::canvas::Canvas as JournalCanvas;
use crate::journal::canvas::{SvgStyle, Frame};
use crate::journal::stable_hash;

#[derive(Debug, Default, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Entry {
    #[rhai_type(skip)]
    pub(super) pages: Vec<Page>,
}

impl Entry {
    fn add_page(&mut self, page: Page) -> Result<(), Box<EvalAltResult>> {
        let label = page.label(self.pages.len());
        if self.pages.iter().enumerate().any(|(i, p)| p.label(i) == label) {
            return Err(format!("journal_entry.add: the entry already has a page named '{}'", label).into());
        }
        self.pages.push(page);
        Ok(())
    }
    /// `journal_entry.add(pages)`, such as a split page's tiles.
    fn add_pages(&mut self, pages: rhai::Array) -> Result<(), Box<EvalAltResult>> {
        for (i, page) in pages.into_iter().enumerate() {
            let type_name = page.type_name();
            let page = page.try_cast::<Page>()
                .ok_or_else(|| format!("journal_entry.add: item {} is a {}, not a page", i, type_name))?;
            self.add_page(page)?;
        }
        Ok(())
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("add", Self::add_page)
            .with_fn("add", Self::add_pages);
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
    pub fn selected_pages<'a>(&'a self, only: Option<&'a [usize]>) -> impl Iterator<Item = (usize, &'a Page)> + 'a {
        self.pages.iter().enumerate()
            .filter(move |(i, _)| only.is_none_or(|pages| pages.contains(i)))
    }
}

/// Names become file names, so they're limited to letters, digits, `_`,
/// `-` and `.` (not leading).
pub(super) fn check_name(caller: &str, name: Dynamic) -> Result<String, Box<EvalAltResult>> {
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


/// What `new_page` takes, as the page's properties.
const PAGE_PROPS: [&str; 4] = ["name", "size", "margin", "svg_style"];

const SVG_STYLE_KEYS: [&str; 4] = ["roughness", "point_sample_distance", "trail_off_percent", "noise_amplitude"];

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Page {
    #[rhai_type(skip)]
    pub(super) canvases: Vec<CanvasData>,
    /// Output files are named after this, or `page_{index}` without one.
    #[rhai_type(skip)]
    pub(super) name: Option<String>,
    /// `None` draws the SVG output as simple strokes.
    #[rhai_type(skip)]
    pub(super) svg_style: Option<SvgStyle>,
    /// Space around each canvas's strokes, in canvas units; `None` for the
    /// default (see `margin_for`).
    #[rhai_type(skip)]
    pub(super) margin: Option<f64>,
    /// A set size (`page.size`): the page is the region from the origin to
    /// this, its canvases cut off at its edges. `None` for as big as its
    /// strokes.
    #[rhai_type(skip)]
    pub(super) size: Option<PVec>,
}

impl Default for Page {
    fn default() -> Self {
        Page {
            canvases: Vec::new(),
            name: None,
            svg_style: Some(SvgStyle::DEFAULT),
            margin: None,
            size: None,
        }
    }
}

impl Page {
    /// `page.add(canvas)` adds a copy of the canvas as it is now, and
    /// returns the page, so `new_page(#{ ... }).add(canvas)` chains.
    pub fn add_canvas(&mut self, canvas: Canvas) -> Result<Page, Box<EvalAltResult>> {
        let canvas = canvas.get();
        let label = canvas.label(self.canvases.len());
        if self.canvases.iter().enumerate().any(|(i, c)| c.label(i) == label) {
            return Err(format!("page.add: the page already has a canvas named '{}'", label).into());
        }
        self.canvases.push(canvas);
        Ok(self.clone())
    }
    pub fn canvases(&self) -> &Vec<CanvasData> {
        &self.canvases
    }
    /// The canvases with something to render, as they're rendered (see
    /// `render_canvas`), with their names in output.
    pub fn rendered_canvases(&self) -> impl Iterator<Item = (String, JournalCanvas)> + '_ {
        self.canvases.iter().enumerate()
            .filter_map(|(i, canvas)| self.render_canvas(canvas).map(|rendered| (canvas.label(i), rendered)))
    }
    /// How the page's SVG output is styled; `None` for plain strokes.
    pub fn svg_style(&self) -> Option<&SvgStyle> {
        self.svg_style.as_ref()
    }
    /// `canvas` as it's rendered: padded by its margin, or a tile's frame.
    /// `None` if there's nothing to render.
    pub fn render_canvas(&self, canvas: &CanvasData) -> Option<JournalCanvas> {
        let mut rendered: JournalCanvas = canvas.into();
        if rendered.frame().is_none() {
            if !rendered.has_strokes() {
                return None;
            }
            match self.size {
                Some(size) => rendered.set_frame(Some(Frame::whole(Aabb::new(PVec::ZERO, size)))),
                None => rendered.pad(self.margin_for(canvas)),
            }
        }
        Some(rendered)
    }
    fn get_size(&mut self) -> Dynamic {
        self.size.map_or(Dynamic::UNIT, |s| Dynamic::from(Vec2::new(s.x as f64, s.y as f64)))
    }
    /// `page.size = vec2(width, height)`: the page is the region from
    /// the origin to there, whatever is drawn on it, and strokes past its
    /// edges are cut off; `()` for as big as its strokes (the default).
    fn set_size(&mut self, size: Dynamic) -> RhaiResult<()> {
        if size.is_unit() {
            self.size = None;
            return Ok(());
        }
        let type_name = size.type_name();
        let size: PVec = size.try_cast::<Vec2>()
            .ok_or_else(|| -> Box<EvalAltResult> { format!("page.size: expected a vec (width, height) or (), got {}", type_name).into() })?
            .into();
        if !(size.x >= 1.0 && size.y >= 1.0) {
            return Err(format!("page.size: width and height must be at least 1, got ({}, {})", size.x, size.y).into());
        }
        self.size = Some(size);
        Ok(())
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
        check_keys(CALLER, &options, &SVG_STYLE_KEYS)?;
        let get = |key: &str, default: f32| -> Result<f32, Box<EvalAltResult>> {
            match number(CALLER, &options, key)? {
                Some(n) if n >= 0.0 => Ok(n),
                Some(n) => Err(format!("{}: '{}' can't be negative, got {}", CALLER, key, n).into()),
                None => Ok(default),
            }
        };
        let d = SvgStyle::DEFAULT;
        let point_sample_distance = get("point_sample_distance", d.point_sample_distance)?;
        if point_sample_distance == 0.0 {
            return Err(format!("{}: 'point_sample_distance' must be positive", CALLER).into());
        }
        self.svg_style = Some(SvgStyle {
            roughness: get("roughness", d.roughness)?,
            point_sample_distance,
            trail_off_percent: get("trail_off_percent", d.trail_off_percent)?,
            noise_amplitude: get("noise_amplitude", d.noise_amplitude)?,
        });
        Ok(())
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
        check_keys(CALLER, &options, &["columns", "rows", "tile_width", "tile_height"])?;
        let name = self.name.clone()
            .ok_or_else(|| format!("{}: name the page first; its tiles are named after it ({{name}}_{{row}}_{{column}})", CALLER))?;
        // Typeset pages are framed to the whole page: they split by it.
        let whole = |frame: &Frame| frame.columns == 1 && frame.rows == 1;
        if self.canvases.iter().any(|canvas| canvas.frame.is_some_and(|f| !whole(&f))) {
            return Err(format!("{}: the page is already a tile; split the page it came from instead", CALLER).into());
        }
        let canvases: Vec<(usize, &CanvasData)> = self.canvases.iter().enumerate()
            .filter(|(_, canvas)| !canvas.strokes.is_empty() || canvas.frame.is_some()).collect();
        // A page of a set size splits by it; others by what their canvases
        // render.
        let mut region = match self.size {
            Some(size) => Aabb::new(PVec::ZERO, size),
            None => {
                let mut region = Aabb::new_invalid();
                for (_, canvas) in &canvases {
                    let bounds = match canvas.frame {
                        Some(frame) => frame.page,
                        None => self.render_canvas(canvas).expect("the canvas has strokes").bounds(),
                    };
                    region.take_point(bounds.mins);
                    region.take_point(bounds.maxs);
                }
                region
            }
        };
        if canvases.is_empty() {
            return Err(format!("{}: the page has nothing drawn on it to split", CALLER).into());
        }
        // How many tiles across and down, growing the region to a whole
        // number of tiles where they have a set size.
        let mut grid = |count_key: &str, size_key: &str, axis: usize| -> Result<u32, Box<EvalAltResult>> {
            let extent = region.extents()[axis].max(1.0);
            match (options.get(count_key), number(CALLER, &options, size_key)?) {
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
                    // Each tile is framed to its part of the page.
                    size: None,
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
            .with_fn("add", Self::add_canvas)
            .with_fn("new_page", Self::default)
            .with_fn("new_page", |ctx: NativeCallContext, options: Map| apply_props(&ctx, "new_page", Page::default(), options, &PAGE_PROPS))
            .with_get_set("name", Self::get_name, Self::set_name)
            .with_get_set("svg_style", Self::get_svg_style, Self::set_svg_style)
            .with_get_set("margin", Self::get_margin, Self::set_margin)
            .with_get_set("size", Self::get_size, Self::set_size)
            .with_fn("split", |page: &mut Page| page.split_with(Map::new()))
            .with_fn("split", Self::split_with)
            .with_get("tile", Self::get_tile)
        ;
    }
}

pub(super) fn register(engine: &mut Engine) {
    engine
        .build_type::<Entry>()
        .build_type::<Page>();
}
