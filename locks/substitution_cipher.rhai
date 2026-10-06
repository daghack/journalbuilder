// substitution_cipher: each symbol replaced by the ordering's symbol at
// that position. With the ordering "IKBNAZ...", A (1) becomes K, the
// ordering's symbol 1, and space (0) becomes I.
//
// The ordering must hold every symbol of the base exactly once, or the
// cipher couldn't be undone.

fn options() {
	#{ base_n: () }
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base_n), ordering: symbol_type(o.base_n) }
}

fn outputs(o) {
	// The ordering is passed through: unlocking needs it.
	#{ cipher_text: symbol_type(o.base_n), ordering: symbol_type(o.base_n) }
}

fn lock(o, i) {
	let ordering = i.ordering;
	check_permutation(ordering, o.base_n);
	#{ cipher_text: i.plain_text.map(|s| ordering[s]), ordering: ordering }
}

fn unlock(o, out) {
	check_permutation(out.ordering, o.base_n);
	let inverse = [];
	inverse.pad(o.base_n, 0);
	for (s, n) in out.ordering {
		inverse[s] = n;
	}
	#{ plain_text: out.cipher_text.map(|s| inverse[s]) }
}

fn check_permutation(ordering, base) {
	if ordering.len() != base {
		throw `the ordering must hold each of the ${base} symbols once; it has ${ordering.len()}`;
	}
	let seen = [];
	seen.pad(base, false);
	for s in ordering {
		if seen[s] {
			throw `the ordering repeats '${symbol_text(base, [s])}'`;
		}
		seen[s] = true;
	}
}

fn examples() {
	let ordering = symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS");
	[
		#{
			options: #{ base_n: 27 },
			inputs: #{ plain_text: symbols(27, "ATTACK AT DAWN"), ordering: ordering },
			outputs: #{ cipher_text: symbols(27, "KUUKNWIKUIAKPH"), ordering: ordering },
		},
		#{
			options: #{ base_n: 27 },
			inputs: #{ plain_text: symbols(27, "ABC"), ordering: symbols(27, "AAAA") },
			error: "the ordering must hold each of the 27 symbols once",
		},
		#{
			options: #{ base_n: 27 },
			inputs: #{ plain_text: symbols(27, "ABC"), ordering: symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDA") },
			error: "the ordering repeats 'A'",
		},
	]
}
