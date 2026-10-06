// Smoke-test fixture: small, so it renders in seconds, but it exercises
// every rhai feature journals rely on, including lock chains and mazes.
// Rendered by scripts/smoke.sh and evaluated by tests/journal_scripts.rs.

let ink_pen = new_brush("Ink Pen", 0.3, 0.7, 1.0);
let rough = new_brush("Rough Ink", 0.25, 0.5, 1.0);
let calli = new_brush("Calligraphy", 0.6, 1.0, 0.7);
let tapered = new_brush("Ink Pen", 0.1, 1.0, 1.5);
tapered.pressure_profile = "taper";

// Named, hex, hsl-with-alpha and rgba colors.
let outline = new_strokestyle(2.0, "black", ink_pen);
let text = new_strokestyle(1.5, "darkred", rough);
let accent = new_strokestyle(2.0, "#1d2a5c", calli);
let faint = new_strokestyle(1.0, "hsl(160, 50%, 40%, 0.6)", ink_pen);
let shade = new_strokestyle(1.0, "rgba(0, 0, 0, 0.4)", ink_pen);
let swell = new_strokestyle(2.0, "#5c1d2a", tapered);

let font = new_fontset(["copper_upper", "loche_upper"], ["copper_lower", "loche_lower"]);

// ---- page one: shapes, curves, symbols, and text on a path ----
let shapes = new_canvas();
shapes.add_strokepath(new_circle(new_vec(0.0, 0.0), 60.0), outline);
shapes.add_strokepath(new_rectangle(new_vec(160.0, 0.0), 100.0, 70.0), accent);
shapes.add_strokepath(new_line(new_vec(-60.0, 90.0), new_vec(210.0, 90.0)), faint);
shapes.add_strokepath(new_line(new_vec(-60.0, 110.0), new_vec(210.0, 110.0)), swell);
shapes.add_strokepath(new_arc(new_vec(0.0, 170.0), 50.0, 0.0, 4.71), shade);
shapes.add_strokepath(
	new_strokepath().move_to(new_vec(100.0, 170.0))
		.quad_to(new_vec(150.0, 100.0), new_vec(200.0, 170.0))
		.cubic_to(new_vec(230.0, 230.0), new_vec(130.0, 250.0), new_vec(100.0, 170.0))
		.close(),
	accent);

// A symbol, transformed with chained calls, at a seeded random tilt.
seed(3);
shapes.add_strokepath(
	load_symbol("basic_eye").scale_by(30.0).rotate_around(new_vec(0.0, 0.0), random_range(-0.3, 0.3)).translate_by(new_vec(0.0, 0.0)),
	outline);

// Text fitted along an arc.
shapes.add_canvas(new_arc(new_vec(80.0, 60.0), 150.0, 3.4, 6.0).fit_msg_to(text, font, "smoke and mirrors"));

// Glyphs: a ring of ternary glyphs, a lowercase-font word, and a named
// glyph, placed and turned by hand.
let ring = ternary_font("balanced_ternary").glyphs("smoke");
for (g, i) in ring {
	let angle = i * 6.283 / ring.len();
	g.scale = 20.0;
	g.position = new_vec(-150.0 + 40.0 * angle.cos(), 40.0 * angle.sin());
	g.rotate_by(angle);
	shapes.add_glyph(g, outline);
}
// A for loop's variable is a copy, so index to change the glyphs in place.
let word = lowercase_font("loche_upper").glyphs("ash");
for i in 0..word.len() {
	word[i].scale = 18.0;
	word[i].position = new_vec(-180.0 + i * 20.0, 100.0);
}
shapes.add_glyphs(word, text);
shapes.add_glyph(glyph("basic_eye").scale_by(24.0).translate_by(new_vec(-150.0, 0.0)), accent);

