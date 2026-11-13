// What a plate is built from besides its subject: frames, ticks, rings of
// glyphs, blots. For the puzzle illustrations, with lib/inks' pens.
// import "lib/plates" as plates;

import "lib/chance" as chance;

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
