// Journal entry 17: two pages.
//
//   maze              a confession, enciphered (substitution, then Vigenère
//                     keyed by the substituted text) and written into a maze
//                     of balanced-ternary glyphs; the key flows around it.
//   balanced_ternary  a dial of the 27 balanced-ternary glyphs and their
//                     digits, with short notes scattered over it.

let maze_brush = new_brush("Calligraphy", 0.15, 0.25, 1.5);
let dial_brush = new_brush("Calligraphy", 0.15, 0.25, 1.5);
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
	let g = font("balanced_ternary").glyph((cell.symbol - 13));
	g.scale = cell_size;
	g.position = vec2(cell.x * cell_size, cell.y * cell_size);
	cells.push(g);
	avoid.add_glyph(g, new_style(2.0, "black", maze_brush));
}
let b = avoid.bounds();

let page_canvas = new_canvas();
page_canvas.add_glyphs(cells, new_style(5.0, "black", maze_brush));

// The key, flowed through a box a little larger than the maze, each glyph
// nudged a little off true.
let around = new_textbox(#{
	center: b.center, width: b.width * 1.2, height: b.height * 1.2,
	symbol_scale: 52.0, avoid: avoid,
});
let filled = around.fill(ternary_font("balanced_ternary").glyphs(key));
if filled.unplaced.len() > 0 {
	warn(filled.unplaced.len() + " key symbols didn't fit around the maze");
}
let key_style = new_style(1.5, "darkred", maze_brush);
for g in filled.placed {
	g.scale_by(1.0 + random_range(-0.025, 0.025));
	g.translate_by(vec2(random_range(-2.5, 2.5), random_range(-2.5, 2.5)));
	g.rotate_by(random_range(-PI() / 32.0, PI() / 32.0));
	page_canvas.add_glyph(g, key_style);
}

let maze_page = new_page();
maze_page.name = "maze";
maze_page.svg_style = svg_look;
maze_page.add_canvas(page_canvas);
journal_entry.add_page(maze_page);

// ---- page two: the balanced-ternary dial ----

// Rings of digit glyphs, each in a circle. Ring one holds every digit,
// -13 to 13, with 0 at the top; each inner ring holds the digits a power of
// three apart, and lines join every digit to its rounded third inward.
let symbol_radius = 50.0;
let circle_radius = symbol_radius / 1.5;
let ring_step = symbol_radius * 4.0;
let outer_ring = 600.0;
let brush = dial_brush;

fn ring_point(count, step, radius) {
	let angle = step * 2.0 * PI() / count - PI() / 2.0;
	#{ angle: angle, center: vec2(radius * angle.cos(), radius * angle.sin()) }
}

fn add_digit(digit, at, style) {
	let g = font("balanced_ternary").glyph(digit);
	g.scale = 50.0;
	g.position = at.center;
	g.rotation = at.angle + PI() / 2.0;
	this.add_path(new_circle(at.center, 50.0 / 1.5), style);
	this.add_glyph(g, style);
}

let dial = new_canvas();
let rings = [
	#{ count: 27, digit_step: 1, width: 3.0 },
	#{ count: 9, digit_step: 3, width: 4.5 },
	#{ count: 3, digit_step: 9, width: 6.75 },
];
for (ring, r) in rings {
	let radius = outer_ring - r * ring_step;
	let style = new_style(ring.width, "black", brush);
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
			dial.add_path(new_line(from, to), style);
		}
	}
}

// Concentric circles at the hub.
let hub_style = new_style(6.75, "black", brush);
let hub_max = outer_ring - 2.0 * ring_step - symbol_radius / 2.0;
for i in 0..5 {
	dial.add_path(new_circle(vec2(0.0, 0.0), 15.0 + i / 5.0 * (hub_max - 15.0)), hub_style);
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
dial.add_canvas(dial.scatter_notes(shorts, font("loche_upper"), new_style(2.0, "darkred", brush),
	#{ symbol_scale: 30.0, width_range: [200, 500], height_range: [300, 600] }));

let dial_page = new_page();
dial_page.name = "balanced_ternary";
dial_page.svg_style = svg_look;
dial_page.add_canvas(dial);
journal_entry.add_page(dial_page);
