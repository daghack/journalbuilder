use scribe::journal::rhaientries::{JournalEngine, LockLibrary, Sources, DEFAULT_CHAINS_DIR, DEFAULT_LAYOUTS_DIR, DEFAULT_LOCKS_DIR, DEFAULT_RENDERERS_DIR};
use scribe::journal::{fonts, darkly, publish};
use scribe::journal::progress::{clock, Progress};
use std::cell::RefCell;

use std::rc::Rc;

/// Create the output directory, or clear out the renders (`.png`, `.svg`
/// and `.pdf` files) of earlier runs, so renamed or removed pages don't
/// linger. Other files are left alone.
fn prepare_output_dir(dir: &str) -> Result<(), String> {
    let io_error = |what: &str, e: std::io::Error| format!("couldn't {} {}: {}", what, dir, e);
    std::fs::create_dir_all(dir).map_err(|e| io_error("create", e))?;
    let mut cleared = 0;
    for entry in std::fs::read_dir(dir).map_err(|e| io_error("read", e))? {
        let path = entry.map_err(|e| io_error("read", e))?.path();
        let is_render = path.extension().is_some_and(|ext| ext == "png" || ext == "svg" || ext == "pdf");
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
fn check_missing_glyphs(symd: &fonts::FontBook, strict: bool) -> Result<(), String> {
    if let Some(report) = symd.missing_glyph_report() {
        if strict {
            return Err(format!("{} (failing because of --strict-glyphs)", report));
        }
        eprintln!("warning: {}", report);
    }
    Ok(())
}

const DEFAULT_SUPERSAMPLE: f32 = 1.2;

const USAGE: &str = "Usage: journalbuild <journal_name> [--page N[,N...]] [--svg]
                    [--scale S] [--supersample K] [--strict-glyphs]
                    [--parchment] [--pdf] [--stitch N]
                    [--journals-dir DIR] [--output-dir DIR] [--glyphsets PATH]
                    [--locks-dir DIR] [--layouts-dir DIR] [--renderers-dir DIR]
                    [--chains-dir DIR]
       journalbuild --check-locks [--locks-dir DIR] [--chains-dir DIR]
       journalbuild --check-gpu

  --page P,...        Render only these pages, by index (0-based) or by
                      name. Other pages' existing output is kept.
  --svg               Also render each canvas as an SVG ({page}_{canvas}.svg)
                      and that SVG as a PNG; by default only Darkly renders.
                      Stroke widths are geometric widths there, while in
                      Darkly they scale brush pressure.
  --parchment         Render each page as one image on parchment,
                      darkly_{page}.png, its canvases layered in order: a
                      mottled scroll browning toward its edges. Pages get
                      a border of parchment round their strokes; each tile
                      of a split page is a sheet of its own. Light inks are
                      drawn dark, so they show.
  --pdf               Put the pages in one PDF, {journal}.pdf, a page per
                      page, rather than writing images: each page's
                      canvases layered into one image (on parchment,
                      with --parchment), 150 canvas units to the inch.
  --stitch N          Also lay every N pages side by side, in order, tops
                      level, named for the pages they hold: page_0 to
                      page_5 as darkly_page_0-5.png (the last may have
                      fewer; a page left alone isn't stitched). Pages are rendered as one image
                      each, darkly_{page}.png, as with --parchment (which
                      it goes with, on parchment, or without, on nothing).
                      Not with --pdf.
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
                      repository's webapp/journals/).
  --glyphsets PATH    Glyphset files: a directory with a NAME.json for each
                      glyphset, or one file with every glyphset (default:
                      the repository's webapp/fonts/).
  --output-dir DIR    Where to write renders (default: NAME_output in the current
                      directory, emptied first unless --page). In a given
                      directory, earlier .png/.svg/.pdf renders are cleared
                      first (except with --page); other files are kept.
  --locks-dir DIR     Directory of lock files, one per lock type (default:
                      the repository's webapp/locks/).
  --layouts-dir DIR   Directory of page layout files, for layout(name)
                      (default: the repository's webapp/layouts/).
  --renderers-dir DIR Directory of renderer files, for lock.attach
                      (default: the repository's webapp/renderers/).
  --chains-dir DIR    Directory of chain files, for chain(name) (default:
                      the repository's webapp/chains/).
  --check-locks       Run every lock file's examples, and every chain
                      file's, through lock and unlock, instead of building
                      a journal.
  --check-gpu         Check that Darkly can render (there's a GPU wgpu can
                      use), instead of building a journal; fails if not.
                      Without one, --svg renders still work, and cargo
                      test checks them (tests/svg_renders.rs).";

struct Args {
    strict_glyphs: bool,
    check_locks: bool,
    check_gpu: bool,
    journal_name: String,
    journals_dir: String,
    pages: Option<Vec<String>>,
    svg: bool,
    scale: f32,
    supersample: f32,
    output_dir: Option<String>,
    font_file: String,
    locks_dir: String,
    layouts_dir: String,
    renderers_dir: String,
    chains_dir: String,
    parchment: bool,
    pdf: bool,
    stitch: Option<usize>,
}

fn parse_args() -> Result<Args, String> {
    let mut journal_name = None;
    let mut pages: Option<Vec<String>> = None;
    let mut svg = false;
    let mut pdf = false;
    let mut strict_glyphs = false;
    let mut check_locks = false;
    let mut check_gpu = false;
    let mut scale = 1.0;
    // Supersample by default: Darkly aliases brush marks thinner than about
    // a pixel into ragged edges. Measured on journal_entry_17, 1.2x costs
    // about 1.6x the render time (1.5x about 2.6x, 2x about 3.3x: mark area
    // grows with K²).
    let mut supersample = DEFAULT_SUPERSAMPLE;
    // Defaults are found from the repository, not the working directory.
    let mut journals_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/webapp/journals").to_string();
    let mut font_file = fonts::DEFAULT_FONTS.to_string();
    let mut locks_dir = DEFAULT_LOCKS_DIR.to_string();
    let mut layouts_dir = DEFAULT_LAYOUTS_DIR.to_string();
    let mut renderers_dir = DEFAULT_RENDERERS_DIR.to_string();
    let mut chains_dir = DEFAULT_CHAINS_DIR.to_string();
    let mut output_dir = None;
    let mut parchment = false;
    let mut stitch = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--svg" => svg = true,
            "--pdf" => pdf = true,
            "--strict-glyphs" => strict_glyphs = true,
            "--check-locks" => check_locks = true,
            "--check-gpu" => check_gpu = true,
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
            "--parchment" => parchment = true,
            "--stitch" => {
                let value = args.next().ok_or("--stitch needs how many pages to a stitch, e.g. --stitch 6")?;
                stitch = Some(value.parse::<usize>().ok().filter(|n| *n >= 1)
                    .ok_or_else(|| format!("--stitch needs a whole number of at least 1, got '{}'", value))?);
            }
            "--journals-dir" => {
                journals_dir = args.next().ok_or("--journals-dir needs a directory")?;
            }
            "--glyphsets" => {
                font_file = args.next().ok_or("--glyphsets needs a path")?;
            }
            "--locks-dir" => {
                locks_dir = args.next().ok_or("--locks-dir needs a directory")?;
            }
            "--layouts-dir" => {
                layouts_dir = args.next().ok_or("--layouts-dir needs a directory")?;
            }
            "--renderers-dir" => {
                renderers_dir = args.next().ok_or("--renderers-dir needs a directory")?;
            }
            "--chains-dir" => {
                chains_dir = args.next().ok_or("--chains-dir needs a directory")?;
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
    if check_gpu {
        if journal_name.is_some() || check_locks {
            return Err("--check-gpu only checks for a GPU, so it takes no journal name or --check-locks".to_string());
        }
        journal_name = Some(String::new());
    }
    let journal_name = match (journal_name, check_locks) {
        (Some(name), false) => name,
        (None, true) => String::new(),
        (Some(_), true) => return Err("--check-locks checks lock files, so it takes no journal name".to_string()),
        (None, false) => return Err("missing journal name".to_string()),
    };
    if stitch.is_some() && pdf {
        return Err("--stitch lays rendered pages side by side, so it can't go with --pdf".to_string());
    }
    Ok(Args { strict_glyphs, check_locks, check_gpu, journal_name, journals_dir, pages, svg, scale, supersample, output_dir, font_file, locks_dir, layouts_dir, renderers_dir, chains_dir, parchment, pdf, stitch })
}

fn main() {
    let args = parse_args().unwrap_or_else(|err| {
        eprintln!("error: {}\n\n{}", err, USAGE);
        std::process::exit(1);
    });
    let result = if args.check_gpu {
        darkly::check_gpu().map(|()| println!("ok   Darkly can render: there's a GPU"))
            .map_err(|err| format!("Darkly can't render, no GPU: {}", err))
    } else if args.check_locks {
        check_locks(&args.locks_dir, &args.chains_dir)
    } else {
        run(&args)
    };
    if let Err(err) = result {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), String> {
    let started = std::time::Instant::now();
    let symd = fonts::FontBook::from_file(&args.font_file)
        .map_err(|err| format!("couldn't load the glyphset files {}: {}", args.font_file, err))?;
    let symd = Rc::new(symd);

    let journal_name = &args.journal_name;
    let journal_path = format!("{}/{}/journal_entry.rhai", args.journals_dir, journal_name);
    // By default, NAME_output in the current directory, emptied first.
    let default_output = args.output_dir.is_none();
    let output_dir = args.output_dir.clone()
        .unwrap_or_else(|| format!("{}_output", journal_name));
    let mut engine = JournalEngine::with_dirs(symd.clone(), &args.locks_dir, &args.layouts_dir, &args.renderers_dir, &args.chains_dir);
    // A spinner while the script runs, which can take a while.
    let running = Rc::new(RefCell::new(Progress::new(&format!("Running {}", journal_name), 0)));
    let spinner = running.clone();
    engine.on_progress(move |ops| {
        if ops % 256 == 0 {
            spinner.borrow_mut().tick("");
        }
    });
    let entry = engine.build_entry_file(&journal_path);
    running.borrow_mut().finish();
    let entry = entry?;
    check_missing_glyphs(&symd, args.strict_glyphs)?;
    if entry.page_count() == 0 {
        return Err(format!("{} produced no pages; add them with journal_entry.add(page)", journal_path));
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
    if default_output && pages.is_none() {
        match std::fs::remove_dir_all(&output_dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("couldn't clear {}: {}", output_dir, e)),
            _ => {}
        }
    }
    if pages.is_some() {
        // Rendering a subset: keep the other pages' existing output.
        std::fs::create_dir_all(&output_dir).map_err(|e| format!("couldn't create {}: {}", output_dir, e))?;
    } else {
        prepare_output_dir(&output_dir)?;
    }
    let only = pages.as_deref();
    if args.svg {
        publish::svgs(&entry, &output_dir, args.scale, only)?;
    }
    // One Darkly engine for the whole run; canvases resize it as needed.
    let mut darkly_engine = darkly::new_engine(1, 1);
    let render = publish::Render { scale: args.scale, supersample: args.supersample, parchment: args.parchment };
    if args.pdf {
        let path = format!("{}/{}.pdf", output_dir, journal_name);
        publish::pdf(&entry, &mut darkly_engine, &path, journal_name, &render, only)?;
        println!("Wrote {}", path);
    } else if render.parchment || args.stitch.is_some() {
        publish::page_images(&entry, &mut darkly_engine, &output_dir, &render, only, args.stitch)?;
    } else {
        publish::canvas_images(&entry, &mut darkly_engine, &output_dir, &render, only)?;
    }
    let count = only.map_or(entry.page_count(), |pages| pages.len());
    eprintln!("Rendered {} page{} of {} to {} in {}", count, if count == 1 { "" } else { "s" }, journal_name, output_dir, clock(started.elapsed()));
    Ok(())
}

/// Run every lock file's examples, then every chain file's, listing each
/// one's result.
fn check_locks(locks_dir: &str, chains_dir: &str) -> Result<(), String> {
    let library = Rc::new(LockLibrary::new(locks_dir).with_chains(Sources::dir(chains_dir)));
    let results = library.check_all()?;
    if results.is_empty() {
        return Err(format!("no lock files (*.rhai) in {}", locks_dir));
    }
    let chains = library.check_all_chains()?;
    let mut failures = 0;
    for (name, result) in results.iter().chain(&chains) {
        match result {
            Ok(0) => println!("--   {:<24} no examples", name),
            Ok(n) => println!("ok   {:<24} {} example(s)", name, n),
            Err(err) => {
                failures += 1;
                println!("FAIL {:<24} {}", name, err);
            }
        }
    }
    if failures > 0 {
        return Err(format!("{} of {} lock type(s) and chain(s) failed their examples", failures, results.len() + chains.len()));
    }
    Ok(())
}
