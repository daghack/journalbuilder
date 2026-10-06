use crate::journal::symboldict::SymbolPathDict;
use crate::journal::symbol::{Symbol, SymbolBase, SymbolType};

use rand::{Rng, RngCore};
use std::rc::Rc;

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

pub struct SymbolStringBuilder<'symdict> {
    symdict: &'symdict SymbolPathDict,
    transformer: StrToSymTransformer<'symdict>,
    filters: Vec<SymbolFilter>,
}

/// Adjusts a symbol after it's built (e.g. random jitter).
pub type SymbolFilter = Box<dyn Fn(&mut Symbol, &mut dyn RngCore)>;

/// Turns one character into a symbol. Any random choice (such as which
/// variant of a glyph to draw) comes from the `rng` passed to `build`, so
/// seeded callers get the same symbols every run.
pub type StrToSymTransformer<'symdict> = Box<dyn Fn(&'symdict SymbolPathDict, char, &mut dyn RngCore) -> Symbol>;

impl <'symdict> SymbolStringBuilder<'symdict> {
    pub fn new<F: Fn(&'symdict SymbolPathDict, char, &mut dyn RngCore) -> Symbol + 'static>(symdict: &'symdict SymbolPathDict, transformer: F) -> Self {
        Self {
            symdict,
            transformer: Box::new(transformer),
            filters: Vec::new(),
        }
    }
    pub fn add_filter<F: Fn(&mut Symbol, &mut dyn RngCore) + 'static>(&mut self, filter: F) {
        self.filters.push(Box::new(filter));
    }
    pub fn build(&self, s: &str, rng: &mut dyn RngCore) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        for c in s.chars() {
            let mut symbol = (self.transformer)(self.symdict, c, rng);
            for filter in &self.filters {
                filter(&mut symbol, rng);
            }
            symbols.push(symbol);
        }
        symbols
    }
}

pub fn ternary_builder<'sd>(symdict: &'sd SymbolPathDict, prefix: &str) -> SymbolStringBuilder<'sd> {
    let prefix = prefix.to_string();
    SymbolStringBuilder::<'sd>::new(symdict, move |symd, c, _rng| {
        let key = format!("{}_{}", prefix, balanced_base27_symbol(c));
        match symd.get_symbol_base(&key) {
            Some(sym_base) => Symbol::new_default_from_base_rc(sym_base),
            None => symd.fallback_symbol(c, &[key]),
        }
    })
}

pub fn lowercase_builder<'sd>(symdict: &'sd SymbolPathDict, prefix: &str) -> SymbolStringBuilder<'sd> {
    let prefix = prefix.to_string();
    SymbolStringBuilder::<'sd>::new(symdict, move |symd, original, rng| {
        let c = original.to_lowercase().next().unwrap_or(' ');
        if c.is_whitespace() {
            if c == ' ' || c == '\n' {
                Symbol::new_default_from_base_rc(Rc::new(SymbolBase::new(SymbolType::Blank(1.0, 1.0))))
            } else if c == '\t' {
                Symbol::new_default_from_base_rc(Rc::new(SymbolBase::new(SymbolType::Blank(4.0, 1.0))))
            } else {
                symd.fallback_symbol(original, &[])
            }
        } else {
            let mut key = format!("{}_{}", prefix, c);
            let possible_keys = symd.symbols_with_prefix(key.as_str());
            if !possible_keys.is_empty() {
                // choose a random key from possible_keys
                key = possible_keys[rng.random_range(0..possible_keys.len())].clone();
            }
            match symd.get_symbol_base(&key) {
                Some(sym_base) => Symbol::new_default_from_base_rc(sym_base),
                None => symd.fallback_symbol(original, &[key]),
            }
        }
    })
}

#[derive(Clone, Debug)]
pub struct FontSet {
    upper_fonts: Vec<String>,
    lower_fonts: Vec<String>
}

