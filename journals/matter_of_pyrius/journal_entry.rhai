// The matter of Pyrius: one landscape page in four columns, after
// references/old_journal.png. A letter of grievance enciphered with a
// substitution cipher and written in balanced-ternary script, its key in
// the iris of an eye; between the blocks, lines in a plain hand, a few in
// lowercase, and headings gone over and over in a manic hand.
//
// The key: the iris holds the cipher's ordering, one ternary glyph per
// symbol, clockwise from the top, which is the space, then A to Z. A glyph
// at the n-th place in the ring stands, in the blocks, for the n-th
// symbol.
//
// Every glyph is set a little off true, and every stroke strays (a style's
// jitter); the headings are each stroke gone over many times (its passes).

const W = 2760.0;
const H = 2180.0;
const TOP = 60.0;
const BOTTOM = 2130.0;
const GUTTER = 36.0;
const LEFT = 54.0;
const COLUMN = (W - 2.0 * LEFT - 3.0 * GUTTER) / 4.0;

const INK = "#253443";

// The hand: a fine pen, each stroke a little astray.
let pen = new_brush("Ink Pen", 0.55, 0.9, 1.05);
pen.pressure_profile = "constant";
let hand = new_style(1.0, INK, pen);
hand.jitter = 1.2;
const HAND = hand;

// The cipher's glyphs, a touch heavier.
let cipher_pen = new_brush("Ink Pen", 0.55, 0.9, 1.0);
cipher_pen.pressure_profile = "constant";
let cipher = new_style(1.0, INK, cipher_pen);
cipher.jitter = 1.0;
const CIPHER = cipher;

// The manic hand: every stroke gone over seven times, each pass astray.
let manic_pen = new_brush("Ink Pen", 0.5, 0.95, 1.1);
manic_pen.pressure_profile = "taper";
let manic = new_style(1.0, INK, manic_pen);
manic.passes = 7;
manic.jitter = 9.0;
const MANIC = manic;

// Larger letters gone over just twice.
let firm = new_style(1.0, INK, manic_pen);
firm.passes = 2;
firm.jitter = 3.0;
const FIRM = firm;

