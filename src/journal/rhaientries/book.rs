//! Page layouts for rhai scripts: a page is a tree of boxes laid out by
//! flexbox rules (with [stretch](https://github.com/vislyhq/stretch)), and
//! `typeset` pours text, art and ornaments into its slots, page after page,
//! until the text runs out.
//!
//! A layout is a rhai function from a page number to that page's tree,
//! usually in a layout file, `layouts/<name>.rhai`, which defines
//! `page(n, info)` and, optionally, `page_size()`:
//!
//! ```rhai
//! // layouts/two_columns.rhai
//! fn page_size() { #{ width: 1400, height: 1000 } }
//! fn page(n, info) {
//!     row(#{ padding: 80, gap: 60 }, [
//!         column(#{ grow: 1 }, [
//!             if n == 0 { flow("title", #{ role: "heading", height: 90, fit: true }) } else { spacer() },
//!             flow("body", #{ grow: 1 }),
//!         ]),
//!         band("divider", #{ width: 20 }),
//!         flow("body", #{ grow: 1 }),
//!     ])
//! }
//! ```
//!
//! A journal fills one with text, roles and art, and adds the pages:
//!
//! ```rhai
//! let pages = layout("two_columns").typeset(#{
//!     name: "lunar",                       // pages lunar_0, lunar_1, ...
//!     flows: #{ title: "Lunar Influence", body: long_text },
//!     roles: #{
//!         body: #{ font: copper, style: ink, size: 22.0, align: "justify" },
//!         heading: #{ font: copper_caps, style: red, size: 60.0, align: "center" },
//!         divider: #{ style: red },
//!     },
//!     art: #{ divider: knot_path },
//! });
//! journal_entry.add_pages(pages);
//! ```
//!
//! `new_layout("name", Fn("my_page"))` makes one from a function in the
//! journal instead, and `width`/`height` in the content give a page size.
//!
//! **Boxes.** `row(style, children)` and `column(style, children)` lay their
//! children out across or down (the style is optional). Leaves:
//!
//! - `flow(name, style)`: text from the flow `name`. A flow runs through its
//!   slots in the order they come in the tree, page after page: what
//!   doesn't fit one slot goes on in the next.
//! - `label(text, style)`: fixed text, such as a page number.
//! - `initial(name, style)`: a drop cap, the next letter of the flow,
//!   filling its box. Give it `position: "absolute"` in a column with the
//!   flow's slot, and the text wraps around it.
//! - `figure(name, style)`: art from `art`, fitted into the box.
//! - `band(name, style)`: art repeated along the box, an ornamental strip.
//! - `rule(style)`: a line along the box. `spacer(style)`: nothing.
//!
//! **Style.** Sizes and positions are canvas units, or `"50%"` of the
//! parent, or `"auto"`: `width`, `height`, `min_width`, `min_height`,
//! `max_width`, `max_height`, `basis`, `top`, `left`, `right`, `bottom`;
//! `margin` and `padding` take one value, `[vertical, horizontal]`,
//! `[top, right, bottom, left]` or `#{ top, right, bottom, left }`. `grow`
//! and `shrink` share out space (a box that grows with no size starts from
//! nothing); `aspect` fixes width / height; `position: "absolute"` takes a
//! box out of the flow, placed by `top`/`left`/... in its parent. On rows
//! and columns: `gap` between children, `justify` ("start", "end",
//! "center", "space_between", "space_around", "space_evenly"), `align`
//! and `align_content` ("start", "end", "center", "stretch"), `wrap`.
//! Any box can have a `border`: a role whose stroke outlines it.
//!
//! **Roles** are how a slot's text and strokes look, by name: a slot uses
//! the role named after it (a flow's or art's name) unless it says `role`.
//! `font`, `style` (a stroke style) and `size` (the symbol scale) set text;
//! `line_height` (in sizes, default 1.25), `align` ("left", "center",
//! "right", "justify"), `valign` ("top", "center", "bottom"), `indent` (a
//! paragraph's first line, in sizes), `paragraph_gap` (in lines, default
//! 0.5), `spacing` (between glyphs, in sizes), `jagged` (random line
//! starts, in sizes), `clearance` (from what text wraps around, in units,
//! default size / 4) and `fit` (shrink the text until all of it fits).
//! Art, bands and rules only need a `style`. A slot can override any of
//! these in its own style: `flow("body", #{ grow: 1, align: "center" })`.
//!
//! Text wraps around boxes taken out of the flow (`position: "absolute"`):
//! their `wrap` is "box" (the default), "hull" (the outline of what's
//! drawn) or "none". In text, `"\n"` breaks a line and a blank line starts
//! a paragraph.
//!
//! **Art** is a canvas (its strokes keep their styles), a path or glyph
//! (drawn in the slot's role style), or a function given the box,
//! `#{ x, y, width, height, center }`, that returns a canvas or path
//! drawn as it is, for art made to measure.
//!
//! `page(n, info)` gets `info.flows`, how many characters each flow has
//! left, so a layout can change as the text runs out, and `info.art`, the
//! art's names. Typesetting stops when every flow has run out; a page that
//! places none of what's left is an error, as is running past `max_pages`
//! (default 200). Pages render exactly `width` x `height`, so they line
//! up, and split like any other.

use parry2d::bounding_volume::Aabb;
use parry2d::math::Vec2 as PVec;
use rhai::{Array, Dynamic, Engine, EvalAltResult, FnPtr, Map, NativeCallContext, Scope, AST};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use stretch::geometry::{Rect as SRect, Size};
use stretch::number::Number;
use stretch::style::{AlignContent, AlignItems, AlignSelf, Dimension, FlexDirection, FlexWrap, JustifyContent, PositionType, Style};

use super::glyphs::Glyph;
use super::layout::{check_keys, number};
use super::random::SharedRng;
use super::{check_finite, stable_hash, CanvasData, Canvas, Page, Path, Stroke, StrokeStyle};
use crate::journal::avoid::AvoidanceTree;
use crate::journal::canvas::Frame;
use crate::journal::paths::StrokePath;
use crate::journal::styles::Font;
use crate::journal::symbol::Symbol;
use crate::journal::symboldict::SymbolPathDict;
use crate::journal::typeset::{fitting_size, set_text, Align, TextStyle};

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

/// The repository's layout files.
pub const DEFAULT_LAYOUTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/layouts");

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Row,
    Column,
    Flow(String),
    Label(String),
    Initial(String),
    Figure(String),
    Band(String),
    Rule,
    Spacer,
}

impl Kind {
    fn is_container(&self) -> bool {
        matches!(self, Kind::Row | Kind::Column)
    }
    /// The role a slot uses unless it names one.
    fn default_role(&self) -> &str {
        match self {
            Kind::Flow(name) | Kind::Figure(name) | Kind::Band(name) => name,
            Kind::Label(_) => "label",
            Kind::Initial(_) => "initial",
            Kind::Rule => "rule",
            Kind::Row | Kind::Column | Kind::Spacer => "",
        }
    }
    fn describe(&self) -> String {
        match self {
            Kind::Row => "row".into(),
            Kind::Column => "column".into(),
            Kind::Flow(name) => format!("flow(\"{}\")", name),
            Kind::Label(text) => format!("label(\"{}\")", text),
            Kind::Initial(name) => format!("initial(\"{}\")", name),
            Kind::Figure(name) => format!("figure(\"{}\")", name),
            Kind::Band(name) => format!("band(\"{}\")", name),
            Kind::Rule => "rule".into(),
            Kind::Spacer => "spacer".into(),
        }
    }
}

/// A box in a page layout.
#[derive(Clone)]
pub struct Node {
    kind: Kind,
    style: Style,
    gap: f32,
    /// The role a slot's text or strokes take.
    role: String,
    /// Role settings this slot overrides.
    overrides: Map,
    /// The role that outlines it.
    border: Option<String>,
    wrap: Wrap,
    /// Whether its width and height were left to its content.
    auto_size: (bool, bool),
    /// For bands: how far to turn the art, or `None` to follow the band.
    turn: Option<f32>,
    children: Vec<Node>,
}

