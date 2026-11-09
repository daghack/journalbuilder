//! Graph: places, and the ways between them.
//!
//! ```rhai
//! #{ kind: "Graph",
//!    nodes: [#{ name: "Paper Mazes", glyphs: ["basic_eye"] }, #{ name: "Saint's Rest", glyphs: [] }],
//!    edges: [#{ from: 0, to: 1, directed: true, text: "Follow the red thread" }] }
//! ```
//!
//! Each node is a place: its name, and the names of glyphs that go with
//! it. Each edge joins two nodes, by index, one way (`directed`) or both,
//! and its `text` says how ("" for nothing). Names and texts are marked-up
//! text, as `markdown(...)` reads it.
//!
//! Its kernels: `graph::parse(text)` reads a graph written as a `.graph`
//! file, and `graph::layout(graph, sizes[, options])` lays one out by
//! forces (`art::graph_layout`).
//!
//! A `.graph` file, a line at a time:
//!
//! ```text
//! # a comment
//! Paper Mazes: basic_eye archangel_3
//! Saint's Rest: ploughdiem
//!
//! Paper Mazes -> Saint's Rest: Follow the red thread until you find
//!   _To Fall Endlessly_, then use the crawlspace beneath the broken shelf
//! Saint's Rest -- The Cloister
//! ```
//!
//! A line is a node, `Name` or `Name: glyph glyph`, or an edge, `A -> B`
//! (one way) or `A -- B` (both ways), then `: text` if it has any. An
//! indented line carries on the line above; `#` lines are comments, and
//! blank lines are nothing. An edge can name a node no line gives, and
//! nodes come in the order they're first named. Names can't hold a colon.

use parry2d::math::Vec2;
use rhai::{Array, Dynamic, Map, Module, NativeCallContext, INT};

use super::schema::{list, record, Schema};
use super::{check, fail, field, point, read_number, Label, Options, Sketch, ValueKind};
use crate::journal::art::{lay_out_graph, GraphLayoutSettings, LabelPlace};
use crate::journal::markup::Markup;
use crate::journal::paths::StrokePath;

pub struct Graph;

