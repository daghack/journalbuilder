//! With the `embedded-assets` feature, lists every file of the built-in
//! library (`webapp/library/`) for `resources::embedded` to compile in
//! (`$OUT_DIR/embedded_library.rs`).

use std::path::Path;

/// Every kind of resource: its directory, and its files' extension, as
/// `resources::KINDS` lists them.
const KINDS: [(&str, &str); 10] = [
    ("locks", "rhai"),
    ("layouts", "rhai"),
    ("illustrations", "rhai"),
    ("chains", "rhai"),
    ("styles", "rhai"),
    ("lib", "rhai"),
    ("glyphsets", "json"),
    ("content", "txt"),
    ("content", "md"),
    ("content", "json"),
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_EMBEDDED_ASSETS").is_none() {
        return;
    }
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("webapp/library");
    println!("cargo:rerun-if-changed={}", library.display());
    let mut out = String::from("pub const LIBRARY: &[(&str, &str, &[(&str, &str)])] = &[\n");
    for (kind, ext) in KINDS {
        let dir = library.join(kind);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut files = Vec::new();
        if dir.is_dir() {
            collect(&dir, "", ext, &mut files);
        }
        files.sort();
        out.push_str(&format!("    ({:?}, {:?}, &[\n", kind, ext));
        for (name, path) in files {
            out.push_str(&format!("        ({:?}, include_str!({:?})),\n", name, path));
        }
        out.push_str("    ]),\n");
    }
    out.push_str("];\n");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(Path::new(&out_dir).join("embedded_library.rs"), out).unwrap();
}

/// Every `.ext` file under `dir`, by its name: its path from there without
/// the extension.
fn collect(dir: &Path, prefix: &str, ext: &str, files: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            collect(&path, &format!("{}{}/", prefix, stem), ext, files);
        } else if path.extension().is_some_and(|e| e == ext) {
            files.push((format!("{}{}", prefix, stem), path.display().to_string()));
        }
    }
}
