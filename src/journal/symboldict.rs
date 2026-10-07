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
    /// The glyph's strokes, as SVG path data; none for a blank.
    #[serde(default)]
    path: String,
    adj: Option<AdjType>,
    /// Room either side of the glyph's strokes when text is set, in glyph
    /// units (the path spans about -1..1).
    #[serde(default)]
    bearing: f32,
    /// A blank this wide in glyph units, such as a font's space, instead
    /// of strokes.
    blank: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct SymbolPathDict {
    symbols: HashMap<String, StrokePath>,
    symbol_base: HashMap<String, Rc<SymbolBase>>,
    /// Characters drawn with the fallback glyph because no glyph was found.
    missing: RefCell<BTreeMap<char, MissingGlyph>>,
}

/// The repository's symbol dictionary. Resolved at build time, so it's found
/// whatever directory journalbuild (or a test) runs from.
pub const DEFAULT_SYMBOLS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/config/symbols.json");

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
            let invalid = |what: String| io::Error::new(io::ErrorKind::InvalidData, format!("symbol '{}': {}", symbol.name, what));
            if let Some(width) = symbol.blank {
                if !(width.is_finite() && width > 0.0) || !symbol.path.is_empty() {
                    return Err(invalid(format!("a blank needs a positive width and no path, got width {} and path '{}'", width, symbol.path)));
                }
                symbol_map.insert(symbol.name.clone(), StrokePath::new());
                symbol_base.insert(symbol.name, Rc::new(SymbolBase::new(SymbolType::Blank(width, 1.0))));
                continue;
            }
            let mut stroke_path: StrokePath = symbol.path.parse().map_err(invalid)?;
            match symbol.adj {
                Some(AdjType::ZeroToOneRemap) => {
                    // This is for when the incoming path is in a 0-1 coordinate space.
                    // Since our paths are in a -1 to 1 coordinate space, we need to scale and translate.
                    // (This is currently used to automatically convert older symbol paths to the new format)
                    stroke_path = stroke_path.scaled_by(2.0).translated_by(Vec2::new(-1.0, -1.0));
                }
                None => {}
            }
            if !(symbol.bearing.is_finite() && symbol.bearing >= 0.0) {
                return Err(invalid(format!("bearing must be a number of at least 0, got {}", symbol.bearing)));
            }
            symbol_map.insert(symbol.name.clone(), stroke_path.clone());
            let base = SymbolBase::new(SymbolType::Pathed(stroke_path)).with_bearing(symbol.bearing);
            symbol_base.insert(symbol.name, Rc::new(base));
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(json: &str) -> SymbolPathDict {
        SymbolPathDict::from_bytes(json.as_bytes()).expect("dictionary should parse")
    }

    const WITH_X: &str = r#"[{"name": "X", "path": "M -1 -1 L 1 1 M -1 1 L 1 -1", "adj": null}]"#;

    #[test]
    fn straight_line_glyphs_load() {
        for path in ["M -1 -1 L 1 1", "M -1 0 L 1 0", "M 0 -1 L 0 1", "M 0 0 L 0 0"] {
            let json = format!(r#"[{{"name": "line", "path": "{}", "adj": null}}]"#, path);
            let symd = dict(&json);
            assert!(symd.get_symbol_base("line").is_some(), "{}", path);
        }
    }

    #[test]
    fn bearings_widen_glyphs_but_not_their_outlines() {
        let symd = dict(r#"[{"name": "bare", "path": "M -0.5 -1 L 0.5 1"}, {"name": "roomy", "path": "M -0.5 -1 L 0.5 1", "bearing": 0.2}]"#);
        let (bare, roomy) = (symd.get_symbol_base("bare").unwrap(), symd.get_symbol_base("roomy").unwrap());
        let (bare, roomy) = (Symbol::new_default_from_base_rc(bare), Symbol::new_default_from_base_rc(roomy));
        assert!((roomy.hull_aabb().extents().x - bare.hull_aabb().extents().x - 0.4).abs() < 1e-5);
        assert_eq!(roomy.hull_aabb().extents().y, bare.hull_aabb().extents().y);
        assert_eq!(roomy.footprint(), bare.footprint());
        let err = SymbolPathDict::from_bytes(br#"[{"name": "bad", "path": "M 0 0 L 1 1", "bearing": -1}]"#).expect_err("negative bearing").to_string();
        assert!(err.contains("symbol 'bad': bearing"), "{}", err);
    }

    #[test]
    fn blanks_have_a_width_and_no_strokes() {
        let symd = dict(r#"[{"name": "font_ ", "blank": 0.6}]"#);
        let space = Symbol::new_default_from_base_rc(symd.get_symbol_base("font_ ").unwrap());
        assert!(space.is_blank());
        assert!((space.hull_aabb().extents().x - 0.6).abs() < 1e-5);
        for bad in [r#"[{"name": "b", "blank": 0}]"#, r#"[{"name": "b", "blank": 1, "path": "M 0 0 L 1 1"}]"#] {
            let err = SymbolPathDict::from_bytes(bad.as_bytes()).expect_err("bad blank").to_string();
            assert!(err.contains("symbol 'b': a blank needs"), "{}", err);
        }
    }

    #[test]
    fn bad_path_data_names_the_symbol() {
        let json = r#"[{"name": "broken", "path": "M 0 0 L 1", "adj": null}]"#;
        let err = SymbolPathDict::from_bytes(json.as_bytes()).expect_err("bad path should fail").to_string();
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
