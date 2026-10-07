// The showcase: everything journals can draw with, one page each.
//
//   pens     every Darkly brush, the pressure profiles, widths, colours
//            and gradients
//   fonts    every font, its alphabet and a sentence
//   glyphs   every glyph in the symbol dictionary, by family
//   art      shapes and paths, hatching, symbols, mazes, snakes, brambles
//   text     text boxes, text along paths, scattered notes, and text kept
//            clear of drawings with avoidance trees
//   locks    every lock type, a lock chain drawn as a maze, and the page
//            split into tiles
//   layout   pages typeset from layouts: an inline one, text chained over
//            two columns and two pages (layout_0, layout_1), and a layout
//            file (title_0)
//
// When journals gain a capability (a function, an art tool, a brush, a
// font, a lock type), it gets a labelled example here: see CLAUDE.md.
// tests/showcase.rs fails when a function scripts can call, or a glyph
// family, isn't in this file. Strokes are light, for a dark background,
// or dark on paper (journalbuild showcase --background parchment).

// Inks: light, for transparent renders viewed on a dark background, or
// dark when the pages are rendered on paper (--background), which
// background() tells.
let on_paper = background() != ();
let pale = if on_paper { "#2b1d12" } else { "#f2ead8" };
let red = if on_paper { "#9b2318" } else { "#ff8a7a" };
let green = if on_paper { "#3d6b2a" } else { "#a8d68a" };
let blue = if on_paper { "#1f4e7a" } else { "#9ad0ff" };
let gold = if on_paper { "#8a6212" } else { "#e8c170" };
let bark = if on_paper { "#6b4423" } else { "#c8a27a" };

let pen = new_brush("Ink Pen", 0.4, 0.8, 1.6);
let label_style = new_strokestyle(1.0, pale, pen);
// A fine nib: loche is hard to read drawn broad.
let heading_nib = new_brush("Calligraphy", 0.4, 0.8, 1.5);
heading_nib.pressure_profile = "constant";
let heading_style = new_strokestyle(1.2, red, heading_nib);
let line_style = new_strokestyle(1.0, pale, new_brush("Ink Pen", 0.3, 0.7, 2.0));
let thin_red = new_strokestyle(1.0, red, new_brush("Ink Pen", 0.3, 0.7, 1.2));

// Labels and body text are in plain, a font made to read easily, with
// capitals, digits and punctuation; headings are in loche.
let label_font = new_fontset(["plain_upper"], ["plain_lower"]);
let heading_font = lowercase_font("loche_upper");

