use crate::journal::symboldict::SymbolPathDict;
use crate::journal::symbol::{Symbol, SymbolBase, SymbolType};

use std::rc::Rc;

fn to_balanced_ternary(n: i32) -> Vec<i8> {
    if n == 0 {
        return vec![0];
    }
    let mut num = n;
    let mut digits = Vec::new();
    while num != 0 {
        let mut remainder = num % 3;
        num /= 3;
        if remainder == 2 {
            remainder = -1;
            num += 1;
        } else if remainder == -2 {
            remainder = 1;
            num -= 1;
        }
        digits.push(remainder as i8);
    }
    digits
}

fn to_balanced_ternary_digits(n: i32, tern_per_digit: usize) -> Vec<Vec<i8>> {
    let ternary = to_balanced_ternary(n);
    let mut digits = Vec::new();
    let mut current_digit = Vec::new();
    for (i, &digit) in ternary.iter().enumerate() {
        current_digit.push(digit);
        if (i + 1) % tern_per_digit == 0 {
            digits.push(current_digit);
            current_digit = Vec::new();
        }
    }
    if !current_digit.is_empty() {
        digits.push(current_digit);
    }
    digits
}

fn from_balanced_ternary(digits: &Vec<i8>) -> i32 {
    let mut value = 0;
    for (i, &digit) in digits.iter().enumerate() {
        value += (digit as i32) * 3_i32.pow(i as u32);
    }
    value
}

fn into_balanced_ternary_ints(n: i32, tern_per_digit: usize) -> Vec<i32> {
    let balanced_ternary_digits = to_balanced_ternary_digits(n, tern_per_digit);
    let mut ints = Vec::new();
    for digit in balanced_ternary_digits {
        let int_value = from_balanced_ternary(&digit);
        ints.push(int_value);
    }
    ints
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
    pub fn add_collection_filter<F: Fn(&mut Vec<Symbol>) + 'static>(&mut self, filter: F) {
        self.collection_filters.push(Box::new(filter));
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
        let c = c.to_lowercase().next().unwrap_or(' ');
        let char_int = if c.is_alphabetic() {
            let char_ord = c as i32 - 'a' as i32;
            let which_half = char_ord / 13;
            let char_ord = char_ord - (26 * which_half);
            char_ord + (1 - which_half)
        } else {
            0
        };
        let ternary_digits = into_balanced_ternary_ints(char_int, 3);
        for digit in ternary_digits.iter() {
            let key = format!("{}_{}", prefix, digit);
            if let Some(sym_base) = symd.get_symbol_base(&key) {
                return Symbol::new_default_from_base_rc(sym_base);
            } else {
                //print to stderr that the symbol was not found
                eprintln!("Symbol '{}' not found in symbol dictionary, using 'X' instead.", key);
                return Symbol::new_default_from_base_rc(symd.get_symbol_base("X").unwrap());
            }
        }
        eprintln!("No valid symbol found for character '{}', using 'X' instead.", c);
        return Symbol::new_default_from_base_rc(symd.get_symbol_base("X").unwrap());
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

pub fn mixedcase_builder<'sd>(symdict: &'sd SymbolPathDict, prefix_upper: &str, prefix_lower: &str) -> SymbolStringBuilder<'sd> {
    let prefix_upper = prefix_upper.to_string();
    let prefix_lower = prefix_lower.to_string();
    SymbolStringBuilder::<'sd>::new(symdict, move |symd, c| {
        let prefix = if c.is_uppercase() { &prefix_upper } else { &prefix_lower };
        let prefix_alternative = if c.is_uppercase() { &prefix_lower } else { &prefix_upper };
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
            } else {
                // try the alternative prefix
                key = format!("{}_{}", prefix_alternative, c);
                let possible_keys = symd.symbols_with_prefix(key.as_str());
                if !possible_keys.is_empty() {
                    let selection = rand::random::<u32>() % possible_keys.len() as u32;
                    key = possible_keys[selection as usize].clone();
                }
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
