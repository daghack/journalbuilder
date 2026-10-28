// symbol_ring: a base's symbols in a ring, clockwise from the top, each
// joined by a dashed line to the symbol step on from it, round and round:
// base 5 and step 2 is a five-pointed star; where base and step share a
// factor, a figure for each cycle. In the middle, larger, is the symbol
// for the step. With bordered, each symbol, the middle's too,
// is ringed, and the dashes run from border to border.
//
// steps, if not 0, draws only the first that many lines of the walk from
// the top (0, step, 2 step, ...), and half the next, ending in an arrow
// to say it goes on; a walk that comes back to the top before then is
// drawn closed, without the arrow.
//
// The canvas is size across and down, the ring's centre in its middle.
// Its styles are TextStyles, a style source's: glyph_style for the
// symbols and their borders, line_style for the dashed lines.
// glyph_style's glyphs are its type's, at its size, or smaller if the
// ring hasn't room, leant as it leans them; the middle's are center_scale
// times that, up to half the ring's radius. In balanced-ternary digits
// (balanced_ternary's), a symbol is its digit, shifted to sit round 0 (1
// to 13 as they are, 14 to 26 as -13 to -1); in any other glyphset, its
// letter (0 a space, a gap in the ring, 1 to 26 A to Z). Unwired, they're
// near-black and red ink pens, the glyphs loche_upper, as big as the ring
// holds.

fn options() {
	#{
		base: 27, step: 5, steps: 0, bordered: true, size: 900.0, center_scale: 3.0,
		dash: 14.0, gap: 10.0, glyph_ink: "#2b1d12", line_ink: "#9b2318",
	}
}

fn inputs(o) {
	#{ glyph_style: "TextStyle", line_style: "TextStyle" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		glyph_style: #{
			style: new_style(new_brush_style(o.glyph_ink, "Ink Pen", 2.2, #{ min_pressure: 0.4, max_pressure: 0.8 }), hand),
			type: new_type_style(glyphset("loche_upper"), room(o)),
		},
		line_style: #{ style: new_style(new_brush_style(o.line_ink, "Ink Pen", 1.4), hand) },
	}
}

fn render(o, i) {
	if o.base < 2 || o.base > 27 { throw `the base should be from 2 to 27, got ${o.base}`; }
	if o.step < 0 || o.step >= o.base { throw `the step should be from 0 to base - 1 (${o.base - 1}), got ${o.step}`; }
	if o.steps < 0 { throw `steps should be 0 (every line) or more, got ${o.steps}`; }
	let centre = vec2(o.size / 2.0, o.size / 2.0);
	let n = o.base;
	// The ring leaves room round its edge for glyphs as large as fit; they're
	// the style's size, up to that, and the middle's center_scale times
	// theirs, up to half the radius.
	let t = i.glyph_style.type;
	let radius = o.size / 2.0 - room(o);
	let glyph = min(t.glyph_scale * t.glyph_scale_factor, room(o));
	let big = min(glyph * o.center_scale, radius * 0.5);
	// How far from a symbol's middle its lines stop: past its border, or
	// clear of its glyph.
	let clear = if o.bordered { glyph * 0.75 + glyph * 0.15 } else { glyph * 0.65 };
	let centre_clear = if o.bordered { big * 0.75 + glyph * 0.15 } else { big * 0.65 };

	let at = [];
	for k in 0..n {
		let a = -PI() / 2.0 + 2.0 * PI() * k / n;
		at.push(centre + vec2(a.cos(), a.sin()) * radius);
	}

	let c = new_canvas();
	let lines = new_path();
	if o.step > 0 && o.steps == 0 {
		let done = #{};
		for k in 0..n {
			let j = (k + o.step) % n;
			// A step of half the base joins each pair twice: once will do.
			let key = `${min(k, j)}-${max(k, j)}`;
			if key in done { continue; }
			done[key] = true;
			let ends = chord(at[k], at[j], clear);
			dashes(lines, ends[0], ends[1], o.dash, o.gap);
		}
	} else if o.step > 0 {
		let k = 0;
		for s in 0..o.steps {
			let j = (k + o.step) % n;
			let ends = chord(at[k], at[j], clear);
			dashes(lines, ends[0], ends[1], o.dash, o.gap);
			k = j;
			if k == 0 { break; }
		}
		if k != 0 {
			let ends = chord(at[k], at[(k + o.step) % n], clear);
			let tip = half_way(ends[0], ends[1], centre, centre_clear + o.gap);
			if (tip - ends[0]).length() > 0.0 {
				dashes(lines, ends[0], tip, o.dash, o.gap);
				c.add(arrowhead(tip, (tip - ends[0]).normalized(), max(o.dash, glyph * 0.3)), i.line_style.style);
			}
		}
	}
	lines.erase_inside(new_circle(centre, centre_clear));
	c.add(lines, i.line_style.style);

	let style = i.glyph_style.style;
	for k in 0..n {
		draw_symbol(c, t, k, at[k], glyph, o.bordered, style);
	}
	draw_symbol(c, t, o.step, centre, big, o.bordered, style);
	c
}

