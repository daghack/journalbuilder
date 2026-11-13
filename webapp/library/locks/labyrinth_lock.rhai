// labyrinth_lock: the text hidden along the one way through a round
// labyrinth, a symbol to a cell from the entrance in to the heart, and
// every other cell given a symbol picked from the text's, to mislead.
// Unlocking walks the way and reads it.
//
// The labyrinth is rings of cells round its heart (lib/labyrinth), laid
// out by the seed: as many rings as it takes for the way to hold the
// text, and at least rings. Where the way is longer than the text, its
// last cells, nearest the heart, hold 0, and text_length lets unlocking
// drop them again.

import "lib/labyrinth" as lab;

fn options() {
	#{ base: integer(), seed: integer(0), rings: integer(4) }
}

fn check(o) {
	if o.rings < 2 || o.rings > 16 {
		throw `rings must be from 2 to 16, got ${o.rings}`;
	}
}

fn inputs(o) {
	#{ text: symbol_type(o.base) }
}

fn outputs(o) {
	#{ labyrinth: "Labyrinth", text_length: "LockProcessInfo" }
}

fn lock(o, i) {
	let text = i.text;
	let rings = o.rings;
	loop {
		let l = grown(o.base, rings, o.seed);
		// The entrance: the outermost cell whose way to the heart is the
		// shortest that holds the text.
		let entrance = -1;
		for (far, s) in l.far[rings - 1] {
			if far >= text.len() && (entrance < 0 || far < l.far[rings - 1][entrance]) { entrance = s; }
		}
		if entrance >= 0 {
			l.remove("far");
			l.entrance = entrance;
			// Misleading symbols everywhere, then the text along the way.
			let rng = new_random(o.seed + 1);
			for k in 0..rings {
				for s in 0..l.sectors[k] {
					l.symbols[k][s] = if text.is_empty() { 0 } else { rng.pick(text) };
				}
			}
			for (cell, n) in lab::path(l) {
				l.symbols[cell[0]][cell[1]] = if n < text.len() { text[n] } else { 0 };
			}
			return #{ labyrinth: l, text_length: text.len() };
		}
		rings += 1;
		if rings > 16 {
			throw `the text, ${text.len()} symbols, is too long for a labyrinth of 16 rings`;
		}
	}
}

fn unlock(o, out) {
	let l = out.labyrinth;
	lab::check(l);
	let text = [];
	for cell in lab::path(l) {
		text.push(l.symbols[cell[0]][cell[1]]);
	}
	if out.text_length > text.len() {
		throw `the labyrinth's way is ${text.len()} cells, too short for a text of ${out.text_length}`;
	}
	text.truncate(out.text_length);
	#{ text: text }
}

// A labyrinth of `rings` rings with no entrance yet, grown from the cell
// at its heart by a walk that turns at random and backs up from dead
// ends, opening a wall wherever it goes on: every cell is reached, one
// way only. Its far is how many cells the way from each cell to the heart
// passes, both ends counted.
fn grown(base, rings, seed) {
	let counts = lab::sectors(rings);
	let rng = new_random(seed);
	let blank = |value| counts.map(|n| {
		let ring = [];
		for s in 0..n { ring.push(value); }
		ring
	});
	let inward = blank.call(false);
	let clockwise = blank.call(false);
	let far = blank.call(0);
	let heart = min((rng.next() * counts[0]).to_int(), counts[0] - 1);
	far[0][heart] = 1;
	let walk = [[0, heart]];
	while !walk.is_empty() {
		let k = walk[walk.len() - 1][0];
		let s = walk[walk.len() - 1][1];
		let n = counts[k];
		// The cells beside this one that the walk hasn't reached, and the
		// wall to open for each: [ring, cell, the wall's ring, its cell, inward].
		let ways = [];
		if k > 0 { ways.push([k - 1, lab::inside(counts, k, s), k, s, true]); }
		if k < rings - 1 {
			for t in lab::outside(counts, k, s) { ways.push([k + 1, t, k + 1, t, true]); }
		}
		ways.push([k, (s + 1) % n, k, s, false]);
		ways.push([k, (s + n - 1) % n, k, (s + n - 1) % n, false]);
		ways = ways.filter(|w| far[w[0]][w[1]] == 0);
		if ways.is_empty() {
			walk.pop();
			continue;
		}
		let way = rng.pick(ways);
		if way[4] { inward[way[2]][way[3]] = true; } else { clockwise[way[2]][way[3]] = true; }
		far[way[0]][way[1]] = far[k][s] + 1;
		walk.push([way[0], way[1]]);
	}
	#{
		base: base, sectors: counts, entrance: 0, heart: heart,
		inward: inward, clockwise: clockwise, symbols: blank.call(0), far: far,
	}
}

fn examples() {
	[
		#{
			options: #{ base: 27 },
			inputs: #{ text: symbols(27, "THE HEART OF THE MAZE") },
			outputs: #{ text_length: 21 },
		},
		#{
			options: #{ base: 27, seed: 9, rings: 2 },
			inputs: #{ text: symbols(27, "IN") },
		},
		#{
			// Long enough to need more rings than it asks for.
			options: #{ base: 26, seed: 4 },
			inputs: #{ text: symbols(26, "ITWINDSANDITWINDSANDATITSHEARTSOMETHINGWAITSFORWHOEVERCOMESTOREADTHIS") },
		},
		#{
			options: #{ base: 10, seed: 2 },
			inputs: #{ text: [] },
		},
		#{
			options: #{ base: 27, rings: 1 },
			error: "rings must be from 2 to 16, got 1",
		},
	]
}
