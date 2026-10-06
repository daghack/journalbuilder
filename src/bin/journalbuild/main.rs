use scribe::journal::rhaientries::{JournalEngine};
use scribe::journal::{symboldict, darkly};

use std::rc::Rc;

/// Create the output directory, or clear out the renders (`.png` and `.svg`
/// files) of earlier runs, so renamed or removed pages don't linger. Other
/// files are left alone.
fn prepare_output_dir(dir: &str) -> Result<(), String> {
    let io_error = |what: &str, e: std::io::Error| format!("couldn't {} {}: {}", what, dir, e);
    std::fs::create_dir_all(dir).map_err(|e| io_error("create", e))?;
    let mut cleared = 0;
    for entry in std::fs::read_dir(dir).map_err(|e| io_error("read", e))? {
        let path = entry.map_err(|e| io_error("read", e))?.path();
        let is_render = path.extension().is_some_and(|ext| ext == "png" || ext == "svg");
        if path.is_file() && is_render {
            std::fs::remove_file(&path).map_err(|e| format!("couldn't remove the earlier render {}: {}", path.display(), e))?;
            cleared += 1;
        }
    }
    if cleared > 0 {
        println!("Cleared {} earlier render(s) from {}", cleared, dir);
    }
    Ok(())
}

/// Report characters drawn with the fallback glyph; with `strict`, stop
/// before rendering instead.
fn check_missing_glyphs(symd: &symboldict::SymbolPathDict, strict: bool) -> Result<(), String> {
    if let Some(report) = symd.missing_glyph_report() {
        if strict {
            return Err(format!("{} (failing because of --strict-glyphs)", report));
        }
        eprintln!("warning: {}", report);
    }
    Ok(())
}

const DEFAULT_SUPERSAMPLE: f32 = 1.2;

const USAGE: &str = "Usage: journalbuild <journal_name> [--page N[,N...]] [--no-plain]
                    [--scale S] [--supersample K] [--strict-glyphs]
                    [--journals-dir DIR] [--output-dir DIR] [--symbols FILE]

  --page P,...        Render only these pages, by index (0-based) or by
                      name. Other pages' existing output is kept.
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
  --journals-dir DIR  Directory containing journals (default: the
                      repository's journals/).
  --symbols FILE      Symbol dictionary (default: the repository's
                      config/symbols.json).
  --output-dir DIR    Where to write renders (default: <journal>/output).
                      Earlier .png/.svg renders there are cleared first
                      (except with --page); other files are kept.";

struct Args {
    strict_glyphs: bool,
    journal_name: String,
    journals_dir: String,
    pages: Option<Vec<String>>,
    plain: bool,
    scale: f32,
    supersample: f32,
    output_dir: Option<String>,
    symbols: String,
}

fn parse_args() -> Result<Args, String> {
    let mut journal_name = None;
    let mut pages: Option<Vec<String>> = None;
    let mut plain = true;
    let mut strict_glyphs = false;
    let mut scale = 1.0;
    // Supersample by default: Darkly aliases brush marks thinner than about
    // a pixel into ragged edges. Measured on journal_entry_17, 1.2x costs
    // about 1.6x the render time (1.5x about 2.6x, 2x about 3.3x: mark area
    // grows with K²).
    let mut supersample = DEFAULT_SUPERSAMPLE;
    // Defaults are found from the repository, not the working directory.
    let mut journals_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/journals").to_string();
    let mut symbols = symboldict::DEFAULT_SYMBOLS.to_string();
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
            "--symbols" => {
                symbols = args.next().ok_or("--symbols needs a file")?;
            }
            "--output-dir" => {
                output_dir = Some(args.next().ok_or("--output-dir needs a directory")?);
            }
            "--page" => {
                let list = args.next().ok_or("--page needs a page or list, e.g. --page 0,2 or --page maze")?;
                let parsed = list.split(',').map(str::trim)
                    .map(|p| if p.is_empty() { Err(format!("empty page in '{}'", list)) } else { Ok(p.to_string()) })
                    .collect::<Result<Vec<_>, _>>()?;
                pages.get_or_insert_with(Vec::new).extend(parsed);
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option '{}'", flag)),
            name if journal_name.is_none() => journal_name = Some(name.to_string()),
            extra => return Err(format!("unexpected argument '{}'", extra)),
        }
    }
    let journal_name = journal_name.ok_or("missing journal name")?;
    Ok(Args { strict_glyphs, journal_name, journals_dir, pages, plain, scale, supersample, output_dir, symbols })
}

fn main() {
    let args = parse_args().unwrap_or_else(|err| {
        eprintln!("error: {}\n\n{}", err, USAGE);
        std::process::exit(1);
    });
    if let Err(err) = run(&args) {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), String> {
    let symd = symboldict::SymbolPathDict::from_file(&args.symbols)
        .map_err(|err| format!("couldn't load the symbol dictionary {}: {}", args.symbols, err))?;
    let symd = Rc::new(symd);

    let journal_name = &args.journal_name;
    let journal_path = format!("{}/{}/journal_entry.rhai", args.journals_dir, journal_name);
    let output_dir = args.output_dir.clone()
        .unwrap_or_else(|| format!("{}/{}/output", args.journals_dir, journal_name));
    let entry = JournalEngine::new(symd.clone()).build_entry_file(&journal_path)?;
    check_missing_glyphs(&symd, args.strict_glyphs)?;
    if entry.page_count() == 0 {
        return Err(format!("{} produced no pages; add them with journal_entry.add_page(page)", journal_path));
    }
    let empty = entry.empty_canvases();
    if !empty.is_empty() {
        eprintln!("warning: skipping {} empty canvas(es), with nothing drawn on them: {}", empty.len(), empty.join(", "));
    }
    let pages: Option<Vec<usize>> = match &args.pages {
        None => None,
        Some(pages) => Some(pages.iter().map(|page| entry.page_index(page))
            .collect::<Result<_, _>>()
            .map_err(|err| format!("{} (in {})", err, journal_name))?),
    };
    if pages.is_some() {
        // Rendering a subset: keep the other pages' existing output.
        std::fs::create_dir_all(&output_dir).map_err(|e| format!("couldn't create {}: {}", output_dir, e))?;
    } else {
        prepare_output_dir(&output_dir)?;
    }
    let only = pages.as_deref();
    if args.plain {
        entry.publish_pngs(&output_dir, args.scale, only)?;
    }
    // One Darkly engine for the whole run; canvases resize it as needed.
    let mut darkly_engine = darkly::new_engine(1, 1);
    entry.darkly_publish_pngs(&mut darkly_engine, &output_dir, args.scale, args.supersample, only)
}
