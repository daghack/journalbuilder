// mere_title: the motto (motto), hidden in a Bacon cipher's two kinds of
// marks.

fn chain(o) {
	#{
		locks: #{
			motto: #{ type: "baconian", options: #{ base: 27, kinds: 2 } },
		},
		connect: [],
		fixed: #{
			"motto.plain_text": symbols(27, "DEEP"),
		},
		illustrations: #{},
	}
}
