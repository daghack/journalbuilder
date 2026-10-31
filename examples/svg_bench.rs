//! Times what an SVG render costs on the pages tests/svg_renders.rs checks,
//! and hashes what they draw, to see that a speedup changed nothing.
//!
//! ```bash
//! cargo run --release --example svg_bench -- --reps 3 --scale 0.1 --hashes /tmp/before.txt
//! ```
//!
//! Columns, in milliseconds, each the fastest of `--reps` runs: `engine` makes
//! the entry's engine (glyphsets, the Rhai API), `eval` runs its script,
//! `page_svg` makes the web editor's SVG of its pages, and `render` makes their
//! `journalbuild --svg` renders (at `--scale`, like `journalbuild --scale`,
//! which tests render at 0.1).

use scribe::journal::publish::{page_svg, page_svg_renders};
use scribe::journal::rhaientries::{Entry, Journal, Resources};
use std::fmt::Write as _;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Instant;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Every workload: a journal's directory, and whether all its pages are
/// rendered or just each entry's first.
const WORKLOADS: &[(&str, &str, bool)] = &[
    ("tests/smoke_journals", "smoke", true),
    ("webapp/journals", "showcase", true),
    ("webapp/journals", "api_reference", false),
    ("journals", "becoming_us", false),
    ("journals", "black_priests", false),
    ("journals", "copper_letter_1", false),
    ("journals", "first_codex", false),
    ("journals", "journal_entries", false),
    ("journals", "lang_font", false),
    ("journals", "loche_rules", false),
    ("journals", "matter_of_pyrius", false),
    ("journals", "nivelia_letters", false),
    ("journals", "raven_constellations", false),
    ("journals", "welcome_home", false),
];

const COLUMNS: [&str; 4] = ["engine", "eval", "page_svg", "render"];

/// Milliseconds of each of `COLUMNS`.
#[derive(Clone, Copy, Default)]
struct Times([f64; 4]);

impl Times {
    fn fastest(self, other: Times) -> Times {
        Times(std::array::from_fn(|i| self.0[i].min(other.0[i])))
    }
    fn add(&mut self, other: Times) {
        for (a, b) in self.0.iter_mut().zip(other.0) {
            *a += b;
        }
    }
}

fn timed<T>(time: &mut f64, f: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let value = f();
    *time += started.elapsed().as_secs_f64() * 1000.0;
    value
}

fn selected(entry: &Entry, all_pages: bool) -> Vec<usize> {
    if all_pages { (0..entry.page_count()).collect() } else { vec![0] }
}

/// One run of entry `name`, with the hashes of what it drew added to
/// `hashes` the first time.
fn run_entry(journal: &Journal, name: &str, all_pages: bool, scale: f32, hashes: Option<&mut String>) -> Result<Times, String> {
    let mut times = Times::default();
    let mut engine = timed(&mut times.0[0], || journal.engine(Resources::library()))?;
    let entry = timed(&mut times.0[1], || journal.build_entry(&mut engine, name))?;
    let pages = selected(&entry, all_pages);
    let previews: Vec<_> = timed(&mut times.0[2], || {
        entry.selected_pages(Some(&pages)).map(|(i, page)| page_svg(page, &page.label(i))).collect()
    });
    let renders: Vec<_> = timed(&mut times.0[3], || {
        entry.selected_pages(Some(&pages)).flat_map(|(i, page)| page_svg_renders(page, i, scale)).collect()
    });
    if let Some(hashes) = hashes {
        let hash = |bytes: &[u8]| {
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            hasher.finish()
        };
        for ((svg, view), page) in previews.iter().flatten().zip(&pages) {
            writeln!(hashes, "{}/{} page_svg {} {:016x} {:?}", journal.name(), name, page, hash(svg.as_bytes()), view).unwrap();
        }
        for (canvas, svg, png) in &renders {
            let pixels = image::load_from_memory(png).expect("the render is a PNG").to_rgba8();
            writeln!(hashes, "{}/{} {} svg {:016x} {} pixels {:016x} {:?}", journal.name(), name, canvas, hash(svg.as_bytes()), svg.len(), hash(pixels.as_raw()), pixels.dimensions()).unwrap();
        }
    }
    Ok(times)
}

struct Options {
    scale: f32,
    reps: usize,
    only: Option<String>,
    hashes: Option<String>,
}

fn options() -> Options {
    let mut options = Options { scale: 0.1, reps: 3, only: None, hashes: None };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{} needs a value", arg));
        match arg.as_str() {
            "--scale" => options.scale = value().parse().expect("--scale takes a number"),
            "--reps" => options.reps = value().parse().expect("--reps takes a number"),
            "--only" => options.only = Some(value()),
            "--hashes" => options.hashes = Some(value()),
            other => panic!("unknown argument {}", other),
        }
    }
    options
}

fn main() {
    let options = options();
    let mut hashes = String::new();
    println!("{:<24}{}", "journal", COLUMNS.iter().map(|c| format!("{:>10}", c)).collect::<String>());
    let mut total = Times::default();
    let row = |name: &str, times: Times| {
        println!("{:<24}{}", name, times.0.iter().map(|t| format!("{:>10.1}", t)).collect::<String>());
    };
    for &(dir, name, all_pages) in WORKLOADS {
        if options.only.as_ref().is_some_and(|only| only != name) {
            continue;
        }
        let journal = Journal::open(format!("{}/{}/{}", ROOT, dir, name)).unwrap();
        let mut sum = Times::default();
        for entry in &journal.manifest.entries {
            let first = run_entry(&journal, entry, all_pages, options.scale, Some(&mut hashes)).unwrap();
            let best = (1..options.reps).fold(first, |best, _| {
                best.fastest(run_entry(&journal, entry, all_pages, options.scale, None).unwrap())
            });
            sum.add(best);
        }
        row(name, sum);
        total.add(sum);
    }
    row("TOTAL", total);
    if let Some(path) = options.hashes {
        std::fs::write(path, hashes).unwrap();
    }
}