impl ValueKind for Graph {
    fn name(&self) -> &'static str {
        "Graph"
    }

    fn module(&self) -> Option<&'static str> {
        Some("graph")
    }

    fn schema(&self) -> Schema {
        record(&[
            ("kind", Schema::Kind("Graph")),
            ("nodes", list(record(&[("name", Schema::Text), ("glyphs", list(Schema::Text))]))),
            ("edges", list(record(&[("from", Schema::Int), ("to", Schema::Int), ("directed", Schema::Bool), ("text", Schema::Text)]))),
        ])
    }

    fn validate(&self, value: &Map) -> Result<(), String> {
        let nodes = list_of(value, "nodes");
        let mut names: Vec<String> = Vec::new();
        for (k, node) in nodes.iter().enumerate() {
            let name = text_of(&as_map(node), "name");
            if name.trim().is_empty() {
                return Err(format!("nodes[{}] needs a name", k));
            }
            if names.contains(&name) {
                return Err(format!("there are two nodes named {}", name));
            }
            names.push(name);
        }
        for (e, edge) in list_of(value, "edges").iter().enumerate() {
            let edge = as_map(edge);
            let end = |key: &str| field(&edge, key).as_int().ok().and_then(|i| usize::try_from(i).ok()).filter(|&i| i < nodes.len());
            match (end("from"), end("to")) {
                (Some(a), Some(b)) if a != b => {}
                (Some(_), Some(_)) => return Err(format!("edges[{}] joins a node to itself", e)),
                _ => return Err(format!("edges[{}] should join two of the {} nodes", e, nodes.len())),
            }
        }
        Ok(())
    }

    fn kernels(&self, module: &mut Module) {
        module.set_native_fn("parse", |ctx: NativeCallContext, text: &str| -> Result<Map, Box<rhai::EvalAltResult>> {
            parse(text).map_err(|m| fail(&ctx, "graph::parse", m))
        });
        module.set_native_fn("layout", |ctx: NativeCallContext, graph: Map, sizes: Array| -> Result<Map, Box<rhai::EvalAltResult>> {
            layout(&ctx, &graph, &sizes, &Map::new())
        });
        module.set_native_fn("layout", |ctx: NativeCallContext, graph: Map, sizes: Array, options: Map| -> Result<Map, Box<rhai::EvalAltResult>> {
            layout(&ctx, &graph, &sizes, &options)
        });
    }

    /// Each node a box round its name, a ring under it for each glyph;
    /// each edge its line, an arrowhead if it goes one way, and its text
    /// upright across it, a few words to a line.
    fn sketch(&self, value: &Map) -> Option<Sketch> {
        const SIZE: f32 = 20.0;
        let s = SIZE as f64;
        let nodes: Vec<Map> = list_of(value, "nodes").iter().map(as_map).collect();
        let edges: Vec<Map> = list_of(value, "edges").iter().map(as_map).collect();
        let names: Vec<String> = nodes.iter().map(|n| plain(&text_of(n, "name"))).collect();
        let glyphs: Vec<usize> = nodes.iter().map(|n| list_of(n, "glyphs").len()).collect();
        let sizes: Vec<(f64, f64)> = names.iter().zip(&glyphs).map(|(name, &g)| {
            let wide = (name.chars().count() as f64 * 0.6 * s).max(g as f64 * 0.9 * s);
            (wide + s, if g > 0 { 2.8 * s } else { 1.6 * s })
        }).collect();
        let texts: Vec<Vec<String>> = edges.iter().map(|e| wrap(&plain(&text_of(e, "text")), 24)).collect();
        let labels: Vec<Option<(f64, f64)>> = texts.iter().map(|lines| {
            let longest = lines.iter().map(|l| l.chars().count()).max()?;
            Some((longest as f64 * 0.6 * s, lines.len() as f64 * 1.2 * s))
        }).collect();
        let ends: Vec<(usize, usize)> = edges.iter().map(|e| (index_of(e, "from"), index_of(e, "to"))).collect();
        let settings = GraphLayoutSettings { width: 1000.0, height: 800.0, gap: 0.5 * s, seed: 0, iterations: 300, place: LabelPlace::Upright };
        let laid = lay_out_graph(&sizes, &ends, &labels, &settings).ok()?;
        let (mut boxes, mut rings, mut lines, mut arrows) = (StrokePath::new(), StrokePath::new(), StrokePath::new(), StrokePath::new());
        let mut words = Vec::new();
        for (k, (&c, &(w, h))) in laid.points.iter().zip(&sizes).enumerate() {
            boxes.append(StrokePath::rectangle(c, w as f32, h as f32));
            let name_at = if glyphs[k] > 0 { c - Vec2::new(0.0, 0.6 * SIZE) } else { c };
            words.push(Label { at: name_at, text: names[k].clone() });
            for g in 0..glyphs[k] {
                let x = (g as f32 - (glyphs[k] as f32 - 1.0) / 2.0) * 0.9 * SIZE;
                rings.append(StrokePath::circle(c + Vec2::new(x, 0.7 * SIZE), 0.3 * SIZE));
            }
        }
        for (e, edge) in laid.edges.iter().enumerate() {
            for &(a, b) in &edge.pieces {
                lines.append(StrokePath::line(a, b));
            }
            if field(&edges[e], "directed").as_bool().unwrap_or(false) {
                let u = (edge.end - edge.start).normalize_or_zero();
                let back = edge.end - u * 0.6 * SIZE;
                let side = u.perp() * 0.3 * SIZE;
                arrows.append(StrokePath::line(back + side, edge.end));
                arrows.append(StrokePath::line(edge.end, back - side));
            }
            let rows = texts[e].len() as f32;
            for (r, row) in texts[e].iter().enumerate() {
                words.push(Label { at: edge.label + Vec2::new(0.0, (r as f32 - (rows - 1.0) / 2.0) * 1.2 * SIZE), text: row.clone() });
            }
        }
        Some(Sketch { parts: vec![("boxes".into(), boxes), ("glyphs".into(), rings), ("lines".into(), lines), ("arrows".into(), arrows)], labels: words, label_size: SIZE })
    }

    fn example(&self) -> Option<&'static str> {
        Some(concat!(
            "graph::parse(\"Paper Mazes: basic_eye archangel_3\\nSaint's Rest: ploughdiem\\n",
            "Paper Mazes -> Saint's Rest: Follow the red thread until you find _To Fall Endlessly_, then use the crawlspace beneath the broken shelf\\n",
            "Saint's Rest -- The Cloister: down the bell stair\\nThe Cloister -> Paper Mazes: through the reading room\")",
        ))
    }
}

