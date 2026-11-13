// Rows of marks, a digit each, read left to right and top to bottom: where
// the moon_phases and gazing_eyes illustrations set theirs.
// import "lib/digit_rows" as rows;

// Throws unless o's per_row is 1 or more and its group 0 or more.
fn check(o) {
	if o.per_row < 1 { throw `per_row should be 1 or more, got ${o.per_row}`; }
	if o.group < 0 { throw `group should be 0 (no groups) or more, got ${o.group}`; }
}

// The middle of each of count marks across by down, o.per_row to a row,
// o.gap apart and the rows o.row_gap apart; with o.group, the marks of a
// row in groups of that many, o.group_gap more between them (a symbol's
// digits side by side, say). The first mark's corner is at (0, 0).
fn places(count, across, down, o) {
	let at = [];
	for k in 0..count {
		let col = k % o.per_row;
		let row = k / o.per_row;
		let x = across / 2.0 + col * (across + o.gap);
		if o.group > 0 { x += (col / o.group) * o.group_gap; }
		at.push(vec2(x, down / 2.0 + row * (down + o.row_gap)));
	}
	at
}
