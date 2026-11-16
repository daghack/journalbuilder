// mere_deep: the deep's two secrets. The tablets' (tablets), a trifid
// cipher under its key; and the whirls' (whirls), a chaocipher, its two
// alphabets the whirls.

fn chain(o) {
	#{
		locks: #{
			tablets: #{ type: "trifid", options: #{ period: 5 } },
			whirls: #{ type: "chaocipher", options: #{ base: 27 } },
		},
		connect: [],
		fixed: #{
			"tablets.plain_text": symbols(27, "THE TWO WHIRLS TURN AS ONE"),
			"tablets.key": symbols(27, "OBSIDIAN"),
			"whirls.plain_text": symbols(27, "IT HUNTS BENEATH THE STILL WATER"),
			"whirls.left": symbols(27, "MAELSTROWBCDFGHIJKNPQUVXYZ "),
			"whirls.right": symbols(27, "WHIRLPO ABCDEFGJKMNQSTUVXYZ"),
		},
		illustrations: #{},
	}
}
