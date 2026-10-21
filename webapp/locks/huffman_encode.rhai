// huffman_encode: text in input_base rewritten in output_base by the
// Huffman code its own symbols build, so the symbols it uses most get the
// shortest codes. It gives the code too, as its tree, a HuffmanCode value,
// as huffman::code(plain_text, input_base, output_base) builds it.
//
// The code is the key: unlocking reads the cipher text back with it. An
// art node such as star_chart can show it, huffman_apply can lock another
// message with it, and the cipher text needn't be used at all.

import "lib/huffman" as huff;

fn options() {
	#{ input_base: integer(), output_base: integer(), smoothing: integer(1) }
}

fn check(o) {
	if o.smoothing < 0 {
		throw `smoothing must be at least 0, got ${o.smoothing}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.input_base) }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(o.output_base), code: "HuffmanCode" }
}

fn lock(o, i) {
	let code = huffman::code(i.plain_text, o.input_base, o.output_base, #{ smoothing: o.smoothing });
	#{ cipher_text: huff::encode(code, i.plain_text), code: code }
}

fn unlock(o, out) {
	huff::check(out.code, o);
	let plain_text = huff::decode(out.code, out.cipher_text);
	// The code has to be the one the text builds, or it isn't this lock's.
	let built = huffman::code(plain_text, o.input_base, o.output_base, #{ smoothing: o.smoothing });
	if built.nodes != out.code.nodes {
		throw "the code isn't the one the text it reads builds";
	}
	#{ plain_text: plain_text }
}

fn examples() {
	[
		#{
			// Base 27 to binary: E and spaces, the commonest, get short codes.
			options: #{ input_base: 27, output_base: 2 },
			inputs: #{ plain_text: symbols(27, "EVERY LOCK OPENS") },
		},
		#{
			// A, B and C equally common, base 3, nothing else counted: one digit each.
			options: #{ input_base: 3, output_base: 3, smoothing: 0 },
			inputs: #{ plain_text: [2, 0, 1] },
			outputs: #{ cipher_text: [2, 0, 1] },
		},
		#{
			// One symbol, nothing else counted: it gets one digit.
			options: #{ input_base: 10, output_base: 2, smoothing: 0 },
			inputs: #{ plain_text: symbols(10, "777") },
			outputs: #{ cipher_text: [1, 1, 1] },
		},
		#{
			options: #{ input_base: 27, output_base: 3, smoothing: -1 },
			inputs: #{ plain_text: symbols(27, "HI") },
			error: "smoothing must be at least 0",
		},
	]
}
