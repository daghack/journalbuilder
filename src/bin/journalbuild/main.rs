use scribe::journal::{Journal, JournalManifest, JournalPage};
use scribe::journal::rhaientries::{JournalEngine};
use scribe::journal::canvas::artistic::*;
use scribe::journal::{symboldict, darkly};
use scribe::journal::canvas::{Brush, Canvas, StrokeStyle};
use std::collections::HashMap;
use scribe::journal::canvas::textbox::{TextBox, TextBoxOptions, TextBoxLineMode};
use scribe::journal::styles;
use parry2d::math::{Vec2};
use parry2d::bounding_volume::Aabb;
use rand::{Fill};

use std::sync::Arc;
use ::darkly::engine::DarklyEngine;

type JournalEntryFunc = for<'j> fn(&'j mut Journal<'j>, &'j symboldict::SymbolPathDict) -> JournalManifest<'j>;

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

        let rng: &mut dyn rand::RngCore = &mut rand::rng();
        let fateful_symbols = builder.build(fateful_pages, rng);

        let spider_symbols = builder.build(spider_web, rng);

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
        let fateful_sym_set = textbox.place_all_symbols(fateful_symbols, rng);
        let spider_sym_set = textbox.place_all_symbols(spider_symbols, rng);
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
                        brush: None,
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

const LEGACY_DARKLY_SIZE: f32 = 1080.0;