/// What text wrapping around a box keeps clear of.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Wrap {
    Box,
    Hull,
    None,
}

const BOX_KEYS: [&str; 20] = [
    "width", "height", "min_width", "min_height", "max_width", "max_height", "grow", "shrink", "basis",
    "margin", "padding", "align_self", "position", "top", "left", "right", "bottom", "aspect", "border", "wrap",
];
const CONTAINER_KEYS: [&str; 4] = ["gap", "justify", "align", "align_content"];
/// Role settings, which roles and slots take.
const ROLE_KEYS: [&str; 13] = [
    "font", "style", "size", "line_height", "align", "valign", "indent", "paragraph_gap", "spacing", "clearance", "jagged", "fit", "role",
];

fn dimension(caller: &str, key: &str, value: &Dynamic) -> RhaiResult<Dimension> {
    if let Ok(n) = value.as_float() {
        return Ok(Dimension::Points(n as f32));
    }
    if let Ok(n) = value.as_int() {
        return Ok(Dimension::Points(n as f32));
    }
    let bad = || -> Box<EvalAltResult> { format!("{}: '{}' should be a number, \"N%\" or \"auto\", got {}", caller, key, value).into() };
    let text = value.clone().into_immutable_string().map_err(|_| bad())?;
    if text == "auto" {
        return Ok(Dimension::Auto);
    }
    let percent: f32 = text.strip_suffix('%').and_then(|n| n.trim().parse().ok()).ok_or_else(bad)?;
    Ok(Dimension::Percent(percent / 100.0))
}

fn edges(caller: &str, key: &str, value: &Dynamic) -> RhaiResult<SRect<Dimension>> {
    let rect = |top, right, bottom, left| SRect { top, end: right, bottom, start: left };
    if let Some(map) = value.clone().try_cast::<Map>() {
        check_keys(&format!("{} '{}'", caller, key), &map, &["top", "right", "bottom", "left"])?;
        let side = |name: &str| map.get(name).map_or(Ok(Dimension::Undefined), |v| dimension(caller, key, v));
        return Ok(rect(side("top")?, side("right")?, side("bottom")?, side("left")?));
    }
    if let Some(list) = value.clone().try_cast::<Array>() {
        let d: Vec<Dimension> = list.iter().map(|v| dimension(caller, key, v)).collect::<RhaiResult<_>>()?;
        return match d.as_slice() {
            [all] => Ok(rect(*all, *all, *all, *all)),
            [v, h] => Ok(rect(*v, *h, *v, *h)),
            [t, h, b] => Ok(rect(*t, *h, *b, *h)),
            [t, r, b, l] => Ok(rect(*t, *r, *b, *l)),
            _ => Err(format!("{}: '{}' takes 1 to 4 values, got {}", caller, key, d.len()).into()),
        };
    }
    let all = dimension(caller, key, value)?;
    Ok(rect(all, all, all, all))
}

fn string(caller: &str, key: &str, value: &Dynamic) -> RhaiResult<String> {
    value.clone().into_immutable_string().map(|s| s.to_string())
        .map_err(|t| format!("{}: '{}' should be a string, got {}", caller, key, t).into())
}

fn choice<T: Copy>(caller: &str, key: &str, value: &Dynamic, choices: &[(&str, T)]) -> RhaiResult<T> {
    let name = string(caller, key, value)?;
    choices.iter().find(|(n, _)| *n == name).map(|(_, v)| *v).ok_or_else(|| {
        let names: Vec<&str> = choices.iter().map(|(n, _)| *n).collect();
        format!("{}: unknown {} '{}'; expected one of {}", caller, key, name, names.join(", ")).into()
    })
}

/// Check a role setting's value, as roles and slots give them.
fn check_role_setting(caller: &str, key: &str, value: &Dynamic) -> RhaiResult<()> {
    let positive = |what: &str| -> RhaiResult<()> {
        match number(caller, &Map::from_iter([(key.into(), value.clone())]), key)? {
            Some(n) if n > 0.0 && n.is_finite() => Ok(()),
            _ => Err(format!("{}: '{}' must be {}, got {}", caller, key, what, value).into()),
        }
    };
    match key {
        "font" if !value.is::<Font>() => Err(format!("{}: 'font' should be a font, got {}", caller, value.type_name()).into()),
        "style" if !value.is::<StrokeStyle>() => Err(format!("{}: 'style' should be a stroke style, got {}", caller, value.type_name()).into()),
        "size" | "line_height" => positive("a positive number"),
        "indent" | "paragraph_gap" | "spacing" | "clearance" | "jagged" => {
            match number(caller, &Map::from_iter([(key.into(), value.clone())]), key)? {
                Some(n) if n.is_finite() && (n >= 0.0 || key == "spacing") => Ok(()),
                _ => Err(format!("{}: '{}' must be a number of at least 0, got {}", caller, key, value).into()),
            }
        }
        "align" => choice(caller, key, value, &[("left", ()), ("center", ()), ("right", ()), ("justify", ())]),
        "valign" => choice(caller, key, value, &[("top", ()), ("center", ()), ("bottom", ())]),
        "fit" if !value.is_bool() => Err(format!("{}: 'fit' should be true or false, got {}", caller, value).into()),
        "role" => string(caller, key, value).map(|_| ()),
        _ => Ok(()),
    }
}

