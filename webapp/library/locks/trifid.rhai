// trifid: Delastelle's cipher of the cube. The 27 symbols (space, A to
// Z) fill a cube three by three by three in key order, the key's
// symbols, each the first time it comes, then the rest in order: three
// squares, each three rows of three. Each symbol is its square, row and
// column, three digits 0 to 2. The text is taken period symbols at a
// time; a group's squares are written in a line, its rows under them and
// its columns under those, and the lines read straight across in threes,
// each three a square, a row and a column: a symbol of the cipher text.
// Every symbol of the cipher text so mixes three of the text's.
//
// With period 0 the whole text is one group. Unlocking needs the key and
// the period.

fn options() {
	#{ period: integer(5) }
}

fn check(o) {
	if o.period < 0 {
		throw `the period must be 0 (the whole text) or more, got ${o.period}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(27), key: symbol_type(27) }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(27), key: symbol_type(27) }
}

// The 27 symbols in key order: the cube, square by square, row by row.
fn cube(key) {
	let order = [];
	let all = [];
	for s in 0..27 { all.push(s); }
	for s in key + all {
		if !order.contains(s) { order.push(s); }
	}
	order
}

// Where each symbol sits in the cube.
fn places(order) {
	let at = [];
	at.pad(27, 0);
	for (s, n) in order { at[s] = n; }
	at
}

fn groups(count, period) {
	let p = if period == 0 { count } else { period };
	let out = [];
	let g = 0;
	while g < count {
		out.push([g, min(p, count - g)]);
		g += p;
	}
	out
}

fn lock(o, i) {
	let order = cube(i.key);
	let at = places(order);
	let text = i.plain_text;
	let cipher = [];
	for g in groups(text.len(), o.period) {
		let lines = [[], [], []];
		for s in text.extract(g[0], g[1]) {
			let n = at[s];
			lines[0].push(n / 9);
			lines[1].push(n / 3 % 3);
			lines[2].push(n % 3);
		}
		let digits = lines[0] + lines[1] + lines[2];
		for t in 0..g[1] {
			cipher.push(order[digits[3 * t] * 9 + digits[3 * t + 1] * 3 + digits[3 * t + 2]]);
		}
	}
	#{ cipher_text: cipher, key: i.key }
}

fn unlock(o, out) {
	let order = cube(out.key);
	let at = places(order);
	let cipher = out.cipher_text;
	let plain = [];
	for g in groups(cipher.len(), o.period) {
		let digits = [];
		for c in cipher.extract(g[0], g[1]) {
			let n = at[c];
			digits += [n / 9, n / 3 % 3, n % 3];
		}
		let w = g[1];
		for t in 0..w {
			plain.push(order[digits[t] * 9 + digits[w + t] * 3 + digits[2 * w + t]]);
		}
	}
	#{ plain_text: plain }
}

fn examples() {
	[
		#{
			options: #{ period: 5 },
			inputs: #{ plain_text: symbols(27, "THREE STONES OF BLACK GLASS"), key: symbols(27, "OBSIDIAN") },
		},
		#{
			// The whole text one group.
			options: #{ period: 0 },
			inputs: #{ plain_text: symbols(27, "CUBE"), key: symbols(27, "FELIX MARIE DELASTELLE") },
		},
		#{
			// No key: the cube in plain order. A is square 0 row 0
			// column 1, B 0 0 2: AB is 00 00 12, read 000 012, space and
			// E.
			options: #{ period: 0 },
			inputs: #{ plain_text: symbols(27, "AB"), key: [] },
			outputs: #{ cipher_text: symbols(27, " E") },
		},
		#{
			options: #{ period: 3 },
			inputs: #{ plain_text: [], key: symbols(27, "KEY") },
		},
		#{
			options: #{ period: -1 },
			error: "the period must be 0",
		},
	]
}
