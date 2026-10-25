// Journal entry 17: two pages.
//
//   maze              a confession, enciphered (substitution, then Vigenère
//                     keyed by the substituted text) and written into a maze
//                     of balanced-ternary glyphs; the key flows around it.
//   balanced_ternary  a dial of the 27 balanced-ternary glyphs and their
//                     digits, with short notes scattered over it.

// The Calligraphy nib in `color`, pressed `weight` times as hard as it
// presses lightly (Darkly's pressure stops at 1).
fn calligraphy(color, weight) {
	let nib = new_brush_style(color, "Calligraphy", 1.5, #{ min_pressure: (0.15 * weight).min(1.0), max_pressure: (0.25 * weight).min(1.0) });
	new_style(nib, new_handwriting_style())
}
let svg_look = #{ roughness: 2.0, noise_amplitude: 1.5 };

// ---- page one: the maze ----

let confession = "IT IS A TRULY AND DEEPLY INCONVENIENT TIME FOR THE BENALIONS TO BE SO CONCERNED WITH THE DETAILS OF THEIR RITES I DO NOT INTEND TO END IN COMPLETE MY BLASPHEMIES NOR IS THE INQUISITIONERS PRICE TO NECESSARILY CLEANSE ME A PRICE I FIND MYSELF WILLING OR ABLE TO PAY PERHAPS WHAT THEY CALL MEANING IS NOT THE SAME AS THAT WHICH I PURSUE FOR WHAT I PURSUE I FIND AMONG THE THORNS IN TOWN WHATEVER PRICE AND POWER THEY ARE SUPPOSED TO HAVE PAID";
let conversion = "I AM BEGINNING TO DOUBT MY CONVERSION IF IN EVERY WAY I AM PULLED TO HER AND HER HUNGERS HOW AM I TO BE ANYTHING OTHER THAN HER CREATURE AS BEFORE";

let lock = new_lock("page_one_lock")
	.add("prime_sub", "substitution_cipher", #{ base_n: 27 })
	.add("maze_content", "vigenere_cipher", #{ key_base: 27, plain_base: 27, cipher_base: 27 })
	.add("maze_encode", "maze_lock", #{ input_base: 27, strategy: "LeftHanded" })
	.connect("prime_sub.cipher_text", "maze_content.key")
	.connect("maze_content.cipher_text", "maze_encode.maze_text")
	.set_input("prime_sub.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS"))
	.set_input("prime_sub.plain_text", symbols(27, confession))
	.set_input("maze_content.plain_text", symbols(27, conversion))
	.set_input("maze_encode.random_seed", symbols(10, "10843"));
let result = lock.run(#{});
let maze = result.get("maze_encode.generated_maze").maze();
let key = result.trace()["maze_content.key"].text();

// One glyph per maze cell. A thin copy of the glyphs is what the key keeps
// clear of.
let cell_size = 50.0;
let avoid = new_canvas();
let cells = [];
for cell in maze.cells() {
	let g = glyphset("balanced_ternary").glyph(cell.symbol - 13, #{ scale: cell_size, position: vec2(cell.x * cell_size, cell.y * cell_size) });
	cells.push(g);
	avoid.add(g, calligraphy("black", 2.0));
}
let b = avoid.bounds();

let page_canvas = new_canvas().add(cells, calligraphy("black", 5.0));

// The key, flowed through a box a little larger than the maze, each glyph
// nudged a little off true.
let around = new_textbox(#{
	center: b.center, width: b.width * 1.2, height: b.height * 1.2,
	type: #{ glyph_scale: 52.0 }, avoid: avoid,
	style: #{ glyph_position_jitter: 2.5, glyph_rotation_jitter: PI() / 32.0, glyph_scale_jitter: 0.025 },
});
let filled = around.fill(ternary_glyphset("balanced_ternary").glyphs(key));
if filled.unplaced.len() > 0 {
	warn(filled.unplaced.len() + " key symbols didn't fit around the maze");
}
page_canvas.add(filled.placed, calligraphy("darkred", 1.5));

journal_entry.add(new_page(#{ name: "maze", svg_style: svg_look }).add(page_canvas));

// ---- page two: the balanced-ternary dial ----

// Rings of digit glyphs, each in a circle. Ring one holds every digit,
// -13 to 13, with 0 at the top; each inner ring holds the digits a power of
// three apart, and lines join every digit to its rounded third inward.
let symbol_radius = 50.0;
let circle_radius = symbol_radius / 1.5;
let ring_step = symbol_radius * 4.0;
let outer_ring = 600.0;

fn ring_point(count, step, radius) {
	let angle = step * 2.0 * PI() / count - PI() / 2.0;
	#{ angle: angle, center: vec2(radius * angle.cos(), radius * angle.sin()) }
}

fn add_digit(digit, at, style) {
	this.add(new_circle(at.center, 50.0 / 1.5), style);
	this.add(glyphset("balanced_ternary").glyph(digit, #{ scale: 50.0, position: at.center, rotation: at.angle + PI() / 2.0 }), style);
}

let dial = new_canvas();
let rings = [
	#{ count: 27, digit_step: 1, width: 3.0 },
	#{ count: 9, digit_step: 3, width: 4.5 },
	#{ count: 3, digit_step: 9, width: 6.75 },
];
for (ring, r) in rings {
	let radius = outer_ring - r * ring_step;
	let style = calligraphy("black", ring.width);
	let half = ring.count / 2;
	for i in -half..=half {
		let at = ring_point(ring.count, i, radius);
		dial.add_digit(i * ring.digit_step, at, style);
		if r + 1 < rings.len() {
			// From this circle's inner edge to the outer edge of the circle
			// for the digit rounded to the next ring's step.
			let inner = ring_point(rings[r + 1].count, (i / 3.0).round(), radius - ring_step);
			let from = vec2(at.center.x - circle_radius * at.angle.cos(), at.center.y - circle_radius * at.angle.sin());
			let to = vec2(inner.center.x + circle_radius * inner.angle.cos(), inner.center.y + circle_radius * inner.angle.sin());
			dial.add(new_line(from, to), style);
		}
	}
}

// Concentric circles at the hub.
let hub_style = calligraphy("black", 6.75);
let hub_max = outer_ring - 2.0 * ring_step - symbol_radius / 2.0;
for i in 0..5 {
	dial.add(new_circle(vec2(0.0, 0.0), 15.0 + i / 5.0 * (hub_max - 15.0)), hub_style);
}

let shorts = [
	"Lying to you is my new favorite sin, Copper.",
	"It is not enough. It is never enough. Again and again I have pressed pen to page, onl y to know that they will be insufficient. Words are not my strength.",
	"Do you enjoy stargazing? Have you died before? Noviember Atesh, Copper, Crom, Lonan Welcome you to share your story of Death as we gaze at the night sky.",
	"The man wrapped in snakeskin has a box with a secret in it. Who is wearing snakeskin and where does he keep his boxes?",
	"I am beginning to understand that while Alcona and I may be both Collectors, there is a great and hungry chasm between her experience of the warband and my own.",
	"I am beginning to wonder if so much of what I knew of the Collectors was, in fact, true mostly of Jakem and the other Lazerines within the warband.",
	"Walking thoughts still ill-informed, but the man in the snake skin sets my eyes upon a destination - feet, find me my path.",
	"Who was Sir Ingrid? Not Max, I think. Have I forgotten you, and why?",
	"Hold the Hunger, Hunt and Harvest come before Feast.",
	"A bone that heals wrong must be shattered again if it is to bear weight.",
	"To reforge the soul, one must first break it.",
	"Let the wound reopen, let the mind split anew.",
	"A bone that heals wrong must be shattered again if it is to bear weight.",
	"To reforge the soul, one must first break it. Let the wound reopen, let the mind split anew.",
	"slit you open from navel to lip to catalogue everything I find within",
	"unspin your organs across the floor before pinning them on sheets of cardboard",
	"slim chance of winning when our game is only in its beginning",
	"Do you see this world for what it is?",
	"There is no number of walking sticks I could retrieve to balance out the ruin I have wrought",
];
// Notes set askew: each glyph turned and scaled a little before they're placed.
let notes = calligraphy("darkred", 2.0);
notes.glyph_rotation_jitter = PI() / 16.0;
notes.glyph_scale_jitter = 0.15;
dial.add(dial.scatter_notes(shorts, [200, 500], [300, 600], #{
	style: notes, type: new_type_style(glyphset("loche_upper"), 30.0), paragraph: #{ line_start_jitter: 3.0 },
}));

journal_entry.add(new_page(#{ name: "balanced_ternary", svg_style: svg_look }).add(dial));