fn as_map(value: &Dynamic) -> Map {
    value.read_lock::<Map>().map(|m| m.clone()).unwrap_or_default()
}

fn list_of(map: &Map, key: &str) -> Array {
    field(map, key).read_lock::<Array>().map(|a| a.clone()).unwrap_or_default()
}

fn text_of(map: &Map, key: &str) -> String {
    field(map, key).clone().into_string().unwrap_or_default()
}

fn index_of(map: &Map, key: &str) -> usize {
    field(map, key).as_int().ok().and_then(|i| usize::try_from(i).ok()).unwrap_or(0)
}

/// Marked-up text as it reads, the marks taken out.
fn plain(text: &str) -> String {
    Markup::parse(text).map_or_else(|_| text.to_string(), |m| m.text)
}

/// Words in lines of about `width` characters.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// A node as it's read: its name, its glyphs, and the line that gave it,
/// if one did.
struct Node {
    name: String,
    glyphs: Vec<String>,
    line: Option<usize>,
}

/// A graph from its `.graph` text.
pub fn parse(source: &str) -> Result<Map, String> {
    // Lines with what's indented under them joined on, and their numbers.
    let mut lines: Vec<(usize, String)> = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if raw.starts_with(char::is_whitespace) {
            let Some((_, line)) = lines.last_mut() else {
                return Err(format!("line {}: an indented line carries on the line above, and there's none", i + 1));
            };
            line.push(' ');
            line.push_str(trimmed);
        } else {
            lines.push((i + 1, trimmed.to_string()));
        }
    }
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<(usize, usize, bool, String)> = Vec::new();
    let named = |nodes: &mut Vec<Node>, name: &str| -> usize {
        nodes.iter().position(|n| n.name == name).unwrap_or_else(|| {
            nodes.push(Node { name: name.to_string(), glyphs: Vec::new(), line: None });
            nodes.len() - 1
        })
    };
    for (n, line) in &lines {
        let (head, tail) = match line.split_once(':') {
            Some((head, tail)) => (head, Some(tail.trim())),
            None => (line.as_str(), None),
        };
        let at = |e: String| format!("line {}: {}", n, e);
        if let Some((a, op, b)) = edge_of(head).map_err(at)? {
            if a == b {
                return Err(at(format!("{} {} {} joins {} to itself", a, op, b, a)));
            }
            let text = tail.unwrap_or("");
            for t in [a, b, text] {
                Markup::parse(t).map_err(at)?;
            }
            let (from, to) = (named(&mut nodes, a), named(&mut nodes, b));
            edges.push((from, to, op == "->", text.to_string()));
        } else {
            let name = head.trim();
            if name.is_empty() {
                return Err(at("a line names a place, or joins two with -> or --".into()));
            }
            Markup::parse(name).map_err(at)?;
            let k = named(&mut nodes, name);
            if let Some(m) = nodes[k].line {
                return Err(at(format!("{} has a line already, line {}", name, m)));
            }
            nodes[k].line = Some(*n);
            nodes[k].glyphs = tail.unwrap_or("").split_whitespace().map(String::from).collect();
        }
    }
    let mut graph = Map::new();
    graph.insert("kind".into(), "Graph".into());
    graph.insert("nodes".into(), Dynamic::from_array(nodes.into_iter().map(|node| {
        let mut m = Map::new();
        m.insert("name".into(), node.name.into());
        m.insert("glyphs".into(), Dynamic::from_array(node.glyphs.into_iter().map(Dynamic::from).collect()));
        Dynamic::from_map(m)
    }).collect()));
    graph.insert("edges".into(), Dynamic::from_array(edges.into_iter().map(|(from, to, directed, text)| {
        let mut m = Map::new();
        m.insert("from".into(), Dynamic::from_int(from as INT));
        m.insert("to".into(), Dynamic::from_int(to as INT));
        m.insert("directed".into(), Dynamic::from_bool(directed));
        m.insert("text".into(), text.into());
        Dynamic::from_map(m)
    }).collect()));
    Ok(graph)
}

