use crate::journal::symboldict::SymbolPathDict;
use crate::journal::symbol::{Symbol, SymbolBase, SymbolType};

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
    filters: Vec<Box<dyn Fn(&mut Symbol)>>,
    collection_filters: Vec<Box<dyn Fn(&mut Vec<Symbol>)>>,
}

pub type StrToSymTransformer<'symdict> = Box<dyn Fn(&'symdict SymbolPathDict, char) -> Symbol>;

impl <'symdict> SymbolStringBuilder<'symdict> {
    pub fn new<F: Fn(&'symdict SymbolPathDict, char) -> Symbol + 'static>(symdict: &'symdict SymbolPathDict, transformer: F) -> Self {
        Self {
            symdict,
            transformer: Box::new(transformer),
            filters: Vec::new(),
            collection_filters: Vec::new(),
        }
    }
    pub fn add_filter<F: Fn(&mut Symbol) + 'static>(&mut self, filter: F) {
        self.filters.push(Box::new(filter));
    }
    pub fn build(&self, s: &str) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        for c in s.chars() {
            let mut symbol = (self.transformer)(self.symdict, c);
            for filter in &self.filters {
                filter(&mut symbol);
            }
            symbols.push(symbol);
        }
        for collection_filter in &self.collection_filters {
            collection_filter(&mut symbols);
        }
        symbols
    }
}

pub fn ternary_builder<'sd>(symdict: &'sd SymbolPathDict, prefix: &str) -> SymbolStringBuilder<'sd> {
    let prefix = prefix.to_string();
    SymbolStringBuilder::<'sd>::new(symdict, move |symd, c| {
        let key = format!("{}_{}", prefix, balanced_base27_symbol(c));
        if let Some(sym_base) = symd.get_symbol_base(&key) {
            Symbol::new_default_from_base_rc(sym_base)
        } else {
            //print to stderr that the symbol was not found
            eprintln!("Symbol '{}' not found in symbol dictionary, using 'X' instead.", key);
            Symbol::new_default_from_base_rc(symd.get_symbol_base("X").unwrap())
        }
    })
}

pub fn lowercase_builder<'sd>(symdict: &'sd SymbolPathDict, prefix: &str) -> SymbolStringBuilder<'sd> {
    let prefix = prefix.to_string();
    SymbolStringBuilder::<'sd>::new(symdict, move |symd, c| {
        let c = c.to_lowercase().next().unwrap_or(' ');
        if c.is_whitespace() {
            if c == ' ' || c == '\n' {
                return Symbol::new_default_from_base_rc(Rc::new(SymbolBase::new(SymbolType::Blank(1.0, 1.0))));
            } else if c == '\t' {
                return Symbol::new_default_from_base_rc(Rc::new(SymbolBase::new(SymbolType::Blank(4.0, 1.0))));
            } else {
                return Symbol::new_default_from_base_rc(symd.get_symbol_base("X").unwrap());
            }
        } else {
            let mut key = format!("{}_{}", prefix, c);
            let possible_keys = symd.symbols_with_prefix(key.as_str());
            if !possible_keys.is_empty() {
                // choose a random key from possible_keys
                let selection = rand::random::<u32>() % possible_keys.len() as u32;
                key = possible_keys[selection as usize].clone();
            }
            if let Some(sym_base) = symd.get_symbol_base(&key) {
                return Symbol::new_default_from_base_rc(sym_base);
            } else {
                //print to stderr that the symbol was not found
                eprintln!("Symbol '{}' not found in symbol dictionary, using 'X' instead.", key);
                return Symbol::new_default_from_base_rc(symd.get_symbol_base("X").unwrap());
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
    pub fn into_symbol_string_builder<'sd>(self, symdict: &'sd SymbolPathDict) -> SymbolStringBuilder<'sd> {
        SymbolStringBuilder::<'sd>::new(symdict, move |symd, c| {
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
                    _ => {
                        symd.get_symbol_base("X").unwrap()
                    }
                };
                return Symbol::new_default_from_base_rc(symbase);
            } else {
                let mut chosen_keys = None;
                let fontset = if c.is_lowercase() {
                    lower_fonts
                } else {
                    upper_fonts
                };
                for prefix in fontset {
                    let key = format!("{}_{}", prefix, prefix_c);
                    let poss_keys = symd.symbols_with_prefix(key.as_str());
                    if !poss_keys.is_empty() {
                        chosen_keys = Some(poss_keys);
                        break
                    }
                }
                match chosen_keys {
                    Some(keys) => {
                        let selection = rand::random::<u32>() % keys.len() as u32;
                        let key = keys[selection as usize].clone();
                        if let Some(sym_base) = symd.get_symbol_base(&key) {
                            return Symbol::new_default_from_base_rc(sym_base);
                        } else {
                            //print to stderr that the symbol was not found
                            eprintln!("Symbol '{}' not found in symbol dictionary, using 'X' instead.", key);
                            return Symbol::new_default_from_base_rc(symd.get_symbol_base("X").unwrap());
                        }
                    }
                    None => {
                        //print to stderr that the symbol was not found
                        eprintln!("Symbol '{}' not found in symbol set, using 'X' instead.", prefix_c);
                        Symbol::new_default_from_base_rc(symd.get_symbol_base("X").unwrap())
                    }
                }
            }
        })
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
