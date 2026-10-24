// substitution_cipher: each symbol replaced by the ordering's symbol at
// that position. With the ordering "IKBNAZ...", A (1) becomes K, the
// ordering's symbol 1, and space (0) becomes I.
//
// The ordering must hold every symbol of the base exactly once, or the
// cipher couldn't be undone.

import "lib/permutations" as perm;

fn options() {
	#{ base_n: integer() }
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base_n), ordering: symbol_type(o.base_n) }
}

fn outputs(o) {
	// The ordering is passed through: unlocking needs it.
	#{ cipher_text: symbol_type(o.base_n), ordering: symbol_type(o.base_n) }
}

fn lock(o, i) {
	perm::check(i.ordering, o.base_n, "the ordering");
	#{ cipher_text: perm::apply(i.plain_text, i.ordering), ordering: i.ordering }
}

fn unlock(o, out) {
	perm::check(out.ordering, o.base_n, "the ordering");
	#{ plain_text: perm::apply(out.cipher_text, perm::inverse(out.ordering)) }
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
