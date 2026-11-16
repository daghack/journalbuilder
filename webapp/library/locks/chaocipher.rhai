// chaocipher: Byrne's two wheels, left and right, each a ring of all the
// base's symbols. A symbol of the text is found on the right wheel and
// the left wheel's symbol beside it is written; then both wheels are
// stirred, so no symbol is written the same way twice running: the left
// turned till the written symbol is at the top and the next one round
// taken out and put back halfway round (the nadir), the right turned till
// the text's symbol is at the top and one further, and the symbol two on
// taken out and put back at the nadir.
//
// The wheels as they start are the key: left and right, each holding
// every symbol once, top first and round. Unlocking finds each symbol on
// the left wheel instead and reads the right, stirring them alike.

import "lib/permutations" as perm;

fn options() {
	#{ base: integer(26) }
}

fn check(o) {
	if o.base < 4 {
		throw `base must be 4 or more, for wheels to stir, got ${o.base}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base), left: symbol_type(o.base), right: symbol_type(o.base) }
}

fn outputs(o) {
	// The wheels as they start are passed through: unlocking needs them.
	#{ cipher_text: symbol_type(o.base), left: symbol_type(o.base), right: symbol_type(o.base) }
}

// `wheel` turned so its place j is at the top.
fn turn(wheel, j) {
	wheel.extract(j) + wheel.extract(0, j)
}

// Both wheels stirred after the symbols at place j were used.
fn stir(left, right, j, nadir) {
	let l = turn(left, j);
	let x = l.remove(1);
	l.insert(nadir, x);
	let r = turn(turn(right, j), 1);
	let y = r.remove(2);
	r.insert(nadir, y);
	[l, r]
}

fn lock(o, i) {
	perm::check(i.left, o.base, "the left wheel");
	perm::check(i.right, o.base, "the right wheel");
	let left = i.left;
	let right = i.right;
	let cipher = [];
	for p in i.plain_text {
		let j = right.index_of(p);
		cipher.push(left[j]);
		let w = stir(left, right, j, o.base / 2);
		left = w[0];
		right = w[1];
	}
	#{ cipher_text: cipher, left: i.left, right: i.right }
}

fn unlock(o, out) {
	perm::check(out.left, o.base, "the left wheel");
	perm::check(out.right, o.base, "the right wheel");
	let left = out.left;
	let right = out.right;
	let plain = [];
	for c in out.cipher_text {
		let j = left.index_of(c);
		plain.push(right[j]);
		let w = stir(left, right, j, o.base / 2);
		left = w[0];
		right = w[1];
	}
	#{ plain_text: plain }
}

fn examples() {
	[
		#{
			// Rubin's worked example.
			options: #{ base: 26 },
			inputs: #{
				plain_text: symbols(26, "WELLDONEISBETTERTHANWELLSAID"),
				left: symbols(26, "HXUCZVAMDSLKPEFJRIGTWOBNYQ"),
				right: symbols(26, "PTLNBQDEOYSFAVZKGJRIHWXUMC"),
			},
			outputs: #{ cipher_text: symbols(26, "OAHQHCNYNXTSZJRRHJBYHQKSOUJY") },
		},
		#{
			options: #{ base: 27 },
			inputs: #{
				plain_text: symbols(27, "THE WATER TURNS AND TURNS"),
				left: symbols(27, "MAELSTROWBCDFGHIJKNPQUVXYZ "),
				right: symbols(27, "WHIRLPOOLABCDEFGJKMNQSTUVXY"),
			},
			error: "repeats",
		},
		#{
			options: #{ base: 27 },
			inputs: #{
				plain_text: symbols(27, "THE WATER TURNS AND TURNS"),
				left: symbols(27, "MAELSTROWBCDFGHIJKNPQUVXYZ "),
				right: symbols(27, "WHIRLPO ABCDEFGJKMNQSTUVXYZ"),
			},
		},
		#{
			options: #{ base: 3 },
			error: "base must be 4",
		},
	]
}
