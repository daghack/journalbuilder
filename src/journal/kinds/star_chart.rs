//! StarChart: a sky of named stars over pages, as it's drawn.
//!
//! ```rhai
//! #{ kind: "StarChart", input_base: 27, output_base: 3,
//!    pages: [#{ name: "Obilaegal",
//!               stars: [#{ name: "Shiathri", point: [520.0, 300.0], size: 30.0, label: [560.0, 270.0] }, ...],
//!               links: [[0, 1, 2], ...] }] }
//! ```
//!
//! Each page is what's drawn on it: its stars, where they are, how far
//! their marks reach (`size`) and where their labels go, and its links as
//! [parent, child, digit] indices into its stars. A star on more than one
//! page has the same name on each; nothing else ties the pages together.
//!
//! Its kernels, `stars::layout` and `stars::split`, lay out a tree and cut
//! it into pages, as `art::huffman_stars` and `art::constellations` do.

use parry2d::math::Vec2;
use rhai::{Array, Dynamic, Map, Module, NativeCallContext, INT};

use super::huffman_code::Tree;
use super::schema::{list, record, Schema};
use super::{check, fail, field, get, point, read_index, read_number, read_point, Label, Options, Sketch, ValueKind};
use crate::journal::art::{constellations, lay_out_tree, ConstellationSettings, StarChartSettings};
use crate::journal::paths::StrokePath;

pub struct StarChart;

