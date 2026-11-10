// symbols_to_string: symbols in the base written out as a string, a letter
// each: in bases up to 26 the base's first letters, A for 0 (base 10 too:
// A to J, not digits); in base 27 a space for 0, then A to Z. Unlocking
// reads the string back, its letters in either case.

fn options() {
	#{ base: integer() }
}

fn check(o) {
	if o.base < 2 || o.base > 27 {
		throw `base must be from 2 to 27, got ${o.base}`;
	}
}

fn inputs(o) {
	#{ symbols: symbol_type(o.base) }
}

fn outputs(o) {
	#{ text: "String" }
}

// The base's characters, symbol 0's first.
fn alphabet(base) {
	let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
	if base == 27 { " " + letters } else { letters.sub_string(0, base) }
}

fn lock(o, i) {
	let letters = alphabet(o.base);
	let text = "";
	for s in i.symbols {
		text += letters[s];
	}
	#{ text: text }
}

fn unlock(o, out) {
	let letters = alphabet(o.base);
	let symbols = [];
	for c in out.text.to_upper().chars() {
		let s = letters.index_of(c);
		if s < 0 {
			let help = if o.base == 27 { "letters and spaces" } else { `letters A-${letters[o.base - 1]}` };
			throw `'${c}' isn't a base-${o.base} symbol (${help})`;
		}
		symbols.push(s);
	}
	#{ symbols: symbols }
}

fn examples() {
	[
		#{
			options: #{ base: 27 },
			inputs: #{ symbols: [8, 5, 12, 12, 15, 0, 23, 15, 18, 12, 4] },
			outputs: #{ text: "HELLO WORLD" },
		},
		#{
			options: #{ base: 26 },
			inputs: #{ symbols: [0, 1, 2, 25] },
			outputs: #{ text: "ABCZ" },
		},
		#{
			options: #{ base: 10 },
			inputs: #{ symbols: [1, 0, 8, 4] },
			outputs: #{ text: "BAIE" },
		},
		#{
			options: #{ base: 2 },
			inputs: #{ symbols: [] },
			outputs: #{ text: "" },
		},
		#{
			options: #{ base: 1 },
			error: "base must be from 2 to 27, got 1",
		},
	]
}
