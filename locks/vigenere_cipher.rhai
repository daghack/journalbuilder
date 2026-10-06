// vigenere_cipher: each plain symbol plus the key symbol under it, the key
// repeating as often as it takes, mod cipher_base.

fn options() {
	#{ plain_base: integer(), key_base: integer(), cipher_base: integer() }
}

fn check(o) {
	// A smaller cipher base would fold plain symbols together.
	if o.cipher_base < o.plain_base {
		throw `cipher_base (${o.cipher_base}) must be at least plain_base (${o.plain_base})`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.plain_base), key: symbol_type(o.key_base) }
}

fn outputs(o) {
	// The key is passed through: unlocking needs it.
	#{ cipher_text: symbol_type(o.cipher_base), key: symbol_type(o.key_base) }
}

fn lock(o, i) {
	if i.key.is_empty() {
		throw "the key is empty";
	}
	let cipher = [];
	for (s, n) in i.plain_text {
		cipher.push((s + i.key[n % i.key.len()]) % o.cipher_base);
	}
	#{ cipher_text: cipher, key: i.key }
}

fn unlock(o, out) {
	if out.key.is_empty() {
		throw "the key is empty";
	}
	let plain = [];
	for (c, n) in out.cipher_text {
		let k = out.key[n % out.key.len()] % o.cipher_base;
		plain.push((c - k + o.cipher_base) % o.cipher_base);
	}
	// The key is a pass-through, so it needn't be returned.
	#{ plain_text: plain }
}

fn examples() {
	[
		#{
			options: #{ plain_base: 27, key_base: 27, cipher_base: 27 },
			inputs: #{ plain_text: symbols(27, "ATTACK AT DAWN"), key: symbols(27, "KEY") },
			outputs: #{ cipher_text: symbols(27, "LYRLHIKFRKIZGS"), key: symbols(27, "KEY") },
		},
		#{
			// A base-10 key over base-27 text.
			options: #{ plain_base: 27, key_base: 10, cipher_base: 27 },
			inputs: #{ plain_text: symbols(27, "HELLO"), key: symbols(10, "31") },
		},
		#{
			options: #{ plain_base: 27, key_base: 27, cipher_base: 27 },
			inputs: #{ plain_text: symbols(27, "HELLO"), key: symbols(27, "") },
			error: "the key is empty",
		},
	]
}
