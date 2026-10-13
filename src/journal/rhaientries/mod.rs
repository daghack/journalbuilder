mod art;
mod avoid;
mod canvas;
mod entry;
mod glyphs;
mod gradient;
mod layout;
mod locks;
mod markdown;
mod options;
mod path;
mod random;
mod style;
mod text;
mod vec;

pub use layout::DEFAULT_LAYOUTS_DIR;
pub use locks::{LockLibrary, DEFAULT_LOCKS_DIR};
pub use canvas::{CanvasData, Stroke};
pub use entry::{Entry, Page};
pub use style::{BrushSettings, StrokeStyle};
pub use vec::Vec2;
use entry::check_name;
use path::TextSource;

use rhai::{Dynamic, Engine, EvalAltResult, Scope};

/// What rhai functions return: a value, or an error for the script.
pub(super) type RhaiResult<T> = Result<T, Box<EvalAltResult>>;
use super::paths::StrokePath;
use super::fonts::FontBook;

use std::cell::RefCell;
use std::rc::Rc;

/// Fail if a path has an infinite or NaN coordinate. Scaling or dividing
/// by zero in a script makes them, and they'd otherwise break the render's
/// sizing far from the line that caused them.
pub(super) fn check_finite(caller: &str, path: &StrokePath) -> Result<(), Box<EvalAltResult>> {
    if path.points_within().iter().all(|p| p.is_finite()) {
        Ok(())
    } else {
        Err(format!("{}: the path has a non-finite (infinite or NaN) coordinate", caller).into())
    }
}

/// A shared, mutable value, as rhai scripts see paths, glyphs and canvases:
/// copying one (assigning it, passing it, a `for` loop's variable) shares
/// the value, so changes through any copy, including chained calls like
/// `p.move_to(a).line_to(b)`, all reach it. `.copy()` makes an independent
/// one.
pub struct Shared<T>(Rc<RefCell<T>>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T: Clone> Shared<T> {
    pub fn new(value: T) -> Self {
        Shared(Rc::new(RefCell::new(value)))
    }
    pub fn borrow(&self) -> std::cell::Ref<'_, T> {
        self.0.borrow()
    }
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, T> {
        self.0.borrow_mut()
    }
    /// The value as it is now.
    pub fn get(&self) -> T {
        self.0.borrow().clone()
    }
    /// An independent copy, for `.copy()`.
    pub fn copy(&mut self) -> Self {
        Shared::new(self.get())
    }
    /// Change the value in place and return this same handle, so the change
    /// reaches the variable even partway through a chain.
    pub fn update(&mut self, f: impl FnOnce(&mut T)) -> Self {
        f(&mut self.0.borrow_mut());
        self.clone()
    }
}

/// A path as scripts see it (see `Shared`).
pub type Path = Shared<StrokePath>;

/// A canvas as scripts see it (see `Shared`). Pages keep copies of the
/// canvases added to them.
pub type Canvas = Shared<CanvasData>;

pub struct JournalEngine {
    engine: Engine,
    warnings: Rc<RefCell<Vec<String>>>,
}