// `text` written from (x, y) to the right, along a line through the middle
// of its glyphs.
fn write(text, x, y, size, style, font) {
	write_with(text, x, y, size, style, font, #{})
}

fn write_with(text, x, y, size, style, font, options) {
	let c = new_canvas();
	let line = new_line(new_vec(x, y), new_vec(x + 5000.0, y));
	for part in line.fill_with_msg(style, font, size, text, options) {
		c.add_canvas(part);
	}
	c
}

// A wave from x0 to x1 about y.
fn wave(x0, x1, y, rise) {
	let step = (x1 - x0) / 3.0;
	let path = new_strokepath().move_to(new_vec(x0, y));
	for i in 0..3 {
		let a = x0 + i * step;
		path.cubic_to(new_vec(a + step / 3.0, y - rise), new_vec(a + 2.0 * step / 3.0, y + rise), new_vec(a + step, y));
	}
	path
}

fn heading(text, style, font) {
	write(text, 0.0, -90.0, 64.0, style, font)
}

// ---- pens: every brush ----
//
// Each brush draws a wave, over bare paper and then over hatching, since
// some (Blur, Smudge, Liquify, Clone) only move ink that's already there.

let pens = new_canvas();
pens.name = "brushes";
pens.add_canvas(heading("pens", heading_style, heading_font));
let y = 0.0;
for name in brush_names() {
	pens.add_canvas(write(name, 0.0, y, 26.0, label_style, label_font));
	pens.add_strokepath(new_rectangle(new_vec(1040.0, y), 420.0, 60.0).hatch(7.0, 0.8, 0.6), thin_red);
	let brush = new_brush(name, 0.3, 0.9, 6.0);
	brush.size = 9.0;
	pens.add_strokepath(wave(360.0, 1250.0, y, 26.0), new_strokestyle(1.0, pale, brush));
	y += 90.0;
}

// Pressure runs along each stroke as its brush's profile says, between the
// brush's min_pressure and max_pressure.
y += 40.0;
pens.add_canvas(write("pressure profiles", 0.0, y, 34.0, heading_style, heading_font));
y += 70.0;
for profile in ["ramp", "taper", "constant"] {
	let brush = new_brush("Ink Pen", 0.05, 1.0, 10.0);
	brush.pressure_profile = profile;
	if brush.min_pressure != 0.05 || brush.max_pressure != 1.0 || brush.brush_style != "Ink Pen" { throw "unexpected brush " + brush; }
	pens.add_canvas(write(profile, 0.0, y, 26.0, label_style, label_font));
	pens.add_strokepath(new_line(new_vec(360.0, y), new_vec(1250.0, y)), new_strokestyle(1.0, gold, brush));
	y += 70.0;
}

// A style's width scales its brush's pressure (in SVG output it's the
// stroke's width).
y += 40.0;
pens.add_canvas(write("widths", 0.0, y, 34.0, heading_style, heading_font));
y += 70.0;
for (width, i) in [0.3, 0.7, 1.2] {
	let style = new_strokestyle(1.0, blue, new_brush("Charcoal", 0.3, 0.8, 8.0));
	style.width = width;
	pens.add_canvas(write(["light", "middling", "heavy"][i], 0.0, y, 26.0, label_style, label_font));
	pens.add_strokepath(wave(360.0, 1250.0, y, 14.0), style);
	y += 70.0;
}

// Colours: CSS names, hex, rgb(), hsl() and alpha.
y += 40.0;
pens.add_canvas(write("colours", 0.0, y, 34.0, heading_style, heading_font));
y += 70.0;
let colours = if on_paper {
	[["named", "darkgoldenrod"], ["hex", "#1f4e7a"], ["rgb", "rgb(155, 35, 24)"], ["hsl", "hsl(100, 50%, 28%)"], ["alpha", "rgba(43, 29, 18, 0.4)"]]
} else {
	[["named", "gold"], ["hex", "#9ad0ff"], ["rgb", "rgb(255, 138, 122)"], ["hsl", "hsl(100, 50%, 70%)"], ["alpha", "rgba(242, 234, 216, 0.4)"]]
};
for (colour, i) in colours {
	let x = i * 260.0;
	let style = new_strokestyle(1.0, colour[1], new_brush("Rough Ink", 0.4, 0.8, 9.0));
	if style.color != colour[1] { throw "colour didn't stick"; }
	pens.add_canvas(write(colour[0], x, y, 24.0, label_style, label_font));
	pens.add_strokepath(wave(x, x + 200.0, y + 50.0, 14.0), style);
}
// A stroke built from a path and a style, then added.
pens.add_stroke(new_stroke(new_line(new_vec(0.0, y + 110.0), new_vec(1250.0, y + 110.0)), thin_red));

// Gradients, in place of a colour: along each stroke from start to end,
// across the page from one point to another, or out from a centre.
// Colours are spaced evenly, or placed as [position, colour].
y += 200.0;
pens.add_canvas(write("gradients", 0.0, y, 34.0, heading_style, heading_font));
y += 80.0;
let ink_brush = new_brush("Rough Ink", 0.4, 0.8, 9.0);
pens.add_canvas(write("new_gradient(colours): along each stroke", 0.0, y, 24.0, label_style, label_font));
pens.add_strokepath(wave(620.0, 1250.0, y, 22.0), new_strokestyle(1.0, new_gradient([red, gold, blue]), ink_brush));
y += 90.0;
pens.add_canvas(write("#{ from, to }: across, colours placed", 0.0, y, 24.0, label_style, label_font));
let across = new_gradient([green, [0.7, gold], red], #{ from: new_vec(620.0, 0.0), to: new_vec(1250.0, 0.0) });
pens.add_strokepath(new_rectangle(new_vec(935.0, y), 630.0, 56.0).hatch(#{ spacing: 8.0, angle: 1.1 }), new_strokestyle(1.0, across, pen));
y += 90.0;
let words = new_strokestyle(1.0, red, pen);
words.color = new_gradient([blue, pale, red], #{ from: new_vec(620.0, 0.0), to: new_vec(1250.0, 0.0) });
pens.add_canvas(write("style.color = a gradient", 0.0, y, 24.0, label_style, label_font));
pens.add_canvas(write("Written in a gradient, left to right", 620.0, y, 30.0, words, label_font));
y += 150.0;
pens.add_canvas(write("#{ center, radius }: out from a point", 0.0, y, 24.0, label_style, label_font));
let glow = new_gradient([pale, gold, if on_paper { "rgba(155, 35, 24, 0.3)" } else { "rgba(255, 138, 122, 0.2)" }], #{ center: new_vec(935.0, y), radius: 110.0 });
let spiral = new_strokepath().move_to(new_vec(935.0, y));
for i in 1..240 {
	let a = i * 0.11;
	spiral.line_to(new_vec(935.0 + a * 4.2 * a.cos(), y + a * 4.2 * a.sin()));
}
pens.add_strokepath(spiral, new_strokestyle(1.0, glow, new_brush("Ink Pen", 0.4, 0.8, 3.0)));

let pens_page = new_page();
pens_page.name = "pens";
pens_page.add_canvas(pens);
journal_entry.add_page(pens_page);

// ---- fonts ----

let pangram = "the quick brown fox jumps over the lazy dog";
let alphabet = "abcdefghijklmnopqrstuvwxyz";
let ink = new_strokestyle(1.0, pale, new_brush("Calligraphy", 0.4, 0.8, 2.0));

let fonts = new_canvas();
fonts.add_canvas(heading("fonts", heading_style, heading_font));
let y = 0.0;
for f in [
	#{ label: "plain: new_fontset([\"plain_upper\"], [\"plain_lower\"]), made to read easily", font: label_font, lines: [alphabet.to_upper(), alphabet, "0123456789  . , ; : ! ? ' \" - — _ ( ) [ ] { } / # + = * < > % &", "The Quick Brown Fox, 1234 times over the lazy dog."] },
	#{ label: "copper: new fontset of copper upper and copper lower", font: new_fontset(["copper_upper"], ["copper_lower"]), lines: [alphabet.to_upper(), alphabet, pangram, "variants picked at random  aaaa eeee ssss"] },
	#{ label: "loche: lowercase font of loche upper", font: lowercase_font("loche_upper"), lines: [alphabet, pangram + ".", "punctuation  . , ; : ' \" ? - —"] },
	#{ label: "loche lower", font: lowercase_font("loche_lower"), lines: [alphabet, pangram + "."] },
	#{ label: "lang, with spacing", font: new_fontset(["lang_upper"], ["lang_lower"]), lines: [alphabet.to_upper(), alphabet, "The quick brown fox, the lazy dog."], options: #{ spacing: 0.12 } },
	#{ label: "eldritch", font: lowercase_font("eldritch"), lines: [alphabet, pangram] },
	#{ label: "sharp lower", font: lowercase_font("sharp_lower"), lines: [alphabet, pangram] },
	#{ label: "balanced ternary: letters to the ternary digits", font: ternary_font("balanced_ternary"), lines: ["abcdefghijklm", "nopqrstuvwxyz", pangram] },
	#{ label: "a font set falling back from copper to loche", font: new_fontset(["copper_upper", "loche_upper"], ["copper_lower", "loche_lower"]), lines: ["Every Path Leads Back Here?"] },
] {
	fonts.add_canvas(write(f.label, 0.0, y, 24.0, thin_red, label_font));
	y += 60.0;
	for line in f.lines {
		fonts.add_canvas(write_with(line, 0.0, y, 44.0, ink, f.font, f.options ?? #{}));
		y += 60.0;
	}
	y += 30.0;
}

// Glyphs one at a time: placed, scaled and turned by hand.
fonts.add_canvas(write("glyphs placed by hand", 0.0, y, 24.0, thin_red, label_font));
y += 70.0;
let word = new_fontset(["copper_upper"], ["copper_lower"]).glyphs("turning");
for (g, i) in word {
	if g.is_blank { continue; }
	g.scale = 50.0;
	g.position = new_vec(40.0 + i * 70.0, y);
	g.rotation = i * 0.2;
	g.translate_by(new_vec(0.0, 8.0 * i)).scale_by(1.0 + 0.05 * i).rotate_by(0.05);
}
fonts.add_glyphs(word, ink);
let eye = glyph("basic_eye").copy();
eye.scale = 80.0;
eye.position = new_vec(700.0, y);
fonts.add_glyph(eye, new_strokestyle(1.0, gold, pen));
fonts.add_strokepath(eye.path().translate_by(new_vec(120.0, 0.0)), new_strokestyle(1.0, blue, pen));

let fonts_page = new_page();
fonts_page.name = "fonts";
fonts_page.add_canvas(fonts);
journal_entry.add_page(fonts_page);

// ---- glyphs: the whole symbol dictionary ----
//
// Every glyph, a row (or more) per family: a name less its last parts of
// three characters or fewer, so copper_lower_a_2 is in copper_lower.

fn family(name) {
	let parts = name.split("_");
	while parts.len() > 1 && parts[parts.len() - 1].len() <= 3 {
		parts.pop();
	}
	let joined = parts[0];
	for part in parts.extract(1) {
		joined += "_" + part;
	}
	joined
}

let catalogue = new_canvas();
catalogue.add_canvas(heading("every glyph", heading_style, heading_font));
let y = 0.0;
let current = ();
let column = 0;
for name in glyph_names("") {
	let f = family(name);
	if f.starts_with("debug") { continue; }
	if f != current || column == 30 {
		if f != current {
			y += if current == () { 0.0 } else { 70.0 };
			catalogue.add_canvas(write(f, 0.0, y, 22.0, thin_red, label_font));
			current = f;
		}
		y += 60.0;
		column = 0;
	}
	let g = glyph(name);
	g.scale = 40.0;
	g.position = new_vec(25.0 + column * 52.0, y);
	catalogue.add_glyph(g, ink);
	column += 1;
}
// load_symbol gives a glyph's path as it's stored, spanning -1..1.
catalogue.add_canvas(write("load symbol", 1300.0, 0.0, 22.0, thin_red, label_font));
catalogue.add_strokepath(load_symbol("ploughdiem").scale_by(40.0).translate_by(new_vec(1520.0, 0.0)), new_strokestyle(1.0, gold, pen));

let glyphs_page = new_page();
glyphs_page.name = "glyphs";
glyphs_page.add_canvas(catalogue);
journal_entry.add_page(glyphs_page);

// ---- art ----

seed(7);
let art = new_canvas();
art.add_canvas(heading("art", heading_style, heading_font));

// Shapes and curves, moved, scaled and turned.
art.add_canvas(write("shapes and curves", 0.0, 0.0, 24.0, thin_red, label_font));
art.add_strokepath(new_circle(new_vec(60.0, 110.0), 50.0), line_style);
art.add_strokepath(new_rectangle(new_vec(200.0, 110.0), 100.0, 80.0).rotate_around(new_vec(200.0, 110.0), 0.2), line_style);
art.add_strokepath(new_line(new_vec(270.0, 60.0), new_vec(370.0, 160.0)), line_style);
art.add_strokepath(new_arc(new_vec(450.0, 110.0), 50.0, 0.0, 4.5), line_style);
art.add_strokepath(
	new_strokepath().move_to(new_vec(520.0, 160.0))
		.quad_to(new_vec(570.0, 40.0), new_vec(620.0, 160.0))
		.cubic_to(new_vec(660.0, 220.0), new_vec(560.0, 240.0), new_vec(520.0, 160.0))
		.close()
		.scale_by(1.0).translate_by(new_vec(10.0, 0.0)),
	new_strokestyle(1.0, blue, pen));
// Hatching fills a closed path's inside with lines: hatch(spacing, angle,
// roughness), here at a random slant.
art.add_strokepath(new_circle(new_vec(740.0, 110.0), 50.0), line_style);
art.add_strokepath(new_circle(new_vec(740.0, 110.0), 50.0).hatch(5.0, random_range(0.0, PI()), 1.0), thin_red);
art.add_strokepath(new_rectangle(new_vec(870.0, 110.0), 90.0, 90.0).hatch(6.0, random() * PI(), 0.5), new_strokestyle(1.0, gold, pen));

// A maze's corridors.
art.add_canvas(write("a maze", 1000.0, 0.0, 24.0, thin_red, label_font));
let maze = new_maze(4, 5, 4);
if maze.width() < 1 || maze.height() < 1 { throw "a maze has a size"; }
art.add_strokepath(maze.path().scale_by(40.0).translate_by(new_vec(1020.0, 60.0)), line_style);

// Snakes: one with too little slack to loop, one coiled over itself, and
// writing along a snake's spine.
art.add_canvas(write("art snake path and snake spine", 0.0, 330.0, 24.0, thin_red, label_font));
art.add_strokepath(art::snake_path(new_vec(60.0, 560.0), -PI() / 2.0, new_vec(80.0, 940.0), 480.0, 40.0, 4), line_style);
art.add_strokepath(art::snake_path(new_vec(300.0, 600.0), -PI() / 2.0, new_vec(310.0, 660.0), 900.0, 36.0, 7), line_style);
let spine = art::snake_spine(new_vec(600.0, 480.0), -PI() / 2.0, new_vec(640.0, 900.0), 640.0, 48.0, 2);
art.add_strokepath(art::snake_path(new_vec(600.0, 480.0), -PI() / 2.0, new_vec(640.0, 900.0), 640.0, 48.0, 2), line_style);
art.add_canvas(spine.fill_with_msg(new_strokestyle(1.0, red, pen), label_font, 22.0, "a snake with words down its back")[0]);

// A bramble: canes, leaves and flowers, stroked separately.
art.add_canvas(write("art bramble", 860.0, 330.0, 24.0, thin_red, label_font));
let plant = art::bramble(new_vec(1060.0, 960.0), -PI() / 2.0, 540.0, 90.0, 4);
art.add_strokepath(plant.vines, new_strokestyle(1.0, bark, pen));
art.add_strokepath(plant.leaves, new_strokestyle(1.0, green, pen));
art.add_strokepath(plant.flowers, line_style);

// Along a path: a maze grown with brambles, and made a many-headed serpent.
art.add_canvas(write("art bramble along and snake along a maze", 0.0, 1060.0, 24.0, thin_red, label_font));
let maze_lines = new_maze(5, 4, 3).path().scale_by(90.0);
let along = art::bramble_along(maze_lines.copy().translate_by(new_vec(30.0, 1150.0)), 40.0, 2);
art.add_strokepath(along.vines, new_strokestyle(1.0, bark, pen));
art.add_strokepath(along.leaves, new_strokestyle(1.0, green, pen));
art.add_strokepath(along.flowers, line_style);
art.add_strokepath(art::snake_along(maze_lines.copy().translate_by(new_vec(700.0, 1150.0)), 30.0, 2), line_style);

// Hatching with options. Each line is a stroke of its own, inside the
// shape: a ring (two circles appended into one path) keeps its hole, and
// lines can cross, bow and stop short of the outline. Fill rules decide
// what's inside where shapes overlap: "holes" (the default) leaves the
// overlap out, "nonzero" merges them, and "evenodd" also leaves out where
// one line crosses itself, like the middle of a star.
art.add_canvas(write("hatch with options: a ring, angles, roughness, bow, inset, and fill rules", 0.0, 1580.0, 24.0, thin_red, label_font));
let ring = new_circle(new_vec(90.0, 1730.0), 80.0).append(new_circle(new_vec(90.0, 1730.0), 40.0));
art.add_strokepath(ring, line_style);
art.add_strokepath(ring.hatch(#{ spacing: 9.0, angle: [0.6, 2.2], roughness: 1.0 }), thin_red);
fn discs(x, y) {
	new_circle(new_vec(x - 35.0, y), 60.0).append(new_circle(new_vec(x + 35.0, y), 60.0))
}
let star = new_strokepath();
for i in 0..5 {
	let a = -PI() / 2.0 + i * 4.0 * PI() / 5.0;
	let p = new_vec(800.0 + 85.0 * a.cos(), 1735.0 + 85.0 * a.sin());
	if i == 0 { star.move_to(p); } else { star.line_to(p); }
}
star.close();
for shape in [
	[discs(330.0, 1730.0), "holes", 0.9, 300.0],
	[discs(570.0, 1730.0), "nonzero", 0.9, 520.0],
	[star, "evenodd", -0.4, 760.0],
] {
	art.add_strokepath(shape[0], line_style);
	art.add_strokepath(shape[0].hatch(#{ spacing: 8.0, angle: shape[2], fill_rule: shape[1], inset: 2.0 }), new_strokestyle(1.0, gold, pen));
	art.add_canvas(write(shape[1], shape[3], 1850.0, 20.0, label_style, label_font));
}
let coiled = art::snake_path(new_vec(1000.0, 1660.0), 0.0, new_vec(1230.0, 1800.0), 620.0, 50.0, 5);
art.add_strokepath(coiled, line_style);
art.add_strokepath(coiled.hatch(#{ spacing: 7.0, angle: 1.2, bow: 2.5, inset: 2.0 }), new_strokestyle(1.0, green, pen));

// Vectors add, subtract and scale by numbers (whole or decimal), and
// measure: a seven-pointed star from one spoke turned round its hub, and
// an arrow whose head is its direction turned back either way, with a dot
// halfway along. Bounds work for paths as for canvases.
art.add_canvas(write("vectors: + - * /, length, distance, dot, angle, normalized, rotated, and path bounds", 0.0, 1930.0, 24.0, thin_red, label_font));
let hub = new_vec(110, 2100);
let spoke = new_vec(0, -85);
let star = new_strokepath().move_to(hub + spoke);
for i in 1..=7 {
	star.line_to(hub + spoke.rotated(i * 6 * PI() / 7));
}
art.add_strokepath(star, new_strokestyle(1, gold, pen));
let tail = new_vec(300, 2160);
let tip = new_vec(620, 2030);
let along = (tip - tail).normalized();
let arrow = new_line(tail, tip);
arrow.append(new_line(tip, tip - along.rotated(0.45) * 28));
arrow.append(new_line(tip, tip - along.rotated(-0.45) * 28));
art.add_strokepath(arrow, line_style);
art.add_strokepath(new_circle((tail + tip) / 2, 6), thin_red);
if (tip - tail).length() != tail.distance(tip) || along.dot(along) < 0.999 || along.angle() >= 0.0 || -along == along {
	throw "vectors should measure";
}
let box = arrow.bounds();
art.add_strokepath(new_rectangle(box.center, box.width + 20, box.height + 20), new_strokestyle(1, blue, pen));

let art_page = new_page();
art_page.name = "art";
art_page.svg_style = #{ roughness: 1.5, noise_amplitude: 0.8 };
art_page.margin = 30;
art_page.add_canvas(art);
journal_entry.add_page(art_page);

// ---- text ----

let body = "Fate is not subtle. I knew from the moment I picked up the slightly ripped paper that it was a thing to be returned and not kept. ";
body = body + body + body;
let text = new_canvas();
text.add_canvas(heading("text", heading_style, heading_font));
let words_ink = new_strokestyle(1.0, pale, pen);

// A text box, flat (glyph by glyph) with jagged edges.
text.add_canvas(write("new textbox, flat, jagged", 0.0, 0.0, 24.0, thin_red, label_font));
let flat = new_textbox(#{ center: new_vec(250.0, 190.0), width: 480.0, height: 280.0, symbol_scale: 22.0, jagged_edge: 0.6 });
text.add_glyphs(flat.fill(label_font.glyphs(body)).placed, words_ink);

// Words kept whole, spaced out, spread over as many boxes as it takes.
text.add_canvas(write("words, spacing, place all", 560.0, 0.0, 24.0, thin_red, label_font));
let boxes = new_textbox(#{ center: new_vec(0.0, 0.0), width: 220.0, height: 280.0, symbol_scale: 20.0, layout: "words", spacing: 0.05 });
for (block, i) in boxes.place_all(label_font.glyphs(body)) {
	for g in block { g.translate_by(new_vec(680.0 + i * 240.0, 190.0)); }
	text.add_glyphs(block, words_ink);
}

// Text kept clear of a drawing: of its lines, or (add_hull) of everything
// inside its outline.
text.add_canvas(write("avoid a drawing: its lines, or its hull", 0.0, 380.0, 24.0, thin_red, label_font));
let plant = art::bramble(new_vec(250.0, 860.0), -PI() / 2.0, 340.0, 60.0, 9);
let drawing = new_canvas();
drawing.add_strokepath(plant.vines, new_strokestyle(1.0, bark, pen));
drawing.add_strokepath(plant.leaves, new_strokestyle(1.0, green, pen));
drawing.add_strokepath(plant.flowers, line_style);
let b = drawing.bounds();
let lines_box = new_textbox(#{ center: b.center, width: 480.0, height: b.height + 40.0, symbol_scale: 16.0, layout: "words", avoid: drawing, clearance: 3.0 });
text.add_canvas(drawing.copy());
text.add_glyphs(lines_box.fill(label_font.glyphs(body + body)).placed, words_ink);
let hull_tree = new_avoidance_tree().add_hull(drawing, 8.0);
if hull_tree.is_empty || !hull_tree.blocks(b.center) { throw "the hull should cover the drawing's middle"; }
let shifted = drawing.copy().translate(new_vec(560.0, 0.0));
text.add_canvas(shifted);
let hull_box = new_textbox(#{ center: new_vec(b.center.x + 560.0, b.center.y), width: 480.0, height: b.height + 40.0, symbol_scale: 16.0, layout: "words", avoid: new_avoidance_tree().add_hull(shifted, 8.0) });
text.add_glyphs(hull_box.fill(label_font.glyphs(body + body)).placed, words_ink);

// Text along paths: as many copies of a circle as it takes, fitted to an
// arc, and along a spiral that keeps out of a ring and off a line, words
// whole.
text.add_canvas(write("fill with msg, fit msg to, and avoiding along a path", 0.0, 960.0, 24.0, thin_red, label_font));
// The ring takes the message round three times, a copy of it each time.
let ring_text = new_circle(new_vec(70.0, 1130.0), 55.0).fill_with_msg(new_strokestyle(1.0, gold, pen), label_font, 16.0, "around and around and around we go, as many times as it takes to say it all, ");
for (part, i) in ring_text {
	text.add_canvas(part.translate(new_vec(i * 135.0, 0.0)));
}
text.add_canvas(new_arc(new_vec(600.0, 1230.0), 140.0, 3.4, 6.0).fit_msg_to(new_strokestyle(1.0, blue, pen), label_font, "fitted to the arc"));
let spiral = new_strokepath().move_to(new_vec(980.0, 1150.0));
for i in 1..360 {
	let a = i * 0.05;
	let r = 15.0 + a * 9.0;
	spiral.line_to(new_vec(980.0 + r * a.cos(), 1150.0 + r * a.sin()));
}
let keep_out = new_avoidance_tree();
keep_out.add_region(new_circle(new_vec(1030.0, 1110.0), 45.0));
keep_out.add(new_line(new_vec(860.0, 1000.0), new_vec(1120.0, 1300.0)), 4.0);
text.add_strokepath(new_circle(new_vec(1030.0, 1110.0), 45.0), thin_red);
text.add_strokepath(new_line(new_vec(860.0, 1000.0), new_vec(1120.0, 1300.0)), thin_red);
text.add_canvas(spiral.fill_with_msg(new_strokestyle(1.0, green, pen), label_font, 14.0, body, #{ avoid: keep_out, layout: "words" })[0]);

// Notes scattered over a drawing, clear of it, of a line across it and of
// each other, words kept whole.
text.add_canvas(write("splat msgs", 0.0, 1420.0, 24.0, thin_red, label_font));
let board = new_canvas();
board.add_strokepath(new_rectangle(new_vec(560.0, 1680.0), 1120.0, 440.0), thin_red);
board.add_strokepath(art::snake_path(new_vec(400.0, 1570.0), 0.0, new_vec(700.0, 1770.0), 900.0, 40.0, 3), line_style);
text.add_canvas(board);
text.add_canvas(board.splat_msgs(["a note", "another note", "kept clear of the snake", "and of each other", "fate is not subtle"],
	label_font, new_strokestyle(1.0, gold, pen),
	#{ symbol_scale: 20.0, width_range: [260, 380], height_range: [25, 35], clearance: 24.0, layout: "words", avoid: new_line(new_vec(0.0, 1680.0), new_vec(1120.0, 1680.0)) }));

let text_page = new_page();
text_page.name = "text";
text_page.add_canvas(text);
journal_entry.add_page(text_page);

// ---- locks ----

let locks = new_canvas();
locks.name = "ciphers";
locks.add_canvas(heading("locks", heading_style, heading_font));
let plain = "EVERY LOCK OPENS";
let letters = "EVERYLOCKOPENS";
let ordering = symbols(26, "QWERTYUIOPASDFGHJKLZXCVBNM");
let rows = [
	["shift cipher", new_lock("shift").add("s", "shift_cipher", #{ base: 27, shift: 3 }), #{ "s.plain_text": symbols(27, plain) }, "s.cipher_text"],
	["substitution cipher", new_lock("sub").add("s", "substitution_cipher", #{ base_n: 27 }).set_input("s.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS")), #{ "s.plain_text": symbols(27, plain) }, "s.cipher_text"],
	["vigenere cipher", new_lock("vig").add("v", "vigenere_cipher", #{ key_base: 27, plain_base: 27, cipher_base: 27 }), #{ "v.plain_text": symbols(27, plain), "v.key": symbols(27, "KEY") }, "v.cipher_text"],
	["doublering", new_lock("ring").add("d", "doublering", #{ canonical_order: "QWERTYUIOPASDFGHJKLZXCVBNM" }), #{ "d.plain_text": symbols(26, letters) }, "d.cipher_text"],
	["keyed doublering", new_lock("keyed").add("k", "keyed_doublering", #{ base: 26 }), #{ "k.plain_text": symbols(26, letters), "k.base_ordering": ordering }, "k.cipher_text"],
	["merge alpha", new_lock("merge").add("m", "merge_alpha", #{ base_n: 27, input_count: 2 }), #{ "m.input_0": symbols(27, "EVERY LOCK"), "m.input_1": symbols(27, " OPENS") }, "m.merged_output"],
	["split symbols", new_lock("split").add("s", "split_symbols", #{ base_n: 27, output_count: 2 }), #{ "s.plain_text": symbols(27, plain) }, "s.output_1"],
];
let y = 0.0;
for row in rows {
	let lock = row[1];
	let result = lock.run(row[2]);
	if result.get(row[3]).base() < 26 || result.get(row[3]).type_name() == "" { throw "unexpected output type"; }
	locks.add_canvas(write(row[0], 0.0, y, 24.0, thin_red, label_font));
	locks.add_canvas(write(plain + "  becomes  " + result.text(row[3]), 340.0, y, 24.0, label_style, label_font));
	y += 60.0;
}

// A chain of locks, its result drawn as a maze of balanced-ternary glyphs,
// and unlocked again.
y += 30.0;
locks.add_canvas(write("a chain of locks ending in a maze lock", 0.0, y, 24.0, thin_red, label_font));
let chain = new_lock("chain")
	.add("sub", "substitution_cipher", #{ base_n: 27 })
	.add("vig", "vigenere_cipher", #{ key_base: 27, plain_base: 27, cipher_base: 27 })
	.add("maze", "maze_lock", #{ input_base: 27, strategy: "RightHanded" })
	.connect("sub.cipher_text", "vig.key")
	.connect("vig.cipher_text", "maze.maze_text")
	.set_input("sub.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS"))
	.set_input("maze.random_seed", symbols(10, "7"));
if chain.inputs().len() == 0 || chain.outputs().len() == 0 { throw "a chain has open inputs and outputs"; }
let result = chain.run(#{ "sub.plain_text": symbols(27, "key"), "vig.plain_text": symbols(27, plain) });
if chain.unlock(result).text("vig.plain_text") != plain { throw "the chain didn't unlock"; }
if result.trace().len() == 0 || result.values().len() == 0 || result.get("maze.random_seed").symbols().len() == 0 || result.get("maze.random_seed").value() == () { throw "nothing traced"; }
let cell_size = 36.0;
let maze_canvas = new_canvas();
let grid = result.get("maze.generated_maze").maze();
maze_canvas.add_strokepath(grid.path().scale_by(cell_size), new_strokestyle(1.0, red, pen));
for cell in grid.cells() {
	let g = glyph("balanced_ternary_" + (cell.symbol - 13));
	g.scale = cell_size * 0.8;
	g.position = new_vec(cell.x * cell_size, cell.y * cell_size);
	maze_canvas.add_glyph(g, ink);
}
// A maze can also be filled with symbols directly.
let blank = new_maze(3, 2, 2);
blank.fill(symbols(27, "HELLO").symbols());
if blank.cells()[0].symbol != 8 { throw "fill writes symbols in walking order"; }
locks.add_canvas(maze_canvas.translate(new_vec(20.0, y + 60.0)));

let locks_page = new_page();
locks_page.name = "locks";
locks_page.add_canvas(locks);
journal_entry.add_page(locks_page);

// The locks page again, split into two tiles side by side, which fit
// together into it.
let tiles = locks_page.split(2, 1);
if tiles[1].tile.column != 1 { throw "tiles know their place"; }
journal_entry.add_pages(tiles);

// ---- layout: pages typeset from flexbox layouts ----
//
// A layout gives page n's tree of boxes: rows and columns, and slots that
// typeset fills. Here: a heading that shrinks to fit, a drop cap the text
// wraps around, a body flowing down one column and on down the next (and
// on to a second page), a band of ornaments between them, a figure fitted
// to its box, a rule, a spacer and a page label. Layouts usually live in
// layouts/ (see the file one below and layouts/README.md).
fn showcase_page(n, info) {
	column(#{ padding: 50, gap: 16 }, [
		if n == 0 { flow("title", #{ role: "heading", height: 90, fit: true }) } else { spacer(#{ height: 90 }) },
		rule(#{ height: 10, margin: [0, 200] }),
		row(#{ grow: 1, gap: 30 }, [
			column(#{ grow: 1 }, [
				if n == 0 { initial("body", #{ position: "absolute", top: 0, left: 0, width: 80, height: 80 }) } else { spacer() },
				flow("body", #{ grow: 1 }),
			]),
			band("diamond", #{ width: 14 }),
			column(#{ grow: 1 }, [
				if n == 0 { figure("plant", #{ height: 260, border: "frame" }) } else { spacer() },
				flow("body", #{ grow: 1 }),
			]),
		]),
		label(if n == 0 { "the first page" } else { "the second page" }, #{ role: "folio", align: "center", height: 30 }),
	])
}
let diamond = new_strokepath().move_to(new_vec(0.0, -10.0)).line_to(new_vec(6.0, 0.0)).line_to(new_vec(0.0, 10.0)).line_to(new_vec(-6.0, 0.0)).close();
let story = body + body + body + "\n\n" + body + body + body + body + body + body + body;
let page_layout = new_layout("layout", Fn("showcase_page"));
if page_layout.name != "layout" { throw "layouts have names"; }
journal_entry.add_pages(page_layout.typeset(#{
	width: 1100, height: 900,
	flows: #{ title: "pages from a layout", body: story },
	roles: #{
		body: #{ font: label_font, style: words_ink, size: 20.0, align: "justify", indent: 1.5, line_height: 1.3 },
		heading: #{ font: heading_font, style: heading_style, size: 70.0, align: "center", valign: "center" },
		initial: #{ font: heading_font, style: heading_style },
		folio: #{ font: label_font, style: thin_red, size: 18.0 },
		rule: #{ style: thin_red },
		frame: #{ style: thin_red },
		diamond: #{ style: thin_red },
	},
	art: #{ diamond: diamond, plant: |b| {
		let plant = art::bramble(new_vec(b.center.x, b.y + b.height), -PI() / 2.0, b.height * 0.95, b.height * 0.2, 5);
		let c = new_canvas();
		c.add_strokepath(plant.vines, new_strokestyle(1.0, bark, new_brush("Ink Pen", 0.4, 0.8, 1.6)));
		c.add_strokepath(plant.leaves, new_strokestyle(1.0, green, new_brush("Ink Pen", 0.4, 0.8, 1.6)));
		c.add_strokepath(plant.flowers, new_strokestyle(1.0, pale, new_brush("Ink Pen", 0.4, 0.8, 1.6)));
		c
	} },
}));

// A layout file: layouts/missal_title.rhai, a framed title page.
let leaf = new_strokepath().move_to(new_vec(0.0, -22.0)).quad_to(new_vec(14.0, -4.0), new_vec(0.0, 22.0)).quad_to(new_vec(-14.0, -4.0), new_vec(0.0, -22.0));
journal_entry.add_pages(layout("missal_title").typeset(#{
	name: "title",
	flows: #{ title: "The Showcase", subtitle: "everything there is to draw with", credits: "a layout file\nlayouts/missal_title.rhai" },
	roles: #{
		title: #{ font: new_fontset(["copper_upper"], ["copper_lower"]), style: heading_style, size: 150.0, line_height: 1.0 },
		subtitle: #{ font: label_font, style: words_ink, size: 40.0 },
		credits: #{ font: label_font, style: words_ink, size: 30.0 },
		frame: #{ style: thin_red },
		border: #{ style: line_style },
	},
	art: #{ border: leaf },
}));

// Every character above has a glyph.
if missing_glyphs().len() > 0 {
	warn("characters with no glyph: " + missing_glyphs());
}
