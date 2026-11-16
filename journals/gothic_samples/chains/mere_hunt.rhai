// mere_hunt: the hunt. Its trail (court), written along a knight's tour of
// the board and read off row by row; and the count of the hunters (game),
// a homophonic cipher's numbers.

fn chain(o) {
	#{
		locks: #{
			court: #{ type: "knights_tour", options: #{ base: 27, size: 6, seed: 2 } },
			game: #{ type: "homophonic", options: #{ base: 27, digits: 2, seed: 5 } },
		},
		connect: [],
		fixed: #{
			"court.plain_text": symbols(27, "THE HUNT ENDS AT THE WATER"),
			"game.plain_text": symbols(27, "SEVEN WENT OUT NONE CAME HOME"),
		},
		illustrations: #{},
	}
}
