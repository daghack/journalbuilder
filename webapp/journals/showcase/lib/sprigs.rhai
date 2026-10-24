// Sprigs for the showcase's pages, imported with import "lib/sprigs".

// A stem from `at`, `size` long, leaning right, with a leaf either side.
fn sprig(at, size) {
	let tip = at + vec2(size * 0.3, -size);
	let leaf = |from, to, bulge| new_path().move_to(from).quad_to((from + to) * 0.5 + bulge, to).quad_to((from + to) * 0.5 - bulge * 0.2, from);
	let mid = at + vec2(size * 0.12, -size * 0.45);
	new_path().move_to(at).quad_to(at + vec2(0, -size * 0.6), tip)
		.append(leaf.call(mid, mid + vec2(-size * 0.4, -size * 0.2), vec2(0, -size * 0.15)))
		.append(leaf.call(mid, mid + vec2(size * 0.4, -size * 0.1), vec2(0, size * 0.12)))
}
