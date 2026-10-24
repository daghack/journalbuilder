// The Book of Lang's script, traced from the book: lang_upper_* and
// lang_lower_*. Page "passage" sets the book's own words (from "The Echoes
// of Power") to compare against the scan; page "alphabet" shows every glyph.
// The book has no capital H, J, K, Q, U, X, Y or Z; those are drawn in its
// hand rather than traced.

// Glyphs are 0.8 units to the x-height and drawn at half their size, so at
// size 80 the x-height is 32 canvas units. The book leaves about 0.3
// x-heights between letters: 0.12 of the glyph scale.
let size = 80.0;
let hand = #{
	type: new_type_style(glyphset("lang"), size),
	style: new_style(new_brush_style("rgba(25, 20, 15, 0.92)", "Calligraphy", 2.3, #{ min_pressure: 0.52, max_pressure: 0.91, pressure_profile: "constant" }), new_handwriting_style()),
};
let leading = size * 0.4 * 2.4;   // the book's baselines are 2.4 x-heights apart

// One line of text from `at` on its baseline: glyphs are placed on the line
// through the middle of their x-height.
fn on_baseline(text, at, hand) {
	write_line(text, at - vec2(0.0, hand.type.glyph_scale * 0.4 / 2.0), hand)
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
let page_text = new_canvas();
for (text, n) in passage {
	if text != "" {
		page_text.add(on_baseline(text, vec2(0.0, n * leading), hand));
	}
}
journal_entry.add(new_page(#{ name: "passage" }).add(page_text));

let alphabet = new_canvas()
	.add(on_baseline("A B C D E F G H I J K L M N O P Q R S T U V W X Y Z", vec2(0.0, 0.0), hand))
	.add(on_baseline("a b c d e f g h i k l m n o p q r s t u v w x y z", vec2(0.0, leading), hand))
	.add(on_baseline("the quick brown fox jumps over the lazy dog", vec2(0.0, leading * 2.0), hand));
journal_entry.add(new_page(#{ name: "alphabet" }).add(alphabet));
