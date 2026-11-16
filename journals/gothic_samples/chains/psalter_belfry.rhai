// psalter_belfry: the bells' warning. Its letters (words) as Steiner
// triples, three of thirteen bells each, rung under the ringers' key
// (bells); and the board in the ringing chamber (board), every letter of
// the alphabet as its triple.

fn chain(o) {
	#{
		locks: #{
			words: #{ type: "steiner_triples", options: #{ points: 3, seed: 4 } },
			bells: #{ type: "steiner_cipher", options: #{} },
			board: #{ type: "steiner_triples", options: #{ base: 26, points: 3, seed: 1 } },
		},
		connect: [["words.points", "bells.plain_text"]],
		fixed: #{
			"words.plain_text": symbols(27, "DO NOT ANSWER HER"),
			"bells.key": symbols(13, "JE"),
			"board.plain_text": symbols(26, "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
		},
		illustrations: #{},
	}
}
