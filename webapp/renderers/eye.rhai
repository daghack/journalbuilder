// eye: an eye stood on end, its corners top and bottom, its iris ringed
// with one text's symbols (outer, in outer_base) as balanced-ternary glyphs,
// clockwise from the top, and its pupil filled with another's (inner, in
// inner_base), as close as they fit. Each symbol is shifted to sit round
// 0, as a letter of the ternary font: in base 27, space is 0, A to M 1 to
// 13 and N to Z -13 to -1. Welcome Home's eye holds a substitution
// cipher's ordering in its iris and the cipher text in its pupil.
//
// The canvas is width across and length down, the eye's corners at the
// middle of its top and bottom edges. Its lines run from top_ink down to
// bottom_ink; the pupil's glyphs are in inner_ink.

fn options() {
	#{
		outer_base: 27, inner_base: 27,
		length: 1640.0, width: 717.2,
		top_ink: "#09603d", bottom_ink: "#0b2a1c", inner_ink: "#000000",
	}
}

fn inputs(o) {
	#{ outer: `Base${o.outer_base}Symbol`, inner: `Base${o.inner_base}Symbol` }
}

fn render(o, i) {
	let centre = vec2(o.width / 2.0, o.length / 2.0);
	let shape = eye_shape(centre, o.length, o.width);
	let canvas = new_canvas();
	let ink = new_gradient([o.top_ink, o.bottom_ink], #{ vertical: true });
	draw_eye(canvas, centre, shape, ink, letters(i.outer, o.outer_base));
	fill_pupil(canvas, centre, shape, o.inner_ink, letters(i.inner, o.inner_base));
	canvas
}

fn example() {
	#{ outer: symbols(27, "MQ ZEHWBRKYDTXOAJGNLUFCVISP").symbols(), inner: symbols(27, "WELCOME HOME").symbols() }
}

// Symbols in `base` as the ternary font's letters, each shifted to sit
// round 0: 0 a space, 1 to 13 A to M, -13 to -1 N to Z.
fn letters(symbols, base) {
	let text = "";
	for s in symbols {
		let v = if s <= (base - 1) / 2 { s } else { s - base };
		text += if v == 0 { " " } else if v > 0 { "ABCDEFGHIJKLM"[v - 1] } else { "NOPQRSTUVWXYZ"[v + 13] };
	}
	text
}

// The eye's lids, corners top and bottom, bowing out left and right: the
// two lids, and the iris's rings' radii.
fn eye_shape(centre, length, width) {
	let outer = width / 2.0 - 6.0;
	let inner = outer * 0.56;
	let half = length / 2.0;
	let rise = (outer + 6.0) / 0.75;
	let left = new_path().move_to(centre + vec2(-3, -half))
		.cubic_to(centre + vec2(-rise, -half * 0.45), centre + vec2(-rise, half * 0.45), centre + vec2(-3, half));
	let right = new_path().move_to(centre + vec2(9, -half + 8))
		.cubic_to(centre + vec2(rise, -half * 0.45), centre + vec2(rise, half * 0.45), centre + vec2(9, half - 8));
	#{ left: left, right: right, outer: outer, inner: inner }
}

// The eye drawn: lids and rings gone over, and the ring of ternary glyphs,
// clockwise from the top, each turned to the ring and a little off true.
fn draw_eye(canvas, centre, shape, ink, key_text) {
	let pen = new_brush("Rough Ink", 0.3, 0.6, 2.0, #{ pressure_profile: "taper", pressure_variation: 0.15, pool: 0.85, pool_depth: 0.6 });
	// The glyphs: the same pen unpooled, so their strokes taper away.
	let key_pen = new_brush("Rough Ink", 0.3, 0.6, 2.0, #{ pressure_profile: "taper", pressure_variation: 0.15 });
	let lids = new_style(ink, pen, #{ passes: 2, jitter: 2.5 });
	let rings = new_style(ink, pen, #{ passes: 3, jitter: 4.0 });
	let key = new_style(ink, key_pen, #{ passes: 3, jitter: 2.0 });
	canvas.add(shape.left, lids);
	canvas.add(shape.right, lids);
	canvas.add(new_circle(centre, shape.outer), rings);
	canvas.add(new_circle(centre, shape.inner), rings);
	let glyphs = ternary_font("balanced_ternary", #{ character_padding: 0.32 }).glyphs(key_text);
	let count = glyphs.len();
	if count == 0 { return; }
	for (at, i) in new_circle(centre, (shape.outer + shape.inner) / 2.0).points_along(count, #{ start: 0.75 }) {
		let g = glyphs[i];
		g.scale = (shape.outer - shape.inner) * 0.8;
		g.position = at.position + vec2(random_range(-0.24, 0.24), random_range(-0.36, 0.36));
		g.rotation = at.angle + random_range(-0.07, 0.07);
		canvas.add(g.scale_by(random_range(0.93, 1.07)), key);
	}
}

// The pupil filled with the text, glyph by glyph, as close to the inner
// ring as the ring's wandering passes allow, top to bottom and side to
// side: at the largest size, from 40 down, that holds all of it.
fn fill_pupil(canvas, centre, shape, ink, text) {
	if text == "" { return; }
	let font = ternary_font("balanced_ternary", #{ character_padding: 0.08 });
	// A finer nib for the small glyphs, pooled at their ends.
	let pen = new_brush("Rough Ink", 0.3, 0.6, 1.2, #{ pressure_profile: "taper", pressure_variation: 0.15, pool: 0.85, pool_depth: 0.6 });
	let size = 40.0;
	loop {
		let pupil = new_textbox(#{
			shape: new_circle(centre, shape.inner - 7.0),
			size: size,
			layout: "flat",
			align: "justify",
			valign: "center",
			position_jitter: [0.03, 0.04],
			rotation_jitter: 0.05,
		});
		let filled = pupil.fill(font.glyphs(text));
		if filled.unplaced.is_empty() {
			canvas.add(filled.placed, new_style(ink, pen, #{ jitter: 0.8 }));
			return;
		}
		size -= 0.5;
		if size <= 0.0 { throw "the pupil's text doesn't fit at any size"; }
	}
}
