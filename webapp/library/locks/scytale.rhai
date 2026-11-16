// scytale: a strip wound round a rod of faces sides, the text written
// along the rod a face at a time, then the strip unwound. Each turn of
// the strip crosses every face once, so the strip reads the faces' first
// symbols, then their second, and on. With 3 faces, SWORDS is written
//
//   face 1:  S W
//   face 2:  O R
//   face 3:  D S
//
// and the strip reads SOD WRS. The rod is as many turns long as the text
// needs; the last face is as short as the text leaves it, nothing added,
// and unlocking needs only the faces: wind the strip round a rod as
// thick.

fn options() {
	#{ base: integer(), faces: integer(4) }
}

fn check(o) {
	if o.faces < 2 {
		throw `the rod must have at least 2 faces, got ${o.faces}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base) }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(o.base) }
}

// Which place of the text each place of the strip comes from.
fn order(faces, count) {
	let turns = (count + faces - 1) / faces;
	let order = [];
	for t in 0..turns {
		for f in 0..faces {
			let k = f * turns + t;
			if k < count { order.push(k); }
		}
	}
	order
}

fn lock(o, i) {
	let text = i.plain_text;
	#{ cipher_text: order(o.faces, text.len()).map(|k| text[k]) }
}

fn unlock(o, out) {
	let cipher = out.cipher_text;
	let plain = cipher.map(|s| 0);
	for (k, n) in order(o.faces, cipher.len()) {
		plain[k] = cipher[n];
	}
	#{ plain_text: plain }
}

fn examples() {
	[
		#{
			options: #{ base: 26, faces: 3 },
			inputs: #{ plain_text: symbols(26, "SWORDS") },
			outputs: #{ cipher_text: symbols(26, "SODWRS") },
		},
		#{
			// The last face left empty.
			options: #{ base: 26 },
			inputs: #{ plain_text: symbols(26, "BLADES") },
			outputs: #{ cipher_text: symbols(26, "BAELDS") },
		},
		#{
			options: #{ base: 27, faces: 5 },
			inputs: #{ plain_text: symbols(27, "WIND IT ROUND THE HILT") },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: [] },
		},
		#{
			options: #{ base: 27, faces: 1 },
			error: "at least 2 faces",
		},
	]
}
