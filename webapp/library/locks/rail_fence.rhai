// rail_fence: the text written in a zigzag, down and up across rails rails
// as a snake winds, then read off rail by rail. With 3 rails,
// WEAREDISCOVERED is written
//
//   W . . . E . . . C . . . R . .
//   . E . R . D . S . O . E . E .
//   . . A . . . I . . . V . . . D
//
// and read WECR, ERDSOEE, AIVD. offset starts the zigzag that many places
// along, so the first symbol needn't be on the top rail. Nothing is added:
// the cipher text is the plain text's symbols, rearranged, and unlocking
// needs only the rails and the offset.

fn options() {
	#{ base: integer(), rails: integer(3), offset: integer(0) }
}

fn check(o) {
	if o.rails < 2 {
		throw `there must be at least 2 rails, got ${o.rails}`;
	}
	if o.offset < 0 {
		throw `the offset must be 0 or more, got ${o.offset}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base) }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(o.base) }
}

// Which place of the cipher text each place of a text count long comes
// from: the places on the top rail in order, then the next rail's, and on.
fn order(o, count) {
	let round = 2 * (o.rails - 1);
	let rails = [];
	for r in 0..o.rails { rails.push([]); }
	for k in 0..count {
		let at = (k + o.offset) % round;
		rails[if at < o.rails { at } else { round - at }].push(k);
	}
	let order = [];
	for rail in rails { order += rail; }
	order
}

fn lock(o, i) {
	let text = i.plain_text;
	#{ cipher_text: order(o, text.len()).map(|k| text[k]) }
}

fn unlock(o, out) {
	let cipher = out.cipher_text;
	let plain = cipher.map(|s| 0);
	for (k, n) in order(o, cipher.len()) {
		plain[k] = cipher[n];
	}
	#{ plain_text: plain }
}

fn examples() {
	[
		#{
			options: #{ base: 26 },
			inputs: #{ plain_text: symbols(26, "WEAREDISCOVERED") },
			outputs: #{ cipher_text: symbols(26, "WECRERDSOEEAIVD") },
		},
		#{
			options: #{ base: 27, rails: 2 },
			inputs: #{ plain_text: symbols(27, "SERPENT") },
			outputs: #{ cipher_text: symbols(27, "SRETEPN") },
		},
		#{
			// Started one along: the first symbol on the second rail.
			options: #{ base: 26, rails: 4, offset: 1 },
			inputs: #{ plain_text: symbols(26, "THESNAKEWINDSDOWNANDUP") },
		},
		#{
			// More rails than symbols: the text as it was.
			options: #{ base: 10, rails: 9 },
			inputs: #{ plain_text: symbols(10, "314") },
			outputs: #{ cipher_text: symbols(10, "314") },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: [] },
		},
		#{
			options: #{ base: 27, rails: 1 },
			error: "there must be at least 2 rails, got 1",
		},
	]
}
