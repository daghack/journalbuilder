pub mod canvas;
pub mod symbol;
pub mod paths;
pub mod styles;
pub mod symboldict;
pub mod img;
pub mod parse;
pub mod rhaientries;
pub mod darkly;

pub use paths::StrokePath;

use serde::{Deserialize};
use std::rc::Rc;
use std::collections::HashMap;

use crate::composite_from_locktypes;
use crate::lockbuilder::*;
use crate::lockbuilder::locks::maze::{MazeLock};
use crate::lockbuilder::locks::doublering::{DoubleRingLock, DoubleRingOptions, KeyedDoubleRingLock, KeyedDoubleRingOptions};
use crate::lockbuilder::locks::util::mergealpha::{MergeAlphaInputs, MergeAlpha26Options};
use crate::lockbuilder::locks::util::splitsymbols::{SplitSymbolsLock, SplitSymbolsOptions};
use crate::lockbuilder::locks::shift::{ShiftCipherLock, ShiftCipherOptions, VignCipherLock, VignCipherOptions};
use crate::lockbuilder::locks::huffman::{HuffmanEncodingLock, HuffmanEncodingOptions};
use crate::lockbuilder::locks::substitution::{SubstitutionCipherLock, SubstitutionCipherOptions};
use crate::journal::canvas::{Canvas, CanvasStyleOptions};

use crate::journal::canvas::artistic::{MazeOptions, draw_msgs_overtop};

#[derive(Debug, Clone)]
pub struct JournalEntry {
    pub shorts: Vec<String>,
    pub canvas_options: Option<CanvasStyleOptions>,
    pub named_entries: HashMap<String, String>,
    pub lock_entries: HashMap<String, CompositeLock>,
}

impl<'de> Deserialize<'de> for JournalEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct JournalEntryHelper {
            pub shorts: Vec<String>,
            pub named_entries: HashMap<String, String>,
            pub journal_locks: HashMap<String, NamelessLockConfig>,
            pub canvas_options: Option<CanvasStyleOptions>,
        }
        let helper = JournalEntryHelper::deserialize(deserializer)?;
        let mut lock_entries = HashMap::new();
        for (key, value) in helper.journal_locks.into_iter() {
            let mut lock_config = value.add_name(key.clone());
            for (static_key, static_input) in lock_config.static_inputs.iter_mut() {
                // if the static input begins with "entry:", replace it
                // with the corresponding named entry
                if static_input.starts_with("entry:") {
                    let entry_name = &static_input[6..];
                    if let Some(entry_value) = helper.named_entries.get(entry_name) {
                        *static_input = entry_value.clone();
                    } else {
                        return Err(serde::de::Error::custom(format!("Named entry '{}' not found for static input '{}'", entry_name, static_key)));
                    }
                }
            }
            let mut lock = composite_from_locktypes!(lock_config,
                "shift_cipher", ShiftCipherLock, ShiftCipherOptions,
                "vigenere_cipher", VignCipherLock, VignCipherOptions,
                "huffman_encoding", HuffmanEncodingLock, HuffmanEncodingOptions,
                "doublering", DoubleRingLock, DoubleRingOptions,
                "keyed_doublering", KeyedDoubleRingLock, KeyedDoubleRingOptions,
                "merge_alpha", MergeAlphaInputs, MergeAlpha26Options,
                "split_symbols", SplitSymbolsLock, SplitSymbolsOptions,
                "substitution_cipher", SubstitutionCipherLock, SubstitutionCipherOptions,
                "maze_lock", MazeLock, MazeOptions
            ).expect("Failed to create lock from config");
            for (key, input_var_type) in lock.inputs().into_iter() {
                // if the key exists in the named_entries,
                // add it as a static input (after parsing)
                if let Some(entry_value) = helper.named_entries.get(&key) {
                    let entry_value: InfoVar = entry_value.clone().into();
                    if input_var_type != entry_value.var_type {
                        return Err(serde::de::Error::custom(format!(
                            "Type mismatch for static input '{}': expected {:?}, got {:?}",
                            key, input_var_type, entry_value.var_type
                        )));
                    }
                    lock.add_static_input(key, entry_value.clone().into());
                }
            }
            lock_entries.insert(key, lock);
        }
        Ok(JournalEntry {
            shorts: helper.shorts,
            named_entries: helper.named_entries,
            canvas_options: helper.canvas_options,
            lock_entries,
        })
    }
}

#[derive(Debug)]
pub struct JournalManifest<'p> {
    pub pages: &'p HashMap<String, (Canvas, Option<CanvasStyleOptions>)>,
}

