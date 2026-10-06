// Permutations of a base's symbols, as tables: arrays indexed by symbol,
// holding the symbol each one becomes. For the substitution and doublering
// lock files: import "lib/permutations" as perm;

// Throws unless `ring` holds each of the base's symbols exactly once.
// `what` names it in the error.
fn check(ring, base, what) {
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

// Each symbol replaced by the table's entry for it.
fn apply(symbols, table) {
	symbols.map(|s| table[s])
}

// The table that undoes `table`.
fn inverse(table) {
	let inverse = [];
	inverse.pad(table.len(), 0);
	for (s, n) in table {
		inverse[s] = n;
	}
	inverse
}

// The ring's first half over its second, each symbol swapping with the one
// across from it. Applying the table twice changes nothing.
fn pairs(ring) {
	let half = ring.len() / 2;
	let table = [];
	table.pad(ring.len(), 0);
	for n in 0..half {
		table[ring[n]] = ring[n + half];
		table[ring[n + half]] = ring[n];
	}
	table
}
