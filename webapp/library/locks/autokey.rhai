// autokey: Vigenère's own cipher, keyed by a primer and then by the text
// itself: each symbol shifted by the key's, the key the primer followed
// by the text. Under the primer QUEENLY, ATTACKATDAWN is shifted by
// QUEENLYATTAC and becomes QNXEPVYTWTWP. Unlocking needs the primer;
// each symbol it gives back keys one further on.

fn options() {
	#{ base: integer() }
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base), key: symbol_type(o.base) }
}

fn outputs(o) {
	// The primer is passed through: unlocking needs it.
	#{ cipher_text: symbol_type(o.base), key: symbol_type(o.base) }
}

fn lock(o, i) {
	if i.key.is_empty() {
		throw "the key is empty";
	}
	let stream = i.key + i.plain_text;
	let cipher = [];
	for (s, k) in i.plain_text {
		cipher.push((s + stream[k]) % o.base);
	}
	#{ cipher_text: cipher, key: i.key }
}

fn unlock(o, out) {
	let key = out.key;
	if key.is_empty() {
		throw "the key is empty";
	}
	let plain = [];
	for (c, k) in out.cipher_text {
		let by = if k < key.len() { key[k] } else { plain[k - key.len()] };
		plain.push(((c - by) % o.base + o.base) % o.base);
	}
	#{ plain_text: plain }
}

fn examples() {
	[
		#{
			options: #{ base: 26 },
			inputs: #{ plain_text: symbols(26, "ATTACKATDAWN"), key: symbols(26, "QUEENLY") },
			outputs: #{ cipher_text: symbols(26, "QNXEPVYTWTWP") },
		},
		#{
			// A primer longer than the text.
			options: #{ base: 27 },
			inputs: #{ plain_text: symbols(27, "RUN"), key: symbols(27, "BLOOD IN THE WATER") },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: symbols(27, "IT FOLLOWS ITS OWN TRAIL"), key: symbols(27, "HOUND") },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: symbols(27, "NO PRIMER"), key: [] },
			error: "the key is empty",
		},
	]
}
