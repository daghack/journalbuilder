// modular_maze: a ring in disguise, as a map of places and the ways
// between them, each way so many steps long. The places are a ring's,
// 0 to base - 1 round it, and each way goes its steps clockwise round the
// ring, but the map is laid out by forces as graph_map lays out its map
// (graph::layout), so the ring doesn't show. The ways that pass over the
// start, place 0, or end on it are drawn in red. Told to walk a number
// of steps from the start, by any ways that add up to it, a reader ends
// at the number's remainder over the base, whichever ways they take, and
// has taken as many red ways as the base goes into it: walking it
// divides.
//
// places, an array of strings, are the places in ring order, the first
// the start, each a place line as graph::parse reads one ("Name", or
// "Name: glyph glyph" for its glyphs); how many there are is the base.
// Unwired, the first base of a list of 27 placeholder names.
//
// Every number can be walked from the start. The ways that don't pass
// over it go forward round the ring, so walked from the start they add
// up to the place they reach: every place but the start has a way in from
// a place before it, so the start reaches each place p in p steps; and
// one red way leads back into the start, so the start is the base steps
// round, as often as a walk likes. edges is how many ways there are,
// from base (just those) to base * (base - 1) / 2 (every two places
// joined); 0 is a quarter more than the places, rounded. The rest join
// pairs of places not yet joined, one way or the other, and no two
// places are joined both ways (a round trip between them would always be
// the base, giving it away). Some places may be dead ends: a maze's
// blind ways. All of it's chosen by seed. chosen gives the ways instead,
// as [from, steps] pairs, and walks, an array of whole numbers, the walks
// they must allow (the reader will be told to take): if they don't, it's
// an error. Each way's text
// is step_text with {n} as its steps and {word} as them in words ("{word}
// paces"). With mark_start, the start's name is boxed twice.
//
// Its styles are TextStyles, a style source's, each drawn whole, as
// graph_map's: name_style, glyph_style and box_style draw the places,
// edge_style and line_style the ways, and wrap_edge_style and
// wrap_line_style the red ways. Unwired, near-black ink pens, and red for
// the red ways, the names and steps in plain, centred and at a steady
// pressure, the glyphs from symbols. The rest of the options are graph_map's;
// generations, as graph::layout's, evolves the map after the forces have
// laid it out, for fewer crossings, shorter lines and more room.

import "lib/graph_maps" as maps;
import "lib/chance" as chance;

fn options() {
	#{
		base: 13, edges: 0, seed: 0, chosen: [], step_text: "{n}", mark_start: true,
		width: 1200.0, height: 900.0, label_place: "along", iterations: 300, generations: 400,
		name_width: 260.0, text_width: 320.0, padding: 10.0, gap: 24.0, arrow: 18.0,
		name_size: 30.0, edge_size: 24.0, glyph_size: 36.0, dot: 10.0,
	}
}

fn inputs(o) {
	#{
		places: "Any", walks: "Any",
		name_style: "TextStyle", edge_style: "TextStyle", glyph_style: "TextStyle",
		box_style: "TextStyle", line_style: "TextStyle",
		wrap_edge_style: "TextStyle", wrap_line_style: "TextStyle",
	}
}