// Notes scattered over the page, clear of the drawing and each other.
let notes = shapes.splat_msgs(["smoke rises", "glass breaks"], lowercase_font("loche_upper"), faint,
	#{ symbol_scale: 12.0, width_range: [60, 100], height_range: [30, 50] });
shapes.add_canvas(notes);

let page_one = new_page();
page_one.add_canvas(shapes);
journal_entry.add_page(page_one);

// ---- page two: a maze filled with text, and a second canvas on the page ----
let page_maze = new_maze(7, 3, 3);
if page_maze.cells().len() != 9 + 8 { throw "a 3x3 maze should have 9 nodes and 8 passages"; }
let maze_path = page_maze.path();
maze_path.scale_by(40.0);
let maze_canvas = maze_path.fit_msg_to(text, font, "Every path leads back here.");
maze_canvas.add_strokepath(maze_path, faint);

let filled = new_line(new_vec(0.0, 0.0), new_vec(200.0, 0.0)).fill_with_msg(accent, font, 14.0, "a line of text");

let page_two = new_page();
page_two.plain_style = ();   // plain strokes, unstyled
page_two.add_canvas(maze_canvas);
page_two.add_canvas(filled[0].translate(new_vec(0.0, 160.0)));
journal_entry.add_page(page_two);

// ---- page three: a lock chain drawn as a maze of glyphs ----
let lock = new_lock("smoke_lock")
	.add("sub", "substitution_cipher", #{ base_n: 27 })
	.add("vig", "vigenere_cipher", #{ key_base: 27, plain_base: 27, cipher_base: 27 })
	.add("maze", "maze_lock", #{ input_base: 27, strategy: "RightHanded" })
	.connect("sub.cipher_text", "vig.key")
	.connect("vig.cipher_text", "maze.maze_text")
	.set_input("sub.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS"))
	.set_input("maze.random_seed", symbols(10, "7"));
let plain = "every lock opens";
let result = lock.run(#{ "sub.plain_text": symbols(27, "key"), "vig.plain_text": symbols(27, plain) });
if lock.unlock(result).text("vig.plain_text") != plain.to_upper() {
	throw "smoke lock didn't round-trip";
}

let maze = result.get("maze.generated_maze").maze();
let cell_size = 24.0;
let maze_canvas = new_canvas();
maze_canvas.add_strokepath(maze.path().scale_by(cell_size), faint);
for cell in maze.cells() {
	maze_canvas.add_strokepath(
		load_symbol("balanced_ternary_" + (cell.symbol - 13)).scale_by(cell_size / 2.0)
			.translate_by(new_vec(cell.x * cell_size, cell.y * cell_size)),
		outline);
}
// Shade the maze's start, then flow the key around the glyphs in a box a
// little larger than the maze.
maze_canvas.add_strokepath(new_circle(new_vec(0.0, 0.0), cell_size * 0.75).hatch(3.0, random_range(0.0, 6.28), 1.0), shade);
let b = maze_canvas.bounds();
let around = new_textbox(#{
	center: b.center, width: b.width * 1.3, height: b.height * 1.3,
	symbol_scale: 12.0, avoid: maze_canvas,
});
let key = around.fill(ternary_font("balanced_ternary").glyphs("key key key key key key key key key key"));
maze_canvas.add_glyphs(key.placed, text);

// Words wrapped into two small boxes, side by side.
let boxes = new_textbox(#{ center: new_vec(0.0, 0.0), width: 60, height: 30, symbol_scale: 9.0, layout: "words", jagged_edge: 0.5 })
	.place_all(lowercase_font("loche_upper").glyphs("words wrap at the spaces between them and spill into another box"));
for (block, i) in boxes {
	for g in block {
		maze_canvas.add_glyph(g.translate_by(new_vec(i * 70.0, b.max.y + 40.0)), text);
	}
}

// Named pages and canvases name their output files: lock.png and
// darkly_lock_maze.png, instead of page_2.png and darkly_page_2_0.png.
maze_canvas.name = "maze";
let page_three = new_page();
page_three.name = "lock";
page_three.plain_style = #{ roughness: 2.0, noise_amplitude: 1.5 };
page_three.add_canvas(maze_canvas);
journal_entry.add_page(page_three);
