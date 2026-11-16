// psalter_churchyard: the churchyard's three secrets. Where she isn't
// (rails), written down three rails and read off rail by rail; where to go
// (stone), as a book cipher's references into the psalter (content
// psalter); and the month the watchers stand in a line (watch), as two
// sightings of them a year apart.

fn chain(o) {
	#{
		sources: #{ book: #{ type: "content", name: "psalter" } },
		locks: #{
			rails: #{ type: "rail_fence", options: #{ base: 27, rails: 3 } },
			stone: #{ type: "book_cipher", options: #{ seed: 3 } },
			watch: #{ type: "star_alignment", options: #{ sightings: 2 } },
		},
		connect: [["book.value", "stone.book"]],
		fixed: #{
			"rails.plain_text": symbols(27, "SHEISNOTINHERGRAVE"),
			"stone.plain_text": symbols(27, "BELFRY"),
			"watch.alignment": symbols(13, "JE"),
		},
		illustrations: #{},
	}
}
