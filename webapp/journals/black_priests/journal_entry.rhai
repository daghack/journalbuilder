// The black priests' symbols as a hand writes them: glyphset("black_priests"),
// a to m, each a few strokes simplified from the filled originals
// (scripts/black_priests_font.py). Each page writes them in a hand from an
// earlier journal: large, one by one; at writing size; and hurried, turned
// and wandering, to see whether they stay apart.

const W = 1500.0;
const H = 1300.0;
const INK = "rgba(25, 20, 15, 0.92)";
const LETTERS = "a b c d e f g h i j k l m";
const WORDS = ["fadlem", "bicgke", "hajmdl", "gkebif", "lmcahj", "dgfkbe"];

let steady = new_handwriting_style();
let hands = [
	// lang_font's book hand
	["lang", new_style(new_brush_style(INK, "Calligraphy", 2.3, #{ min_pressure: 0.52, max_pressure: 0.91, pressure_profile: "constant" }), steady)],
	// nivelia_letter's pen
	["nivelia", new_style(new_brush_style(INK, "Ink Pen", 2.0, #{ min_pressure: 0.6, max_pressure: 0.8 }), steady)],
	// becoming_us's tapering rough ink
	["becoming_us", new_style(new_brush_style(INK, "Rough Ink", 2.0, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15, pool: 0.5, pool_depth: 0.4 }), steady)],
	// matter_of_pyrius's light calligraphy
	["pyrius", new_style(new_brush_style(INK, "Calligraphy", 1.4, #{ min_pressure: 0.25, max_pressure: 0.6 }), steady)],
];

let priests = glyphset("black_priests");
for hand in hands {
	let canvas = new_canvas();
	let style = hand[1];
	let big = #{ style: style, type: new_type_style(priests, 130.0) };
	let small = #{ style: style, type: new_type_style(priests, 60.0) };
	// The same hand in a hurry: lines wandering, glyphs turned, scaled and off true.
	let hasty = style;
	hasty.baseline_wander = 18.0;
	hasty.glyph_rotation_jitter = 0.3;
	hasty.glyph_scale_jitter = 0.2;
	hasty.glyph_position_jitter = 7.2;
	let hurried = small + #{ style: hasty };
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
