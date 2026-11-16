// door_chamber: the bedchamber's two secrets. A word (post), as a book
// cipher's references into the account of the nights (content nights);
// and the warning in the glasses (glasses), written down three rails and
// read off rail by rail.

fn chain(o) {
	#{
		sources: #{ book: #{ type: "content", name: "nights" } },
		locks: #{
			post: #{ type: "book_cipher", options: #{ seed: 3 } },
			glasses: #{ type: "rail_fence", options: #{ base: 27, rails: 3 } },
		},
		connect: [["book.value", "post.book"]],
		fixed: #{
			"post.plain_text": symbols(27, "COVER"),
			"glasses.plain_text": symbols(27, "WAKEBEFORETHETHIRDKNOCK"),
		},
		illustrations: #{},
	}
}
