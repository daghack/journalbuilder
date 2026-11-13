// orrery: symbols as worlds circling twin suns, a world to a symbol, each
// on an orbit of its own: the first symbol's on the innermost, the last's
// on the outermost. A world's place round its orbit is its symbol: the
// base's symbols stand in a ring round the rim, clockwise from the top,
// with a tick inside each, and a world lies on the line from the suns to
// its symbol. The worlds are moons in their phases, dark stars
// and ringed worlds, each its own by the seed, and stars specks of dust
// between them. It suits a word or a key, a dozen symbols or so: more
// orbits are closer.
//
// ordering is the ring: the symbol at each place, clockwise from the top
// (a substitution cipher's ordering, say); unwired, the symbols in order,
// as symbol_ring has them. With show_ring false the rim's symbols are
// left out, its ticks kept: the orrery alone, to read against a ring
// drawn elsewhere (symbol_ring's, rearranged_symbol_ring's and sigil's
// places, at the same size and base, are these).
//
// The canvas is size across and down, the suns in its middle. Its styles
// are TextStyles, a style source's, each drawn whole: orbit_style draws
// the orbits, the ticks and the dust, and world_style the worlds and the
// suns. The rim's symbols are symbol_set's, a SymbolSet, at its size or
// smaller if the ring hasn't room, upright; a blank symbol a gap in the
// ring. Unwired, near-black and red ink pens, and the letters set. With
// editor, each rim symbol is committed to the visual editor as EDITOR_0 to
// EDITOR_N, so any can be changed there.

import "lib/symbol_rings" as rings;
import "lib/permutations" as perm;
import "lib/chance" as chance;

fn options() {
	#{ base: 27, size: 900.0, show_ring: true, seed: 0, stars: 40, editor: "", orbit_ink: "#2b1d12", world_ink: "#9b2318", glyph_ink: "#2b1d12" }
}

fn inputs(o) {
	#{
		worlds: `Base${o.base}Symbol`, ordering: `Base${o.base}Symbol`,
		orbit_style: "TextStyle", world_style: "TextStyle", symbol_set: "SymbolSet",
	}
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	let steady = #{ min_pressure: 0.4, max_pressure: 0.7, pressure_profile: "constant" };
	let ordering = [];
	for s in 0..o.base { ordering.push(s); }
	#{
		ordering: ordering,
		orbit_style: #{ style: new_style(new_brush_style(o.orbit_ink, "Ink Pen", 0.9, steady), hand) },
		world_style: #{ style: new_style(new_brush_style(o.world_ink, "Ink Pen", 1.5, steady), hand) },
		symbol_set: symbol_set("letters", #{ color: o.glyph_ink }),
	}
}

