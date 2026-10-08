// The showcase: a sample book of everything journals can draw with, every
// page the same size. After the title page and the contents:
//
//   pens      every Darkly brush; pressure profiles, widths, colours and
//             gradients
//   fonts     every font, its alphabet and a line, font settings, and
//             glyphs placed by hand
//   glyphs    every variant of every glyph of every font
//   art       shapes and paths, hatching, vectors, mazes, snakes, brambles
//   text      text boxes, text kept clear of drawings, text along paths,
//             scattered notes, words split round drawings
//   locks     every lock type, and a lock chain drawn as a maze
//   spread    one drawing across two pages: a page split into tiles
//   layouts   pages typeset from layouts: an inline one, and the layout
//             files (the title page is one too)
//   visions   two plates drawn with the brushes together: an insane
//             grimoire's eye in a summoning circle, and a horned dreamer
//
// When journals gain a capability (a function, an art tool, a brush, a
// font, a lock type), it gets a labelled example here: see CLAUDE.md.
// tests/showcase.rs fails when a function scripts can call, or a font,
// isn't in this file. Render it on parchment to see it as a book:
// journalbuild showcase --parchment (or --pdf).

// ---- the book's look ----

// Every page is this size, its content between these margins.
const W = 1200.0;
const H = 1600.0;
const LEFT = 100.0;
const RIGHT = 1100.0;
const TOP = 210.0;
const BOTTOM = 1470.0;

// Red and near-black inks, with a few darker colours for drawings.
const INK = "#2b1d12";
const RED = "#9b2318";
const GREEN = "#3d6b2a";
const BLUE = "#1f4e7a";
const GOLD = "#8a6212";
const BARK = "#6b4423";

const PEN = new_brush("Ink Pen", 0.4, 0.8, 1.6);
const TEXT = new_style(1.0, INK, PEN);
const CAPTION = new_style(1.0, RED, new_brush("Ink Pen", 0.3, 0.7, 1.2));
const LINE = new_style(1.0, INK, new_brush("Ink Pen", 0.3, 0.7, 2.0));
// Headings' calligraphy nib, with the constant pressure a broad nib wants.
let nib = new_brush("Calligraphy", 0.4, 0.8, 1.5);
nib.pressure_profile = "constant";
const HEADING = new_style(1.2, RED, nib);
// Font samples and glyphs: a fine pen, so the glyphs' shapes show.
const SAMPLE = new_style(1.0, INK, new_brush("Ink Pen", 0.4, 0.8, 2.2));

// Text is in plain, a font made to read easily; headings are in loche.
const PLAIN = font("plain");
const LOCHE = font("loche_upper");

const LOREM = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. ";

// ---- writing ----

