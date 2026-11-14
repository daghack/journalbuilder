// What a plate is built from besides its subject: frames, ticks, rulers,
// points and rings of glyphs round a ring, rings and lines ruled twice,
// inscriptions, blots. For the puzzle illustrations, with lib/inks' pens.
// import "lib/plates" as plates;

import "lib/chance" as chance;
import "lib/inks" as inks;

// Which way a `turn` of the way round is from a middle, clockwise from
// the top: 0.25 is to the right.
fn toward(turn) {
	let a = -PI() / 2.0 + 2.0 * PI() * turn;
	vec2(a.cos(), a.sin())
}

// Ticks round a ring, as a path: `count` of them, evenly, clockwise from
// the top, each from `inner` to `outer` out from the centre; every
// `every`-th from `inner` to `long` instead (0 for none).
fn ticks(centre, count, inner, outer, every, long) {
	let path = new_path();
	for k in 0..count {
		let way = toward(k * 1.0 / count);
		let end = if every > 0 && k % every == 0 { long } else { outer };
		path.append(new_line(centre + way * inner, centre + way * end));
	}
	path
}

// A frame for a plate `width` by `height`, its corner at (0, 0): two
// rules, `inset` and `inset + gap` in from the edges, each running on past
// its corners to the edge as ruled lines do, and ticks between them,
// about `step` apart. Gives #{ rules, ticks }, paths.
fn frame(width, height, inset, gap, step) {
	let rules = new_path();
	for d in [inset, inset + gap] {
		rules.append(new_line(vec2(0, d), vec2(width, d)));
		rules.append(new_line(vec2(0, height - d), vec2(width, height - d)));
		rules.append(new_line(vec2(d, 0), vec2(d, height)));
		rules.append(new_line(vec2(width - d, 0), vec2(width - d, height)));
	}
	let ticks = new_path();
	let across = ((width - 2.0 * inset) / step).round().to_int();
	for k in 1..across {
		let x = inset + (width - 2.0 * inset) * k / across;
		ticks.append(new_line(vec2(x, inset), vec2(x, inset + gap)));
		ticks.append(new_line(vec2(x, height - inset), vec2(x, height - inset - gap)));
	}
	let down = ((height - 2.0 * inset) / step).round().to_int();
	for k in 1..down {
		let y = inset + (height - 2.0 * inset) * k / down;
		ticks.append(new_line(vec2(inset, y), vec2(inset + gap, y)));
		ticks.append(new_line(vec2(width - inset, y), vec2(width - inset - gap, y)));
	}
	#{ rules: rules, ticks: ticks }
}

// Glyphs set round a ring, added to canvas c: names[k] (() for a gap) in
// the middle of the k-th of names.len() even places, clockwise from the
// top, each `size` across, in the type's glyphset, its foot to the centre.
// styles is one style, or one for each.
fn glyph_ring(c, t, names, centre, radius, size, styles) {
	let count = names.len();
	for k in 0..count {
		if names[k] == () { continue; }
		let turn = (k + 0.5) / count;
		let g = t.glyphset.glyph(names[k]);
		g.scale = size;
		g.position = centre + toward(turn) * radius;
		g.rotation = 2.0 * PI() * turn;
		c.add(g, if type_of(styles) == "array" { styles[k] } else { styles });
	}
}

// The names of thirteen digits among a glyphset's own glyphs: its first
// thirteen letters where it has an alphabet, its digits 0 to 12 in
// balanced-ternary digits, or failing those its first thirteen
// characters, as symbol_ring finds symbols'.
fn digit_names(set, names) {
	if names[0] == () { names.extract(1, 13) } else { names.extract(0, 13) }
}

