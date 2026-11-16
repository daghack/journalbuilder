// baconian: each symbol as a group of marks of a few kinds, as Bacon hid
// letters in a text written in two founts. With 2 kinds, a letter of 26
// is five marks, A 00000, B 00001 and on, its number in base 2; with 3
// kinds, a symbol of 27 is three. The groups are as wide as the kinds
// need to number every symbol: 5 for 26 or 27 symbols in 2 kinds, 3 in
// 3 kinds.
//
// The marks are meant to be hidden: each one a letter of a cover text
// written plain or ornate, upright or leaning, struck or whole; or
// notches of two or three depths on a blade. Unlocking reads the groups
// off again.

fn options() {
	#{ base: integer(27), kinds: integer(2) }
}

fn check(o) {
	if o.kinds < 2 || o.kinds > o.base {
		throw `kinds must be from 2 to the base, ${o.base}, got ${o.kinds}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base) }
}

fn outputs(o) {
	#{ marks: symbol_type(o.kinds) }
}

// How many marks a group takes: enough to number base symbols.
fn width(o) {
	let w = 1;
	let reach = o.kinds;
	while reach < o.base {
		reach *= o.kinds;
		w += 1;
	}
	w
}

fn lock(o, i) {
	let w = width(o);
	let marks = [];
	for s in i.plain_text {
		let group = [];
		group.pad(w, 0);
		let n = s;
		for k in 0..w {
			group[w - 1 - k] = n % o.kinds;
			n /= o.kinds;
		}
		marks += group;
	}
	#{ marks: marks }
}

fn unlock(o, out) {
	let w = width(o);
	let marks = out.marks;
	if marks.len() % w != 0 {
		throw `the marks, ${marks.len()} of them, don't make whole groups of ${w}`;
	}
	let text = [];
	for g in 0..marks.len() / w {
		let n = 0;
		for k in 0..w { n = n * o.kinds + marks[g * w + k]; }
		if n >= o.base {
			throw `group ${g + 1} numbers ${n}, beyond the ${o.base} symbols`;
		}
		text.push(n);
	}
	#{ plain_text: text }
}

fn examples() {
	[
		#{
			options: #{ base: 26 },
			inputs: #{ plain_text: symbols(26, "BACON") },
			outputs: #{ marks: [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 1] },
		},
		#{
			// Three kinds: three marks a symbol.
			options: #{ base: 27, kinds: 3 },
			inputs: #{ plain_text: symbols(27, "HI") },
			outputs: #{ marks: [0, 2, 2, 1, 0, 0] },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: symbols(27, "HIDDEN IN PLAIN HAND") },
		},
		#{
			options: #{ base: 10, kinds: 3 },
			inputs: #{ plain_text: symbols(10, "90210") },
		},
		#{
			options: #{ base: 27, kinds: 1 },
			error: "kinds must be from 2",
		},
	]
}
