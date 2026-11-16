// mere_soundings: what came up (soundings), as a straddling
// checkerboard's figures under its key; and what's under the title
// (chart), an autokey cipher.

fn chain(o) {
	#{
		locks: #{
			soundings: #{ type: "straddling_checkerboard", options: #{ base: 27, first_blank: 2, second_blank: 6 } },
			chart: #{ type: "autokey", options: #{ base: 26 } },
		},
		connect: [],
		fixed: #{
			"soundings.plain_text": symbols(27, "NONE CAME UP"),
			"soundings.key": symbols(27, "FATHOM"),
			"chart.plain_text": symbols(26, "TURNTHEGUARD"),
			"chart.key": symbols(26, "DEEP"),
		},
		illustrations: #{},
	}
}