fn render(o, i) {
	rings::check_base(o.base);
	perm::check(i.ordering, o.base, "the ordering");
	if o.stars < 0 { throw `stars should be 0 or more, got ${o.stars}`; }
	let set = i.symbol_set;
	if set.base < o.base { throw `orrery: the symbol set '${set.name}' has ${set.base} symbols, fewer than the base, ${o.base}`; }
	let centre = vec2(o.size / 2.0, o.size / 2.0);
	let room = rings::room(o.size, o.base);
	let rim = o.size / 2.0 - room;
	let glyph = min(set.size, room);
	let c = new_canvas();
	if o.show_ring {
		for k in 0..o.base {
			let mark = write_at(set.text([i.ordering[k]]), centre + toward(k, o.base) * rim, #{ type: #{ glyph_scale: glyph } });
			if o.editor != "" { mark = mark.commit_to_editor(`${o.editor}_${k}`); }
			c.add(mark);
		}
	}
	// The ticks, inside the rim's symbols, and the orbits between the
	// suns and them.
	let edge = rim - rings::clear_of(glyph, glyph, false) - glyph * 0.1;
	let fine = new_path();
	for k in 0..o.base {
		fine.append(new_line(centre + toward(k, o.base) * edge, centre + toward(k, o.base) * (edge - glyph * 0.3)));
	}
	let count = i.worlds.len();
	let inmost = o.size * 0.1;
	let outmost = edge - glyph * 0.75;
	let apart = if count > 1 { (outmost - inmost) / (count - 1) } else { 0.0 };
	let big = min(o.size * 0.026, max(apart, o.size * 0.05) * 0.42);
	let place = perm::inverse(i.ordering);
	let worlds = new_path();
	let at = [];
	for (s, k) in i.worlds {
		let orbit = if count > 1 { inmost + apart * k } else { (inmost + outmost) / 2.0 };
		let world = centre + toward(place[s], o.base) * orbit;
		let across = big * (0.75 + 0.5 * chance::of(o.seed, k, 0));
		// The orbit stops either side of its world.
		fine.append(new_circle(centre, orbit).erase_inside(new_circle(world, across * 1.5)));
		worlds.append(world(world, across, o.seed, k));
		at.push(world);
	}
	// Dust: specks anywhere inside the ticks that's clear of the worlds.
	for n in 0..o.stars {
		let a = 2.0 * PI() * chance::of(o.seed, n, 7);
		let speck = centre + vec2(a.cos(), a.sin()) * (inmost * 0.8 + (outmost - inmost * 0.8) * chance::of(o.seed, n, 8).sqrt());
		if at.some(|w| (w - speck).length() < big * 2.5) { continue; }
		let r = o.size * (0.0022 + 0.003 * chance::of(o.seed, n, 9));
		fine.append(new_line(speck - vec2(r, 0), speck + vec2(r, 0))).append(new_line(speck - vec2(0, r), speck + vec2(0, r)));
	}
	// The twin suns, one the smaller, their rays between them left out.
	for sun in [[-1.0, 0.03], [1.0, 0.022]] {
		let r = o.size * sun[1];
		let middle = centre + vec2(sun[0] * o.size * 0.034, 0);
		worlds.append(new_circle(middle, r));
		for ray in 0..12 {
			let way = toward(ray, 12);
			if way.x * sun[0] < -0.6 { continue; }
			worlds.append(new_line(middle + way * r * 1.35, middle + way * r * (if ray % 2 == 0 { 2.0 } else { 1.7 })));
		}
	}
	c.add(fine, i.orbit_style.style).add(worlds, i.world_style.style)
}

fn example() {
	#{ worlds: symbols(27, "ALMANAC").symbols() }
}

// Which way place k of count is from the middle, clockwise from the top.
fn toward(k, count) {
	let a = -PI() / 2.0 + 2.0 * PI() * k / count;
	vec2(a.cos(), a.sin())
}

// World k, about `at` and `across` in radius, as a path: a moon in a
// phase, its dark side hatched; a black star; or a ringed world.
fn world(at, across, seed, k) {
	let kind = (chance::of(seed, k, 1) * 3.0).to_int();
	if kind == 0 {
		// Never new nor full: a moon with a dark side and a lit one.
		let moon = art::moon(at, across, 0.12 + 0.76 * chance::of(seed, k, 2));
		return new_path().append(moon.outline).append(moon.terminator).append(moon.dark.hatch(across * 0.22, #{ angle: 0.8 }));
	}
	if kind == 1 {
		let star = new_star(at, across * 1.25, across * 0.5, 5 + (chance::of(seed, k, 3) * 3.0).to_int(), #{ angle: 2.0 * PI() * chance::of(seed, k, 4) });
		return new_path().append(star).append(star.hatch(across * 0.16, #{ angle: [PI() / 4.0, -PI() / 4.0] }));
	}
	let globe = new_circle(at, across * 0.8);
	let band = new_ellipse(at, across * 1.5, across * 0.45, #{ angle: -0.5 + chance::of(seed, k, 5) });
	new_path().append(globe).append(band.erase_inside(globe))
}