impl Node {
    fn new(caller: &str, kind: Kind, options: Map, children: Array) -> RhaiResult<Node> {
        let container = kind.is_container();
        let mut known: Vec<&str> = BOX_KEYS.to_vec();
        if container {
            known.extend(CONTAINER_KEYS);
        } else {
            known.extend(ROLE_KEYS);
            if matches!(kind, Kind::Band(_)) {
                known.push("turn");
            }
        }
        check_keys(caller, &options, &known)?;
        let mut style = Style {
            flex_direction: if kind == Kind::Row { FlexDirection::Row } else { FlexDirection::Column },
            ..Style::default()
        };
        let mut overrides = Map::new();
        let (mut role, mut border, mut wrap, mut gap, mut turn) = (kind.default_role().to_string(), None, None, 0.0, None);
        let mut basis_given = false;
        for (key, value) in &options {
            let key = key.as_str();
            let dim = || dimension(caller, key, value);
            let num = || -> RhaiResult<f32> {
                number(caller, &options, key)?.filter(|n| n.is_finite())
                    .ok_or_else(|| format!("{}: '{}' should be a number, got {}", caller, key, value).into())
            };
            match key {
                "width" => style.size.width = dim()?,
                "height" => style.size.height = dim()?,
                "min_width" => style.min_size.width = dim()?,
                "min_height" => style.min_size.height = dim()?,
                "max_width" => style.max_size.width = dim()?,
                "max_height" => style.max_size.height = dim()?,
                "basis" => {
                    style.flex_basis = dim()?;
                    basis_given = true;
                }
                "top" => style.position.top = dim()?,
                "left" => style.position.start = dim()?,
                "right" => style.position.end = dim()?,
                "bottom" => style.position.bottom = dim()?,
                "margin" => style.margin = edges(caller, key, value)?,
                "padding" => style.padding = edges(caller, key, value)?,
                "grow" | "shrink" | "aspect" | "gap" => {
                    let n = num()?;
                    if n < 0.0 {
                        return Err(format!("{}: '{}' can't be negative, got {}", caller, key, n).into());
                    }
                    match key {
                        "grow" => style.flex_grow = n,
                        "shrink" => style.flex_shrink = n,
                        "aspect" => style.aspect_ratio = Number::Defined(n),
                        _ => gap = n,
                    }
                }
                "position" => style.position_type = choice(caller, key, value, &[("relative", PositionType::Relative), ("absolute", PositionType::Absolute)])?,
                "align_self" => style.align_self = choice(caller, key, value, &[
                    ("auto", AlignSelf::Auto), ("start", AlignSelf::FlexStart), ("end", AlignSelf::FlexEnd),
                    ("center", AlignSelf::Center), ("stretch", AlignSelf::Stretch),
                ])?,
                "justify" => style.justify_content = choice(caller, key, value, &[
                    ("start", JustifyContent::FlexStart), ("end", JustifyContent::FlexEnd), ("center", JustifyContent::Center),
                    ("space_between", JustifyContent::SpaceBetween), ("space_around", JustifyContent::SpaceAround),
                    ("space_evenly", JustifyContent::SpaceEvenly),
                ])?,
                "align" if container => style.align_items = choice(caller, key, value, &[
                    ("start", AlignItems::FlexStart), ("end", AlignItems::FlexEnd), ("center", AlignItems::Center), ("stretch", AlignItems::Stretch),
                ])?,
                "align_content" => style.align_content = choice(caller, key, value, &[
                    ("start", AlignContent::FlexStart), ("end", AlignContent::FlexEnd), ("center", AlignContent::Center),
                    ("stretch", AlignContent::Stretch), ("space_between", AlignContent::SpaceBetween), ("space_around", AlignContent::SpaceAround),
                ])?,
                "wrap" if container => {
                    style.flex_wrap = if value.as_bool().map_err(|_| format!("{}: 'wrap' should be true or false, got {}", caller, value))? {
                        FlexWrap::Wrap
                    } else {
                        FlexWrap::NoWrap
                    }
                }
                "wrap" => wrap = Some(choice(caller, key, value, &[("box", Wrap::Box), ("hull", Wrap::Hull), ("none", Wrap::None)])?),
                "border" => border = Some(string(caller, key, value)?),
                "turn" => turn = Some(num()?),
                "role" => role = string(caller, key, value)?,
                _ => {
                    check_role_setting(caller, key, value)?;
                    overrides.insert(key.into(), value.clone());
                }
            }
        }
        // A box that grows from no size of its own grows from nothing, as
        // CSS's `flex: 1` does, so growing boxes share space evenly.
        if style.flex_grow > 0.0 && !basis_given && !options.contains_key("width") && !options.contains_key("height") {
            style.flex_basis = Dimension::Points(0.0);
        }
        let children = children.into_iter().enumerate().map(|(i, child)| {
            let type_name = child.type_name();
            child.try_cast::<Node>().ok_or_else(|| -> Box<EvalAltResult> {
                format!("{}: child {} is a {}, not a layout box (row, column, flow, ...)", caller, i, type_name).into()
            })
        }).collect::<RhaiResult<Vec<Node>>>()?;
        let absolute = style.position_type == PositionType::Absolute;
        Ok(Node {
            auto_size: (!options.contains_key("width"), !options.contains_key("height")),
            wrap: wrap.unwrap_or(if absolute { Wrap::Box } else { Wrap::None }),
            kind, style, gap, role, overrides, border, turn, children,
        })
    }

    /// Every box with its children, depth first.
    fn walk<'a>(&'a self, out: &mut Vec<&'a Node>) {
        out.push(self);
        for child in &self.children {
            child.walk(out);
        }
    }
}

/// A layout: where its pages come from.
#[derive(Clone)]
pub struct Layout {
    name: String,
    source: Source,
}

#[derive(Clone)]
enum Source {
    File { ast: Rc<AST>, file: String },
    Function(FnPtr),
}

impl Layout {
    /// Page `n`'s tree.
    fn page(&self, ctx: &NativeCallContext, n: usize, info: Map) -> RhaiResult<Node> {
        let args = (n as i64, Dynamic::from_map(info));
        let (tree, from) = match &self.source {
            Source::File { ast, file } => (ctx.engine().call_fn::<Dynamic>(&mut Scope::new(), ast, "page", args), file.clone()),
            Source::Function(f) => (f.call_within_context::<Dynamic>(ctx, args), format!("{}()", f.fn_name())),
        };
        let tree = tree.map_err(|e| format!("layout '{}' page {}: {}", self.name, n, e))?;
        let type_name = tree.type_name();
        tree.try_cast::<Node>().ok_or_else(|| format!("layout '{}' page {}: {} returned a {}, not a layout box", self.name, n, from, type_name).into())
    }
    /// The page size its file gives, if it does.
    fn page_size(&self, ctx: &NativeCallContext) -> RhaiResult<Option<Map>> {
        let Source::File { ast, .. } = &self.source else { return Ok(None) };
        if !ast.iter_functions().any(|f| f.name == "page_size" && f.params.is_empty()) {
            return Ok(None);
        }
        let size = ctx.engine().call_fn::<Dynamic>(&mut Scope::new(), ast, "page_size", ())
            .map_err(|e| format!("layout '{}' page_size(): {}", self.name, e))?;
        let type_name = size.type_name();
        size.try_cast::<Map>().map(Some)
            .ok_or_else(|| format!("layout '{}': page_size() should return #{{ width, height }}, got {}", self.name, type_name).into())
    }
}

fn load_layout(ctx: &NativeCallContext, dir: &std::path::Path, name: &str) -> RhaiResult<Layout> {
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(format!("layout: '{}' isn't a layout name; layouts are files in {}", name, dir.display()).into());
    }
    let path = dir.join(format!("{}.rhai", name));
    if !path.is_file() {
        let mut names: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten()
            .filter_map(|e| e.path().file_stem().filter(|_| e.path().extension().is_some_and(|x| x == "rhai")).map(|s| s.to_string_lossy().into_owned()))
            .collect();
        names.sort();
        return Err(format!("layout: no layout file {}; layouts: {}", path.display(), names.join(", ")).into());
    }
    let file = path.display().to_string();
    let ast = ctx.engine().compile_file(path).map_err(|e| format!("layout '{}': {}", name, e))?;
    if !ast.iter_functions().any(|f| f.name == "page" && f.params.len() == 2) {
        return Err(format!("layout '{}': {} doesn't define page(n, info)", name, file).into());
    }
    Ok(Layout { name: name.to_string(), source: Source::File { ast: Rc::new(ast), file } })
}

/// How a slot's text and strokes look.
struct Role {
    font: Option<Font>,
    style: Option<StrokeStyle>,
    text: Option<TextStyle>,
    fit: bool,
    valign: f32,
}

/// A flow of text, as far as it's been set.
struct FlowState {
    text: String,
    chars: Vec<char>,
    cursor: usize,
    paragraph_start: bool,
    /// The whole text's glyphs in each role that's set it, made when first
    /// needed (glyph variants are picked at random).
    glyphs: HashMap<String, Vec<Symbol>>,
}

impl FlowState {
    fn remaining(&self) -> usize {
        self.chars.len() - self.cursor
    }
}

/// Everything `typeset` was given, checked.
struct Content {
    width: f32,
    height: f32,
    name: String,
    flows: Vec<(String, FlowState)>,
    roles: Map,
    art: Map,
    max_pages: usize,
}

const CONTENT_KEYS: [&str; 7] = ["name", "width", "height", "flows", "roles", "art", "max_pages"];

impl Content {
    fn flow(&mut self, name: &str) -> RhaiResult<&mut FlowState> {
        match self.flows.iter().position(|(n, _)| n == name) {
            Some(i) => Ok(&mut self.flows[i].1),
            None => {
                let names: Vec<&str> = self.flows.iter().map(|(n, _)| n.as_str()).collect();
                Err(format!("typeset: the layout sets a flow '{}', but there's no such flow; flows: {}", name, names.join(", ")).into())
            }
        }
    }

