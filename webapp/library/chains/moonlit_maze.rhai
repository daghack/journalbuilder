// moonlit_maze: a secret locked with a key word (vigenere_cipher),
// the cipher text hidden along the one way through a circular maze, and the
// key word written as the moon's phases (the moons symbol set), two moons a letter. Give
// vig.plain_text, the secret, and vig.key, the key word, in base 27;
// maze draws the circular maze and moons the key. rings is how many rings the
// circular maze has, at least, and seed lays it out.

fn options() {
	#{ rings: integer(4), seed: integer(0) }
}

fn chain(o) {
	#{
		locks: #{
			vig: #{ type: "vigenere_cipher", options: #{ plain_base: 27, key_base: 27, cipher_base: 27 } },
			lab: #{ type: "circular_maze_lock", options: #{ base: 27, rings: o.rings, seed: o.seed } },
			nights: #{ type: "symbols_to_digits", options: #{ input_base: 27, output_base: 8 } },
		},
		connect: [
			["vig.cipher_text", "lab.text"],
			["vig.key", "nights.symbols"],
		],
		fixed: #{},
		sources: #{
			phases: #{ type: "symbolset", name: "moons" },
		},
		illustrations: #{
			maze: #{ type: "circular_maze", options: #{}, inputs: #{ maze: "lab.maze" } },
			moons: #{ type: "symbol_rows", options: #{ group: 2, size: 80.0, per_row: 8 }, inputs: #{ symbols: "nights.digits", symbol_set: "phases.value" } },
		},
	}
}

// Where the web editor draws each lock and illustration.
fn layout() {
	#{ vig: [120, 60], lab: [480, 60], nights: [480, 300], phases: [120, 300], maze: [840, 60], moons: [840, 300] }
}

fn example() {
	#{
		"vig.plain_text": symbols(27, "IT SLEEPS BENEATH THE HOUSE"),
		"vig.key": symbols(27, "MOON"),
	}
}
