// The black priests' symbols as a hand writes them: font("black_priests"),
// a to m, each a few strokes simplified from the filled originals
// (scripts/black_priests_font.py). Each page writes them in a hand from an
// earlier journal: large, one by one; at writing size; and hurried, turned
// and wandering, to see whether they stay apart.

const W = 1500.0;
const H = 1300.0;
const INK = "rgba(25, 20, 15, 0.92)";
const LETTERS = "a b c d e f g h i j k l m";
const WORDS = ["fadlem", "bicgke", "hajmdl", "gkebif", "lmcahj", "dgfkbe"];

let hands = [
	// lang_font's book hand
	["lang", new_style(INK, new_brush("Calligraphy", 0.4, 0.7, 2.3, #{ pressure_profile: "constant" }), #{ width: 1.3 })],
	// nivelia_letter's pen
	["nivelia", new_style(INK, new_brush("Ink Pen", 0.6, 0.8, 2.0))],
	// becoming_us's tapering rough ink
	["becoming_us", new_style(INK, new_brush("Rough Ink", 0.3, 0.6, 2.0, #{ pressure_profile: "taper", pressure_variation: 0.15, pool: 0.5, pool_depth: 0.4 }))],
	// matter_of_pyrius's light calligraphy
	["pyrius", new_style(INK, new_brush("Calligraphy", 0.25, 0.6, 1.4))],
];

let font = font("black_priests");
for hand in hands {
	let canvas = new_canvas();
	let style = hand[1];
	let big = #{ font: font, size: 130.0, style: style };
	let small = #{ font: font, size: 60.0, style: style };
	let hurried = small + #{ path_jitter: 0.3, rotation_jitter: 0.3, scale_jitter: 0.2, position_jitter: 0.12 };
	canvas.add(write_line(LETTERS.sub_string(0, 13), vec2(80.0, 120.0), big));
	canvas.add(write_line(LETTERS.sub_string(14), vec2(80.0, 300.0), big));
	canvas.add(write_line(LETTERS, vec2(80.0, 470.0), small));
	let lines = [WORDS[0] + " " + WORDS[1] + " " + WORDS[2], WORDS[3] + " " + WORDS[4] + " " + WORDS[5]];
	let y = 580.0;
	for settings in [small, hurried] {
		for line in lines {
			canvas.add(write_line(line, vec2(80.0, y), settings));
			y += 110.0;
		}
	}
	canvas.add(write_line(LETTERS, vec2(80.0, y + 30.0), hurried));
	journal_entry.add(new_page(#{ name: hand[0] }).add(canvas));
}