impl FontSet {
    pub fn new(upper_fonts: Vec<String>, lower_fonts: Vec<String>) -> Self {
        FontSet{upper_fonts, lower_fonts}
    }
    /// Every font prefix this set draws from, upper fonts first.
    pub fn prefixes(&self) -> impl Iterator<Item = &str> {
        self.upper_fonts.iter().chain(&self.lower_fonts).map(String::as_str)
    }
    pub fn into_symbol_string_builder<'sd>(self, symdict: &'sd SymbolPathDict) -> SymbolStringBuilder<'sd> {
        SymbolStringBuilder::<'sd>::new(symdict, move |symd, c, rng| {
            let FontSet{upper_fonts, lower_fonts} = &self;
            let prefix_c = c.to_lowercase().next().unwrap_or(' ');
            if prefix_c.is_whitespace() {
                let symbase = match prefix_c {
                    ' ' | '\n' => {
                        Rc::new(SymbolBase::new(SymbolType::Blank(1.0, 1.0)))
                    }
                    '\t' => {
                        Rc::new(SymbolBase::new(SymbolType::Blank(4.0, 1.0)))
                    }
                    _ => return symd.fallback_symbol(c, &[]),
                };
                Symbol::new_default_from_base_rc(symbase)
            } else {
                let mut chosen_keys = None;
                // Lowercase letters look in the lower fonts first; everything
                // else (capitals, digits, punctuation) in the upper fonts
                // first. Either way the other set is the fallback, before
                // the fallback glyph.
                let (first, then) = if c.is_lowercase() {
                    (lower_fonts, upper_fonts)
                } else {
                    (upper_fonts, lower_fonts)
                };
                let mut tried = Vec::new();
                for prefix in first.iter().chain(then) {
                    let key = format!("{}_{}", prefix, prefix_c);
                    tried.push(key.clone());
                    let poss_keys = symd.symbols_with_prefix(key.as_str());
                    if !poss_keys.is_empty() {
                        chosen_keys = Some(poss_keys);
                        break
                    }
                }
                match chosen_keys {
                    Some(keys) => {
                        let key = keys[rng.random_range(0..keys.len())].clone();
                        match symd.get_symbol_base(&key) {
                            Some(sym_base) => Symbol::new_default_from_base_rc(sym_base),
                            None => symd.fallback_symbol(c, &[key]),
                        }
                    }
                    None => symd.fallback_symbol(c, &tried),
                }
            }
        })
    }
}

/// A way of turning text into symbols, for anything that draws text.
#[derive(Clone, Debug)]
pub enum Font {
    /// Glyphs looked up in a list of fonts, in order: the lower fonts for
    /// lowercase letters and the upper fonts for everything else.
    Set(FontSet),
    /// Base-27 balanced-ternary glyphs, `{prefix}_-13` to `{prefix}_13`
    /// (see `balanced_base27_symbol`).
    Ternary(String),
    /// Glyphs from one font, `{prefix}_{c}` for each character lowercased,
    /// picking among the font's variants of it at random.
    Lowercase(String),
}

impl Font {
    pub fn builder<'sd>(&self, symdict: &'sd SymbolPathDict) -> SymbolStringBuilder<'sd> {
        match self {
            Font::Set(set) => set.clone().into_symbol_string_builder(symdict),
            Font::Ternary(prefix) => ternary_builder(symdict, prefix),
            Font::Lowercase(prefix) => lowercase_builder(symdict, prefix),
        }
    }
    /// The font prefixes this font draws from.
    pub fn prefixes(&self) -> Vec<&str> {
        match self {
            Font::Set(set) => set.prefixes().collect(),
            Font::Ternary(prefix) | Font::Lowercase(prefix) => vec![prefix.as_str()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::balanced_base27_symbol;

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
    }

    #[test]
    fn everything_else_shares_zero() {
        for c in [' ', '\n', '7', '.', ',', '\'', 'é', 'ß', 'λ'] {
            assert_eq!(balanced_base27_symbol(c), 0, "{:?}", c);
        }
    }
}
