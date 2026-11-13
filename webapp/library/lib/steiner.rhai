// The Steiner triple system S(2, 3, 13): thirteen points, 0 to 12, in 26
// triples, so that any two points are together in exactly one triple. 26
// triples are a letter each, and two points of a triple are enough to
// name it. What the steiner_triples and steiner_cipher locks use.
// import "lib/steiner" as steiner;

// The 26 triples, each [a, b, c]: triple i (A to M) is i, i + 1 and
// i + 4 round the thirteen, and triple 13 + i (N to Z) is i, i + 2 and
// i + 7. The gaps between a triple's points, 1, 3 and 4 or 2, 5 and 6
// either way round, are every gap there is, once each: so each pair of
// points is in one triple.
fn triples() {
	let all = [];
	for i in 0..13 { all.push([i, (i + 1) % 13, (i + 4) % 13]); }
	for i in 0..13 { all.push([i, (i + 2) % 13, (i + 7) % 13]); }
	all
}

// Which triple each two points are in: pairs()[a][b] is its place in
// triples(), or -1 where a and b are the same point.
fn pairs() {
	let table = [];
	for a in 0..13 {
		let row = [];
		for b in 0..13 { row.push(-1); }
		table.push(row);
	}
	for (triple, n) in triples() {
		for a in triple {
			for b in triple {
				if a != b { table[a][b] = n; }
			}
		}
	}
	table
}

// The point that makes up a triple with each two: thirds()[a][b], or a
// itself where a and b are the same point.
fn thirds() {
	let all = triples();
	let table = [];
	for (row, a) in pairs() {
		table.push(row.map(|n, b| if n < 0 { a } else { all[n][0] + all[n][1] + all[n][2] - a - b }));
	}
	table
}