/// The two ends of an edge and its `->` or `--`, if the line is one.
fn edge_of(head: &str) -> Result<Option<(&str, &'static str, &str)>, String> {
    let found = ["->", "--"].iter().filter_map(|&op| head.find(op).map(|at| (at, op))).min();
    let Some((at, op)) = found else { return Ok(None) };
    let (before, after) = (&head[..at], &head[at + op.len()..]);
    let (a, b) = (before.trim(), after.trim());
    if a.is_empty() || b.is_empty() {
        return Err(format!("an edge needs a place either side of {}", op));
    }
    if !before.ends_with(' ') || !after.starts_with(' ') {
        return Err(format!("put a space either side of {}", op));
    }
    if b.contains(" -> ") || b.contains(" -- ") {
        return Err("an edge joins two places: give each way a line of its own".into());
    }
    Ok(Some((a, op, b)))
}

/// `[width, height]`, neither below 0.
fn read_size(value: &Dynamic, what: &str) -> Result<(f64, f64), String> {
    let wrong = || format!("{} should be a size [width, height]", what);
    let wh = value.read_lock::<Array>().filter(|a| a.len() == 2).ok_or_else(wrong)?;
    Ok((read_number(&wh[0], what).map_err(|_| wrong())?, read_number(&wh[1], what).map_err(|_| wrong())?))
}

const LAYOUT_KEYS: [&str; 7] = ["labels", "label_place", "gap", "width", "height", "seed", "iterations"];

