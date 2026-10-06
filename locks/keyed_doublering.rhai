// keyed_doublering: a doublering whose order is an input, base_ordering,
// rather than an option, in any even base: the ordering's first half over
// its second, each symbol swapping with the one across from it.

fn options() {
	#{ base: () }
}

fn check(o) {
	if type_of(o.base) != "i64" || o.base % 2 != 0 {
		throw `base must be an even integer, got ${o.base}`;
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
	let ordering = i.base_ordering;
	#{ cipher_text: swapped(i.plain_text, partners(ordering, o.base)), base_ordering: ordering }
}

fn unlock(o, out) {
	#{ plain_text: swapped(out.cipher_text, partners(out.base_ordering, o.base)) }
}

// Each symbol's partner across the rings, indexed by symbol.
fn partners(ring, base) {
	check_permutation(ring, base, "base_ordering");
	let half = base / 2;
	let partner = [];
	partner.pad(base, 0);
	for n in 0..half {
		partner[ring[n]] = ring[n + half];
		partner[ring[n + half]] = ring[n];
	}
	partner
}

fn swapped(symbols, partner) {
	symbols.map(|s| partner[s])
}

// Throws unless `ring` holds each of the base's symbols exactly once.
fn check_permutation(ring, base, what) {
	if ring.len() != base {
		throw `${what} must hold each of the ${base} symbols once; it has ${ring.len()}`;
	}
	let seen = [];
	seen.pad(base, false);
	for s in ring {
		if seen[s] {
			throw `${what} repeats '${symbol_text(base, [s])}'`;
		}
		seen[s] = true;
	}
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
			error: "base must be an even integer, got 27",
		},
	]
}
