// keyed_doublering: a doublering whose order is an input, base_ordering,
// rather than an option, in any even base: the ordering's first half over
// its second, each symbol swapping with the one across from it.

import "lib/permutations" as perm;

fn options() {
	#{ base: integer() }
}

fn check(o) {
	if o.base % 2 != 0 {
		throw `base must be even, got ${o.base}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base), base_ordering: symbol_type(o.base) }
}

fn outputs(o) {
	// The ordering is passed through: unlocking needs it.
	#{ cipher_text: symbol_type(o.base), base_ordering: symbol_type(o.base) }
}

fn lock(o, i) {
	#{ cipher_text: perm::apply(i.plain_text, partners(i.base_ordering, o.base)), base_ordering: i.base_ordering }
}

fn unlock(o, out) {
	#{ plain_text: perm::apply(out.cipher_text, partners(out.base_ordering, o.base)) }
}

// Each symbol's partner across the rings, as a table.
fn partners(ring, base) {
	perm::check(ring, base, "base_ordering");
	perm::pairs(ring)
}

fn examples() {
	let ordering = symbols(26, "QWERTYUIOPASDFGHJKLZXCVBNM");
	[
		#{
			options: #{ base: 26 },
			inputs: #{ plain_text: symbols(26, "ATTACKATDAWN"), base_ordering: ordering },
			outputs: #{ cipher_text: symbols(26, "BKKBOTBKMBGS"), base_ordering: ordering },
		},
		#{
			options: #{ base: 10 },
			inputs: #{ plain_text: symbols(10, "8675309"), base_ordering: symbols(10, "3141592065") },
			error: "base_ordering repeats '1'",
		},
		#{
			options: #{ base: 10 },
			inputs: #{ plain_text: symbols(10, "8675309"), base_ordering: symbols(10, "9876543210") },
		},
		#{
			// The Rust version indexed past the end of a short ordering.
			options: #{ base: 26 },
			inputs: #{ plain_text: symbols(26, "HELLO"), base_ordering: symbols(26, "AB") },
			error: "base_ordering must hold each of the 26 symbols once; it has 2",
		},
		#{
			options: #{ base: 27 },
			error: "base must be even, got 27",
		},
	]
}
