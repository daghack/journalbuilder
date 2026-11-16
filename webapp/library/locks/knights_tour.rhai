// knights_tour: the text written along a knight's tour of a board, a
// symbol to a square, the knight's every move two squares one way and
// one the other, never landing twice; then read off the board row by
// row. The tour is the key: draw it as the hunt's tracks, and the reader
// follows them square to square.
//
// The board is size squares across, or with size 0 the smallest from 5
// up that holds the text. The tour is found by the seed: a start square,
// then always the square with the fewest ways on (Warnsdorff's rule), the
// seed breaking ties. A text shorter than the board fills the tour's
// first squares; the rest stay empty and are skipped in reading (see
// lib/routes), so nothing is added.
//
// tour is a KnightsTour, #{ size, squares }: the squares in the order the
// knight takes them, each numbered row by row from 0 at the top left.

import "lib/routes" as routes;

fn options() {
	#{ base: integer(), size: integer(0), seed: integer(0) }
}

fn check(o) {
	if o.size != 0 && (o.size < 5 || o.size > 16) {
		throw `size must be 0 (the smallest that holds the text) or from 5 to 16, got ${o.size}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base) }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(o.base), tour: "KnightsTour" }
}

fn moves() {
	[[1, 2], [2, 1], [2, -1], [1, -2], [-1, -2], [-2, -1], [-2, 1], [-1, 2]]
}

// The squares a knight on `at` can reach that `seen` hasn't marked.
fn onward(at, size, seen) {
	let x = at % size;
	let y = at / size;
	let out = [];
	for m in moves() {
		let nx = x + m[0];
		let ny = y + m[1];
		if nx >= 0 && ny >= 0 && nx < size && ny < size && !seen[ny * size + nx] {
			out.push(ny * size + nx);
		}
	}
	out
}

// A tour of a board size across, found by Warnsdorff's rule from a start
// the seed picks, ties broken by the seed; tried again from elsewhere
// when it strands itself.
fn tour(size, seed) {
	let rng = new_random(seed);
	let cells = [];
	for c in 0..size * size { cells.push(c); }
	for attempt in 0..400 {
		let seen = [];
		seen.pad(size * size, false);
		let at = rng.pick(cells);
		seen[at] = true;
		let squares = [at];
		while squares.len() < size * size {
			let best = [];
			let fewest = 9;
			for next in onward(at, size, seen) {
				seen[next] = true;
				let ways = onward(next, size, seen).len();
				seen[next] = false;
				if ways < fewest {
					fewest = ways;
					best = [next];
				} else if ways == fewest {
					best.push(next);
				}
			}
			if best.is_empty() { break; }
			at = rng.pick(best);
			seen[at] = true;
			squares.push(at);
		}
		if squares.len() == size * size {
			return #{ size: size, squares: squares };
		}
	}
	throw `found no knight's tour of a board ${size} across`;
}

fn check_tour(t) {
	routes::check(t.squares, t.size * t.size, "the tour");
	for k in 1..t.squares.len() {
		let a = t.squares[k - 1];
		let b = t.squares[k];
		let dx = (a % t.size - b % t.size).abs();
		let dy = (a / t.size - b / t.size).abs();
		if !((dx == 1 && dy == 2) || (dx == 2 && dy == 1)) {
			throw `the tour's move ${k}, from square ${a} to ${b}, isn't a knight's`;
		}
	}
}

fn lock(o, i) {
	let text = i.plain_text;
	let size = o.size;
	if size == 0 {
		size = 5;
		while size * size < text.len() { size += 1; }
	}
	if text.len() > size * size {
		throw `the text, ${text.len()} symbols, is longer than a board ${size} across holds`;
	}
	let t = tour(size, o.seed);
	#{ cipher_text: routes::order(t.squares, text.len()).map(|k| text[k]), tour: t }
}

fn unlock(o, out) {
	check_tour(out.tour);
	let cipher = out.cipher_text;
	let plain = cipher.map(|s| 0);
	for (k, n) in routes::order(out.tour.squares, cipher.len()) {
		plain[k] = cipher[n];
	}
	#{ plain_text: plain }
}

fn examples() {
	[
		#{
			options: #{ base: 27, seed: 1 },
			inputs: #{ plain_text: symbols(27, "THE HUNT RIDES TONIGHT") },
		},
		#{
			// A board bigger than the text needs: its last squares empty.
			options: #{ base: 26, size: 6, seed: 3 },
			inputs: #{ plain_text: symbols(26, "FOLLOWTHETRACKS") },
		},
		#{
			// Longer than 36: a board 7 across.
			options: #{ base: 27, seed: 8 },
			inputs: #{ plain_text: symbols(27, "THE HOUNDS RUN WHERE THE WATER IS BLACK") },
		},
		#{
			options: #{ base: 10 },
			inputs: #{ plain_text: [] },
		},
		#{
			options: #{ base: 26, size: 5 },
			inputs: #{ plain_text: symbols(26, "ABCDEFGHIJKLMNOPQRSTUVWXYZ") },
			error: "longer than a board 5 across holds",
		},
		#{
			options: #{ base: 26, size: 4 },
			error: "size must be 0",
		},
	]
}
