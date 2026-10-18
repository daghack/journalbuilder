//! Fonts: glyphs by character, with the settings text is set with, loaded
//! from a font file (by default `webapp/config/fonts.json`):
//!
//! ```json
//! {
//!     "copper": {
//!         "character_padding": 0.05,
//!         "space_width": 0.5,
//!         "fallback": "loche_upper",
//!         "glyph_padding": { ".": 0.2 },
//!         "glyphs": {
//!             "A": "M -0.6 -0.6 C ...",
//!             "a": ["M 0.4 -0.2 C ...", "M 0.5 -0.1 C ..."]
//!         }
//!     }
//! }
//! ```
//!
//! - `glyphs` maps each character (or, in a set of symbols, each name) to
//!   SVG path data spanning about -1..1, y down, or to a list of variants,
//!   one of which is picked at random each time the character is set.
//! - `character_padding` is room between neighbouring glyphs' outlines and
//!   `space_width` the width of a space, both in font sizes (default 0 and
//!   0.5). A tab is four spaces.
//! - `glyph_padding` gives glyphs more room than `character_padding`: the
//!   amount, in font sizes, added to it for each glyph listed. A lone dot
//!   needs it: drawn, its ink is as wide as the padding, so a full stop
//!   between letters (`page.tile`) is lost in them.
//! - `fallback` names a font to look in for characters this one hasn't.
//!
//! Text is set with each glyph as close to the last as their outlines (the
//! convex hulls of their strokes, leant as they're drawn) allow, plus the
//! padding: see `symbol::Outline`. So a font can be leant into italics
//! (`Font::skew`, which scripts set) without opening up its spacing.
//!
//! A character is looked for as it is in the font and then its fallbacks,
//! then in the other case (so a font of capitals sets lowercase text in
//! capitals), and failing that drawn as the fallback glyph, `X` from the
//! `symbols` set, and reported (see `FontBook::missing_glyph_report`).

use rand::{Rng, RngCore};
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use super::paths::StrokePath;
use super::symbol::{Symbol, SymbolBase, SymbolType};

/// The repository's font file. Resolved at build time, so it's found
/// whatever directory journalbuild (or a test) runs from.
pub const DEFAULT_FONTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/webapp/config/fonts.json");

/// The glyph drawn for a character no font has: a font, and its glyph.
pub const FALLBACK_GLYPH: (&str, &str) = ("symbols", "X");

/// A space's width in fonts that don't give one, in font sizes.
pub const DEFAULT_SPACE_WIDTH: f32 = 0.5;

/// A tab, in spaces.
const TAB_SPACES: f32 = 4.0;

