// Circular mazes: rings of cells round a heart, walled off from each other but
// for one way through (with blind ways beside it: a maze, not a labyrinth,
// which has a single path). What the circular_maze_lock lock makes and the
// circular_maze illustration draws.
// import "lib/circular_maze" as lab;
//
// A CircularMaze is plain data:
//   base       its symbols' base
//   sectors    how many cells each ring has, the innermost ring first
//   entrance   the cell of the outermost ring the way in opens on
//   heart      the cell of the innermost ring that opens on the heart
//   inward     for each ring, for each cell: whether it opens on the cell
//              inside it (the innermost ring's are all false)
//   clockwise  for each ring, for each cell: whether it opens on the next
//              cell clockwise
//   symbols    for each ring, each cell's symbol
// Cells are counted clockwise from the top. A ring has as many cells as
// the one inside it, or twice as many, two to each of that ring's.

// How many cells each of `rings` rings has, the innermost first: 8, and
// twice the ring inside's wherever cells would be more than twice as long
// as a ring is wide. The heart is one and a half rings' width in radius.
fn sectors(rings) {
	let counts = [];
	let n = 8;
	for k in 0..rings {
		if 2.0 * PI() * (k + 2.0) / n > 2.0 { n *= 2; }
		counts.push(n);
	}
	counts
}

// The heart's radius, in rings' widths.
fn heart_radius() {
	1.5
}

// The cell of the ring inside ring k that its cell s is against.
fn inside(sectors, k, s) {
	s * sectors[k - 1] / sectors[k]
}

// The cells of ring k + 1 against ring k's cell s.
fn outside(sectors, k, s) {
	let each = sectors[k + 1] / sectors[k];
	let cells = [];
	for r in 0..each { cells.push(s * each + r); }
	cells
}

// The cells cell [k, s] opens on, each [ring, cell].
fn open_from(l, k, s) {
	let n = l.sectors[k];
	let cells = [];
	if k > 0 && l.inward[k][s] { cells.push([k - 1, inside(l.sectors, k, s)]); }
	if k < l.sectors.len() - 1 {
		for t in outside(l.sectors, k, s) {
			if l.inward[k + 1][t] { cells.push([k + 1, t]); }
		}
	}
	if l.clockwise[k][s] { cells.push([k, (s + 1) % n]); }
	if l.clockwise[k][(s + n - 1) % n] { cells.push([k, (s + n - 1) % n]); }
	cells
}

// The way through: the cells from the entrance to the one that opens on
// the heart, each [ring, cell], the shortest way there is. Throws if the
// heart can't be reached.
fn path(l) {
	let rings = l.sectors.len();
	let from = l.sectors.map(|n| []);
	for k in 0..rings {
		for s in 0..l.sectors[k] { from[k].push(()); }
	}
	let start = [rings - 1, l.entrance];
	from[start[0]][start[1]] = start;
	let queue = [start];
	let next = 0;
	while next < queue.len() {
		let cell = queue[next];
		next += 1;
		if cell[0] == 0 && cell[1] == l.heart { break; }
		for to in open_from(l, cell[0], cell[1]) {
			if from[to[0]][to[1]] == () {
				from[to[0]][to[1]] = cell;
				queue.push(to);
			}
		}
	}
	if from[0][l.heart] == () {
		throw "the circular maze has no way from its entrance to its heart";
	}
	let way = [];
	let cell = [0, l.heart];
	while cell != start {
		way.insert(0, cell);
		cell = from[cell[0]][cell[1]];
	}
	way.insert(0, start);
	way
}

// Throws unless `l` has a CircularMaze's parts, each ring's the right size.
fn check(l) {
	if type_of(l) != "map" { throw `expected a CircularMaze, got ${type_of(l)}`; }
	for key in ["base", "sectors", "entrance", "heart", "inward", "clockwise", "symbols"] {
		if !(key in l) { throw `the circular maze has no ${key}`; }
	}
	let rings = l.sectors.len();
	if rings < 1 { throw "the circular maze has no rings"; }
	for key in ["inward", "clockwise", "symbols"] {
		if l[key].len() != rings { throw `the circular maze's ${key} should have ${rings} rings, not ${l[key].len()}`; }
		for k in 0..rings {
			if l[key][k].len() != l.sectors[k] {
				throw `ring ${k} of the circular maze's ${key} should have ${l.sectors[k]} cells, not ${l[key][k].len()}`;
			}
		}
	}
	for k in 1..rings {
		let each = l.sectors[k] / l.sectors[k - 1];
		if l.sectors[k] % l.sectors[k - 1] != 0 || each < 1 || each > 2 {
			throw `ring ${k} of the circular maze should have as many cells as ring ${k - 1}, or twice as many`;
		}
	}
	if l.entrance < 0 || l.entrance >= l.sectors[rings - 1] { throw `the circular maze's entrance, ${l.entrance}, isn't a cell of its outermost ring`; }
	if l.heart < 0 || l.heart >= l.sectors[0] { throw `the circular maze's heart, ${l.heart}, isn't a cell of its innermost ring`; }
}
