// many_eye_lock: This lock splits one large message into several smaller
// keyword ciphers.

fn chain(o) {
	#{
		locks: #{
			key: #{ type: "shift_cipher", options: #{ base: 27, shift: 0 } },
			message: #{ type: "split_symbols", options: #{ base_n: 27, output_count: 7 } },
			vc1: #{ type: "vigenere_cipher", options: #{ cipher_base: 27, key_base: 27, plain_base: 27 } },
			vc2: #{ type: "vigenere_cipher", options: #{ cipher_base: 27, key_base: 27, plain_base: 27 } },
			vc3: #{ type: "vigenere_cipher", options: #{ cipher_base: 27, key_base: 27, plain_base: 27 } },
			vc4: #{ type: "vigenere_cipher", options: #{ cipher_base: 27, key_base: 27, plain_base: 27 } },
			vc5: #{ type: "vigenere_cipher", options: #{ cipher_base: 27, key_base: 27, plain_base: 27 } },
			vc6: #{ type: "vigenere_cipher", options: #{ cipher_base: 27, key_base: 27, plain_base: 27 } },
			vc7: #{ type: "vigenere_cipher", options: #{ cipher_base: 27, key_base: 27, plain_base: 27 } },
		},
		connect: [
			["key.cipher_text", "vc1.key"],
			["key.cipher_text", "vc2.key"],
			["key.cipher_text", "vc3.key"],
			["key.cipher_text", "vc4.key"],
			["key.cipher_text", "vc5.key"],
			["key.cipher_text", "vc6.key"],
			["key.cipher_text", "vc7.key"],
			["message.output_0", "vc1.plain_text"],
			["message.output_1", "vc2.plain_text"],
			["message.output_2", "vc3.plain_text"],
			["message.output_3", "vc4.plain_text"],
			["message.output_4", "vc5.plain_text"],
			["message.output_5", "vc6.plain_text"],
			["message.output_6", "vc7.plain_text"],
		],
		fixed: #{},
		renderers: #{
			e1: #{ type: "eye", options: #{}, inputs: #{ iris: "vc1.key", pupil: "vc1.cipher_text" } },
			e2: #{ type: "eye", options: #{}, inputs: #{ iris: "vc2.key", pupil: "vc2.cipher_text" } },
			e3: #{ type: "eye", options: #{}, inputs: #{ iris: "vc3.key", pupil: "vc3.cipher_text" } },
			e4: #{ type: "eye", options: #{}, inputs: #{ iris: "vc4.key", pupil: "vc4.cipher_text" } },
			e5: #{ type: "eye", options: #{}, inputs: #{ iris: "vc5.key", pupil: "vc5.cipher_text" } },
			e6: #{ type: "eye", options: #{}, inputs: #{ iris: "vc6.key", pupil: "vc6.cipher_text" } },
			e7: #{ type: "eye", options: #{}, inputs: #{ iris: "vc7.key", pupil: "vc7.cipher_text" } },
		},
	}
}

// Where the web editor draws each lock and renderer.
fn layout() {
	#{ key: [-120, 34], message: [-152, 267], vc1: [479, -113], vc2: [479, -24], vc3: [474, 75], vc4: [472, 178], vc5: [471, 280], vc6: [464, 394], vc7: [467, 504], e1: [808, -109], e2: [807, -12], e3: [822, 79], e4: [789, 181], e5: [758, 290], e6: [762, 407], e7: [766, 502] }
}
