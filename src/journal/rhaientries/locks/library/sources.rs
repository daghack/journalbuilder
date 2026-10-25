//! A chain's sources: the journal's resources a chain draws on, each a node
//! with one output, `ID.value`. In a chain file:
//!
//! ```rhai
//! sources: #{
//!     raven: #{ type: "content", name: "raven", base: 27 },     // symbols, or text without base
//!     hand: #{ type: "style", name: "plain_hand", options: #{ size: 30.0 } },
//!     copper: #{ type: "glyphset", name: "copper" },
//!     page: #{ type: "layout", name: "two_column_article", options: #{} },
//! },
//! ```
//!
//! Content is data, so it can feed a lock's input as well as an
//! illustration's: a `.json` file's symbols, in its own base; a `.txt` or
//! `.md` file's text as symbols in `base` (what doesn't fit dropped), or as
//! text (a String) without one. Styles, glyphsets and layouts are things to
//! draw with, so only illustrations take them, at inputs typed TextStyle,
//! Glyphset or Layout; they're made in the journal's engine when the chain
//! draws (`text_style(name, options)`, `glyphset(name)`, `layout(name,
//! options)`).

use super::LockLibrary;
use crate::lockbuilder::{base_n_symbols, InfoVar, InfoVarType};
use rhai::{Array, Dynamic, Map};

/// What a source draws on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Content,
    Style,
    Glyphset,
    Layout,
}

impl SourceKind {
    pub fn name(self) -> &'static str {
        match self {
            SourceKind::Content => "content",
            SourceKind::Style => "style",
            SourceKind::Glyphset => "glyphset",
            SourceKind::Layout => "layout",
        }
    }

    fn parse(name: &str) -> Option<SourceKind> {
        [SourceKind::Content, SourceKind::Style, SourceKind::Glyphset, SourceKind::Layout].into_iter().find(|kind| kind.name() == name)
    }
}

/// A source of a chain: its id, what it draws on, and how.
#[derive(Clone, Debug)]
pub struct SourceSpec {
    pub id: String,
    pub kind: SourceKind,
    /// The file's name: content/NAME.*, styles/NAME.rhai, glyphsets/NAME.json, layouts/NAME.rhai.
    pub name: String,
    /// A style's or layout's options.
    pub options: Map,
    /// Text content as symbols in this base.
    pub base: Option<u32>,
}

/// The port a source's value comes out of.
pub const SOURCE_PORT: &str = "value";

/// A chain file's `sources`, checked for their shape.
pub fn parse_sources(spec: &Map) -> Result<Vec<SourceSpec>, String> {
    let Some(sources) = spec.get("sources") else { return Ok(Vec::new()) };
    let sources = sources.clone().try_cast::<Map>().ok_or("sources should be a map of ids to #{ type, name, ... }")?;
    sources.into_iter().map(|(id, entry)| {
        let bad = |what: &str| format!("sources: '{}' {}", id, what);
        if id.is_empty() || id.contains('.') {
            return Err(format!("sources: '{}' can't be a source's id; ids must be non-empty, without '.'", id));
        }
        let entry = entry.try_cast::<Map>().ok_or_else(|| bad("should be a map, #{ type, name }"))?;
        let kind_name = entry.get("type").and_then(|t| t.clone().into_string().ok())
            .ok_or_else(|| bad("needs its type: content, style, glyphset or layout"))?;
        let kind = SourceKind::parse(&kind_name).ok_or_else(|| bad(&format!("has type '{}'; a source is content, style, glyphset or layout", kind_name)))?;
        let allowed: &[&str] = match kind {
            SourceKind::Content => &["type", "name", "base"],
            SourceKind::Style | SourceKind::Layout => &["type", "name", "options"],
            SourceKind::Glyphset => &["type", "name"],
        };
        if let Some(unknown) = entry.keys().find(|key| !allowed.contains(&key.as_str())) {
            return Err(bad(&format!("has '{}'; a {} source takes {}", unknown, kind.name(), allowed.join(", "))));
        }
        let name = entry.get("name").and_then(|n| n.clone().into_string().ok()).ok_or_else(|| bad("needs the name of the file it draws on"))?;
        let options = match entry.get("options") {
            None => Map::new(),
            Some(options) => options.clone().try_cast::<Map>().ok_or_else(|| bad("has options that aren't a map"))?,
        };
        let base = match entry.get("base") {
            None => None,
            Some(base) => Some(base.as_int().ok().and_then(|b| u32::try_from(b).ok()).filter(|b| (2..=27).contains(b))
                .ok_or_else(|| bad(&format!("has base {}; a base is a whole number from 2 to 27", base)))?),
        };
        Ok(SourceSpec { id: id.to_string(), kind, name, options, base })
    }).collect()
}

/// Content's text, without the newline ending its file.
fn text(mut text: String) -> String {
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    text
}

