// Smoke-test fixture: small, so it renders in seconds, but it exercises
// every rhai feature journals rely on. Rendered by scripts/smoke.sh and
// evaluated by tests/journal_scripts.rs.

let ink_pen = new_brush("Ink Pen", 0.3, 0.7, 1.0);
let rough = new_brush("Rough Ink", 0.25, 0.5, 1.0);
let calli = new_brush("Calligraphy", 0.6, 1.0, 0.7);

// Named, hex, hsl-with-alpha and rgba colors.
let outline = new_strokestyle(2.0, "black", ink_pen);
let text = new_strokestyle(1.5, "darkred", rough);
let accent = new_strokestyle(2.0, "#1d2a5c", calli);
let faint = new_strokestyle(1.0, "hsl(160, 50%, 40%, 0.6)", ink_pen);
let shade = new_strokestyle(1.0, "rgba(0, 0, 0, 0.4)", ink_pen);

let font = new_fontset(["copper_upper", "loche_upper"], ["copper_lower", "loche_lower"]);

// ---- page one: shapes, curves, symbols, and text on a path ----
let shapes = new_canvas();
shapes.add_strokepath(new_circle(new_vec(0.0, 0.0), 60.0), outline);
shapes.add_strokepath(new_rectangle(new_vec(160.0, 0.0), 100.0, 70.0), accent);
shapes.add_strokepath(new_line(new_vec(-60.0, 90.0), new_vec(210.0, 90.0)), faint);
shapes.add_strokepath(new_arc(new_vec(0.0, 170.0), 50.0, 0.0, 4.71), shade);
shapes.add_strokepath(
	new_strokepath().move_to(new_vec(100.0, 170.0))
		.quad_to(new_vec(150.0, 100.0), new_vec(200.0, 170.0))
		.cubic_to(new_vec(230.0, 230.0), new_vec(130.0, 250.0), new_vec(100.0, 170.0))
		.close(),
	accent);

// A symbol, transformed with chained calls.
shapes.add_strokepath(
	load_symbol("basic_eye").scale_by(30.0).rotate_around(new_vec(0.0, 0.0), 0.3).translate_by(new_vec(0.0, 0.0)),
	outline);

// Text fitted along an arc.
shapes.add_canvas(new_arc(new_vec(80.0, 60.0), 150.0, 3.4, 6.0).fit_msg_to(text, font, "smoke and mirrors"));

let page_one = new_page();
page_one.add_canvas(shapes);
journal_entry.add_page(page_one);

// ---- page two: a maze filled with text, and a second canvas on the page ----
let maze_path = left_handed_maze_path(7, 3, 3);
maze_path.scale_by(40.0);
let maze_canvas = maze_path.fit_msg_to(text, font, "Every path leads back here.");
maze_canvas.add_strokepath(maze_path, faint);

let filled = new_line(new_vec(0.0, 0.0), new_vec(200.0, 0.0)).fill_with_msg(accent, font, 14.0, "a line of text");

let page_two = new_page();
page_two.add_canvas(maze_canvas);
page_two.add_canvas(filled[0].translate(new_vec(0.0, 160.0)));
journal_entry.add_page(page_two);
