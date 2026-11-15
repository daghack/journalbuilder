//! Brushes: Darkly brush graphs, kept in Darkly's own format, the YAML its
//! brush builder copies and its built-in brushes are kept in
//! (`PortableBrush`). A journal's are its `brushes/NAME.yaml`, in front of
//! the library's: Darkly's built-ins, and the library's own. A brush is called by the
//! `name` its file gives, as Darkly calls it ("Ink Pen"), or by its file's
//! name if it gives none.
//!
//! Strokes carry the brush they're drawn in (`BrushGraph`), read and checked
//! when a style names it, so a canvas renders without the files it came from.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use darkly::brush::portable::PortableBrush;
use darkly::brush::wire::BrushWireType;
use darkly::gpu::params::PortableValue;
use darkly::nodegraph::{NodeId, PortDir};

/// The node every brush has, whose `size` and `stabilize` strokes set.
pub const SETTINGS_NODE: &str = "brush_settings";

/// A brush to draw with: its name, and its node graph as Darkly loads one.
/// Two are the same brush if they're of the same name and file.
#[derive(Debug)]
pub struct BrushGraph {
    name: String,
    /// The file it was read from.
    source: String,
    /// The graph as JSON, for `DarklyEngine::set_brush_graph`. (Its nodes
    /// are in no particular order.)
    graph: String,
}

impl PartialEq for BrushGraph {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.source == other.source
    }
}

impl BrushGraph {
    /// The brush in a brush file's text, called `name` if the file doesn't
    /// say what it's called; or what's wrong with it: it isn't Darkly's
    /// format, its graph doesn't compile, or it has no brush settings for
    /// strokes to set its size with.
    pub fn parse(yaml: &str, name: &str) -> Result<BrushGraph, String> {
        let portable: PortableBrush = serde_yaml_ng::from_str(yaml).map_err(|e| format!("it isn't a Darkly brush: {}", e))?;
        let name = match portable.name.trim() {
            "" => name.to_string(),
            given => given.to_string(),
        };
        let graph = portable.into_graph(darkly::brush::registry())?;
        // Darkly names a graph's first node of each type after its type.
        let settings = graph.nodes().get(&NodeId(SETTINGS_NODE.into()))
            .ok_or_else(|| format!("it has no {} node, which strokes set its size with", SETTINGS_NODE))?;
        for port in ["size", "stabilize"] {
            if !settings.ports.iter().any(|p| p.name == port && p.dir == PortDir::Input) {
                return Err(format!("its {} node has no '{}'", SETTINGS_NODE, port));
            }
        }
        let graph = serde_json::to_string(&graph).map_err(|e| format!("its graph can't be written out: {}", e))?;
        Ok(BrushGraph { name, source: yaml.to_string(), graph })
    }

    /// What it's called.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its graph, as JSON, for `DarklyEngine::set_brush_graph`.
    pub fn graph(&self) -> &str {
        &self.graph
    }
}

/// The name a brush file gives its brush, if it gives one; None if it
/// isn't YAML.
pub fn file_name(yaml: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Named {
        #[serde(default)]
        name: String,
    }
    serde_yaml_ng::from_str::<Named>(yaml).ok().map(|named| named.name.trim().to_string())
}

/// Every type of node brushes are made of, for the brush editor: Darkly's
/// registrations, sorted by type, each input's `value` as brush files
/// write it (`[[0, 0], [1, 1]]` for a curve, `true` for a toggle).
pub fn node_types_json() -> serde_json::Value {
    let types = darkly::brush::registry().types().into_iter().map(|reg| {
        let mut node = serde_json::to_value(&reg.node).expect("a node registration serializes");
        if let Some(ports) = node.get_mut("ports").and_then(|p| p.as_array_mut()) {
            for (json, port) in ports.iter_mut().zip(&reg.node.ports) {
                json["value"] = serde_json::to_value(port.value.to_portable()).expect("a value serializes");
            }
        }
        node
    }).collect();
    serde_json::Value::Array(types)
}

/// A brush file's text as JSON of the same shape, for the brush editor
/// (its exposed ports in the file's order); or why it isn't a brush file.
pub fn file_to_json(yaml: &str) -> Result<String, String> {
    let portable: PortableBrush = serde_yaml_ng::from_str(yaml).map_err(|e| format!("it isn't a Darkly brush: {}", e))?;
    serde_json::to_string(&portable).map_err(|e| e.to_string())
}