fn legacy_journalbuild(symd: &symboldict::SymbolPathDict, engine: &mut DarklyEngine, journal_dir: &str, journal_name: &str, dir_name: &str, plain: bool, scale: f32, supersample: f32, strict_glyphs: bool) {
    let journal_funcs: HashMap<&str, JournalEntryFunc> = HashMap::from([
        ("journal_entry_18", journal_entry_18 as JournalEntryFunc),
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
    check_missing_glyphs(symd, strict_glyphs);
    if plain {
        manifest.render_all(dir_name, 2.0 * scale);
    } else {
        create_dir(dir_name);
    }

    let pages = manifest.pages();
    // Legacy journals don't set per-stroke brushes, so everything uses this.
    let default_brush = Brush {
        name: "Calligraphy".to_string(),
        // Darkly's dev Calligraphy treats size as the nib's long axis (it
        // was 2.5x that under v0.7.0's area-preserving aspect), so 1.75 here
        // matches the old 0.7 (measured: same ink coverage within 0.1%).
        size: 1.75,
        min_pressure: 0.15,
        max_pressure: 0.25,
    };
    for (page_name, (page_canvas, _))  in pages {
        println!("Rendering with Darkly.");
        // Fit the longest side to LEGACY_DARKLY_SIZE, preserving aspect ratio.
        let dims = page_canvas.bounds().extents();
        let fit = LEGACY_DARKLY_SIZE * scale / dims.x.max(dims.y);
        // Brush sizes are relative to the LEGACY_DARKLY_SIZE render, so
        // they scale with --scale.
        let buff = darkly::render_canvas_scaled(
            page_canvas, engine, Some(&default_brush),
            (dims.x * fit) as u32, (dims.y * fit) as u32,
            scale, supersample,
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

/// Report characters drawn with the fallback glyph; with `strict`, stop
/// before rendering instead.
fn check_missing_glyphs(symd: &symboldict::SymbolPathDict, strict: bool) {
    if let Some(report) = symd.missing_glyph_report() {
        if strict {
            eprintln!("error: {} (failing because of --strict-glyphs)", report);
            std::process::exit(1);
        }
        eprintln!("warning: {}", report);
    }
}

const DEFAULT_SUPERSAMPLE: f32 = 1.2;

const USAGE: &str = "Usage: journalbuild <journal_name> [--legacy] [--page N[,N...]] [--no-plain]
                    [--scale S] [--supersample K] [--strict-glyphs]
                    [--journals-dir DIR] [--output-dir DIR]

  --legacy            Build a legacy JSON journal instead of a rhai one.
  --page N,...        Render only these rhai pages (0-based). Other pages'
                      existing output is kept.
  --no-plain          Skip the plain PNG/SVG output; only render with Darkly.
  --strict-glyphs     Fail before rendering if any character had no glyph
                      (by default they're drawn as X and listed as a warning).
  --scale S           Scale output images (default 1.0), brushes included.
                      Darkly work scales with output size, so e.g. 0.25
                      renders much faster.
  --supersample K     Render Darkly output at K times the size and filter it
                      back down (default 1.2; any number >= 1, and 1 turns
                      it off). Smooths thin, aliased strokes; render time
                      grows with K squared.
  --journals-dir DIR  Directory containing journals (default: journals).
  --output-dir DIR    Where to write renders (default: <journal>/output).";

struct Args {
    strict_glyphs: bool,
    journal_name: String,
    journals_dir: String,
    legacy: bool,
    pages: Option<Vec<usize>>,
    plain: bool,
    scale: f32,
    supersample: f32,
    output_dir: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut journal_name = None;
    let mut legacy = false;
    let mut pages: Option<Vec<usize>> = None;
    let mut plain = true;
    let mut strict_glyphs = false;
    let mut scale = 1.0;
    // Supersample by default: Darkly aliases brush marks thinner than about
    // a pixel into ragged edges. Measured on journal_entry_17, 1.2x costs
    // about 1.6x the render time (1.5x about 2.6x, 2x about 3.3x: mark area
    // grows with K²).
    let mut supersample = DEFAULT_SUPERSAMPLE;
    let mut journals_dir = "journals".to_string();
    let mut output_dir = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--legacy" => legacy = true,
            "--no-plain" => plain = false,
            "--strict-glyphs" => strict_glyphs = true,
            "--scale" => {
                let value = args.next().ok_or("--scale needs a value, e.g. --scale 0.25")?;
                scale = value.parse::<f32>().ok().filter(|s| *s > 0.0)
                    .ok_or(format!("invalid scale '{}'; expected a positive number", value))?;
            }
            "--supersample" => {
                let value = args.next().ok_or("--supersample needs a value, e.g. --supersample 2")?;
                supersample = value.parse::<f32>().ok().filter(|k| *k >= 1.0)
                    .ok_or(format!("invalid supersample '{}'; expected a number >= 1, e.g. 1.5", value))?;
            }
            "--journals-dir" => {
                journals_dir = args.next().ok_or("--journals-dir needs a directory")?;
            }
            "--output-dir" => {
                output_dir = Some(args.next().ok_or("--output-dir needs a directory")?);
            }
            "--page" => {
                let list = args.next().ok_or("--page needs a page number or list, e.g. --page 0,2")?;
                let parsed = list.split(',')
                    .map(|n| n.trim().parse::<usize>().map_err(|_| format!("invalid page number '{}'", n)))
                    .collect::<Result<Vec<_>, _>>()?;
                pages.get_or_insert_with(Vec::new).extend(parsed);
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option '{}'", flag)),
            name if journal_name.is_none() => journal_name = Some(name.to_string()),
            extra => return Err(format!("unexpected argument '{}'", extra)),
        }
    }
    let journal_name = journal_name.ok_or("missing journal name")?;
    if legacy && pages.is_some() {
        return Err("--page is only supported for rhai journals".to_string());
    }
    Ok(Args { strict_glyphs, journal_name, journals_dir, legacy, pages, plain, scale, supersample, output_dir })
}

fn main() {
    let args = parse_args().unwrap_or_else(|err| {
        eprintln!("error: {}\n\n{}", err, USAGE);
        std::process::exit(1);
    });
    let symd = symboldict::SymbolPathDict::from_file("config/symbols.json")
        .expect("Failed to load symbols.json");

    let journal_name = &args.journal_name;
    let journal_dir = args.journals_dir.as_str();
    let output_dir = args.output_dir.clone()
        .unwrap_or_else(|| format!("{}/{}/output", journal_dir, journal_name));
    // One Darkly engine for the whole run; canvases resize it as needed.
    let mut darkly_engine = darkly::new_engine(1, 1);
    if args.legacy {
        // TODO: eventually I intend to build out the legacy journals into rhai.
        legacy_journalbuild(&symd, &mut darkly_engine, journal_dir, journal_name, &output_dir, args.plain, args.scale, args.supersample, args.strict_glyphs);
        return;
    }
    let symdarc = Arc::new(symd);
    let mut engine = JournalEngine::new(symdarc.clone());
    let entry_path = format!("{}/{}", journal_dir, journal_name);
    println!("EntryPath: {}", entry_path);
    let journal_path = format!("{}/journal_entry.rhai", entry_path);
    println!("JournalPath: {}", journal_path);
    let entry = engine
        .build_entry_file(&journal_path)
        .expect("Failed to execute result.");
    check_missing_glyphs(&symdarc, args.strict_glyphs);
    if let Some(pages) = &args.pages {
        if let Some(bad) = pages.iter().find(|&&p| p >= entry.page_count()) {
            eprintln!("error: page {} doesn't exist; {} has {} page(s) (0-{})",
                bad, journal_name, entry.page_count(), entry.page_count().saturating_sub(1));
            std::process::exit(1);
        }
    }
    if args.pages.is_some() {
        // Rendering a subset: keep the other pages' existing output.
        std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");
    } else {
        create_dir(&output_dir);
    }
    let only = args.pages.as_deref();
    if args.plain {
        entry.publish_pngs(&output_dir, args.scale, only);
    }
    entry.darkly_publish_pngs(&mut darkly_engine, &output_dir, args.scale, args.supersample, only);
}
