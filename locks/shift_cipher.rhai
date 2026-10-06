// shift_cipher: every symbol moved `shift` places along the base, wrapping
// around (a Caesar cipher). A negative shift moves the other way.

fn options() {
	#{ base: integer(), shift: integer() }
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base) }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(o.base) }
}

fn lock(o, i) {
	#{ cipher_text: shifted(i.plain_text, o.shift, o.base) }
}

fn unlock(o, out) {
	#{ plain_text: shifted(out.cipher_text, -o.shift, o.base) }
}

// Each symbol moved `by` places, wrapping around within the base.
fn shifted(symbols, by, base) {
	symbols.map(|s| ((s + by) % base + base) % base)
}

fn examples() {
	[
		#{
			options: #{ base: 27, shift: 3 },
			inputs: #{ plain_text: symbols(27, "HELLO WORLD") },
			outputs: #{ cipher_text: symbols(27, "KHOORCZRUOG") },
		},
		#{
			// ROT13, with A as 0.
			options: #{ base: 26, shift: 13 },
			inputs: #{ plain_text: symbols(26, "HELLO") },
			outputs: #{ cipher_text: symbols(26, "URYYB") },
		},
		#{
			options: #{ base: 27, shift: -30 },
			inputs: #{ plain_text: symbols(27, "WRAP AROUND") },
		},
		#{
			options: #{ base: 27, shift: 1.5 },
			inputs: #{ plain_text: symbols(27, "HI") },
			error: "option 'shift' should be an integer, got a number",
		},
	]
}
