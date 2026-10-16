// Journal entry 18: two texts set in small word-wrapped boxes and laid out
// along a left-handed walk through a maze, each box drawn from one text or
// the other at random. Short notes are scattered over the top.

let fateful_pages = "In my quiet moments of contemplation I consider often the Gospel of Saint Decimus. While it is true that a great number of those passages speak directly to the soul of me, this Market, as many others, leave in me a resonance with one phrase in particular: Fate is not subtle. A letter was delivered to me this market, not by the hand of mortal mail-carrier, but by the way it lay, slightly crumpled and tucked to the side of the tavern. When I saw the name upon it, I knew then the message for what it was—a writ to me from lines predescribed upon the stars in the sky as much as one from a Quiet compatriot to you. Fate is not subtle. I knew from the moment I picked up the slightly ripped paper that it was a thing to be returned and not kept. When later I picked up the token which read of a prize to be obtained aft speaking to you, I knew the time to do so was imminent, and went to the tavern to wait your inevitable morning arrival. It was there, with your letter and your token near at hand that I was contemplating when and how best to approach you with what was yours that, once again, Fate was characteristically unsubtle—a stranger approached me and handed me a letter, informing me that she had found it pinned to the tavern front; it was one of my own so artfully borrowed by you—and with timely answer of my own prayers, I knew to likewise return to you yours and thereby receive my prize from you. And what a prize it was. It is a conversation I cannot pull from my mind—delicate, porcelain words too fragile to be spoken but too urgent to be set wholly aside playing in the corners of mirthful lips and eyes glowing with a light too brilliant and wild to wholly disguise.";

let spider_web = "Blind little spider, come out to play: the venom as you say remains, and silks untwine in patient, well-familiar shapes. Path by path, we build out our ways and methods by which we predate, to feast or play on prey in these meager efforts to persist from day to day. Neither patience nor venom does danger make, but rather the hungers that we wish to slake that we might bring an end to our soul's ceaseless ache. Ochre-stained, my master ate from plate and bade that I must likewise sate. In obedience I mused, scooped on my spoon a helping of that putrid offal sat in stew of congealed and rancid fat. In that moment then, I thought I knew the lesson meant for me and devoured senselessly, indulging in its phantom taste, welcome meals in heat-wrought wastes. You say you feel mad for the thoughts you hold ... To feel madness in the presence of purpose is not a flaw. It is a sign that you are listening. Enlightenment came to me a Door, impossible and Black on some strange ocean shore. It bore no knob nor way to knock, but set upon its surface a strange-shaped lock whose existence never can be met with strange-matched key—but here listen long and you will find the wretched blasphemy that haunts behind, the truth that, in dreams, most Lazarines unknowingly strive to know and find. Listen in, my spider dear, and you will learn we play a game, each therein our piece is placed then our peace displaced, and in this we have no say. I seek not to convert, for this is not so direct my way, but I warn you this: should you see this Door so Black, turn away—do not look back. Eschew in thought black ill-lumen gate, instead dream of better days where Lazarolth's truth lay before us, skewered, ripped apart, and burned away. Fill your ache, but refuse to feed on feasts that Silent Queen brings from black and unrepentant dreams.";

let shorts = [
	"There are some games you can only play when you do not yet know the rules.",
	"People bring their own delights to bear.",
	"Fate is not subtle. Had I not been drawn after it, I would not have pressed my hands to the cold stones of that wall.",
	"I would not have lingered in that place, stretching my fingers for that glinting scrap of metal. I would not have heard them.",
	"From beyond the Door, carried on the hush of night, came voices. Words. I could not help but listen, at first out of passing curiosity, but then, as their meaning unraveled, out of horror.",
	"It was in the quiet of the night, beneath the endless vault of the heavens, that Cyaniel came to me-not in voice nor in form, but in knowing.",
	"The realization struck as surely as any blade: every act, no matter how minute, shifts the course of history.",
	"A hand laid upon a friend's shoulder. A kindness given. A word spoken or withheld. Each choice is a stone cast into the waters of eternity, and the ripples never cease.",
	"The wind had stilled. The coin at my feet, so dogged in its journey, was forgotten. The voices faded into the stillness, leaving me alone beneath the unblinking eyes of the heavens.",
	"The stars were bright above me. So bright, so endless, dizzying as they turned through their great dance. And then, they pressed into me.",
	"I felt them, as though they were not distant fires but hands, unseen yet firm. A weightless pressure upon my skin, a whisper of heat against my check. My chin lifted, though not of my own accord, my gaze drawn upward as though a gentle hand tilted my head to the sky.",
	"And in that moment, I saw.",
];

let box_size = 150.0;
let symbol_scale = 19.0;
let font = font("loche_upper");
let boxes = new_textbox(#{
	center: vec2(0.0, 0.0), width: box_size, height: box_size,
	symbol_scale: symbol_scale, layout: "words", jagged_edge: 0.5,
});
let fateful = boxes.place_all(font.glyphs(fateful_pages));
let spider = boxes.place_all(font.glyphs(spider_web));

// A maze of n x n nodes has 2n² - 1 cells, passages included: enough for
// every box.
let nodes = ((fateful.len() + spider.len()) / 2.0).sqrt().ceiling().to_int();
let spacing = box_size + symbol_scale;

// Nib sizes are in canvas units, like everything else; 2.43 keeps the look
// of the legacy render (which fit this page to 1080 px with a 1.75 nib).
let brush = new_brush("Calligraphy", 0.15, 0.25, 2.43);
let fateful_style = new_style(2.0, "black", brush);
let spider_style = new_style(2.0, "hsl(160, 50%, 40%, 0.8)", brush);

let canvas = new_canvas();
let next_fateful = 0;
let next_spider = 0;
for cell in new_maze(1043, nodes, nodes).cells() {
	// Prefer one text at random, falling back to the other once it runs out.
	let take_fateful = if random() < 0.5 { next_fateful < fateful.len() } else { next_spider >= spider.len() };
	let block = ();
	let style = ();
	if take_fateful && next_fateful < fateful.len() {
		block = fateful[next_fateful];
		style = fateful_style;
		next_fateful += 1;
	} else if next_spider < spider.len() {
		block = spider[next_spider];
		style = spider_style;
		next_spider += 1;
	} else {
		break;
	}
	let offset = vec2(cell.x * spacing, cell.y * spacing);
	for g in block {
		canvas.add_glyph(g.translate_by(offset), style);
	}
}

canvas.add_canvas(canvas.scatter_notes(shorts, font, new_style(2.0, "hsl(200, 50%, 40%, 0.8)", brush),
	#{ symbol_scale: 23.0, width_range: [200, 500], height_range: [300, 600] }));

let page = new_page();
page.name = "maze_textbox";
page.svg_style = #{ noise_amplitude: 1.5 };
page.add_canvas(canvas);
journal_entry.add_page(page);
