// doublering: the 26 letters in two rings of 13, the canonical order's
// first half over its second, each letter swapping with the one across
// from it. Swapping twice gives the letter back, so locking and unlocking
// are the same. Symbols are base 26, A = 0.

import "lib/permutations" as perm;

fn options() {
	#{ canonical_order: string() }
}

fn check(o) {
	partners(o.canonical_order);
}

fn inputs(o) {
	#{ plain_text: symbol_type(26) }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(26) }
}

fn lock(o, i) {
	#{ cipher_text: perm::apply(i.plain_text, partners(o.canonical_order)) }
}

fn unlock(o, out) {
	#{ plain_text: perm::apply(out.cipher_text, partners(o.canonical_order)) }
}

// Each letter's partner across the rings, as a table.
fn partners(order) {
	let ring = symbols(26, order);
	perm::check(ring, 26, "canonical_order");
	perm::pairs(ring)
}

fn examples() {
	[
		#{
			options: #{ canonical_order: "QWERTYUIOPASDFGHJKLZXCVBNM" },
			inputs: #{ plain_text: symbols(26, "HELLOWORLD") },
			outputs: #{ cipher_text: symbols(26, "EHYYCGCJYM") },
		},
		#{
			// In alphabetical order, it's ROT13.
			options: #{ canonical_order: "abcdefghijklmnopqrstuvwxyz" },
			inputs: #{ plain_text: symbols(26, "HELLO") },
			outputs: #{ cipher_text: symbols(26, "URYYB") },
		},
		#{
			options: #{ canonical_order: "ABC" },
			error: "canonical_order must hold each of the 26 symbols once; it has 3",
		},
		#{
			options: #{ canonical_order: "QWERTYUIOPASDFGHJKLZXCVBNQ" },
			error: "canonical_order repeats 'Q'",
		},
	]
}
