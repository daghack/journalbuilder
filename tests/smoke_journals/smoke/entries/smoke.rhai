// Smoke-test fixture: small, so it renders in seconds, but it exercises
// every rhai feature journals rely on, including lock chains and mazes.
// Rendered by scripts/smoke.sh and evaluated by tests/journal_scripts.rs.

let steady = new_handwriting_style();

// Named, hex, hsl-with-alpha and rgba colors.
let outline = new_style(new_brush_style("black", "Ink Pen", 1.0, #{ min_pressure: 0.6, max_pressure: 1.0 }), steady);
let text = new_style(new_brush_style("darkred", "Rough Ink", 1.0, #{ min_pressure: 0.375, max_pressure: 0.75 }), steady);
let accent = new_style(new_brush_style("#1d2a5c", "Calligraphy", 0.7, #{ min_pressure: 1.0, max_pressure: 1.0 }), steady);
let faint = new_style(new_brush_style("hsl(160, 50%, 40%, 0.6)", "Ink Pen", 1.0), steady);
let shade = new_style(new_brush_style("rgba(0, 0, 0, 0.4)", "Ink Pen", 1.0), new_handwriting_style(#{ passes: 2, pass_end_jitter: 0.5 }));
let swell = new_style(new_brush_style("#5c1d2a", "Ink Pen", 1.5, #{ min_pressure: 0.2, max_pressure: 1.0, pressure_profile: "taper" }), steady);

let font = glyphset("copper");

// ---- page one: shapes, curves, symbols, and text on a path ----
let shapes = new_canvas();
shapes.add(new_circle(vec2(0.0, 0.0), 60.0), outline);
shapes.add(new_rectangle(vec2(160.0, 0.0), 100.0, 70.0), accent);
shapes.add(new_line(vec2(-60.0, 90.0), vec2(210.0, 90.0)), faint);
shapes.add(new_line(vec2(-60.0, 110.0), vec2(210.0, 110.0)), swell);
shapes.add(new_arc(vec2(0.0, 170.0), 50.0, 0.0, 4.71), shade);
// Gradients along a stroke and across the page, drawn next to each other
// and between solid strokes.
shapes.add(new_arc(vec2(0.0, 170.0), 35.0, 0.0, 4.71), new_style(new_brush_style(new_gradient(["darkred", "gold", "#1d2a5c"]), "Rough Ink", 1.0, #{ min_pressure: 0.5, max_pressure: 1.0 }), steady));
shapes.add(new_rectangle(vec2(0.0, 170.0), 40.0, 30.0).hatch(4.0, #{ angle: [0.5, 2.1] }),
	new_style(new_brush_style(new_gradient(["#1d2a5c", [0.3, "darkred"], "gold"], #{ center: vec2(0.0, 170.0), radius: 25.0 }), "Ink Pen", 1.0), steady));
shapes.add(
	new_path().move_to(vec2(100.0, 170.0))
		.quad_to(vec2(150.0, 100.0), vec2(200.0, 170.0))
		.cubic_to(vec2(230.0, 230.0), vec2(130.0, 250.0), vec2(100.0, 170.0))
		.close(),
	accent);

// A symbol, transformed with chained calls, at a seeded random tilt.
seed(3);
shapes.add(
	glyphset("symbols").glyph("basic_eye").scale_by(2).path().scale_by(30.0).rotate_around(vec2(0.0, 0.0), random_range(-0.3, 0.3)).translate_by(vec2(0.0, 0.0)),
	outline);

// Text fitted along an arc.
shapes.add(new_arc(vec2(80.0, 60.0), 150.0, 3.4, 6.0).fit_text("smoke and mirrors", #{ style: text, type: #{ glyphset: font } }));

// Glyphs: a ring of ternary glyphs, a lowercase-glyphset word, and a named
// glyph, placed and turned by hand.
let ring = ternary_glyphset("balanced_ternary").glyphs("smoke");
for (g, i) in ring {
	let angle = i * 6.283 / ring.len();
	g.scale = 20.0;
	g.position = vec2(-150.0 + 40.0 * angle.cos(), 40.0 * angle.sin());
	g.rotate_by(angle);
	shapes.add(g, outline);
}
// A for loop's variable is a copy, so index to change the glyphs in place.
let word = glyphset("loche_upper").glyphs("ash");
for i in 0..word.len() {
	word[i].scale = 18.0;
	word[i].position = vec2(-180.0 + i * 20.0, 100.0);
}
shapes.add(word, text);
shapes.add(glyphset("symbols").glyph("basic_eye").scale_by(24.0).translate_by(vec2(-150.0, 0.0)), accent);

// Notes scattered over the page, clear of the drawing and each other.
let notes = shapes.scatter_notes(["smoke rises", "glass breaks"], [60, 100], [30, 50], #{ style: faint, type: new_type_style(glyphset("loche_upper"), 12.0) });
shapes.add(notes);

// Shapes from their own constructors, and glyphs round a ring.
shapes.add(new_polygon(vec2(260.0, 170.0), 30.0, 5, #{ step: 2 }), faint);
shapes.add(new_star(vec2(330.0, 170.0), 30.0, 12.0, 6), faint);
shapes.add(new_ellipse(vec2(260.0, 240.0), 40.0, 15.0, #{ angle: 0.3 }), faint);
shapes.add(new_wave(vec2(220.0, 290.0), vec2(360.0, 290.0), #{ amplitude: 8.0 }), faint);
shapes.add(new_polyline([vec2(300.0, 230.0), vec2(330.0, 250.0), vec2(360.0, 230.0)], #{ closed: true }), faint);
for at in new_circle(vec2(330.0, 0.0), 40.0).points_along(8, #{ start: 0.75 }) {
	shapes.add(ternary_glyphset("balanced_ternary").glyph(balanced_ternary(3), #{ position: at.position, scale: 14.0, rotation: at.angle }), outline);
}

journal_entry.add(new_page().add(shapes));

// ---- page two: a maze filled with text, and a second canvas on the page ----
let page_maze = new_maze(3, 3, #{ seed: 7 });
if page_maze.cells().len() != 9 + 8 { throw "a 3x3 maze should have 9 nodes and 8 passages"; }
let maze_path = page_maze.path();
maze_path.scale_by(40.0);
let maze_canvas = maze_path.fit_text("Every path leads back here.", #{ style: text, type: #{ glyphset: font } });
maze_canvas.add(maze_path, faint);

let filled = new_line(vec2(0.0, 0.0), vec2(200.0, 0.0)).fill_with_text("a line of text", #{ style: accent, type: new_type_style(font, 14.0) });
// Text along a line that keeps clear of a post crossing it, a closed
// shape, and the inside of a drawing's outline, whole words at a time.
let post = new_line(vec2(60.0, -30.0), vec2(60.0, 30.0));
let keep_off = new_avoidance_tree()
	.add(post, #{ margin: 3.0 })
	.add_region(new_circle(vec2(150.0, 0.0), 12.0))
	.add_hull(new_arc(vec2(220.0, 0.0), 15.0, 0.0, 3.0));
let avoiding = new_line(vec2(0.0, 0.0), vec2(300.0, 0.0))
	.fill_with_text("words go around what they avoid", #{ style: accent, type: new_type_style(font, 12.0), paragraph: #{ layout: "words" }, avoid: keep_off })[0];
avoiding.add(post, faint);
avoiding.add(new_circle(vec2(150.0, 0.0), 12.0), faint);

// A line written straight, a block wrapped to a width, and balanced
// ternary along a line, each glyph a little off true.
let jostled = text;
jostled.glyph_rotation_jitter = 0.05;
jostled.glyph_position_jitter = 0.6;
let hand = #{ style: jostled, type: new_type_style(glyphset("loche_upper"), 12.0) };
let written = write_line("written by hand", vec2(0.0, 260.0), hand);
let block = write_block("a block of words wrapped to a width, line after line", vec2(0.0, 290.0), 120.0, hand + #{ paragraph: #{ line_height: 1.4 } });
written.add(block.canvas);
written.add(new_line(vec2(0.0, block.bottom), vec2(120.0, block.bottom)).fit_text(balanced_ternary(2026), #{ style: accent }));

// Plain strokes, unstyled, in its SVG.
journal_entry.add(new_page(#{ svg_style: () })
	.add(maze_canvas)
	.add(filled[0].translate_by(vec2(0.0, 160.0)))
	.add(avoiding.translate_by(vec2(0.0, 200.0)))
	.add(written));

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
maze_canvas.add(maze.path().scale_by(cell_size), faint);
for cell in maze.cells() {
	maze_canvas.add(
		glyphset("balanced_ternary").glyph((cell.symbol - 13)).scale_by(2).path().scale_by(cell_size / 2.0)
			.translate_by(vec2(cell.x * cell_size, cell.y * cell_size)),
		outline);
}
// Shade the maze's start, then flow the key around the glyphs in a box a
// little larger than the maze.
maze_canvas.add(new_circle(vec2(0.0, 0.0), cell_size * 0.75).hatch(3.0, #{ angle: random_range(0.0, 6.28), roughness: 1.0 }), shade);
let b = maze_canvas.bounds();
let around = new_textbox(#{
	center: b.center, width: b.width * 1.3, height: b.height * 1.3,
	type: #{ glyph_scale: 12.0 }, avoid: maze_canvas,
});
let key = around.fill(ternary_glyphset("balanced_ternary").glyphs("key key key key key key key key key key"));
if key.unplaced.len() > 0 { warn(key.unplaced.len() + " key glyphs didn't fit"); }
maze_canvas.add(key.placed, text);

// Words wrapped into two small boxes, side by side.
let boxes = new_textbox(#{ center: vec2(0.0, 0.0), width: 60, height: 30, type: #{ glyph_scale: 9.0 }, paragraph: #{ layout: "words", line_start_jitter: 4.5 } })
	.place_all(glyphset("loche_upper").glyphs("words wrap at the spaces between them and spill into another box"));
for (block, i) in boxes {
	for g in block {
		maze_canvas.add(g.translate_by(vec2(i * 70.0, b.max.y + 40.0)), text);
	}
}

// Named pages and canvases name their output files: lock_maze.png and
// darkly_lock_maze.png, instead of page_2_0.png and darkly_page_2_0.png.
maze_canvas.name = "maze";
journal_entry.add(new_page(#{ name: "lock", svg_style: #{ roughness: 2.0, noise_amplitude: 1.5 } }).add(maze_canvas));

// ---- page four: procedural art ----
// Two snakes (one with too little slack to loop, one coiled over itself),
// a bramble, and a maze grown with brambles and made a serpent.
let pale_pen = new_brush_style("#f2ead8", "Ink Pen", 1.0, #{ min_pressure: 0.42, max_pressure: 0.98 });
let pale = new_style(pale_pen, steady);
let snakes = new_canvas();
snakes.add(art::snake(vec2(0.0, 0.0), vec2(10.0, 200.0), 260.0, #{ head_angle: -PI() / 2.0, max_width: 14.0, seed: 4 }).outline, pale);
snakes.add(art::snake(vec2(140.0, 0.0), vec2(150.0, 30.0), 480.0, #{ head_angle: -PI() / 2.0, max_width: 14.0, seed: 4 }).outline, pale);
// A bramble, its vines, leaves and flowers stroked separately.
let plant = art::bramble(vec2(320.0, 220.0), 420.0, #{ angle: -PI() / 2.0, flower_size: 70.0, seed: 4 });
pale_pen.color = "#c8a27a";
let vines = new_style(pale_pen, steady);
pale_pen.color = "#a8d68a";
let bramble_inks = #{ vines: vines, leaves: new_style(pale_pen, steady), flowers: pale };
snakes.add(plant, bramble_inks);
// A small maze grown with brambles, and made a many-headed serpent.
let maze_lines = new_maze(3, 2, #{ seed: 5 }).path().scale_by(50.0);
let along = art::bramble_along(maze_lines.copy().translate_by(vec2(0.0, 300.0)), 30.0, #{ seed: 2 });
snakes.add(along, bramble_inks);
snakes.add(art::snake_along(maze_lines.copy().translate_by(vec2(300.0, 300.0)), 20.0, #{ seed: 2 }), pale);
let page_four = new_page(#{ name: "art" }).add(snakes);
journal_entry.add(page_four);
// The same page as a 2 x 2 grid of tiles, art_0_0 to art_1_1, which fit
// together into it.
journal_entry.add(page_four.split(#{ columns: 2, rows: 2 }));

// ---- page five: a typeset page ----
// A layout (here inline; usually a file in layouts/) given text, roles and
// art: a heading, a drop cap the text wraps around, two columns with a band
// between them, and a figure. The text runs on to a second page.
fn smoke_page(n, info) {
	column([
		if n == 0 { flow("title", #{ role: "heading", flex: #{ height: 34 }, paragraph: #{ fit: true } }) } else { spacer(#{ flex: #{ height: 34 } }) },
		row([
			column([
				if n == 0 { initial("body", #{ flex: #{ position: "absolute", top: 0, left: 0, width: 30, height: 30 } }) } else { spacer() },
				flow("body", #{ flex: #{ grow: 1 } }),
			], #{ flex: #{ grow: 1 } }),
			band("dot", #{ flex: #{ width: 8 } }),
			column([
				if n == 0 { figure("eye", #{ flex: #{ height: 60, border: "frame" } }) } else { spacer() },
				flow("body", #{ flex: #{ grow: 1 } }),
			], #{ flex: #{ grow: 1 } }),
		], #{ flex: #{ grow: 1, gap: 12 } }),
	], #{ flex: #{ padding: 20, gap: 8 } })
}
let words = "every path leads back here and every lock opens in the end ";
journal_entry.add(new_layout("book", Fn("smoke_page")).typeset(#{
	width: 360, height: 260,
	flows: #{ title: "the smoke book", body: words + words + words + "\n\n" + words + words + words + words + words + words + words + words + words },
	roles: #{
		body: #{ style: text, type: new_type_style(glyphset("loche_upper"), 11.0), paragraph: #{ align: "justify", indent: 1.0 } },
		heading: #{ style: accent, type: new_type_style(font, 28.0), paragraph: #{ align: "center" } },
		initial: #{ style: accent, type: #{ glyphset: font } },
		dot: #{ style: faint },
		eye: #{ style: outline },
		frame: #{ style: faint },
	},
	art: #{ dot: new_circle(vec2(0.0, 0.0), 2.0), eye: glyphset("symbols").glyph("basic_eye").scale_by(2).path() },
}));
