use scribe::journal::{Journal, JournalManifest, JournalPage};
use scribe::journal::rhaientries::{JournalEngine};
use scribe::journal::canvas::artistic::*;
use scribe::journal::{symboldict, darkly};
use scribe::journal::canvas::{Canvas, StrokeStyle, CanvasShape};
use scribe::journal::styles::{ternary_builder};
use std::collections::HashMap;
use scribe::journal::canvas::textbox::{TextBox, TextBoxOptions, TextBoxLineMode};
use scribe::journal::styles;
use parry2d::math::{Vec2};
use parry2d::bounding_volume::Aabb;
use rand::{Fill};

use std::sync::Arc;
use ::darkly::engine::DarklyEngine;

type JournalEntryFunc = for<'j> fn(&'j mut Journal<'j>, &'j symboldict::SymbolPathDict) -> JournalManifest<'j>;

fn journal_entry_17<'j>(journal: &'j mut Journal<'j>, symd: &'j symboldict::SymbolPathDict) -> JournalManifest<'j> {
    journal.add_symboldict(&symd);
    journal.build_entries(|journal, _, entry| {

        let maze_canvas = build_maze_canvas_from_entry(&entry, &symd);
        journal.add_page(
            "maze".to_string(),
            JournalPage::CanvasIllustration(maze_canvas),
            entry.canvas_options.clone(),
        );

        let bt_canvas = build_balanced_ternary_canvas(&symd);
        journal.add_page(
            "balanced_ternary".to_string(),
            JournalPage::SplatMsgsOnCanvas(bt_canvas, entry.shorts.clone(), 30.0, 2.0, "darkred".to_string()),
            entry.canvas_options.clone(),
        );
    })
}

fn journal_entry_18<'j>(journal: &'j mut Journal<'j>, symd: &'j symboldict::SymbolPathDict) -> JournalManifest<'j> {
    let textbox_width = 150.0;
    let textbox_height = 150.0;
    let symbol_scale = 19.0;
    journal.add_symboldict(&symd);
    journal.build_entries(move |journal, entry_name, entry| {
        let builder = styles::lowercase_builder(&symd, "loche_upper");
        let fateful_pages = entry.named_entries.get("fateful_pages")
            .expect("Entry 'fateful_pages' not found in journal entry");
        let spider_web = entry.named_entries.get("spider_web")
            .expect("Entry 'spider_web' not found in journal entry");

        let fateful_symbols = builder.build(fateful_pages);

        let spider_symbols = builder.build(spider_web);

        let bounds = Aabb::new(
            Vec2::new(-textbox_width / 2.0, -textbox_height / 2.0),
            Vec2::new(textbox_width / 2.0, textbox_height / 2.0),
        );
        let textbox_opts = TextBoxOptions {
            jagged_edge: 0.5,
            layout: TextBoxLineMode::WordsByBlankSymbol,
            bounds,
            symbol_scale,
            avoid_tree: None,
        };

        let textbox = TextBox::new(textbox_opts);
        let fateful_sym_set = textbox.place_all_symbols(fateful_symbols);
        let spider_sym_set = textbox.place_all_symbols(spider_symbols);
        let total_count = (fateful_sym_set.len() + spider_sym_set.len()) as f32 / 2.0;
        let canvas_count = total_count.sqrt().ceil() as usize;

        #[derive(Clone, Default)]
        struct Orientation(u32);
        impl Fill for Orientation {
            fn fill<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) -> () {
                self.0 = rng.random_range(0..4);
            }
        }
        let built_maze = BuiltMaze::<Orientation>::new(1043, canvas_count as u32, canvas_count as u32);
        let maze_iter = built_maze.iter_inners(MazeTraversalStrategy::LeftHanded);
        let mut fateful_iter = fateful_sym_set.into_iter().map(|sym| (sym, "black"));
        let mut spider_iter = spider_sym_set.into_iter().map(|sym| (sym, "hsl(160, 50%, 40%, 0.8)"));
        let mut page_canvas = Canvas::new();
        for ((x, y), _orientation) in maze_iter {
            let first_choice = rand::random::<bool>();
            if let Some((next_block, color)) = if first_choice {
                fateful_iter.next().or_else(|| spider_iter.next())
            } else {
                spider_iter.next().or_else(|| fateful_iter.next())
            } {
                for mut symbol in next_block {
                    // translate the symbol by textbox_width * x, textbox_height * y
                    symbol.translate_by(Vec2::new(
                        (textbox_width + symbol_scale) * x as f32,
                        (textbox_height + symbol_scale) * y as f32,
                    ));
                    page_canvas.add_strokes(symbol, StrokeStyle {
                        color: color.to_string(),
                        width: 2.0,
                    });
                }
            }
        }
        journal.add_page(
            format!("{}.maze_textbox", entry_name),
            JournalPage::SplatMsgsOnCanvas(page_canvas, entry.shorts.clone(), 23.0, 2.0, "hsl(200, 50%, 40%, 0.8)".to_string()),
            entry.canvas_options.clone(),
        );
    })
}

