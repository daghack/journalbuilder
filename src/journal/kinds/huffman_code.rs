//! HuffmanCode: an M-ary Huffman code as its tree.
//!
//! ```rhai
//! #{ kind: "HuffmanCode", input_base: 27, output_base: 3,
//!    nodes: [#{ parent: (), digit: (), symbol: () }, #{ parent: 0, digit: 0, symbol: 5 }, ...] }
//! ```
//!
//! The nodes are the root first, then breadth first, each node's children
//! in the order of their digits; a leaf's `symbol` is the input symbol its
//! path from the root encodes. `huffman::code(symbols, input_base,
//! output_base)` builds one as the `huffman` lock builds its code.

use rhai::{Array, Dynamic, Map, Module, NativeCallContext, INT};

use super::schema::{list, optional, record, Schema};
use super::{fail, field, read_index, read_optional_index, Label, Options, Sketch, ValueKind};
use crate::journal::art::huffman_tree;
use crate::journal::paths::StrokePath;
use crate::lockbuilder::symbols_to_string;

pub struct HuffmanCode;

/// A code's tree: each node's parent, digit and symbol.
pub struct Tree {
    pub input_base: u32,
    pub output_base: u32,
    pub parent: Vec<Option<usize>>,
    pub digit: Vec<Option<u32>>,
    pub symbol: Vec<Option<u32>>,
}

impl Tree {
    /// A checked HuffmanCode's tree.
    pub fn read(code: &Map) -> Result<Tree, String> {
        let base = |key: &str| read_index(field(code, key), key).map(|b| b as u32);
        let nodes = field(code, "nodes").read_lock::<Array>().ok_or("nodes should be an array")?.clone();
        let mut tree = Tree { input_base: base("input_base")?, output_base: base("output_base")?, parent: Vec::new(), digit: Vec::new(), symbol: Vec::new() };
        for (i, node) in nodes.iter().enumerate() {
            let node = node.read_lock::<Map>().ok_or_else(|| format!("nodes[{}] should be a map", i))?;
            let at = |key: &str| format!("nodes[{}].{}", i, key);
            tree.parent.push(read_optional_index(field(&node, "parent"), &at("parent"))?);
            tree.digit.push(read_optional_index(field(&node, "digit"), &at("digit"))?.map(|d| d as u32));
            tree.symbol.push(read_optional_index(field(&node, "symbol"), &at("symbol"))?.map(|s| s as u32));
        }
        Ok(tree)
    }

    /// Each node's children, in digit order.
    pub fn children(&self) -> Vec<Vec<usize>> {
        let mut children = vec![Vec::new(); self.parent.len()];
        for (i, p) in self.parent.iter().enumerate() {
            if let Some(p) = *p {
                children[p].push(i);
            }
        }
        children
    }

    /// The tree as a HuffmanCode value.
    pub fn value(&self) -> Map {
        let opt = |v: Option<usize>| v.map_or(Dynamic::UNIT, |v| Dynamic::from_int(v as INT));
        let nodes: Array = (0..self.parent.len()).map(|i| {
            let mut node = Map::new();
            node.insert("parent".into(), opt(self.parent[i]));
            node.insert("digit".into(), opt(self.digit[i].map(|d| d as usize)));
            node.insert("symbol".into(), opt(self.symbol[i].map(|s| s as usize)));
            Dynamic::from_map(node)
        }).collect();
        let mut code = Map::new();
        code.insert("kind".into(), "HuffmanCode".into());
        code.insert("input_base".into(), Dynamic::from_int(self.input_base as INT));
        code.insert("output_base".into(), Dynamic::from_int(self.output_base as INT));
        code.insert("nodes".into(), Dynamic::from_array(nodes));
        code
    }
}

