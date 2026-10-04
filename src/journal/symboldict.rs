use super::paths::StrokePath;
use super::symbol::{Symbol, SymbolBase, SymbolType};

use parry2d::math::Vec2;

use std::collections::HashMap;
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
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SymbolSetPath {
    adj: Option<AdjType>,
    path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SymbolSet {
    pub set_name: String,
    pub set_description: String,
    pub ordered_symbols: Vec<String>,
    pub symbol_paths: Vec<SymbolSetPath>,
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
            let mut stroke_path = StrokePath::from_str(&symbol.path);
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
        })
    }
    pub fn symbols_with_prefix(&self, prefix: &str) -> Vec<String> {
        self.symbols.keys().filter(|k| k.starts_with(prefix)).cloned().collect()
    }
    pub fn get_strokepath(&self, name: &str) -> Option<StrokePath> {
        self.symbols.get(name).cloned()
    }
    pub fn get_symbol_base(&self, name: &str) -> Option<Rc<SymbolBase>> {
        self.symbol_base.get(name).cloned()
    }
    pub fn keys(&self) -> Vec<String> {
        self.symbols.keys().cloned().collect()
    }
    pub fn new_symbol(&self, name: &str) -> Option<Symbol> {
        let origin = Vec2::new(0.0, 0.0);
        self.get_strokepath(name).map(|path| {
            Symbol::new(path, 1.0, origin, 0.0)
        })
    }
}