    /// A slot's role: the content's role of its name, with the slot's
    /// overrides on top.
    fn role(&self, node: &Node, page: usize) -> RhaiResult<Role> {
        let caller = format!("typeset: page {}'s {}", page, node.kind.describe());
        let mut settings = match self.roles.get(node.role.as_str()) {
            Some(role) => role.clone().try_cast::<Map>()
                .ok_or_else(|| format!("typeset: role '{}' should be a map of settings", node.role))?,
            None => Map::new(),
        };
        if !self.roles.contains_key(node.role.as_str()) && node.overrides.is_empty() && needs_role(&node.kind) {
            let mut names: Vec<&str> = self.roles.keys().map(|k| k.as_str()).collect();
            names.sort();
            return Err(format!("{} uses role '{}', which `roles` doesn't have; roles: {}", caller, node.role, names.join(", ")).into());
        }
        for (key, value) in &node.overrides {
            settings.insert(key.clone(), value.clone());
        }
        let font = settings.get("font").and_then(|f| f.clone().try_cast::<Font>());
        let style = settings.get("style").and_then(|s| s.clone().try_cast::<StrokeStyle>());
        let size = number(&caller, &settings, "size")?;
        let text = match size {
            None => None,
            Some(size) => {
                let mut text = TextStyle::new(size);
                let get = |key: &str| number(&caller, &settings, key);
                text.line_height = get("line_height")?.unwrap_or(text.line_height);
                text.indent = get("indent")?.unwrap_or(0.0);
                text.paragraph_gap = get("paragraph_gap")?.unwrap_or(text.paragraph_gap);
                text.spacing = get("spacing")?.unwrap_or(0.0);
                text.clearance = get("clearance")?.unwrap_or(text.clearance);
                text.jagged = get("jagged")?.unwrap_or(0.0);
                if let Some(align) = settings.get("align") {
                    text.align = Align::from_name(&align.to_string()).expect("checked when given");
                }
                Some(text)
            }
        };
        let valign = match settings.get("valign").map(|v| v.to_string()).as_deref() {
            Some("center") => 0.5,
            Some("bottom") => 1.0,
            _ => 0.0,
        };
        Ok(Role { font, style, text, fit: settings.get("fit").and_then(|f| f.as_bool().ok()).unwrap_or(false), valign })
    }
}

fn needs_role(kind: &Kind) -> bool {
    matches!(kind, Kind::Flow(_) | Kind::Label(_) | Kind::Initial(_) | Kind::Rule)
}

fn parse_content(caller: &str, content: &Map, layout: &Layout, ctx: &NativeCallContext) -> RhaiResult<Content> {
    check_keys(caller, content, &CONTENT_KEYS)?;
    let size = layout.page_size(ctx)?.unwrap_or_default();
    let dimension = |key: &str| -> RhaiResult<f32> {
        let value = number(caller, content, key)?.or(number(caller, &size, key)?)
            .ok_or_else(|| format!("{}: give the page's '{}' (the layout doesn't)", caller, key))?;
        if !(value > 0.0 && value.is_finite()) {
            return Err(format!("{}: '{}' must be positive, got {}", caller, key, value).into());
        }
        Ok(value)
    };
    let map = |key: &str| -> RhaiResult<Map> {
        match content.get(key) {
            None => Ok(Map::new()),
            Some(v) => v.clone().try_cast::<Map>().ok_or_else(|| format!("{}: '{}' should be a map", caller, key).into()),
        }
    };
    let mut flows = Vec::new();
    let mut names: Vec<(String, Dynamic)> = map("flows")?.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, text) in names {
        let text = text.into_immutable_string().map_err(|t| format!("{}: flow '{}' should be a string, got {}", caller, name, t))?.to_string();
        flows.push((name, FlowState { chars: text.chars().collect(), text, cursor: 0, paragraph_start: true, glyphs: HashMap::new() }));
    }
    let roles = map("roles")?;
    for (name, role) in &roles {
        let settings = role.clone().try_cast::<Map>().ok_or_else(|| format!("{}: role '{}' should be a map of settings", caller, name))?;
        let role_caller = format!("{} role '{}'", caller, name);
        check_keys(&role_caller, &settings, &ROLE_KEYS[..ROLE_KEYS.len() - 1])?;
        for (key, value) in &settings {
            check_role_setting(&role_caller, key, value)?;
        }
    }
    let max_pages = match content.get("max_pages") {
        None => 200,
        Some(n) => n.as_int().ok().filter(|n| *n >= 1).map(|n| n as usize)
            .ok_or_else(|| format!("{}: 'max_pages' should be a whole number of at least 1", caller))?,
    };
    let name = match content.get("name") {
        None => layout.name.clone(),
        Some(name) => super::check_name(caller, name.clone())?,
    };
    Ok(Content { width: dimension("width")?, height: dimension("height")?, name, flows, roles, art: map("art")?, max_pages })
}

/// A slot's place on the page: its box, and inside its padding.
#[derive(Clone, Copy)]
struct Placed {
    outer: Aabb,
    inner: Aabb,
}

fn rect_map(rect: Aabb) -> Map {
    let mut map = Map::new();
    map.insert("x".into(), Dynamic::from(rect.mins.x as f64));
    map.insert("y".into(), Dynamic::from(rect.mins.y as f64));
    map.insert("width".into(), Dynamic::from(rect.extents().x as f64));
    map.insert("height".into(), Dynamic::from(rect.extents().y as f64));
    let c = rect.center();
    map.insert("center".into(), Dynamic::from(super::Vec2::new(c.x as f64, c.y as f64)));
    map
}

/// Lay out a page's tree with stretch at `size`, giving each box its place.
/// `sizes` overrides boxes' widths and heights (from measuring their text).
fn compute(root: &Node, size: (f32, f32), sizes: &HashMap<usize, (Option<f32>, Option<f32>)>) -> RhaiResult<Vec<Placed>> {
    let mut stretch = stretch::Stretch::new();
    let nodes = {
        let mut all = Vec::new();
        root.walk(&mut all);
        all
    };
    // Build bottom up, children before parents; ids in walk order.
    fn build(stretch: &mut stretch::Stretch, node: &Node, index: &mut usize, page: (f32, f32), sizes: &HashMap<usize, (Option<f32>, Option<f32>)>, ids: &mut Vec<Option<stretch::node::Node>>) -> Result<stretch::node::Node, String> {
        let me = *index;
        *index += 1;
        ids.push(None);
        let mut children = Vec::new();
        for (i, child) in node.children.iter().enumerate() {
            let built = build(stretch, child, index, page, sizes, ids)?;
            // The container's gap, as margins between children.
            if i > 0 && node.gap > 0.0 {
                let mut style = *stretch.style(built).map_err(|e| format!("{:?}", e))?;
                let side = if node.kind == Kind::Row { &mut style.margin.start } else { &mut style.margin.top };
                *side = match *side {
                    Dimension::Points(m) => Dimension::Points(m + node.gap),
                    Dimension::Undefined => Dimension::Points(node.gap),
                    other => other,
                };
                stretch.set_style(built, style).map_err(|e| format!("{:?}", e))?;
            }
            children.push(built);
        }
        let mut style = node.style;
        if me == 0 {
            // The root is the page.
            style.size = Size { width: Dimension::Points(page.0), height: Dimension::Points(page.1) };
        }
        if let Some((w, h)) = sizes.get(&me) {
            if let Some(w) = w { style.size.width = Dimension::Points(*w); }
            if let Some(h) = h { style.size.height = Dimension::Points(*h); }
        }
        let id = stretch.new_node(style, children).map_err(|e| format!("{:?}", e))?;
        ids[me] = Some(id);
        Ok(id)
    }
    let mut ids = Vec::new();
    let root_id = build(&mut stretch, root, &mut 0, size, sizes, &mut ids).map_err(|e| format!("typeset: stretch: {}", e))?;
    stretch.compute_layout(root_id, Size { width: Number::Defined(size.0), height: Number::Defined(size.1) })
        .map_err(|e| format!("typeset: stretch couldn't lay the page out: {:?}", e))?;
    // Absolute places: each box's location is from its parent's corner.
    let mut placed = vec![Placed { outer: Aabb::new_invalid(), inner: Aabb::new_invalid() }; nodes.len()];
    fn place(stretch: &stretch::Stretch, node: &Node, index: &mut usize, origin: PVec, ids: &[Option<stretch::node::Node>], placed: &mut [Placed]) {
        let me = *index;
        *index += 1;
        let layout = stretch.layout(ids[me].expect("built")).expect("laid out");
        let min = origin + PVec::new(layout.location.x, layout.location.y);
        let outer = Aabb::new(min, min + PVec::new(layout.size.width, layout.size.height));
        let resolve = |d: Dimension| match d {
            Dimension::Points(p) => p,
            Dimension::Percent(f) => f * layout.size.width,
            _ => 0.0,
        };
        let p = node.style.padding;
        let inner = Aabb::new(
            outer.mins + PVec::new(resolve(p.start), resolve(p.top)),
            (outer.maxs - PVec::new(resolve(p.end), resolve(p.bottom))).max(outer.mins + PVec::new(resolve(p.start), resolve(p.top))),
        );
        placed[me] = Placed { outer, inner };
        for child in &node.children {
            place(stretch, child, index, min, ids, placed);
        }
    }
    place(&stretch, root, &mut 0, PVec::ZERO, &ids, &mut placed);
    Ok(placed)
}

