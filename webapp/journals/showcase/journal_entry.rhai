// The showcase: a sample book of everything journals can draw with, every
// page the same size. After the title page and the contents:
//
//   pens      every Darkly brush; pressure profiles, widths, colours,
//             gradients, strokes gone over in passes and reshaped,
//             pooling ink and wandering pressure
//   fonts     every font, its alphabet and a line, font settings,
//             italics and spacing by outlines, and glyphs placed by hand
//   glyphs    every variant of every glyph of every font
//   art       shapes and paths (polygons, stars, ellipses, waves, points
//             along a path), hatching, vectors, mazes, snakes, brambles,
//             eyes, moons in every phase, paths cut by shapes, brambles
//             thickening toward an origin, Huffman trees as star charts
//   generators  point distributions, vec3 and vec4, spanning trees by
//             algorithm and metric, balanced-ternary values (as glyphs, and
//             along paths), angles in degrees
//   text      text boxes, text kept clear of drawings, text along paths
//             (turned to them or held at an angle), scattered notes,
//             words split round drawings, shaped boxes, and writing by
//             hand: lines, blocks and glyphs set a little off true
//   locks     every lock type, Huffman codes between bases, a lock
//             chain drawn as a maze, value kinds and their sketches, an
//             art node: a lock keyed by a code drawn as a star chart, and
//             renderers drawing a chain's values as canvases
//   spread    one drawing across two pages: a page split into tiles
//   layouts   pages typeset from layouts: an inline one, the layout files
//             (the title page is one too), and marked-up text, markdown()
//
// Pages are bound with new_book: numbered, named and listed in the
// contents.
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
const TEXT = new_style(INK, PEN);
const CAPTION = new_style(RED, new_brush("Ink Pen", 0.3, 0.7, 1.2));
const LINE = new_style(INK, new_brush("Ink Pen", 0.3, 0.7, 2.0));
// Headings' calligraphy nib, with the constant pressure a broad nib wants.
const HEADING = new_style(RED, new_brush("Calligraphy", 0.4, 0.8, 1.5, #{ pressure_profile: "constant" }), #{ width: 1.2 });
// Font samples and glyphs: a fine pen, so the glyphs' shapes show.
const SAMPLE = new_style(INK, new_brush("Ink Pen", 0.4, 0.8, 2.2));
// A bramble's parts' styles, by part.
const BRAMBLE = #{ vines: new_style(BARK, PEN), leaves: new_style(GREEN, PEN), flowers: LINE };

// Text is in plain, a font made to read easily; headings are in loche.
const PLAIN = font("plain");
const LOCHE = font("loche_upper");

const LOREM = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. ";

// ---- writing ----

// `text` written from (x, y) to the right, along a line through the middle
// of its glyphs: write_line(text, at, settings), settings being a role's.
fn write(text, x, y, size) {
	write_line(text, vec2(x, y), #{ font: global::PLAIN, style: global::TEXT, size: size })
}

fn write_in(text, x, y, size, style, font) {
	write_line(text, vec2(x, y), #{ font: font, size: size, style: style })
}

// A label naming the calls an example shows.
fn caption(text, y) {
	write_in(text, global::LEFT, y, 22, global::CAPTION, global::PLAIN)
}

// A heading within a page.
fn subhead(text, y) {
	write_in(text, global::LEFT, y, 36, global::HEADING, global::LOCHE)
}

// A wave from x0 to x1 about y, its curves' controls rise above and below
// it: rising at most rise * sqrt(3) / 6.
fn wave(x0, x1, y, rise) {
	new_wave(vec2(x0, y), vec2(x1, y), #{ amplitude: rise * 3.0.sqrt() / 6.0 })
}

// An art::eye's parts drawn as a canvas: its iris washed in colour with
// watercolour brushes sized by its iris_radius, darkest where it pools at
// the rim, its lids gone round in the colour, and its lines in ink.
fn inked_eye(eye, colour, ink) {
	let r = eye.iris_radius;
	let brush = |name, min, max, size| new_brush(name, min, max, size, #{ pressure_profile: "taper" });
	// Part by part, so the colour goes under the lines (canvas.add(eye,
	// styles) would draw them in name order).
	new_canvas()
		.add(eye.wash, new_style(colour, brush.call("Smooth Watercolor", 0.2, 0.35, r * 0.3)))
		.add(eye.rim, new_style(colour, brush.call("Rough Watercolor", 0.6, 0.95, r * 0.12)))
		.add(eye.accent, new_style(colour, brush.call("Ink Pen", 0.2, 0.6, 1.2)))
		.add(eye.lines, ink)
}

// ---- binding pages into the book ----
//
// new_book numbers pages from 3, after the title page and the contents,
// and names them for their number and section, so their files sort in
// order; book.contents says where each section starts.

let book = new_book(#{ first: 3 });

// A fresh page for `section`: a canvas with its running head drawn.
fn sheet(section) {
	new_canvas()
		.add(write_in(section, global::LEFT, 105, 48, global::HEADING, global::LOCHE))
		.add(new_line(vec2(global::LEFT, 150), vec2(global::RIGHT, 150)), global::CAPTION)
}

// Bind a sheet: a page the book's size, with a folio under it.
fn bind(section, canvas) {
	canvas.add(write_in(`${this.next}`, global::W / 2 - 8, global::H - 80, 24, global::CAPTION, global::PLAIN));
	this.add(section, new_page(#{ size: vec2(global::W, global::H) }).add(canvas));
}

// ---- pens: every brush ----
//
// Each brush draws a wave, over bare paper and then over hatching, since
// some (Blur, Smudge, Liquify, Clone) only move ink that's already there.

let pens = sheet("pens");
pens.name = "brushes";
pens.add(caption("brush_names(), new_brush(name, min_pressure, max_pressure, size), brush.size", TOP));
let y = TOP + 70;
for name in brush_names() {
	pens.add(write(name, LEFT, y, 24));
	pens.add(new_rectangle(vec2(910, y), 380, 54).hatch(7.0, #{ angle: 0.8, roughness: 0.6 }), CAPTION);
	let brush = new_brush(name, 0.3, 0.9, 6);
	brush.size = 9;
	pens.add(wave(380, RIGHT, y, 24), new_style(INK, brush));
	y += 76;
}
book.bind("pens", pens);

// Pressure runs along each stroke as its brush's profile says, between the
// brush's min_pressure and max_pressure.
let pens = sheet("pens");
pens.add(subhead("pressure profiles", TOP));
pens.add(caption("new_brush(..., #{ pressure_profile }): \"ramp\", \"taper\" or \"constant\"; brush.name", TOP + 50));
let y = TOP + 110;
for profile in ["ramp", "taper", "constant"] {
	let brush = new_brush("Ink Pen", 0.05, 1.0, 10, #{ pressure_profile: profile });
	assert(brush.min_pressure == 0.05 && brush.max_pressure == 1.0 && brush.name == "Ink Pen", "unexpected brush " + brush);
	pens.add(write(profile, LEFT, y, 24));
	pens.add(new_line(vec2(380, y), vec2(RIGHT, y)), new_style(GOLD, brush));
	y += 60;
}

// A style's width scales its brush's pressure (in SVG output it's the
// stroke's width).
y += 30;
pens.add(subhead("widths", y));
pens.add(caption("new_style(colour, brush, #{ width }), style.width", y + 50));
y += 110;
for (width, i) in [0.3, 0.7, 1.2] {
	let style = new_style(BLUE, new_brush("Charcoal", 0.3, 0.8, 8), #{ width: width });
	assert(style.width == width, "width reads back");
	pens.add(write(["light", "middling", "heavy"][i], LEFT, y, 24));
	pens.add(wave(380, RIGHT, y, 14), style);
	y += 60;
}

// Colours: CSS names, hex, rgb(), hsl() and alpha.
y += 30;
pens.add(subhead("colours", y));
pens.add(caption("CSS colours: named, hex, rgb(), hsl(), and alpha", y + 50));
y += 110;
let colours = [["named", "darkgoldenrod"], ["hex", "#1f4e7a"], ["rgb", "rgb(155, 35, 24)"], ["hsl", "hsl(100, 50%, 28%)"], ["alpha", "rgba(43, 29, 18, 0.4)"]];
for (colour, i) in colours {
	let x = LEFT + i * 205;
	let style = new_style(colour[1], new_brush("Rough Ink", 0.4, 0.8, 9));
	assert(style.color == colour[1], "colour didn't stick");
	pens.add(write(colour[0], x, y, 22));
	pens.add(wave(x, x + 170, y + 45, 14), style);
}

// Gradients, in place of a colour: along each stroke from start to end,
// across the page from one point to another, or out from a centre.
// Colours are spaced evenly, or placed as [position, colour].
y += 150;
pens.add(subhead("gradients", y));
y += 70;
pens.add(caption("new_gradient(colours): along each stroke", y));
pens.add(wave(640, RIGHT, y, 22), new_style(new_gradient([RED, GOLD, BLUE]), new_brush("Rough Ink", 0.4, 0.8, 9)));
y += 75;
pens.add(caption("#{ from, to }: across, colours placed", y));
let across = new_gradient([GREEN, [0.7, GOLD], RED], #{ from: vec2(640, 0), to: vec2(RIGHT, 0) });
pens.add(new_rectangle(vec2(870, y), 460, 50).hatch(8.0, #{ angle: 1.1 }), new_style(across, PEN));
y += 75;
let words = new_style(RED, PEN);
words.color = new_gradient([BLUE, INK, RED], #{ from: vec2(640, 0), to: vec2(RIGHT, 0) });
pens.add(caption("style.color = a gradient", y));
pens.add(write_in("Lorem ipsum dolor sit amet", 640, y, 28, words, PLAIN));
y += 130;
pens.add(caption("#{ center, radius }: out from a point", y));
let glow = new_gradient([INK, GOLD, "rgba(155, 35, 24, 0.3)"], #{ center: vec2(870, y), radius: 100 });
let spiral = new_path().move_to(vec2(870, y));
for i in 1..220 {
	let a = i * 0.11;
	spiral.line_to(vec2(870 + a * 4.0 * a.cos(), y + a * 4.0 * a.sin()));
}
pens.add(spiral, new_style(glow, new_brush("Ink Pen", 0.4, 0.8, 3)));
book.bind("pens", pens);

// Strokes gone over: a style draws each stroke in `passes` passes, each
// pass's ends straying by up to `jitter` units. One pass with some jitter
// roughens a hand; many make the manic, scored-over look of writing gone
// over again and again. Text in the style is gone over too.
let pens = sheet("pens");
pens.add(subhead("going over strokes", TOP));
pens.add(caption("new_style(..., #{ passes: n, jitter: d }): each stroke drawn n times, its ends astray by up to d", TOP + 50));
let y = TOP + 130;
for row in [[1, 0, "clean"], [1, 2.5, "a rough hand"], [3, 4, "gone over"], [7, 7, "and over"]] {
	let style = new_style(INK, new_brush("Ink Pen", 0.5, 0.9, 0.8), #{ passes: row[0], jitter: row[1] });
	assert(style.passes == row[0] && style.jitter == row[1], "passes and jitter read back");
	pens.add(write(`${row[2]}: ${row[0]}, ${row[1]}`, LEFT, y, 24));
	pens.add(wave(380, 760, y, 22), style);
	pens.add(new_circle(vec2(850, y), 30), style);
	pens.add(write_in("lorem", 920, y, 56, style, LOCHE));
	y += 110;
}
y += 40;
pens.add(caption("a manic hand: passes = 7, jitter = 9, in a tapering pen", y));
let scored = new_brush("Ink Pen", 0.5, 0.95, 1.1, #{ pressure_profile: "taper" });
let manic = new_style(INK, scored, #{ passes: 7, jitter: 9 });
pens.add(write_in("lorem ipsum", LEFT, y + 110, 96, manic, LOCHE));
pens.add(write_in("dolor sit amet", LEFT, y + 210, 96, manic, LOCHE));
y += 300;
pens.add(caption("a heading gone over twice, passes = 2, jitter = 3, in red", y));
let twice = new_style(RED, scored, #{ passes: 2, jitter: 3 });
pens.add(write_in("consectetur adipiscing", LEFT, y + 80, 60, twice, LOCHE));
y += 170;
pens.add(caption("body text in a rough hand: style.passes = 1, style.jitter = 1.2", y));
let rough = TEXT;
rough.passes = 1;
rough.jitter = 1.2;
pens.add(write_in("Lorem ipsum dolor sit amet, consectetur adipiscing elit,", LEFT, y + 60, 30, rough, PLAIN));
pens.add(write_in("sed do eiusmod tempor incididunt ut labore et dolore.", LEFT, y + 110, 30, rough, PLAIN));
book.bind("pens", pens);

// Strokes reshaped: point_jitter moves every point of every pass, control
// points included, each its own way, so curves bend differently rather
// than just shifting.
let pens = sheet("pens");
pens.add(subhead("reshaping strokes", TOP));
pens.add(caption("new_style(..., #{ point_jitter: d }): every point of every pass, control points too, astray by up to d", TOP + 50));
let y = TOP + 130;
for d in [0, 1.5, 4, 8] {
	let style = new_style(INK, new_brush("Ink Pen", 0.5, 0.9, 0.8), #{ point_jitter: d });
	assert(style.point_jitter == d, "point_jitter reads back");
	pens.add(write(`point_jitter ${d}`, LEFT, y, 24));
	pens.add(wave(380, 760, y, 22), style);
	pens.add(new_circle(vec2(850, y), 30), style);
	pens.add(write_in("lorem", 920, y, 56, style, LOCHE));
	y += 110;
}
y += 40;
pens.add(caption("with passes: passes = 4, jitter = 3, point_jitter = 3, each pass bent its own way", y));
let bent = new_style(INK, scored, #{ passes: 4, jitter: 3, point_jitter: 3 });
pens.add(write_in("lorem ipsum", LEFT, y + 110, 96, bent, LOCHE));
y += 200;
pens.add(caption("body text with unsteady curves: style.point_jitter = 0.8, in red", y));
let unsteady = TEXT;
unsteady.color = RED;
unsteady.point_jitter = 0.8;
pens.add(write_in("Lorem ipsum dolor sit amet, consectetur adipiscing elit,", LEFT, y + 60, 30, unsteady, PLAIN));
pens.add(write_in("sed do eiusmod tempor incididunt ut labore et dolore.", LEFT, y + 110, 30, unsteady, PLAIN));
book.bind("pens", pens);

// Ink as a hand leaves it: pooling toward the bottom of each glyph, a
// gradient down each glyph, and pressure wandering along each stroke.
let pens = sheet("pens");
pens.add(subhead("pooling ink", TOP));
pens.add(caption("new_brush(..., #{ pool: p, pool_depth: d }): up to p heavier over the lowest d of each glyph", TOP + 50));
let y = TOP + 150;
for row in [[0, 0.3], [0.6, 0.3], [1.2, 0.5]] {
	let brush = new_brush("Ink Pen", 0.2, 0.45, 3.2, #{ pool: row[0], pool_depth: row[1] });
	assert(brush.pool == row[0] && brush.pool_depth == row[1], "pool and pool_depth read back");
	pens.add(write(`pool ${row[0]}, depth ${row[1]}`, LEFT, y, 24));
	pens.add(write_in("lorem ipsum", 400, y, 72, new_style(INK, brush), LOCHE));
	y += 105;
}
pens.add(caption("new_gradient(colours, #{ vertical: true }): down each glyph, top to bottom", y));
let darkening = new_gradient([RED, [0.6, RED], INK], #{ vertical: true });
pens.add(write_in("dolor sit amet", LEFT, y + 90, 72, new_style(darkening, new_brush("Ink Pen", 0.3, 0.6, 3.2)), LOCHE));
y += 175;
pens.add(caption("both: ink pooling dark at the bottom of plain text", y));
// Properties set the same settings after: brush.pool, brush.pool_depth.
let pooled = new_brush("Ink Pen", 0.3, 0.6, 1.6);
pooled.pool = 0.5;
pooled.pool_depth = 0.3;
let pooled_ink = new_style(new_gradient([RED, [0.5, RED], INK], #{ vertical: true }), pooled);
pens.add(write_in("Lorem ipsum dolor sit amet, consectetur adipiscing elit,", LEFT, y + 60, 30, pooled_ink, PLAIN));
pens.add(write_in("sed do eiusmod tempor incididunt ut labore et dolore.", LEFT, y + 110, 30, pooled_ink, PLAIN));

y += 190;
pens.add(subhead("wandering pressure", y));
pens.add(caption("brush.pressure_variation = v, brush.variation_length = l: up to v either way, every l or so", y + 50));
y += 130;
for profile in ["ramp", "taper", "constant"] {
	pens.add(write(profile, LEFT, y, 24));
	for (variation, i) in [0, 0.35] {
		let brush = new_brush("Ink Pen", 0.05, 0.7, 10);
		brush.pressure_profile = profile;
		brush.pressure_variation = variation;
		brush.variation_length = 70;
		assert(brush.pressure_variation == variation && brush.variation_length == 70, "variation reads back");
		pens.add(new_line(vec2(330 + i * 400, y), vec2(690 + i * 400, y)), new_style(GOLD, brush));
	}
	y += 55;
}
pens.add(caption("a hand that wanders: new_brush(..., #{ pressure_variation: 0.35, variation_length: 10 }), tapering", y));
let wandering = new_brush("Ink Pen", 0.3, 0.7, 1.8, #{ pressure_profile: "taper", pressure_variation: 0.35, variation_length: 10 });
pens.add(write_in("lorem ipsum dolor", LEFT, y + 80, 72, new_style(INK, wandering), LOCHE));
book.bind("pens", pens);

// ---- fonts ----
//
// Fonts come from fonts/ (a file for each font), each with its glyphs, the padding
// between them, the width of a space, and a font to fall back to.

let alphabet = "abcdefghijklmnopqrstuvwxyz";
let lorem = "lorem ipsum dolor sit amet, consectetur";
let samples = [
	#{ label: "font(\"plain\"): made to read easily", font: PLAIN, lines: [alphabet.to_upper(), alphabet, "0123456789 . , ; : ! ? ' \" - — _ ~ ( ) [ ] { } / \\ # + = * < > % &", "Lorem ipsum dolor sit amet, consectetur."] },
	#{ label: "font(\"copper\"): falling back to loche_upper for what it lacks", font: font("copper"), lines: [alphabet.to_upper(), alphabet, "Lorem Ipsum Dolor Sit Amet", "variants at random: aaaa eeee ssss"] },
	#{ label: "font(\"loche_upper\"): capitals, lowercase text set in them", font: LOCHE, lines: [alphabet, lorem + ".", "punctuation . , ; : ' \" ? - —"] },
	#{ label: "font(\"loche_lower\"): falling back to loche_upper for capitals and stops", font: font("loche_lower"), lines: [alphabet, "Lorem ipsum dolor sit amet, consectetur."] },
	#{ label: "font(\"nivelia\"): traced from her letter, falling back to loche_upper for the capitals she never wrote", font: font("nivelia"), size: 46, lines: [alphabet, alphabet.to_upper(), "Lorem ipsum dolor sit amet, consectetur.", "A iq ixoias rx jrg, ibs A srb'z Kibz zr nc."] },
	#{ label: "font(\"lang\")", font: font("lang"), lines: [alphabet.to_upper(), alphabet, "Lorem ipsum dolor sit amet."] },
	#{ label: "font(\"eldritch\")", font: font("eldritch"), lines: [alphabet, "lorem ipsum dolor sit amet"] },
	#{ label: "font(\"sharp_lower\")", font: font("sharp_lower"), lines: [alphabet, "lorem ipsum dolor sit amet"] },
	#{ label: "ternary_font(\"balanced_ternary\", #{ character_padding: 0.2 }): letters to the ternary digits", font: ternary_font("balanced_ternary", #{ character_padding: 0.2 }), lines: ["abcdefghijklm", "nopqrstuvwxyz", "lorem ipsum dolor"] },
];
let fonts = sheet("fonts");
let y = TOP;
for f in samples {
	// Fonts with a small x-height are written larger, so the pen doesn't
	// fill them in.
	let size = if "size" in f { f.size } else { 36 };
	let leading = 54 * size / 36;
	let height = 50 + f.lines.len() * leading + 30;
	if y + height > BOTTOM {
		book.bind("fonts", fonts);
		fonts = sheet("fonts");
		y = TOP;
	}
	fonts.add(caption(f.label, y));
	y += 50 + (leading - 54) / 2;
	for line in f.lines {
		fonts.add(write_in(line, LEFT, y, size, SAMPLE, f.font));
		y += leading;
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
fonts.add(subhead("font settings", y));
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
	fonts.add(caption(row[0], y));
	fonts.add(write_in(lorem, LEFT, y + 46, 36, SAMPLE, row[1]));
	y += 106;
}
let lettered = font("eldritch");
lettered.fallback = "copper";
assert(lettered.fallback.name == "copper" && font("plain").fallback == () && font_names().contains("loche_lower"), "fallbacks and names");
fonts.add(caption("f.fallback = \"copper\": eldritch, with capitals and stops from copper", y));
fonts.add(write_in("Lorem ipsum. Dolor sit amet.", LEFT, y + 46, 36, SAMPLE, lettered));
y += 130;

// Italics: a font's skew leans every glyph it sets (its fallbacks' too),
// tops forward, in radians. Text is set with each glyph as close to the
// last as their outlines allow, plus the padding, so leaning text doesn't
// open it up, and an A tucks under a V.
if y + 560 > BOTTOM {
	book.bind("fonts", fonts);
	fonts = sheet("fonts");
	y = TOP;
}
fonts.add(subhead("italics", y));
y += 60;
let backslant = font("loche_lower");
backslant.skew = -0.2;
assert(backslant.skew == -0.2 && font("loche_lower").skew == 0, "a font's skew is its copy's");
let leant = [
	["font(\"plain\", #{ skew: 0.25 })", font("plain", #{ skew: 0.25 }), "Lorem ipsum dolor sit amet, consectetur."],
	["font(\"copper\", #{ skew: 0.3 }): its fallback, loche_upper, leant too", font("copper", #{ skew: 0.3 }), "Lorem Ipsum. Dolor Sit Amet?"],
	["f.skew = -0.2: leant back", backslant, lorem],
];
for row in leant {
	fonts.add(caption(row[0], y));
	fonts.add(write_in(row[2], LEFT, y + 46, 36, SAMPLE, row[1]));
	y += 106;
}
fonts.add(caption("set by outlines, upright and leant: glyphs tuck in as far as their hulls allow", y));
for (f, i) in [PLAIN, font("plain", #{ skew: 0.25 })] {
	fonts.add(write_in("AVATAR. To Tea, LT; Yo! don't", LEFT, y + 46 + i * 54, 36, SAMPLE, f));
}
y += 170;

// Glyphs one at a time: placed, scaled and turned by hand.
if y + 330 > BOTTOM {
	book.bind("fonts", fonts);
	fonts = sheet("fonts");
	y = TOP;
}
fonts.add(caption("glyphs placed by hand: glyphs(text), glyph(name), and their scale, position, rotation and skew", y));
y += 80;
let word = font("copper").glyphs("turning");
for (g, i) in word {
	if g.is_blank { continue; }
	g.scale = 50;
	g.position = vec2(LEFT + 40 + i * 70, y);
	g.rotation = i * 0.2;
	g.skew = 0.3 - 0.1 * i;
	g.translate_by(vec2(0, 8 * i)).scale_by(1 + 0.05 * i).rotate_by(0.05);
}
fonts.add(word, SAMPLE);
let eye = font("symbols").glyph("basic_eye", #{ scale: 80, position: vec2(800, y) });
fonts.add(eye, new_style(GOLD, PEN));
fonts.add(eye.copy().path().translate_by(vec2(140, 0)), new_style(BLUE, PEN));
y += 120;
fonts.add(caption("glyph(name, #{ position, scale, rotation, skew }); an array of glyphs moved, scaled and turned as one", y));
y += 80;
let row = [];
for n in -4..=4 {
	row.push(font("balanced_ternary").glyph(n, #{ position: vec2(LEFT + 40 + (n + 4) * 50, y), scale: 40, rotation: n * 0.1 }));
}
fonts.add(row, SAMPLE);
let turned = font("copper").glyphs("lorem");
for (g, i) in turned { g.scale = 40; g.position = vec2(i * 34, 0); }
fonts.add(turned.scale_by(1.2).rotate_around(vec2(0, 0), -0.15).translate_by(vec2(620, y)), SAMPLE);
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
	catalogue.add(caption(`font("${name}")`, y));
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
			g.position = vec2(LEFT + 20 + column * 52, y);
			catalogue.add(g, SAMPLE);
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
art.add(caption("new_circle, new_rectangle, new_line, new_arc, new_path with quad_to and cubic_to", TOP));
let y = TOP + 120;
art.add(new_circle(vec2(160, y), 50), LINE);
art.add(new_rectangle(vec2(300, y), 100, 80).rotate_around(vec2(300, y), 0.2), LINE);
art.add(new_line(vec2(370, y - 50), vec2(470, y + 50)), LINE);
art.add(new_arc(vec2(550, y), 50, 0, 4.5), LINE);
art.add(
	new_path().move_to(vec2(620, y + 50))
		.quad_to(vec2(670, y - 70), vec2(720, y + 50))
		.cubic_to(vec2(760, y + 110), vec2(660, y + 130), vec2(620, y + 50))
		.close()
		.scale_by(1).translate_by(vec2(10, 0)),
	new_style(BLUE, PEN));

// Hatching fills a closed path's inside with lines: hatch(spacing,
// #{ angle, roughness }), here at a random slant.
art.add(new_circle(vec2(880, y), 50), LINE);
art.add(new_circle(vec2(880, y), 50).hatch(5, #{ angle: random_range(0.0, PI()), roughness: 1 }), CAPTION);
art.add(new_rectangle(vec2(1020, y), 90, 90).hatch(6, #{ angle: random() * PI(), roughness: 0.5 }), new_style(GOLD, PEN));

// Hatching with options. Each line is a stroke of its own, inside the
// shape: a ring (two circles appended into one path) keeps its hole, and
// lines can cross, bow and stop short of the outline. Fill rules decide
// what's inside where shapes overlap: "holes" (the default) leaves the
// overlap out, "nonzero" merges them, and "evenodd" also leaves out where
// one line crosses itself, like the middle of a star.
y += 160;
art.add(caption("hatch(spacing, #{ angle, roughness, bow, inset, fill_rule }): a ring, angles, and fill rules", y));
y += 150;
let ring = new_circle(vec2(180, y), 80).append(new_circle(vec2(180, y), 40));
art.add(ring, LINE);
art.add(ring.hatch(9.0, #{ angle: [0.6, 2.2], roughness: 1.0 }), CAPTION);
fn discs(x, y) {
	new_circle(vec2(x - 35, y), 60).append(new_circle(vec2(x + 35, y), 60))
}
// A star in one line: five corners, each joined to the second on.
let star = new_polygon(vec2(880, y + 5), 85, 5, #{ step: 2 });
for shape in [
	[discs(410, y), "holes", 0.9, 380],
	[discs(650, y), "nonzero", 0.9, 600],
	[star, "evenodd", -0.4, 840],
] {
	art.add(shape[0], LINE);
	art.add(shape[0].hatch(8.0, #{ angle: shape[2], fill_rule: shape[1], inset: 2.0 }), new_style(GOLD, PEN));
	art.add(write(shape[1], shape[3], y + 120, 20));
}
y += 200;
let coiled = art::snake(vec2(180, y), vec2(560, y + 120), 700, #{ head_angle: 0, max_width: 50, seed: 5 }).outline;
art.add(coiled, LINE);
art.add(coiled.hatch(7.0, #{ angle: 1.2, bow: 2.5, inset: 2.0 }), new_style(GREEN, PEN));

// Vectors add, subtract and scale by numbers (whole or decimal), and
// measure: a seven-pointed star from one spoke turned round its hub, and
// an arrow whose head is its direction turned back either way, with a dot
// halfway along. Bounds work for paths as for canvases.
y += 220;
art.add(caption("vecs: + - * /, length, distance, dot, angle, normalized, rotated; path bounds", y));
y += 150;
let hub = vec2(190, y);
let spoke = vec2(0, -85);
let star = new_path().move_to(hub + spoke);
for i in 1..=7 {
	star.line_to(hub + spoke.rotated(i * 6 * PI() / 7));
}
art.add(star, new_style(GOLD, PEN));
let tail = vec2(380, y + 60);
let tip = vec2(700, y - 70);
let along = (tip - tail).normalized();
let arrow = new_line(tail, tip);
arrow.append(new_line(tip, tip - along.rotated(0.45) * 28));
arrow.append(new_line(tip, tip - along.rotated(-0.45) * 28));
art.add(arrow, LINE);
art.add(new_circle((tail + tip) / 2, 6), CAPTION);
assert((tip - tail).length() == tail.distance(tip) && along.dot(along) > 0.999 && along.angle() < 0.0 && -along != along, "vectors should measure");
let box = arrow.bounds();
art.add(new_rectangle(box.center, box.width + 20, box.height + 20), new_style(BLUE, PEN));
book.bind("art", art);

let art = sheet("art");
// A maze's corridors.
art.add(caption("new_maze(width, height, #{ seed, strategy }), maze.width, maze.height", TOP));
let maze = new_maze(8, 3, #{ seed: 4 });
assert(maze.width == 15 && maze.height == 5, "a maze's size is its cells'");
art.add(maze.path().scale_by(30).translate_by(vec2(LEFT + 20, TOP + 50)), LINE);
// Shapes from their own constructors, and points spaced along a path.
art.add(write("new_polygon, new_star, new_ellipse, new_wave, new_polyline", 600, TOP + 40, 20));
art.add(new_polygon(vec2(640, TOP + 110), 36, 6), LINE);
art.add(new_star(vec2(730, TOP + 110), 40, 16, 5), new_style(GOLD, PEN));
art.add(new_ellipse(vec2(830, TOP + 110), 45, 22, #{ angle: 0.4 }), LINE);
art.add(new_wave(vec2(890, TOP + 110), vec2(1090, TOP + 110), #{ amplitude: 18, waves: 4 }), new_style(BLUE, PEN));
art.add(new_polyline([vec2(600, TOP + 210), vec2(650, TOP + 170), vec2(700, TOP + 210), vec2(750, TOP + 170)]), LINE);
art.add(write("path.points_along(12)", 790, TOP + 190, 20));
for at in new_circle(vec2(1060, TOP + 180), 30).points_along(12, #{ start: 0.75 }) {
	art.add(new_line(at.position, at.position + vec2(12, 0).rotated(at.angle - PI() / 2.0)), CAPTION);
}

// Snakes: one with too little slack to loop, one coiled over itself, and
// writing along a snake's spine.
art.add(caption("art::snake(head, tail, length, #{ head_angle, max_width, seed }): outline, spine", TOP + 225));
art.add(art::snake(vec2(160, 590), vec2(180, 960), 480, #{ head_angle: -PI() / 2.0, max_width: 40, seed: 4 }).outline, LINE);
art.add(art::snake(vec2(400, 630), vec2(410, 690), 900, #{ head_angle: -PI() / 2.0, max_width: 36, seed: 7 }).outline, LINE);
let snake = art::snake(vec2(700, 550), vec2(740, 960), 640, #{ head_angle: -PI() / 2.0, max_width: 48, seed: 2 });
art.add(snake.outline, LINE);
art.add(snake.spine.fill_with_text("lorem ipsum dolor sit amet", #{ font: PLAIN, size: 22, style: new_style(RED, PEN) }));

// A bramble: canes, leaves and flowers, stroked separately, each part in
// the style named for it.
art.add(caption("art::bramble(base, length, #{ angle, flower_size, seed }), canvas.add(parts, #{ vines, leaves, flowers })", 1010));
// path.subpaths() splits a path at each move, so each piece of the
// flowers can have a style of its own: here, red and blue by turns.
let plant = art::bramble(vec2(600, 1450), 400, #{ flower_size: 80, seed: 4 });
art.add(plant.vines, BRAMBLE.vines);
art.add(plant.leaves, BRAMBLE.leaves);
for (piece, i) in plant.flowers.subpaths() {
	art.add(piece, new_style(if i % 2 == 0 { RED } else { BLUE }, PEN));
}
art.add(write("plant.flowers.subpaths(), red and blue by turns", 380, 1500, 20));
book.bind("art", art);

// Along a path: a maze grown with brambles, and made a many-headed serpent.
let art = sheet("art");
art.add(caption("art::bramble_along a maze", TOP));
// Scaled to fit half the page, from its bounds.
let maze_lines = new_maze(4, 3, #{ seed: 5 }).path();
maze_lines.scale_by(780 / maze_lines.bounds().width);
art.add(art::bramble_along(maze_lines.copy().translate_by(vec2(LEFT + 110, TOP + 90)), 50, #{ seed: 2 }), BRAMBLE);
art.add(caption("art::snake_along the same maze", 860));
art.add(art::snake_along(maze_lines.copy().translate_by(vec2(LEFT + 110, 940)), 40, #{ seed: 2 }), LINE);
book.bind("art", art);

// Eyes as sketched in a journal: art::eye(center, length, #{ gaze_angle,
// messiness, seed }) gives its parts, #{ lines, accent, wash, rim }, and
// iris_radius, to size the wash's brushes by. Messiness runs from 0 (each
// line once, true) to 1 (gone over and over, overshooting); the gaze angle
// is on the page, in radians.
let art = sheet("art");
art.add(caption("art::eye(center, length, #{ gaze_angle, messiness, seed }): messiness 0, 0.25, 0.5, 0.75, 1", TOP));
for (m, i) in [0.0, 0.25, 0.5, 0.75, 1.0] {
	art.add(inked_eye(art::eye(vec2(LEFT + 95 + i * 205, TOP + 110), 150, #{ gaze_angle: -PI() / 4.0, messiness: m, seed: 3 + i }), BLUE, LINE));
}
art.add(caption("gaze angles: right, down, left, up; the same seed looking each way", TOP + 220));
for (g, i) in [0.0, PI() / 2.0, PI(), -PI() / 2.0] {
	art.add(inked_eye(art::eye(vec2(LEFT + 115 + i * 255, TOP + 330), 200, #{ gaze_angle: g, messiness: 0.4, seed: 11 }), BLUE, LINE));
}
// Options: angle tilts the eye, gaze is how far it looks (0 to 1),
// openness is its height over its length, and flat draws its lids as
// straight lines meeting above and below the middle (as the symbols
// font's eye has). Where the lids cover the iris, it's hidden. The last
// draws only some parts: its lines, and its wash in red.
art.add(caption("#{ angle, gaze, openness, flat }; eye.wash and eye.lines alone", TOP + 440));
art.add(inked_eye(art::eye(vec2(LEFT + 105, TOP + 610), 200, #{ gaze_angle: 0.0, messiness: 0.5, seed: 4, angle: -0.5, gaze: 0.0 }), BLUE, LINE));
art.add(inked_eye(art::eye(vec2(LEFT + 370, TOP + 610), 200, #{ gaze_angle: PI(), messiness: 0.5, seed: 4, gaze: 1.0, openness: 0.22 }), BLUE, LINE));
art.add(inked_eye(art::eye(vec2(LEFT + 635, TOP + 610), 200, #{ gaze_angle: PI() / 4.0, messiness: 0.4, seed: 6, flat: true, openness: 0.45 }), BLUE, LINE));
let red_eye = art::eye(vec2(LEFT + 890, TOP + 610), 200, #{ gaze_angle: 0.3, messiness: 0.6, seed: 4, angle: 1.3, openness: 0.5 });
art.add(red_eye.wash, new_style(RED, new_brush("Smooth Watercolor", 0.2, 0.35, red_eye.iris_radius * 0.3, #{ pressure_profile: "taper" })));
art.add(red_eye.lines, new_style(INK, PEN));
art.add(write("angle -0.5, gaze 0", LEFT + 10, TOP + 790, 20));
art.add(write("gaze 1, openness 0.22", LEFT + 265, TOP + 790, 20));
art.add(write("flat, openness 0.45", LEFT + 530, TOP + 790, 20));
art.add(write("angle 1.3, lines, wash", LEFT + 790, TOP + 790, 20));
// A crowd of them, overlapping as they come.
art.add(caption("a page of eyes, messy and overlapping", TOP + 850));
seed(5);
for i in 0..10 {
	let at = vec2(LEFT + 110 + (i % 5) * 195 + random_range(-30, 30), TOP + 980 + (i / 5) * 190 + random_range(-30, 30));
	let colour = if i % 4 == 3 { RED } else { BLUE };
	art.add(inked_eye(art::eye(at, random_range(150, 240), #{ gaze_angle: random_range(-PI(), PI()), messiness: random_range(0.5, 1.0), seed: i, angle: random_range(-1.0, 1.0), gaze: random(), flat: i % 3 == 1 }), colour, LINE));
}
book.bind("art", art);

// The moon in a phase: art::moon(center, radius, phase) gives #{ outline,
// terminator, lit, dark }, the sides closed to hatch or fill. The phase
// runs from 0 (new) through 0.25 (first quarter, right half lit), 0.5
// (full) and 0.75 (last quarter) back to new at 1, and wraps.
let art = sheet("art");
art.add(caption("art::moon(center, radius, phase): phase 0 to 1 by 0.125, the dark side hatched", TOP));
for i in 0..9 {
	let phase = i * 0.125;
	let at = vec2(LEFT + 55 + i * 111, TOP + 110);
	let moon = art::moon(at, 45, phase);
	art.add(moon, #{ outline: LINE, terminator: LINE });
	if !moon.dark.is_empty { art.add(moon.dark.hatch(5.0, #{ angle: 0.8, roughness: 0.5 }), LINE); }
	art.add(write(`${phase}`, at.x - 22, TOP + 190, 20));
}
art.add(caption("moon.lit hatched in red, moon.dark cross-hatched; phases wrap: 1.3 is 0.3, -0.3 is 0.7", TOP + 260));
for (phase, i) in [1.3, -0.3] {
	let moon = art::moon(vec2(LEFT + 250 + i * 500, TOP + 480), 160, phase);
	art.add(moon.outline, LINE);
	art.add(moon.terminator, LINE);
	art.add(moon.lit.hatch(9.0, #{ angle: 0.5, roughness: 1.0 }), CAPTION);
	art.add(moon.dark.hatch(6.0, #{ angle: [0.8, 2.4], roughness: 1.0 }), LINE);
	art.add(write(`phase ${phase}`, LEFT + 190 + i * 500, TOP + 680, 22));
}
// A month of them, a day a moon.
art.add(caption("a month: art::moon(at, 34, day / 29.5), the dark side hatched", TOP + 740));
for day in 0..30 {
	let at = vec2(LEFT + 70 + (day % 6) * 172, TOP + 820 + (day / 6) * 100);
	let moon = art::moon(at, 34, day / 29.5);
	art.add(moon.outline, LINE);
	if !moon.dark.is_empty { art.add(moon.dark.hatch(4.0, #{ angle: 0.8, roughness: 0.4 }), LINE); }
	art.add(write(`${day}`, at.x + 48, at.y + 8, 20));
}
book.bind("art", art);

// Cutting paths by a closed shape: erase_inside takes away what the shape
// covers (lines behind a figure), erase_outside what it doesn't. The
// shape's subpaths close as for hatching, with the same fill rules, and
// curves are cut as curves.
let art = sheet("art");
art.add(caption("path.erase_inside(shape): lines behind a disc; path.erase_outside(shape, #{ fill_rule })", TOP));
let rays = new_path();
for i in 0..18 {
	let y = TOP + 80 + i * 22;
	rays.append(wave(LEFT, LEFT + 440, y, 8));
}
let disc = new_circle(vec2(LEFT + 220, TOP + 270), 120);
art.add(rays.erase_inside(disc), LINE);
art.add(disc, CAPTION);
// A star drawn in one line: with evenodd its middle is outside, so the
// waves inside the star keep to its points.
let star = new_polygon(vec2(LEFT + 760, TOP + 270), 190, 5, #{ step: 2 });
let waves = new_path();
for i in 0..18 {
	waves.append(wave(LEFT + 560, LEFT + 960, TOP + 80 + i * 22, 8));
}
art.add(waves.erase_outside(star, #{ fill_rule: "evenodd" }), new_style(BLUE, PEN));
art.add(star, CAPTION);

// Brambles grow thicker toward an origin, as old wood is: canes thickness
// times as wide there, easing to their usual width reach away.
art.add(caption("art::bramble_along(path, flower_size, #{ seed, origin, thickness, reach }): the origin ringed", TOP + 560));
let maze_lines = new_maze(5, 3, #{ seed: 8 }).path();
maze_lines.scale_by(900 / maze_lines.bounds().width);
maze_lines.translate_by(vec2(LEFT + 50, TOP + 720));
let origin = maze_lines.bounds().min;
let grown = art::bramble_along(maze_lines, 45, #{ seed: 6, origin: origin, thickness: 10, reach: 1000 });
art.add(grown, BRAMBLE);
art.add(new_circle(origin, 10), CAPTION);
book.bind("art", art);

// Huffman trees as star charts: art::huffman_stars(symbols, output_base,
// center, width, height, options) charts the code the huffman lock would
// make, rewriting the symbols in output_base: its nodes are stars, bigger
// the commoner their symbols, its branches the links. The tree is grown
// from its root at random angles and lengths, then tidied by forces, each
// star with a space and a pull of its own and each link a length of its
// own; links never cross. It gives #{ stars, links } as paths, points and
// edges (as new_edges takes them) and nodes, each star's #{ position,
// radius, weight, depth, code, symbol, text, parent }.
let art = sheet("art");
let pangram = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG";
art.add(caption("art::huffman_stars(symbols(27, TEXT), 3, center, width, height, #{ seed }): letters with their codes", TOP));
art.add(caption(`${pangram}, base 27 to base 3; stars hatched, leaves in red`, TOP + 34));
let chart = art::huffman_stars(symbols(27, pangram), 3, vec2(560, TOP + 340), 880, 520, #{ seed: 7 });
art.add(chart.links, LINE);
for node in chart.nodes {
	let style = if node.symbol == () { LINE } else { CAPTION };
	let star = new_circle(node.position, node.radius);
	art.add(star, style);
	art.add(star.hatch(2.5, #{ angle: 0.7 }), style);
	if node.symbol != () {
		let name = if node.text == " " { "SPACE" } else { node.text };
		art.add(write_in(`${name} ${node.code.reduce(|t, d| t + d, "")}`, node.position.x + node.radius + 8, node.position.y, 18, CAPTION, PLAIN));
	}
}
// Its options, a panel each, all from the same pangram in base 2.
let panels = [
	["#{ smoothing: 0, round: true }", #{ smoothing: 0, round: true }],
	["#{ iterations: 0 }: as grown, untidied", #{ iterations: 0 }],
	["#{ variation: 0 }: stars and links alike", #{ variation: 0 }],
	["#{ variation: 1, star_size: 6, gap: 0 }, new_edges(points, edges)", #{ variation: 1, star_size: 6, gap: 0 }],
];
for (panel, i) in panels {
	let x = LEFT + 245 + (i % 2) * 510;
	let y = TOP + 680 + (i / 2) * 330;
	art.add(write_in(panel[0], x - 245, y, 18, CAPTION, PLAIN));
	let options = panel[1];
	options.seed = 4;
	let small = art::huffman_stars(symbols(27, pangram), 2, vec2(x, y + 155), 490, 260, options);
	if i == 3 {
		// The same chart from its points and edges, links meeting at the stars.
		art.add(new_edges(small.points, small.edges), LINE);
		art.add(small.stars, CAPTION);
	} else {
		art.add(small, #{ stars: CAPTION, links: LINE });
	}
}
book.bind("art", art);

// A chart cut into constellations, a page each: art::constellations(chart,
// count, center, width, height, options) cuts its tree into count pieces
// that hold together, neighbours sharing the star they meet at, fits each
// to the box on its own and finds each star a spot for its label clear of
// the links, the stars and the other labels. Each piece is #{ stars (the
// chart's indices), points, pairs (links, into points), labels, scale }.
let art = sheet("art");
art.add(caption("art::constellations(chart, 4, center, width, height, #{ label_width, label_height, label_gap })", TOP));
art.add(caption("the pangram's chart above in four pieces, each fitted to a 470 by 520 box, stars numbered at their labels", TOP + 34));
let sky = art::huffman_stars(symbols(27, "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG"), 3, vec2(0, 0), 2000, 1400, #{ seed: 7, star_size: 12 });
for (piece, i) in art::constellations(sky, 4, vec2(0, 0), 470, 520, #{ label_width: 24, label_height: 24, label_gap: 6 }) {
	let at = vec2(LEFT + 250 + (i % 2) * 510, TOP + 380 + (i / 2) * 600);
	art.add(new_edges(piece.points, piece.pairs).translate_by(at), LINE);
	for (s, k) in piece.stars {
		art.add(new_circle(piece.points[k] + at, sky.nodes[s].radius), if sky.nodes[s].symbol == () { LINE } else { CAPTION });
		art.add(write_in(`${k}`, piece.labels[k].x + at.x - 6, piece.labels[k].y + at.y, 18, CAPTION, PLAIN));
	}
}
book.bind("art", art);

// #{ split_at: [stars] } splits the constellation holding each star in two
// at it, its branches either side: here the root's, left and right.
let art = sheet("art");
art.add(caption("art::constellations(chart, 4, center, width, height, #{ split_at: [0] }): the root's piece split left and right", TOP));
let split = art::constellations(sky, 4, vec2(0, 0), 470, 520, #{ label_width: 24, label_height: 24, label_gap: 6, split_at: [0] });
art.add(caption(`${split.len()} pieces; those holding the root, star 0, below, the root ringed`, TOP + 34));
let shown = 0;
for piece in split {
	if !piece.stars.contains(0) { continue; }
	let at = vec2(LEFT + 250 + (shown % 2) * 510, TOP + 380 + (shown / 2) * 600);
	art.add(new_edges(piece.points, piece.pairs).translate_by(at), LINE);
	for (s, k) in piece.stars {
		art.add(new_circle(piece.points[k] + at, sky.nodes[s].radius), if sky.nodes[s].symbol == () { LINE } else { CAPTION });
		if s == 0 { art.add(new_circle(piece.points[k] + at, sky.nodes[s].radius + 10), CAPTION); }
	}
	shown += 1;
}
book.bind("art", art);

// ---- generators: scattered points, trees, digits ----

// distribute_points_2/3/4(kind, count, [[low, high], ...]) scatters count
// points through a box, giving vec2s, vec3s or vec4s; the random kinds
// draw from seed(). Each panel is the same box and count.
let gen = sheet("generators");
gen.add(caption("distribute_points(kind, 120, [[low, high], [low, high]])", TOP));
seed(8);
for (kind, i) in ["uniform_random", "poisson_disk", "jittered_grid", "grid", "halton", "golden"] {
	let x0 = LEFT + (i % 3) * 345;
	let y0 = TOP + 60 + (i / 3) * 320;
	gen.add(new_rectangle(vec2(x0 + 150, y0 + 115), 310, 240), new_style(RED, new_brush("Ink Pen", 0.2, 0.4, 1)));
	for p in distribute_points(kind, 120, [[x0 + 5, x0 + 295], [y0 + 5, y0 + 225]]) {
		gen.add(new_circle(p, 3), LINE);
	}
	gen.add(write(kind, x0, y0 + 265, 22));
}

// vec3 and vec4 work as vec2s do, and drop to fewer coordinates with
// xy() and xyz(); vec3s cross. Here points in a box of depth, drawn
// smaller and fainter the deeper they are, and a vec4's colour.
let y = TOP + 720;
gen.add(caption("vec3(x, y, z), vec4(x, y, z, w), vec3(v2, z), vec4(v3, w); .z, .w, xy(), xyz(), cross", y));
for p in distribute_points("poisson_disk", 140, [[LEFT, LEFT + 460], [y + 50, y + 330], [0, 1]]) {
	gen.add(new_circle(p.xy(), 2 + 6 * (1 - p.z)), if p.z < 0.5 { LINE } else { new_style(BLUE, PEN) });
}
let up = vec3(1, 0, 0).cross(vec3(0, 1, 0));
let lifted = vec4(vec3(vec2(0.1, 0.2), up.z), 0.5);
assert(up == vec3(0, 0, 1) && lifted.xyz().z == 1.0 && lifted.w == 0.5 && vec4(1, 2, 3, 4).length() == 30.0.sqrt(), "vec3 and vec4 should measure");
for c in distribute_points("halton", 40, [[LEFT + 560, RIGHT - 20], [y + 50, y + 330], [0, 255], [0.3, 1]]) {
	let colour = `rgba(${(c.z).to_int()}, 40, ${(255 - c.z).to_int()}, ${c.w})`;
	gen.add(new_circle(c.xy(), 12), new_style(colour, PEN));
}

book.bind("generators", gen);

// points.spanning_tree(#{ algorithm, metric, root, neighbours }) joins
// every point, giving [parent, child] index pairs from the root out, in
// the order they were added; new_edges(points, pairs) draws them. The
// graph algorithms walk each point's nearest neighbours.
let gen = sheet("generators");
gen.add(caption("points.spanning_tree(#{ algorithm, root: nearest(points, the middle) }), drawn as new_edges(points, pairs)", TOP));
seed(3);
let points = distribute_points("poisson_disk", 110, [[0, 290], [0, 220]]);
let middle = nearest(points, vec2(145, 110));
for (algorithm, i) in ["prim", "kruskal", "maximum", "shortest_path", "breadth_first", "depth_first", "random"] {
	let at = vec2(LEFT + (i % 3) * 345, TOP + 60 + (i / 3) * 300);
	let edges = points.spanning_tree(#{ algorithm: algorithm, root: middle });
	gen.add(new_edges(points, edges).translate_by(at), if algorithm == "maximum" { new_style(INK, new_brush("Ink Pen", 0.2, 0.4, 1), #{ width: 0.5 }) } else { LINE });
	gen.add(new_circle(points[middle] + at, 6), CAPTION);
	gen.add(write(algorithm, at.x, at.y + 250, 22));
}
assert(points.spanning_tree() == points.spanning_tree(#{ algorithm: "prim", metric: "euclidean", root: 0 }), "prim and euclidean from the first point by default");

// The metric decides what's near: shortest paths over each.
let y = TOP + 960;
gen.add(caption("#{ algorithm: \"shortest_path\", metric, neighbours: 8 }, by each metric", y));
for (metric, i) in ["euclidean", "squared_euclidean", "manhattan", "chebyshev"] {
	let at = vec2(LEFT + i * 255, y + 50);
	let small = [];
	for p in points { small.push(p * 0.8); }
	gen.add(new_edges(small, small.spanning_tree(#{ algorithm: "shortest_path", metric: metric, root: middle, neighbours: 8 })).translate_by(at), new_style(BLUE, PEN));
	gen.add(write(metric, at.x, at.y + 205, 20));
}
book.bind("generators", gen);

// balanced_ternary(n) is n in balanced ternary: digits -1, 0 and 1, most
// significant first, each worth three times the next. It's made from a
// whole number, "+-0" text or an array of digits; .value reads it back,
// .digits() gives the digits and .digits(width) them in chunks of width,
// padded in front. Three digits make a number from -13 to 13, drawn here
// as its glyph in the balanced_ternary font, which was made for them.
let gen = sheet("generators");
let ternary = font("balanced_ternary");
gen.add(caption("balanced_ternary(n), n from -13 to 13, drawn as font(\"balanced_ternary\").glyph(balanced_ternary(n))", TOP));
for n in -13..=13 {
	let x = LEFT + 18 + (n + 13) * 36.5;
	let bt = balanced_ternary(n);
	assert(bt.value == n && balanced_ternary(bt.digits()) == bt && balanced_ternary(`${bt}`) == bt, "balanced ternary should read back");
	gen.add(ternary.glyph(bt, #{ scale: 32, position: vec2(x, TOP + 100) }), SAMPLE);
	gen.add(write(`${n}`, x - 10, TOP + 160, 16));
}
// Longer numbers, three digits to a glyph.
let year = balanced_ternary(2026);
gen.add(caption(`balanced_ternary(2026): ${year}; .digits(3): ${year.digits(3)}, a glyph each`, TOP + 230));
for (chunk, i) in year.digits(3) {
	gen.add(ternary.glyph(balanced_ternary(chunk), #{ scale: 48, position: vec2(LEFT + 40 + i * 60, TOP + 310) }), SAMPLE);
}

// Angles are radians; degrees(d) gives d degrees in radians, and
// radians(r) says a number is radians already.
let y = TOP + 380;
gen.add(caption("degrees(d), radians(r): a ray every degrees(30), and every radians(PI() / 12)", y));
let hub = vec2(LEFT + 160, y + 160);
for i in 0..12 {
	gen.add(new_line(hub, hub + vec2(110, 0).rotated(i * degrees(30))), LINE);
}
let hub = vec2(LEFT + 560, y + 160);
for i in 0..24 {
	gen.add(new_line(hub, hub + vec2(110, 0).rotated(i * radians(PI() / 12))), new_style(BLUE, PEN));
}
assert((degrees(180) - PI()).abs() < 1e-12, "degrees should be radians");

// Balanced ternary along a path, as the font writes it down its stems:
// a 0 a bar across, a -1 or +1 a stroke bending sharply off to the
// traveller's left or right, marks evenly spaced by where they leave the
// path. fit_text spreads the digits over the whole path; fill_with_text
// sets them size apart, over as many copies of the path as it takes. The
// path is drawn too.
let y = TOP + 700;
let nine = balanced_ternary(year.digits(9)[0]);
gen.add(caption(`path.fit_text(balanced_ternary(${nine}), #{ style }) down a stem, and #{ style, width: 32 } along a wave`, y));
let stem = new_line(vec2(LEFT + 30, y + 40), vec2(LEFT + 30, y + 280));
gen.add(stem, LINE);
gen.add(stem.fit_text(nine, #{ style: LINE }));
let path = wave(LEFT + 140, RIGHT - 20, y + 160, 70);
gen.add(path, LINE);
gen.add(path.fit_text(nine, #{ style: LINE, width: 32 }));

// Digits spaced closer than the path's length needs, and over copies of
// a path too short for them.
let y = TOP + 1020;
gen.add(caption("fit_text(balanced_ternary(\"+-0+-0+-0\")), and fill_with_text(bt, #{ style, size: 30, width: 14 }) round an arc", y));
let path = wave(LEFT + 20, RIGHT - 380, y + 130, 60);
gen.add(path, LINE);
gen.add(path.fit_text(balanced_ternary("+-0+-0+-0"), #{ style: LINE }));
let arc = new_arc(vec2(RIGHT - 160, y + 150), 110, 3.4, 6.0);
gen.add(arc, LINE);
gen.add(arc.fill_with_text(nine, #{ style: LINE, size: 30, width: 14 })[0]);
book.bind("generators", gen);

// ---- text ----

let text = sheet("text");

// A text box, flat (glyph by glyph) with jagged edges.
text.add(caption("new_textbox: flat, with jagged edges", TOP));
let flat = new_textbox(#{ center: vec2(340, TOP + 190), width: 460, height: 300, size: 22, jagged: 0.6 });
text.add(flat.fill(PLAIN.glyphs(LOREM)).placed, TEXT);

// Words kept whole, spaced out, spread over as many boxes as it takes.
text.add(caption("words, spacing, place_all", TOP).translate_by(vec2(540, 0)));
let boxes = new_textbox(#{ center: vec2(0, 0), width: 210, height: 300, size: 20, layout: "words", spacing: 0.05 });
for (block, i) in boxes.place_all(PLAIN.glyphs(LOREM)) {
	text.add(block.translate_by(vec2(760 + i * 230, TOP + 190)), TEXT);
}

// Text kept clear of a drawing: of its lines, or (add_hull) of everything
// inside its outline.
text.add(caption("avoid a drawing: its lines, or tree.add_hull(drawing, #{ clearance })", 600));
let plant = art::bramble(vec2(330, 1060), 380, #{ angle: -PI() / 2.0, flower_size: 60, seed: 9 });
let drawing = new_canvas().add(plant, BRAMBLE);
let b = drawing.bounds();
let lines_box = new_textbox(#{ center: b.center, width: 460, height: b.height + 40, size: 16, layout: "words", avoid: drawing, clearance: 3 });
text.add(drawing.copy());
text.add(lines_box.fill(PLAIN.glyphs(LOREM + LOREM)).placed, TEXT);
let hull_tree = new_avoidance_tree().add_hull(drawing, #{ clearance: 8 });
assert(!hull_tree.is_empty && hull_tree.blocks(b.center), "the hull should cover the drawing's middle");
let shifted = drawing.copy().translate_by(vec2(520, 0));
text.add(shifted);
let hull_box = new_textbox(#{ center: b.center + vec2(520, 0), width: 460, height: b.height + 40, size: 16, layout: "words", avoid: new_avoidance_tree().add_hull(shifted, #{ clearance: 8 }) });
text.add(hull_box.fill(PLAIN.glyphs(LOREM + LOREM)).placed, TEXT);
book.bind("text", text);

let text = sheet("text");
// Text along paths: as many copies of a circle as it takes, fitted to an
// arc, and along a spiral that keeps out of a ring and off a line, words
// whole.
text.add(caption("fill_with_text round a circle, as many times as it takes", TOP));
let ring_text = new_circle(vec2(170, TOP + 130), 60).fill_with_text(LOREM, #{ font: PLAIN, size: 16, style: new_style(GOLD, PEN) });
for (part, i) in ring_text {
	text.add(part.translate_by(vec2((i % 6) * 150, (i / 6) * 150)));
}
text.add(caption("fit_text: as large as fits the arc", 560));
text.add(new_arc(vec2(320, 760), 170, 3.4, 6.0).fit_text("lorem ipsum", #{ font: PLAIN, style: new_style(BLUE, PEN) }));
text.add(caption("along a spiral, avoiding a ring and a line", 560).translate_by(vec2(540, 0)));
let centre = vec2(840, 780);
let turns = [centre];
for i in 1..360 {
	let a = i * 0.05;
	turns.push(centre + vec2(15 + a * 9, 0).rotated(a));
}
let spiral = new_polyline(turns);
let keep_out = new_avoidance_tree();
keep_out.add_region(new_circle(centre + vec2(50, -40), 45));
keep_out.add(new_line(centre + vec2(-120, -150), centre + vec2(140, 150)), #{ clearance: 4 });
text.add(new_circle(centre + vec2(50, -40), 45), CAPTION);
text.add(new_line(centre + vec2(-120, -150), centre + vec2(140, 150)), CAPTION);
text.add(spiral.fill_with_text(LOREM, #{ font: PLAIN, size: 14, style: new_style(GREEN, PEN), avoid: keep_out, layout: "words" })[0]);

// Notes scattered over a drawing, clear of it, of a line across it and of
// each other, words kept whole.
text.add(caption("scatter_notes(notes, width_range, height_range, settings), clear of a drawing, a line and each other", 1030));
let board = new_canvas();
board.add(new_rectangle(vec2(600, 1250), 1000, 360), CAPTION);
board.add(art::snake(vec2(400, 1150), vec2(720, 1330), 900, #{ head_angle: 0, max_width: 40, seed: 3 }).outline, LINE);
text.add(board);
text.add(board.scatter_notes(["lorem ipsum", "dolor sit amet", "consectetur adipiscing", "sed do eiusmod", "tempor incididunt"], [240, 340], [25, 35], #{ font: PLAIN, style: new_style(GOLD, PEN), size: 20, clearance: 24, layout: "words", avoid: new_line(vec2(100, 1250), vec2(1100, 1250)) }));
book.bind("text", text);

// Words round a drawing: whole, moved past it (the default), broken
// round it with split_words, and with split_padding to keep the pieces
// further off. split_words: n is the shortest piece allowed.
let text = sheet("text");
text.add(subhead("splitting words", TOP));
text.add(caption("layout: \"words\", avoid; then split_words: true; then split_padding: 0.6", TOP + 50));
fn seal(c) {
	new_canvas()
		.add(new_circle(c, 78), global::CAPTION)
		.add(new_polygon(c, 70, 5, #{ step: 2 }), global::CAPTION)
}
for (options, i) in [#{}, #{ split_words: true }, #{ split_words: true, split_padding: 0.6 }] {
	let c = vec2(LEFT + 160 + i * 340, TOP + 330);
	let drawing = seal(c + vec2(-50, -60));
	text.add(drawing);
	let box = #{ center: c, width: 320, height: 420, size: 17, layout: "words", avoid: drawing, clearance: 4 };
	box += options;
	text.add(new_textbox(box).fill(PLAIN.glyphs(LOREM + LOREM)).placed, TEXT);
}
// Along a path too, and in layouts (a role's split_words and
// split_padding: see the inline layout).
let y = TOP + 680;
text.add(caption("fill_with_text along a line past three circles: whole words, then split_words: 2", y));
for (options, i) in [#{ layout: "words" }, #{ layout: "words", split_words: 2 }] {
	let line_y = y + 80 + i * 110;
	let rings = new_avoidance_tree();
	for k in 0..3 {
		let at = vec2(LEFT + 210 + k * 300, line_y);
		text.add(new_circle(at, 30), CAPTION);
		rings.add_region(new_circle(at, 30), #{ clearance: 6 });
	}
	options.avoid = rings;
	text.add(new_line(vec2(LEFT, line_y), vec2(RIGHT, line_y)).fill_with_text(LOREM, #{ font: PLAIN, size: 22, style: TEXT } + options)[0]);
}
// Glyphs held at an angle, rather than turned to the path: set along it,
// as close as their outlines at that angle allow.
y += 340;
text.add(caption("along a curve, turned to it; then #{ angle: 0 }, upright; then #{ angle: 0.3 }", y));
for (angle, i) in [(), 0, 0.3] {
	let x = LEFT + i * 340;
	let path = new_path().move_to(vec2(x, y + 120)).cubic_to(vec2(x + 110, y + 40), vec2(x + 210, y + 200), vec2(x + 320, y + 120));
	text.add(path, CAPTION);
	let options = if angle == () { #{} } else { #{ angle: angle } };
	text.add(path.fill_with_text("lorem ipsum dolor sit", #{ font: PLAIN, size: 28, style: TEXT } + options)[0]);
}
book.bind("text", text);

// Text boxes of any closed shape: #{ shape: path } in place of center,
// width and height. Each subpath is an outline, closed back to its start,
// and one inside another is a hole. #{ angle } turns the lines (radians;
// lines follow on to their right). A rectangle at angle 0 is a plain box.
let text = sheet("text");
text.add(subhead("shaped text boxes", TOP));
text.add(caption("new_textbox #{ shape }: a circle; a ring (a circle in a circle); a star, new_star(center, outer, inner, points)", TOP + 50));
let y = TOP + 250;
let shapes = [
	new_circle(vec2(LEFT + 150, y), 150),
	new_circle(vec2(LEFT + 500, y), 150).append(new_circle(vec2(LEFT + 500, y), 60)),
	new_star(vec2(LEFT + 850, y + 10), 170, 80, 5),
];
for shape in shapes {
	text.add(shape, CAPTION);
	text.add(new_textbox(#{ shape: shape, size: 16 }).fill(PLAIN.glyphs(LOREM + LOREM)).placed, TEXT);
}
text.add(caption("#{ angle }: 0.4 in a square; -PI() / 4 in a circle; PI() / 2, lines down the page", TOP + 470));
let y = TOP + 680;
for (options, i) in [
	#{ shape: new_rectangle(vec2(LEFT + 150, y), 280, 280), angle: 0.4 },
	#{ shape: new_circle(vec2(LEFT + 500, y), 150), angle: -PI() / 4.0, layout: "words" },
	#{ shape: new_rectangle(vec2(LEFT + 850, y), 280, 280), angle: PI() / 2.0, jagged: 0.8 },
] {
	text.add(options.shape, CAPTION);
	options.size = 16;
	text.add(new_textbox(options).fill(PLAIN.glyphs(LOREM + LOREM + LOREM)).placed, TEXT);
}
// An oval round a flat-lidded eye, words kept whole and clear of it.
text.add(caption("an oval, words at an angle of 0.15, avoiding an eye", TOP + 900));
let centre = vec2(W / 2.0, TOP + 1100);
let oval = new_ellipse(centre, 480, 170);
let eye = inked_eye(art::eye(centre, 260, #{ gaze_angle: 0.0, messiness: 0.3, seed: 2, flat: true, openness: 0.4, gaze: 0.0 }), BLUE, LINE);
text.add(eye);
text.add(oval, CAPTION);
text.add(new_textbox(#{ shape: oval, angle: 0.15, size: 18, layout: "words", avoid: eye, clearance: 10 }).fill(PLAIN.glyphs(LOREM + LOREM + LOREM)).placed, TEXT);
book.bind("text", text);

// Lines aligned in a text box: set from their start, then moved along as
// far as their glyphs' ink still fits the shape (all the way, or half way),
// or spread to meet both ends, all but the text's last line.
let text = sheet("text");
text.add(subhead("aligned text boxes", TOP));
text.add(caption("new_textbox #{ align }: left, center, right and justify, in circles", TOP + 50));
let y = TOP + 270;
for (align, i) in ["left", "center", "right", "justify"] {
	let at = vec2(LEFT + 125 + i * 255, y);
	let circle = new_circle(at, 115);
	text.add(circle, CAPTION);
	text.add(new_textbox(#{ shape: circle, size: 26, layout: "words", align: align }).fill(PLAIN.glyphs(LOREM)).placed, TEXT);
	text.add(write_in(align, at.x - 40, y + 140, 22, CAPTION, PLAIN));
}
text.add(caption("justify, words kept whole, a ring; flat, a star: ink meets the edges", TOP + 520));
let y = TOP + 760;
let ring = new_circle(vec2(LEFT + 250, y), 200).append(new_circle(vec2(LEFT + 250, y), 80));
text.add(ring, CAPTION);
text.add(new_textbox(#{ shape: ring, size: 18, layout: "words", align: "justify" }).fill(PLAIN.glyphs(LOREM + LOREM)).placed, TEXT);
let star = new_star(vec2(LEFT + 750, y), 210, 100, 6);
text.add(star, CAPTION);
text.add(new_textbox(#{ shape: star, size: 16, align: "justify" }).fill(PLAIN.glyphs(LOREM + LOREM)).placed, TEXT);
text.add(caption("new_textbox #{ valign }: top, center and bottom, centred lines", TOP + 1000));
let y = TOP + 1140;
for (valign, i) in ["top", "center", "bottom"] {
	let circle = new_circle(vec2(LEFT + 150 + i * 350, y), 110);
	text.add(circle, CAPTION);
	text.add(new_textbox(#{ shape: circle, size: 26, layout: "words", align: "center", valign: valign }).fill(PLAIN.glyphs("Lorem ipsum dolor sit amet, consectetur adipiscing elit.")).placed, TEXT);
}
book.bind("text", text);

// Text written as a hand writes it: a line from a point, and a block
// wrapped to a width, line after line, giving where the next text goes.
// Every text call takes a role's settings (layouts' role keys, so a role
// can be passed straight in; keys a call doesn't use are ignored), and
// every one, roles too, can set each glyph a little off true.
let text = sheet("text");
text.add(subhead("writing by hand", TOP));
text.add(caption("write_line(text, at, #{ font, size, style })", TOP + 50));
let BODY = #{ font: PLAIN, size: 26, style: TEXT, line_height: 1.4 };
text.add(write_line("Lorem ipsum dolor sit amet, consectetur adipiscing elit.", vec2(LEFT, TOP + 110), BODY));
text.add(caption("write_block(text, at, width, settings): #{ canvas, bottom }, with line_height, jagged and line_jitter (right)", TOP + 170));
let block = write_block(LOREM, vec2(LEFT, TOP + 230), 460, BODY + #{ jagged: 0.4 });
text.add(block.canvas);
text.add(new_line(vec2(LEFT, block.bottom), vec2(LEFT + 460, block.bottom)), CAPTION);
text.add(write("bottom", LEFT + 470, block.bottom, 20));
let narrow = write_block(LOREM, vec2(LEFT + 560, TOP + 230), 440, BODY + #{ font: LOCHE, size: 22, style: new_style(RED, PEN), layout: "flat", line_jitter: 0.25 });
text.add(narrow.canvas);
let y = block.bottom.max(narrow.bottom) + 60;
text.add(caption("#{ position_jitter, rotation_jitter, scale_jitter }: each glyph a little off true", y));
y += 60;
for (jitter, i) in [#{}, #{ position_jitter: 0.06, rotation_jitter: 0.08, scale_jitter: 0.08 }, #{ position_jitter: [0.02, 0.15], rotation_jitter: 0.25 }] {
	text.add(write_line("lorem ipsum dolor sit amet", vec2(LEFT, y + i * 70), BODY + #{ font: LOCHE, size: 40 } + jitter));
}
y += 230;
text.add(caption("in a text box, along a path, and in scattered notes alike", y));
let jostled = #{ position_jitter: 0.03, rotation_jitter: 0.06, scale_jitter: 0.05 };
text.add(new_textbox(#{ center: vec2(LEFT + 230, y + 170), width: 460, height: 260, size: 22, layout: "words" } + jostled).fill(PLAIN.glyphs(LOREM)).placed, TEXT);
text.add(new_arc(vec2(820, y + 300), 200, 3.6, 5.8).fit_text("lorem ipsum dolor", BODY + jostled));
book.bind("text", text);

// Lines written by hand wander: write_line and write_block bend the line
// they write along by up to path_jitter sizes.
let text = sheet("text");
text.add(subhead("wandering lines", TOP));
text.add(caption("write_line(text, at, #{ path_jitter }): the line wanders from straight, 0, 0.15 and 0.3 sizes", TOP + 50));
for (jitter, i) in [0.0, 0.15, 0.3] {
	text.add(write_line("lorem ipsum dolor sit amet", vec2(LEFT, TOP + 120 + i * 70), BODY + #{ font: LOCHE, size: 40, path_jitter: jitter }));
}
let y = TOP + 360;
text.add(caption("write_block(text, at, width, #{ path_jitter }): each line its own way, 0.2 sizes; 0.25 and jagged (right)", y));
let wandering = write_block(LOREM, vec2(LEFT, y + 60), 460, BODY + #{ path_jitter: 0.2 });
text.add(wandering.canvas);
let loose = write_block(LOREM, vec2(LEFT + 560, y + 60), 440, BODY + #{ font: LOCHE, size: 22, style: new_style(RED, PEN), layout: "flat", path_jitter: 0.25, jagged: 0.4 });
text.add(loose.canvas);
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
	// A composite lock file: shift_cipher wired into vigenere_cipher.
	// Its shift is 3 unless an option says otherwise.
	["shift_vigenere (composite)", new_lock("composite").add("c", "shift_vigenere"), #{ "c.key": symbols(27, "KEY"), "c.plain_text": symbols(27, plain) }, "c.cipher_text"],
];
locks.add(caption("new_lock(name).add(part, lock type, options), run: " + plain + " becomes", TOP));
let y = TOP + 60;
for row in rows {
	let lock = row[1];
	let result = lock.run(row[2]);
	assert(result.get(row[3]).base >= 26 && result.get(row[3]).type_name != "", "value.base and value.type_name");
	locks.add(write(row[0], LEFT, y, 22));
	locks.add(write(result.text(row[3]), 450, y, 22));
	y += 52;
}

// Huffman codes: a corpus in one base sets the code, which rewrites text
// in another base, the corpus's commonest symbols getting the shortest
// codes. The corpus is passed through, for unlocking to rebuild the code.
y += 20;
let corpus = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG";
locks.add(caption("huffman (input_base 27, output_base 2, then 3): the letters commonest in the corpus", y));
locks.add(caption(`${corpus} get the shortest codes.`, y + 34));
locks.add(caption(`${plain}, letter by letter:`, y + 68));
y += 134;
let fine = new_style(INK, new_brush("Ink Pen", 0.3, 0.7, 1.2));
for base in [2, 3] {
	let huffman = new_lock("huffman").add("h", "huffman", #{ input_base: 27, output_base: base }).set_input("h.corpus", symbols(27, corpus));
	let result = huffman.run(#{ "h.plain_text": symbols(27, plain) });
	assert(huffman.unlock(result).text("h.plain_text") == plain, "huffman should unlock");
	let codes = [];
	for c in plain.chars() {
		let code = huffman.run(#{ "h.plain_text": symbols(27, c.to_string()) }).get("h.cipher_text").value();
		codes.push(code.reduce(|t, d| t + d, ""));
	}
	assert(codes.reduce(|t, c| t + c, "") == result.get("h.cipher_text").value().reduce(|t, d| t + d, ""), "codes should join up");
	locks.add(write_in(codes.reduce(|t, c| if t == "" { c } else { t + " " + c }, ""), LEFT, y, 22, fine, PLAIN));
	y += 52;
}

// A chain of locks, its result drawn as a maze of balanced-ternary glyphs,
// and unlocked again.
y += 30;
locks.add(caption("a chain of locks ending in a maze lock, drawn in balanced-ternary glyphs", y));
let chain = new_lock("chain")
	.add("sub", "substitution_cipher", #{ base_n: 27 })
	.add("vig", "vigenere_cipher", #{ key_base: 27, plain_base: 27, cipher_base: 27 })
	.add("maze", "maze_lock", #{ input_base: 27, strategy: "RightHanded" })
	.connect("sub.cipher_text", "vig.key")
	.connect("vig.cipher_text", "maze.maze_text")
	.set_input("sub.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS"))
	.set_input("maze.random_seed", symbols(10, "7"));
assert(chain.inputs().len() > 0 && chain.outputs().len() > 0, "a chain has open inputs and outputs");
let result = chain.run(#{ "sub.plain_text": symbols(27, "key"), "vig.plain_text": symbols(27, plain) });
assert(chain.unlock(result).text("vig.plain_text") == plain, "the chain didn't unlock");
assert(result.trace().len() > 0 && result.values().len() > 0 && result.get("maze.random_seed").symbols().len() > 0 && result.get("maze.random_seed").value() != (), "nothing traced");
// With every input set, a chain runs with nothing more; symbols(base,
// text, #{ lenient: true }) keeps what the base can write.
let fixed = new_lock("fixed").add("s", "shift_cipher", #{ base: 27, shift: 1 }).set_input("s.plain_text", symbols(27, "Every lock — opens!", #{ lenient: true }));
assert(fixed.run().text("s.cipher_text") == "FWFSZAMPDLAPQFOT", "a fixed chain runs, its text made fit");
let cell_size = 70;
let maze_canvas = new_canvas();
let grid = result.get("maze.generated_maze").maze();
maze_canvas.add(grid.path().scale_by(cell_size), new_style(RED, PEN));
let ternary = font("balanced_ternary");
for cell in grid.cells() {
	maze_canvas.add(ternary.glyph(cell.symbol - 13, #{ scale: cell_size * 0.8, position: vec2(cell.x * cell_size, cell.y * cell_size) }), SAMPLE);
}
// A maze can also be filled with symbols directly.
let blank = new_maze(2, 2, #{ seed: 3 });
blank.fill(symbols(27, "HELLO").symbols());
assert(blank.cells()[0].symbol == 8, "fill writes symbols in walking order");
locks.add(maze_canvas.translate_by(vec2(LEFT + 30, y + 80)));
book.bind("locks", locks);

// Value kinds: lock values that are plain data, tagged with their kind,
// checked against its shape wherever they cross into or out of a lock.
// Their functions are the same in journals and lock files, and a kind's
// sketch is how it's drawn by default: art::sketch(data), or value.sketch()
// for a lock result's value, gives its parts to canvas.add(parts, styles)
// and its labels to write.

// A kind's sketch fitted into the box at corner, w by h: the parts styles
// names, and the labels.
fn draw_sketch(sketch, styles, corner, w, h) {
	let lo = vec2(1e9, 1e9);
	let hi = vec2(-1e9, -1e9);
	for name in styles.keys() {
		let b = sketch[name].bounds();
		lo = vec2(if b.min.x < lo.x { b.min.x } else { lo.x }, if b.min.y < lo.y { b.min.y } else { lo.y });
		hi = vec2(if b.max.x > hi.x { b.max.x } else { hi.x }, if b.max.y > hi.y { b.max.y } else { hi.y });
	}
	let fx = w / (hi.x - lo.x + sketch.label_size * 2.0);
	let fy = h / (hi.y - lo.y + sketch.label_size * 2.0);
	let f = if fx < fy { fx } else { fy };
	let place = |p| (p - lo + vec2(sketch.label_size, sketch.label_size)) * f + corner;
	let c = new_canvas();
	for name in styles.keys() {
		c.add(sketch[name].copy().translate_by(vec2(sketch.label_size, sketch.label_size) - lo).scale_by(f).translate_by(corner), styles[name]);
	}
	let size = sketch.label_size * f;
	for label in sketch.labels {
		let at = place.call(label.at);
		// Sketches mark a space ·, which plain hasn't.
		let text = if label.text == "·" { "_" } else { label.text };
		c.add(write_in(text, at.x - size * 0.3 * text.len(), at.y, size, global::CAPTION, global::PLAIN));
	}
	c
}

let kinds = sheet("locks");
let corpus = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG";
let code = huffman::code(symbols(27, corpus), 3);
kinds.add(caption("huffman::code(symbols(27, corpus), 3): the corpus's HuffmanCode, as the huffman lock builds it;", TOP));
kinds.add(caption("art::sketch(code) draws its tree, each leaf's letter under it", TOP + 34));
assert(code.kind == "HuffmanCode" && code.nodes[0].parent == (), "a code is plain data, the root first");
kinds.add(draw_sketch(art::sketch(code), #{ links: LINE, nodes: LINE }, vec2(LEFT, TOP + 70), RIGHT - LEFT, 380));
// The functions star_chart is made of: the tree laid out, then cut into
// pieces fitted to a box.
kinds.add(caption("stars::layout(code, #{ seed }) then stars::split(points, edges, sizes, #{ count: 2, width, height })", TOP + 520));
let laid = stars::layout(code, #{ seed: 3 });
let edges = [];
for (node, k) in code.nodes { if node.parent != () { edges.push([node.parent, k]); } }
for (piece, i) in stars::split(laid.points, edges, laid.sizes, #{ count: 2, width: 470, height: 560 }) {
	let at = vec2(LEFT + i * 530, TOP + 580);
	let points = piece.points.map(|p| vec2(p[0], p[1]));
	kinds.add(new_edges(points, piece.pairs).translate_by(at), LINE);
	for (s, k) in piece.stars {
		kinds.add(new_circle(points[k] + at, laid.sizes[s] * 0.5), if code.nodes[s].symbol == () { LINE } else { CAPTION });
	}
}
book.bind("locks", kinds);

// An art node: a lock whose output is a drawing's data. Here the key
// huffman_encode gives, the code its text builds, locks a message with
// huffman_apply and is drawn by star_chart as named
// stars over pages; star_chart unlocks from what the pages show (shared
// star names, leaf names' letters, links' digits), so the pages are the key.
let art_lock = sheet("locks");
let sky = new_lock("sky")
	.add("enc", "huffman_encode", #{ input_base: 27, output_base: 3 })
	.add("msg", "huffman_apply", #{ input_base: 27, output_base: 3 })
	.add("sky", "star_chart", #{ input_base: 27, output_base: 3, most_stars: 12, width: 480, height: 420, star_size: 14, label_width: 16, label_height: 16, label_gap: 6 })
	.connect("enc.code", "msg.code")
	.connect("enc.code", "sky.code");
let locked = sky.run(#{ "enc.plain_text": symbols(27, corpus), "msg.plain_text": symbols(27, "EVERY LOCK OPENS") });
assert(sky.unlock(locked).text("msg.plain_text") == "EVERY LOCK OPENS", "the chart should unlock the message");
art_lock.add(caption("huffman_encode's code, the pangram's, applied by huffman_apply and drawn by star_chart: its chart is the key", TOP));
art_lock.add(caption(`EVERY LOCK OPENS locks to ${locked.text("msg.cipher_text")}`, TOP + 34));
art_lock.add(caption("result.get(\"sky.chart\").sketch(): its pages in a grid, stars numbered page by page", TOP + 68));
let chart = locked.get("sky.chart");
assert(chart.type_name == "StarChart" && chart.value().pages.len() > 1, "a star chart is pages of plain data");
art_lock.add(draw_sketch(chart.sketch(), #{ links: LINE, stars: LINE }, vec2(LEFT, TOP + 120), RIGHT - LEFT, 900));
book.bind("locks", art_lock);

// Renderers: renderers/TYPE.rhai files that draw a lock's values as
// canvases, in the journal's engine, apart from the lock. Attach one to a
// chain, wire values to its inputs as locks are wired, and a run's
// result.canvases(id) is what it draws: here a maze lock's maze in glyphs.
// A chain file, chains/NAME.rhai, is a chain with its renderers that
// chain(name) builds (the lock editor makes them, and journals follow its
// edits): raven_sky locks a message with a text's code, draws the cipher
// text in balanced-ternary marks and the star chart a page a constellation
// (each page-sized canvas, shown small).
let drawn = sheet("locks");
drawn.add(caption("lock.attach(id, renderer, options), lock.connect(port, \"id.input\"), result.canvases(id): renderers", TOP));
drawn.add(caption("chain(name, options): chains/raven_sky.rhai, its trit_message and star_pages renderers drawn", TOP + 34));
let grid = new_lock("grid")
	.add("maze", "maze_lock", #{ input_base: 27 })
	.attach("drawn", "maze_glyphs", #{ cell: 46.0 })
	.connect("maze.generated_maze", "drawn.maze");
assert(grid.renderers()["drawn"] == "maze_glyphs", "renderers() lists what's attached");
let gridded = grid.run(#{ "maze.maze_text": symbols(27, "HELLO WORLD"), "maze.random_seed": symbols(10, "4") });
drawn.add(gridded.canvases("drawn")[0].translate_by(vec2(LEFT + 40, TOP + 120)));
let raven = chain("raven_sky", #{ most_stars: 12 });
assert(raven.renderers()["message"] == "trit_message" && raven.renderers()["pages"] == "star_pages", "a chain file's renderers come with it");
let drawn_sky = raven.run(#{ "enc.plain_text": symbols(27, corpus), "msg.plain_text": symbols(27, "EVERY LOCK OPENS") });
drawn.add(drawn_sky.canvases("message")[0].scale_by(0.42).translate_by(vec2(LEFT + 470, TOP + 70)));
for (page, i) in drawn_sky.canvases("pages") {
	if i >= 3 { break; }
	drawn.add(page.scale_by(0.27).translate_by(vec2(LEFT + i * 340, TOP + 430)));
}
// eye: two texts in an eye on end, one round its iris, one in its pupil,
// as Welcome Home draws a substitution cipher's ordering and cipher text.
let watched = new_lock("watched")
	.add("sub", "substitution_cipher", #{ base_n: 27 })
	.set_input("sub.ordering", symbols(27, "MQ ZEHWBRKYDTXOAJGNLUFCVISP"))
	.attach("eye", "eye", #{ top_ink: RED, bottom_ink: INK, pupil_ink: INK })
	.connect("sub.ordering", "eye.iris")
	.connect("sub.cipher_text", "eye.pupil")
	.run(#{ "sub.plain_text": symbols(27, "EVERY LOCK OPENS") });
drawn.add(watched.canvases("eye")[0].scale_by(0.22).translate_by(vec2(LEFT + 20, TOP + 890)));
drawn.add(write_in("eye (iris_base, pupil_base, length, width, top_ink, bottom_ink, pupil_ink):", LEFT + 220, TOP + 1000, 22, CAPTION, PLAIN));
drawn.add(write_in("a substitution cipher's ordering round the iris, its cipher text in the pupil", LEFT + 220, TOP + 1034, 22, CAPTION, PLAIN));
book.bind("locks", drawn);

// ---- a spread: one drawing over two pages ----
//
// A page twice as wide, split into two tiles side by side: each a page of
// its own (on parchment, a sheet of its own), which together make up the
// whole drawing.

let spread = new_canvas();
spread.add(write_in("a spread", LEFT, 105, 48, HEADING, LOCHE));
spread.add(caption("new_page(#{ name, size, margin }).split(#{ columns: 2 }): one page twice as wide, cut into two", TOP));
let path = new_path().move_to(vec2(250, 1300)).cubic_to(vec2(700, 200), vec2(1700, 1500), vec2(2150, 400));
let serpent = art::snake(vec2(2150, 400), vec2(260, 1250), 3200, #{ head_angle: -0.8, max_width: 90, seed: 11 }).outline;
spread.add(serpent, LINE);
spread.add(serpent.hatch(9.0, #{ angle: 0.7, bow: 2.0, inset: 2.0 }), new_style(GREEN, PEN));
spread.add(path.fill_with_text(LOREM, #{ font: PLAIN, size: 24, style: CAPTION, avoid: serpent, clearance: 10, layout: "words" })[0]);
// Its two halves' folios.
for half in [0, 1] {
	spread.add(write_in(`${book.next + half}`, half * W + W / 2 - 8, H - 80, 24, CAPTION, PLAIN));
}
// Page margins only matter for pages without a size, which grow to their
// strokes plus the margin.
let wide = new_page(#{ name: "spread", size: vec2(2 * W, H), margin: 30 }).add(spread);
assert(wide.margin == 30 && wide.size == vec2(2 * W, H), "page settings read back");
let tiles = wide.split(#{ columns: 2 });
assert(tiles[1].tile.column == 1, "tiles know their place");
book.add("spread", tiles);

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
	column([
		if n == 0 { flow("title", #{ role: "heading", height: 110, fit: true }) } else { spacer(#{ height: 110 }) },
		rule(#{ height: 10, margin: [0, 250] }),
		row([
			column([
				if n == 0 { initial("body", #{ position: "absolute", top: 0, left: 0, width: 90, height: 90 }) } else { spacer() },
				flow("body", #{ grow: 1, split_words: true, split_padding: 0.2 }),
			], #{ grow: 1 }),
			band("diamond", #{ width: 14 }),
			column([
				if n == 0 { figure("plant", #{ height: 360, border: "frame" }) } else { spacer() },
				flow("body", #{ grow: 1 }),
			], #{ grow: 1 }),
		], #{ grow: 1, gap: 30 }),
		// roman(n) for folios: i, ii, iii.
		label(if n == 0 { "an inline layout" } else { `and on, page ${roman(n + 1)}` }, #{ role: "folio", align: "center", height: 30 }),
	], #{ padding: [90, 100, 60, 100], gap: 16 })
}
let diamond = new_path().move_to(vec2(0, -10)).line_to(vec2(6, 0)).line_to(vec2(0, 10)).line_to(vec2(-6, 0)).close();
fn plant_in(b) {
	let plant = art::bramble(vec2(b.center.x, b.y + b.height), b.height * 0.95, #{ angle: -PI() / 2.0, flower_size: b.height * 0.2, seed: 5 });
	new_canvas().add(plant, #{ vines: new_style(global::BARK, global::PEN), leaves: new_style(global::GREEN, global::PEN), flowers: new_style(global::INK, global::PEN) })
}
let body_role = #{ font: PLAIN, style: TEXT, size: 22, align: "justify", indent: 1.5, line_height: 1.3 };
let story = LOREM + LOREM + LOREM + "\n\n" + LOREM + LOREM + LOREM + LOREM + LOREM + LOREM + LOREM + LOREM;
let sample_layout = new_layout("sample", Fn("sample_page"));
assert(sample_layout.name == "sample", "layouts have names");
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
	book.add("layouts", page);
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
	book.add("layouts", page);
}

// layouts/captioned_figure.rhai: a figure filling the page, captioned.
for page in layout("captioned_figure").typeset(#{
	width: W, height: H,
	flows: #{ caption: "sed do eiusmod tempor incididunt" },
	roles: #{ caption: #{ font: PLAIN, style: CAPTION, size: 28 } },
	art: #{ figure: Fn("plant_in") },
}) {
	book.add("layouts", page);
}

// Marked-up text: markdown(text) makes a flow that's styled as it's set.
// *emphasis* and **strong** look as the role's em and strong say (by
// default italic, and gone over twice); ~~text~~ is struck through; and
// [text]{settings} sets its own colour, passes, jitter, point_jitter,
// lean, size, character padding and word spacing, and lines under and through it.
// Lines run across the spaces in what they mark, and break where the text
// does: at the ends of lines, and either side of what it wraps round.
fn markdown_page(n, info) {
	column([
		flow("title", #{ role: "heading", height: 110 }),
		flow("marks_label", #{ role: "caption" }),
		flow("marks"),
		flow("spans_label", #{ role: "caption" }),
		flow("spans"),
		flow("rules_label", #{ role: "caption" }),
		flow("rules"),
		flow("roles_label", #{ role: "caption" }),
		flow("roles", #{ role: "loud" }),
		flow("wrap_label", #{ role: "caption" }),
		column([
			figure("seal", #{ position: "absolute", top: 10, left: 360, width: 280, height: 280, wrap: "hull" }),
			flow("wrap", #{ grow: 1, split_words: true, split_padding: 0.3 }),
		], #{ grow: 1 }),
		label("markdown(text)", #{ role: "folio", align: "center", height: 30 }),
	], #{ padding: [90, 100, 60, 100], gap: 12 })
}
let marked = #{ font: PLAIN, style: TEXT, size: 24, line_height: 1.45 };
let loud = marked + #{ em: #{ color: RED }, strong: #{ underline: true, passes: 2, jitter: 1 }, rotation_jitter: 0.05 };
for page in new_layout("marked", Fn("markdown_page")).typeset(#{
	width: W, height: H,
	flows: #{
		title: "Markdown",
		// escape_markdown(text): text set as it is, its marks escaped.
		marks_label: markdown(escape_markdown("*emphasis* or _emphasis_, **strong**, ***both***, ~~struck through~~; escape_markdown(text) sets marks as they are")),
		marks: markdown("Lorem ipsum *dolor sit amet*, consectetur _adipiscing_ elit, **sed do eiusmod** tempor ~~incididunt ut labore~~ et dolore magna aliqua. Ut enim ***ad minim*** veniam, quis nostrud exercitation."),
		spans_label: "[text]{color, size, italic, skew, passes, jitter, point_jitter, character_padding, word_spacing}",
		spans: markdown("Lorem ipsum [dolor sit]{color: #9b2318} amet, [consectetur]{size: 1.6} adipiscing [elit]{size: 0.7}, [sed do eiusmod]{italic} tempor [incididunt]{skew: -0.3} ut [labore et dolore]{passes: 4, jitter: 2.5} magna [aliqua]{point_jitter: 2}, [aliqua ut enim]{character_padding: 0.5} ad [minim veniam quis nostrud]{word_spacing: 1.4} exercitation, [ullamco [laboris]{size: 1.5} nisi]{color: #9b2318, italic}."),
		rules_label: "underline: true, a count, or {count, spacing, color, passes, jitter, point_jitter}; strike (strikethrough) likewise",
		rules: markdown("Lorem ipsum [dolor sit amet]{underline} consectetur [adipiscing elit]{underline: 2} sed do [eiusmod tempor incididunt ut labore et dolore magna aliqua, ut enim ad minim veniam]{underline: {color: #9b2318}} quis nostrud ~~exercitation~~ ullamco [laboris nisi ut aliquip]{strike: {count: 3, color: #9b2318}} ex ea [commodo consequat]{color: #9b2318, strike: {count: 4, spacing: 0.2, color: #2b1d12, jitter: 1.5}}."),
		roles_label: "a role's em and strong: em: #{ color }, strong: #{ underline: true, passes: 2, jitter: 1 }; rotation_jitter: 0.05",
		roles: markdown("Lorem ipsum *dolor sit* amet, **consectetur adipiscing** elit, sed *do eiusmod* tempor **incididunt** ut labore."),
		wrap_label: "wrapped round a figure with split_words: marked words, and their lines, break either side",
		wrap: markdown("[Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt]{underline} ut labore et dolore magna aliqua. Ut enim ad minim veniam, [quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat]{strike: {count: 2, color: #9b2318}}. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. [Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum]{underline: {count: 2, color: #9b2318}}."),
	},
	roles: #{
		heading: #{ font: LOCHE, style: HEADING, size: 80, align: "center", valign: "center" },
		caption: #{ font: PLAIN, style: CAPTION, size: 20 },
		marks: marked, spans: marked, rules: marked, loud: loud,
		wrap: marked + #{ align: "justify" },
		seal: #{ style: CAPTION },
		folio: #{ font: PLAIN, style: CAPTION, size: 20 },
	},
	art: #{ seal: seal(vec2(0, 0)) },
}) {
	book.add("layouts", page);
}

// ---- the title page and the contents, which go first ----

// layouts/bordered_title.rhai, a framed title page.
let leaf = new_path().move_to(vec2(0, -22)).quad_to(vec2(14, -4), vec2(0, 22)).quad_to(vec2(-14, -4), vec2(0, -22));
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
journal_entry.add(title);

let contents = sheet("contents");
let y = TOP + 40;
for entry in book.contents {
	contents.add(write_in(entry[0], LEFT + 60, y, 40, HEADING, LOCHE));
	contents.add(new_line(vec2(420, y + 12), vec2(980, y + 12)), new_style(RED, new_brush("Ink Pen", 0.2, 0.4, 1)));
	contents.add(write(`${entry[1]}`, 1010, y, 30));
	y += 110;
}
journal_entry.add(new_page(#{ name: "02_contents", size: vec2(W, H), svg_style: #{ roughness: 1.5, noise_amplitude: 0.8 } }).add(contents));

journal_entry.add(book.pages);

// Every character above has a glyph.
if missing_glyphs().len() > 0 {
	warn("characters with no glyph: " + missing_glyphs());
}
