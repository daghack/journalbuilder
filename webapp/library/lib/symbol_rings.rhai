// Rings of a base's symbols, clockwise from the top, and the walks round
// them that a step takes: what the symbol ring illustrations draw with
// (symbol_ring, rearranged_symbol_ring, joined_symbol_ring).
// import "lib/symbol_rings" as rings;

// Throws unless base is from 2 to 27.
fn check_base(base) {
	if base < 2 || base > 27 { throw `the base should be from 2 to 27, got ${base}`; }
}

// Throws unless step, called what, is from 0 to base - 1.
fn check_step(base, step, what) {
	if step < 0 || step >= base { throw `${what} should be from 0 to base - 1 (${base - 1}), got ${step}`; }
}

// Throws unless steps is 0 (every line) or more.
fn check_steps(steps) {
	if steps < 0 { throw `steps should be 0 (every line) or more, got ${steps}`; }
}

// How big a ring's glyphs can be: as large as fit side by side round
// it, at most a fifth of its radius.
fn room(size, base) {
	min(size * 0.4 * 2.0 * (PI() / base).sin() * 0.55, size * 0.4 * 0.2)
}

// A ring of o.base symbols on a canvas o.size across and down, its centre
// in the middle, leaving room round its edge for glyphs as large as fit,
// and count symbols side by side in the middle: at, each place's middle;
// glyph, the size of its symbols (the type's, up to that room); big, the
// middle's (center_scale times theirs, up to as big as fits in a circle
// half the radius across, or for one symbol, half the radius); middle_at,
// each middle symbol's middle; middle_border, the radius of the circle
// round them all; clear, how far from a place's middle its lines stop;
// and middles, #{ at, clear }, the circles lines stop short of: one round
// the middle's symbols. bordered rings the places' symbols and
// center_bordered the middle's.
fn ring(o, t, count) {
	let centre = vec2(o.size / 2.0, o.size / 2.0);
	let radius = o.size / 2.0 - room(o.size, o.base);
	let glyph = min(t.glyph_scale * t.glyph_scale_factor, room(o.size, o.base));
	// From the first middle symbol's middle to the last's, over their size.
	let row = (count - 1) * spacing();
	let big = min(glyph * o.center_scale, radius * 0.5 / (1.0 + row / 1.5));
	let reach = big * row / 2.0;
	let at = [];
	for k in 0..o.base {
		let a = -PI() / 2.0 + 2.0 * PI() * k / o.base;
		at.push(centre + vec2(a.cos(), a.sin()) * radius);
	}
	let middle_at = [];
	for k in 0..count { middle_at.push(centre + vec2(big * spacing() * k - reach, 0.0)); }
	#{
		centre: centre, radius: radius, at: at, glyph: glyph, big: big,
		bordered: o.bordered, center_bordered: o.center_bordered,
		middle_at: middle_at, middle_border: reach + big * 0.75,
		clear: clear_of(glyph, glyph, o.bordered),
		middles: [#{ at: centre, clear: reach + clear_of(big, glyph, o.center_bordered) }],
	}
}

// How far apart the middles of the symbols side by side in the middle
// are, over their size.
fn spacing() {
	0.9
}

// The symbols in the middle, side by side, names[k] in styles[k], and
// round them all, if the ring's center_bordered, one circle in
// border_style.
fn draw_middle(c, r, t, names, styles, border_style) {
	if r.center_bordered { c.add(new_circle(r.centre, r.middle_border), border_style); }
	for k in 0..names.len() {
		draw_symbol(c, t, names[k], r.middle_at[k], r.big, false, styles[k]);
	}
}

// How far from the middle of a symbol size across its lines stop, beside
// glyph-sized ones: past its border, or clear of its glyph.
fn clear_of(size, glyph, bordered) {
	if bordered { size * 0.75 + glyph * 0.15 } else { size * 0.65 }
}

// The names of a base's symbols among a glyphset's own glyphs: its digits,
// shifted to sit round 0, its letters, 0 a gap (()), or failing those its
// characters in order.
fn symbol_names(set, base) {
	let own = set.characters();
	if "-13" in own {
		let digits = [];
		for s in 0..base { digits.push(if s <= 13 { s } else { s - 27 }); }
		return digits;
	}
	let letters = letters_in(own, "A", "Z");
	if letters.len() < 26 { letters = letters_in(own, "a", "z"); }
	if letters.len() == 26 { return [()] + letters; }
	if own.len() < base { throw `glyphset '${set.name}' has ${own.len()} symbols, too few for base ${base}`; }
	own
}

// The single characters of own from one to another.
fn letters_in(own, from, to) {
	own.filter(|c| c.len() == 1 && c >= from && c <= to)
}