impl ValueKind for StarChart {
    fn name(&self) -> &'static str {
        "StarChart"
    }

    fn module(&self) -> Option<&'static str> {
        Some("stars")
    }

    fn schema(&self) -> Schema {
        let star = record(&[("name", Schema::Text), ("point", Schema::Point), ("size", Schema::Number), ("label", Schema::Point)]);
        record(&[
            ("kind", Schema::Kind("StarChart")),
            ("input_base", Schema::Int),
            ("output_base", Schema::Int),
            ("pages", list(record(&[
                ("name", Schema::Text),
                ("stars", list(star)),
                ("links", list(Schema::Tuple(vec![Schema::Int, Schema::Int, Schema::Int]))),
            ]))),
        ])
    }

    fn validate(&self, value: &Map) -> Result<(), String> {
        let output_base = read_index(field(value, "output_base"), "output_base")?;
        for (p, page) in pages(value).iter().enumerate() {
            let stars = list_of(page, "stars");
            let mut names = std::collections::HashSet::new();
            for (k, star) in stars.iter().enumerate() {
                let name = field(&as_map(star), "name").clone().into_string().unwrap_or_default();
                if name.is_empty() {
                    return Err(format!("pages[{}].stars[{}] needs a name", p, k));
                }
                if !names.insert(name.clone()) {
                    return Err(format!("pages[{}] has two stars named {}", p, name));
                }
            }
            for (l, link) in list_of(page, "links").iter().enumerate() {
                let [a, b, digit] = triple(link);
                if a >= stars.len() || b >= stars.len() || a == b {
                    return Err(format!("pages[{}].links[{}] should join two of the page's {} stars", p, l, stars.len()));
                }
                if digit >= output_base {
                    return Err(format!("pages[{}].links[{}]'s digit is {}, outside base {}", p, l, digit, output_base));
                }
            }
        }
        Ok(())
    }

    fn kernels(&self, module: &mut Module) {
        module.set_native_fn("layout", |ctx: NativeCallContext, code: Map| -> Result<Map, Box<rhai::EvalAltResult>> { layout(&ctx, &code, &Map::new()) });
        module.set_native_fn("layout", |ctx: NativeCallContext, code: Map, options: Map| -> Result<Map, Box<rhai::EvalAltResult>> { layout(&ctx, &code, &options) });
        module.set_native_fn("split", |ctx: NativeCallContext, points: Array, edges: Array, sizes: Array| -> Result<Array, Box<rhai::EvalAltResult>> {
            split(&ctx, &points, &edges, &sizes, &Map::new())
        });
        module.set_native_fn("split", |ctx: NativeCallContext, points: Array, edges: Array, sizes: Array, options: Map| -> Result<Array, Box<rhai::EvalAltResult>> {
            split(&ctx, &points, &edges, &sizes, &options)
        });
    }

    /// The pages in a grid, as near square as it goes: each star a ring as
    /// big as its size, its links between them, its number on the page at
    /// its label, and the page's name over it.
    fn sketch(&self, value: &Map) -> Option<Sketch> {
        type Star = (Vec2, f32, Vec2);
        let pages: Vec<(String, Vec<Star>, Array)> = pages(value).iter().map(|page| {
            let stars: Vec<Star> = list_of(page, "stars").iter().map(|s| {
                let s = as_map(s);
                (read_point(field(&s, "point"), "point").unwrap_or_default(), read_number(field(&s, "size"), "size").unwrap_or(1.0) as f32, read_point(field(&s, "label"), "label").unwrap_or_default())
            }).collect();
            (field(page, "name").clone().into_string().unwrap_or_default(), stars, list_of(page, "links"))
        }).filter(|(_, stars, _)| !stars.is_empty()).collect();
        // Each page's box, its stars' marks and labels in it.
        let boxes: Vec<(Vec2, Vec2)> = pages.iter().map(|(_, stars, _)| {
            stars.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), (p, r, l)| {
                (lo.min(*p - Vec2::splat(*r)).min(*l), hi.max(*p + Vec2::splat(*r)).max(*l))
            })
        }).collect();
        let biggest = pages.iter().flat_map(|(_, stars, _)| stars.iter().map(|s| s.1)).fold(1.0f32, f32::max);
        let cell = boxes.iter().fold(Vec2::ZERO, |c, (lo, hi)| c.max(*hi - *lo)) + Vec2::new(biggest * 2.0, biggest * 4.0);
        let columns = (pages.len() as f32).sqrt().ceil().max(1.0) as usize;
        let (mut rings, mut links, mut labels) = (StrokePath::new(), StrokePath::new(), Vec::new());
        for (i, ((name, stars, page_links), (lo, hi))) in pages.iter().zip(&boxes).enumerate() {
            let corner = Vec2::new((i % columns) as f32 * cell.x, (i / columns) as f32 * cell.y);
            let shift = corner - *lo + Vec2::new(0.0, biggest * 2.5);
            for (k, (p, r, l)) in stars.iter().enumerate() {
                rings.append(StrokePath::circle(*p + shift, r * 0.5));
                labels.push(Label { at: *l + shift, text: k.to_string() });
            }
            for link in page_links {
                let [a, b, _] = triple(link);
                let ((pa, ra, _), (pb, rb, _)) = (stars[a], stars[b]);
                let along = (pb - pa).normalize_or_zero();
                if (pb - pa).length() > ra * 0.5 + rb * 0.5 {
                    links.append(StrokePath::line(pa + along * ra * 0.5 + shift, pb - along * rb * 0.5 + shift));
                }
            }
            labels.push(Label { at: corner + Vec2::new((hi.x - lo.x) / 2.0, biggest), text: name.clone() });
        }
        Some(Sketch { parts: vec![("links".into(), links), ("stars".into(), rings)], labels, label_size: biggest })
    }
}

fn as_map(value: &Dynamic) -> Map {
    value.read_lock::<Map>().map(|m| m.clone()).unwrap_or_default()
}

fn list_of(map: &Map, key: &str) -> Array {
    field(map, key).read_lock::<Array>().map(|a| a.clone()).unwrap_or_default()
}

fn pages(chart: &Map) -> Vec<Map> {
    list_of(chart, "pages").iter().map(as_map).collect()
}

/// A checked link's three indices.
fn triple(link: &Dynamic) -> [usize; 3] {
    let items = link.read_lock::<Array>().map(|a| a.clone()).unwrap_or_default();
    let at = |i: usize| items.get(i).and_then(|v| v.as_int().ok()).and_then(|v| usize::try_from(v).ok()).unwrap_or(usize::MAX);
    [at(0), at(1), at(2)]
}

const LAYOUT_KEYS: [&str; 6] = ["seed", "variation", "iterations", "star_size", "width", "height"];