/// `graph::layout(graph, sizes[, #{ labels, label_place, gap, width, height, seed, iterations }])`:
/// #{ points, size, edges: [#{ start, end, label, angle, pieces }] }, the
/// nodes' boxes, of `sizes`, and each edge's line and label.
fn layout(ctx: &NativeCallContext, graph: &Map, sizes: &Array, options: &Map) -> Result<Map, Box<rhai::EvalAltResult>> {
    const CALLER: &str = "graph::layout";
    let err = |m: String| fail(ctx, CALLER, m);
    check(&Graph, &Dynamic::from_map(graph.clone())).map_err(|m| err(format!("a bad Graph: {}", m)))?;
    let o = Options::new(options, &LAYOUT_KEYS).map_err(err)?;
    let (nodes, edges) = (list_of(graph, "nodes"), list_of(graph, "edges"));
    if sizes.len() != nodes.len() {
        return Err(err(format!("there are {} sizes for {} nodes", sizes.len(), nodes.len())));
    }
    let sizes = sizes.iter().enumerate().map(|(i, s)| read_size(s, &format!("sizes[{}]", i))).collect::<Result<Vec<_>, _>>().map_err(err)?;
    let labels = match options.get("labels") {
        None => vec![None; edges.len()],
        Some(labels) => labels.read_lock::<Array>().ok_or_else(|| err("'labels' should be an array of sizes [width, height], or () for none".into()))?
            .iter().enumerate().map(|(e, l)| if l.is_unit() { Ok(None) } else { read_size(l, &format!("labels[{}]", e)).map(Some) })
            .collect::<Result<Vec<_>, _>>().map_err(err)?,
    };
    let place = match options.get("label_place") {
        None => LabelPlace::Along,
        Some(p) => LabelPlace::named(&p.clone().into_string().map_err(|_| err("'label_place' should be a string".into()))?).map_err(err)?,
    };
    let settings = GraphLayoutSettings {
        width: o.number("width", 1000.0).map_err(err)?,
        height: o.number("height", 800.0).map_err(err)?,
        gap: o.number("gap", 12.0).map_err(err)?,
        seed: o.index("seed", 0).map_err(err)? as u64,
        iterations: o.index("iterations", 300).map_err(err)?,
        place,
    };
    let ends: Vec<(usize, usize)> = edges.iter().map(|e| { let e = as_map(e); (index_of(&e, "from"), index_of(&e, "to")) }).collect();
    let laid = lay_out_graph(&sizes, &ends, &labels, &settings).map_err(err)?;
    let mut out = Map::new();
    out.insert("points".into(), Dynamic::from_array(laid.points.into_iter().map(point).collect()));
    out.insert("size".into(), Dynamic::from_array(vec![Dynamic::from_float(laid.size.0), Dynamic::from_float(laid.size.1)]));
    out.insert("edges".into(), Dynamic::from_array(laid.edges.into_iter().map(|edge| {
        let mut m = Map::new();
        m.insert("start".into(), point(edge.start));
        m.insert("end".into(), point(edge.end));
        m.insert("label".into(), point(edge.label));
        m.insert("angle".into(), Dynamic::from_float(edge.angle));
        m.insert("pieces".into(), Dynamic::from_array(edge.pieces.into_iter().map(|(a, b)| Dynamic::from_array(vec![point(a), point(b)])).collect()));
        Dynamic::from_map(m)
    }).collect()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTES: &str = "\
# where the ways go
Paper Mazes: basic_eye archangel_3
Saint's Rest: ploughdiem

Paper Mazes -> Saint's Rest: Follow the red thread until you find
  _To Fall Endlessly_, then use the crawlspace beneath the broken shelf
Saint's Rest -- The Cloister
The Cloister -> Paper Mazes: through the
    reading room
";

    fn nodes(graph: &Map) -> Vec<(String, Vec<String>)> {
        list_of(graph, "nodes").iter().map(|n| {
            let n = as_map(n);
            (text_of(&n, "name"), list_of(&n, "glyphs").iter().map(|g| g.clone().into_string().unwrap()).collect())
        }).collect()
    }

    fn edges(graph: &Map) -> Vec<(usize, usize, bool, String)> {
        list_of(graph, "edges").iter().map(|e| {
            let e = as_map(e);
            (index_of(&e, "from"), index_of(&e, "to"), field(&e, "directed").as_bool().unwrap(), text_of(&e, "text"))
        }).collect()
    }

    #[test]
    fn a_graph_file_reads_as_its_places_and_ways() {
        let graph = parse(ROUTES).unwrap();
        check(&Graph, &Dynamic::from_map(graph.clone())).unwrap();
        let strings = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(nodes(&graph), vec![
            ("Paper Mazes".into(), strings(&["basic_eye", "archangel_3"])),
            ("Saint's Rest".into(), strings(&["ploughdiem"])),
            ("The Cloister".into(), vec![]),
        ]);
        assert_eq!(edges(&graph), vec![
            (0, 1, true, "Follow the red thread until you find _To Fall Endlessly_, then use the crawlspace beneath the broken shelf".into()),
            (1, 2, false, String::new()),
            (2, 0, true, "through the reading room".into()),
        ]);
    }

    #[test]
    fn nodes_come_in_the_order_theyre_first_named() {
        let graph = parse("B -> A\nC\nA: basic_eye").unwrap();
        assert_eq!(nodes(&graph), vec![("B".into(), vec![]), ("A".into(), vec!["basic_eye".to_string()]), ("C".into(), vec![])]);
    }

    #[test]
    fn a_graph_file_that_cant_be_read_says_which_line() {
        for (text, want) in [
            ("A\nB\nA: basic_eye", "line 3: A has a line already, line 1"),
            ("A -> A", "line 1: A -> A joins A to itself"),
            ("\n: basic_eye", "line 2: a line names a place, or joins two with -> or --"),
            ("  carried on", "line 1: an indented line carries on the line above, and there's none"),
            ("A ->B", "line 1: put a space either side of ->"),
            ("A --", "line 1: an edge needs a place either side of --"),
            ("A -> B -> C", "line 1: an edge joins two places: give each way a line of its own"),
            ("A -> B: [far]{color: }", "line 1: "),
        ] {
            let err = parse(text).err().unwrap_or_default();
            assert!(err.starts_with(want), "{:?}: {}", text, err);
        }
    }

    #[test]
    fn graphs_are_checked() {
        let graph = |nodes: &str, edges: &str| -> Dynamic {
            let engine = rhai::Engine::new();
            engine.eval::<Dynamic>(&format!("#{{ kind: \"Graph\", nodes: [{}], edges: [{}] }}", nodes, edges)).unwrap()
        };
        let node = |n: &str| format!("#{{ name: \"{}\", glyphs: [] }}", n);
        let edge = |a: i64, b: i64| format!("#{{ from: {}, to: {}, directed: true, text: \"\" }}", a, b);
        let two = format!("{}, {}", node("A"), node("B"));
        assert!(check(&Graph, &graph(&two, &edge(0, 1))).is_ok());
        for (g, want) in [
            (graph(&format!("{}, {}", node("A"), node("A")), ""), "there are two nodes named A"),
            (graph(&node(" "), ""), "nodes[0] needs a name"),
            (graph(&two, &edge(0, 2)), "edges[0] should join two of the 2 nodes"),
            (graph(&two, &edge(1, 1)), "edges[0] joins a node to itself"),
        ] {
            let err = check(&Graph, &g).unwrap_err();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    fn engine() -> rhai::Engine {
        let mut engine = rhai::Engine::new();
        super::super::register(&mut engine, |_, _| {});
        engine
    }

    #[test]
    fn the_layout_kernel_gives_points_and_lines() {
        let script = format!(
            "let g = graph::parse({:?}); graph::layout(g, [[180, 60], [160, 60], [140, 60]], #{{ labels: [[300, 60], (), [150, 30]], label_place: \"beside\", seed: 2 }})",
            ROUTES,
        );
        let laid = engine().eval::<Map>(&script).unwrap();
        assert_eq!(list_of(&laid, "points").len(), 3);
        let edges = list_of(&laid, "edges");
        assert_eq!(edges.len(), 3);
        for e in &edges {
            let e = as_map(e);
            assert!(["start", "end", "label", "angle", "pieces"].iter().all(|k| e.contains_key(*k)), "{:?}", e);
            assert_eq!(list_of(&e, "pieces").len(), 1, "beside lines don't break");
        }
    }

    #[test]
    fn the_layout_kernel_says_whats_wrong() {
        for (call, want) in [
            ("graph::layout(g, [[1, 1]])", "graph::layout: there are 1 sizes for 2 nodes"),
            ("graph::layout(g, [[1, 1], [1]])", "graph::layout: sizes[1] should be a size [width, height]"),
            ("graph::layout(g, [[1, 1], [1, 1]], #{ labels: [] })", "graph::layout: there are 0 labels for 1 edges"),
            ("graph::layout(g, [[1, 1], [1, 1]], #{ label_place: \"over\" })", "graph::layout: unknown label_place 'over'"),
            ("graph::layout(g, [[1, 1], [1, 1]], #{ spacing: 3 })", "graph::layout: unknown option 'spacing'"),
            ("graph::layout(#{ kind: \"Graph\" }, [])", "graph::layout: a bad Graph: "),
        ] {
            let err = engine().eval::<Map>(&format!("let g = graph::parse(\"A -> B\"); {}", call)).unwrap_err().to_string();
            assert!(err.contains(want), "{}: {}", call, err);
        }
    }

    #[test]
    fn its_sketch_boxes_the_names_and_writes_the_ways_across_their_lines() {
        let sketch = super::super::sketch(&Dynamic::from_map(parse(ROUTES).unwrap())).unwrap().unwrap();
        let names: Vec<&str> = sketch.parts.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["boxes", "glyphs", "lines", "arrows"]);
        let words: Vec<&str> = sketch.labels.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(&words[..3], ["Paper Mazes", "Saint's Rest", "The Cloister"]);
        assert!(words.join(" ").contains("find To Fall Endlessly, then") && !words.iter().any(|w| w.contains('_')), "the marks come out: {:?}", words);
        assert!(words.iter().all(|w| w.chars().count() <= 24), "{:?}", words);
    }

    #[test]
    fn its_example_is_a_graph() {
        let value = engine().eval::<Dynamic>(Graph.example().unwrap()).unwrap();
        check(&Graph, &value).unwrap();
    }
}
