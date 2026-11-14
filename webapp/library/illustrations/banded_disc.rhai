// banded_disc: a disc of rings within rings, as the old diagrams of the
// heavens draw them: ruled rings, rings of ticks and of solid ink, writing
// running round, a ring of medallions each holding a mark, many-pointed
// stars, an eye at the heart. bands lists them from the outside in, each a
// map whose kind's key gives where it's drawn, as a share of the disc's
// radius (1 is the edge):
//   #{ rule: r }               a ruled circle; with double: true, a finer
//                              one gap (0.026) inside it
//   #{ ticks: [inner, outer] } count (364) ticks round, every every-th
//                              (28; 0 for none) reaching out to long
//   #{ solid: [inner, outer] } a ring of solid ink
//   #{ text: r }               the inscription, in as many whole words,
//                              over and over, as go round at size (0.08)
//   #{ marks: r }              a ring of medallions size (0.2) across, a
//                              mark in each: the symbols of symbol_set, or
//                              with none, count (the whole set) of its own
//                              in order, clockwise from the top
//   #{ star: r }               a star polygon of points (13), joining every
//                              step-th (5)
//   #{ eye: r }                an eye at the heart, r across, looking
//                              toward look (a quarter turn: down)
// Any band takes red: true, to be drawn in blood (but the inscription,
// in text_style's ink), weight (1), to make its pen heavier or lighter,
// and turn, to start that share of a turn round.
//
// The canvas is size across and down, the disc's centre in its middle.
// The inscription is written in text_style (unwired, the eldritch script
// in rough ink), at the band's size whatever the style's. With editor,
// each mark is committed to the visual editor as EDITOR_BAND_K, BAND the
// band's place in the list.

import "lib/inks" as inks;
import "lib/plates" as plates;

fn options() {
	#{
		size: 600.0, seed: 0, editor: "", ink: "#2b1d12", blood: "#9b2318",
		bands: [
			#{ rule: 0.97, double: true },
			#{ ticks: [0.905, 0.944], long: 0.956 },
			#{ solid: [0.882, 0.905] },
			#{ text: 0.82 },
			#{ rule: 0.756, double: true },
			#{ marks: 0.593, size: 0.215 },
			#{ rule: 0.46 },
			#{ star: 0.437 },
			#{ star: 0.437, step: 3, red: true, weight: 0.7 },
			#{ star: 0.437, step: 2, weight: 0.55 },
			#{ rule: 0.207, double: true },
			#{ eye: 0.31 },
		],
	}
}

fn inputs(o) {
	#{ symbols: "Any", symbol_set: "SymbolSet", inscription: "String", text_style: "TextStyle" }
}

fn fallbacks(o) {
	#{
		symbols: [],
		symbol_set: symbol_set("priests"),
		inscription: "",
		text_style: #{
			style: new_style(new_brush_style(o.ink, "Rough Ink", o.size * 0.002, #{
				min_pressure: 0.5, max_pressure: 0.8, pressure_profile: "taper", pressure_wander: 0.2, color_jitter: 0.04,
			}), new_handwriting_style(#{ glyph_rotation_jitter: 0.03, glyph_scale_jitter: 0.04 })),
			type: new_type_style(glyphset("eldritch"), o.size * 0.04, #{ glyph_tracking: 0.04 }),
		},
	}
}

fn example() {
	#{ inscription: "every two of them once never none never twice" }
}

// The kind of band `b` (the n-th) is, checking it names one and takes
// only what that kind takes.
fn kind_of(b, n) {
	let all = ["rule", "ticks", "solid", "text", "marks", "star", "eye"];
	let settings = ["double", "gap", "count", "every", "long", "size", "points", "step", "look", "red", "weight", "turn"];
	let kinds = [];
	for k in all { if k in b { kinds.push(k); } }
	if kinds.len() != 1 { throw `banded_disc: band ${n} should name one kind (${all}), got ${b.keys()}`; }
	for key in b.keys() {
		if !(key in all) && !(key in settings) { throw `banded_disc: band ${n} has no setting '${key}'; bands take ${settings}`; }
	}
	kinds[0]
}

fn render(o, i) {
	let r = o.size / 2.0;
	let centre = vec2(r, r);
	// Pens are as heavy as on a disc 540 across.
	let u = r / 270.0;
	let c = new_canvas();
	let n = 0;
	for b in o.bands {
		let kind = kind_of(b, n);
		let ink = if b.red ?? false { o.blood } else { o.ink };
		let weight = b.weight ?? 1.0;
		let turn = b.turn ?? 0.0;
		let at = b[kind];
		if kind == "rule" {
			if b.double ?? false {
				plates::double_ring(c, centre, at * r, (b.gap ?? 0.026) * r, ink, 2.0 * u * weight);
			} else {
				c.add(new_circle(centre, at * r), inks::contour(ink, 2.0 * u * weight));
			}
		} else if kind == "ticks" {
			let long = (b.long ?? at[1]) * r;
			let ticks = plates::ticks(centre, b.count ?? 364, at[0] * r, at[1] * r, b.every ?? 28, long);
			c.add(ticks.rotate_around(centre, 2.0 * PI() * turn), inks::fine(ink, 0.8 * u * weight));
		} else if kind == "solid" {
			let gap = max(3.0 * u, 1.5);
			let ring = new_path().append(new_circle(centre, at[1] * r)).append(new_circle(centre, at[0] * r));
			c.add(inks::solid(ring, gap), inks::fill(ink, gap));
		} else if kind == "text" {
			if i.inscription.trim() != "" {
				let hand = i.text_style;
				let t = hand.type;
				t.glyph_scale = (b.size ?? 0.08) * r;
				hand.type = t;
				let path = new_circle(centre, at * r).rotate_around(centre, 2.0 * PI() * turn - PI() / 2.0);
				c.add(path.fit_text(plates::inscription(path, i.inscription, hand), hand));
			}
		} else if kind == "marks" {
			let set = i.symbol_set;
			let symbols = i.symbols;
			if symbols.is_empty() {
				let count = b.count ?? set.base;
				for k in 0..count { symbols.push(k % set.base); }
			}
			let medal = (b.size ?? 0.2) * r / 2.0;
			for k in 0..symbols.len() {
				let p = centre + plates::toward(k * 1.0 / symbols.len() + turn) * at * r;
				c.add(new_circle(p, medal), inks::band(ink, 2.4 * u * weight));
				c.add(new_circle(p, medal * 0.83), inks::fine(ink, 0.7 * u * weight));
				let m = write_at(set.text([symbols[k]]), p, #{ type: #{ glyph_scale: medal * 1.31 } });
				if o.editor != "" { m = m.commit_to_editor(`${o.editor}_${n}_${k}`); }
				c.add(m);
			}
		} else if kind == "star" {
			let star = new_polygon(centre, at * r, b.points ?? 13, #{ step: b.step ?? 5 });
			c.add(star.rotate_around(centre, 2.0 * PI() * turn), inks::contour(ink, 1.3 * u * weight));
		} else if kind == "eye" {
			let look = b.look ?? (PI() / 2.0);
			let eye = art::eye(centre, at * r, #{ gaze_angle: look, gaze: 0.85, angle: 2.0 * PI() * turn, openness: 0.4, messiness: 0.45, seed: o.seed });
			c.add(inks::eye(eye, ink, o.blood, at * r * 0.0105 * weight));
		}
		n += 1;
	}
	c
}