/// Text a slot sets: its glyphs, characters, and whether it starts a
/// paragraph.
fn slot_text<'a>(content: &'a mut Content, node: &Node, role: &Role, labels: &'a HashMap<usize, (Vec<Symbol>, Vec<char>)>, index: usize, symdict: &SymbolPathDict, rng: &SharedRng) -> RhaiResult<(&'a [Symbol], &'a [char], bool)> {
    match &node.kind {
        Kind::Flow(name) | Kind::Initial(name) => {
            let font = role.font.clone().ok_or_else(|| format!("typeset: {} needs a font in its role '{}'", node.kind.describe(), node.role))?;
            let flow = content.flow(name)?;
            if !flow.glyphs.contains_key(&node.role) {
                let glyphs = font.builder(symdict).build(&flow.text, &mut *rng.borrow_mut());
                flow.glyphs.insert(node.role.clone(), glyphs);
            }
            let start = flow.cursor;
            let glyphs = &flow.glyphs[&node.role];
            Ok((&glyphs[start..], &flow.chars[start..], flow.paragraph_start))
        }
        Kind::Label(_) => {
            let (glyphs, chars) = &labels[&index];
            Ok((glyphs, chars, true))
        }
        _ => Ok((&[], &[], false)),
    }
}

/// Fit `path` into `rect`, keeping its proportions, centred.
fn fitted(path: &StrokePath, from: Aabb, rect: Aabb) -> StrokePath {
    let size = from.extents().max(PVec::splat(1e-3));
    let scale = (rect.extents().x / size.x).min(rect.extents().y / size.y);
    path.clone().translated_by(-from.center()).scaled_by(scale).translated_by(rect.center())
}

/// Strokes of art: a canvas's own, or a path or glyph in `style`.
fn art_strokes(caller: &str, art: &Dynamic, style: Option<&StrokeStyle>) -> RhaiResult<Vec<Stroke>> {
    let styled = |path: StrokePath| -> RhaiResult<Vec<Stroke>> {
        let style = style.ok_or_else(|| format!("{}: the art is a path, so its role needs a 'style'", caller))?;
        Ok(vec![Stroke { path, style: style.clone(), seed: None }])
    };
    if let Some(canvas) = art.clone().try_cast::<Canvas>() {
        Ok(canvas.borrow().strokes.clone())
    } else if let Some(path) = art.clone().try_cast::<Path>() {
        styled(path.get())
    } else if let Some(glyph) = art.clone().try_cast::<Glyph>() {
        styled(glyph.borrow().placed_strokepath())
    } else {
        Err(format!("{}: art should be a canvas, path, glyph or function, got {}", caller, art.type_name()).into())
    }
}

/// The art's lines' bounds (not their widths, which don't scale).
fn bounds_of(strokes: &[Stroke]) -> Aabb {
    let mut bounds = Aabb::new_invalid();
    for (points, _) in strokes.iter().flat_map(|s| s.path.flattened(0.5)) {
        for p in points {
            bounds.take_point(p);
        }
    }
    bounds
}

