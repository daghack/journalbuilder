use scribe::journal::rhaientries::{Journal, LockLibrary, Resources, DEFAULT_JOURNALS_DIRS, DEFAULT_LIBRARY_DIR};
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

const USAGE: &str = "Usage: journalbuild <journal> [--entry E[,E...]] [--page N[,N...]]
                    [--svg] [--scale S] [--supersample K] [--strict-glyphs]
                    [--parchment] [--pdf] [--stitch N]
                    [--journals-dir DIR] [--library DIR] [--output-dir DIR]
       journalbuild --check-locks [<journal>] [--journals-dir DIR] [--library DIR]
       journalbuild --check-gpu

  <journal>           A journal's name in the journals directory, or the path
                      of a journal's directory (anything with a /): its
                      journal.json, its entries in entries/, and its own
                      resources (locks/, layouts/, illustrations/, chains/,
                      styles/, lib/, glyphsets/, content/) over the
                      library's.
  --entry E,...       Build only these entries, by name (default: every
                      entry, in the manifest's order).
  --page P,...        Render only these pages of each entry built, by index
                      (0-based) or by name. Other pages' existing output is
                      kept.
  --svg               Also render each canvas as an SVG ({page}_{canvas}.svg)
                      and that SVG as a PNG; by default only Darkly renders.
                      Stroke widths are geometric widths there, while in
                      Darkly they scale brush pressure.
  --parchment         Render each page as one image on parchment,
                      darkly_{page}.png, its canvases layered in order: a
                      mottled scroll browning toward its edges. Pages get
                      a border of parchment round their strokes; each tile
                      of a split page is a sheet of its own. Light inks are
                      drawn dark, so they show, and ink soaks in a little
                      unevenly: very slightly thinner in places.
  --pdf               Put each entry's pages in one PDF, {entry}.pdf, a page
                      per page, rather than writing images: each page's
                      canvases layered into one image (on parchment,
                      with --parchment), 150 canvas units to the inch.
                      With --stitch N, N pages side by side to a PDF page.
  --stitch N          Also lay every N pages side by side, in order, tops
                      level, named for the pages they hold: page_0 to
                      page_5 as darkly_page_0-5.png (the last may have
                      fewer; a page left alone isn't stitched). Pages are rendered as one image
                      each, darkly_{page}.png, as with --parchment (which
                      it goes with, on parchment, or without, on nothing).
                      With --pdf, the PDF's pages are the stitches instead
                      (a page left alone at the end is a page of its own).
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
                      repository's journals/, then webapp/journals/, the
                      web editor's built-in ones).
  --library DIR       The built-in library journals' resources stand in
                      front of (default: the repository's webapp/library/).
  --output-dir DIR    Where to write renders, a directory per entry (default:
                      NAME_output in the current directory, its entries'
                      directories emptied first unless --page). In a given
                      directory, earlier .png/.svg/.pdf renders in an
                      entry's directory are cleared first (except with
                      --page); other files are kept.
  --check-locks       Run every lock file's examples, and every chain
                      file's, through lock and unlock, instead of building
                      a journal: the library's, or a journal's own.
  --check-gpu         Check that Darkly can render (there's a GPU wgpu can
                      use), instead of building a journal; fails if not.
                      Without one, --svg renders still work, and cargo
                      test checks them (tests/svg_renders.rs).";

struct Args {
    strict_glyphs: bool,
    check_locks: bool,
    check_gpu: bool,
    journal: Option<String>,
    journals_dirs: Vec<String>,
    library_dir: String,
    entries: Option<Vec<String>>,
    pages: Option<Vec<String>>,
    svg: bool,
    scale: f32,
    supersample: f32,
    output_dir: Option<String>,
    parchment: bool,
    pdf: bool,
    stitch: Option<usize>,
}

/// A comma-separated list, `what` naming it in errors.
fn list(value: &str) -> Result<Vec<String>, String> {
    value.split(',').map(str::trim)
        .map(|p| if p.is_empty() { Err(format!("empty item in '{}'", value)) } else { Ok(p.to_string()) })
        .collect()
}

fn parse_args() -> Result<Args, String> {
    let mut journal = None;
    let mut entries: Option<Vec<String>> = None;
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
    let mut journals_dirs: Vec<String> = DEFAULT_JOURNALS_DIRS.iter().map(|dir| dir.to_string()).collect();
    let mut library_dir = DEFAULT_LIBRARY_DIR.to_string();
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
                journals_dirs = vec![args.next().ok_or("--journals-dir needs a directory")?];
            }
            "--library" => {
                library_dir = args.next().ok_or("--library needs a directory")?;
            }
            "--output-dir" => {
                output_dir = Some(args.next().ok_or("--output-dir needs a directory")?);
            }
            "--entry" => {
                let value = args.next().ok_or("--entry needs an entry or list, e.g. --entry letter_1")?;
                entries.get_or_insert_with(Vec::new).extend(list(&value)?);
            }
            "--page" => {
                let value = args.next().ok_or("--page needs a page or list, e.g. --page 0,2 or --page maze")?;
                pages.get_or_insert_with(Vec::new).extend(list(&value)?);
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option '{}'", flag)),
            name if journal.is_none() => journal = Some(name.to_string()),
            extra => return Err(format!("unexpected argument '{}'", extra)),
        }
    }
    if check_gpu && (journal.is_some() || check_locks) {
        return Err("--check-gpu only checks for a GPU, so it takes no journal or --check-locks".to_string());
    }
    if !check_gpu && !check_locks && journal.is_none() {
        return Err("missing journal name".to_string());
    }
    if check_locks && (entries.is_some() || pages.is_some()) {
        return Err("--check-locks checks lock and chain files, so it takes no --entry or --page".to_string());
    }
    Ok(Args { strict_glyphs, check_locks, check_gpu, journal, journals_dirs, library_dir, entries, pages, svg, scale, supersample, output_dir, parchment, pdf, stitch })
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
        check_locks(&args)
    } else {
        run(&args)
    };
    if let Err(err) = result {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}

