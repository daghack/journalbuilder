// Pages in the manner of the Jagged Missal, set
// with the layouts in layouts/: a title page, an article over two-column
// spreads with a drop cap and an illustration, and a full-page figure.
// Each layout is given text, roles (fonts, inks and sizes) and art, and
// chains the text through its pages until it runs out. (The lang and copper
// fonts have no punctuation, so the text has none.)

let nib = new_brush("Calligraphy", 0.4, 0.7, 2.3);
nib.pressure_profile = "constant";
let pen = new_brush("Ink Pen", 0.3, 0.7, 2.0);
// Light inks for a transparent render, viewed on a dark background; dark
// ones on paper (--background).
let on_paper = background() != ();
let pale = if on_paper { "#2b1d12" } else { "#f2ead8" };
let rubric = if on_paper { "#9b2318" } else { "#ff8a7a" };
let ink = new_style(1.3, pale, nib);
let red = new_style(1.3, rubric, nib);
let thin_red = new_style(1.0, rubric, pen);
let thin_ink = new_style(1.0, pale, pen);

let lang = new_fontset(["lang_upper"], ["lang_lower"]);
let caps = new_fontset(["copper_upper"], ["copper_lower"]);

// ---- ornaments ----

// A leaf, for the title page's border.
let leaf = new_path()
	.move_to(new_vec(0.0, -22.0))
	.quad_to(new_vec(14.0, -4.0), new_vec(0.0, 22.0))
	.quad_to(new_vec(-14.0, -4.0), new_vec(0.0, -22.0));
leaf.move_to(new_vec(0.0, -14.0)).line_to(new_vec(0.0, 26.0));
// A knot of two squares, for its corners.
let knot = new_rectangle(new_vec(0.0, 0.0), 40.0, 40.0);
knot.translate_by(new_vec(0.0, 0.0));
let corner = new_canvas();
corner.add_path(knot, thin_red);
corner.add_path(new_rectangle(new_vec(0.0, 0.0), 22.0, 22.0).rotate_around(new_vec(0.0, 0.0), PI() / 4.0), thin_red);
corner.add_path(new_circle(new_vec(0.0, 0.0), 4.0), thin_red);
// A wave, under the title.
let flourish = new_path().move_to(new_vec(-120.0, 0.0));
for i in 0..8 {
	let x = -120.0 + i * 30.0;
	flourish.cubic_to(new_vec(x + 10.0, -10.0), new_vec(x + 20.0, 10.0), new_vec(x + 30.0, 0.0));
}
// A diamond and a dot, between columns.
let diamond = new_path().move_to(new_vec(0.0, -10.0)).line_to(new_vec(6.0, 0.0)).line_to(new_vec(0.0, 10.0)).line_to(new_vec(-6.0, 0.0)).close();
diamond.move_to(new_vec(0.0, 14.0)).line_to(new_vec(0.0, 16.0));

// Art made to measure: a bramble filling the box it's given.
fn bramble_in(b) {
	let plant = art::bramble(new_vec(b.center.x, b.y + b.height), -PI() / 2.0, b.height * 0.95, b.height * 0.2, 5);
	let pen = new_brush("Ink Pen", 0.3, 0.7, 2.0);
	let on_paper = background() != ();
	let c = new_canvas();
	c.add_path(plant.vines, new_style(1.2, if on_paper { "#6b4423" } else { "#c8a27a" }, pen));
	c.add_path(plant.leaves, new_style(1.2, if on_paper { "#3d6b2a" } else { "#a8d68a" }, pen));
	c.add_path(plant.flowers, new_style(1.2, if on_paper { "#2b1d12" } else { "#f2ead8" }, pen));
	c
}

// ---- the title page ----

journal_entry.add_pages(layout("missal_title").typeset(#{
	name: "title",
	flows: #{
		title: "The Jagged Missal",
		subtitle: "second edition",
		credits: "written by the red sorceress\nSylvia Dread\n\nedited by loremaster\nCraden of Kindelmyre",
	},
	roles: #{
		title: #{ font: caps, style: red, size: 150.0, line_height: 1.0 },
		subtitle: #{ font: lang, style: ink, size: 50.0, spacing: 0.12 },
		credits: #{ font: lang, style: ink, size: 40.0, spacing: 0.12, paragraph_gap: 0.3 },
		frame: #{ style: thin_red },
		border: #{ style: thin_ink },
		flourish: #{ style: thin_red },
	},
	art: #{ border: leaf, corner: corner, flourish: flourish },
}));

