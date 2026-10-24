// maze_lock: the text written into the cells of a maze, one symbol per
// cell, in the order a "LeftHanded" or "RightHanded" traversal walks them.
// The maze's layout comes from the random seed, a base-10 number.
//
// Mazes are always square: w x w nodes and the w*w - 1 passages between
// them, the smallest that holds the text. The cells left over hold 0, and
// text_length lets unlocking drop them again.

fn options() {
	#{ input_base: integer(), strategy: one_of(["LeftHanded", "RightHanded"], "LeftHanded") }
}

fn inputs(o) {
	#{ maze_text: symbol_type(o.input_base), random_seed: symbol_type(10) }
}

fn outputs(o) {
	// The seed is passed through: unlocking needs it.
	#{ generated_maze: "Maze", random_seed: symbol_type(10), text_length: "LockProcessInfo" }
}

fn lock(o, i) {
	let text = i.maze_text;
	let side = 1;
	while 2 * side * side - 1 < text.len() {
		side += 1;
	}
	let maze = new_maze(side, side, #{ seed: seed_of(i.random_seed), strategy: o.strategy });
	maze.fill(text);
	#{ generated_maze: maze, random_seed: i.random_seed, text_length: text.len() }
}

fn unlock(o, out) {
	let text = [];
	for cell in out.generated_maze.cells() {
		text.push(cell.symbol);
	}
	text.truncate(out.text_length);
	#{ maze_text: text }
}

// The seed's digits as one number.
fn seed_of(digits) {
	if digits.len() > 18 {
		throw `the random seed must be at most 18 digits, got ${digits.len()}`;
	}
	let seed = 0;
	for d in digits {
		seed = seed * 10 + d;
	}
	seed
}

fn examples() {
	[
		#{
			options: #{ input_base: 27 },
			inputs: #{ maze_text: symbols(27, "HELLO WORLD"), random_seed: symbols(10, "42") },
			outputs: #{ text_length: 11 },
		},
		#{
			options: #{ input_base: 27, strategy: "RightHanded" },
			inputs: #{ maze_text: symbols(27, "EVERY LOCK OPENS"), random_seed: symbols(10, "7") },
		},
		#{
			// An even length, which the square maze still holds.
			options: #{ input_base: 10 },
			inputs: #{ maze_text: symbols(10, "31415926"), random_seed: symbols(10, "1") },
		},
		#{
			options: #{ input_base: 27 },
			inputs: #{ maze_text: [], random_seed: symbols(10, "0") },
		},
		#{
			options: #{ input_base: 27, strategy: "Sideways" },
			error: "option 'strategy' should be one of \"LeftHanded\", \"RightHanded\", got \"Sideways\"",
		},
		#{
			options: #{ input_base: 27 },
			inputs: #{ maze_text: symbols(27, "HI"), random_seed: symbols(10, "1234567890123456789") },
			error: "at most 18 digits",
		},
	]
}