/// The journal `name`: a directory in the first of `journals_dirs` that
/// has it, or, with a `/`, a path.
fn open_journal(name: &str, journals_dirs: &[String]) -> Result<Journal, String> {
    if name.contains('/') {
        let dir = std::path::PathBuf::from(name);
        if !dir.is_dir() {
            return Err(format!("no journal {}: {} isn't a directory", name, dir.display()));
        }
        return Journal::open(dir);
    }
    let dirs: Vec<_> = journals_dirs.iter().map(|dir| std::path::Path::new(dir).join(name)).collect();
    match dirs.iter().find(|dir| dir.is_dir()) {
        Some(dir) => Journal::open(dir),
        None => Err(format!("no journal {}: none of {} is a directory", name, dirs.iter().map(|dir| dir.display().to_string()).collect::<Vec<_>>().join(", "))),
    }
}

fn run(args: &Args) -> Result<(), String> {
    let started = std::time::Instant::now();
    let journal = open_journal(args.journal.as_deref().unwrap_or_default(), &args.journals_dirs)?;
    let journal_name = journal.name();
    for unlisted in journal.unlisted_entries() {
        eprintln!("warning: {} isn't in {}'s journal.json, so it isn't built", journal.entry_path(&unlisted).display(), journal_name);
    }
    let library = Resources::dir(&args.library_dir);
    for path in journal.resources(library.clone()).overrides() {
        eprintln!("note: {}'s {} stands in for the library's", journal_name, path);
    }
    let entries = match &args.entries {
        None => journal.manifest.entries.clone(),
        Some(entries) => {
            for entry in entries {
                if !journal.manifest.entries.contains(entry) {
                    return Err(format!("{} has no entry '{}'; entries: {}", journal_name, entry, journal.manifest.entries.join(", ")));
                }
            }
            entries.clone()
        }
    };
    // By default, NAME_output in the current directory, each entry's
    // directory emptied first.
    let default_output = args.output_dir.is_none();
    let output_dir = args.output_dir.clone().unwrap_or_else(|| format!("{}_output", journal_name));
    // One Darkly engine for the whole run; canvases resize it as needed.
    let mut darkly_engine = darkly::new_engine(1, 1);
    let mut rendered = 0;
    for entry_name in &entries {
        rendered += build_entry(args, &journal, &library, entry_name, &format!("{}/{}", output_dir, entry_name), default_output, &mut darkly_engine)?;
    }
    eprintln!("Rendered {} page{} of {} ({}) to {} in {}", rendered, if rendered == 1 { "" } else { "s" }, journal_name, entries.join(", "), output_dir, clock(started.elapsed()));
    Ok(())
}

