// Numbers picked by a seed, the same every time: what illustrations vary
// their drawings by without moving the journal's generator on (orrery's
// worlds, spider_web's threads).
// import "lib/chance" as chance;

// A number from 0 up to 1 for thing k's n-th choice under seed.
fn of(seed, k, n) {
	let x = ((seed * 12.9898 + k * 78.233 + n * 37.719).sin() * 43758.5453).abs();
	x - x.floor()
}

// A number from -1 up to 1, likewise.
fn either_way(seed, k, n) {
	of(seed, k, n) * 2.0 - 1.0
}
