// steiner_triples: each letter written as points of the Steiner triple
// system S(2, 3, 13) (lib/steiner): thirteen points, 0 to 12, whose 26
// triples are the 26 letters, A the triple 0 1 4, B 1 2 5 and on round
// the thirteen to M, then N the triple 0 2 7, O 1 3 8 and on to Z. Any
// two points are together in one triple only, so two are enough to name
// a letter.
//
// With points 2 a letter is two of its triple's three points, and with
// points 3 all three; which two, and in what order, the seed picks, so a
// letter is written six ways through a text. A space is one point over
// again, any of the thirteen (two points alike are in no triple).
// Unlocking finds the triple each two points are in; given three, they
// must be a triple.
//
// The text is base 27 (space, A to Z) or 26 (A to Z); the points are
// base 13, the thirteen_black_priests glyphset's digits.

import "lib/steiner" as steiner;

fn options() {
	#{ base: integer(27), points: integer(2), seed: integer(0) }
}

fn check(o) {
	if o.base != 26 && o.base != 27 {
		throw `base must be 26 (A to Z) or 27 (space, A to Z), got ${o.base}`;
	}
	if o.points != 2 && o.points != 3 {
		throw `points must be 2 or 3, got ${o.points}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base) }
}

fn outputs(o) {
	#{ points: symbol_type(13) }
}

fn lock(o, i) {
	let triples = steiner::triples();
	let rng = new_random(o.seed);
	let orders = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
	let points = [];
	for s in i.plain_text {
		let letter = s - (o.base - 26);
		if letter < 0 {
			// A space: one point, over again.
			let point = rng.pick(triples)[0];
			for k in 0..o.points { points.push(point); }
			continue;
		}
		let order = rng.pick(orders);
		for k in 0..o.points { points.push(triples[letter][order[k]]); }
	}
	#{ points: points }
}

fn unlock(o, out) {
	let pairs = steiner::pairs();
	let triples = steiner::triples();
	let given = out.points;
	if given.len() % o.points != 0 {
		throw `${given.len()} points aren't a whole number of letters, ${o.points} points each`;
	}
	let plain = [];
	for k in 0..given.len() / o.points {
		let these = given.extract(k * o.points, o.points);
		let triple = pairs[these[0]][these[1]];
		if triple < 0 {
			if o.points == 3 && these[2] != these[0] {
				throw `the points at ${k * o.points}, ${these}, aren't a triple, nor one point for a space`;
			}
			if o.base == 26 { throw `the points at ${k * o.points}, ${these}, are a space, which base 26 hasn't`; }
			plain.push(0);
			continue;
		}
		if o.points == 3 && !(these[2] in triples[triple]) || o.points == 3 && these[2] in [these[0], these[1]] {
			throw `the points at ${k * o.points}, ${these}, aren't a triple: ${these[0]} and ${these[1]} go with ${triples[triple][0] + triples[triple][1] + triples[triple][2] - these[0] - these[1]}`;
		}
		plain.push(triple + (o.base - 26));
	}
	#{ plain_text: plain }
}

fn examples() {
	[
		#{
			options: #{ base: 26, points: 3 },
			inputs: #{ plain_text: symbols(26, "ALMANAC") },
		},
		#{
			options: #{ seed: 5 },
			inputs: #{ plain_text: symbols(27, "THE THIRTEEN MEET IN THREES AND NEVER TWICE") },
		},
		#{
			options: #{ points: 3, seed: 2 },
			inputs: #{ plain_text: symbols(27, "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG") },
		},
		#{
			options: #{},
			inputs: #{ plain_text: [] },
			outputs: #{ points: [] },
		},
		#{
			options: #{ points: 4 },
			error: "points must be 2 or 3, got 4",
		},
		#{
			options: #{ base: 13 },
			error: "base must be 26 (A to Z) or 27 (space, A to Z), got 13",
		},
	]
}