/// Lay out and fill one page. Returns the page's canvas and whether any
/// flow moved on.
#[allow(clippy::too_many_arguments)]
fn typeset_page(ctx: &NativeCallContext, root: &Node, n: usize, content: &mut Content, symdict: &SymbolPathDict, rng: &SharedRng) -> RhaiResult<(CanvasData, bool)> {
    let mut nodes = Vec::new();
    root.walk(&mut nodes);
    let roles: Vec<Option<Role>> = nodes.iter().map(|node| {
        if node.kind.is_container() || node.kind == Kind::Spacer { Ok(None) } else { content.role(node, n).map(Some) }
    }).collect::<RhaiResult<_>>()?;
    // Labels' glyphs, made once.
    let mut labels = HashMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if let Kind::Label(text) = &node.kind {
            let font = roles[i].as_ref().and_then(|r| r.font.clone())
                .ok_or_else(|| format!("typeset: page {}'s {} needs a font in its role '{}'", n, node.kind.describe(), node.role))?;
            labels.insert(i, (font.builder(symdict).build(text, &mut *rng.borrow_mut()), text.chars().collect::<Vec<char>>()));
        }
    }
    // Lay out; then size boxes left to their text by it, and again.
    let page = (content.width, content.height);
    let mut sizes: HashMap<usize, (Option<f32>, Option<f32>)> = HashMap::new();
    let mut placed = compute(root, page, &sizes)?;
    for _ in 0..2 {
        let mut changed = false;
        for (i, node) in nodes.iter().enumerate() {
            if !matches!(node.kind, Kind::Flow(_) | Kind::Label(_)) || !(node.auto_size.0 || node.auto_size.1) || node.style.flex_grow > 0.0 {
                continue;
            }
            let Some(text_style) = roles[i].as_ref().and_then(|r| r.text.clone()) else { continue };
            let role = roles[i].as_ref().expect("text slots have roles");
            let (glyphs, chars, paragraph_start) = slot_text(content, node, role, &labels, i, symdict, rng)?;
            let pad = placed[i].outer.extents() - placed[i].inner.extents();
            let mut width = placed[i].inner.extents().x;
            let mut rng0 = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(0);
            if node.auto_size.0 && width < text_style.size {
                // As wide as its text on one line, up to the page.
                let line = Aabb::new(PVec::ZERO, PVec::new(page.0 * 4.0, text_style.size * text_style.line_height));
                width = set_text(line, &text_style, glyphs, chars, paragraph_start, None, &mut rng0).width.min(page.0 - pad.x).max(text_style.size);
            }
            let tall = Aabb::new(PVec::ZERO, PVec::new(width, page.1 * 4.0));
            let height = set_text(tall, &text_style, glyphs, chars, paragraph_start, None, &mut rng0).height;
            // Rounded up: stretch rounds boxes to whole units.
            let want = (
                node.auto_size.0.then_some((width + pad.x).ceil() + 1.0),
                node.auto_size.1.then_some((height + pad.y).ceil() + 1.0),
            );
            if sizes.get(&i) != Some(&want) {
                sizes.insert(i, want);
                changed = true;
            }
        }
        if !changed {
            break;
        }
        placed = compute(root, page, &sizes)?;
    }

    let mut canvas = CanvasData::default();
    let mut avoid = AvoidanceTree::new();
    let mut moved = false;
    let draw = |canvas: &mut CanvasData, path: StrokePath, style: &StrokeStyle| -> RhaiResult<()> {
        check_finite("typeset", &path)?;
        canvas.add_strokepath(path, style.clone());
        Ok(())
    };
    let keep_clear = |avoid: &mut AvoidanceTree, node: &Node, rect: Aabb, drawn: &[PVec]| match node.wrap {
        Wrap::Box => avoid.add_polygon(&[rect.mins, PVec::new(rect.maxs.x, rect.mins.y), rect.maxs, PVec::new(rect.mins.x, rect.maxs.y)], 0.0),
        Wrap::Hull => avoid.add_hull(drawn, 0.0),
        Wrap::None => {}
    };
    // Everything but flowing text first, so text can wrap around it.
    for (i, node) in nodes.iter().enumerate() {
        let Placed { outer, inner } = placed[i];
        let caller = format!("typeset: page {}'s {}", n, node.kind.describe());
        if let Some(border) = &node.border {
            let style = content.roles.get(border.as_str()).and_then(|r| r.clone().try_cast::<Map>())
                .and_then(|r| r.get("style").and_then(|s| s.clone().try_cast::<StrokeStyle>()))
                .ok_or_else(|| format!("{}: its border's role '{}' needs a 'style'", caller, border))?;
            draw(&mut canvas, StrokePath::rectangle(outer.center(), outer.extents().x, outer.extents().y), &style)?;
        }
        let role = roles[i].as_ref();
        match &node.kind {
            Kind::Rule => {
                let style = role.and_then(|r| r.style.as_ref()).ok_or_else(|| format!("{}: its role '{}' needs a 'style'", caller, node.role))?;
                let (c, e) = (inner.center(), inner.extents());
                let line = if e.x >= e.y {
                    StrokePath::line(PVec::new(inner.mins.x, c.y), PVec::new(inner.maxs.x, c.y))
                } else {
                    StrokePath::line(PVec::new(c.x, inner.mins.y), PVec::new(c.x, inner.maxs.y))
                };
                keep_clear(&mut avoid, node, outer, &line.points_within());
                draw(&mut canvas, line, style)?;
            }
            Kind::Figure(name) | Kind::Band(name) => {
                let art = content.art.get(name.as_str()).cloned().ok_or_else(|| {
                    let mut names: Vec<&str> = content.art.keys().map(|k| k.as_str()).collect();
                    names.sort();
                    format!("{}: there's no art named '{}'; art: {}", caller, name, names.join(", "))
                })?;
                let style = role.and_then(|r| r.style.as_ref());
                let strokes = if let Some(f) = art.clone().try_cast::<FnPtr>() {
                    // Made to measure: drawn as it comes.
                    let made = f.call_within_context::<Dynamic>(ctx, (Dynamic::from_map(rect_map(inner)),))
                        .map_err(|e| format!("{}: {}", caller, e))?;
                    art_strokes(&caller, &made, style)?
                } else {
                    let strokes = art_strokes(&caller, &art, style)?;
                    let from = bounds_of(&strokes);
                    if from.mins.x > from.maxs.x {
                        Vec::new()
                    } else if let Kind::Band(_) = node.kind {
                        band(&strokes, from, inner, node.turn)
                    } else {
                        strokes.into_iter().map(|s| Stroke { path: fitted(&s.path, from, inner), ..s }).collect()
                    }
                };
                let points: Vec<PVec> = strokes.iter().flat_map(|s| s.path.points_within()).collect();
                keep_clear(&mut avoid, node, outer, &points);
                for stroke in strokes {
                    draw(&mut canvas, stroke.path, &stroke.style)?;
                }
            }
            Kind::Initial(_) | Kind::Label(_) => {
                let role = role.expect("text slots have roles");
                let style = role.style.clone().ok_or_else(|| format!("{}: its role '{}' needs a 'style'", caller, node.role))?;
                let text_style = role.text.clone();
                let (glyphs, chars, paragraph_start) = slot_text(content, node, role, &labels, i, symdict, rng)?;
                let (set, consumed) = if let Kind::Initial(_) = node.kind {
                    // The next letter, filling the box.
                    let Some(k) = glyphs.iter().zip(chars).position(|(g, c)| !g.is_blank() && *c != '\n') else { continue };
                    let mut glyph = glyphs[k].clone();
                    let hull = glyph.hull_aabb();
                    let scale = (inner.extents().x * 2.0 / hull.extents().x).min(inner.extents().y * 2.0 / hull.extents().y);
                    glyph.scale_by(scale);
                    glyph.set_position(inner.center() - hull.center() * glyph.scale() / 2.0);
                    (vec![glyph], k + 1)
                } else {
                    let text_style = text_style.ok_or_else(|| format!("{}: its role '{}' needs a 'size'", caller, node.role))?;
                    let setting = set_slot(inner, &text_style, role, glyphs, chars, paragraph_start, &avoid, rng);
                    (setting.placed, setting.consumed)
                };
                let points: Vec<PVec> = set.iter().flat_map(|g| g.footprint()).collect();
                keep_clear(&mut avoid, node, outer, &points);
                for glyph in &set {
                    draw(&mut canvas, glyph.placed_strokepath(), &style)?;
                }
                if let Kind::Initial(name) = &node.kind {
                    let flow = content.flow(name)?;
                    flow.cursor += consumed;
                    flow.paragraph_start = false;
                    moved = true;
                }
            }
            _ => {}
        }
    }
    // Then the flows, in order.
    for (i, node) in nodes.iter().enumerate() {
        let Kind::Flow(name) = &node.kind else { continue };
        let caller = format!("typeset: page {}'s {}", n, node.kind.describe());
        let role = roles[i].as_ref().expect("text slots have roles");
        let style = role.style.clone().ok_or_else(|| format!("{}: its role '{}' needs a 'style'", caller, node.role))?;
        let text_style = role.text.clone().ok_or_else(|| format!("{}: its role '{}' needs a 'size'", caller, node.role))?;
        let (glyphs, chars, paragraph_start) = slot_text(content, node, role, &labels, i, symdict, rng)?;
        if glyphs.is_empty() {
            continue;
        }
        let setting = set_slot(placed[i].inner, &text_style, role, glyphs, chars, paragraph_start, &avoid, rng);
        for glyph in &setting.placed {
            draw(&mut canvas, glyph.placed_strokepath(), &style)?;
        }
        let flow = content.flow(name)?;
        flow.cursor += setting.consumed;
        flow.paragraph_start = setting.paragraph_start;
        moved |= setting.consumed > 0;
    }
    Ok((canvas, moved))
}

/// Set text in a slot: fitted if its role says, and placed down the box as
/// its role says.
#[allow(clippy::too_many_arguments)]
fn set_slot(rect: Aabb, style: &TextStyle, role: &Role, glyphs: &[Symbol], chars: &[char], paragraph_start: bool, avoid: &AvoidanceTree, rng: &SharedRng) -> crate::journal::typeset::Setting {
    let mut style = style.clone();
    if role.fit
        && let Some(size) = fitting_size(rect, &style, glyphs, chars, paragraph_start, Some(avoid)) {
        style.clearance *= size / style.size;
        style.size = size;
    }
    let mut setting = set_text(rect, &style, glyphs, chars, paragraph_start, Some(avoid), &mut *rng.borrow_mut());
    let slack = (rect.extents().y - setting.height).max(0.0) * role.valign;
    if slack > 0.0 {
        for glyph in &mut setting.placed {
            glyph.translate_by(PVec::new(0.0, slack));
        }
    }
    setting
}

/// The most times a band repeats its art.
const MAX_BAND_REPEATS: usize = 2000;

/// Art repeated along a band: scaled to the band's breadth and spaced
/// evenly along it, turned to run along it (or by `turn`).
fn band(strokes: &[Stroke], from: Aabb, rect: Aabb, turn: Option<f32>) -> Vec<Stroke> {
    let (length, breadth) = (rect.extents().x.max(rect.extents().y), rect.extents().x.min(rect.extents().y));
    let along_x = rect.extents().x >= rect.extents().y;
    let art = from.extents().max(PVec::splat(1e-3));
    // Turned a quarter when the art runs the other way from the band.
    let turn = turn.unwrap_or(if (art.x >= art.y) == along_x { 0.0 } else { std::f32::consts::FRAC_PI_2 });
    let quarter = (turn / std::f32::consts::FRAC_PI_2).round() as i32 % 2 != 0;
    let (art_along, art_across) = if quarter == along_x { (art.y, art.x) } else { (art.x, art.y) };
    if !(breadth > 0.0 && length > 0.0) {
        return Vec::new();
    }
    let scale = breadth / art_across;
    let tile = (art_along * scale).max(length / MAX_BAND_REPEATS as f32);
    let count = ((length / tile).floor() as usize).clamp(1, MAX_BAND_REPEATS);
    let step = length / count as f32;
    let mut out = Vec::new();
    for k in 0..count {
        let along = -length / 2.0 + step * (k as f32 + 0.5);
        let at = rect.center() + if along_x { PVec::new(along, 0.0) } else { PVec::new(0.0, along) };
        for s in strokes {
            let path = s.path.clone().translated_by(-from.center()).rotated_by(turn).scaled_by(scale).translated_by(at);
            out.push(Stroke { path, ..s.clone() });
        }
    }
    out
}

