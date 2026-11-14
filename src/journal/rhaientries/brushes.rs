//! A journal's brushes, for scripts: its `brushes/NAME.yaml` in front of
//! the library's (Darkly's built-ins), each called by the name its file
//! gives (see `crate::journal::brushes`).
//!
//! Styles name brushes wherever they're made (constructors, settings maps,
//! text style files, layouts' roles), so the brushes they're looked up in
//! are the ones of the engine made or run last on this thread (`use_here`).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::Arc;

use rhai::{Dynamic, Engine};

use super::sources::Sources;
use crate::journal::brushes::{self, BrushGraph};

/// Where a brush's name is found: its file's name, or why the name is
/// ambiguous.
type Index = BTreeMap<String, Result<String, String>>;

pub struct BrushLibrary {
    sources: Sources,
    /// Every brush's name, found once, as each file's name says.
    index: RefCell<Option<Rc<Index>>>,
    /// Brushes read so far, by file.
    read: RefCell<HashMap<String, Result<Arc<BrushGraph>, String>>>,
}

thread_local! {
    /// The brushes styles look brushes up in here.
    static HERE: RefCell<Option<Rc<BrushLibrary>>> = const { RefCell::new(None) };
}

impl BrushLibrary {
    pub fn new(sources: Sources) -> Self {
        BrushLibrary { sources, index: RefCell::default(), read: RefCell::default() }
    }

    /// Look styles' brushes up in these, on this thread, from now on.
    pub fn use_here(self: &Rc<Self>) {
        HERE.with_borrow_mut(|here| *here = Some(self.clone()));
    }

    /// Every brush's name, and the file it's in. A journal's file stands in
    /// for a library brush of the same name, as for a file of the same name;
    /// two of a journal's (or the library's) files of the same name make it
    /// ambiguous. Files are read without noting them as used: only a brush
    /// drawn with is.
    fn index(&self) -> Rc<Index> {
        if let Some(index) = self.index.borrow().as_ref() {
            return index.clone();
        }
        let files = self.sources.unrecorded();
        let mut own: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut library: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for file in files.names().unwrap_or_default() {
            // A file that isn't YAML is called by its file name, and says
            // what's wrong when it's drawn with.
            let name = files.read(&file).ok().and_then(|yaml| brushes::file_name(&yaml)).filter(|n| !n.is_empty()).unwrap_or_else(|| file.clone());
            let layer = if files.has_own(&file) { &mut own } else { &mut library };
            layer.entry(name).or_default().push(file);
        }
        let mut index = Index::new();
        for layer in [library, own] {
            for (name, files) in layer {
                let found = match files.as_slice() {
                    [file] => Ok(file.clone()),
                    _ => Err(format!("brushes {} are all called '{}'; rename one", files.iter().map(|f| format!("{}/{}.yaml", "brushes", f)).collect::<Vec<_>>().join(" and "), name)),
                };
                index.insert(name, found);
            }
        }
        let index = Rc::new(index);
        *self.index.borrow_mut() = Some(index.clone());
        index
    }

    /// Every brush's name, sorted.
    pub fn names(&self) -> Vec<String> {
        self.index().keys().cloned().collect()
    }

    /// The brush called `name`, read and checked.
    pub fn get(&self, name: &str) -> Result<Arc<BrushGraph>, String> {
        let index = self.index();
        let file = match index.get(name) {
            Some(found) => found.clone()?,
            None => return Err(format!("unknown brush '{}'; the brushes are {}", name, index.keys().cloned().collect::<Vec<_>>().join(", "))),
        };
        if let Some(read) = self.read.borrow().get(&file) {
            return read.clone();
        }
        let read = self.sources.read(&file)
            .map_err(|e| e.to_string())
            .and_then(|yaml| BrushGraph::parse(&yaml, &file))
            .map(Arc::new)
            .map_err(|e| format!("brush '{}' (brushes/{}.yaml) can't be drawn with: {}", name, file, e));
        self.read.borrow_mut().insert(file, read.clone());
        read
    }
}

/// The brush called `name` in the brushes styles use here (see
/// `BrushLibrary::use_here`), or Darkly's built-in of that name if there
/// are none.
pub fn lookup(name: &str) -> Result<Arc<BrushGraph>, String> {
    let here = HERE.with_borrow(|here| here.clone());
    match here {
        Some(library) => library.get(name),
        None => brushes::builtin(name).ok_or_else(|| {
            format!("unknown brush '{}'; the brushes are {}", name, brushes::builtins().keys().cloned().collect::<Vec<_>>().join(", "))
        }),
    }
}