impl ValueKind for HuffmanCode {
    fn name(&self) -> &'static str {
        "HuffmanCode"
    }

    fn module(&self) -> Option<&'static str> {
        Some("huffman")
    }

    fn schema(&self) -> Schema {
        record(&[
            ("kind", Schema::Kind("HuffmanCode")),
            ("input_base", Schema::Int),
            ("output_base", Schema::Int),
            ("nodes", list(record(&[("parent", optional(Schema::Int)), ("digit", optional(Schema::Int)), ("symbol", optional(Schema::Int))]))),
        ])
    }

    fn validate(&self, value: &Map) -> Result<(), String> {
        let tree = Tree::read(value)?;
        for (name, base) in [("input_base", tree.input_base), ("output_base", tree.output_base)] {
            if !(2..=27).contains(&base) {
                return Err(format!("{} must be from 2 to 27, got {}", name, base));
            }
        }
        let n = tree.parent.len();
        if n < 2 {
            return Err("a code needs a root and at least one leaf".into());
        }
        if tree.parent[0].is_some() || tree.digit[0].is_some() {
            return Err("nodes[0] is the root: its parent and digit should be ()".into());
        }
        let children = tree.children();
        let mut seen = vec![false; tree.input_base as usize];
        for i in 1..n {
            let parent = tree.parent[i].ok_or_else(|| format!("nodes[{}] needs a parent: only the root has none", i))?;
            let digit = tree.digit[i].ok_or_else(|| format!("nodes[{}] needs a digit", i))?;
            if digit >= tree.output_base {
                return Err(format!("nodes[{}].digit is {}, outside base {}", i, digit, tree.output_base));
            }
            // Breadth first, children in digit order: parents never go back,
            // and siblings' digits go up.
            let (last_parent, last_digit) = (tree.parent[i - 1], tree.digit[i - 1]);
            if parent >= i || last_parent.is_some_and(|p| parent < p) || (last_parent == Some(parent) && last_digit.is_some_and(|d| digit <= d)) {
                return Err(format!("nodes[{}] is out of order: nodes go root first, then breadth first, each node's children in digit order", i));
            }
        }
        for i in 0..n {
            match (children[i].is_empty(), tree.symbol[i]) {
                (true, None) => return Err(format!("nodes[{}] is a leaf, so it needs a symbol", i)),
                (false, Some(_)) => return Err(format!("nodes[{}] has children, so it can't have a symbol", i)),
                (true, Some(s)) if s >= tree.input_base => return Err(format!("nodes[{}].symbol is {}, outside base {}", i, s, tree.input_base)),
                (true, Some(s)) if std::mem::replace(&mut seen[s as usize], true) => return Err(format!("symbol {} has two leaves", s)),
                _ => {}
            }
        }
        Ok(())
    }

    fn example(&self) -> Option<&'static str> {
        Some("huffman::code(symbols(27, \"THE QUICK BROWN FOX\"), 27, 3)")
    }

    fn kernels(&self, module: &mut Module) {
        module.set_native_fn("code", |ctx: NativeCallContext, symbols: Array, input_base: INT, output_base: INT| -> Result<Map, Box<rhai::EvalAltResult>> {
            code(&ctx, &symbols, input_base, output_base, &Map::new())
        });
        module.set_native_fn("code", |ctx: NativeCallContext, symbols: Array, input_base: INT, output_base: INT, options: Map| -> Result<Map, Box<rhai::EvalAltResult>> {
            code(&ctx, &symbols, input_base, output_base, &options)
        });
    }

    /// The tree drawn top down: leaves spread out in digit order, each
    /// branch over the middle of its children, and each leaf's symbol
    /// under it.
    fn sketch(&self, value: &Map) -> Option<Sketch> {
        let tree = Tree::read(value).ok()?;
        let children = tree.children();
        let n = tree.parent.len();
        let mut depth = vec![0f32; n];
        for i in 1..n {
            depth[i] = depth[tree.parent[i]?] + 1.0;
        }
        // Leaves left to right in a walk that takes children in digit order.
        let mut x = vec![0f32; n];
        let mut next_leaf = 0.0;
        let mut stack = vec![(0usize, false)];
        while let Some((v, done)) = stack.pop() {
            if children[v].is_empty() {
                x[v] = next_leaf;
                next_leaf += 1.0;
            } else if done {
                x[v] = children[v].iter().map(|&c| x[c]).sum::<f32>() / children[v].len() as f32;
            } else {
                stack.push((v, true));
                stack.extend(children[v].iter().rev().map(|&c| (c, false)));
            }
        }
        let at = |i: usize| parry2d::math::Vec2::new(x[i], depth[i] * 1.6);
        let mut links = StrokePath::new();
        let mut nodes = StrokePath::new();
        let mut labels = Vec::new();
        for i in 0..n {
            nodes.append(StrokePath::circle(at(i), 0.15));
            if let Some(p) = tree.parent[i] {
                links.append(StrokePath::line(at(p), at(i)));
            }
            if let Some(s) = tree.symbol[i] {
                let text = symbols_to_string(&[s], tree.input_base);
                labels.push(Label { at: at(i) + parry2d::math::Vec2::new(0.0, 0.7), text: if text == " " { "·".into() } else { text } });
            }
        }
        Some(Sketch { parts: vec![("links".into(), links), ("nodes".into(), nodes)], labels, label_size: 0.6 })
    }
}

/// `huffman::code(symbols, input_base, output_base[, #{ smoothing }])`.
fn code(ctx: &NativeCallContext, symbols: &Array, input_base: INT, output_base: INT, options: &Map) -> Result<Map, Box<rhai::EvalAltResult>> {
    const CALLER: &str = "huffman::code";
    let err = |m: String| fail(ctx, CALLER, m);
    let options = Options::new(options, &["smoothing"]).map_err(err)?;
    let smoothing = options.index("smoothing", 1).map_err(err)? as u32;
    let base = |b: INT, what: &str| u32::try_from(b).ok().filter(|b| (2..=27).contains(b))
        .ok_or_else(|| err(format!("{} must be from 2 to 27, got {}", what, b)));
    let (input_base, output_base) = (base(input_base, "input_base")?, base(output_base, "output_base")?);
    let symbols = symbols.iter().enumerate()
        .map(|(i, s)| read_index(s, &format!("symbol {}", i)).map(|s| s as u32))
        .collect::<Result<Vec<u32>, String>>().map_err(err)?;
    build(&symbols, input_base, output_base, smoothing).map_err(err)
}

/// The HuffmanCode of `symbols`, as the huffman lock builds its code.
pub fn build(symbols: &[u32], input_base: u32, output_base: u32, smoothing: u32) -> Result<Map, String> {
    let stars = huffman_tree(symbols, input_base, output_base, smoothing)?;
    let tree = Tree {
        input_base,
        output_base,
        parent: stars.iter().map(|s| s.parent).collect(),
        digit: stars.iter().map(|s| s.code.last().copied()).collect(),
        symbol: stars.iter().map(|s| s.symbol).collect(),
    };
    Ok(tree.value())
}
