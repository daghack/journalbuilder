// steiner_cipher: a cipher of thirteen symbols by the Steiner triple
// system S(2, 3, 13) (lib/steiner), whose 26 triples hold every two of
// its thirteen points together once: each symbol and the key's symbol
// under it, the key repeating as often as it takes, are two points of
// one triple, and the cipher's symbol is the third. A symbol under
// itself stays as it is. The three of a triple each make up the other
// two's, so locking the cipher text with the same key gives the plain
// text back: to unlock is to lock again.
//
// Both texts and the key are base 13, the thirteen_black_priests
// glyphset's digits: under the key 4, 0 becomes 1 and 1 becomes 0 (the
// triple 0 1 4), and 2 becomes 9 (the triple 2 4 9).

import "lib/steiner" as steiner;

fn options() {
	#{}
}

fn inputs(o) {
	#{ plain_text: symbol_type(13), key: symbol_type(13) }
}

fn outputs(o) {
	// The key is passed through: unlocking needs it.
	#{ cipher_text: symbol_type(13), key: symbol_type(13) }
}

// Each of the text's symbols with the key's under it, as their third.
fn thirds(text, key) {
	if key.is_empty() {
		throw "the key is empty";
	}
	let thirds = steiner::thirds();
	let made = [];
	for (s, n) in text {
		made.push(thirds[s][key[n % key.len()]]);
	}
	made
}

fn lock(o, i) {
	#{ cipher_text: thirds(i.plain_text, i.key), key: i.key }
}

fn unlock(o, out) {
	#{ plain_text: thirds(out.cipher_text, out.key) }
}

fn examples() {
	[
		#{
			options: #{},
			inputs: #{ plain_text: [0, 1, 2, 4], key: [4] },
			outputs: #{ cipher_text: [1, 0, 9, 4] },
		},
		#{
			options: #{},
			inputs: #{ plain_text: symbols(13, "ABCDEFGHIJKLMMLKJIHGFEDCBA"), key: symbols(13, "FACADE") },
		},
		#{
			options: #{},
			inputs: #{ plain_text: [], key: [3] },
		},
		#{
			options: #{},
			inputs: #{ plain_text: [1, 2, 3], key: [] },
			error: "the key is empty",
		},
	]
}