impl<'p> JournalManifest<'p> {
    pub fn pages(&self) -> &'p HashMap<String, (Canvas, Option<CanvasStyleOptions>)> {
        self.pages
    }
    pub fn render_to_pngs(&self, dir: &str, scale: f32) {
        Self::create_dir(dir);
        for (page_name, (canvas, style_opts)) in self.pages.iter() {
            let filename = format!("{}/{}.png", dir, page_name);
            let png_data = canvas.to_png(scale, style_opts.as_ref());
            std::fs::write(filename, png_data).expect("Failed to write PNG file");
        }
    }
    pub fn render_to_svgs(&self, dir: &str) {
        Self::create_dir(dir);
        for (page_name, (canvas, style_opts)) in self.pages.iter() {
            let filename = format!("{}/{}.svg", dir, page_name);
            let svg_data = canvas.to_svg(style_opts.as_ref());
            std::fs::write(filename, svg_data).expect("Failed to write SVG file");
        }
    }
    fn create_dir(dir: &str) {
        if !std::path::Path::new(dir).exists() {
            std::fs::create_dir_all(dir).expect("Failed to create output directory");
        } else {
            // clear out existing files in the directory
            for entry in std::fs::read_dir(dir).expect("Failed to read output directory") {
                let entry = entry.expect("Failed to read directory entry");
                let path = entry.path();
                if path.is_file() {
                    std::fs::remove_file(path).expect("Failed to remove existing file in output directory");
                }
            }
        }
    }
    pub fn render_all(&self, dir: &str, scale: f32) {
        println!("Rendering journal pages to directory: {}", dir);
        Self::create_dir(dir);
        for (page_name, (canvas, style_opts)) in self.pages.iter() {
            println!("Rendering page: {}", page_name);
            let svg_file = format!("{}/{}.svg", dir, page_name);
            println!(" - Generating SVG: {}", svg_file);
            let svg_data = canvas.to_svg(style_opts.as_ref());
            let png_file = format!("{}/{}.png", dir, page_name);
            println!(" - Generating PNG: {}", png_file);
            let svg_tree = usvg::Tree::from_str(&svg_data, &usvg::Options::default()).expect("Failed to parse SVG data");
            let mut png_pixmap = img::pixmap_from_svg_tree(&svg_tree, scale, scale);
            if let Some(style_opts) = style_opts {
                Canvas::stylize_pixmap(&mut png_pixmap, style_opts);
            }
            let png_data = png_pixmap.encode_png().expect("Failed to encode PNG data");
            println!(" - Writing files: {}, {}", svg_file, png_file);
            std::fs::write(svg_file, svg_data).expect("Failed to write SVG file");
            std::fs::write(png_file, png_data).expect("Failed to write PNG file");
        }
    }
}

#[derive(Debug)]
pub struct Journal<'sd> {
    entries: HashMap<String, JournalEntry>,
    pages: HashMap<String, (Canvas, Option<CanvasStyleOptions>)>,
    symd: Option<&'sd symboldict::SymbolPathDict>,
}

impl<'sd> Journal<'sd> {
    pub fn new(symd: &'sd symboldict::SymbolPathDict) -> Self {
        Journal {
            entries: HashMap::new(),
            pages: HashMap::new(),
            symd: Some(symd),
        }
    }
    pub fn pages(&self) -> &HashMap<String, (Canvas, Option<CanvasStyleOptions>)> {
        &self.pages
    }
    fn add_page_entry(&mut self, page_name: String, page: Canvas, style: Option<CanvasStyleOptions>) {
        self.pages.insert(page_name, (page, style));
    }
    pub fn build_entries(&mut self, entry_func: impl Fn(&mut Self, &str, &JournalEntry)) -> JournalManifest<'_> {
        let entry_keys: Vec<String> = self.entries.keys().cloned().collect();
        for entry_name in entry_keys.into_iter() {
            let entry = self.entries.get(&entry_name).expect("Journal entry not found").clone();
            entry_func(self, &entry_name, &entry);
            println!("Built journal entry: {}", entry_name);
        }
        JournalManifest {
            pages: &self.pages,
        }
    }
    pub fn add_symboldict(&mut self, symd: &'sd symboldict::SymbolPathDict) {
        self.symd = Some(symd);
    }
    pub fn add_page(&mut self, page_name: String, page: JournalPage, style: Option<CanvasStyleOptions>) {
        if self.symd.is_none() {
            panic!("Cannot add page without symboldict set");
        }
        let symd = self.symd.unwrap();
        match page {
            JournalPage::SplatMsgsOnCanvas(mut canvas, msgs, sym_scale, width, color) => {
                draw_msgs_overtop(&mut canvas, symd, &msgs, (200.0, 500.0), (300.0, 600.0), sym_scale, width, &color);
                self.add_page_entry(page_name, canvas, style);
            },
            JournalPage::CanvasIllustration(canvas) => {
                self.add_page_entry(page_name, canvas, style);
            },
        }
    }
}

impl<'de> Deserialize<'de> for Journal<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let entries = HashMap::<String, JournalEntry>::deserialize(deserializer)?;
        Ok(Journal {
            symd: None,
            pages: HashMap::new(),
            entries: entries,
        })
    }
}

pub enum JournalPage{
    CanvasIllustration(Canvas),
    SplatMsgsOnCanvas(Canvas, Vec<String>, f32, f32, String),
}