/// The base-27 balanced-ternary symbol for a character, from -13 to 13:
/// the 26 ASCII letters take the non-zero symbols (a..=m -> 1..=13,
/// n..=z -> -13..=-1, case-insensitively), and everything else (space,
/// digits, punctuation, non-ASCII letters) shares 0.
pub fn balanced_base27_symbol(c: char) -> i32 {
    if !c.is_ascii_alphabetic() {
        return 0;
    }
    let ordinal = (c.to_ascii_lowercase() as u8 - b'a') as i32;
    if ordinal < 13 {
        ordinal + 1
    } else {
        ordinal - 26
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FontEntry {
    #[serde(default)]
    character_padding: f32,
    #[serde(default = "default_space_width")]
    space_width: f32,
    fallback: Option<String>,
    #[serde(default)]
    glyph_padding: BTreeMap<String, f32>,
    glyphs: BTreeMap<String, Variants>,
}

fn default_space_width() -> f32 {
    DEFAULT_SPACE_WIDTH
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Variants {
    One(String),
    Many(Vec<String>),
}

/// A font's glyphs as loaded: every variant of each glyph, in file order.
#[derive(Debug)]
pub struct Glyphs {
    name: String,
    glyphs: BTreeMap<String, Vec<Rc<SymbolBase>>>,
    /// Room glyphs keep on top of the font's padding, in font sizes.
    padding: BTreeMap<String, f32>,
}

/// How a font turns characters into glyphs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mapping {
    /// Each character is its own glyph.
    Characters,
    /// Each character is a base-27 balanced-ternary digit, the glyphs
    /// named `-13` to `13` (see `balanced_base27_symbol`).
    BalancedTernary,
}

/// A font as text is set in it: its glyphs, its settings (the file's,
/// unless changed), and what it falls back to.
#[derive(Clone, Debug)]
pub struct Font {
    glyphs: Rc<Glyphs>,
    /// Room between neighbouring glyphs, in font sizes.
    pub character_padding: f32,
    /// A space's width, in font sizes.
    pub space_width: f32,
    /// How far its glyphs lean, as italics do, in radians (see
    /// `Symbol::skew_rad`). It leans every glyph the font sets, its
    /// fallbacks' too.
    pub skew: f32,
    pub fallback: Option<Box<Font>>,
    pub mapping: Mapping,
}

impl Font {
    pub fn name(&self) -> &str {
        &self.glyphs.name
    }
    /// The characters (or names) the font has glyphs for, sorted.
    pub fn characters(&self) -> Vec<String> {
        self.glyphs.glyphs.keys().cloned().collect()
    }
    /// Every variant of the glyph for `key`, padded as the font pads them,
    /// or `None` if the font hasn't one (its fallbacks aren't looked in).
    pub fn variants(&self, key: &str) -> Option<Vec<Symbol>> {
        self.glyphs.glyphs.get(key).map(|bases| bases.iter().map(|base| self.symbol(key, base)).collect())
    }
    /// The glyph `key` drawn as `base`, padded as the font pads it.
    fn symbol(&self, key: &str, base: &Rc<SymbolBase>) -> Symbol {
        let padding = self.character_padding + self.glyphs.padding.get(key).copied().unwrap_or(0.0);
        Symbol::new_default_from_base_rc(base.clone()).with_padding(padding).with_skew(self.skew)
    }
    /// This font and then its fallbacks, in order.
    fn chain(&self) -> impl Iterator<Item = &Font> {
        std::iter::successors(Some(self), |font| font.fallback.as_deref())
    }
    /// The first font in the chain with a glyph for `key`, and its variants.
    fn find(&self, key: &str) -> Option<(&Font, &[Rc<SymbolBase>])> {
        self.chain().find_map(|font| font.glyphs.glyphs.get(key).map(|bases| (font, bases.as_slice())))
    }
    /// A blank `spaces` spaces wide.
    fn blank(&self, spaces: f32) -> Symbol {
        // A font size is 2 glyph units.
        let width = 2.0 * self.space_width * spaces;
        Symbol::new_default_from_base_rc(Rc::new(SymbolBase::new(SymbolType::Blank(width, 1.0))))
    }
    /// The glyphs for `text`, a variant picked with `rng` where a glyph has
    /// several. Characters with no glyph anywhere are drawn with `book`'s
    /// fallback glyph, and recorded there.
    pub fn build(&self, text: &str, book: &FontBook, rng: &mut dyn RngCore) -> Vec<Symbol> {
        text.chars().map(|c| {
            let glyph = self.glyph_for(c, book, rng);
            if glyph.is_blank() { glyph } else { glyph.with_skew(self.skew) }
        }).collect()
    }
    fn glyph_for(&self, c: char, book: &FontBook, rng: &mut dyn RngCore) -> Symbol {
        if self.mapping == Mapping::BalancedTernary {
            let key = balanced_base27_symbol(c).to_string();
            return match self.find(&key) {
                Some((font, bases)) => font.symbol(&key, &bases[0]),
                None => book.fallback_symbol(c, &[self.tried(&key)]),
            };
        }
        match c {
            ' ' | '\n' => return self.blank(1.0),
            '\t' => return self.blank(TAB_SPACES),
            c if c.is_whitespace() => return book.fallback_symbol(c, &[]),
            _ => {}
        }
        // As it is, then in the other case.
        let mut keys = vec![c.to_string()];
        let other: String = if c.is_lowercase() { c.to_uppercase().collect() } else { c.to_lowercase().collect() };
        if other.chars().count() == 1 && other != keys[0] {
            keys.push(other);
        }
        for key in &keys {
            if let Some((font, bases)) = self.find(key) {
                // Always drawn from rng, one variant or many, so a font
                // gaining variants doesn't shift every pick after it.
                return font.symbol(key, &bases[rng.random_range(0..bases.len())]);
            }
        }
        let tried: Vec<String> = keys.iter().flat_map(|key| self.chain().map(move |font| font.tried(key))).collect();
        book.fallback_symbol(c, &tried)
    }
    fn tried(&self, key: &str) -> String {
        format!("{} '{}'", self.name(), key)
    }
}

/// A character that had no glyph and was drawn with the fallback instead.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MissingGlyph {
    /// Fonts and glyphs that were looked for, in order.
    pub tried: Vec<String>,
    /// How many times it was drawn with the fallback.
    pub count: usize,
}

/// Every font in a font file, and the characters drawn with the fallback
/// glyph because no font had them.
#[derive(Debug)]
pub struct FontBook {
    fonts: BTreeMap<String, Font>,
    missing: RefCell<BTreeMap<char, MissingGlyph>>,
}

impl FontBook {
    pub fn from_file(file: &str) -> Result<FontBook, String> {
        let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
        Self::from_bytes(&bytes)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<FontBook, String> {
        let entries: BTreeMap<String, FontEntry> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let mut loaded: BTreeMap<String, (Rc<Glyphs>, &FontEntry)> = BTreeMap::new();
        for (name, entry) in &entries {
            let bad = |what: String| format!("font '{}': {}", name, what);
            if !(entry.character_padding.is_finite() && entry.character_padding >= 0.0) {
                return Err(bad(format!("character_padding must be a number of at least 0, got {}", entry.character_padding)));
            }
            if !(entry.space_width.is_finite() && entry.space_width > 0.0) {
                return Err(bad(format!("space_width must be a positive number, got {}", entry.space_width)));
            }
            if let Some(fallback) = &entry.fallback
                && !entries.contains_key(fallback) {
                return Err(bad(format!("falls back to '{}', which isn't a font", fallback)));
            }
            for (key, extra) in &entry.glyph_padding {
                if !entry.glyphs.contains_key(key) {
                    return Err(bad(format!("glyph_padding gives '{}', which isn't a glyph", key)));
                }
                if !(extra.is_finite() && *extra >= 0.0) {
                    return Err(bad(format!("glyph_padding for '{}' must be a number of at least 0, got {}", key, extra)));
                }
            }
            let mut glyphs = BTreeMap::new();
            for (key, variants) in &entry.glyphs {
                let paths: Vec<&String> = match variants {
                    Variants::One(path) => vec![path],
                    Variants::Many(paths) => paths.iter().collect(),
                };
                if paths.is_empty() {
                    return Err(bad(format!("glyph '{}' has no variants", key)));
                }
                let bases = paths.into_iter().map(|data| {
                    let path: StrokePath = data.parse().map_err(|e| bad(format!("glyph '{}': {}", key, e)))?;
                    Ok(Rc::new(SymbolBase::new(SymbolType::Pathed(path))))
                }).collect::<Result<Vec<_>, String>>()?;
                glyphs.insert(key.clone(), bases);
            }
            loaded.insert(name.clone(), (Rc::new(Glyphs { name: name.clone(), glyphs, padding: entry.glyph_padding.clone() }), entry));
        }
        // Fonts with their fallbacks, which mustn't loop.
        fn resolve(name: &str, loaded: &BTreeMap<String, (Rc<Glyphs>, &FontEntry)>, seen: &mut Vec<String>) -> Result<Font, String> {
            if seen.iter().any(|s| s == name) {
                seen.push(name.to_string());
                return Err(format!("fonts fall back in a loop: {}", seen.join(" -> ")));
            }
            seen.push(name.to_string());
            let (glyphs, entry) = &loaded[name];
            let fallback = entry.fallback.as_deref().map(|f| resolve(f, loaded, seen).map(Box::new)).transpose()?;
            Ok(Font {
                glyphs: glyphs.clone(),
                character_padding: entry.character_padding,
                space_width: entry.space_width,
                skew: 0.0,
                fallback,
                mapping: Mapping::Characters,
            })
        }
        let fonts = loaded.keys().map(|name| Ok((name.clone(), resolve(name, &loaded, &mut Vec::new())?)))
            .collect::<Result<_, String>>()?;
        Ok(FontBook { fonts, missing: RefCell::default() })
    }
    /// Every font's name, sorted.
    pub fn names(&self) -> Vec<String> {
        self.fonts.keys().cloned().collect()
    }
    /// A font as the file sets it.
    pub fn font(&self, name: &str) -> Option<Font> {
        self.fonts.get(name).cloned()
    }
    /// The fallback glyph for a character no glyph was found for (`X`, or a
    /// built-in cross if the file has none), so a missing glyph never stops
    /// a render. Records the substitution for `missing_glyphs`, along with
    /// the fonts and glyphs that were `tried`.
    pub fn fallback_symbol(&self, c: char, tried: &[String]) -> Symbol {
        let mut missing = self.missing.borrow_mut();
        let entry = missing.entry(c).or_default();
        entry.count += 1;
        for key in tried {
            if !entry.tried.contains(key) {
                entry.tried.push(key.clone());
            }
        }
        let (font, key) = FALLBACK_GLYPH;
        let base = self.fonts.get(font).and_then(|f| f.glyphs.glyphs.get(key)).map(|bases| bases[0].clone()).unwrap_or_else(|| {
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
            missing.len(), FALLBACK_GLYPH.1, details.join("; "),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    const LINE: &str = "M -0.5 -1 L 0.5 1";

    fn book(json: &str) -> FontBook {
        FontBook::from_bytes(json.as_bytes()).expect("font file should load")
    }

    fn keys(font: &Font, text: &str, book: &FontBook) -> Vec<String> {
        // Which glyph each character got, by its width (each test glyph is
        // a different width).
        font.build(text, book, &mut StdRng::seed_from_u64(1)).iter()
            .map(|s| format!("{:.2}", s.hull_aabb().extents().x)).collect()
    }

    #[test]
    fn letters_take_the_nonzero_base27_symbols() {
        assert_eq!(balanced_base27_symbol('a'), 1);
        assert_eq!(balanced_base27_symbol('m'), 13);
        assert_eq!(balanced_base27_symbol('n'), -13);
        assert_eq!(balanced_base27_symbol('z'), -1);
        assert_eq!(balanced_base27_symbol('Q'), balanced_base27_symbol('q'));
        let all: std::collections::HashSet<i32> = ('a'..='z').map(balanced_base27_symbol).collect();
        assert_eq!(all.len(), 26);
        assert!(all.iter().all(|&v| v != 0 && (-13..=13).contains(&v)));
        for c in [' ', '\n', '7', '.', ',', '\'', 'é', 'ß', 'λ'] {
            assert_eq!(balanced_base27_symbol(c), 0, "{:?}", c);
        }
    }

    #[test]
    fn characters_are_found_as_they_are_then_in_fallbacks_then_the_other_case() {
        // caps has only capitals; lower falls back to caps. Glyph widths
        // tell them apart: caps' are about 0.2 wide, lower's 0.4.
        let b = book(r#"{
            "caps": { "glyphs": { "A": "M -0.1 0 L 0.1 0", "B": "M -0.1 0 L 0.1 0" } },
            "lower": { "fallback": "caps", "glyphs": { "a": "M -0.2 0 L 0.2 0", "c": "M -0.2 0 L 0.2 0" } }
        }"#);
        let (caps, lower) = (b.font("caps").unwrap(), b.font("lower").unwrap());
        // A capital font sets lowercase text in capitals.
        assert_eq!(keys(&caps, "aB", &b), ["0.19", "0.19"]);
        // 'A' is in the fallback as it is, before 'a' in lower.
        assert_eq!(keys(&lower, "aAbC", &b), ["0.39", "0.19", "0.19", "0.39"]);
        assert_eq!(lower.name(), "lower");
        assert_eq!(caps.characters(), ["A", "B"]);
        assert!(b.missing_glyphs().is_empty());
    }

    #[test]
    fn padding_and_spaces_follow_the_font() {
        let b = book(&format!(r#"{{ "f": {{ "character_padding": 0.25, "space_width": 0.3, "glyphs": {{ "a": "{}" }} }} }}"#, LINE));
        let mut f = b.font("f").unwrap();
        let built = f.build("a \t", &b, &mut StdRng::seed_from_u64(1));
        // The line is 1 unit wide, and a quarter of a font size of padding
        // is a quarter of a unit each side.
        assert!((built[0].hull_aabb().extents().x - 1.5).abs() < 0.03, "{:?}", built[0].hull_aabb());
        assert!((built[1].hull_aabb().extents().x - 0.6).abs() < 1e-5);
        assert!((built[2].hull_aabb().extents().x - 2.4).abs() < 1e-5);
        // Changed settings are used from then on.
        f.character_padding = 0.0;
        f.space_width = 1.0;
        let built = f.build("a ", &b, &mut StdRng::seed_from_u64(1));
        assert!((built[0].hull_aabb().extents().x - 1.0).abs() < 0.03);
        assert!((built[1].hull_aabb().extents().x - 2.0).abs() < 1e-5);
    }

    #[test]
    fn listed_glyphs_get_more_padding() {
        let b = book(&format!(r#"{{ "f": {{ "character_padding": 0.25, "glyph_padding": {{ "b": 0.5 }}, "glyphs": {{ "a": "{0}", "b": "{0}" }} }} }}"#, LINE));
        let f = b.font("f").unwrap();
        let built = f.build("ab", &b, &mut StdRng::seed_from_u64(1));
        assert!((built[0].hull_aabb().extents().x - 1.5).abs() < 0.03, "{:?}", built[0].hull_aabb());
        assert!((built[1].hull_aabb().extents().x - 2.5).abs() < 0.03, "{:?}", built[1].hull_aabb());
        assert!((f.variants("b").unwrap()[0].hull_aabb().extents().x - 2.5).abs() < 0.03);
        for (json, error) in [
            (r#"{ "f": { "glyph_padding": { "z": 0.1 }, "glyphs": {} } }"#, "glyph_padding gives 'z', which isn't a glyph"),
            (r#"{ "f": { "glyph_padding": { "a": -1 }, "glyphs": { "a": "M 0 0 L 1 0" } } }"#, "glyph_padding for 'a' must be"),
        ] {
            let e = FontBook::from_bytes(json.as_bytes()).unwrap_err();
            assert!(e.contains(error), "{}", e);
        }
    }

    #[test]
    fn a_skewed_font_leans_every_glyph_it_sets() {
        let b = book(r#"{
            "caps": { "glyphs": { "A": "M -0.1 0 L 0.1 0" } },
            "lower": { "fallback": "caps", "glyphs": { "a": "M -0.2 0 L 0.2 0" } }
        }"#);
        let mut lower = b.font("lower").unwrap();
        lower.skew = 0.2;
        let built = lower.build("aA ", &b, &mut StdRng::seed_from_u64(1));
        // Its fallback's glyphs too, but not spaces, which have no strokes.
        assert_eq!(built.iter().map(|g| g.skew_rad()).collect::<Vec<_>>(), [0.2, 0.2, 0.0]);
        assert_eq!(lower.variants("a").unwrap()[0].skew_rad(), 0.2);
        // The font it falls back to sets its own text upright.
        assert_eq!(b.font("caps").unwrap().build("A", &b, &mut StdRng::seed_from_u64(1))[0].skew_rad(), 0.0);
    }

    #[test]
    fn variants_are_picked_with_the_generator() {
        let b = book(r#"{ "f": { "glyphs": { "a": ["M -0.1 0 L 0.1 0", "M -0.2 0 L 0.2 0", "M -0.3 0 L 0.3 0"] } } }"#);
        let f = b.font("f").unwrap();
        let picks: std::collections::HashSet<String> = keys(&f, &"a".repeat(40), &b).into_iter().collect();
        assert_eq!(picks.len(), 3);
        assert_eq!(keys(&f, "aaaa", &b), keys(&f, "aaaa", &b));
        assert_eq!(f.variants("a").unwrap().len(), 3);
        assert!(f.variants("b").is_none());
    }

    #[test]
    fn ternary_fonts_map_letters_to_digits() {
        let b = book(r#"{ "t": { "glyphs": { "1": "M -0.1 0 L 0.1 0", "0": "M -0.2 0 L 0.2 0", "-1": "M -0.3 0 L 0.3 0" } } }"#);
        let mut t = b.font("t").unwrap();
        t.mapping = Mapping::BalancedTernary;
        assert_eq!(keys(&t, "a z", &b), ["0.19", "0.39", "0.59"]);
    }

    #[test]
    fn missing_characters_are_drawn_as_x_and_recorded() {
        let b = book(r#"{
            "symbols": { "glyphs": { "X": "M -1 -1 L 1 1 M -1 1 L 1 -1" } },
            "caps": { "glyphs": { "A": "M -0.1 0 L 0.1 0" } },
            "lower": { "fallback": "caps", "glyphs": { "a": "M -0.2 0 L 0.2 0" } }
        }"#);
        assert_eq!(b.missing_glyph_report(), None);
        let lower = b.font("lower").unwrap();
        lower.build("qq—", &b, &mut StdRng::seed_from_u64(1));
        let missing = b.missing_glyphs();
        assert_eq!(missing[&'q'], MissingGlyph { tried: vec!["lower 'q'".into(), "caps 'q'".into(), "lower 'Q'".into(), "caps 'Q'".into()], count: 2 });
        assert_eq!(missing[&'—'].count, 1);
        assert!(b.missing_glyph_report().unwrap().starts_with("2 character(s) had no glyph and were drawn as X: 'q' x2 (tried lower 'q'"));
        // Without an X, a built-in cross.
        assert!(!book("{}").fallback_symbol('q', &[]).is_blank());
    }

    #[test]
    fn bad_font_files_say_what_is_wrong() {
        for (json, want) in [
            (r#"{ "f": { "glyphs": { "a": "M 0 0 L 1" } } }"#, "font 'f': glyph 'a': 'L' expects more numbers"),
            (r#"{ "f": { "fallback": "g", "glyphs": {} } }"#, "font 'f': falls back to 'g', which isn't a font"),
            (r#"{ "f": { "fallback": "g", "glyphs": {} }, "g": { "fallback": "f", "glyphs": {} } }"#, "fonts fall back in a loop: f -> g -> f"),
            (r#"{ "f": { "character_padding": -1, "glyphs": {} } }"#, "font 'f': character_padding must be"),
            (r#"{ "f": { "space_width": 0, "glyphs": {} } }"#, "font 'f': space_width must be"),
            (r#"{ "f": { "glyphs": { "a": [] } } }"#, "font 'f': glyph 'a' has no variants"),
            (r#"{ "f": { "colour": 1, "glyphs": {} } }"#, "unknown field `colour`"),
        ] {
            let err = FontBook::from_bytes(json.as_bytes()).expect_err(json);
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn the_repository_font_file_loads() {
        let b = FontBook::from_file(DEFAULT_FONTS).unwrap();
        for name in ["copper", "plain", "loche_upper", "loche_lower", "lang", "eldritch", "sharp_lower", "nivelia", "balanced_ternary", "symbols"] {
            assert!(b.font(name).is_some(), "{}", name);
        }
        assert_eq!(b.font("copper").unwrap().fallback.unwrap().name(), "loche_upper");
    }
}