/// Build entry `entry_name` and render it into `output_dir`, returning how
/// many pages it rendered.
fn build_entry(args: &Args, journal: &Journal, library: &Resources, entry_name: &str, output_dir: &str, default_output: bool, darkly_engine: &mut darkly::DarklyEngine) -> Result<usize, String> {
    // Each entry runs in an engine of its own: entries share resources,
    // not code.
    let mut engine = journal.engine(library.clone())?;
    let symd = engine.glyphsets().clone();
    let entry_path = journal.entry_path(entry_name).display().to_string();
    // A spinner while the script runs, which can take a while.
    let running = Rc::new(RefCell::new(Progress::new(&format!("Running {}", entry_name), 0)));
    let spinner = running.clone();
    engine.on_progress(move |ops| {
        if ops % 256 == 0 {
            spinner.borrow_mut().tick("");
        }
    });
    let entry = journal.build_entry(&mut engine, entry_name);
    running.borrow_mut().finish();
    let entry = entry?;
    check_missing_glyphs(&symd, args.strict_glyphs)?;
    if entry.page_count() == 0 {
        return Err(format!("{} produced no pages; add them with journal_entry.add(page)", entry_path));
    }
    let empty = entry.empty_canvases();
    if !empty.is_empty() {
        eprintln!("warning: skipping {} empty canvas(es) in {}, with nothing drawn on them: {}", empty.len(), entry_name, empty.join(", "));
    }
    let pages: Option<Vec<usize>> = match &args.pages {
        None => None,
        Some(pages) => Some(pages.iter().map(|page| entry.page_index(page))
            .collect::<Result<_, _>>()
            .map_err(|err| format!("{} (in {})", err, entry_name))?),
    };
    if default_output && pages.is_none() {
        match std::fs::remove_dir_all(output_dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("couldn't clear {}: {}", output_dir, e)),
            _ => {}
        }
    }
    if pages.is_some() {
        // Rendering a subset: keep the other pages' existing output.
        std::fs::create_dir_all(output_dir).map_err(|e| format!("couldn't create {}: {}", output_dir, e))?;
    } else {
        prepare_output_dir(output_dir)?;
    }
    let only = pages.as_deref();
    if args.svg {
        publish::svgs(&entry, output_dir, args.scale, only)?;
    }
    let render = publish::Render { scale: args.scale, supersample: args.supersample, parchment: args.parchment };
    if args.pdf {
        let path = format!("{}/{}.pdf", output_dir, entry_name);
        let title = format!("{}: {}", journal.manifest.title, entry_name);
        publish::pdf(&entry, darkly_engine, &path, &title, &render, only, args.stitch)?;
        println!("Wrote {}", path);
    } else if render.parchment || args.stitch.is_some() {
        publish::page_images(&entry, darkly_engine, output_dir, &render, only, args.stitch)?;
    } else {
        publish::canvas_images(&entry, darkly_engine, output_dir, &render, only)?;
    }
    Ok(only.map_or(entry.page_count(), |pages| pages.len()))
}

/// Run every lock file's examples, then every chain file's, listing each
/// one's result: the library's, or with a journal, the journal's own.
fn check_locks(args: &Args) -> Result<(), String> {
    let library = Resources::dir(&args.library_dir);
    let (resources, own) = match &args.journal {
        None => (library, None),
        Some(name) => {
            let journal = open_journal(name, &args.journals_dirs)?;
            let resources = journal.resources(library);
            let locks = resources.locks.own_names().map_err(|e| e.to_string())?;
            let chains = resources.chains.own_names().map_err(|e| e.to_string())?;
            (resources, Some((journal.name(), locks, chains)))
        }
    };
    let library = Rc::new(LockLibrary::new(&resources));
    let (results, chains) = match &own {
        None => {
            let results = library.check_all()?;
            if results.is_empty() {
                return Err(format!("no lock files (*.rhai) in {}", resources.locks.root().display()));
            }
            (results, library.check_all_chains()?)
        }
        Some((_, locks, chains)) => (
            locks.iter().map(|name| (name.clone(), library.check_examples(name))).collect(),
            chains.iter().map(|name| (name.clone(), library.check_chain(name))).collect(),
        ),
    };
    if let Some((name, _, _)) = &own
        && results.is_empty() && chains.is_empty() {
        println!("--   {} has no lock or chain files of its own", name);
    }
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