pub(super) fn register(engine: &mut Engine, library: Rc<BrushLibrary>) {
    engine.register_fn("brush_names", move || -> rhai::Array { library.names().into_iter().map(Dynamic::from).collect() });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::rhaientries::Resources;

    fn ink_pen() -> &'static str {
        darkly::brush::builtin_brushes::BUILTIN_BRUSHES_YAML.iter().find(|(f, _)| *f == "ink_pen.yaml").unwrap().1
    }

    #[test]
    fn the_library_has_darklys_brushes() {
        let library = BrushLibrary::new(Resources::library().brushes);
        let builtins: Vec<String> = brushes::builtins().keys().cloned().collect();
        assert_eq!(library.names(), builtins);
        assert_eq!(library.get("Ink Pen").unwrap(), brushes::builtin("Ink Pen").unwrap());
    }

    #[test]
    fn a_journals_brushes_stand_by_the_librarys() {
        let quill = ink_pen().replace("name: Ink Pen", "name: Quill");
        let own = Resources::none().brushes.with_overlay([
            ("quill".to_string(), quill),
            ("ink_pen".to_string(), ink_pen().replace("softness: 0.1", "softness: 0.5")),
            ("broken".to_string(), "name: Broken\nnodes: {}\n".to_string()),
        ]);
        let library = BrushLibrary::new(own.over(Resources::library().brushes));
        let names = library.names();
        assert!(names.contains(&"Quill".to_string()) && names.contains(&"Ink Pen".to_string()) && names.contains(&"Pencil".to_string()), "{:?}", names);
        assert_eq!(library.get("Quill").unwrap().name(), "Quill");
        assert_ne!(library.get("Ink Pen").unwrap(), brushes::builtin("Ink Pen").unwrap(), "the journal's Ink Pen stands in for the library's");
        let err = library.get("Broken").unwrap_err();
        assert!(err.contains("brushes/broken.yaml") && err.contains("brush_settings"), "{}", err);
        let err = library.get("Crayon").unwrap_err();
        assert!(err.contains("unknown brush 'Crayon'") && err.contains("Quill"), "{}", err);
    }

    /// The library's brushes are Darkly's own files, as its pinned
    /// revision has them: update them with it.
    #[test]
    fn the_librarys_brush_files_are_darklys() {
        let library = Resources::library().brushes;
        let mut files = library.names().unwrap();
        files.sort();
        let mut darklys: Vec<String> = darkly::brush::builtin_brushes::BUILTIN_BRUSHES_YAML.iter()
            .map(|(file, _)| file.strip_suffix(".yaml").unwrap().to_string()).collect();
        darklys.sort();
        assert_eq!(files, darklys, "webapp/library/brushes/ should hold Darkly's brushes");
        for (file, yaml) in darkly::brush::builtin_brushes::BUILTIN_BRUSHES_YAML {
            assert_eq!(library.read(file.strip_suffix(".yaml").unwrap()).unwrap(), *yaml, "webapp/library/brushes/{} isn't Darkly's", file);
        }
    }

    #[test]
    fn scripts_draw_with_a_journals_brushes() {
        use crate::journal::canvas::Canvas as JournalCanvas;
        let mut journal = Resources::none();
        journal.brushes = journal.brushes.with_overlay([("quill".to_string(), ink_pen().replace("name: Ink Pen", "name: Quill"))]);
        let resources = journal.over(Resources::library());
        let book = Rc::new(resources.glyphset_book().unwrap());
        let mut engine = crate::journal::rhaientries::JournalEngine::with_resources(book, resources);
        let entry = engine.build_entry_str(r#"
            assert(brush_names().contains("Quill") && brush_names().contains("Pencil"), "brush_names() has the journal's and the library's");
            let pen = new_brush_style("black", "Quill", 2.0);
            assert(pen.brush == "Quill", "named as its file says");
            let canvas = new_canvas();
            canvas.add(new_line(vec2(0, 0), vec2(10, 0)), new_style(pen, new_handwriting_style()));
            let page = new_page();
            page.add(canvas);
            journal_entry.add(page);
        "#).unwrap();
        let canvas: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        let brush = canvas.strokes()[0].style.brush.clone().unwrap();
        assert_eq!(brush.name(), "Quill");
        assert!(brush.graph.graph().contains("brush_settings"));
    }

    #[test]
    fn two_brushes_of_one_name_are_ambiguous() {
        let own = Resources::none().brushes.with_overlay([
            ("a".to_string(), ink_pen().replace("name: Ink Pen", "name: Twin")),
            ("b".to_string(), ink_pen().replace("name: Ink Pen", "name: Twin")),
        ]);
        let err = BrushLibrary::new(own).get("Twin").unwrap_err();
        assert!(err.contains("brushes/a.yaml and brushes/b.yaml"), "{}", err);
    }
}
