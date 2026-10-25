// sourced_note: the showcase's own chain drawing on the journal's files:
// the library's lorem.txt shifted as symbols, and written out plain in the
// library's plain_hand, with styled_text.

fn chain(o) {
	#{
		sources: #{
			secret: #{ type: "content", name: "lorem", base: 27 },
			words: #{ type: "content", name: "lorem" },
			hand: #{ type: "style", name: "plain_hand", options: #{ size: 22.0, color: "#9b2318" } },
		},
		locks: #{
			shift: #{ type: "shift_cipher", options: #{ base: 27, shift: 3 } },
		},
		connect: [
			["secret.value", "shift.plain_text"],
		],
		fixed: #{},
		illustrations: #{
			note: #{ type: "styled_text", options: #{ width: 960.0 }, inputs: #{ text: "words.value", style: "hand.value" } },
		},
	}
}

// Where the web editor draws each lock and illustration.
fn layout() {
	#{ secret: [-200, 60], words: [-200, 260], hand: [-200, 400], shift: [160, 60], note: [160, 300] }
}
