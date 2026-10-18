//! With the `embedded-assets` feature, lists every layout and lock file for
//! `rhaientries::embedded` to compile in (`$OUT_DIR/embedded_sources.rs`).

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_EMBEDDED_ASSETS").is_none() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fonts = root.join("webapp/fonts");
    println!("cargo:rerun-if-changed={}", fonts.display());
    let merged = merge_fonts(&fonts);
    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(Path::new(&out_dir).join("fonts.json"), merged).unwrap();
    let mut out = String::new();
    for (constant, dir) in [("LAYOUTS", "webapp/layouts"), ("LOCKS", "webapp/locks")] {
        let dir = root.join(dir);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut scripts = Vec::new();
        collect(&dir, "", &mut scripts);
        scripts.sort();
        out.push_str(&format!("pub const {}: &[(&str, &str)] = &[\n", constant));
        for (name, path) in scripts {
            out.push_str(&format!("    ({:?}, include_str!({:?})),\n", name, path));
        }
        out.push_str("];\n");
    }
    std::fs::write(Path::new(&out_dir).join("embedded_sources.rs"), out).unwrap();
}

/// Every `.rhai` file under `dir`, by its name: its path from there without
/// the extension, `lib/permutations`.
fn collect(dir: &Path, prefix: &str, scripts: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            collect(&path, &format!("{}{}/", prefix, stem), scripts);
        } else if path.extension().is_some_and(|ext| ext == "rhai") {
            scripts.push((format!("{}{}", prefix, stem), path.display().to_string()));
        }
    }
}

/// The font files in `dir` (`NAME.json`, one font each) as one font file,
/// as `fonts::merge_font_dir` makes.
fn merge_fonts(dir: &Path) -> String {
    let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json")).collect();
    files.sort();
    let fonts: Vec<String> = files.iter().map(|path| {
        let name = path.file_stem().unwrap().to_string_lossy();
        format!("{:?}: {}", name, std::fs::read_to_string(path).unwrap().trim())
    }).collect();
    format!("{{\n{}\n}}\n", fonts.join(",\n"))
}