// Blots of spilt ink about `at`: `count` of them, most near it and small,
// a few flung as far as `spread`, the biggest `big` across; and a run or
// two dripping down from the largest. Gives an array of #{ path, size }:
// each lot of blots, and the brush size to draw them with (lib/inks'
// band), so each is filled.
fn splatter(at, spread, big, count, seed) {
	let lots = [];
	for lot in 0..3 {
		// Big, middling and fine: fewer of the big.
		let across = big * [1.0, 0.45, 0.2][lot];
		let path = new_path();
		for n in 0..count {
			if chance::of(seed, n, 40) * 3.0 < lot * 1.0 || lot == 0 && n % 5 != 0 { continue; }
			let far = spread * chance::of(seed, n + lot * 97, 41) ** (1.0 + lot);
			let a = 2.0 * PI() * chance::of(seed, n + lot * 97, 42);
			let middle = at + vec2(a.cos(), a.sin()) * far * (0.3 + 0.35 * lot);
			let r = across * (0.25 + 0.3 * chance::of(seed, n, 43));
			path.append(new_circle(middle, r * 0.5));
			// Thrown blots trail off the way they flew.
			if lot > 0 && chance::of(seed, n, 44) < 0.4 {
				path.append(new_line(middle, middle + vec2(a.cos(), a.sin()) * r * (1.5 + 3.0 * chance::of(seed, n, 45))));
			}
			if lot == 0 && chance::of(seed, n, 46) < 0.6 {
				let run = big * (1.5 + 4.0 * chance::of(seed, n, 47));
				path.append(new_path().move_to(middle).quad_to(middle + vec2(across * 0.1, run * 0.5), middle + vec2(0, run)));
			}
		}
		lots.push(#{ path: path, size: across / 6.0 });
	}
	lots
}

// The symbols for `count` places of a family of marks: `given` (an array
// of whole numbers, as an "Any" input arrives; an empty array for none),
// and where it gives none, `fallback(k)`. Each must be a symbol of `set`,
// `what` naming the family in the error.
fn slot(given, count, set, what, fallback) {
	if type_of(given) != "array" {
		throw `${what} should be symbols, an array of whole numbers, got ${type_of(given)}`;
	}
	let out = [];
	for k in 0..count {
		let s = if k < given.len() { given[k] } else { fallback.call(k) };
		if type_of(s) != "i64" || s < 0 || s >= set.base {
			throw `${what}: symbol ${s} (place ${k}) isn't a symbol of symbol set '${set.name}' (0 to ${set.base - 1})`;
		}
		out.push(s);
	}
	out
}

// `count` points evenly round a circle of `radius` about `centre`,
// clockwise from the top: an array of vecs.
fn around(centre, radius, count) {
	let points = [];
	for k in 0..count { points.push(centre + toward(k * 1.0 / count) * radius); }
	points
}

// A ruler from a to b, as a path: the line, and count + 1 ticks along it,
// evenly from a to b, rising `short` to its left as it runs (up, for one
// running left to right); every `every`-th rises `long` instead (0 for
// none).
fn ruler(a, b, count, short, every, long) {
	let path = new_path().append(new_line(a, b));
	let way = (b - a).normalized();
	let up = vec2(way.y, -way.x);
	for k in 0..=count {
		let at = a + (b - a) * (k * 1.0 / count);
		let rise = if every > 0 && k % every == 0 { long } else { short };
		path.append(new_line(at, at + up * rise));
	}
	path
}

// A ring ruled twice, added to canvas c: a contour `weight` wide at
// `radius` about `centre`, and a finer line `gap` inside it, in `ink`.
fn double_ring(c, centre, radius, gap, ink, weight) {
	c.add(new_circle(centre, radius), inks::contour(ink, weight));
	c.add(new_circle(centre, radius - gap), inks::fine(ink, weight * 0.45));
}

// A line ruled twice, added to canvas c: a contour `weight` wide from a to
// b, and a finer line `gap` to its right as it runs (below, for one
// running left to right), in `ink`.
fn double_rule(c, a, b, gap, ink, weight) {
	let way = (b - a).normalized();
	let down = vec2(-way.y, way.x) * gap;
	c.add(new_line(a, b), inks::contour(ink, weight));
	c.add(new_line(a + down, b + down), inks::fine(ink, weight * 0.45));
}

// What to write along `path` so path.fit_text sets it at about the glyph
// scale of `hand` (a text style, #{ style, type, ... }): as many whole
// words of `text`, over and over, as go along it at that scale, each with
// the space after it. fit_text sets them a little larger, to run the
// path's whole length, so a ring's inscription closes between two words
// and is as big as the hand says, give or take a word's room.
fn inscription(path, text, hand) {
	let words = [];
	for w in text.split(" ") { if w != "" { words.push(w); } }
	if words.is_empty() { return ""; }
	let upto = |k| { let said = ""; for n in 0..k { said += words[n % words.len()] + " "; } said };
	let fits = |k| { let ok = false; try { ok = path.fill_with_text(upto.call(k), hand).len() == 1; } catch { ok = false; } ok };
	let low = 1;
	let high = 2;
	while fits.call(high) { low = high; high *= 2; }
	while high - low > 1 {
		let mid = (low + high) / 2;
		if fits.call(mid) { low = mid; } else { high = mid; }
	}
	upto.call(low)
}
