use super::paths::StrokePath;
use super::symbol::{Symbol, SymbolBase, SymbolType};

use parry2d::math::Vec2;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::fs::File;
use std::io::Read;
use std::io;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
enum AdjType {
    ZeroToOneRemap,
}

#[derive(Serialize, Deserialize, Debug)]
struct SymbolEntry {
    name: String,
    path: String,
    adj: Option<AdjType>,
}

#[derive(Clone, Debug)]
pub struct SymbolPathDict {
    symbols: HashMap<String, StrokePath>,
    symbol_base: HashMap<String, Rc<SymbolBase>>,
    /// Characters drawn with the fallback glyph because no glyph was found.
    missing: RefCell<BTreeMap<char, MissingGlyph>>,
}

/// The glyph drawn for a character a font has no glyph for.
pub const FALLBACK_GLYPH: &str = "X";

/// A character that had no glyph and was drawn with the fallback instead.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MissingGlyph {
    /// Glyph names that were looked up for it, in order.
    pub tried: Vec<String>,
    /// How many times it was drawn with the fallback.
    pub count: usize,
}

impl SymbolPathDict {
    pub fn from_file(file: &str) -> Result<SymbolPathDict, io::Error> {
        // read the json file and parse it into a Vec<SymbolEntry>
        let mut file = File::open(file)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Self::from_bytes(&bytes)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<SymbolPathDict, io::Error> {
        // read the json file and parse it into a Vec<SymbolEntry>
        let symbols: Vec<SymbolEntry> = serde_json::from_slice(bytes)?;
        let mut symbol_map = HashMap::new();
        let mut symbol_base = HashMap::new();
        for symbol in symbols {
            let mut stroke_path: StrokePath = symbol.path.parse()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("symbol '{}': {}", symbol.name, e)))?;
            match symbol.adj {
                Some(AdjType::ZeroToOneRemap) => {
                    // This is for when the incoming path is in a 0-1 coordinate space.
                    // Since our paths are in a -1 to 1 coordinate space, we need to scale and translate.
                    // (This is currently used to automatically convert older symbol paths to the new format)
                    stroke_path = stroke_path.scaled_by(2.0).translated_by(Vec2::new(-1.0, -1.0));
                }
                None => {}
            }
            symbol_map.insert(symbol.name.clone(), stroke_path.clone());
            symbol_base.insert(symbol.name, Rc::new(SymbolBase::new(SymbolType::Pathed(stroke_path))));
        }
        Ok(SymbolPathDict {
            symbols: symbol_map,
            symbol_base,
            missing: RefCell::default(),
        })
    }
    /// Symbol names starting with `prefix`, sorted so that a seeded pick
    /// among them is the same every run.
    pub fn symbols_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut names: Vec<String> = self.symbols.keys().filter(|k| k.starts_with(prefix)).cloned().collect();
        names.sort();
        names
    }
    pub fn get_strokepath(&self, name: &str) -> Option<StrokePath> {
        self.symbols.get(name).cloned()
    }
    pub fn get_symbol_base(&self, name: &str) -> Option<Rc<SymbolBase>> {
        self.symbol_base.get(name).cloned()
    }
    /// The fallback glyph for a character no glyph was found for (`X`, or a
    /// built-in cross if the dictionary has none), so a missing glyph never
    /// stops a render. Records the substitution for `missing_glyphs`, along
    /// with the glyph names that were `tried`.
    pub fn fallback_symbol(&self, c: char, tried: &[String]) -> Symbol {
        let mut missing = self.missing.borrow_mut();
        let entry = missing.entry(c).or_default();
        entry.count += 1;
        for key in tried {
            if !entry.tried.contains(key) {
                entry.tried.push(key.clone());
            }
        }
        let base = self.get_symbol_base(FALLBACK_GLYPH).unwrap_or_else(|| {
            let cross = StrokePath::new()
                .move_to(-1.0, -1.0).line_to(1.0, 1.0)
                .move_to(-1.0, 1.0).line_to(1.0, -1.0);
            Rc::new(SymbolBase::new(SymbolType::Pathed(cross)))
        });
        Symbol::new_default_from_base_rc(base)
    }
    /// Every character drawn with the fallback glyph so far.
    pub fn missing_glyphs(&self) -> BTreeMap<char, MissingGlyph> {
        self.missing.borrow().clone()
    }
    /// A one-line summary of `missing_glyphs`, or `None` if there were none.
    pub fn missing_glyph_report(&self) -> Option<String> {
        let missing = self.missing.borrow();
        if missing.is_empty() {
            return None;
        }
        let details: Vec<String> = missing.iter().map(|(c, glyph)| {
            let tried = if glyph.tried.is_empty() {
                String::new()
            } else {
                format!(" (tried {})", glyph.tried.join(", "))
            };
            format!("{:?} x{}{}", c, glyph.count, tried)
        }).collect();
        Some(format!(
            "{} character(s) had no glyph and were drawn as {}: {}",
            missing.len(), FALLBACK_GLYPH, details.join("; "),
        ))
    }
    pub fn new_symbol(&self, name: &str) -> Option<Symbol> {
        let origin = Vec2::new(0.0, 0.0);
        self.get_strokepath(name).map(|path| {
            Symbol::new(path, 1.0, origin, 0.0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(json: &str) -> SymbolPathDict {
        SymbolPathDict::from_bytes(json.as_bytes()).expect("dictionary should parse")
    }

    const WITH_X: &str = r#"[{"name": "X", "path": "M -1 -1 L 1 1 M -1 1 L 1 -1", "adj": null}]"#;

    #[test]
    fn bad_path_data_names_the_symbol() {
        let json = r#"[{"name": "broken", "path": "M 0 0 L 1", "adj": null}]"#;
        let err = SymbolPathDict::from_bytes(json.as_bytes()).err().expect("bad path should fail").to_string();
        assert!(err.contains("symbol 'broken'") && err.contains("'L' expects more numbers"), "{}", err);
    }

    #[test]
    fn fallback_records_each_missing_character() {
        let d = dict(WITH_X);
        assert_eq!(d.missing_glyph_report(), None);
        d.fallback_symbol('q', &["copper_lower_q".to_string()]);
        d.fallback_symbol('q', &["copper_lower_q".to_string(), "loche_lower_q".to_string()]);
        d.fallback_symbol('—', &[]);
        let missing = d.missing_glyphs();
        assert_eq!(missing[&'q'], MissingGlyph { tried: vec!["copper_lower_q".into(), "loche_lower_q".into()], count: 2 });
        assert_eq!(missing[&'—'].count, 1);
        assert_eq!(
            d.missing_glyph_report().unwrap(),
            "2 character(s) had no glyph and were drawn as X: 'q' x2 (tried copper_lower_q, loche_lower_q); '—' x1",
        );
    }

    #[test]
    fn fallback_uses_x_or_a_builtin_cross() {
        let with_x = dict(WITH_X).fallback_symbol('q', &[]);
        assert!(!with_x.is_blank());
        let without_x = dict("[]").fallback_symbol('q', &[]);
        assert!(!without_x.is_blank(), "a dictionary without X still draws a cross");
    }
}
