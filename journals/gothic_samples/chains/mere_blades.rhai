// mere_blades: the blades' three secrets. A word on the rail (rail), in
// Morse; the count (strip), wound round a four-sided staff, a scytale; and
// the way (guard), written through a turning grille's holes.

fn chain(o) {
	#{
		locks: #{
			rail: #{ type: "morse_code", options: #{ base: 26 } },
			strip: #{ type: "scytale", options: #{ base: 26, faces: 4 } },
			guard: #{ type: "turning_grille", options: #{ base: 27, size: 6, seed: 4 } },
		},
		connect: [],
		fixed: #{
			"rail.plain_text": symbols(26, "OBSIDIAN"),
			"strip.plain_text": symbols(26, "COUNTTHESEVEN"),
			"guard.plain_text": symbols(27, "FOLLOW THE HOUNDS DOWN TO THE WATERS"),
		},
		illustrations: #{},
	}
}