// ---- an article over two-column spreads ----

journal_entry.add_pages(layout("missal_spread").typeset(#{
	name: "fateful_pages",
	flows: #{
		title: "Fateful Pages",
		body: "In my quiet moments of contemplation I consider often the Gospel of Saint Decimus While it is true that a great number of those passages speak directly to the soul of me this Market as many others leave in me a resonance with one phrase in particular Fate is not subtle A letter was delivered to me this market not by the hand of mortal mail carrier but by the way it lay slightly crumpled and tucked to the side of the tavern When I saw the name upon it I knew then the message for what it was a writ to me from lines predescribed upon the stars in the sky as much as one from a Quiet compatriot to you Fate is not subtle I knew from the moment I picked up the slightly ripped paper that it was a thing to be returned and not kept When later I picked up the token which read of a prize to be obtained aft speaking to you I knew the time to do so was imminent and went to the tavern to wait your inevitable morning arrival It was there with your letter and your token near at hand that I was contemplating when and how best to approach you with what was yours that once again Fate was characteristically unsubtle a stranger approached me and handed me a letter informing me that she had found it pinned to the tavern front it was one of my own so artfully borrowed by you and with timely answer of my own prayers I knew to likewise return to you yours and thereby receive my prize from you And what a prize it was It is a conversation I cannot pull from my mind delicate porcelain words too fragile to be spoken but too urgent to be set wholly aside playing in the corners of mirthful lips and eyes glowing with a light too brilliant and wild to wholly disguise\n\nBlind little spider come out to play the venom as you say remains and silks untwine in patient well familiar shapes Path by path we build out our ways and methods by which we predate to feast or play on prey in these meager efforts to persist from day to day Neither patience nor venom does danger make but rather the hungers that we wish to slake that we might bring an end to our souls ceaseless ache Ochre stained my master ate from plate and bade that I must likewise sate In obedience I mused scooped on my spoon a helping of that putrid offal sat in stew of congealed and rancid fat In that moment then I thought I knew the lesson meant for me and devoured senselessly indulging in its phantom taste welcome meals in heat wrought wastes You say you feel mad for the thoughts you hold To feel madness in the presence of purpose is not a flaw It is a sign that you are listening Enlightenment came to me a Door impossible and Black on some strange ocean shore It bore no knob nor way to knock but set upon its surface a strange shaped lock whose existence never can be met with strange matched key but here listen long and you will find the wretched blasphemy that haunts behind the truth that in dreams most Lazarines unknowingly strive to know and find Listen in my spider dear and you will learn we play a game each therein our piece is placed then our peace displaced and in this we have no say I seek not to convert for this is not so direct my way but I warn you this should you see this Door so Black turn away do not look back Eschew in thought black ill lumen gate instead dream of better days where Lazarolths truth lay before us skewered ripped apart and burned away Fill your ache but refuse to feed on feasts that Silent Queen brings from black and unrepentant dreams",
		caption: "the bramble",
	},
	roles: #{
		body: #{ font: lang, style: ink, size: 34.0, line_height: 1.05, spacing: 0.12, align: "justify", indent: 1.5 },
		heading: #{ font: caps, style: red, size: 90.0 },
		caption: #{ font: caps, style: red, size: 46.0 },
		initial: #{ font: caps, style: red },
		folio: #{ font: caps, style: red, size: 30.0 },
		rule: #{ style: thin_red },
		divider: #{ style: thin_red },
	},
	art: #{ divider: diamond, illustration: Fn("bramble_in") },
}));

// ---- a full-page figure ----

journal_entry.add_pages(layout("missal_figure").typeset(#{
	name: "plate",
	flows: #{ caption: "the serpent of many heads" },
	roles: #{ caption: #{ font: caps, style: red, size: 50.0 }, figure: #{ style: thin_ink } },
	art: #{ figure: art::snake_along(new_maze(11, 4, 3).path().scale_by(120.0), 60.0, 3) },
}));