/// JSON of a brush file's shape (as `file_to_json` gives) written as a
/// brush file, as Darkly writes them: each input's value of the type its
/// port takes (JSON has no `1.0`), and the connections in order.
pub fn json_to_file(json: &str) -> Result<String, String> {
    let mut portable: PortableBrush = serde_json::from_str(json).map_err(|e| format!("not a brush: {}", e))?;
    let registry = darkly::brush::registry();
    for (id, node) in portable.nodes.iter_mut() {
        let reg = registry.get(&node.type_id).ok_or_else(|| format!("node '{}' is of no type Darkly has, '{}'", id, node.type_id))?;
        for (name, value) in node.inputs.iter_mut() {
            let port = reg.ports.iter().find(|p| p.name == *name && p.dir == PortDir::Input)
                .ok_or_else(|| format!("node '{}' ({}) has no input '{}'", id, node.type_id, name))?;
            // As the port's type, the way Darkly writes it; but not through
            // Darkly's f32s, which would write 0.1 as 0.10000000149011612.
            *value = match (port.wire_type, value.clone()) {
                (BrushWireType::Scalar, PortableValue::Int(i)) => PortableValue::Float(i as f64),
                (BrushWireType::Int | BrushWireType::Enum, PortableValue::Float(f)) if f.fract() == 0.0 => PortableValue::Int(f as i64),
                (_, value) => value,
            };
            darkly::brush::input_value::InputValue::from_portable(port.wire_type, value.clone())
                .map_err(|m| format!("input '{}' of node '{}': expected {}, got {}", name, id, m.expected, m.actual))?;
        }
    }
    portable.connections.sort_by(|a, b| (&a.from_node, &a.from_port, &a.to_node, &a.to_port).cmp(&(&b.from_node, &b.from_port, &b.to_node, &b.to_port)));
    serde_yaml_ng::to_string(&portable).map_err(|e| e.to_string())
}

/// Darkly's built-in brushes, which the library has files of, by name: what
/// styles draw with when no journal's brushes are at hand.
pub fn builtins() -> &'static BTreeMap<String, Arc<BrushGraph>> {
    static BUILTINS: OnceLock<BTreeMap<String, Arc<BrushGraph>>> = OnceLock::new();
    BUILTINS.get_or_init(|| {
        darkly::brush::builtin_brushes::BUILTIN_BRUSHES_YAML.iter().map(|(file, yaml)| {
            let stem = file.strip_suffix(".yaml").unwrap_or(file);
            let brush = BrushGraph::parse(yaml, stem).unwrap_or_else(|e| panic!("Darkly's brush {}: {}", file, e));
            (brush.name.clone(), Arc::new(brush))
        }).collect()
    })
}

/// Darkly's built-in brush `name`.
pub fn builtin(name: &str) -> Option<Arc<BrushGraph>> {
    builtins().get(name).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn darklys_brushes_parse() {
        let names: Vec<&str> = builtins().keys().map(String::as_str).collect();
        assert!(names.contains(&"Ink Pen") && names.contains(&"Rough Watercolor"), "{:?}", names);
        assert_eq!(builtin("Ink Pen").unwrap().name(), "Ink Pen");
    }

    #[test]
    fn a_brush_without_a_name_is_called_by_its_file() {
        let yaml = darkly::brush::builtin_brushes::BUILTIN_BRUSHES_YAML.iter().find(|(f, _)| *f == "ink_pen.yaml").unwrap().1;
        let unnamed: String = yaml.lines().filter(|l| !l.starts_with("name:")).collect::<Vec<_>>().join("\n");
        assert_eq!(BrushGraph::parse(&unnamed, "quill").unwrap().name(), "quill");
        assert_eq!(file_name(&unnamed).as_deref(), Some(""));
        assert_eq!(file_name(yaml).as_deref(), Some("Ink Pen"));
    }

    /// What the editor reads and writes back is the file, for files Darkly
    /// wrote (its hand-written ones lose their comments and order).
    #[test]
    fn brush_files_go_through_json_and_back() {
        for file in ["ink_pen.yaml", "airbrush.yaml", "rough_ink.yaml"] {
            let yaml = darkly::brush::builtin_brushes::BUILTIN_BRUSHES_YAML.iter().find(|(f, _)| *f == file).unwrap().1;
            let json = file_to_json(yaml).unwrap();
            assert_eq!(json_to_file(&json).unwrap(), yaml, "{}", file);
        }
        // JSON's 1 is a float for a scalar input, and an index for an enum's.
        let json = r#"{"nodes":{"brush_settings":{"type":"brush_settings"},"circle":{"type":"circle","inputs":{"softness":1,"algorithm":2}}},
            "connections":["pen_input.pressure -> paint.size","circle.mask -> stamp.tip"]}"#;
        let yaml = json_to_file(json).unwrap();
        assert!(yaml.contains("softness: 1.0") && yaml.contains("algorithm: 2"), "{}", yaml);
        assert!(yaml.find("circle.mask").unwrap() < yaml.find("pen_input.pressure").unwrap(), "{}", yaml);
        assert!(json_to_file(r#"{"nodes":{"c":{"type":"circle","inputs":{"softness":"soft"}}}}"#).unwrap_err().contains("softness"));
        let types = node_types_json();
        let curve = types.as_array().unwrap().iter().find(|t| t["type_id"] == "curve").unwrap();
        assert_eq!(curve["ports"][1]["value"], serde_json::json!([[0.0, 0.0], [1.0, 1.0]]));
    }

    #[test]
    fn bad_brushes_say_why() {
        assert!(BrushGraph::parse("nodes: [", "x").unwrap_err().contains("isn't a Darkly brush"));
        let err = BrushGraph::parse("nodes:\n  pen_input:\n    type: pen_input\n  paint:\n    type: paint\n", "x").unwrap_err();
        assert!(err.contains("brush_settings"), "{}", err);
        let err = BrushGraph::parse("nodes:\n  x:\n    type: crayon\n", "x").unwrap_err();
        assert!(err.contains("crayon"), "{}", err);
    }
}
