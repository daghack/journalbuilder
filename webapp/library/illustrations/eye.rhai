// eye: an eye stood on end, its corners top and bottom, its iris ringed
// with one text's symbols (outer, in outer_base) as balanced-ternary glyphs,
// clockwise from the top, and its pupil filled with another's (inner, in
// inner_base), as close as they fit. Each symbol is shifted to sit round
// 0, as a letter of the ternary glyphset: in base 27, space is 0, A to M 1 to
// 13 and N to Z -13 to -1. Welcome Home's eye holds a substitution
// cipher's ordering in its iris and the cipher text in its pupil.
//
// The canvas is width across and length down, the eye's corners at the
// middle of its top and bottom edges.
//
// Its styles are TextStyles, a style source's, each drawn whole:
// lid_style draws the lids and ring_style the iris's rings; iris_style
// writes the iris's glyphs and pupil_style the pupil's, in their types'
// glyphsets (whose letters A to M and N to Z are 1 to 13 and -13 to -1),
// each glyph set off true by its hand's glyph jitter. The iris's glyphs are
// iris_style's size, or smaller if the ring hasn't room; the pupil's text
// is as large as all of it fits, up to pupil_style's size, its lines
// justified and centred. Unwired, the styles are rough ink pens, the lids,
// rings and iris inked from top_ink down to bottom_ink and the pupil in
// inner_ink, at pupil_size, the glyphs balanced ternary.
//
// pupil is how the pupil's text goes: "lines", across it line under line,
// or "spiral", round and round out from its middle (art::spiral_text),
// its last turn's glyphs shrinking to fill the room left down to
// pupil_minimum_scale of their size.

fn options() {
	#{
		outer_base: 27, inner_base: 27, pupil: "lines", pupil_minimum_scale: 0.33, pupil_size: 40.0,
		length: 1640.0, width: 717.2,
		top_ink: "#09603d", bottom_ink: "#0b2a1c", inner_ink: "#000000",
	}
}

fn inputs(o) {
	#{
		outer: `Base${o.outer_base}Symbol`, inner: `Base${o.inner_base}Symbol`,
		lid_style: "TextStyle", ring_style: "TextStyle", iris_style: "TextStyle", pupil_style: "TextStyle",
	}
}

