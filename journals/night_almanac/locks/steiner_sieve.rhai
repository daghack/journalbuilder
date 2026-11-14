// steiner_sieve: a text hidden in a heap of cards, some of them forgeries.
// The text is 26 symbols, one for each triple of the Steiner system
// S(2, 3, 13) (lib/steiner): thirteen points, 0 to 12, in 26 triples such
// that every two points are in exactly one. Each card names three points,
// a letter and a place: the 26 true cards are the system's triples,
// relabelled by a secret shuffle of the points, each holding the letter
// that stands at its place in the text; the forgeries, `decoys` of them,
// are triples that aren't the system's, with letters and places of their
// own, so two cards claim the same place. A place is the symbol that
// reads, as a balanced-ternary number (the ternary symbol set), the
// place's number from -13: the 26 places are -13 to 12, so the places
// read in order of number are the text in order.
//
// To read it, find the cards among them that cover every pair of points
// exactly once, and read their letters by place. The true cards are the
// only such set (lock re-draws its forgeries till they are).
//
// The text is base 27 (space, A to Z) and exactly 26 symbols; the cards,
// shuffled, are base 27 symbols too, five to a card: three points, the
// letter and the place (the place 0 to 25 as the symbol (place + 14) % 27).

import "lib/steiner" as steiner;
import "lib/chance" as chance;

fn options() {
	#{ seed: integer(0), decoys: integer(10) }
}

fn check(o) {
	if o.decoys < 0 || o.decoys > 60 {
		throw `decoys must be from 0 to 60, got ${o.decoys}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(27) }
}

fn outputs(o) {
	#{ cards: symbol_type(27) }
}

// A shuffle of 0 to count - 1 by seed.
fn shuffled(count, seed, n) {
	let out = [];
	for k in 0..count { out.push(k); }
	for k in 1..count {
		let j = count - k;
		let at = (chance::of(seed, j, n) * (j + 1)).to_int();
		let held = out[j];
		out[j] = out[at];
		out[at] = held;
	}
	out
}

// A triple's points in order, smallest first.
fn sorted(t) {
	let a = [t[0], t[1], t[2]];
	a.sort(|x, y| x - y);
	a
}

fn lock(o, i) {
	let text = i.plain_text;
	if text.len() != 26 {
		throw `steiner_sieve: the text needs 26 symbols, one for each triple, got ${text.len()}`;
	}
	let relabel = shuffled(13, o.seed, 100);
	let places = shuffled(26, o.seed, 101);
	let real = [];
	for (t, n) in steiner::triples() {
		real.push(sorted([relabel[t[0]], relabel[t[1]], relabel[t[2]]]));
	}
	let cards = [];
	for n in 0..26 {
		cards.push(real[n] + [text[places[n]], (places[n] + 14) % 27]);
	}
	// Forgeries, till the true cards are the only way round.
	let tries = 0;
	let heap = [];
	while true {
		heap = cards;
		let seed = o.seed * 1000 + tries;
		let made = 0;
		let k = 0;
		while made < o.decoys {
			k += 1;
			let t = sorted([
				(chance::of(seed, k, 110) * 13).to_int(),
				(chance::of(seed, k, 111) * 13).to_int(),
				(chance::of(seed, k, 112) * 13).to_int(),
			]);
			if t[0] == t[1] || t[1] == t[2] { continue; }
			let dup = false;
			for c in heap { if c[0] == t[0] && c[1] == t[1] && c[2] == t[2] { dup = true; break; } }
			if dup { continue; }
			let letter = 1 + (chance::of(seed, k, 113) * 26).to_int();
			let place = (chance::of(seed, k, 114) * 26).to_int();
			heap.push(t + [letter, (place + 14) % 27]);
			made += 1;
		}
		if covers(heap, 2).len() == 1 { break; }
		tries += 1;
		if tries > 200 { throw "steiner_sieve: no forgeries leave the true cards alone; try another seed"; }
	}
	// The heap in a shuffled order.
	let order = shuffled(heap.len(), o.seed, 102);
	let out = [];
	for n in order { out += heap[n]; }
	#{ cards: out }
}

fn unlock(o, out) {
	let given = out.cards;
	if given.len() % 5 != 0 {
		throw `${given.len()} symbols aren't a whole number of cards, five to a card`;
	}
	let heap = [];
	for k in 0..given.len() / 5 {
		let c = given.extract(k * 5, 5);
		for p in 0..3 {
			if c[p] > 12 { throw `card ${k} has a point ${c[p]}: the points are 0 to 12`; }
		}
		if c[0] == c[1] || c[1] == c[2] || c[0] == c[2] { throw `card ${k}'s three points aren't different: ${c[0]}, ${c[1]}, ${c[2]}`; }
		if c[4] == 13 { throw `card ${k} has the place number 13: the places are -13 to 12`; }
		heap.push(sorted(c) + [c[3], c[4]]);
	}
	let found = covers(heap, 2);
	if found.len() == 0 { throw "no cards cover every pair of the thirteen points exactly once"; }
	if found.len() > 1 { throw "more than one set of the cards covers every pair of the thirteen points exactly once"; }
	let text = [];
	text.pad(26, 0);
	for k in found[0] {
		text[(heap[k][4] + 13) % 27] = heap[k][3];
	}
	#{ plain_text: text }
}

fn examples() {
	[
		#{
			options: #{ seed: 3 },
			inputs: #{ plain_text: symbols(27, "COUNT THE HOUSES AGAIN NOW") },
		},
		#{
			options: #{ seed: 8, decoys: 24 },
			inputs: #{ plain_text: symbols(27, "THE THIRTEEN MEET IN THREE") },
		},
		#{
			options: #{ decoys: 61 },
			error: "decoys must be from 0 to 60, got 61",
		},
		#{
			options: #{},
			inputs: #{ plain_text: symbols(27, "TOO SHORT") },
			error: "steiner_sieve: the text needs 26 symbols, one for each triple, got 9",
		},
	]
}

// ---- finding the true cards ----

// Whether the triple c has the point p.
fn has(c, p) {
	c[0] == p || c[1] == p || c[2] == p
}

// Which pair of two points (a before b): its place in a table of covered
// pairs.
fn pair(a, b) {
	a * 13 + b
}

// Every set of cards (as arrays of their places in `heap`) covering every
// pair of the thirteen points exactly once, up to `limit` of them: the
// uncovered pair with fewest cards through it first, as exact cover's
// algorithm X has it.
fn covers(heap, limit) {
	let covered = [];
	covered.pad(169, false);
	search(heap, covered, [], limit)
}

fn search(heap, covered, chosen, limit) {
	let best = -1;
	let options = [];
	for a in 0..13 {
		for b in a + 1..13 {
			if covered[pair(a, b)] { continue; }
			let through = [];
			for k in 0..heap.len() {
				let c = heap[k];
				if has(c, a) && has(c, b) && !covered[pair(c[0], c[1])] && !covered[pair(c[0], c[2])] && !covered[pair(c[1], c[2])] {
					through.push(k);
				}
			}
			if through.len() == 0 { return []; }
			if best < 0 || through.len() < options.len() {
				best = pair(a, b);
				options = through;
			}
		}
	}
	if best < 0 { return [chosen]; }
	let found = [];
	for k in options {
		let c = heap[k];
		let next = covered;
		next[pair(c[0], c[1])] = true;
		next[pair(c[0], c[2])] = true;
		next[pair(c[1], c[2])] = true;
		found += search(heap, next, chosen + [k], limit - found.len());
		if found.len() >= limit { break; }
	}
	found
}