impl JournalEngine {
    /// An engine with the repository's lock types (DEFAULT_LOCKS_DIR).
    pub fn new(symdict: Rc<FontBook>) -> Self {
        Self::with_locks_dir(symdict, DEFAULT_LOCKS_DIR)
    }
    /// An engine whose lock types are the lock files in `locks_dir`.
    pub fn with_locks_dir(symdict: Rc<FontBook>, locks_dir: impl Into<std::path::PathBuf>) -> Self {
        Self::with_dirs(symdict, locks_dir, DEFAULT_LAYOUTS_DIR)
    }
    /// An engine with the lock files in `locks_dir` and the layout files
    /// in `layouts_dir`.
    pub fn with_dirs(symdict: Rc<FontBook>, locks_dir: impl Into<std::path::PathBuf>, layouts_dir: impl Into<std::path::PathBuf>) -> Self {
        let mut engine = Engine::new();
        let rng = random::register(&mut engine);
        let text: TextSource = (symdict.clone(), rng.clone());

        canvas::register(&mut engine);
        path::register(&mut engine, text);
        style::register(&mut engine);
        vec::register(&mut engine);
        entry::register(&mut engine);

        // warn("...") reports a problem without stopping the script: it goes
        // to stderr, marked with the script line, like journalbuild's own
        // warnings.
        let warnings: Rc<RefCell<Vec<String>>> = Rc::default();
        let script_warnings = warnings.clone();
        engine.register_fn("warn", move |ctx: rhai::NativeCallContext, message: &str| {
            let warning = match ctx.call_position().line() {
                Some(line) => format!("{} (line {})", message, line),
                None => message.to_string(),
            };
            eprintln!("warning: {}", warning);
            script_warnings.borrow_mut().push(warning);
        });

        // Characters drawn with the fallback glyph so far:
        // #{ "q": #{ count: 2, tried: ["copper_lower_q", ...] }, ... }
        let missing_symdict = symdict.clone();
        engine.register_fn("missing_glyphs", move || -> rhai::Map {
            missing_symdict.missing_glyphs().into_iter().map(|(c, glyph)| {
                let mut info = rhai::Map::new();
                info.insert("count".into(), Dynamic::from(glyph.count as i64));
                info.insert("tried".into(), Dynamic::from_array(glyph.tried.into_iter().map(Dynamic::from).collect()));
                (c.to_string().into(), Dynamic::from_map(info))
            }).collect()
        });

        art::register(&mut engine);
        avoid::register(&mut engine);
        layout::register(&mut engine, symdict.clone(), rng.clone(), layouts_dir.into());
        markdown::register(&mut engine);
        glyphs::register(&mut engine, symdict.clone(), rng.clone());
        gradient::register(&mut engine);
        text::register(&mut engine, symdict.clone(), rng.clone());
        locks::register(&mut engine, Rc::new(LockLibrary::new(locks_dir)));

        JournalEngine {
            engine,
            warnings,
        }
    }
    /// The name of every function scripts can call that this crate
    /// registers (not rhai's standard library), with `module::` for those
    /// in a module like `art`. Properties are `get$name` and `set$name`.
    pub fn function_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.engine.gen_fn_signatures(false).into_iter()
            .map(|signature| signature.split('(').next().unwrap_or_default().to_string())
            .collect();
        names.sort();
        names.dedup();
        names
    }
    /// Everything scripts have passed to `warn` so far.
    pub fn warnings(&self) -> Vec<String> {
        self.warnings.borrow().clone()
    }
    /// Run a journal script, returning the `journal_entry` it built. Errors
    /// are rhai's message, with the script line.
    pub fn build_entry_str(&mut self, script_str: &str) -> Result<Entry, String> {
        let mut scope = Scope::new();
        scope.push("journal_entry", Entry::default());
        self.engine.run_with_scope(&mut scope, script_str).map_err(|err| err.to_string())?;
        scope.get_value("journal_entry")
            .ok_or_else(|| "the script replaced journal_entry with something that isn't an entry".to_string())
    }
    /// Call `f` as a script runs, with how many operations it's done so
    /// far: to show that it's working.
    pub fn on_progress(&mut self, f: impl Fn(u64) + 'static) {
        self.engine.on_progress(move |ops| {
            f(ops);
            None
        });
    }
    pub fn build_entry_file(&mut self, rhai_filename: &str) -> Result<Entry, String> {
        let file_contents = std::fs::read_to_string(rhai_filename)
            .map_err(|err| format!("couldn't read {}: {}", rhai_filename, err))?;
        self.build_entry_str(&file_contents).map_err(|err| format!("{}: {}", rhai_filename, err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::canvas::Canvas as JournalCanvas;
    use crate::journal::canvas::PressureProfile;
    use crate::journal::publish;

    fn engine() -> JournalEngine {
        let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_FONTS).expect("Failed to load fonts.json");
        JournalEngine::new(Rc::new(symd))
    }

    const BRUSH: &str = r#"let brush = new_brush("Calligraphy", 0.25, 0.5, 1.0);"#;

    #[test]
    fn new_style_accepts_css_colors() {
        let script = format!(r#"{}
            let a = new_style(1.5, "darkred", brush);
            let b = new_style(1.5, "hsl(160, 50%, 40%, 0.8)", brush);
        "#, BRUSH);
        assert!(engine().build_entry_str(&script).is_ok());
    }

    #[test]
    fn new_style_rejects_bad_color_with_line() {
        let script = format!("{}\nlet bad = new_style(1.5, \"darkredd\", brush);", BRUSH);
        let err = engine().build_entry_str(&script).expect_err("bad color should fail").to_string();
        assert!(err.contains("darkredd"), "{}", err);
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn new_brush_rejects_unknown_name_with_line() {
        let err = engine().build_entry_str("let ok = new_brush(\"Ink Pen\", 0.2, 0.5, 1.0);\nlet bad = new_brush(\"Crayon\", 0.2, 0.5, 1.0);")
            .expect_err("unknown brush should fail").to_string();
        assert!(err.contains("unknown brush 'Crayon'"), "{}", err);
        assert!(err.contains("Calligraphy"), "error should list available brushes: {}", err);
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn brush_names_lists_what_new_brush_takes() {
        let names: rhai::Array = engine().engine.eval("brush_names()").unwrap();
        let names: Vec<String> = names.into_iter().map(|n| n.into_string().unwrap()).collect();
        assert!(names.contains(&"Calligraphy".to_string()) && names.contains(&"Ink Pen".to_string()), "{:?}", names);
        assert!(names.windows(2).all(|w| w[0] < w[1]));
        for name in &names {
            engine().engine.run(&format!("new_brush({:?}, 0.2, 0.5, 1.0);", name)).unwrap();
        }
    }

    #[test]
    fn new_brush_rejects_bad_pressures_and_sizes() {
        for (args, want) in [
            ("0.8, 0.2, 1.0", "0 <= min <= max <= 1, got min 0.8 and max 0.2"),
            ("-0.1, 0.5, 1.0", "got min -0.1"),
            ("0.2, 1.5, 1.0", "max 1.5"),
            ("0.2, 0.5, 0.0", "size must be positive, got 0"),
            ("0.2, 0.5, -1.0", "size must be positive"),
        ] {
            let err = engine().build_entry_str(&format!("let ok = 1;\nnew_brush(\"Ink Pen\", {});", args))
                .expect_err(args).to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
        // Equal pressures (a fixed pressure) are fine.
        assert!(engine().build_entry_str("new_brush(\"Calligraphy\", 1.0, 1.0, 0.8); journal_entry.add_page(new_page());").is_ok());
    }

    #[test]
    fn new_style_requires_brush() {
        let err = engine().build_entry_str(r#"let s = new_style(1.5, "black");"#)
            .expect_err("brushless style should fail").to_string();
        assert!(err.contains("new_style"), "{}", err);
    }

    fn path_string(entry_script: &str) -> String {
        let engine = engine();
        let path: Path = engine.engine.eval(entry_script).expect("script should evaluate");
        path.get().to_string()
    }

    #[test]
    fn path_functions_mutate_in_place_and_chain() {
        // Statement form mutates the variable...
        let statements = path_string(r#"
            let p = new_path();
            p.move_to(new_vec(0.0, 0.0));
            p.line_to(new_vec(1.0, 0.0));
            p.scale_by(2.0);
            p.translate_by(new_vec(1.0, 1.0));
            p
        "#);
        // ...and chaining (including on temporaries) gives the same result.
        let chained = path_string(r#"
            new_path().move_to(new_vec(0.0, 0.0)).line_to(new_vec(1.0, 0.0))
                .scale_by(2.0).translate_by(new_vec(1.0, 1.0))
        "#);
        let reassigned = path_string(r#"
            let p = new_path();
            p = p.move_to(new_vec(0.0, 0.0));
            p = p.line_to(new_vec(1.0, 0.0));
            p = p.scale_by(2.0);
            p = p.translate_by(new_vec(1.0, 1.0));
            p
        "#);
        // Chains on a variable change the variable by every call.
        let chained_variable = path_string(r#"
            let p = new_path();
            p.move_to(new_vec(0.0, 0.0)).line_to(new_vec(1.0, 0.0)).scale_by(2.0).translate_by(new_vec(1.0, 1.0));
            p
        "#);
        assert_eq!(statements, "M 1 1 L 3 1 ");
        assert_eq!(chained, statements);
        assert_eq!(reassigned, statements);
        assert_eq!(chained_variable, statements);
    }

    #[test]
    fn copies_share_until_copied_and_adding_takes_a_snapshot() {
        let script = r#"
            let a = new_line(new_vec(0.0, 0.0), new_vec(1.0, 0.0));
            let b = a;
            b.translate_by(new_vec(5.0, 0.0));
            if a.to_string() != b.to_string() { throw "assignment should share"; }
            let c = a.copy();
            c.translate_by(new_vec(5.0, 0.0));
            if a.to_string() == c.to_string() { throw "copy() should be independent"; }

            let style = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let canvas = new_canvas();
            canvas.add_path(a, style);
            a.translate_by(new_vec(100.0, 0.0));   // after adding: the canvas keeps its copy
            let page = new_page();
            page.add_canvas(canvas);
            canvas.add_path(a, style);        // after adding: the page keeps its copy
            journal_entry.add_page(page);

            // Loop variables share too, so loops can change array elements.
            let gs = font("loche_upper").glyphs("ab");
            for g in gs { g.scale = 7.0; }
            if gs[0].scale != 7.0 || gs[1].scale != 7.0 { throw "loop should change the glyphs"; }
            // A canvas can add itself.
            canvas.add_canvas(canvas);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let strokes = &entry.pages[0].canvases()[0].strokes;
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].path.to_string(), "M 5 0 L 6 0 ");
    }

    #[test]
    fn shape_and_curve_functions_build_paths() {
        assert_eq!(path_string("new_line(new_vec(0.0, 0.0), new_vec(2.0, 3.0))"), "M 0 0 L 2 3 ");
        assert_eq!(path_string("new_rectangle(new_vec(0.0, 0.0), 2.0, 4.0)"), "M -1 -2 L 1 -2 L 1 2 L -1 2 Z ");
        assert_eq!(
            path_string("new_path().move_to(new_vec(0.0, 0.0)).quad_to(new_vec(1.0, 1.0), new_vec(2.0, 0.0)).cubic_to(new_vec(3.0, 1.0), new_vec(4.0, 1.0), new_vec(5.0, 0.0)).close()"),
            "M 0 0 Q 1 1 2 0 C 3 1 4 1 5 0 Z ",
        );
        // A circle is four cubic quarter-arcs.
        assert_eq!(path_string("new_circle(new_vec(0.0, 0.0), 1.0)").matches('C').count(), 4);
    }

    #[test]
    fn sized_pages_are_their_size_whatever_is_drawn() {
        let entry = engine().build_entry_str(r#"
            let c = new_canvas();
            c.add_path(new_line(new_vec(10, 10), new_vec(500, 10)), new_style(1, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0)));
            let p = new_page();
            p.name = "sized";
            p.size = new_vec(200, 100);
            p.add_canvas(c);
            if p.size.x != 200.0 || p.size.y != 100.0 { throw "the size reads back"; }
            journal_entry.add_page(p);
            journal_entry.add_pages(p.split(2, 1));
            let q = new_page();
            q.size = new_vec(10, 10);
            q.size = ();
            if q.size != () { throw "() clears the size"; }
        "#).expect("script should run");
        let whole = parry2d::bounding_volume::Aabb::new(parry2d::math::Vec2::ZERO, parry2d::math::Vec2::new(200.0, 100.0));
        let (_, canvas) = entry.pages[0].rendered_canvases().next().unwrap();
        assert_eq!(canvas.frame().unwrap().page, whole, "the page is its size, though the line runs past it");
        assert!(!canvas.frame().unwrap().is_tile());
        // Its tiles divide the size, not the strokes.
        let (_, tile) = entry.pages[2].rendered_canvases().next().unwrap();
        assert_eq!((tile.frame().unwrap().page, tile.frame().unwrap().column), (whole, 1));
        for (script, want) in [("p.size = 3;", "page.size: expected a vec"), ("p.size = new_vec(0, 10);", "at least 1")] {
            let err = engine().build_entry_str(&format!("let p = new_page();\n{}", script)).expect_err(script);
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn missing_glyphs_are_drawn_and_reported() {
        let script = r#"
            let style = new_style(1.5, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = font("copper");
            let canvas = new_line(new_vec(0.0, 0.0), new_vec(400.0, 0.0)).fit_text("a~b~", font, style);
            let missing = missing_glyphs();
            if missing["~"].count != 2 { throw "expected ~ twice, got " + missing; }
            // The font, then its fallback.
            if missing["~"].tried != ["copper '~'", "loche_upper '~'"] { throw "unexpected tried list: " + missing["~"].tried; }
        "#;
        engine().build_entry_str(script).expect("script should run");
    }

    /// The svg of the first page of `script`'s entry, built by a fresh engine.
    fn page_svg(script: &str) -> String {
        let entry = engine().build_entry_str(script).expect("script should run");
        let canvas: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        canvas.to_svg(None, 0)
    }

    #[test]
    fn glyph_variants_follow_the_seed() {
        // copper_lower has several variants of most letters, so the text's
        // glyphs depend on which variant each pick lands on.
        let script = |seed: i64| format!(r#"{}
            seed({});
            let style = new_style(1.5, "black", brush);
            let font = font("copper");
            let path = new_path().move_to(new_vec(0.0, 0.0)).line_to(new_vec(2000.0, 0.0));
            let page = new_page();
            page.add_canvas(path.fit_text("a banana and an aardvark", font, style));
            journal_entry.add_page(page);
        "#, BRUSH, seed);
        // Each engine loads its own dictionary, so this also checks the pick
        // doesn't depend on hash map order.
        assert_eq!(page_svg(&script(3)), page_svg(&script(3)));
        assert_ne!(page_svg(&script(3)), page_svg(&script(4)));
    }

    const NAMED: &str = r#"
        let style = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
        let ring = new_canvas();
        ring.add_path(new_circle(new_vec(0.0, 0.0), 10.0), style);
        let named = ring.copy();
        named.name = "ring";
        let maze = new_page();
        maze.name = "maze";
        maze.add_canvas(named);
        maze.add_canvas(ring);
        journal_entry.add_page(new_page());
        journal_entry.add_page(maze);
    "#;

    #[test]
    fn pages_and_canvases_are_labelled_by_name_or_index() {
        let entry = engine().build_entry_str(NAMED).expect("script should run");
        let labels: Vec<String> = entry.pages.iter().enumerate().map(|(i, p)| p.label(i)).collect();
        assert_eq!(labels, ["page_0", "maze"]);
        let canvases: Vec<String> = entry.pages[1].canvases().iter().enumerate().map(|(i, c)| c.label(i)).collect();
        assert_eq!(canvases, ["ring", "1"]);

        let dir = std::env::temp_dir().join(format!("journal_names_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        publish::svgs(&entry, dir.to_str().unwrap(), 0.1, Some(&[1])).expect("publish should succeed");
        assert!(dir.join("maze_ring.png").is_file() && dir.join("maze_1.png").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn text_that_cannot_fit_its_path_is_an_error() {
        let setup = r#"
            let s = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = font("copper");
            let short = new_line(new_vec(0.0, 0.0), new_vec(10.0, 0.0));
        "#;
        for (script, want) in [
            ("short.fill_with_text(\"abc\", font, 50.0, s);", "fill_with_text: glyph 1 of 3 doesn't fit"),
            ("short.fill_with_text(\"abc\", font, 0.0, s);", "font size must be positive"),
            ("new_line(new_vec(0.0, 0.0), new_vec(0.01, 0.0)).fit_text(\"a long message\", font, s);", "fit_text: the text doesn't fit"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", setup, script)).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
        // A path that fits a few glyphs at a time still gets the whole message.
        let parts: rhai::Array = engine().engine.eval(&format!("{}\nnew_line(new_vec(0.0, 0.0), new_vec(60.0, 0.0)).fill_with_text(\"abcdefghijklmnopqrstuvwxyz\", font, 10.0, s)", setup)).unwrap();
        assert!(parts.len() > 1);
    }

    /// Where each glyph of `msg` lands along a straight line, at font size
    /// 20, with `options`.
    fn glyph_xs(msg: &str, options: &str) -> Vec<f32> {
        let script = format!(r#"
            let s = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = font("loche_upper");
            let line = new_line(new_vec(0.0, 0.0), new_vec(2000.0, 0.0));
            let page = new_page();
            for c in line.fill_with_text("{}", font, 20.0, s{}) {{ page.add_canvas(c); }}
            journal_entry.add_page(page);
        "#, msg, options);
        let entry = engine().build_entry_str(&script).expect("script should run");
        entry.pages[0].canvases()[0].strokes.iter().map(|stroke| {
            let xs: Vec<f32> = stroke.path.points_within().iter().map(|p| p.x).collect();
            xs.iter().sum::<f32>() / xs.len() as f32
        }).collect()
    }

    #[test]
    fn text_sits_at_glyph_widths_plus_spacing() {
        // Narrow glyphs used to be pushed a whole glyph unit apart, however
        // narrow they were.
        let tight = glyph_xs("iiii", "");
        let gaps: Vec<f32> = tight.windows(2).map(|w| w[1] - w[0]).collect();
        let i_width = {
            let symd = FontBook::from_file(crate::journal::fonts::DEFAULT_FONTS).unwrap();
            let i = symd.font("loche_upper").unwrap().variants("I").unwrap().remove(0);
            i.hull_aabb().extents().x * 20.0 / 2.0
        };
        assert!(gaps.iter().all(|g| (g - i_width).abs() < 0.5), "gaps {:?}, i is {} wide", gaps, i_width);
        // spacing adds a fraction of the font size between glyphs.
        let spaced = glyph_xs("iiii", ", #{ spacing: 0.5 }");
        let spaced_gaps: Vec<f32> = spaced.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(spaced_gaps.iter().zip(&gaps).all(|(s, g)| (s - g - 10.0).abs() < 0.5), "{:?} vs {:?}", spaced_gaps, gaps);
        let err = engine().build_entry_str("new_line(new_vec(0.0, 0.0), new_vec(9.0, 0.0)).fill_with_text(\"a\", font(\"loche_upper\"), 2.0, new_style(1.0, \"black\", new_brush(\"Ink Pen\", 0.3, 0.7, 1.0)), #{ spacng: 1 });")
            .expect_err("unknown option").to_string();
        assert!(err.contains("unknown option 'spacng'"), "{}", err);
    }

    #[test]
    fn text_on_a_looping_path_still_avoids_itself() {
        // A small circle can't hold the message twice over: glyphs coming
        // round again must not land on the first ones.
        let script = r#"
            let s = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = font("loche_upper");
            let ring = new_circle(new_vec(0.0, 0.0), 30.0);
            ring.fill_with_text("abcdefghijklmnopqrstuvwxyz", font, 20.0, s).len()
        "#;
        let copies: i64 = engine().engine.eval(script).expect("should run");
        assert!(copies > 1, "the circle should need more than one copy, got {}", copies);
    }

    #[test]
    fn text_along_a_path_keeps_clear_of_what_it_avoids() {
        // A line of text crossed by a post at x = 300: no glyph's outline
        // comes within the clearance (2) of it, and with the words layout no
        // word is split around it.
        let script = |layout: &str| format!(r#"
            let s = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let font = font("loche_upper");
            let post = new_line(new_vec(300.0, -100.0), new_vec(300.0, 100.0));
            let line = new_line(new_vec(0.0, 0.0), new_vec(2000.0, 0.0));
            let page = new_page();
            page.add_canvas(line.fill_with_text("abcdefghijklmnopqrstuvwxyz the quick brown fox jumps over the lazy dog", font, 20.0, s, #{{ avoid: post, clearance: 2.0, layout: "{}" }})[0]);
            journal_entry.add_page(page);
        "#, layout);
        for layout in ["flat", "words"] {
            let entry = engine().build_entry_str(&script(layout)).expect("script should run");
            let strokes = &entry.pages[0].canvases()[0].strokes;
            // Strokes are drawn with their hulls inside the glyph's, so check
            // stroke points: the closest may be just under 2 from the hull.
            let mut left = 0;
            for stroke in strokes {
                let xs: Vec<f32> = stroke.path.points_within().iter().map(|p| p.x).collect();
                let (min, max) = xs.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
                assert!(max < 298.5 || min > 301.5, "{}: a glyph spans {}..{}, over the post", layout, min, max);
                if max < 300.0 { left += 1; }
            }
            assert!(left > 0 && left < strokes.len(), "{}: text should be on both sides of the post", layout);
        }
        // "words" moves the whole word that would straddle the post.
        let words = engine().build_entry_str(&script("words")).unwrap();
        let flat = engine().build_entry_str(&script("flat")).unwrap();
        let first_after = |entry: &Entry| entry.pages[0].canvases()[0].strokes.iter()
            .map(|s| s.path.points_within().iter().map(|p| p.x).fold(f32::MAX, f32::min))
            .find(|x| *x > 300.0).unwrap();
        assert!(first_after(&words) >= first_after(&flat));
    }

    const SPLIT: &str = r##"
        let pen = new_brush("Ink Pen", 0.3, 0.7, 3.0);
        let c = new_canvas();
        for i in 0..8 {
            c.add_path(new_line(new_vec(i * 30.0, 0.0), new_vec(300.0 - i * 20.0, 200.0)), new_style(2.0, "#203040", pen));
        }
        c.add_path(new_circle(new_vec(150.0, 100.0), 90.0), new_style(3.0, "darkred", pen));
        let other = new_canvas();
        other.name = "dot";
        other.add_path(new_circle(new_vec(20.0, 20.0), 5.0), new_style(1.0, "black", pen));
        let p = new_page();
        p.name = "big";
        p.svg_style = #{ roughness: 1.5, noise_amplitude: 1.0 };
        p.add_canvas(c);
        p.add_canvas(other);
        p.add_canvas(new_canvas());
        journal_entry.add_page(p);
    "##;

    fn png_pixels(file: &std::path::Path) -> image::RgbaImage {
        image::open(file).unwrap_or_else(|e| panic!("{}: {}", file.display(), e)).to_rgba8()
    }

    #[test]
    fn split_pages_render_tiles_that_make_the_whole() {
        let script = format!("{}\njournal_entry.add_pages(p.split(3, 2));", SPLIT);
        let entry = engine().build_entry_str(&script).expect("script should run");
        let labels: Vec<String> = entry.pages.iter().enumerate().map(|(i, p)| p.label(i)).collect();
        assert_eq!(labels, ["big", "big_0_0", "big_0_1", "big_0_2", "big_1_0", "big_1_1", "big_1_2"]);
        // Tiles keep the canvases' names, and not the empty canvas.
        let names: Vec<String> = entry.pages[1].canvases().iter().enumerate().map(|(i, c)| c.label(i)).collect();
        assert_eq!(names, ["0", "dot"]);
        assert!(entry.empty_canvases() == ["big/2"], "{:?}", entry.empty_canvases());

        let dir = std::env::temp_dir().join(format!("journal_split_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        publish::svgs(&entry, dir.to_str().unwrap(), 0.75, None).expect("publish should succeed");
        let whole = png_pixels(&dir.join("big_0.png"));
        let tiles: Vec<image::RgbaImage> = (0..6).map(|i| png_pixels(&dir.join(format!("big_{}_{}_0.png", i / 3, i % 3)))).collect();
        // Side by side, in order, they make the whole canvas's render.
        let widths: u32 = tiles[..3].iter().map(|t| t.width()).sum();
        let heights: u32 = [&tiles[0], &tiles[3]].iter().map(|t| t.height()).sum();
        assert_eq!((widths, heights), whole.dimensions());
        let mut worst = 0;
        let (mut y0, mut diff_sum) = (0, 0u64);
        for row in 0..2 {
            let mut x0 = 0;
            for column in 0..3 {
                let tile = &tiles[row * 3 + column];
                assert!(tile.height() == tiles[row * 3].height(), "tiles in a row share a height");
                for (x, y, p) in tile.enumerate_pixels() {
                    let q = whole.get_pixel(x0 + x, y0 + y);
                    // Compare premultiplied, so transparent pixels' colors don't count.
                    for c in 0..4 {
                        let (a, b) = (p[c] as i32 * p[3] as i32 / 255, q[c] as i32 * q[3] as i32 / 255);
                        let d = (a - b).abs();
                        worst = worst.max(d);
                        diff_sum += d as u64;
                    }
                }
                x0 += tile.width();
            }
            y0 += tiles[row * 3].height();
        }
        // Anti-aliasing can differ a hair where a stroke's outline crosses
        // a seam; nowhere more.
        assert!(worst <= 12, "a pixel differs by {}", worst);
        assert!((diff_sum as f64) / (whole.len() as f64) < 0.01, "mean difference {}", diff_sum as f64 / whole.len() as f64);
        // Every canvas's tiles cover the whole page, the same pixels each.
        assert_eq!(png_pixels(&dir.join("big_1_2_dot.png")).dimensions(), tiles[5].dimensions());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn split_takes_tile_sizes_and_checks_its_options() {
        let script = format!("{}\nlet tiles = p.split(#{{ tile_width: 100, rows: 2 }});\nif tiles.len() != 8 {{ throw \"expected 4 x 2, got \" + tiles.len(); }}\nif tiles[5].tile.column != 1 || tiles[5].tile.rows != 2 {{ throw \"bad tile: \" + tiles[5].tile; }}\nif p.tile != () {{ throw \"the page isn't a tile\"; }}\njournal_entry.add_pages(tiles);", SPLIT);
        let entry = engine().build_entry_str(&script).expect("script should run");
        // 100-unit tiles: the page's ~310 units grow to 400, evenly.
        let canvas: JournalCanvas = entry.pages[1].canvases()[0].clone().into();
        let frame = canvas.frame().expect("tiles have frames");
        assert!((frame.page.extents().x - 400.0).abs() < 1e-3, "{:?}", frame.page);
        let whole = entry.pages[0].render_canvas(&entry.pages[0].canvases()[0]).unwrap().bounds();
        assert!((frame.page.center().x - whole.center().x).abs() < 1e-3);
        for (script, want) in [
            ("let q = new_page();\nq.split(2, 2);", "name the page first"),
            ("let q = new_page(); q.name = \"q\";\nq.split(2, 2);", "nothing drawn on it"),
            ("p.split(0, 2);", "'columns' should be a whole number from 1 to 1000"),
            ("p.split(#{ columns: 2, tile_width: 10 });", "give 'columns' or 'tile_width', not both"),
            ("p.split(#{ tile_height: -1 });", "'tile_height' must be positive"),
            ("p.split(#{ colums: 2 });", "unknown option 'colums'"),
            ("p.split(2, 2)[0].split(2, 2);", "already a tile"),
            ("journal_entry.add_pages([p.split(2, 2)[0], 3]);", "add_pages: item 1 is a i64"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", SPLIT, script)).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn tiles_keep_their_strokes_seeds_from_the_whole_page() {
        let script = format!("{}\njournal_entry.add_pages(p.split(2, 2));", SPLIT);
        let entry = engine().build_entry_str(&script).expect("script should run");
        let whole: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        let tile: JournalCanvas = entry.pages[4].canvases()[0].clone().into();
        assert_eq!(whole.stroke_seeds(), tile.stroke_seeds());
        assert!(tile.strokes().iter().all(|s| s.seed.is_some()));
    }

    #[test]
    fn degenerate_canvases_render_or_are_skipped() {
        let script = r#"
            let s = new_style(0.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let line = new_canvas();
            line.add_path(new_line(new_vec(0.0, 0.0), new_vec(0.0, 100.0)), s);
            let p = new_page();
            p.add_canvas(line);
            p.add_canvas(new_canvas());
            journal_entry.add_page(p);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        assert_eq!(entry.empty_canvases(), vec!["page_0/1".to_string()]);
        let line: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        assert_eq!((line.render_extents().x, line.render_extents().y), (1.0, 100.0));
        let dir = std::env::temp_dir().join(format!("journal_degenerate_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        publish::svgs(&entry, dir.to_str().unwrap(), 1.0, None).expect("publish should succeed");
        assert!(dir.join("page_0_0.png").is_file() && !dir.join("page_0_1.png").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn non_finite_coordinates_fail_where_they_enter() {
        let setup = r#"
            let s = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let c = new_canvas();
        "#;
        for (script, want) in [
            ("new_vec(1.0 / 0.0, 0.0);", "new_vec: x must be finite"),
            ("new_vec(0.0, 0.0 / 0.0);", "new_vec: y must be finite"),
            ("new_circle(new_vec(0.0, 0.0), 5.0).scale_by(0.0 / 0.0);", "scale_by: the scale must be finite"),
            // Finite, but past what paths hold.
            ("c.add_path(new_circle(new_vec(0.0, 0.0), 5.0).scale_by(1e300), s);", "add_path: the path has a non-finite"),
            ("c.add_glyph(font(\"symbols\").glyph(\"basic_eye\").scale_by(1e300), s);", "add_glyph: the path has a non-finite"),
        ] {
            let err = engine().build_entry_str(&format!("{}\n{}", setup, script)).expect_err(script).to_string();
            assert!(err.contains(want), "{}: {}", want, err);
        }
    }

    #[test]
    fn margins_default_to_half_the_largest_brush() {
        let script = r#"
            let thin = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let wide = new_style(1.0, "black", new_brush("Ink Pen", 0.3, 0.7, 6.0));
            let c = new_canvas();
            c.add_path(new_line(new_vec(0.0, 0.0), new_vec(10.0, 10.0)), thin);
            c.add_path(new_line(new_vec(0.0, 0.0), new_vec(10.0, 0.0)), wide);
            let auto = new_page();
            if auto.margin != () { throw "default margin should read as ()"; }
            auto.add_canvas(c);
            journal_entry.add_page(auto);
            let set = new_page();
            set.margin = 20;
            set.add_canvas(c);
            journal_entry.add_page(set);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let canvas = &entry.pages[0].canvases()[0];
        assert_eq!(entry.pages[0].margin_for(canvas), 3.0);
        assert_eq!(entry.pages[1].margin_for(canvas), 20.0);
        let mut padded: JournalCanvas = canvas.into();
        let before = padded.render_extents();
        padded.pad(3.0);
        assert_eq!(padded.render_extents(), before + parry2d::math::Vec2::splat(6.0));
        for (value, want) in [("-1", "at least 0"), ("\"wide\"", "expected a number or ()")] {
            let err = engine().build_entry_str(&format!("let p = new_page();\np.margin = {};", value)).expect_err(value).to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn warn_reports_with_the_line_and_keeps_going() {
        let mut engine = engine();
        engine.build_entry_str("let x = 1;\nwarn(\"too many \" + 3);\njournal_entry.add_page(new_page());")
            .expect("warn shouldn't stop the script");
        assert_eq!(engine.warnings(), vec!["too many 3 (line 2)".to_string()]);
    }

    #[test]
    fn pages_are_found_by_index_or_name() {
        let entry = engine().build_entry_str(NAMED).expect("script should run");
        assert_eq!(entry.page_index("0"), Ok(0));
        assert_eq!(entry.page_index("maze"), Ok(1));
        assert_eq!(entry.page_index("page_0"), Ok(0));
        let err = entry.page_index("2").unwrap_err();
        assert!(err.contains("pages are 0-1: page_0, maze"), "{}", err);
        assert!(entry.page_index("lock").is_err());
    }

    #[test]
    fn names_must_be_unique_and_file_safe() {
        for (script, want) in [
            ("let p = new_page(); p.name = \"maze\"; journal_entry.add_page(p);\njournal_entry.add_page(p);", "already has a page named 'maze'"),
            // An unnamed page's label counts too.
            ("let p = new_page(); p.name = \"page_1\"; journal_entry.add_page(p);\njournal_entry.add_page(new_page());", "already has a page named 'page_1'"),
            ("let p = new_page(); let c = new_canvas(); c.name = \"a\"; p.add_canvas(c);\np.add_canvas(c);", "already has a canvas named 'a'"),
            ("let p = new_page();\np.name = \"../escape\";", "page.name: invalid name '../escape'"),
            ("let c = new_canvas();\nc.name = 3;", "canvas.name: a name must be a string"),
        ] {
            let err = engine().build_entry_str(script).err().unwrap_or_else(|| panic!("should fail: {}", want)).to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn svg_style_defaults_merges_and_turns_off() {
        let script = r#"
            let p = new_page();
            if p.name != () { throw "unnamed pages have no name"; }
            if p.svg_style.roughness != 1.0 || p.svg_style.noise_amplitude != 0.3 { throw "unexpected default: " + p.svg_style; }
            p.svg_style = #{ roughness: 2, noise_amplitude: 1.5 };
            if p.svg_style.roughness != 2.0 || p.svg_style.trail_off_percent != 0.15 { throw "not merged: " + p.svg_style; }
            journal_entry.add_page(p);
            let plain = new_page();
            plain.svg_style = ();
            journal_entry.add_page(plain);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let style = entry.pages[0].svg_style.as_ref().expect("styled");
        assert_eq!((style.roughness, style.point_sample_distance, style.noise_amplitude), (2.0, 1.0, 1.5));
        assert!(entry.pages[1].svg_style.is_none());
        for (script, want) in [
            ("let p = new_page();\np.svg_style = #{ roughnes: 1 };", "unknown option 'roughnes'"),
            ("let p = new_page();\np.svg_style = #{ roughness: -1 };", "'roughness' can't be negative"),
            ("let p = new_page();\np.svg_style = 2;", "expected a map of options or ()"),
        ] {
            let err = engine().build_entry_str(script).expect_err("script should fail").to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn brushes_carry_a_pressure_profile() {
        let script = r#"
            let b = new_brush("Ink Pen", 0.2, 0.8, 1.0);
            if b.pressure_profile != "ramp" { throw "default should be ramp"; }
            b.pressure_profile = "taper";
            let c = new_canvas();
            c.add_path(new_line(new_vec(0.0, 0.0), new_vec(10.0, 0.0)), new_style(1.0, "black", b));
            let p = new_page();
            p.add_canvas(c);
            journal_entry.add_page(p);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let jc: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        assert_eq!(jc.strokes()[0].style.brush.as_ref().unwrap().pressure_profile, PressureProfile::Taper);
        let err = engine().build_entry_str("let b = new_brush(\"Ink Pen\", 0.2, 0.8, 1.0);\nb.pressure_profile = \"wobbly\";")
            .expect_err("unknown profile should fail").to_string();
        assert!(err.contains("unknown profile 'wobbly'") && err.contains("ramp, taper, constant") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn styles_go_over_their_strokes_in_passes() {
        let script = r#"
            let s = new_style(1.0, "black", new_brush("Ink Pen", 0.2, 0.8, 1.0));
            if s.passes != 1 || s.jitter != 0.0 { throw "one clean pass by default"; }
            let manic = s;
            manic.passes = 5;
            manic.jitter = 3;
            if manic.passes != 5 || manic.jitter != 3.0 || s.passes != 1 { throw "styles are values"; }
            let c = new_canvas();
            let line = new_line(new_vec(0, 0), new_vec(100, 0));
            c.add_path(line, s);
            c.add_path(line.copy().append(new_circle(new_vec(50, 50), 20)), manic);
            // Bounds take in where the passes stray.
            let b = c.bounds();
            if b.max.x <= 101.0 && b.min.x >= -1.0 { throw "bounds miss the passes: " + b; }
            let p = new_page();
            p.add_canvas(c);
            journal_entry.add_page(p);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let canvas = &entry.pages[0].canvases()[0];
        let jc: JournalCanvas = canvas.clone().into();
        let subpaths: Vec<usize> = jc.strokes().iter().map(|s| s.path.to_substrokes().len()).collect();
        assert_eq!(subpaths, vec![1, 10]);
        // The same every time it's drawn.
        let again: JournalCanvas = canvas.clone().into();
        assert_eq!(jc.strokes()[1].path.to_string(), again.strokes()[1].path.to_string());
        for (script, want) in [
            ("s.passes = 0;", "style.passes: should be from 1 to 64, got 0"),
            ("s.passes = 1.5;", "style.passes: should be a whole number"),
            ("s.jitter = -1;", "style.jitter: can't be negative"),
        ] {
            let err = engine().build_entry_str(&format!("let s = new_style(1, \"black\", new_brush(\"Ink Pen\", 0.2, 0.8, 1));\n{}", script))
                .expect_err("should fail").to_string();
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
    }

    #[test]
    fn per_stroke_brushes_reach_journal_canvas() {
        let script = r#"
            let ink = new_style(1.5, "black", new_brush("Rough Ink", 0.25, 0.5, 1.0));
            let calli = new_style(1.5, "darkred", new_brush("Calligraphy", 1.0, 1.0, 0.8));
            let path = new_path().move_to(new_vec(0.0, 0.0)).line_to(new_vec(10.0, 10.0));
            let canvas = new_canvas();
            canvas.add_path(path, ink);
            canvas.add_path(path, calli);
            let page = new_page();
            page.add_canvas(canvas);
            journal_entry.add_page(page);
        "#;
        let entry = engine().build_entry_str(script).expect("script should run");
        let jc: JournalCanvas = entry.pages[0].canvases()[0].clone().into();
        let brushes: Vec<_> = jc.strokes().iter()
            .map(|s| s.style.brush.clone().expect("rhai strokes always have a brush"))
            .map(|b| (b.name, b.size))
            .collect();
        assert_eq!(brushes, vec![("Rough Ink".to_string(), 1.0), ("Calligraphy".to_string(), 0.8)]);
    }
}
