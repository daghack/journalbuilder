// columnar_transposition: the text written in rows under the key, a
// column to each of the key's symbols, then read off column by column,
// the columns taken in the order of their key symbols (the lowest first;
// alike, the leftmost first). Under the key WEB, SPIDERS is written
//
//   W E B
//   S P I
//   D E R
//   S
//
// and read IR (B's column), PE (E's), SDS (W's). Nothing is added: the
// last row is as short as the text leaves it, and unlocking needs only the
// key.

fn options() {
	#{ base: integer(), key_base: integer(27) }
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base), key: symbol_type(o.key_base) }
}

fn outputs(o) {
	// The key is passed through: unlocking needs it.
	#{ cipher_text: symbol_type(o.base), key: symbol_type(o.key_base) }
}

// Which place of the text each place of a cipher text count long comes
// from: down each column in turn, the columns in their key symbols' order.
fn order(key, count) {
	if key.is_empty() {
		throw "the key is empty";
	}
	let columns = [];
	for c in 0..key.len() { columns.push(c); }
	// A stable sort by key symbol: insertion, since rhai's sort isn't
	// promised stable.
	for a in 1..columns.len() {
		let c = columns[a];
		let b = a;
		while b > 0 && key[columns[b - 1]] > key[c] {
			columns[b] = columns[b - 1];
			b -= 1;
		}
		columns[b] = c;
	}
	let order = [];
	for c in columns {
		let k = c;
		while k < count {
			order.push(k);
			k += key.len();
		}
	}
	order
}

fn lock(o, i) {
	let text = i.plain_text;
	#{ cipher_text: order(i.key, text.len()).map(|k| text[k]), key: i.key }
}

fn unlock(o, out) {
	let cipher = out.cipher_text;
	let plain = cipher.map(|s| 0);
	for (k, n) in order(out.key, cipher.len()) {
		plain[k] = cipher[n];
	}
	#{ plain_text: plain }
}

fn examples() {
	[
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: symbols(27, "SPIDERS"), key: symbols(27, "WEB") },
			outputs: #{ cipher_text: symbols(27, "IRPESDS") },
		},
		#{
			// A key with symbols alike: the leftmost column first.
			options: #{ base: 27 },
			inputs: #{ plain_text: symbols(27, "EIGHT LEGS EIGHT EYES"), key: symbols(27, "ARACHNE") },
		},
		#{
			// A key in digits, longer than the text.
			options: #{ base: 26, key_base: 10 },
			inputs: #{ plain_text: symbols(26, "SILK"), key: symbols(10, "271828") },
			outputs: #{ cipher_text: symbols(26, "LSIK") },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: [], key: symbols(27, "KEY") },
		},
		#{
			options: #{ base: 27 },
			inputs: #{ plain_text: symbols(27, "SPIDERS"), key: [] },
			error: "the key is empty",
		},
	]
}
