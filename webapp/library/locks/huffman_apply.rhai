// huffman_apply: text in input_base rewritten in output_base by a Huffman
// code given as its tree, a HuffmanCode value: one huffman_encode gives
// from its own text, or huffman::code(corpus, input_base, output_base)
// builds. So a message can be locked with another text's code.
//
// The code is the key, passed through: unlocking reads the cipher text
// back with it.

import "lib/huffman" as huff;

fn options() {
	#{ input_base: integer(), output_base: integer() }
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.input_base), code: "HuffmanCode" }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(o.output_base), code: "HuffmanCode" }
}

fn lock(o, i) {
	huff::check(i.code, o);
	#{ cipher_text: huff::encode(i.code, i.plain_text), code: i.code }
}

fn unlock(o, out) {
	huff::check(out.code, o);
	#{ plain_text: huff::decode(out.code, out.cipher_text) }
}

fn examples() {
	[
		#{
			// Base 27 to binary, the code built from a pangram.
			options: #{ input_base: 27, output_base: 2 },
			inputs: #{ plain_text: symbols(27, "EVERY LOCK OPENS"), code: huffman::code(symbols(27, "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG"), 27, 2) },
		},
		#{
			// A, B and C equally common, base 3: one digit each.
			options: #{ input_base: 3, output_base: 3 },
			inputs: #{ plain_text: [2, 0, 1], code: huffman::code([0, 1, 2], 3, 3, #{ smoothing: 0 }) },
			outputs: #{ cipher_text: [2, 0, 1] },
		},
		#{
			options: #{ input_base: 27, output_base: 2 },
			inputs: #{ plain_text: symbols(27, "ABD"), code: huffman::code(symbols(27, "ABC"), 27, 2, #{ smoothing: 0 }) },
			error: "symbol 4 has no code",
		},
		#{
			options: #{ input_base: 27, output_base: 3 },
			inputs: #{ plain_text: symbols(27, "HI"), code: huffman::code(symbols(27, "HI"), 27, 2) },
			error: "but the lock is base 27 to base 3",
		},
	]
}