fn example() {
	#{}
}

// How big the ring's glyphs can be: as large as fit side by side round
// it, at most a fifth of its radius.
fn room(o) {
	min(o.size * 0.4 * 2.0 * (PI() / o.base).sin() * 0.55, o.size * 0.4 * 0.2)
}

// Symbol s drawn at `at`, `size` across, in the type's glyphset (its
// digit shifted to sit round 0, or its letter, 0 a space), in a circle if
// bordered.
fn draw_symbol(c, t, s, at, size, bordered, style) {
	if bordered { c.add(new_circle(at, size * 0.75), style); }
	let g = if "-13" in t.glyphset.characters() {
		t.glyphset.glyph(if s <= 13 { s } else { s - 27 })
	} else {
		if s == 0 { return; }
		t.glyphset.glyphs(`${" ABCDEFGHIJKLMNOPQRSTUVWXYZ"[s]}`)[0]
	};
	g.scale = size;
	g.position = at;
	g.skew = t.skew ?? t.glyphset.skew;
	c.add(g, style);
}

// The line from one symbol to another, as [start, end], starting and
// stopping clear short of each.
fn chord(from, to, clear) {
	let dir = (to - from).normalized();
	[from + dir * clear, to - dir * clear]
}

// Half way from `from` to `to`, or short of that where the line would
// reach within `clear` of the centre: where it gets that close.
fn half_way(from, to, centre, clear) {
	let length = (to - from).length();
	let dir = (to - from) / length;
	let t = length / 2.0;
	// Where the line comes within clear of the centre: along it to the
	// nearest point, back by the half chord.
	let near = (centre - from).dot(dir);
	let miss = (from + dir * near - centre).length();
	if miss < clear {
		t = min(t, near - (clear * clear - miss * miss).sqrt());
	}
	from + dir * max(t, 0.0)
}

// An arrowhead with its tip at `tip`, pointing along `dir`, its barbs
// `size` long.
fn arrowhead(tip, dir, size) {
	new_path().move_to(tip - turned(dir, 0.45) * size).line_to(tip).line_to(tip - turned(dir, -0.45) * size)
}

// v turned by angle a.
fn turned(v, a) {
	vec2(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos())
}

// A dashed line from `from` to `to`, added to path: dashes about dash
// long with gap between, as many as fit, stretched to end on a dash.
fn dashes(path, from, to, dash, gap) {
	let length = (to - from).length();
	if length <= 0.0 { return; }
	let dir = (to - from) / length;
	let count = max(1, ((length + gap) / (dash + gap)).round().to_int());
	let d = (length - gap * (count - 1)) / count;
	if d <= 0.0 { d = length; count = 1; }
	for k in 0..count {
		let t = k * (d + gap);
		path.move_to(from + dir * t).line_to(from + dir * (t + d));
	}
}
