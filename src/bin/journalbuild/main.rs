use scribe::journal::rhaientries::{JournalEngine};
use scribe::journal::{symboldict, darkly};

use std::sync::Arc;

/// Create the output directory, or clear out the renders (`.png` and `.svg`
/// files) of earlier runs, so renamed or removed pages don't linger. Other
/// files are left alone.
fn prepare_output_dir(dir: &str) {
    std::fs::create_dir_all(dir).expect("Failed to create output directory");
    let mut cleared = 0;
    for entry in std::fs::read_dir(dir).expect("Failed to read output directory") {
        let path = entry.expect("Failed to read directory entry").path();
        let is_render = path.extension().is_some_and(|ext| ext == "png" || ext == "svg");
        if path.is_file() && is_render {
            std::fs::remove_file(&path).expect("Failed to remove an earlier render");
            cleared += 1;
        }
    }
    if cleared > 0 {
        println!("Cleared {} earlier render(s) from {}", cleared, dir);
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

const USAGE: &str = "Usage: journalbuild <journal_name> [--page N[,N...]] [--no-plain]
                    [--scale S] [--supersample K] [--strict-glyphs]
                    [--journals-dir DIR] [--output-dir DIR]

  --page N,...        Render only these pages (0-based). Other pages'
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
  --output-dir DIR    Where to write renders (default: <journal>/output).
                      Earlier .png/.svg renders there are cleared first
                      (except with --page); other files are kept.";

struct Args {
    strict_glyphs: bool,
    journal_name: String,
    journals_dir: String,
    pages: Option<Vec<usize>>,
    plain: bool,
    scale: f32,
    supersample: f32,
    output_dir: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut journal_name = None;
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
    Ok(Args { strict_glyphs, journal_name, journals_dir, pages, plain, scale, supersample, output_dir })
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
        prepare_output_dir(&output_dir);
    }
    let only = args.pages.as_deref();
    if args.plain {
        entry.publish_pngs(&output_dir, args.scale, only);
    }
    entry.darkly_publish_pngs(&mut darkly_engine, &output_dir, args.scale, args.supersample, only);
}
