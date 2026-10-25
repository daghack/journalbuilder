// string_to_symbols: a string as symbols in the base, as symbols(base,
// text) reads it; with lenient, any text is made to fit the base first.
// Unlocking writes the symbols back out as text, which for lenient text is
// the text as it was fitted.

fn options() {
	#{ base: integer(), lenient: bool(false) }
}

fn check(o) {
	if o.base < 2 || o.base > 27 {
		throw `base must be from 2 to 27, got ${o.base}`;
	}
}

fn inputs(o) {
	#{ text: "String" }
}

fn outputs(o) {
	#{ symbols: symbol_type(o.base) }
}

fn lock(o, i) {
	#{ symbols: symbols(o.base, i.text, #{ lenient: o.lenient }) }
}

fn unlock(o, out) {
	#{ text: symbol_text(o.base, out.symbols) }
}

fn examples() {
	[
		#{
			options: #{ base: 27 },
			inputs: #{ text: "HELLO WORLD" },
			outputs: #{ symbols: symbols(27, "HELLO WORLD") },
		},
		#{
			options: #{ base: 10 },
			inputs: #{ text: "1084" },
			outputs: #{ symbols: [1, 0, 8, 4] },
		},
		#{
			options: #{ base: 27, lenient: true },
			inputs: #{ text: "EVERY LOCK OPENS" },
			outputs: #{ symbols: symbols(27, "EVERY LOCK OPENS") },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ text: "Hello, world" },
			error: "symbols",
		},
		#{
			options: #{ base: 40 },
			error: "base must be from 2 to 27, got 40",
		},
	]
}