const CAPS = font("loche_upper", #{ character_padding: 0.1 });
const LOWER = font("loche_lower", #{ character_padding: 0.38, space_width: 0.7 });
let ternary = ternary_font("balanced_ternary");
ternary.character_padding = 0.32;
const TERNARY = ternary;

seed(1043);

// ---- the cipher ----

// Letters and spaces only, in capitals: a base-27 symbol each.
fn letters_only(text) {
	let out = "";
	let space = true;
	for c in text.to_upper() {
		if c >= 'A' && c <= 'Z' {
			out += c;
			space = false;
		} else if (c == ' ' || c == '-' || c == '—') && !space {
			out += ' ';
			space = true;
		}
	}
	out.trim();
	out
}

let letter = [
	"I have long considered what to do with the matter of Pyrius. I am not offended by rejection, and were it simply a matter of my being personally unfit for clergy, sin-laden or slave to craven habits, I would not be so malcontent",
	"Instead, he reveals such a fundemental misalignment from the path I would take as to be doctrinally incomprehensible. To say that he will not annoint any who has not proved themselves a leader is a blatant departure from established Doctrine and its proper practice. To see and understand this, one need look no farther than the vow of the Cyanites.",
	"For all Pyrius is the recipient of vision, he is shepherd to sheep and does not seem sympathetic in spirit with many of God's six other great angels. I have read oft and contemplated at great length the Cyanite analogies of the waterwheel.",
	"How many hours I have spent imagining the near-perfect turning of its wheels, and how many more I have dreamed of their making? I could not say.",
	"I have come to believe that there is more to the metaphor than its apparent interpretation—that its importance is in the precise and clockwork way the workings of the wheel are made to carry the waters of revelation, the accomplished work of a learned engineer to devise and bring to fruition the preciense of perfect, destined reflection. There is, I know, a deeper kernel of God's way therein.",
	"I see the thousand hours spent in silent service, fine-tuning mechanation with the fine shaving of a sharp knife. I see the thousand hours more spent tending its turning parts with the oils of black sin refined. Would Pyrius say the same? Would Bishop Aranea?",
	"There are more sins than merely murder in Cenotaph, more varied are the guilts of spirit than mere violence. I see in this place endless shades of differentiation, shades of blacks and grays in sharp contrast to the warmth and grace of God's candlelight. I would paint the wall with all their varied occlusions, and by these workings further in my own way His might.",
	"Make your excuses, Pyrius—that I carry the weight of sin, that the order has been corrupted, and that I am not a leader. Each in turn may be true, but I believe I have perceived a more fundamental aspect of you—your path leads so perfectly to Melandhim, so alike in the angel's nature that you cannot imagine the Cyanites have not so far departed their intended path, but merely misused intended tools. What Cyanite's path could lead through you?",
];

const ORDERING = "P DZTINWOFUEYHBJVSCXGLQMAKR";
let lock = new_lock("pyrius").add("sub", "substitution_cipher", #{ base_n: 27 });
let blocks = [];
for paragraph in letter {
	let result = lock.run(#{ "sub.plain_text": symbols(27, letters_only(paragraph)), "sub.ordering": symbols(27, ORDERING) });
	if lock.unlock(result).text("sub.plain_text") != letters_only(paragraph) { throw "the cipher should undo"; }
	blocks.push(result.text("sub.cipher_text"));
}

// ---- writing ----

// A glyph set a little off true, as a hand sets it.
fn jostle(g, size) {
	g.translate_by(vec2(random_range(-0.04, 0.04) * size, random_range(-0.06, 0.06) * size));
	g.rotate_by(random_range(-0.07, 0.07));
	g.scale_by(random_range(0.93, 1.07));
}

// `text` in lines `width` wide from (x, y), `pitch` apart, left edges a
// little ragged. Returns where a line after them would go.
fn lines(canvas, text, font, size, style, x, y, width, pitch, layout) {
	let tb = new_textbox(#{
		center: vec2(x + width / 2.0, 0), width: width, height: size * 0.5,
		symbol_scale: size, layout: layout, jagged_edge: 0.3,
	});
	let rows = tb.place_all(font.glyphs(text));
	for (row, i) in rows {
		for g in row {
			g.translate_by(vec2(0, y + i * pitch));
			jostle(g, size);
		}
		canvas.add_glyphs(row, style);
	}
	y + rows.len() * pitch
}

// What each column writes.
fn prose(c, text, x, y) {
	lines(c, text, global::CAPS, 28, global::HAND, x, y + 16, global::COLUMN, 37, "words") + 22
}
fn lowered(c, text, x, y) {
	lines(c, text, global::LOWER, 34, global::HAND, x, y + 22, global::COLUMN, 44, "words") + 18
}
fn manic(c, text, x, y) {
	lines(c, text, global::CAPS, 82, global::MANIC, x, y + 46, global::COLUMN, 78, "words") + 14
}
fn firm(c, text, x, y) {
	lines(c, text, global::CAPS, 60, global::FIRM, x, y + 34, global::COLUMN, 64, "words") + 12
}
fn ciphered(c, text, x, y) {
	lines(c, text, global::TERNARY, 31, global::CIPHER, x, y + 21, global::COLUMN, 42, "flat") + 22
}

// ---- the eye ----

// An eye `width` across, centred on `centre`, its iris ringed with the
// key: the glyph for each symbol of the ordering, clockwise from the top.
fn eye(c, centre, width) {
	let lids = new_style(1.0, global::INK, new_brush("Ink Pen", 0.55, 0.9, 1.3));
	lids.passes = 2;
	lids.jitter = 2.5;
	let rings = new_style(1.0, global::INK, new_brush("Ink Pen", 0.55, 0.9, 1.4));
	rings.passes = 3;
	rings.jitter = 4.0;
	let key = new_style(1.0, global::INK, new_brush("Ink Pen", 0.5, 0.9, 1.1));
	key.passes = 4;
	key.jitter = 2.8;

	let outer = width * 0.215;
	let inner = outer * 0.56;
	// Lids that just meet the iris, their corners not quite closed.
	let half = width / 2.0;
	let rise = (outer + 6.0) / 0.75;
	let upper = new_path().move_to(centre + vec2(-half, -3))
		.cubic_to(centre + vec2(-half * 0.45, -rise), centre + vec2(half * 0.45, -rise), centre + vec2(half, -3));
	let lower = new_path().move_to(centre + vec2(-half + 8, 9))
		.cubic_to(centre + vec2(-half * 0.45, rise), centre + vec2(half * 0.45, rise), centre + vec2(half - 8, 9));
	c.add_path(upper, lids);
	c.add_path(lower, lids);
	c.add_path(new_circle(centre, outer), rings);
	c.add_path(new_circle(centre, inner), rings);

	let ring = (outer + inner) / 2.0;
	for (g, i) in global::TERNARY.glyphs(global::ORDERING) {
		let angle = -PI() / 2.0 + i * 2.0 * PI() / 27.0;
		g.scale = (outer - inner) * 0.8;
		g.position = centre + vec2(ring * angle.cos(), ring * angle.sin());
		g.rotation = angle + PI() / 2.0;
		jostle(g, 6);
		c.add_glyph(g, key);
	}
}

// ---- the page, column by column ----

let page_canvas = new_canvas();
let c = page_canvas;
let x = [LEFT, LEFT + COLUMN + GUTTER, LEFT + 2.0 * (COLUMN + GUTTER), LEFT + 3.0 * (COLUMN + GUTTER)];
let ends = [];

// One: the eye and its key, then the first of the letter.
eye(c, vec2(x[0] + COLUMN / 2.0, TOP + 120), 560);
let y = TOP + 270;
y = prose(c, "He turned me away at the chapel door and called it mercy. I bowed, as one bows to a shepherd, and kept my tongue behind my teeth. The candles did not gutter. Nothing in that place knew that anything had been decided.", x[0], y);
y = manic(c, "The wheel does not ask", x[0], y);
y = prose(c, "I dreamt of it again: the spokes turning in black water, each paddle lifting its share of the river and letting it fall. \"Who made you?\" I asked it. It did not answer, but it did not stop, and I knew that for an answer of a kind.", x[0], y);
y = ciphered(c, blocks[0], x[0], y);
y = manic(c, "Oils of black sin refined", x[0], y);
y = prose(c, "\"It is not your sin that troubles me,\" he said. I have turned those words over until they shone. If not my sin, then what? My hands? My patience? The knife I keep sharp for the work?", x[0], y);
y = lowered(c, "the fine shaving of a sharp knife.", x[0], y);
ends.push(y);

// Two.
y = TOP;
y = prose(c, "I have watched the Cyanites at their mills. They do not pray aloud. They oil the axle and listen, and when the wheel sings wrong they know it before the water does. That is a faith with its sleeves rolled up.", x[1], y);
y = manic(c, "And what is a leader", x[1], y);
y = ciphered(c, blocks[1], x[1], y);
y = prose(c, "Bishop Aranea would have me kneel. Pyrius would have me follow. Neither would have me build, and it is building I was made for: a thousand hours at the bench, and a thousand more.", x[1], y);
y = manic(c, "But a wheel", x[1], y);
y = lowered(c, "a shepherd to sheep.", x[1], y);
y = ciphered(c, blocks[2], x[1], y);
ends.push(y);

// Three.
y = TOP;
y = ciphered(c, blocks[3], x[2], y);
y = lowered(c, "not a leader, he says.", x[2], y);
y = manic(c, "More sins", x[2], y);
y = ciphered(c, blocks[4], x[2], y);
y = firm(c, "The waters rise", x[2], y);
y = ciphered(c, blocks[5], x[2], y);
y = lowered(c, "grit in the gears.", x[2], y);
y = prose(c, "Let him keep his flock. I will keep the dark, and count its shades.", x[2], y);
ends.push(y);

// Four.
y = TOP;
y = manic(c, "His path leads down", x[3], y);
y = ciphered(c, blocks[6], x[3], y);
y = lowered(c, "turning, turning.", x[3], y);
y = ciphered(c, blocks[7], x[3], y);
y = firm(c, "Melandhim waits", x[3], y);
y = prose(c, "Say that a wheel needs no wright, Pyrius. Say that the water lifts itself.", x[3], y);
ends.push(y);

for (end, i) in ends {
	if end > BOTTOM { warn(`column ${i + 1} runs ${(end - BOTTOM).round()} past the bottom`); }
}

let page = new_page();
page.name = "entry";
page.size = vec2(W, H);
page.add_canvas(page_canvas);
journal_entry.add_page(page);