fn typeset(ctx: NativeCallContext, layout: &mut Layout, content: Map, symdict: &SymbolPathDict, rng: &SharedRng) -> RhaiResult<Array> {
    let caller = "typeset";
    let mut content = parse_content(caller, &content, layout, &ctx)?;
    let mut pages = Array::new();
    for n in 0.. {
        if content.flows.iter().all(|(_, f)| f.remaining() == 0) && n > 0 {
            break;
        }
        if n >= content.max_pages {
            let left: Vec<String> = content.flows.iter().filter(|(_, f)| f.remaining() > 0).map(|(name, f)| format!("'{}' ({} characters)", name, f.remaining())).collect();
            return Err(format!("{}: still text left after {} pages (max_pages): {}", caller, n, left.join(", ")).into());
        }
        let mut info = Map::new();
        info.insert("width".into(), Dynamic::from(content.width as f64));
        info.insert("height".into(), Dynamic::from(content.height as f64));
        info.insert("flows".into(), Dynamic::from_map(content.flows.iter().map(|(name, f)| (name.into(), Dynamic::from(f.remaining() as i64))).collect()));
        info.insert("art".into(), Dynamic::from_array(content.art.keys().map(|k| Dynamic::from(k.to_string())).collect()));
        let root = layout.page(&ctx, n, info)?;
        let (mut canvas, moved) = typeset_page(&ctx, &root, n, &mut content, symdict, rng)?;
        let left: Vec<String> = content.flows.iter().filter(|(_, f)| f.remaining() > 0).map(|(name, _)| format!("'{}'", name)).collect();
        if !moved && !left.is_empty() {
            return Err(format!("{}: page {} sets none of what's left of flow(s) {}; give them slots, or more room", caller, n, left.join(", ")).into());
        }
        let name = format!("{}_{}", content.name, n);
        canvas.frame = Some(Frame {
            page: Aabb::new(PVec::ZERO, PVec::new(content.width, content.height)),
            column: 0, columns: 1, row: 0, rows: 1,
            svg_seed: stable_hash(&format!("{}_0", name)),
        });
        pages.push(Dynamic::from(Page { name: Some(name), canvases: vec![canvas], ..Page::default() }));
    }
    Ok(pages)
}

pub fn register(engine: &mut Engine, symdict: Rc<SymbolPathDict>, rng: SharedRng, layouts_dir: PathBuf) {
    engine.register_type_with_name::<Node>("LayoutBox");
    engine.register_type_with_name::<Layout>("Layout");
    for (name, kind) in [("row", Kind::Row), ("column", Kind::Column)] {
        let k = kind.clone();
        engine.register_fn(name, move |children: Array| Node::new(name, k.clone(), Map::new(), children));
        engine.register_fn(name, move |options: Map, children: Array| Node::new(name, kind.clone(), options, children));
    }
    type Leaf = fn(String) -> Kind;
    for (name, make) in [
        ("flow", Kind::Flow as Leaf), ("label", Kind::Label as Leaf), ("initial", Kind::Initial as Leaf),
        ("figure", Kind::Figure as Leaf), ("band", Kind::Band as Leaf),
    ] {
        engine.register_fn(name, move |text: &str| Node::new(name, make(text.to_string()), Map::new(), Array::new()));
        engine.register_fn(name, move |text: &str, options: Map| Node::new(name, make(text.to_string()), options, Array::new()));
    }
    for (name, kind) in [("rule", Kind::Rule), ("spacer", Kind::Spacer)] {
        let k = kind.clone();
        engine.register_fn(name, move || Node::new(name, k.clone(), Map::new(), Array::new()));
        engine.register_fn(name, move |options: Map| Node::new(name, kind.clone(), options, Array::new()));
    }
    engine.register_fn("layout", move |ctx: NativeCallContext, name: &str| load_layout(&ctx, &layouts_dir, name));
    engine.register_fn("new_layout", |name: &str, page: FnPtr| -> RhaiResult<Layout> {
        Ok(Layout { name: super::check_name("new_layout", Dynamic::from(name.to_string()))?, source: Source::Function(page) })
    });
    engine.register_get("name", |l: &mut Layout| l.name.clone());
    engine.register_fn("typeset", move |ctx: NativeCallContext, layout: &mut Layout, content: Map| typeset(ctx, layout, content, &symdict, &rng));
}