// The ring's symbols, drawn on c in style: at place k, symbol k times
// `times` (1 for the symbols in order). Times 0 takes no path, and leaves
// every place empty.
fn draw_symbols(c, r, t, names, times, style) {
	let base = r.at.len();
	for k in 0..base {
		let name = if times == 0 { () } else { names[(k * times) % base] };
		draw_symbol(c, t, name, r.at[k], r.glyph, r.bordered, style);
	}
}

// The symbol called name drawn at `at`, `size` across, in the type's
// glyphset (() is a gap), in a circle if bordered.
fn draw_symbol(c, t, name, at, size, bordered, style) {
	if bordered { c.add(new_circle(at, size * 0.75), style); }
	if name == () { return; }
	let g = t.glyphset.glyph(name);
	g.scale = size;
	g.position = at;
	g.skew = t.skew ?? t.glyphset.skew;
	c.add(g, style);
}

// The walk from the top by step, as the places each line joins, [k, j]:
// lines, and half, the line drawn half way with an arrow, or [] for none.
// Step 0 takes no path: no lines.
// With steps 0 it's every line, each once (a step of half the base joins
// each pair twice); otherwise the first steps lines of the walk, and half
// the next, unless it's back at the top by then.
fn walk(base, step, steps) {
	let lines = [];
	let half = [];
	if step == 0 { return #{ lines: lines, half: half }; }
	if steps == 0 {
		let done = #{};
		for k in 0..base {
			let j = (k + step) % base;
			let key = key_of(k, j);
			if key in done { continue; }
			done[key] = true;
			lines.push([k, j]);
		}
	} else {
		let k = 0;
		for s in 0..steps {
			let j = (k + step) % base;
			lines.push([k, j]);
			k = j;
			if k == 0 { break; }
		}
		if k != 0 { half = [k, (k + step) % base]; }
	}
	#{ lines: lines, half: half }
}

// The lines two walks both draw, by key_of.
fn common(a, b) {
	let in_a = #{};
	for l in every_line(a) { in_a[key_of(l[0], l[1])] = true; }
	let both = #{};
	for l in every_line(b) {
		let key = key_of(l[0], l[1]);
		if key in in_a { both[key] = true; }
	}
	both
}

// A walk's lines, its half line too.
fn every_line(w) {
	if w.half.len() == 2 { w.lines + [w.half] } else { w.lines }
}

// The line between two places, whichever way it's walked.
fn key_of(k, j) {
	`${min(k, j)}-${max(k, j)}`
}

// A walk drawn on c in style: dashed lines from place to place, dash and
// gap long (o's), the half line stopping half way, or short of the
// middle's symbols, in an arrow; none inside the middle's symbols. The
// lines in common are moved nudge to the left of the line from their
// lower place to their higher, so that two walks nudged opposite ways
// keep apart where they cross the same line.
fn draw_walk(c, r, w, style, o, nudge, common) {
	let lines = new_path();
	for l in w.lines {
		let ends = line_ends(r, l, nudge, common);
		dashes(lines, ends[0], ends[1], o.dash, o.gap);
	}
	if w.half.len() == 2 {
		let ends = line_ends(r, w.half, nudge, common);
		let tip = half_way(ends[0], ends[1], r.middles, o.gap);
		if (tip - ends[0]).length() > 0.0 {
			dashes(lines, ends[0], tip, o.dash, o.gap);
			c.add(arrowhead(tip, (tip - ends[0]).normalized(), max(o.dash, r.glyph * 0.3)), style);
		}
	}
	for m in r.middles { lines.erase_inside(new_circle(m.at, m.clear)); }
	c.add(lines, style);
}

// Where line l's dashes start and end: clear of the symbols at each end,
// nudged aside if it's in common.
fn line_ends(r, l, nudge, common) {
	let ends = chord(r.at[l[0]], r.at[l[1]], r.clear);
	if nudge == 0.0 || !(key_of(l[0], l[1]) in common) { return ends; }
	let along = (r.at[max(l[0], l[1])] - r.at[min(l[0], l[1])]).normalized();
	let aside = turned(along, -PI() / 2.0) * nudge;
	[ends[0] + aside, ends[1] + aside]
}

// The line from one symbol to another, as [start, end], starting and
// stopping clear short of each.
fn chord(from, to, clear) {
	let dir = (to - from).normalized();
	[from + dir * clear, to - dir * clear]
}

// Half way from `from` to `to`, or short of that where the line would
// reach within a middle symbol's clear, and gap more, of it: where it
// first gets that close.
fn half_way(from, to, middles, gap) {
	let length = (to - from).length();
	let dir = (to - from) / length;
	let t = length / 2.0;
	for m in middles {
		// Where the line comes within clear of the middle: along it to
		// the nearest point, back by the half chord.
		let clear = m.clear + gap;
		let near = (m.at - from).dot(dir);
		let miss = (from + dir * near - m.at).length();
		if miss < clear {
			t = min(t, near - (clear * clear - miss * miss).sqrt());
		}
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