/// `stars::layout(code[, #{ seed, variation, iterations, star_size, width, height }])`:
/// #{ points, sizes }, a star for each of the code's nodes, in its order.
fn layout(ctx: &NativeCallContext, code: &Map, options: &Map) -> Result<Map, Box<rhai::EvalAltResult>> {
    const CALLER: &str = "stars::layout";
    let err = |m: String| fail(ctx, CALLER, m);
    let kind = get("HuffmanCode").expect("HuffmanCode is a kind");
    check(kind, &Dynamic::from_map(code.clone())).map_err(|m| err(format!("a bad HuffmanCode: {}", m)))?;
    let o = Options::new(options, &LAYOUT_KEYS).map_err(err)?;
    let (width, height) = (o.number("width", 1000.0).map_err(err)?, o.number("height", 800.0).map_err(err)?);
    let settings = StarChartSettings {
        center: Vec2::new(width as f32 / 2.0, height as f32 / 2.0),
        width,
        height,
        round: false,
        smoothing: 0,
        variation: o.number("variation", 0.6).map_err(err)?,
        iterations: o.index("iterations", 150).map_err(err)?,
        star_size: o.optional_number("star_size").map_err(err)?,
        gap: None,
        seed: o.index("seed", 0).map_err(err)? as u64,
    };
    let tree = Tree::read(code).map_err(err)?;
    let (points, sizes) = lay_out_tree(&tree.parent, &settings).map_err(err)?;
    let mut out = Map::new();
    out.insert("points".into(), Dynamic::from_array(points.into_iter().map(point).collect()));
    out.insert("sizes".into(), Dynamic::from_array(sizes.into_iter().map(Dynamic::from_float).collect()));
    Ok(out)
}

const SPLIT_KEYS: [&str; 8] = ["count", "most_stars", "split_at", "width", "height", "label_width", "label_height", "label_gap"];

/// `stars::split(points, edges, sizes[, #{ count, most_stars, split_at, width, height, label_width, label_height, label_gap }])`:
/// the tree cut into pieces, each fitted to the box from (0, 0) to
/// (width, height): #{ stars, points, labels, pairs }.
fn split(ctx: &NativeCallContext, points: &Array, edges: &Array, sizes: &Array, options: &Map) -> Result<Array, Box<rhai::EvalAltResult>> {
    const CALLER: &str = "stars::split";
    let err = |m: String| fail(ctx, CALLER, m);
    let o = Options::new(options, &SPLIT_KEYS).map_err(err)?;
    let points = points.iter().enumerate().map(|(i, p)| read_point(p, &format!("points[{}]", i))).collect::<Result<Vec<_>, _>>().map_err(err)?;
    let sizes = sizes.iter().enumerate().map(|(i, s)| read_number(s, &format!("sizes[{}]", i))).collect::<Result<Vec<_>, _>>().map_err(err)?;
    let edges = edges.iter().enumerate().map(|(i, e)| {
        let pair = e.read_lock::<Array>().filter(|p| p.len() == 2).ok_or_else(|| format!("edges[{}] should be [parent, child]", i))?;
        Ok((read_index(&pair[0], &format!("edges[{}]", i))?, read_index(&pair[1], &format!("edges[{}]", i))?))
    }).collect::<Result<Vec<_>, String>>().map_err(err)?;
    let (width, height) = (o.number("width", 1000.0).map_err(err)?, o.number("height", 860.0).map_err(err)?);
    let settings = ConstellationSettings {
        center: Vec2::new(width as f32 / 2.0, height as f32 / 2.0),
        width,
        height,
        label_width: o.number("label_width", 30.0).map_err(err)?,
        label_height: o.number("label_height", 24.0).map_err(err)?,
        label_gap: o.number("label_gap", 6.0).map_err(err)?,
        split_at: o.indices("split_at").map_err(err)?,
        most_stars: o.optional_index("most_stars").map_err(err)?,
    };
    let pieces = constellations(&points, &sizes, &edges, o.index("count", 1).map_err(err)?, &settings).map_err(err)?;
    let int = |i: usize| Dynamic::from_int(i as INT);
    Ok(pieces.into_iter().map(|piece| {
        let mut m = Map::new();
        m.insert("stars".into(), Dynamic::from_array(piece.stars.iter().map(|&s| int(s)).collect()));
        m.insert("points".into(), Dynamic::from_array(piece.points.into_iter().map(point).collect()));
        m.insert("labels".into(), Dynamic::from_array(piece.labels.into_iter().map(point).collect()));
        m.insert("pairs".into(), Dynamic::from_array(piece.pairs.iter().map(|&(a, b)| Dynamic::from_array(vec![int(a), int(b)])).collect()));
        Dynamic::from_map(m)
    }).collect())
}
