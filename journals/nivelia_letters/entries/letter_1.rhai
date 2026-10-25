// Nivelia's hand, traced from her letter to Loche and the torn slip that
// came with it: the nivelia glyphset. Page "letter" sets the letter's own words
// as she wrote them (in her letter-swapping cipher) to compare against the
// photograph; page "slip" sets the slip; page "alphabet" shows every glyph.
// The letter has no f, m, t or y (the cipher swaps them out), so those, and
// l and k, come from the slip. The only capitals she wrote are A, I, K, L, R,
// X and Z; the glyphset falls back to loche_upper for the others, its glyphs
// drawn to loche_upper's baseline and her ascenders as tall as its capitals.

// Glyphs are 1.08 units to the x-height on a baseline at 0.81 (loche_upper's),
// and drawn at half their size, so at size 60 the x-height is 32 canvas
// units.
let size = 60.0;
let xh = size * 1.08 / 2.0;
let hand = #{
	type: new_type_style(glyphset("nivelia"), size),
	style: new_style(new_brush_style("rgba(30, 26, 24, 0.92)", "Ink Pen", 2.0, #{ min_pressure: 0.6, max_pressure: 0.8 }), new_handwriting_style()),
};

// One line of text from `at` on its baseline: glyphs are placed with their
// origin on the line, the baseline 0.81 units below it.
fn on_baseline(text, at, hand) {
	write_line(text, at - vec2(0.0, hand.type.glyph_scale * 0.81 / 2.0), hand)
}

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
		page_text.add(on_baseline(line[1], vec2(line[0] * xh, n * leading), hand));
	}
}
journal_entry.add(new_page(#{ name: "letter" }).add(page_text));

// The slip: four lines, the last centred under the rest.
let slip = [
	[1.0, "the outlander zealot has no qualm as she tries"],
	[0.0, "to fix the path. the jury accepts no alibi and"],
	[0.0, "offers no second chance. the unending wick"],
	[9.0, "shall never burn the veil."],
];
let slip_text = new_canvas();
for (line, n) in slip {
	slip_text.add(on_baseline(line[1], vec2(line[0] * xh, n * xh * 2.8), hand));
}
journal_entry.add(new_page(#{ name: "slip" }).add(slip_text));

let alphabet = new_canvas();
alphabet.add(on_baseline("a b c d e f g h i j k l m", vec2(0.0, 0.0), hand));
alphabet.add(on_baseline("n o p q r s t u v w x y z", vec2(0.0, leading), hand));
alphabet.add(on_baseline("A I K L R X Z . , '", vec2(0.0, leading * 2.0), hand));
alphabet.add(on_baseline("B C D E F G H J M N O P Q S T U V W Y", vec2(0.0, leading * 3.0), hand));
alphabet.add(on_baseline("the quick brown fox jumps over the lazy dog", vec2(0.0, leading * 4.0), hand));
journal_entry.add(new_page(#{ name: "alphabet" }).add(alphabet));