fn fallbacks(o) {
	let ink = "#2b1d12";
	let red = "#9b2318";
	let hand = new_handwriting_style();
	let pen = |color, width| new_style(new_brush_style(color, "Ink Pen", width), hand);
	// Text at a steady pressure, so its dots and commas show.
	let steady = |color, width| new_style(new_brush_style(color, "Ink Pen", width, #{ min_pressure: 0.5, max_pressure: 0.9, pressure_profile: "constant" }), hand);
	let plain = glyphset("plain");
	let centred = new_paragraph_style(#{ align: "center" });
	let names = placeholder_places();
	if o.base < 3 || o.base > names.len() { throw `modular_maze: base should be from 3 to ${names.len()} without places wired, got ${o.base}`; }
	#{
		places: names.extract(0, o.base),
		walks: [],
		name_style: #{ style: steady.call(ink, 1.6), type: new_type_style(plain, o.name_size), paragraph: centred },
		edge_style: #{ style: steady.call(ink, 1.4), type: new_type_style(plain, o.edge_size), paragraph: centred },
		wrap_edge_style: #{ style: steady.call(red, 1.4), type: new_type_style(plain, o.edge_size), paragraph: centred },
		glyph_style: #{ style: pen.call(ink, 1.6), type: new_type_style(glyphset("symbols"), o.glyph_size) },
		box_style: #{ style: pen.call(ink, 1.8) },
		line_style: #{ style: pen.call(ink, 1.4) },
		wrap_line_style: #{ style: pen.call(red, 1.6) },
	}
}

fn render(o, i) {
	maps::check(o, "modular_maze");
	let places = i.places;
	let n = places.len();
	if n < 3 { throw `modular_maze: there should be 3 places or more, got ${n}`; }
	for w in i.walks {
		if type_of(w) != "i64" || w < 0 { throw `modular_maze: walks should be whole numbers, 0 or more, got ${w}`; }
	}
	let ways = if o.chosen.is_empty() { choose(o, n, i.walks) } else { check_chosen(o.chosen, n, i.walks) };
	// The places' lines, then a line for each way.
	let names = [];
	for p in places {
		let name = p.split(":")[0];
		name.trim();
		names.push(name);
	}
	let lines = places;
	for w in ways {
		lines.push(`${names[w[0]]} -> ${names[(w[0] + w[1]) % n]}: ${step_text(o.step_text, w[1])}`);
	}
	let g = graph::parse(lines.reduce(|text, line| text + line + "\n", ""));
	let edges = [];
	let pens = [];
	for w in ways {
		let wraps = w[0] + w[1] >= n;
		edges.push(if wraps { i.wrap_edge_style } else { i.edge_style });
		pens.push(if wraps { i.wrap_line_style } else { i.line_style });
	}
	maps::draw(g, o, #{
		name_style: i.name_style, glyph_style: i.glyph_style, box_style: i.box_style,
		edge_styles: edges, line_styles: pens, twice: if o.mark_start { [0] } else { [] },
	}, "modular_maze")
}

fn example() {
	#{}
}

// The ways, [from, steps] each: o.edges of them round n places (0 a
// quarter more than the places), picked by o.seed, picked again until
// they fit (as ways_for has them) and every walk can be walked.
fn choose(o, n, walks) {
	let most = n * (n - 1) / 2;
	let count = if o.edges == 0 { (n * 1.25).round().to_int() } else { o.edges };
	if count < n || count > most {
		throw `modular_maze: edges should be from ${n} (a way into every place, and one back to the start) to ${most} (every two places joined), got ${count}`;
	}
	let last = "the ways didn't fit";
	for attempt in 0..500 {
		let ways = ways_for(o.seed * 1009 + attempt, n, count);
		if ways == () { continue; }
		last = "";
		for total in walks {
			if !walkable(ways, n, total) { last = `no walk of ${total}`; break; }
		}
		if last == "" { return ways; }
	}
	throw `modular_maze: none of 500 picks of ${count} ways by seed ${o.seed} would do (the last: ${last}); try another seed, or more edges`;
}

// count ways round n places by seed, or () if the seed's picks don't fit,
// so that every number can be walked from the start:
// - a way into every place but the start from a place before it round
//   the ring, never passing over the start, so that the start reaches
//   every place p by ways that add up to p: every number less than the
//   base can be walked;
// - a way back into the start, from a place it has no way to: round to
//   it and back to the start is the base, so every larger number can be
//   walked, round and round and then on to its remainder;
// - the rest between pairs of places not yet joined, in an order of the
//   seed's, one way or the other.
// No two places are joined both ways: a round trip between them would
// always be the base, giving it away. Sorted by place and steps.
fn ways_for(seed, n, count) {
	let ways = [];
	let joined = #{};
	for p in 1..n {
		let from = pick(seed, p, 3, p);
		ways.push([from, p - from]);
		joined[`${from}-${p}`] = true;
	}
	let back = [];
	for p in 1..n {
		if !(`0-${p}` in joined) { back.push(p); }
	}
	if back.is_empty() { return (); }
	let r = back[pick(seed, 0, 4, back.len())];
	ways.push([r, n - r]);
	joined[`0-${r}`] = true;
	let pairs = [];
	for a in 0..n {
		for b in (a + 1)..n {
			if !(`${a}-${b}` in joined) { pairs.push([a, b]); }
		}
	}
	pairs.sort(|p, q| sign(chance::of(seed, p[0] * n + p[1], 7) - chance::of(seed, q[0] * n + q[1], 7)));
	for p in pairs.extract(0, count - n) {
		let flip = chance::of(seed, p[0] * n + p[1], 8) < 0.5;
		let a = if flip { p[1] } else { p[0] };
		let b = if flip { p[0] } else { p[1] };
		ways.push([a, (b - a + n) % n]);
	}
	ways.sort(|v, w| if v[0] != w[0] { v[0] - w[0] } else { v[1] - w[1] });
	ways
}

// A whole number from 0 up to count, thing k's n-th choice under seed.
fn pick(seed, k, n, count) {
	min((chance::of(seed, k, n) * count).floor().to_int(), count - 1)
}

// -1, 0 or 1, as a sort's comparison wants.
fn sign(x) {
	if x < 0.0 { -1 } else if x > 0.0 { 1 } else { 0 }
}

// The chosen ways, checked: [from, steps] pairs, from a place, 1 to n - 1
// steps, no way twice, and every walk walkable.
fn check_chosen(chosen, n, walks) {
	let seen = #{};
	for w in chosen {
		if type_of(w) != "array" || w.len() != 2 || type_of(w[0]) != "i64" || type_of(w[1]) != "i64" {
			throw `modular_maze: chosen should be [from, steps] pairs of whole numbers, got ${w}`;
		}
		if w[0] < 0 || w[0] >= n { throw `modular_maze: a chosen way is from ${w[0]}, not one of the places, 0 to ${n - 1}`; }
		if w[1] < 1 || w[1] >= n { throw `modular_maze: a chosen way is ${w[1]} steps; ways are 1 to ${n - 1}`; }
		let key = `${w[0]}-${w[1]}`;
		if key in seen { throw `modular_maze: the way ${w} is chosen twice`; }
		seen[key] = true;
	}
	for total in walks {
		if !walkable(chosen, n, total) { throw `modular_maze: the chosen ways can't walk ${total}`; }
	}
	chosen
}

// Whether some ways from the start add up to total.
fn walkable(ways, n, total) {
	walks_to(ways, n, total)[total]
}

// For each t from 0 to total, whether some ways from the start add up to
// t. Every walk of t steps ends at t's remainder, so it's whether some
// way of s steps leaves that remainder of t - s, for a walk of t - s.
fn walks_to(ways, n, total) {
	let ok = [true];
	for t in 1..total + 1 {
		let reach = false;
		for w in ways {
			if w[1] <= t && ok[t - w[1]] && (t - w[1]) % n == w[0] { reach = true; break; }
		}
		ok.push(reach);
	}
	ok
}

// A way's text: template's {n} its steps, {word} them in words.
fn step_text(template, steps) {
	let words = [
		"zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
		"eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
	];
	let tens = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];
	let word = if steps < 20 { words[steps] } else if steps < 100 {
		tens[steps / 10] + (if steps % 10 == 0 { "" } else { "-" + words[steps % 10] })
	} else { `${steps}` };
	let text = template;
	text.replace("{n}", `${steps}`);
	text.replace("{word}", word);
	text
}

// Places to fill a map with until a journal gives its own.
fn placeholder_places() {
	[
		"The Gate", "Saint's Rest", "The Cloister", "The Black Stair", "Hollow Mere", "The Drowned Bell",
		"Lantern Hill", "The Tattered Hall", "Pale Orchard", "The Low Chapel", "Weeping Arch", "The Ossuary",
		"Salt Gallows", "Mourner's Ford", "The Long Pier", "Cinder Well", "Rook's Hollow", "The Quiet House",
		"Starfall Field", "The Ninth Door", "Ashen Road", "The Mask Maker's", "Twin Sun Inn", "The Still Lake",
		"Old Mill", "Crow Bridge", "The Last Lamp",
	]
}
