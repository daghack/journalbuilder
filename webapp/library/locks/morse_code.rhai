// morse_code: each symbol as its Morse code, dots and dashes, ended by a
// gap. The marks are base 3: 0 a gap, 1 a dot, 2 a dash. A space (base
// 27) has no dots or dashes, only its gap, so a word ends in two gaps
// together. SOS is ... --- ..., marked 1110 2220 1110.
//
// The text is base 27 (space, A to Z), 26 (A to Z) or 10 (digits).
// Cut them anywhere: stabs and slashes in wood, short and long notches,
// knocks and scrapes.

fn options() {
	#{ base: integer(27) }
}

fn check(o) {
	if o.base != 10 && o.base != 26 && o.base != 27 {
		throw `base must be 10 (digits), 26 (A to Z) or 27 (space, A to Z), got ${o.base}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base) }
}

fn outputs(o) {
	#{ marks: symbol_type(3) }
}

// Each symbol's code, dots and dashes as . and -.
fn codes(base) {
	let letters = [".-", "-...", "-.-.", "-..", ".", "..-.", "--.", "....", "..", ".---", "-.-", ".-..", "--",
		"-.", "---", ".--.", "--.-", ".-.", "...", "-", "..-", "...-", ".--", "-..-", "-.--", "--.."];
	let digits = ["-----", ".----", "..---", "...--", "....-", ".....", "-....", "--...", "---..", "----."];
	if base == 10 { digits } else if base == 26 { letters } else { [""] + letters }
}

fn lock(o, i) {
	let codes = codes(o.base);
	let marks = [];
	for s in i.plain_text {
		for ch in codes[s].chars() {
			marks.push(if ch == '.' { 1 } else { 2 });
		}
		marks.push(0);
	}
	#{ marks: marks }
}

fn unlock(o, out) {
	let codes = codes(o.base);
	let text = [];
	let code = "";
	for m in out.marks {
		if m == 0 {
			let s = codes.index_of(code);
			if s < 0 {
				throw if code == "" { "two gaps together, where there's no space" } else { `'${code}' is no symbol's code` };
			}
			text.push(s);
			code = "";
		} else {
			code += if m == 1 { "." } else { "-" };
		}
	}
	if code != "" {
		throw `the marks end in the middle of a code, '${code}'`;
	}
	#{ plain_text: text }
}

fn examples() {
	[
		#{
			options: #{ base: 26 },
			inputs: #{ plain_text: symbols(26, "SOS") },
			outputs: #{ marks: [1, 1, 1, 0, 2, 2, 2, 0, 1, 1, 1, 0] },
		},
		#{
			// A space: a gap alone, so two gaps together.
			options: #{ base: 27 },
			inputs: #{ plain_text: symbols(27, "A B") },
			outputs: #{ marks: [1, 2, 0, 0, 2, 1, 1, 1, 0] },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: symbols(27, " CUT DEEP ") },
		},
		#{
			options: #{ base: 10 },
			inputs: #{ plain_text: symbols(10, "1907") },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: [] },
		},
		#{
			options: #{ base: 13 },
			error: "base must be 10",
		},
	]
}
