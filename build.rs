//! With the `embedded-assets` feature, lists every file of the built-in
//! library (`webapp/library/`) for `resources::embedded` to compile in
//! (`$OUT_DIR/embedded_library.rs`). Pictures go in as data URLs, as
//! `Sources` reads them.

use std::path::Path;

/// Every kind of resource: its directory, and its files' extension, as
/// `resources::KINDS` lists them.
const KINDS: [(&str, &str); 14] = [
    ("locks", "rhai"),
    ("helpers", "rhai"),
    ("layouts", "rhai"),
    ("illustrations", "rhai"),
    ("chains", "rhai"),
    ("styles", "json"),
    ("lib", "rhai"),
    ("glyphsets", "json"),
    ("content", "txt"),
    ("content", "md"),
    ("content", "json"),
    ("images", "png"),
    ("images", "jpg"),
    ("fluids", "json"),
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
            match image_type(ext) {
                Some(mime) => {
                    println!("cargo:rerun-if-changed={}", path);
                    let url = format!("data:{};base64,{}", mime, base64(&std::fs::read(&path).unwrap()));
                    out.push_str(&format!("        ({:?}, {:?}),\n", name, url));
                }
                None => out.push_str(&format!("        ({:?}, include_str!({:?})),\n", name, path)),
            }
        }
        out.push_str("    ]),\n");
    }
    out.push_str("];\n");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(Path::new(&out_dir).join("embedded_library.rs"), out).unwrap();
}

/// The media type of pictures with extension `ext`, as
/// `sources::image_type` has it; `None` for text.
fn image_type(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" => Some("image/jpeg"),
        _ => None,
    }
}

/// `bytes` in base64, padded, as `sources::base64` writes it.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= chunk.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
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
