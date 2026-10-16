// Nivelia's hand, traced from her letter to Loche and the torn slip that
// came with it: the nivelia font. Page "letter" sets the letter's own words
// as she wrote them (in her letter-swapping cipher) to compare against the
// photograph; page "slip" sets the slip; page "alphabet" shows every glyph.
// The letter has no f, m, t or y (the cipher swaps them out), so those, and
// l and k, come from the slip. The only capitals she wrote are A, I, K, L, R,
// X and Z; the font falls back to loche_upper for the others, its glyphs
// drawn to loche_upper's baseline and her ascenders as tall as its capitals.

let font = font("nivelia");
// Glyphs are 1.08 units to the x-height on a baseline at 0.81 (loche_upper's),
// and drawn at half their size, so at size 60 the x-height is 32 canvas
// units.
let pen = new_brush("Ink Pen", 0.6, 0.8, 2.0);
let ink = new_style(1.0, "rgba(30, 26, 24, 0.92)", pen);

// One line of text from `at` on its baseline.
fn write_line(text, at, size, style, font) {
	let line = new_canvas();
	// Glyphs are placed with their origin on the line, the baseline 0.81
	// units below it.
	let y = at.y - size * 0.81 / 2.0;
	let path = new_line(vec2(at.x, y), vec2(at.x + 4000.0, y));
	for c in path.fill_with_text(text, font, size, style) { line.add_canvas(c); }
	line
}

let size = 60.0;
let xh = size * 1.08 / 2.0;

// The letter: [indent in x-heights, text]. Its baselines are about 3.8
// x-heights apart.
let letter = [
	[1.0, "Loche,"],
	[3.0, "A iq ixoias rX jrg, ibs A srb'z Kibz zr nc."],
	[0.0, "Ibs A wbrK az ad nceigdc A sr brz"],
	[1.0, "gbscodzibs jrg. zprgup hcopihd jrg hocxco az"],
	[0.0, "zpiz Kij."],
	[],
	[2.0, "Zogzpxg vvj, zpcoc ioc qroc hcrhvc A sr brz"],
	[0.0, "gbscodzibs zpib zprdc A sr, ngz dgep ad vaxc"],
	[0.0, "Kpcb oiadcs ab adrvizarb."],
	[4.0, "A Krgvs ocivvj vaws zr nc jrgo ivvj, ax"],
	[0.0, "xro br rzpco ocidrb zpib brz pilabu zr xcio"],
	[1.0, "dvcchabu zKr xccz xorq jrg. Ivdr A vaws Iverbi"],
	[0.0, "ibs dpc vawse jrg, ibs qj bcK pgdnibsd ibs Kalcd"],
	[0.0, "dccq zr vaws jrg, dr A's vaws zr vaws jrg id"],
	[0.0, "Kcvv. Ro iz vcidz brz xcio jrg."],
	[3.0, "A Kid scvaupzcs zr dpioc i qciv Kazp jrg rb"],
	[0.0, "qj Kcssabu baupz. A prhc zpcoc eib nc qroc ab"],
	[0.0, "zpc xgzgoc."],
	[3.0, "A Koazc zr jrg Kazp rhcb pibsd, br"],
	[7.0, "zoaewcoj ro scecaz."],
	[],
	[3.0, "Kazp ocdhcez,"],
];
let leading = xh * 3.8;
let page_text = new_canvas();
for (line, n) in letter {
	if line.len() > 0 {
		page_text.add_canvas(write_line(line[1], vec2(line[0] * xh, n * leading), size, ink, font));
	}
}
let p1 = new_page();
p1.name = "letter";
p1.add_canvas(page_text);
journal_entry.add_page(p1);

// The slip: four lines, the last centred under the rest.
let slip = [
	[1.0, "the outlander zealot has no qualm as she tries"],
	[0.0, "to fix the path. the jury accepts no alibi and"],
	[0.0, "offers no second chance. the unending wick"],
	[9.0, "shall never burn the veil."],
];
let slip_text = new_canvas();
for (line, n) in slip {
	slip_text.add_canvas(write_line(line[1], vec2(line[0] * xh, n * xh * 2.8), size, ink, font));
}
let p2 = new_page();
p2.name = "slip";
p2.add_canvas(slip_text);
journal_entry.add_page(p2);

let alphabet = new_canvas();
alphabet.add_canvas(write_line("a b c d e f g h i j k l m", vec2(0.0, 0.0), size, ink, font));
alphabet.add_canvas(write_line("n o p q r s t u v w x y z", vec2(0.0, leading), size, ink, font));
alphabet.add_canvas(write_line("A I K L R X Z . , '", vec2(0.0, leading * 2.0), size, ink, font));
alphabet.add_canvas(write_line("B C D E F G H J M N O P Q S T U V W Y", vec2(0.0, leading * 3.0), size, ink, font));
alphabet.add_canvas(write_line("the quick brown fox jumps over the lazy dog", vec2(0.0, leading * 4.0), size, ink, font));
let p3 = new_page();
p3.name = "alphabet";
p3.add_canvas(alphabet);
journal_entry.add_page(p3);
