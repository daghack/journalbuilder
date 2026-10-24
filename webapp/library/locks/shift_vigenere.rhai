// shift_vigenere: the key shifted along the alphabet, then used as the key
// of a Vigenère cipher. A composite lock: shift_cipher wired into
// vigenere_cipher.

fn options() {
	#{ shift: integer(3) }
}

fn chain(o) {
	#{
		locks: #{
			shift: #{ type: "shift_cipher", options: #{ base: 27, shift: o.shift } },
			vig: #{ type: "vigenere_cipher", options: #{ cipher_base: 27, key_base: 27, plain_base: 27 } },
		},
		connect: [
			["shift.cipher_text", "vig.key"],
		],
		fixed: #{},
		inputs: #{ key: "shift.plain_text", plain_text: "vig.plain_text" },
		outputs: #{ cipher_text: "vig.cipher_text", shifted_key: "vig.key" },
	}
}

// Where the web editor draws each lock.
fn layout() {
	#{ shift: [120, 60], vig: [480, 60] }
}

fn examples() {
	[
		#{
			options: #{ shift: 3 },
			inputs: #{ key: symbols(27, "KEY"), plain_text: symbols(27, "ATTACK AT DAWN") },
			outputs: #{ cipher_text: symbols(27, "OAUOKLNIUNLBJV"), shifted_key: symbols(27, "NHA") },
		},
	]
}
