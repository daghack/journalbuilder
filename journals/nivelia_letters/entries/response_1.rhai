// Nivelia's glyphset. Some jitter, both rotational and positional.
// Slight ink pooling (0.4), a navy blue color, and a vertical gradiant to a darker blue.
// The pen should be "Rough Ink", with a taper profile and lightly wandering pressure.
const NAVY = "#2b4184";
const DARK_NAVY = "#0b1433";
// The eye's, a little translucent.
const EYE_NAVY = "rgba(43, 65, 132, 0.75)";
const EYE_TEAL = "rgba(46, 232, 214, 0.7)";
const EYE_DARK_NAVY = "rgba(11, 20, 51, 0.6)";
const SIZE = 60.0;
const WIDTH = 1700.0;

let ink = new_gradient([NAVY, DARK_NAVY], #{ vertical: true });
let pen = new_brush_style(ink, "Rough Ink", 2.0, #{ max_pressure: 0.6, pressure_profile: "taper", pressure_wander: 0.15, pool: 0.4 });
let steady = new_handwriting_style();
let paragraph = new_paragraph_style(#{ line_height: 2.0, line_start_jitter: SIZE * 0.5 });
let hand = #{
	style: new_style(pen, new_handwriting_style(#{ glyph_rotation_jitter: 0.06, glyph_position_jitter: [SIZE * 0.04, SIZE * 0.06] })),
	type: new_type_style(glyphset("nivelia"), SIZE),
	paragraph: paragraph,
};

seed(1043);

// This journal will utilize 3 locks.
// Each a substitution cipher: the ordering's symbol at a letter's place
// (space, then A to Z) stands for it. The full one moves every letter but
// keeps spaces.
const WEAK = " BCDEFGHIJKLMNOPQRSTUVWXYZA";
const FULL = " QMFWOZXCKAHIEVRYGUPLDSNBTJ";
const TERNARY = "UOLKPYCTVWQAZRFH SEDBMIXGJN";
let lock = new_lock("nivelia_response")
	.add("weak", "substitution_cipher", #{ base_n: 27 })
	.add("full", "substitution_cipher", #{ base_n: 27 })
	.add("ternary", "substitution_cipher", #{ base_n: 27 })
	.set_input("weak.ordering", symbols(27, WEAK))
	.set_input("full.ordering", symbols(27, FULL))
	.set_input("ternary.ordering", symbols(27, TERNARY));

// Plain Text
let para_1 = "Are you afraid of every person you do not understand? Such constant fear, I do not envy. Worse, if understanding of me would be your remedy, I do not believe curing your ailment is within your grasp. You and I are not of the same kind.";

// A weak substitution cipher, where about half the characters map to themselves so that it's already partially solved.
let para_2 = "This does not mean that we cannot be allies, or that you cannot like me. To love the storm best is to fear it, is it not? And, indeed, you have already identified your strongest avenue to this allyship, for how best to bond with a Collector? As it has ever been, the answer is by meal.";

// This one is a full substitution cipher such that no character maps to itself.
let para_3 = "I know you are not capable of trickery, or deceit. Not, at least, in such a careful and serious endeavor as in a letter to me. I have seen you refuse the smallest of subterfuges, after all. You could not even bring yourself to fetch for me discretely an item I had secreted, and that is the smallest of tasks I could have put to you. It is curious that you felt you had to allay my concerns in this regard. You are guileless, a trait which you sometimes manage to make refreshing.";

// This one is not only a substitution cipher, but uses the ternary symbols to further obscure.
let para_4 = "Here, I shall help you, walk you through, step by step, the path to mastery of these lesser arts of the Collectors. A way to read and hide the most basic of secret things. Consider this my payment for what I have taken from you.";

// Letters and spaces only, in capitals: a base-27 symbol each.
let result = lock.run(#{
	"weak.plain_text": symbols(27, para_2, #{ lenient: true }),
	"full.plain_text": symbols(27, para_3, #{ lenient: true }),
	"ternary.plain_text": symbols(27, para_4, #{ lenient: true }),
});
assert(lock.unlock(result).text("full.plain_text") == symbols(27, para_3, #{ lenient: true }).text(), "the ciphers should undo");

// Set in lowercase, so the letters are Nivelia's own rather than her
// glyphset's capitals (loche_upper's). The ternary is set glyph by glyph.
let flat = paragraph;
flat.layout = "flat";
let ternary = hand + #{ type: new_type_style(ternary_glyphset("balanced_ternary"), SIZE, #{ glyph_outline_gap: 0.32 }), paragraph: flat };
let blocks = [
	[para_1, hand],
	[result.text("weak.cipher_text").to_lower(), hand],
	[result.text("full.cipher_text").to_lower(), hand],
	[result.text("ternary.cipher_text").to_lower(), ternary],
];

let canvas = new_canvas();
let y = 0.0;
for block in blocks {
	let written = write_block(block[0], vec2(0.0, y), WIDTH, block[1]);
	canvas.add(written.canvas);
	y = written.bottom + SIZE;
}

// The signature should be in "loche_upper" instead, and drawn over several times with relatively high jitter and pooling brought down to 0.1. Beneath should be a single straight-edged eye, with a radial gradient with that light teal in the center and that same dark navy blue on the outside.
let signature = "Loche";

let signing_pen = pen;
signing_pen.pool = 0.1;
let signed = write_line(signature, vec2(0.0, y + SIZE), #{
	style: new_style(signing_pen, new_handwriting_style(#{ passes: 4, pass_end_jitter: 9.0 })),
	type: new_type_style(glyphset("loche_upper"), SIZE * 1.5),
});
let name = signed.bounds();
signed.translate_by(vec2(WIDTH / 2.0 - name.center.x, 0.0));
canvas.add(signed);

let centre = vec2(WIDTH / 2.0, name.max.y + SIZE * 1.2);
let eye = art::eye(centre, name.width, #{ flat: true });
let r = eye.iris_radius;
let wash_color = new_gradient([EYE_TEAL, EYE_DARK_NAVY], #{ center: centre, radius: name.width * 0.18 });
let eye_brush = |brush, min, max, size| new_brush_style(wash_color, brush, size, #{ min_pressure: min, max_pressure: max, pressure_profile: "taper" });
// The colour first, under the lines.
canvas.add(eye.wash, new_style(eye_brush.call("Smooth Watercolor", 0.2, 0.35, r * 0.3), steady));
canvas.add(eye.rim, new_style(eye_brush.call("Rough Watercolor", 0.6, 0.95, r * 0.12), steady));
canvas.add(eye.accent, new_style(eye_brush.call("Ink Pen", 0.2, 0.6, 1.2), steady));
pen.color = EYE_NAVY;
canvas.add(eye.lines, new_style(pen, steady));

journal_entry.add(new_page(#{ name: "response" }).add(canvas));
