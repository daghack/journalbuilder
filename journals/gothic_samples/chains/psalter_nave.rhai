// psalter_nave: what the nave holds. The saint's walk (nave), written in
// columns under her word and read off column by column in the order of
// the word's letters, a pillar to each column; and where she sleeps (rose),
// the one way through the rose window's circular maze.

fn chain(o) {
	#{
		locks: #{
			nave: #{ type: "columnar_transposition", options: #{ base: 27 } },
			rose: #{ type: "circular_maze_lock", options: #{ base: 27, seed: 9, rings: 6 } },
		},
		connect: [],
		fixed: #{
			"nave.plain_text": symbols(27, "THESAINTWALKSTHENAVEATVESPERS"),
			"nave.key": symbols(27, "LAZARUS"),
			"rose.text": symbols(27, "SHESLEEPSUNDERTHEALTAR"),
		},
		illustrations: #{},
	}
}
