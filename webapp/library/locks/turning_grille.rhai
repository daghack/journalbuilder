// turning_grille: a Fleissner grille. A square plate, size cells across
// (size even), pierced with a quarter of its cells, so that turned a
// quarter round three times its holes fall on every cell once. Laid over
// a square, the text is written through its holes, top row first and
// left to right; the plate is turned a quarter clockwise and the writing
// goes on, and again, and again. The square read row by row is the
// cipher text; laid over it and turned the same way, the plate gives the
// text back.
//
// The holes are picked by the seed, one of each four cells a turn takes
// round to each other. The plate is size across, or with size 0 the
// smallest from 4 that holds the text. A text shorter than the square
// fills the cells the holes reach first; the rest stay empty and are
// skipped in reading (see lib/routes), so nothing is added.
//
// grille is a TurningGrille, #{ size, holes }: the cells pierced as it
// first lies, each numbered row by row from 0 at the top left.

import "lib/routes" as routes;

fn options() {
	#{ base: integer(), size: integer(0), seed: integer(0) }
}

fn check(o) {
	if o.size != 0 && (o.size < 2 || o.size > 16 || o.size % 2 != 0) {
		throw `size must be 0 (the smallest that holds the text) or even, from 2 to 16, got ${o.size}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base) }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(o.base), grille: "TurningGrille" }
}

// Cell `cell` of a square size across, turned a quarter clockwise.
fn turned(cell, size) {
	let x = cell % size;
	let y = cell / size;
	x * size + (size - 1 - y)
}

// The cells the holes take, in writing order: each turn's holes row by
// row, the turns in order.
fn route(g) {
	let holes = g.holes;
	let route = [];
	for turn in 0..4 {
		let now = holes;
		now.sort();
		route += now;
		holes = holes.map(|cell| turned(cell, g.size));
	}
	route
}

fn lock(o, i) {
	let text = i.plain_text;
	let size = o.size;
	if size == 0 {
		size = 4;
		while size * size < text.len() { size += 2; }
	}
	if text.len() > size * size {
		throw `the text, ${text.len()} symbols, is longer than a grille ${size} across holds`;
	}
	let rng = new_random(o.seed);
	let holes = [];
	let half = size / 2;
	for y in 0..half {
		for x in 0..half {
			let cell = y * size + x;
			for k in 0..rng.pick([0, 1, 2, 3]) { cell = turned(cell, size); }
			holes.push(cell);
		}
	}
	let g = #{ size: size, holes: holes };
	#{ cipher_text: routes::order(route(g), text.len()).map(|k| text[k]), grille: g }
}

fn unlock(o, out) {
	let g = out.grille;
	if g.size < 2 || g.size % 2 != 0 {
		throw `the grille must be an even number of cells across, got ${g.size}`;
	}
	let r = route(g);
	routes::check(r, g.size * g.size, "the grille, turned four ways,");
	let cipher = out.cipher_text;
	let plain = cipher.map(|s| 0);
	for (k, n) in routes::order(r, cipher.len()) {
		plain[k] = cipher[n];
	}
	#{ plain_text: plain }
}

fn examples() {
	[
		#{
			options: #{ base: 27, seed: 2 },
			inputs: #{ plain_text: symbols(27, "TURN THE GUARD AND READ") },
		},
		#{
			// Exactly full: 36 symbols on a grille 6 across.
			options: #{ base: 26, size: 6, seed: 5 },
			inputs: #{ plain_text: symbols(26, "THEBLADEISTURNEDFOURTIMESINTHENIGHTX") },
		},
		#{
			options: #{ base: 10, size: 2 },
			inputs: #{ plain_text: symbols(10, "1234") },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: [] },
		},
		#{
			options: #{ base: 26, size: 5 },
			error: "size must be 0",
		},
	]
}
