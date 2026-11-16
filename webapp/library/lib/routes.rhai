// Texts written into a grid's cells along a route and read off row by
// row: what the route transpositions (knights_tour, turning_grille)
// share. import "lib/routes" as routes;
//
// A route is an array of a grid's cells, every one once, each numbered
// row by row (row * width + column). A text count long fills the route's
// first count cells; read off row by row, the filled cells give the
// cipher text, the empty ones skipped. Nothing is added, and unlocking
// needs only the route.

// Which place of the text each place of the cipher text comes from.
fn order(route, count) {
	if count > route.len() {
		throw `the text, ${count} symbols, is longer than the ${route.len()} cells its route holds`;
	}
	let at = [];
	at.pad(route.len(), -1);
	for (cell, k) in route {
		if k < count { at[cell] = k; }
	}
	let order = [];
	for k in at {
		if k >= 0 { order.push(k); }
	}
	order
}

// Throws unless `route` takes each of `cells` cells exactly once. `what`
// names it in the error.
fn check(route, cells, what) {
	if route.len() != cells {
		throw `${what} must take each of its ${cells} cells once; it takes ${route.len()}`;
	}
	let seen = [];
	seen.pad(cells, false);
	for cell in route {
		if cell < 0 || cell >= cells {
			throw `${what} has a cell, ${cell}, off its grid`;
		}
		if seen[cell] {
			throw `${what} takes cell ${cell} twice`;
		}
		seen[cell] = true;
	}
}