fn fallbacks(o) {
	let ink = new_gradient([o.top_ink, o.bottom_ink], #{ vertical: true });
	let pen = new_brush_style(ink, "Rough Ink", 2.0, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15, pool: 0.85, pool_depth: 0.6 });
	// The glyphs: the same pen unpooled, so their strokes taper away.
	let key_pen = new_brush_style(ink, "Rough Ink", 2.0, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15 });
	// A finer nib for the pupil's small glyphs, pooled at their ends.
	let pupil_pen = new_brush_style(o.inner_ink, "Rough Ink", 1.2, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15, pool: 0.85, pool_depth: 0.6 });
	let ring = eye_shape(vec2(o.width / 2.0, o.length / 2.0), o.length, o.width);
	#{
		lid_style: #{ style: new_style(pen, new_handwriting_style(#{ passes: 2, pass_end_jitter: 2.5 })) },
		ring_style: #{ style: new_style(pen, new_handwriting_style(#{ passes: 3, pass_end_jitter: 4.0 })) },
		iris_style: #{
			style: new_style(key_pen, new_handwriting_style(#{ passes: 3, pass_end_jitter: 2.0, glyph_position_jitter: [0.24, 0.36], glyph_rotation_jitter: 0.07, glyph_scale_jitter: 0.07 })),
			type: new_type_style(ternary_glyphset("balanced_ternary"), iris_room(ring)),
		},
		pupil_style: #{
			style: new_style(pupil_pen, new_handwriting_style(#{ pass_end_jitter: 0.8, glyph_position_jitter: [0.03 * o.pupil_size, 0.04 * o.pupil_size], glyph_rotation_jitter: 0.05 })),
			type: new_type_style(ternary_glyphset("balanced_ternary"), o.pupil_size, #{ glyph_outline_gap: 0.08 }),
			paragraph: #{ line_height: 1.05, clearance: 4.0 },
		},
	}
}

fn render(o, i) {
	let centre = vec2(o.width / 2.0, o.length / 2.0);
	let shape = eye_shape(centre, o.length, o.width);
	let canvas = new_canvas();
	draw_eye(canvas, centre, shape, i, letters(i.outer, o.outer_base));
	let text = letters(i.inner, o.inner_base);
	switch o.pupil {
		"lines" => fill_pupil(canvas, centre, shape, i, text),
		"spiral" => spiral_pupil(canvas, centre, shape, i, text, o.pupil_minimum_scale),
		_ => throw `the pupil should be "lines" or "spiral", got "${o.pupil}"`,
	}
	canvas
}

fn example() {
	#{ outer: symbols(27, "MQ ZEHWBRKYDTXOAJGNLUFCVISP").symbols(), inner: symbols(27, "WELCOME HOME").symbols() }
}

// Symbols in `base` as the ternary glyphset's letters, each shifted to sit
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

// How big the iris's glyphs can be: 0.8 of the ring's breadth.
fn iris_room(shape) {
	(shape.outer - shape.inner) * 0.8
}

// The eye drawn, in i's styles: lids and rings gone over, and the ring of
// iris_style's glyphs, clockwise from the top, each turned to the ring
// and set off true by its hand.
fn draw_eye(canvas, centre, shape, i, key_text) {
	let lids = i.lid_style.style;
	let rings = i.ring_style.style;
	canvas.add(shape.left, lids);
	canvas.add(shape.right, lids);
	canvas.add(new_circle(centre, shape.outer), rings);
	canvas.add(new_circle(centre, shape.inner), rings);
	let t = i.iris_style.type;
	let glyphs = t.glyphset.glyphs(key_text);
	let count = glyphs.len();
	if count == 0 { return; }
	let hand = i.iris_style.style;
	let across = hand.glyph_position_jitter;
	let down = across;
	if type_of(across) == "array" { down = across[1]; across = across[0]; }
	let size = min(t.glyph_scale * t.glyph_scale_factor, iris_room(shape));
	for (at, k) in new_circle(centre, (shape.outer + shape.inner) / 2.0).points_along(count, #{ start: 0.75 }) {
		let g = glyphs[k];
		g.scale = size;
		g.position = at.position + vec2(either_way(across), either_way(down));
		g.rotation = at.angle + either_way(hand.glyph_rotation_jitter);
		canvas.add(g.scale_by(1.0 + either_way(hand.glyph_scale_jitter)), hand);
	}
}

// Up to `amount` either way, at random; nothing drawn for 0.
fn either_way(amount) {
	if amount > 0.0 { random_range(-amount, amount) } else { 0.0 }
}

// The pupil filled with the text in pupil_style, written round and round
// out from the middle to the inner ring, at the largest size, up to its
// glyph scale, that holds all of it, the last turn's glyphs shrinking to
// fill the room left, down to minimum_scale of that size.
fn spiral_pupil(canvas, centre, shape, i, text, minimum_scale) {
	if text == "" { return; }
	canvas.add(art::spiral_text(text, centre, shape.inner - 7.0, i.pupil_style + #{ minimum_scale: minimum_scale }));
}

// The pupil filled with the text in pupil_style, glyph by glyph, as close
// to the inner ring as the ring's wandering passes allow, top to bottom
// and side to side, its lines justified and centred: at the largest size,
// up to its glyph scale, that holds all of it.
fn fill_pupil(canvas, centre, shape, i, text) {
	if text == "" { return; }
	let pupil = i.pupil_style;
	pupil.paragraph.align = "justify";
	pupil.paragraph.valign = "center";
	pupil.paragraph.fit = true;
	canvas.add(write_inside_path(text, new_circle(centre, shape.inner - 7.0), pupil).canvas);
}