#[cfg(test)]
mod tests {
    use super::super::{Entry, JournalEngine};
    use crate::journal::canvas::Canvas as JournalCanvas;
    use crate::journal::symboldict::SymbolPathDict;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        let symd = SymbolPathDict::from_file(crate::journal::symboldict::DEFAULT_SYMBOLS).expect("Failed to load symbols.json");
        JournalEngine::new(Rc::new(symd))
    }

    const SETUP: &str = r#"
        let pen = new_brush("Ink Pen", 0.3, 0.7, 1.0);
        let ink = new_strokestyle(1.0, "black", pen);
        let red = new_strokestyle(1.0, "darkred", pen);
        let font = lowercase_font("loche_upper");
        let words = "the quick brown fox jumps over the lazy dog ";
        let roles = #{
            body: #{ font: font, style: ink, size: 12.0 },
            initial: #{ font: font, style: red },
            heading: #{ font: font, style: red, size: 30.0, align: "center" },
            divider: #{ style: red },
            rule: #{ style: red },
        };
        fn two_columns(n, info) {
            column(#{ padding: 20, gap: 10 }, [
                if n == 0 { flow("title", #{ role: "heading", height: 40, fit: true }) } else { spacer(#{ height: 40 }) },
                row(#{ grow: 1, gap: 10 }, [
                    column(#{ grow: 1 }, [
                        if n == 0 { initial("body", #{ position: "absolute", top: 0, left: 0, width: 40, height: 40 }) } else { spacer() },
                        flow("body", #{ grow: 1 }),
                    ]),
                    band("divider", #{ width: 6 }),
                    flow("body", #{ grow: 1 }),
                ]),
                rule(#{ height: 4 }),
            ])
        }
        let book = new_layout("book", Fn("two_columns"));
    "#;

    fn build(script: &str) -> Result<Entry, String> {
        engine().build_entry_str(&format!("{}\n{}", SETUP, script))
    }

    /// Glyph strokes on each page, and where they are.
    fn glyph_centres(entry: &Entry, page: usize, color: &str) -> Vec<parry2d::math::Vec2> {
        entry.pages[page].canvases()[0].strokes.iter()
            .filter(|s| s.style.color == crate::journal::canvas::StrokeColor::from(color))
            .map(|s| {
                let points = s.path.points_within();
                points.iter().fold(parry2d::math::Vec2::ZERO, |a, p| a + *p) / points.len() as f32
            })
            .collect()
    }

    #[test]
    fn text_chains_through_columns_and_pages_until_it_runs_out() {
        let entry = build(r#"
            let text = words + words + words + words + words + words + words + words;
            text = text + text + text;
            journal_entry.add_pages(book.typeset(#{
                width: 400, height: 300, flows: #{ title: "Title", body: text }, roles: roles,
                art: #{ divider: new_circle(new_vec(0.0, 0.0), 3.0) },
            }));
        "#).expect("script should run");
        assert!(entry.pages.len() > 1, "{} pages", entry.pages.len());
        let labels: Vec<String> = entry.pages.iter().enumerate().map(|(i, p)| p.label(i)).collect();
        assert_eq!(labels[..2], ["book_0", "book_1"]);
        // Every letter set once: one stroke per non-blank glyph, the drop
        // cap's included (in red, with the title and ornaments).
        let letters = ("the quick brown fox jumps over the lazy dog ".len() - 9) * 24;
        let body: usize = (0..entry.pages.len()).map(|p| glyph_centres(&entry, p, "black").len()).sum();
        assert_eq!(body + 1, letters);
        // Pages fill both columns before the next page starts.
        let first = glyph_centres(&entry, 0, "black");
        assert!(first.iter().any(|p| p.x < 190.0) && first.iter().any(|p| p.x > 210.0));
        // Pages render the whole page, every time.
        for page in &entry.pages {
            let canvas: JournalCanvas = page.canvases()[0].clone().into();
            let frame = canvas.frame().expect("typeset pages are framed");
            assert_eq!((frame.page.extents().x, frame.page.extents().y), (400.0, 300.0));
        }
    }

    #[test]
    fn text_wraps_around_the_drop_cap() {
        let entry = build(r#"
            let text = words + words + words + words + words + words;
            journal_entry.add_pages(book.typeset(#{
                width: 400, height: 300, flows: #{ title: "Title", body: text }, roles: roles,
                art: #{ divider: new_circle(new_vec(0.0, 0.0), 3.0) },
            }));
        "#).expect("script should run");
        // The drop cap's box is 40 x 40 at the column's corner, (20, 70).
        for p in glyph_centres(&entry, 0, "black") {
            assert!(!(p.x < 60.0 + 3.0 && p.y < 110.0), "a glyph at {:?} overlaps the drop cap", p);
        }
        // The drop cap is the first letter, "t", and the text goes on from "he".
        let red = glyph_centres(&entry, 0, "darkred");
        assert!(red.iter().any(|p| p.x > 20.0 && p.x < 60.0 && p.y > 70.0 && p.y < 110.0), "{:?}", red);
    }

    #[test]
    fn layout_files_load_by_name() {
        let entry = build(r#"
            let pages = layout("missal_figure").typeset(#{
                flows: #{ caption: "a circle" },
                roles: #{ caption: #{ font: font, style: red, size: 40.0 }, figure: #{ style: ink } },
                art: #{ figure: new_circle(new_vec(0.0, 0.0), 1.0) },
            });
            if pages.len() != 1 { throw "one page"; }
            journal_entry.add_pages(pages);
        "#).expect("script should run");
        let canvas: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        // The page size from the file, and the circle fitted to its box.
        assert_eq!(canvas.frame().unwrap().page.extents().x, 1400.0);
        let circle = &canvas.strokes()[0];
        let xs: Vec<f32> = circle.path.points_within().iter().map(|p| p.x).collect();
        let width = xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min);
        assert!(width > 500.0, "the figure should be scaled up to its box, got {}", width);
        let err = engine().build_entry_str("let ok = 1;\nlayout(\"no_such_layout\");").expect_err("unknown layout");
        assert!(err.contains("no layout file") && err.contains("missal_spread") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn bands_repeat_their_art_along_the_box() {
        let entry = build(r#"
            let strip = new_layout("strip", |n, info| column(#{ padding: 10 }, [band("dot", #{ height: 20 })]));
            journal_entry.add_pages(strip.typeset(#{ width: 220, height: 40, roles: #{ dot: #{ style: ink } }, art: #{ dot: new_circle(new_vec(0.0, 0.0), 5.0) } }));
        "#).expect("script should run");
        // 200 long, 20 across: ten circles 20 across.
        assert_eq!(entry.pages[0].canvases()[0].strokes.len(), 10);
    }

    #[test]
    fn mistakes_fail_with_what_went_wrong() {
        let typeset = |content: &str| format!("journal_entry.add_pages(book.typeset(#{{ width: 400, height: 300, roles: roles, art: #{{ divider: new_circle(new_vec(0.0, 0.0), 3.0) }}, {} }}));", content);
        for (script, want) in [
            ("let ok = 1;\nrow(#{ colour: 1 }, []);", "row: unknown option 'colour'"),
            ("let ok = 1;\nflow(\"a\", #{ width: \"wide\" });", "'width' should be a number, \"N%\" or \"auto\""),
            ("let ok = 1;\nflow(\"a\", #{ align: \"middle\" });", "unknown align 'middle'"),
            ("let ok = 1;\ncolumn(#{ justify: \"left\" }, []);", "unknown justify 'left'"),
            ("let ok = 1;\nrow([1]);", "child 0 is a i64"),
            ("let ok = 1;\nflow(\"a\", #{ gap: 3 });", "unknown option 'gap'"),
            (&typeset("flows: #{ title: \"t\" }"), "sets a flow 'body', but there's no such flow"),
            (&typeset("flows: #{ title: \"t\", body: \"b\", extra: \"never placed\" }"), "page 1 sets none of what's left of flow(s) 'extra'"),
            (&typeset("flows: #{ title: \"t\", body: words + words + words + words + words + words + words + words + words + words + words + words + words + words + words + words }, max_pages: 1"), "still text left after 1 pages"),
            ("journal_entry.add_pages(book.typeset(#{ width: 400, height: 300, roles: roles, flows: #{ title: \"t\", body: \"b\" } }));", "there's no art named 'divider'"),
            ("roles.remove(\"body\");\njournal_entry.add_pages(book.typeset(#{ width: 400, height: 300, roles: roles, flows: #{ title: \"t\", body: \"b\" } }));", "uses role 'body', which `roles` doesn't have"),
            ("journal_entry.add_pages(book.typeset(#{ roles: roles }));", "give the page's 'width'"),
            ("roles.body.size = -1;\njournal_entry.add_pages(book.typeset(#{ width: 400, height: 300, roles: roles }));", "'size' must be a positive number"),
            ("journal_entry.add_pages(new_layout(\"bad\", |n, info| 3).typeset(#{ width: 10, height: 10 }));", "returned a i64, not a layout box"),
        ] {
            let err = build(script).expect_err(script);
            assert!(err.contains(want), "{}:\n{}", want, err);
        }
    }

    #[test]
    fn layout_file_mistakes_name_the_file_and_line() {
        let dir = std::env::temp_dir().join(format!("journal_layouts_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bad.rhai"), "fn page(n, info) {\n\trow(#{ colour: 1 }, [])\n}\n").unwrap();
        std::fs::write(dir.join("pageless.rhai"), "fn other() {}\n").unwrap();
        let symd = SymbolPathDict::from_file(crate::journal::symboldict::DEFAULT_SYMBOLS).unwrap();
        let mut engine = JournalEngine::with_dirs(Rc::new(symd), crate::journal::rhaientries::DEFAULT_LOCKS_DIR, &dir);
        let err = engine.build_entry_str("layout(\"bad\").typeset(#{ width: 10, height: 10 });").expect_err("bad layout");
        assert!(err.contains("layout 'bad' page 0") && err.contains("unknown option 'colour'") && err.contains("line 2"), "{}", err);
        let err = engine.build_entry_str("layout(\"pageless\");").expect_err("no page()");
        assert!(err.contains("doesn't define page(n, info)"), "{}", err);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn typeset_pages_split_into_tiles() {
        let entry = build(r#"
            let pages = book.typeset(#{ width: 400, height: 300, flows: #{ title: "Title", body: words }, roles: roles, art: #{ divider: new_circle(new_vec(0.0, 0.0), 3.0) } });
            journal_entry.add_pages(pages[0].split(2, 1));
        "#).expect("script should run");
        let tile: JournalCanvas = entry.pages[1].canvases()[0].clone().into();
        assert_eq!(tile.frame().unwrap().page.extents().x, 400.0);
        assert_eq!(tile.frame().unwrap().columns, 2);
    }
}