// `text` written from (x, y) to the right, along a line through the middle
// of its glyphs.
fn write(text, x, y, size) {
	write_in(text, x, y, size, global::TEXT, global::PLAIN, #{})
}

fn write_in(text, x, y, size, style, font, options) {
	let c = new_canvas();
	let line = new_line(new_vec(x, y), new_vec(x + 5000.0, y));
	for part in line.fill_with_text(text, font, size, style, options) {
		c.add_canvas(part);
	}
	c
}

// A label naming the calls an example shows.
fn caption(text, y) {
	write_in(text, global::LEFT, y, 22, global::CAPTION, global::PLAIN, #{})
}

// A heading within a page.
fn subhead(text, y) {
	write_in(text, global::LEFT, y, 36, global::HEADING, global::LOCHE, #{})
}

// A wave from x0 to x1 about y.
fn wave(x0, x1, y, rise) {
	let step = (x1 - x0) / 3.0;
	let path = new_path().move_to(new_vec(x0, y));
	for i in 0..3 {
		let a = x0 + i * step;
		path.cubic_to(new_vec(a + step / 3.0, y - rise), new_vec(a + 2.0 * step / 3.0, y + rise), new_vec(a + step, y));
	}
	path
}

// ---- binding pages into the book ----
//
// Pages are numbered from 3, after the title page and the contents, and
// named for their number and section, so their files sort in order.

let book = #{ pages: [], contents: [] };

fn padded(n) {
	if n < 10 { `0${n}` } else { `${n}` }
}

// A fresh page for `section`: a canvas with its running head drawn.
fn sheet(section) {
	let c = new_canvas();
	c.add_canvas(write_in(section, global::LEFT, 105, 48, global::HEADING, global::LOCHE, #{}));
	c.add_path(new_line(new_vec(global::LEFT, 150), new_vec(global::RIGHT, 150)), global::CAPTION);
	c
}

// Bind a page, numbered and named, into the book's `section`.
fn bind_page(section, page) {
	let number = this.pages.len() + 3;
	page.name = `${padded(number)}_${section}`;
	this.pages.push(page);
	if this.contents.len() == 0 || this.contents[this.contents.len() - 1][0] != section {
		this.contents.push([section, number]);
	}
}

// Bind a sheet: a page the book's size, with a folio under it.
fn bind(section, canvas) {
	let number = this.pages.len() + 3;
	canvas.add_canvas(write_in(`${number}`, global::W / 2 - 8, global::H - 80, 24, global::CAPTION, global::PLAIN, #{}));
	let page = new_page();
	page.size = new_vec(global::W, global::H);
	page.add_canvas(canvas);
	this.bind_page(section, page);
}

// ---- pens: every brush ----
//
// Each brush draws a wave, over bare paper and then over hatching, since
// some (Blur, Smudge, Liquify, Clone) only move ink that's already there.

let pens = sheet("pens");
pens.name = "brushes";
pens.add_canvas(caption("brush_names(), new_brush(name, min_pressure, max_pressure, size)", TOP));
let y = TOP + 70;
for name in brush_names() {
	pens.add_canvas(write(name, LEFT, y, 24));
	pens.add_path(new_rectangle(new_vec(910, y), 380, 54).hatch(7.0, 0.8, 0.6), CAPTION);
	let brush = new_brush(name, 0.3, 0.9, 6);
	brush.size = 9;
	pens.add_path(wave(380, RIGHT, y, 24), new_style(1, INK, brush));
	y += 76;
}
book.bind("pens", pens);

// Pressure runs along each stroke as its brush's profile says, between the
// brush's min_pressure and max_pressure.
let pens = sheet("pens");
pens.add_canvas(subhead("pressure profiles", TOP));
pens.add_canvas(caption("brush.pressure_profile = \"ramp\", \"taper\" or \"constant\"", TOP + 50));
let y = TOP + 110;
for profile in ["ramp", "taper", "constant"] {
	let brush = new_brush("Ink Pen", 0.05, 1.0, 10);
	brush.pressure_profile = profile;
	if brush.min_pressure != 0.05 || brush.max_pressure != 1.0 || brush.brush_style != "Ink Pen" { throw "unexpected brush " + brush; }
	pens.add_canvas(write(profile, LEFT, y, 24));
	pens.add_path(new_line(new_vec(380, y), new_vec(RIGHT, y)), new_style(1, GOLD, brush));
	y += 60;
}

// A style's width scales its brush's pressure (in SVG output it's the
// stroke's width).
y += 30;
pens.add_canvas(subhead("widths", y));
pens.add_canvas(caption("new_style(width, colour, brush), style.width", y + 50));
y += 110;
for (width, i) in [0.3, 0.7, 1.2] {
	let style = new_style(1, BLUE, new_brush("Charcoal", 0.3, 0.8, 8));
	style.width = width;
	pens.add_canvas(write(["light", "middling", "heavy"][i], LEFT, y, 24));
	pens.add_path(wave(380, RIGHT, y, 14), style);
	y += 60;
}

// Colours: CSS names, hex, rgb(), hsl() and alpha.
y += 30;
pens.add_canvas(subhead("colours", y));
pens.add_canvas(caption("CSS colours: named, hex, rgb(), hsl(), and alpha", y + 50));
y += 110;
let colours = [["named", "darkgoldenrod"], ["hex", "#1f4e7a"], ["rgb", "rgb(155, 35, 24)"], ["hsl", "hsl(100, 50%, 28%)"], ["alpha", "rgba(43, 29, 18, 0.4)"]];
for (colour, i) in colours {
	let x = LEFT + i * 205;
	let style = new_style(1, colour[1], new_brush("Rough Ink", 0.4, 0.8, 9));
	if style.color != colour[1] { throw "colour didn't stick"; }
	pens.add_canvas(write(colour[0], x, y, 22));
	pens.add_path(wave(x, x + 170, y + 45, 14), style);
}
// A stroke built from a path and a style, then added.
pens.add_stroke(new_stroke(new_line(new_vec(LEFT, y + 100), new_vec(RIGHT, y + 100)), CAPTION));

// Gradients, in place of a colour: along each stroke from start to end,
// across the page from one point to another, or out from a centre.
// Colours are spaced evenly, or placed as [position, colour].
y += 150;
pens.add_canvas(subhead("gradients", y));
y += 70;
pens.add_canvas(caption("new_gradient(colours): along each stroke", y));
pens.add_path(wave(640, RIGHT, y, 22), new_style(1, new_gradient([RED, GOLD, BLUE]), new_brush("Rough Ink", 0.4, 0.8, 9)));
y += 75;
pens.add_canvas(caption("#{ from, to }: across, colours placed", y));
let across = new_gradient([GREEN, [0.7, GOLD], RED], #{ from: new_vec(640, 0), to: new_vec(RIGHT, 0) });
pens.add_path(new_rectangle(new_vec(870, y), 460, 50).hatch(#{ spacing: 8.0, angle: 1.1 }), new_style(1, across, PEN));
y += 75;
let words = new_style(1, RED, PEN);
words.color = new_gradient([BLUE, INK, RED], #{ from: new_vec(640, 0), to: new_vec(RIGHT, 0) });
pens.add_canvas(caption("style.color = a gradient", y));
pens.add_canvas(write_in("Lorem ipsum dolor sit amet", 640, y, 28, words, PLAIN, #{}));
y += 130;
pens.add_canvas(caption("#{ center, radius }: out from a point", y));
let glow = new_gradient([INK, GOLD, "rgba(155, 35, 24, 0.3)"], #{ center: new_vec(870, y), radius: 100 });
let spiral = new_path().move_to(new_vec(870, y));
for i in 1..220 {
	let a = i * 0.11;
	spiral.line_to(new_vec(870 + a * 4.0 * a.cos(), y + a * 4.0 * a.sin()));
}
pens.add_path(spiral, new_style(1, glow, new_brush("Ink Pen", 0.4, 0.8, 3)));
book.bind("pens", pens);

// ---- fonts ----
//
// Fonts come from config/fonts.json, each with its glyphs, the padding
// between them, the width of a space, and a font to fall back to.

let alphabet = "abcdefghijklmnopqrstuvwxyz";
let lorem = "lorem ipsum dolor sit amet, consectetur";
let samples = [
	#{ label: "font(\"plain\"): made to read easily", font: PLAIN, lines: [alphabet.to_upper(), alphabet, "0123456789 . , ; : ! ? ' \" - — _ ( ) [ ] { } / # + = * < > % &", "Lorem ipsum dolor sit amet, consectetur."] },
	#{ label: "font(\"copper\"): falling back to loche_upper for what it lacks", font: font("copper"), lines: [alphabet.to_upper(), alphabet, "Lorem Ipsum Dolor Sit Amet", "variants at random: aaaa eeee ssss"] },
	#{ label: "font(\"loche_upper\"): capitals, lowercase text set in them", font: LOCHE, lines: [alphabet, lorem + ".", "punctuation . , ; : ' \" ? - —"] },
	#{ label: "font(\"loche_lower\"): falling back to loche_upper for capitals and stops", font: font("loche_lower"), lines: [alphabet, "Lorem ipsum dolor sit amet, consectetur."] },
	#{ label: "font(\"lang\")", font: font("lang"), lines: [alphabet.to_upper(), alphabet, "Lorem ipsum dolor sit amet."] },
	#{ label: "font(\"eldritch\")", font: font("eldritch"), lines: [alphabet, "lorem ipsum dolor sit amet"] },
	#{ label: "font(\"sharp_lower\")", font: font("sharp_lower"), lines: [alphabet, "lorem ipsum dolor sit amet"] },
	#{ label: "ternary_font(\"balanced_ternary\"): letters to the ternary digits", font: ternary_font("balanced_ternary"), lines: ["abcdefghijklm", "nopqrstuvwxyz", "lorem ipsum dolor"] },
];
let fonts = sheet("fonts");
let y = TOP;
for f in samples {
	let height = 50 + f.lines.len() * 54 + 30;
	if y + height > BOTTOM {
		book.bind("fonts", fonts);
		fonts = sheet("fonts");
		y = TOP;
	}
	fonts.add_canvas(caption(f.label, y));
	y += 50;
	for line in f.lines {
		fonts.add_canvas(write_in(line, LEFT, y, 36, SAMPLE, f.font, #{}));
		y += 54;
	}
	y += 30;
}

// A font's settings, from the font file unless changed: padding between
// glyphs and the width of a space (both in font sizes), and the font to
// fall back to. Changing a font changes that copy only.
if y + 520 > BOTTOM {
	book.bind("fonts", fonts);
	fonts = sheet("fonts");
	y = TOP;
}
fonts.add_canvas(subhead("font settings", y));
y += 60;
let loose = font("loche_lower");
loose.character_padding = 0.3;
loose.space_width = 1;
let spaced = [
	["font(\"loche_lower\", #{ character_padding: 0, space_width: 0.3 })", font("loche_lower", #{ character_padding: 0, space_width: 0.3 })],
	[`font("loche_lower"): as the font file sets it, padding ${font("loche_lower").character_padding}`, font("loche_lower")],
	["f.character_padding = 0.3; f.space_width = 1", loose],
];
for row in spaced {
	fonts.add_canvas(caption(row[0], y));
	fonts.add_canvas(write_in(lorem, LEFT, y + 46, 36, SAMPLE, row[1], #{}));
	y += 106;
}
let lettered = font("eldritch");
lettered.fallback = "copper";
if lettered.fallback.name != "copper" || font("plain").fallback != () || !font_names().contains("loche_lower") { throw "fallbacks and names"; }
fonts.add_canvas(caption("f.fallback = \"copper\": eldritch, with capitals and stops from copper", y));
fonts.add_canvas(write_in("Lorem ipsum. Dolor sit amet.", LEFT, y + 46, 36, SAMPLE, lettered, #{}));
y += 130;

// Glyphs one at a time: placed, scaled and turned by hand.
if y + 200 > BOTTOM {
	book.bind("fonts", fonts);
	fonts = sheet("fonts");
	y = TOP;
}
fonts.add_canvas(caption("glyphs placed by hand: glyphs(text), glyph(name), and their scale, position and rotation", y));
y += 80;
let word = font("copper").glyphs("turning");
for (g, i) in word {
	if g.is_blank { continue; }
	g.scale = 50;
	g.position = new_vec(LEFT + 40 + i * 70, y);
	g.rotation = i * 0.2;
	g.translate_by(new_vec(0, 8 * i)).scale_by(1 + 0.05 * i).rotate_by(0.05);
}
fonts.add_glyphs(word, SAMPLE);
let eye = font("symbols").glyph("basic_eye").copy();
eye.scale = 80;
eye.position = new_vec(800, y);
fonts.add_glyph(eye, new_style(1, GOLD, PEN));
fonts.add_path(eye.path().translate_by(new_vec(140, 0)), new_style(1, BLUE, PEN));
book.bind("fonts", fonts);

// ---- glyphs: every glyph of every font ----
//
// A row (or more) per font, every variant of every glyph.

let catalogue = sheet("glyphs");
let y = TOP;
for name in font_names() {
	if name.starts_with("debug") { continue; }
	let f = font(name);
	if y + 110 > BOTTOM {
		book.bind("glyphs", catalogue);
		catalogue = sheet("glyphs");
		y = TOP;
	}
	catalogue.add_canvas(caption(`font("${name}")`, y));
	y += 60;
	let column = 0;
	for key in f.characters() {
		for g in f.variants(key) {
			if column == 19 {
				column = 0;
				y += 56;
				if y > BOTTOM {
					book.bind("glyphs", catalogue);
					catalogue = sheet("glyphs");
					y = TOP;
				}
			}
			g.scale = 38;
			g.position = new_vec(LEFT + 20 + column * 52, y);
			catalogue.add_glyph(g, SAMPLE);
			column += 1;
		}
	}
	y += 80;
}
book.bind("glyphs", catalogue);

// ---- art ----

seed(7);
let art = sheet("art");

// Shapes and curves, moved, scaled and turned.
art.add_canvas(caption("new_circle, new_rectangle, new_line, new_arc, new_path with quad_to and cubic_to", TOP));
let y = TOP + 120;
art.add_path(new_circle(new_vec(160, y), 50), LINE);
art.add_path(new_rectangle(new_vec(300, y), 100, 80).rotate_around(new_vec(300, y), 0.2), LINE);
art.add_path(new_line(new_vec(370, y - 50), new_vec(470, y + 50)), LINE);
art.add_path(new_arc(new_vec(550, y), 50, 0, 4.5), LINE);
art.add_path(
	new_path().move_to(new_vec(620, y + 50))
		.quad_to(new_vec(670, y - 70), new_vec(720, y + 50))
		.cubic_to(new_vec(760, y + 110), new_vec(660, y + 130), new_vec(620, y + 50))
		.close()
		.scale_by(1).translate_by(new_vec(10, 0)),
	new_style(1, BLUE, PEN));

// Hatching fills a closed path's inside with lines: hatch(spacing, angle,
// roughness), here at a random slant.
art.add_path(new_circle(new_vec(880, y), 50), LINE);
art.add_path(new_circle(new_vec(880, y), 50).hatch(5, random_range(0.0, PI()), 1), CAPTION);
art.add_path(new_rectangle(new_vec(1020, y), 90, 90).hatch(6, random() * PI(), 0.5), new_style(1, GOLD, PEN));

// Hatching with options. Each line is a stroke of its own, inside the
// shape: a ring (two circles appended into one path) keeps its hole, and
// lines can cross, bow and stop short of the outline. Fill rules decide
// what's inside where shapes overlap: "holes" (the default) leaves the
// overlap out, "nonzero" merges them, and "evenodd" also leaves out where
// one line crosses itself, like the middle of a star.
y += 160;
art.add_canvas(caption("hatch(#{ ... }): a ring, angles, roughness, bow, inset, and fill rules", y));
y += 150;
let ring = new_circle(new_vec(180, y), 80).append(new_circle(new_vec(180, y), 40));
art.add_path(ring, LINE);
art.add_path(ring.hatch(#{ spacing: 9.0, angle: [0.6, 2.2], roughness: 1.0 }), CAPTION);
fn discs(x, y) {
	new_circle(new_vec(x - 35, y), 60).append(new_circle(new_vec(x + 35, y), 60))
}
let star = new_path();
for i in 0..5 {
	let a = -PI() / 2.0 + i * 4.0 * PI() / 5.0;
	let p = new_vec(880 + 85 * a.cos(), y + 5 + 85 * a.sin());
	if i == 0 { star.move_to(p); } else { star.line_to(p); }
}
star.close();
for shape in [
	[discs(410, y), "holes", 0.9, 380],
	[discs(650, y), "nonzero", 0.9, 600],
	[star, "evenodd", -0.4, 840],
] {
	art.add_path(shape[0], LINE);
	art.add_path(shape[0].hatch(#{ spacing: 8.0, angle: shape[2], fill_rule: shape[1], inset: 2.0 }), new_style(1, GOLD, PEN));
	art.add_canvas(write(shape[1], shape[3], y + 120, 20));
}
y += 200;
let coiled = art::snake_path(new_vec(180, y), 0, new_vec(560, y + 120), 700, 50, 5);
art.add_path(coiled, LINE);
art.add_path(coiled.hatch(#{ spacing: 7.0, angle: 1.2, bow: 2.5, inset: 2.0 }), new_style(1, GREEN, PEN));

// Vectors add, subtract and scale by numbers (whole or decimal), and
// measure: a seven-pointed star from one spoke turned round its hub, and
// an arrow whose head is its direction turned back either way, with a dot
// halfway along. Bounds work for paths as for canvases.
y += 220;
art.add_canvas(caption("vecs: + - * /, length, distance, dot, angle, normalized, rotated; path bounds", y));
y += 150;
let hub = new_vec(190, y);
let spoke = new_vec(0, -85);
let star = new_path().move_to(hub + spoke);
for i in 1..=7 {
	star.line_to(hub + spoke.rotated(i * 6 * PI() / 7));
}
art.add_path(star, new_style(1, GOLD, PEN));
let tail = new_vec(380, y + 60);
let tip = new_vec(700, y - 70);
let along = (tip - tail).normalized();
let arrow = new_line(tail, tip);
arrow.append(new_line(tip, tip - along.rotated(0.45) * 28));
arrow.append(new_line(tip, tip - along.rotated(-0.45) * 28));
art.add_path(arrow, LINE);
art.add_path(new_circle((tail + tip) / 2, 6), CAPTION);
if (tip - tail).length() != tail.distance(tip) || along.dot(along) < 0.999 || along.angle() >= 0.0 || -along == along {
	throw "vectors should measure";
}
let box = arrow.bounds();
art.add_path(new_rectangle(box.center, box.width + 20, box.height + 20), new_style(1, BLUE, PEN));
book.bind("art", art);

let art = sheet("art");
// A maze's corridors.
art.add_canvas(caption("new_maze(seed, width, height): a maze's corridors", TOP));
let maze = new_maze(4, 8, 3);
if maze.width() < 1 || maze.height() < 1 { throw "a maze has a size"; }
art.add_path(maze.path().scale_by(30).translate_by(new_vec(LEFT + 20, TOP + 50)), LINE);

// Snakes: one with too little slack to loop, one coiled over itself, and
// writing along a snake's spine.
art.add_canvas(caption("art::snake_path, and text down an art::snake_spine", TOP + 250));
art.add_path(art::snake_path(new_vec(160, 560), -PI() / 2.0, new_vec(180, 930), 480, 40, 4), LINE);
art.add_path(art::snake_path(new_vec(400, 600), -PI() / 2.0, new_vec(410, 660), 900, 36, 7), LINE);
let spine = art::snake_spine(new_vec(700, 520), -PI() / 2.0, new_vec(740, 930), 640, 48, 2);
art.add_path(art::snake_path(new_vec(700, 520), -PI() / 2.0, new_vec(740, 930), 640, 48, 2), LINE);
art.add_canvas(spine.fill_with_text("lorem ipsum dolor sit amet", PLAIN, 22, new_style(1, RED, PEN))[0]);

// A bramble: canes, leaves and flowers, stroked separately.
art.add_canvas(caption("art::bramble: canes, leaves and flowers", 1010));
let plant = art::bramble(new_vec(600, 1450), -PI() / 2.0, 400, 80, 4);
art.add_path(plant.vines, new_style(1, BARK, PEN));
art.add_path(plant.leaves, new_style(1, GREEN, PEN));
art.add_path(plant.flowers, LINE);
book.bind("art", art);

// Along a path: a maze grown with brambles, and made a many-headed serpent.
let art = sheet("art");
art.add_canvas(caption("art::bramble_along a maze", TOP));
// Scaled to fit half the page, from its bounds.
let maze_lines = new_maze(5, 4, 3).path();
maze_lines.scale_by(780 / maze_lines.bounds().width);
let along = art::bramble_along(maze_lines.copy().translate_by(new_vec(LEFT + 110, TOP + 90)), 50, 2);
art.add_path(along.vines, new_style(1, BARK, PEN));
art.add_path(along.leaves, new_style(1, GREEN, PEN));
art.add_path(along.flowers, LINE);
art.add_canvas(caption("art::snake_along the same maze", 860));
art.add_path(art::snake_along(maze_lines.copy().translate_by(new_vec(LEFT + 110, 940)), 40, 2), LINE);
book.bind("art", art);

// ---- text ----

let text = sheet("text");

// A text box, flat (glyph by glyph) with jagged edges.
text.add_canvas(caption("new_textbox: flat, with jagged edges", TOP));
let flat = new_textbox(#{ center: new_vec(340, TOP + 190), width: 460, height: 300, symbol_scale: 22, jagged_edge: 0.6 });
text.add_glyphs(flat.fill(PLAIN.glyphs(LOREM)).placed, TEXT);

// Words kept whole, spaced out, spread over as many boxes as it takes.
text.add_canvas(caption("words, spacing, place_all", TOP).translate_by(new_vec(540, 0)));
let boxes = new_textbox(#{ center: new_vec(0, 0), width: 210, height: 300, symbol_scale: 20, layout: "words", spacing: 0.05 });
for (block, i) in boxes.place_all(PLAIN.glyphs(LOREM)) {
	for g in block { g.translate_by(new_vec(760 + i * 230, TOP + 190)); }
	text.add_glyphs(block, TEXT);
}

// Text kept clear of a drawing: of its lines, or (add_hull) of everything
// inside its outline.
text.add_canvas(caption("avoid a drawing: its lines, or add_hull", 600));
let plant = art::bramble(new_vec(330, 1060), -PI() / 2.0, 380, 60, 9);
let drawing = new_canvas();
drawing.add_path(plant.vines, new_style(1, BARK, PEN));
drawing.add_path(plant.leaves, new_style(1, GREEN, PEN));
drawing.add_path(plant.flowers, LINE);
let b = drawing.bounds();
let lines_box = new_textbox(#{ center: b.center, width: 460, height: b.height + 40, symbol_scale: 16, layout: "words", avoid: drawing, clearance: 3 });
text.add_canvas(drawing.copy());
text.add_glyphs(lines_box.fill(PLAIN.glyphs(LOREM + LOREM)).placed, TEXT);
let hull_tree = new_avoidance_tree().add_hull(drawing, 8);
if hull_tree.is_empty || !hull_tree.blocks(b.center) { throw "the hull should cover the drawing's middle"; }
let shifted = drawing.copy().translate_by(new_vec(520, 0));
text.add_canvas(shifted);
let hull_box = new_textbox(#{ center: b.center + new_vec(520, 0), width: 460, height: b.height + 40, symbol_scale: 16, layout: "words", avoid: new_avoidance_tree().add_hull(shifted, 8) });
text.add_glyphs(hull_box.fill(PLAIN.glyphs(LOREM + LOREM)).placed, TEXT);
book.bind("text", text);

let text = sheet("text");
// Text along paths: as many copies of a circle as it takes, fitted to an
// arc, and along a spiral that keeps out of a ring and off a line, words
// whole.
text.add_canvas(caption("fill_with_text round a circle, as many times as it takes", TOP));
let ring_text = new_circle(new_vec(170, TOP + 130), 60).fill_with_text(LOREM, PLAIN, 16, new_style(1, GOLD, PEN));
for (part, i) in ring_text {
	text.add_canvas(part.translate_by(new_vec((i % 6) * 150, (i / 6) * 150)));
}
text.add_canvas(caption("fit_text: as large as fits the arc", 560));
text.add_canvas(new_arc(new_vec(320, 760), 170, 3.4, 6.0).fit_text("lorem ipsum", PLAIN, new_style(1, BLUE, PEN)));
text.add_canvas(caption("along a spiral, avoiding a ring and a line", 560).translate_by(new_vec(540, 0)));
let centre = new_vec(840, 780);
let spiral = new_path().move_to(centre);
for i in 1..360 {
	let a = i * 0.05;
	let r = 15 + a * 9;
	spiral.line_to(centre + new_vec(r * a.cos(), r * a.sin()));
}
let keep_out = new_avoidance_tree();
keep_out.add_region(new_circle(centre + new_vec(50, -40), 45));
keep_out.add(new_line(centre + new_vec(-120, -150), centre + new_vec(140, 150)), 4);
text.add_path(new_circle(centre + new_vec(50, -40), 45), CAPTION);
text.add_path(new_line(centre + new_vec(-120, -150), centre + new_vec(140, 150)), CAPTION);
text.add_canvas(spiral.fill_with_text(LOREM, PLAIN, 14, new_style(1, GREEN, PEN), #{ avoid: keep_out, layout: "words" })[0]);

// Notes scattered over a drawing, clear of it, of a line across it and of
// each other, words kept whole.
text.add_canvas(caption("scatter_notes, clear of a drawing, a line and each other", 1030));
let board = new_canvas();
board.add_path(new_rectangle(new_vec(600, 1250), 1000, 360), CAPTION);
board.add_path(art::snake_path(new_vec(400, 1150), 0, new_vec(720, 1330), 900, 40, 3), LINE);
text.add_canvas(board);
text.add_canvas(board.scatter_notes(["lorem ipsum", "dolor sit amet", "consectetur adipiscing", "sed do eiusmod", "tempor incididunt"],
	PLAIN, new_style(1, GOLD, PEN),
	#{ symbol_scale: 20, width_range: [240, 340], height_range: [25, 35], clearance: 24, layout: "words", avoid: new_line(new_vec(100, 1250), new_vec(1100, 1250)) }));
book.bind("text", text);

// Words round a drawing: whole, moved past it (the default), broken
// round it with split_words, and with split_padding to keep the pieces
// further off. split_words: n is the shortest piece allowed.
let text = sheet("text");
text.add_canvas(subhead("splitting words", TOP));
text.add_canvas(caption("layout: \"words\", avoid; then split_words: true; then split_padding: 0.6", TOP + 50));
fn seal(c) {
	let s = new_canvas();
	let star = new_path();
	for i in 0..=5 {
		let a = -PI() / 2.0 + i * 4.0 * PI() / 5.0;
		let p = c + new_vec(70 * a.cos(), 70 * a.sin());
		if i == 0 { star.move_to(p); } else { star.line_to(p); }
	}
	s.add_path(new_circle(c, 78), global::CAPTION);
	s.add_path(star, global::CAPTION);
	s
}
for (options, i) in [#{}, #{ split_words: true }, #{ split_words: true, split_padding: 0.6 }] {
	let c = new_vec(LEFT + 160 + i * 340, TOP + 330);
	let drawing = seal(c + new_vec(-50, -60));
	text.add_canvas(drawing);
	let box = #{ center: c, width: 320, height: 420, symbol_scale: 17, layout: "words", avoid: drawing, clearance: 4 };
	box += options;
	text.add_glyphs(new_textbox(box).fill(PLAIN.glyphs(LOREM + LOREM)).placed, TEXT);
}
// Along a path too, and in layouts (a role's split_words and
// split_padding: see the inline layout).
let y = TOP + 680;
text.add_canvas(caption("fill_with_text along a line past three circles: whole words, then split_words: 2", y));
for (options, i) in [#{ layout: "words" }, #{ layout: "words", split_words: 2 }] {
	let line_y = y + 80 + i * 110;
	let rings = new_avoidance_tree();
	for k in 0..3 {
		let at = new_vec(LEFT + 210 + k * 300, line_y);
		text.add_path(new_circle(at, 30), CAPTION);
		rings.add_region(new_circle(at, 30), 6);
	}
	options.avoid = rings;
	text.add_canvas(new_line(new_vec(LEFT, line_y), new_vec(RIGHT, line_y)).fill_with_text(LOREM, PLAIN, 22, TEXT, options)[0]);
}
book.bind("text", text);

// ---- locks ----

let locks = sheet("locks");
locks.name = "ciphers";
let plain = "EVERY LOCK OPENS";
let letters = "EVERYLOCKOPENS";
let ordering = symbols(26, "QWERTYUIOPASDFGHJKLZXCVBNM");
let rows = [
	["shift_cipher", new_lock("shift").add("s", "shift_cipher", #{ base: 27, shift: 3 }), #{ "s.plain_text": symbols(27, plain) }, "s.cipher_text"],
	["substitution_cipher", new_lock("sub").add("s", "substitution_cipher", #{ base_n: 27 }).set_input("s.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS")), #{ "s.plain_text": symbols(27, plain) }, "s.cipher_text"],
	["vigenere_cipher", new_lock("vig").add("v", "vigenere_cipher", #{ key_base: 27, plain_base: 27, cipher_base: 27 }), #{ "v.plain_text": symbols(27, plain), "v.key": symbols(27, "KEY") }, "v.cipher_text"],
	["doublering", new_lock("ring").add("d", "doublering", #{ canonical_order: "QWERTYUIOPASDFGHJKLZXCVBNM" }), #{ "d.plain_text": symbols(26, letters) }, "d.cipher_text"],
	["keyed_doublering", new_lock("keyed").add("k", "keyed_doublering", #{ base: 26 }), #{ "k.plain_text": symbols(26, letters), "k.base_ordering": ordering }, "k.cipher_text"],
	["merge_alpha", new_lock("merge").add("m", "merge_alpha", #{ base_n: 27, input_count: 2 }), #{ "m.input_0": symbols(27, "EVERY LOCK"), "m.input_1": symbols(27, " OPENS") }, "m.merged_output"],
	["split_symbols", new_lock("split").add("s", "split_symbols", #{ base_n: 27, output_count: 2 }), #{ "s.plain_text": symbols(27, plain) }, "s.output_1"],
];
locks.add_canvas(caption("new_lock(name).add(part, lock type, options), run: " + plain + " becomes", TOP));
let y = TOP + 60;
for row in rows {
	let lock = row[1];
	let result = lock.run(row[2]);
	if result.get(row[3]).base() < 26 || result.get(row[3]).type_name() == "" { throw "unexpected output type"; }
	locks.add_canvas(write(row[0], LEFT, y, 22));
	locks.add_canvas(write(result.text(row[3]), 450, y, 22));
	y += 52;
}

// A chain of locks, its result drawn as a maze of balanced-ternary glyphs,
// and unlocked again.
y += 30;
locks.add_canvas(caption("a chain of locks ending in a maze lock, drawn in balanced-ternary glyphs", y));
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
let cell_size = 70;
let maze_canvas = new_canvas();
let grid = result.get("maze.generated_maze").maze();
maze_canvas.add_path(grid.path().scale_by(cell_size), new_style(1, RED, PEN));
let ternary = font("balanced_ternary");
for cell in grid.cells() {
	let g = ternary.glyph(cell.symbol - 13);
	g.scale = cell_size * 0.8;
	g.position = new_vec(cell.x * cell_size, cell.y * cell_size);
	maze_canvas.add_glyph(g, SAMPLE);
}
// A maze can also be filled with symbols directly.
let blank = new_maze(3, 2, 2);
blank.fill(symbols(27, "HELLO").symbols());
if blank.cells()[0].symbol != 8 { throw "fill writes symbols in walking order"; }
locks.add_canvas(maze_canvas.translate_by(new_vec(LEFT + 30, y + 80)));
book.bind("locks", locks);

// ---- a spread: one drawing over two pages ----
//
// A page twice as wide, split into two tiles side by side: each a page of
// its own (on parchment, a sheet of its own), which together make up the
// whole drawing.

let spread = new_canvas();
spread.add_canvas(write_in("a spread", LEFT, 105, 48, HEADING, LOCHE, #{}));
spread.add_canvas(caption("page.split(2, 1): one page twice as wide, cut into two", TOP));
let path = new_path().move_to(new_vec(250, 1300));
path.cubic_to(new_vec(700, 200), new_vec(1700, 1500), new_vec(2150, 400));
let serpent = art::snake_path(new_vec(2150, 400), -0.8, new_vec(260, 1250), 3200, 90, 11);
spread.add_path(serpent, LINE);
spread.add_path(serpent.hatch(#{ spacing: 9.0, angle: 0.7, bow: 2.0, inset: 2.0 }), new_style(1, GREEN, PEN));
spread.add_canvas(path.fill_with_text(LOREM, PLAIN, 24, CAPTION, #{ avoid: serpent, clearance: 10, layout: "words" })[0]);
// Its two halves' folios.
for half in [0, 1] {
	let number = book.pages.len() + 3 + half;
	spread.add_canvas(write_in(`${number}`, half * W + W / 2 - 8, H - 80, 24, CAPTION, PLAIN, #{}));
}
let wide = new_page();
wide.name = "spread";
wide.size = new_vec(2 * W, H);
wide.add_canvas(spread);
// Page margins only matter for pages without a size, which grow to their
// strokes plus the margin.
wide.margin = 30;
if wide.margin != 30 { throw "margins read back"; }
let tiles = wide.split(2, 1);
if tiles[1].tile.column != 1 { throw "tiles know their place"; }
for tile in tiles {
	book.bind_page("spread", tile);
}

// ---- layouts: pages typeset from flexbox layouts ----
//
// A layout gives page n's tree of boxes: rows and columns, and slots that
// typeset fills. Here: a heading that shrinks to fit, a drop cap the text
// wraps around, a body flowing down one column and on down the next (and
// on to a second page), a band of ornaments between them, a figure fitted
// to its box, a rule, a spacer and a page label. Words beside the drop cap
// may break round it (split_words, split_padding). Layouts usually live in
// layouts/ (see the files used below, and layouts/README.md).
fn sample_page(n, info) {
	column(#{ padding: [90, 100, 60, 100], gap: 16 }, [
		if n == 0 { flow("title", #{ role: "heading", height: 110, fit: true }) } else { spacer(#{ height: 110 }) },
		rule(#{ height: 10, margin: [0, 250] }),
		row(#{ grow: 1, gap: 30 }, [
			column(#{ grow: 1 }, [
				if n == 0 { initial("body", #{ position: "absolute", top: 0, left: 0, width: 90, height: 90 }) } else { spacer() },
				flow("body", #{ grow: 1, split_words: true, split_padding: 0.2 }),
			]),
			band("diamond", #{ width: 14 }),
			column(#{ grow: 1 }, [
				if n == 0 { figure("plant", #{ height: 360, border: "frame" }) } else { spacer() },
				flow("body", #{ grow: 1 }),
			]),
		]),
		label(if n == 0 { "an inline layout" } else { "and on" }, #{ role: "folio", align: "center", height: 30 }),
	])
}
let diamond = new_path().move_to(new_vec(0, -10)).line_to(new_vec(6, 0)).line_to(new_vec(0, 10)).line_to(new_vec(-6, 0)).close();
fn plant_in(b) {
	let plant = art::bramble(new_vec(b.center.x, b.y + b.height), -PI() / 2.0, b.height * 0.95, b.height * 0.2, 5);
	let c = new_canvas();
	c.add_path(plant.vines, new_style(1, global::BARK, global::PEN));
	c.add_path(plant.leaves, new_style(1, global::GREEN, global::PEN));
	c.add_path(plant.flowers, new_style(1, global::INK, global::PEN));
	c
}
let body_role = #{ font: PLAIN, style: TEXT, size: 22, align: "justify", indent: 1.5, line_height: 1.3 };
let story = LOREM + LOREM + LOREM + "\n\n" + LOREM + LOREM + LOREM + LOREM + LOREM + LOREM + LOREM + LOREM;
let sample_layout = new_layout("sample", Fn("sample_page"));
if sample_layout.name != "sample" { throw "layouts have names"; }
for page in sample_layout.typeset(#{
	width: W, height: H,
	flows: #{ title: "Lorem Ipsum", body: story },
	roles: #{
		body: body_role,
		heading: #{ font: LOCHE, style: HEADING, size: 80, align: "center", valign: "center" },
		initial: #{ font: LOCHE, style: HEADING },
		folio: #{ font: PLAIN, style: CAPTION, size: 20 },
		rule: #{ style: CAPTION },
		frame: #{ style: CAPTION },
		diamond: #{ style: CAPTION },
	},
	art: #{ diamond: diamond, plant: Fn("plant_in") },
}) {
	book.bind_page("layouts", page);
}

// A layout file: layouts/two_column_article.rhai, an article in two
// columns with a heading, a drop cap and an illustration.
for page in layout("two_column_article").typeset(#{
	width: W, height: H,
	flows: #{ title: "Dolor Sit Amet", body: LOREM + LOREM + LOREM + LOREM + LOREM + LOREM, caption: "consectetur adipiscing" },
	roles: #{
		body: body_role,
		heading: #{ font: font("copper"), style: HEADING, size: 90 },
		caption: #{ font: PLAIN, style: CAPTION, size: 22, align: "center" },
		initial: #{ font: font("copper"), style: HEADING },
		folio: #{ font: PLAIN, style: CAPTION, size: 22 },
		rule: #{ style: CAPTION },
		divider: #{ style: CAPTION },
	},
	art: #{ divider: diamond, illustration: Fn("plant_in") },
}) {
	book.bind_page("layouts", page);
}

// layouts/captioned_figure.rhai: a figure filling the page, captioned.
for page in layout("captioned_figure").typeset(#{
	width: W, height: H,
	flows: #{ caption: "sed do eiusmod tempor incididunt" },
	roles: #{ caption: #{ font: PLAIN, style: CAPTION, size: 28 } },
	art: #{ figure: Fn("plant_in") },
}) {
	book.bind_page("layouts", page);
}

// ---- visions: two plates drawn with the brushes ----
//
// Pictures rather than samples: the brushes together, as a page of an
// insane grimoire would use them. Order matters: Smudge, Liquify and Blur
// work on the ink already down, and the watercolours take up what's under
// them.

seed(13);
const NIGHT = "rgba(24, 12, 10, 0.2)";
const SMOKE = "rgba(43, 29, 18, 0.18)";
const BLOOD = "#7a1410";

// Brushes for the plates, each with the pressure profile its marks want.
fn plate_brush(name, low, high, size, profile) {
	let b = new_brush(name, low, high, size);
	b.pressure_profile = profile;
	b
}
const SHADOW = new_style(1, NIGHT, plate_brush("Airbrush", 0.3, 0.9, 90, "taper"));
const HAZE = new_style(1, SMOKE, plate_brush("Airbrush", 0.2, 0.8, 50, "taper"));
const GLOW = new_style(1, "rgba(155, 35, 24, 0.55)", plate_brush("Airbrush", 0.2, 0.9, 40, "taper"));
const OUTLINE = new_style(1, INK, plate_brush("Rough Ink", 0.3, 0.9, 5, "taper"));
const FINE = new_style(1, INK, plate_brush("Ink Pen", 0.2, 0.7, 2.2, "taper"));
const THORN = new_style(1, INK, plate_brush("Rough Ink", 0.05, 1.0, 6, "ramp"));
const LASH = new_style(1, INK, plate_brush("Hair", 0.3, 0.9, 7, "taper"));
const SHADE = new_style(1, "rgba(43, 29, 18, 0.7)", plate_brush("Charcoal", 0.3, 0.8, 5, "constant"));
const GRAPHITE = new_style(1, "rgba(43, 29, 18, 0.6)", plate_brush("Pencil", 0.3, 0.8, 2.5, "taper"));
const SIGIL = new_style(1, RED, plate_brush("Calligraphy", 0.5, 0.9, 3.5, "constant"));
const WASH = new_style(1, "rgba(122, 20, 16, 0.6)", plate_brush("Rough Watercolor", 0.4, 0.9, 7, "taper"));
const BLEED = new_style(1, "rgba(155, 35, 24, 0.3)", plate_brush("Smooth Watercolor", 0.2, 0.9, 4, "ramp"));
const SPONGED = new_style(1, "rgba(24, 12, 10, 0.45)", plate_brush("Sponge", 0.2, 0.9, 14, "taper"));
const SMEAR = new_style(1, INK, plate_brush("Smudge", 0.4, 0.9, 30, "taper"));
const MELT = new_style(1, INK, plate_brush("Liquify", 0.4, 0.9, 40, "taper"));
const DREAM = new_style(1, INK, plate_brush("Blur", 0.4, 0.9, 50, "constant"));

// A point on a cubic curve.
fn bez(p0, p1, p2, p3, t) {
	let u = 1.0 - t;
	p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
}

// A curl's points: from `from`, heading `angle`, bending more and more
// (by `bend` per step to start with).
fn curl_points(from, angle, length, bend, steps) {
	let points = [from];
	let at = from;
	for i in 0..steps {
		at += new_vec(angle.cos(), angle.sin()) * (length / steps);
		points.push(at);
		angle += bend * (1.0 + i / steps);
	}
	points
}
fn curl(from, angle, length, bend, steps) {
	through(curl_points(from, angle, length, bend, steps))
}

// A path through `points`, or through them backwards.
fn through(points) {
	let p = new_path().move_to(points[0]);
	for i in 1..points.len() { p.line_to(points[i]); }
	p
}
fn reversed(points) {
	let back = points;
	back.reverse();
	through(back)
}

// A wisp of smoke: a wavering line rising from `from`.
fn wisp(from, height, sway) {
	let p = new_path().move_to(from);
	let phase = random() * 6.28;
	for i in 1..=24 {
		let t = i / 24.0;
		p.line_to(from + new_vec(sway * (phase + t * 9.0).sin() * t, -height * t));
	}
	p
}

// An eye at `c`, `r` from the middle to a corner, turned by `turn`,
// `wide` open: lids, shaded white, a hatched iris with a slit pupil, and
// lashes like thorns. Small eyes are drawn with finer pens.
fn eye(c, r, turn, wide) {
	let e = new_canvas();
	let small = r < 80;
	let outline = if small { global::FINE } else { global::OUTLINE };
	let left = c + new_vec(-r, 0);
	let right = c + new_vec(r, 0);
	let up = [left, c + new_vec(-r * 0.45, -r * wide), c + new_vec(r * 0.45, -r * wide), right];
	let down = [left, c + new_vec(-r * 0.45, r * wide * 0.8), c + new_vec(r * 0.45, r * wide * 0.8), right];
	let lids = new_path().move_to(left).cubic_to(up[1], up[2], right).cubic_to(down[2], down[1], left).close();
	if !small {
		e.add_path(lids.hatch(#{ spacing: r * 0.06, angle: 2.4, roughness: r * 0.01, inset: r * 0.03 }), global::GRAPHITE);
	}
	let iris = new_circle(c, r * 0.42);
	e.add_path(iris.hatch(#{ spacing: if small { 4 } else { r * 0.08 }, angle: [0.7, 2.3], roughness: r * 0.01 }), global::GRAPHITE);
	let fibres = if small { 12 } else { 36 };
	for i in 0..fibres {
		let a = i * 2.0 * PI() / fibres;
		let d = new_vec(a.cos(), a.sin());
		e.add_path(new_line(c + d * (r * 0.14), c + d * (r * 0.41)), global::FINE);
	}
	e.add_path(iris, outline);
	let slit = new_path().move_to(c + new_vec(0, -r * 0.36)).quad_to(c + new_vec(r * 0.13, 0), c + new_vec(0, r * 0.36))
		.quad_to(c + new_vec(-r * 0.13, 0), c + new_vec(0, -r * 0.36)).close();
	e.add_path(slit.hatch(#{ spacing: if small { 2 } else { r * 0.012 }, angle: [0.3, 1.9] }), global::FINE);
	e.add_path(slit, outline);
	e.add_path(lids, outline);
	// Lashes from the upper lid, drawn tip first so the ramp thickens toward the lid.
	for i in 1..14 {
		let t = i / 14.0;
		let root = bez(up[0], up[1], up[2], up[3], t);
		let out = (root - c).normalized();
		e.add_path(curl(root, out.angle(), r * (0.25 + 0.35 * (1.0 - (2.0 * t - 1.0).abs())), (t - 0.5) * 0.25, 10), if small { global::GRAPHITE } else { global::LASH });
	}
	e.rotate_around(c, turn)
}

// A hand of shadow reaching up from `wrist`, its fingers clawed toward
// `toward` (an angle).
fn hand(wrist, toward, size) {
	let h = new_canvas();
	let dir = new_vec(toward.cos(), toward.sin());
	let palm = wrist + dir * (size * 0.5);
	h.add_path(new_line(wrist - dir * (size * 1.6), palm), global::HAZE);
	h.add_path(new_line(wrist - dir * (size * 1.6), palm), global::THORN);
	let across = dir.rotated(PI() / 2.0);
	for f in 0..5 {
		// The thumb splays out; the fingers reach long and claw inward.
		let spread = if f == 0 { -0.9 } else { (f - 2.5) * 0.16 };
		let len = size * [0.8, 1.5, 1.75, 1.6, 1.2][f];
		let base = palm + across * (size * 0.13 * (f - 2.2)) + dir * (if f == 0 { -size * 0.2 } else { size * 0.1 });
		let finger = curl_points(base, toward + spread, len, if f == 0 { 0.06 } else { -spread * 0.12 - 0.015 }, 16);
		// Tip first, so the ramp thickens toward the knuckles.
		h.add_path(reversed(finger), global::THORN);
	}
	h.add_path(new_circle(palm, size * 0.3).hatch(#{ spacing: size * 0.05, angle: [toward, toward + 1.5], roughness: 2 }), global::SHADE);
	h
}

// Points on a circle.
fn ring_point(c, r, a) {
	c + new_vec(a.cos(), a.sin()) * r
}

// The first plate: the sleeper's eye, in a seven-pointed circle, weeping
// blood, with hands of shadow reaching up for it.
let plate = new_canvas();
let centre = new_vec(600, 780);
// Night gathering at the edges.
for i in 0..26 {
	let a = i * PI() / 13;
	let from = ring_point(centre, 760 + random() * 120, a);
	plate.add_path(curl(from, a + PI() + 0.4, 260 + random() * 160, 0.05, 16), SHADOW);
}
for i in 0..14 {
	plate.add_path(wisp(new_vec(LEFT + random() * 1000, BOTTOM + 40), 300 + random() * 500, 60), HAZE);
}
// The circle: rings, a chant in eldritch between them, and a heptagram.
let r_out = 420;
let r_in = 360;
plate.add_path(new_circle(centre, r_out + 18), OUTLINE);
plate.add_path(new_circle(centre, r_out), SIGIL);
plate.add_path(new_circle(centre, r_in), SIGIL);
let chant = "ph nglui mglw nafh cthulhu r lyeh wgah nagl fhtagn ia ia ";
plate.add_canvas(new_circle(centre, (r_out + r_in) / 2 - 10).fit_text(chant + chant, font("eldritch"), new_style(1, INK, PEN)));
let star = new_path();
for i in 0..=7 {
	let p = ring_point(centre, r_in, -PI() / 2.0 + i * 6.0 * PI() / 7.0);
	if i == 0 { star.move_to(p); } else { star.line_to(p); }
}
plate.add_path(star, SIGIL);
// The seven archangels' seals between the points, outside the circle.
for (seal, i) in font("archangel").glyphs("0123456") {
	seal.scale = 70;
	seal.position = ring_point(centre, r_out + 75, -PI() / 2.0 + (i + 0.5) * 2.0 * PI() / 7.0);
	plate.add_glyph(seal, OUTLINE);
	plate.add_path(new_circle(seal.position, 46), FINE);
}
plate.add_canvas(eye(centre, 230, 0.0, 0.62));
// Tears of blood down to a pool, and a stain where they gather.
for (dx, i) in [-70, 10, 85] {
	let from = centre + new_vec(dx, 150 - dx.abs() * 0.4);
	let fall = new_path().move_to(from);
	for k in 1..=20 {
		let t = k / 20.0;
		fall.line_to(from + new_vec(14 * (t * 11 + i).sin(), 520 * t));
	}
	plate.add_path(fall.copy(), BLEED);
	plate.add_path(fall, new_style(1, BLOOD, plate_brush("Ink Pen", 0.1, 0.9, 3, "ramp")));
	plate.add_path(new_circle(from + new_vec(0, 535), 14 + i * 3).hatch(4, 0.8, 1), new_style(1, BLOOD, PEN));
}
plate.add_path(wave(400, 800, 1335, 10), new_style(1, BLOOD, plate_brush("Rough Ink", 0.2, 0.6, 4, "taper")));
// Hands reaching up from below.
plate.add_canvas(hand(new_vec(260, 1430), -PI() / 2.0 + 0.45, 150));
plate.add_canvas(hand(new_vec(950, 1450), -PI() / 2.0 - 0.4, 170));
// The dream takes hold: sponged dark at the corners, the ring smeared and
// warped, the lower edge blurred.
for i in 0..5 {
	let a = 0.3 + i * 1.25;
	plate.add_path(curl(ring_point(centre, r_out - 30, a), a + PI() / 2.0, 160, 0.06, 10), MELT);
}
for i in 0..3 {
	let a = 2.2 + i * 0.5;
	plate.add_path(new_arc(centre, (r_out + r_in) / 2, a, a + 0.25), SMEAR);
}
plate.add_path(new_line(new_vec(LEFT, 1440), new_vec(RIGHT, 1440)), DREAM);
// The running head and label over the picture.
plate.add_canvas(sheet("visions"));
plate.add_canvas(caption("the sleeper's eye: Airbrush, Charcoal, Hair, Pencil, Calligraphy, Watercolor, Smudge, Liquify", TOP));
book.bind("visions", plate);

// A ridged horn from `root`, curling as `curl_points` does, `width` across
// at the root and narrowing to a point.
fn horn(root, angle, length, bend, width) {
	let h = new_canvas();
	let spine = curl_points(root, angle, length, bend, 36);
	let n = spine.len();
	let left = [];
	let right = [];
	for i in 0..n {
		let t = i / (n - 1.0);
		let along = (spine[if i + 1 < n { i + 1 } else { i }] - spine[if i > 0 { i - 1 } else { i }]).normalized();
		let w = width * (1.0 - t) * (1.0 - 0.25 * t) + 1.0;
		left.push(spine[i] + along.rotated(PI() / 2.0) * w);
		right.push(spine[i] - along.rotated(PI() / 2.0) * w);
	}
	let back = right;
	back.reverse();
	let outline = through(left + back).close();
	h.add_path(outline.hatch(#{ spacing: 7, angle: angle + 1.3, roughness: 1, bow: 1.5, inset: 1 }), global::SHADE);
	for i in range(2, n - 2, 2) {
		h.add_path(new_path().move_to(left[i]).quad_to(spine[i] + (spine[i + 1] - spine[i]) * 1.5, right[i]), global::FINE);
	}
	h.add_path(outline, global::OUTLINE);
	h
}

// The second plate: a horned thing dreaming under a black moon, its body
// smoke and serpents, falling into a whirlpool of text; eyes everywhere.
let plate = new_canvas();
// A bruised sky, and night at the edges.
for i in 0..3 {
	plate.add_path(wave(LEFT - 60, RIGHT + 60, TOP + 120 + i * 90, 30 + i * 8), new_style(1, "rgba(122, 20, 16, 0.12)", plate_brush("Rough Watercolor", 0.2, 0.7, 16, "taper")));
}
for i in 0..22 {
	let a = i * PI() / 11;
	plate.add_path(curl(ring_point(new_vec(600, 800), 780 + random() * 120, a), a + PI() - 0.3, 240 + random() * 160, -0.04, 16), SHADOW);
}
// The black moon: sponged dark, haloed, the halo blurred.
let moon = new_vec(900, 400);
plate.add_path(new_circle(moon, 125), new_style(1, "rgba(155, 35, 24, 0.22)", plate_brush("Airbrush", 0.3, 0.9, 20, "constant")));
plate.add_path(new_circle(moon, 85).hatch(#{ spacing: 14, angle: [0.5, 2.1], roughness: 3 }), SPONGED);
plate.add_path(new_circle(moon, 88), OUTLINE);
plate.add_path(new_circle(moon, 170), DREAM);

// The body: smoke pouring down from the jaw, and serpents in it.
let head = new_vec(520, 690);
for i in 0..18 {
	let from = head + new_vec(-160 + i * 19, 150 + random() * 60);
	let fall = new_path().move_to(from);
	let phase = random() * 6.28;
	for k in 1..=20 {
		let t = k / 20.0;
		fall.line_to(from + new_vec((i - 9) * 22 * t + 40 * (phase + t * 7).sin() * t, 520 * t));
	}
	plate.add_path(fall.copy(), HAZE);
	plate.add_path(fall, LASH);
}
for s in [[new_vec(330, 1010), 2.6, new_vec(470, 1300), 3], [new_vec(760, 980), 0.4, new_vec(640, 1280), 8], [new_vec(560, 1160), -0.6, new_vec(400, 1180), 11]] {
	plate.add_path(art::snake_path(s[0], s[1], s[2], 620, 30, s[3]), OUTLINE);
}

// The head: horns, a pointed face shaded down one side, three eyes, a
// grin of teeth.
plate.add_canvas(horn(head + new_vec(-95, -130), -2.0, 520, -0.065, 46));
plate.add_canvas(horn(head + new_vec(95, -130), -1.14, 520, 0.065, 46));
let face = new_path().move_to(head + new_vec(0, -190))
	.cubic_to(head + new_vec(120, -190), head + new_vec(175, -60), head + new_vec(150, 20))
	.cubic_to(head + new_vec(130, 120), head + new_vec(60, 200), head + new_vec(0, 250))
	.cubic_to(head + new_vec(-60, 200), head + new_vec(-130, 120), head + new_vec(-150, 20))
	.cubic_to(head + new_vec(-175, -60), head + new_vec(-120, -190), head + new_vec(0, -190));
plate.add_path(face.hatch(#{ spacing: 7, angle: 2.3, roughness: 1.5, inset: 3 }), GRAPHITE);
let shadow_side = new_path().move_to(head + new_vec(40, -185)).cubic_to(head + new_vec(160, -150), head + new_vec(170, 60), head + new_vec(0, 248))
	.cubic_to(head + new_vec(90, 120), head + new_vec(110, -40), head + new_vec(40, -185));
plate.add_path(shadow_side.hatch(#{ spacing: 6, angle: [0.9, 1.6], roughness: 1.5 }), SHADE);
// Hollow cheeks and a cleft brow.
for side in [-1, 1] {
	plate.add_path(new_path().move_to(head + new_vec(side * 120, 10)).quad_to(head + new_vec(side * 70, 60), head + new_vec(side * 95, 140)), THORN);
	plate.add_path(new_line(head + new_vec(side * 12, 20), head + new_vec(side * 22, 60)), THORN);
}
plate.add_path(face, OUTLINE);
for side in [-1, 1] {
	let at = head + new_vec(side * 62, -20);
	plate.add_path(new_line(at - new_vec(25, 0), at + new_vec(25, 0)), GLOW);
	plate.add_canvas(eye(at, 52, side * 0.35, 0.42));
	plate.add_path(new_path().move_to(head + new_vec(side * 15, -70)).quad_to(head + new_vec(side * 70, -45), head + new_vec(side * 125, -75)), THORN);
}
plate.add_canvas(eye(head + new_vec(0, -125), 34, PI() / 2.0, 0.5));
// The grin: the dark of the mouth, its edges notched by fangs, which are
// left bare.
let corner_l = head + new_vec(-110, 95);
let corner_r = head + new_vec(110, 95);
let upper = [corner_l];
let lower = [];
for k in 1..12 {
	let t = k / 12.0;
	let top = bez(corner_l, head + new_vec(-37, 125), head + new_vec(37, 125), corner_r, t);
	let bottom = bez(corner_l, head + new_vec(-37, 190), head + new_vec(37, 190), corner_r, t);
	let fang = 30 - 30 * (2.0 * t - 1.0).abs();
	upper.push(if k % 2 == 1 { top + new_vec(0, 8 + fang) } else { top });
	lower.push(if k % 2 == 0 { bottom - new_vec(0, 4 + fang * 0.4) } else { bottom });
}
upper.push(corner_r);
lower.reverse();
let grin = through(upper + lower).close();
plate.add_path(grin.hatch(#{ spacing: 3, angle: [1.2, 2.0] }), FINE);
plate.add_path(grin, OUTLINE);

// Eyes in the dark, watching.
for e in [[new_vec(190, 400), 0.3, 40], [new_vec(1020, 800), -0.5, 46], [new_vec(170, 920), 0.9, 34], [new_vec(1000, 1150), 0.2, 38], [new_vec(700, 330), -0.2, 26], [new_vec(280, 1250), -0.7, 30]] {
	plate.add_canvas(eye(e[0], e[2], e[1], 0.55));
}

// The whirlpool the dreamer falls into: text spiralling in, then warped.
let pool = new_vec(600, 1330);
let spiral = new_path().move_to(pool + new_vec(360, 0));
for i in 1..300 {
	let a = i * 0.04;
	let r = 360 - i * 1.05;
	spiral.line_to(pool + new_vec(r * a.cos(), r * a.sin() * 0.4));
}
plate.add_canvas(spiral.fill_with_text(LOREM, PLAIN, 20, new_style(1, RED, PEN))[0]);
for i in 0..3 {
	plate.add_path(new_arc(pool, 140 + i * 70, 0.4 + i, 1.4 + i).scale_by(1), MELT);
}

// Thorns climbing in from the corners, blooming red.
for b in [[new_vec(LEFT - 20, BOTTOM + 40), -1.2, 3], [new_vec(RIGHT + 20, BOTTOM + 40), -1.94, 6]] {
	let thorns = art::bramble(b[0], b[1], 520, 40, b[2]);
	plate.add_path(thorns.vines, OUTLINE);
	plate.add_path(thorns.leaves, FINE);
	plate.add_path(thorns.flowers, SIGIL);
}
// Spattered ink.
for i in 0..40 {
	let at = head + new_vec(random_range(-420, 420), random_range(-480, -180));
	plate.add_path(new_circle(at, random_range(0.5, 3)), new_style(1, INK, plate_brush("Rough Ink", 0.5, 1.0, 2, "constant")));
}
// Smeared from the chin into the smoke.
for i in 0..3 {
	plate.add_path(new_line(head + new_vec(-40 + i * 40, 230), head + new_vec(-60 + i * 60, 420)), SMEAR);
}
plate.add_canvas(sheet("visions"));
plate.add_canvas(caption("the horned dreamer: Rough Watercolor, Sponge, Blur, Airbrush, Hair, Charcoal, Liquify", TOP));
book.bind("visions", plate);

// ---- the title page and the contents, which go first ----

// layouts/bordered_title.rhai, a framed title page.
let leaf = new_path().move_to(new_vec(0, -22)).quad_to(new_vec(14, -4), new_vec(0, 22)).quad_to(new_vec(-14, -4), new_vec(0, -22));
let title = layout("bordered_title").typeset(#{
	width: W, height: H,
	flows: #{ title: "The Showcase", subtitle: "a sample book of everything there is to draw with", credits: "set with a layout file:\nlayouts/bordered_title.rhai" },
	roles: #{
		title: #{ font: font("copper"), style: HEADING, size: 150, line_height: 1.0 },
		subtitle: #{ font: PLAIN, style: TEXT, size: 34 },
		credits: #{ font: PLAIN, style: TEXT, size: 26 },
		frame: #{ style: CAPTION },
		border: #{ style: LINE },
	},
	art: #{ border: leaf },
})[0];
title.name = "01_title";
journal_entry.add_page(title);

let contents = sheet("contents");
let y = TOP + 40;
for entry in book.contents {
	contents.add_canvas(write_in(entry[0], LEFT + 60, y, 40, HEADING, LOCHE, #{}));
	contents.add_path(new_line(new_vec(420, y + 12), new_vec(980, y + 12)), new_style(1, RED, new_brush("Ink Pen", 0.2, 0.4, 1)));
	contents.add_canvas(write(`${entry[1]}`, 1010, y, 30));
	y += 110;
}
let contents_page = new_page();
contents_page.name = "02_contents";
contents_page.size = new_vec(W, H);
contents_page.svg_style = #{ roughness: 1.5, noise_amplitude: 0.8 };
contents_page.add_canvas(contents);
journal_entry.add_page(contents_page);

journal_entry.add_pages(book.pages);

// Every character above has a glyph.
if missing_glyphs().len() > 0 {
	warn("characters with no glyph: " + missing_glyphs());
}
