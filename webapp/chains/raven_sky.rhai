// raven_sky: a message locked with a text's Huffman code, and the code
// drawn as a sky of named stars, cut into constellations of at most
// most_stars stars, a page each: the pages are the key. Give
// enc.plain_text, the text, and msg.plain_text, the message, in base 27;
// pages draws the sky, message the message's cipher text in
// balanced-ternary marks.

fn options() {
	#{ most_stars: integer(9), name_seed: integer(11), output_base: integer(3), seed: integer(7), size_seed: integer(29) }
}

fn chain(o) {
	#{
		locks: #{
			enc: #{ type: "huffman_encode", options: #{ input_base: 27, output_base: o.output_base } },
			msg: #{ type: "huffman_apply", options: #{ input_base: 27, output_base: o.output_base } },
			sky: #{ type: "star_chart", options: #{ input_base: 27, most_stars: o.most_stars, name_seed: o.name_seed, output_base: o.output_base, seed: o.seed, size_seed: o.size_seed } },
		},
		connect: [
			["enc.code", "msg.code"],
			["enc.code", "sky.code"],
		],
		fixed: #{},
		renderers: #{
			message: #{ type: "trit_message", options: #{ base: o.output_base }, inputs: #{ message: "msg.cipher_text" } },
			pages: #{ type: "star_pages", options: #{}, inputs: #{ chart: "sky.chart" } },
		},
	}
}

// Where the web editor draws each lock and renderer.
fn layout() {
	#{ enc: [120, 60], msg: [480, 300], sky: [480, 60], message: [840, 300], pages: [840, 60] }
}

fn example() {
	#{
		"enc.plain_text": symbols(27, "ONCE UPON A MIDNIGHT DREARY WHILE I PONDERED WEAK AND WEARY"),
		"msg.plain_text": symbols(27, "NEVERMORE"),
	}
}
