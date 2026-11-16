// door_threshold: what knocks. Its letters (words) as Steiner triples,
// three knocks of thirteen each, scored under the key (knocks); and the
// asterisms, every letter of the alphabet as its triple.

fn chain(o) {
	#{
		locks: #{
			words: #{ type: "steiner_triples", options: #{ points: 3, seed: 6 } },
			knocks: #{ type: "steiner_cipher", options: #{} },
			asterisms: #{ type: "steiner_triples", options: #{ base: 26, points: 3, seed: 2 } },
		},
		connect: [["words.points", "knocks.plain_text"]],
		fixed: #{
			"words.plain_text": symbols(27, "IT IS YOU WHO KNOCKS"),
			"knocks.key": symbols(13, "LC"),
			"asterisms.plain_text": symbols(26, "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
		},
		illustrations: #{},
	}
}