fn first_codex<'j>(journal: &'j mut Journal<'j>, symd: &'j symboldict::SymbolPathDict) -> JournalManifest<'j> {
    let builder = ternary_builder(&symd, "balanced_ternary");
    let symbol_scale = 30.0;
    let symbol_rings = [2, 3, 5, 7, 11, 13, 27];
    let mut base_canvas = Canvas::new();
    let symbol_set = "abcdefghijklmnopqrstuvwxyz";
    for (i, &ring_size) in symbol_rings.iter().enumerate() {
        let radius = 100.0 + i as f32 * (symbol_scale * 3.0);
        let circle = CanvasShape::Circle {
            center: Vec2::new(0.0, 0.0),
            radius,
        };
        let half_ring = ring_size / 2;
        let first_half = &symbol_set[0..half_ring];
        let second_half = &symbol_set[symbol_set.len() - half_ring..].chars().collect::<String>();
        let msg = format!(" {}{}", first_half, second_half);
        let symbols = builder.build(msg.as_str());
        let circle_count = 5;
        for _ in 0..circle_count {
            base_canvas.add_strokes(circle.clone(), StrokeStyle {
                color: "hsla(150, 30%, 50%, 0.1)".to_string(),
                width: 2.5,
            });
        }
        let drawn_canvas = symbols_in_circle(symbols, &SymbolCircleParams {
            rotation: i as f32 * 137.5_f32.to_radians(),
            center: Vec2::new(0.0, 0.0),
            radius: 100.0 + i as f32 * (symbol_scale * 3.0),
            style_options: Some(SymbolCircleStyleOptions{
                stroke_style: StrokeStyle {
                    color: "rgba(0, 0, 0, 0.7)".to_string(),
                    width: 2.5,
                },
                fill_style: Some(StrokeStyle {
                    color: "hsla(150, 30%, 70%, 0.3)".to_string(),
                    width: 2.0,
                }),
            }),
            symbol_scale,
        });
        base_canvas.merge_canvas(drawn_canvas);
    }
    journal.add_symboldict(&symd);
    journal.add_page(
        "first_codex".to_string(),
        JournalPage::CanvasIllustration(base_canvas),
        None,
    );
    journal.build_entries(|_, _, _| {})
}

const LEGACY_DARKLY_SIZE: f32 = 1080.0;

fn legacy_journalbuild(symd: &symboldict::SymbolPathDict, engine: &mut DarklyEngine, journal_dir: &str, journal_name: &str) {
    let journal_funcs: HashMap<&str, JournalEntryFunc> = HashMap::from([
        ("journal_entry_17", journal_entry_17 as JournalEntryFunc),
        ("journal_entry_18", journal_entry_18 as JournalEntryFunc),
        ("first_codex", first_codex as JournalEntryFunc),
    ]);

    let journal_path = format!("{}/{}/journal_entry.json", journal_dir, journal_name);
    let journal_data = std::fs::read_to_string(&journal_path)
        .expect(&format!("Failed to read {}", journal_path));
    let mut journal : Journal = serde_json::from_str(&journal_data)
        .expect("Failed to parse journal JSON");

    // everything before here is pretty generic, and so something that is
    // fine to stay in here. The behavior that I want to be able to customize
    // from a file is here on out.

    let journal_entry_func = journal_funcs.get(journal_name)
        .expect("Journal entry function not found for given journal name");
    let manifest = journal_entry_func(&mut journal, &symd);
    let dir_name = format!("{}/{}/output", journal_dir, journal_name);
    manifest.render_all(&dir_name, 2.0);

    let pages = manifest.pages();
    for (page_name, (page_canvas, _))  in pages {
        println!("Rendering with Darkly.");
        // Fit the longest side to LEGACY_DARKLY_SIZE, preserving aspect ratio.
        let dims = page_canvas.bounds().extents();
        let scale = LEGACY_DARKLY_SIZE / dims.x.max(dims.y);
        darkly::resize_canvas(engine, (dims.x * scale) as u32, (dims.y * scale) as u32);
        let buff = darkly::render_canvas(
            page_canvas,
            engine,
            "Calligraphy",
            1.0,
            (0.15, 0.25),
        );
        let filename = format!("{}/darkly_{}.png", dir_name, page_name);
        println!("\t - {}", filename);
        buff.save(filename).expect("Failed to save png");
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

fn main() {
    let symd = symboldict::SymbolPathDict::from_file("config/symbols.json")
        .expect("Failed to load symbols.json");
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <journal_name> [--legacy]", args[0]);
        std::process::exit(1);
    }

    let journal_name = &args[1];
    let journal_dir = "journals";
    let legacy_flagged = args.iter().find(|f| *f == "--legacy").is_some();
    // One Darkly engine for the whole run; canvases resize it as needed.
    let mut darkly_engine = darkly::new_engine(1, 1);
    if legacy_flagged {
        // TODO: eventually I intend to build out the legacy journals into rhai.
        legacy_journalbuild(&symd, &mut darkly_engine, journal_dir, journal_name);
        return;
    }
    let symdarc = Arc::new(symd);
    let mut engine = JournalEngine::new(symdarc);
    let entry_path = format!("{}/{}", journal_dir, journal_name);
    println!("EntryPath: {}", entry_path);
    let journal_path = format!("{}/journal_entry.rhai", entry_path);
    println!("JournalPath: {}", journal_path);
    let entry = engine
        .build_entry_file(&journal_path)
        .expect("Failed to execute result.");
    let output_dir = format!("{}/output", entry_path);
    create_dir(&output_dir);
    entry.clone().publish_pngs(&output_dir);
    entry.darkly_publish_pngs(&mut darkly_engine, &output_dir, 1.0);
}
