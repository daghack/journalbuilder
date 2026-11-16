// door_glass: what the mirrors say. The gallery's words (gallery), written
// in columns under the key word and read off column by column in the
// order of its letters, a glass to each column; and the key word itself
// (round), along the one way through the round glass's circular maze.

fn chain(o) {
	#{
		locks: #{
			gallery: #{ type: "columnar_transposition", options: #{ base: 27 } },
			round: #{ type: "circular_maze_lock", options: #{ base: 27, seed: 5, rings: 6 } },
		},
		connect: [],
		fixed: #{
			"gallery.plain_text": symbols(27, "ITOPENSONLYFROMTHEOTHERSIDE"),
			"gallery.key": symbols(27, "SHADOW"),
			"round.text": symbols(27, "THEGLASSESANSWERTOSHADOW"),
		},
		illustrations: #{},
	}
}
