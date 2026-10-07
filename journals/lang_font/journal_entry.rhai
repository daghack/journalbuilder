// The Book of Lang's script, traced from the book: lang_upper_* and
// lang_lower_*. Page "passage" sets the book's own words (from "The Echoes
// of Power") to compare against the scan; page "alphabet" shows every glyph.

let font = new_fontset(["lang_upper"], ["lang_lower"]);
// Glyphs are 0.8 units to the x-height and drawn at half their size, so at
// size 80 the x-height is 32 canvas units.
let nib = new_brush("Calligraphy", 0.4, 0.7, 2.3);
nib.pressure_profile = "constant";
let ink = new_strokestyle(1.3, "rgba(25, 20, 15, 0.92)", nib);

// One line of text from `at` on its baseline. The book leaves about 0.3
// x-heights between letters: 0.12 of the font size.
fn write_line(text, at, size, style, font) {
	let line = new_canvas();
	// Glyphs are placed on the line through the middle of their x-height.
	let y = at.y - size * 0.4 / 2.0;
	let path = new_line(new_vec(at.x, y), new_vec(at.x + 4000.0, y));
	for c in path.fill_with_msg(style, font, size, text, #{ spacing: 0.12 }) { line.add_canvas(c); }
	line
}

let passage = [
	"Malket is the ancient practice of",
	"repeating a word or phrase multiple",
	"times.",
	"",
	"Malket is a word derived from the",
	"draconic phrase Mol kin Tet, the will",
	"of the word, and was designed to help",
	"practitioners of magic access a higher",
	"power and the true nature of Form.",
];
let size = 80.0;
let leading = size * 0.4 * 2.4;   // the book's baselines are 2.4 x-heights apart
let page_text = new_canvas();
for (text, n) in passage {
	if text != "" {
		page_text.add_canvas(write_line(text, new_vec(0.0, n * leading), size, ink, font));
	}
}
let p1 = new_page();
p1.name = "passage";
p1.add_canvas(page_text);
journal_entry.add_page(p1);

let alphabet = new_canvas();
alphabet.add_canvas(write_line("A B C D E F G I L M N O P R S T V W", new_vec(0.0, 0.0), size, ink, font));
alphabet.add_canvas(write_line("a b c d e f g h i k l m n o p q r s t u v w x y z", new_vec(0.0, leading), size, ink, font));
alphabet.add_canvas(write_line("the quick brown fox jumps over the lazy dog", new_vec(0.0, leading * 2.0), size, ink, font));
let p2 = new_page();
p2.name = "alphabet";
p2.add_canvas(alphabet);
journal_entry.add_page(p2);