impl LockLibrary {
    /// The type of `source`'s value: symbols or String for content,
    /// TextStyle, Glyphset or Layout for the rest. Fails if its file isn't
    /// there.
    pub fn source_type(&self, source: &SourceSpec) -> Result<InfoVarType, String> {
        let at = |e: String| format!("sources: '{}': {}", source.id, e);
        let resources = self.resources.as_ref().ok_or_else(|| at("there are no journal files here to draw on".to_string()))?;
        let missing = |what: &str, names: Vec<String>| at(format!("there's no {} named '{}'; {}: {}", what, source.name, what,
            if names.is_empty() { "none".to_string() } else { names.join(", ") }));
        match source.kind {
            SourceKind::Content => {
                if source.base.is_some() {
                    self.content(source).map_err(at)?;
                    return Ok(InfoVarType::BaseNSymbol(source.base.unwrap()));
                }
                Ok(self.source_value(source)?.var_type)
            }
            SourceKind::Style if resources.styles.has(&source.name) => Ok(InfoVarType::Named("TextStyle".to_string())),
            SourceKind::Style => Err(missing("style", resources.styles.names().unwrap_or_default())),
            SourceKind::Glyphset if resources.glyphsets.has(&source.name) => Ok(InfoVarType::Named("Glyphset".to_string())),
            SourceKind::Glyphset => Err(missing("glyphset", resources.glyphsets.names().unwrap_or_default())),
            SourceKind::Layout if resources.layouts.has(&source.name) => Ok(InfoVarType::Named("Layout".to_string())),
            SourceKind::Layout => Err(missing("layout", resources.layouts.names().unwrap_or_default())),
        }
    }

    /// Content `source`'s file: its extension and text.
    fn content(&self, source: &SourceSpec) -> Result<(&'static str, String), String> {
        let resources = self.resources.as_ref().ok_or("there are no journal files here to draw on")?;
        let kinds = resources.content_kinds(&source.name);
        let sources = match kinds.as_slice() {
            ["txt"] => &resources.text,
            ["md"] => &resources.markdown,
            ["json"] => &resources.symbols,
            [] => {
                let names = resources.content_names();
                return Err(format!("there's no content named '{}'; content: {}", source.name, if names.is_empty() { "none".to_string() } else { names.join(", ") }));
            }
            _ => return Err(format!("content '{}' is ambiguous: there's more than one file of that name", source.name)),
        };
        let read = sources.read(&source.name).map_err(|e| format!("couldn't read {}: {}", sources.path(&source.name).display(), e))?;
        Ok((kinds[0], text(read)))
    }

    /// Content `source`'s value, as lock files see values: symbols as an
    /// array of whole numbers, text as a string. Only content is data: the
    /// other sources fail, being things only illustrations draw with.
    pub fn source_value(&self, source: &SourceSpec) -> Result<InfoVar, String> {
        let at = |e: String| format!("sources: '{}': {}", source.id, e);
        if source.kind != SourceKind::Content {
            return Err(at(format!("a {} is something to draw with, so only an illustration's input can take it", source.kind.name())));
        }
        let (ext, text) = self.content(source).map_err(at)?;
        let symbols = |base: u32, symbols: Vec<u32>| {
            let data: Array = symbols.into_iter().map(|s| Dynamic::from_int(s as i64)).collect();
            InfoVar::new(InfoVarType::BaseNSymbol(base), data.into())
        };
        match (ext, source.base) {
            ("json", Some(_)) => Err(at(format!("content/{}.json is symbols in its own base already: give text content a base", source.name))),
            ("json", None) => {
                let file: serde_json::Value = serde_json::from_str(&text).map_err(|e| at(format!("content/{}.json: {}", source.name, e)))?;
                let base = file["base"].as_u64().and_then(|b| u32::try_from(b).ok()).filter(|b| (2..=27).contains(b))
                    .ok_or_else(|| at(format!("content/{}.json should be {{ \"base\": N, \"symbols\": [...] }}", source.name)))?;
                let list = file["symbols"].as_array().ok_or_else(|| at(format!("content/{}.json has no symbols", source.name)))?;
                let values = list.iter().map(|s| s.as_u64().and_then(|s| u32::try_from(s).ok()).filter(|s| *s < base))
                    .collect::<Option<Vec<u32>>>()
                    .ok_or_else(|| at(format!("content/{}.json has a symbol outside base {}'s 0-{}", source.name, base, base - 1)))?;
                Ok(symbols(base, values))
            }
            (_, Some(base)) => {
                let fitted = super::super::lenient_text(base, &text);
                Ok(symbols(base, base_n_symbols(base, &fitted).map_err(at)?))
            }
            (_, None) => Ok(InfoVar::new(InfoVarType::Named("String".to_string()), text.into())),
        }
    }
}
