// What the almanac's pages share, so they look like one man's notebook:
// its leaves (lib/leaves' leaves, aged as his are: tea rings, foxing and
// the fold, never burnt), walled with his account in sepia, and the hands
// he writes in (an old script for the account, a cursive for his notes,
// plain figures for numbers). Everything else a page does with its leaf
// is lib/leaves' (place, clear, ribbon).
// import "lib/leaves" as leaves;
// import "lib/book" as book;
//
//   let leaf = book::leaf("wheel", 14);
//   leaves::place(leaf, figure.translate_by(at), 6);
//   book::jot(leaf, "Old to old.", vec2(660, 1150), 440, #{ ink: inks::blood() });
//   journal_entry.add(book::bind(leaf, content("watch")));

import "lib/inks" as inks;
import "lib/leaves" as leaves;

// The paler ink the walls of writing are in, so the figures stand out of
// them: iron gall gone brown.
fn sepia() { "#5b3e22" }

// ---- the leaf ----

// A leaf of the notebook to draw a page on (leaves::leaf), aged as his
// are: three tea stains, foxing and a fold, and never burnt. o as
// leaves::leaf takes it; his fold is "down" and his foxing 160 specks.
fn leaf(name, seed) { leaf(name, seed, #{}) }
fn leaf(name, seed, o) {
	leaves::leaf(name, seed, #{
		tea: [104, 66, 24], stain_size: [120, 80], stain_strength: [0.12, 0.07],
		fox: [120, 74, 30], foxing: 160, foxing_margin: 40, fold: "down", burn: 0,
	} + o)
}

// A note of his on the leaf in his cursive (leaves::jot, o as it takes it).
fn jot(leaf, text, at, width) { jot(leaf, text, at, width, #{}) }
fn jot(leaf, text, at, width, o) { leaves::jot(leaf, text, at, width, cursive(o)) }

// A note in his cursive (leaves::note, o as it takes it).
fn note(text, at, width) { note(text, at, width, #{}) }
fn note(text, at, width, o) { leaves::note(text, at, width, cursive(o)) }

// The leaf as a page, walled with his account (leaves::bind): o: size
// (15), the wall's glyphs; hand (wall_hand at that size); area (the leaf,
// 40 in from its edges).
fn bind(leaf, account) { bind(leaf, account, #{}) }
fn bind(leaf, account, o) {
	leaves::bind(leaf, account, #{
		hand: wall_hand(o.size ?? 15.0),
		area: new_rectangle(leaf.size / 2.0, leaf.size.x - 80, leaf.size.y - 80),
		density: 0.55,
	} + o)
}

// A note's options with his cursive as its hand, at its size and in its ink.
fn cursive(o) { #{ hand: note_hand(o.size ?? 30.0, o.ink ?? inks::black()) } + o }

// ---- hands ----

// The old hand the account is in: a glyph for a letter, in a pen that
// runs dry and is dipped again, so the wall of it is lighter and darker
// by turns.
fn old_hand(size) {
	#{
		style: new_style(new_brush_style(inks::black(), "Rough Ink", size * 0.035, #{
			min_pressure: 0.35, max_pressure: 0.66, pressure_profile: "taper", pressure_wander: 0.15,
			pool: 0.25, pool_depth: 0.4, color_jitter: 0.05, ink_load: 0.36, redip_every: 70,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.04, glyph_position_jitter: [size * 0.03, size * 0.05], glyph_scale_jitter: 0.05, baseline_wander: size * 0.08 })),
		type: new_type_style(glyphset("eldritch"), size, #{ glyph_tracking: 0.02 }),
		paragraph: new_paragraph_style(#{ line_height: 1.18, clearance: 5.0, line_start_jitter: size * 0.3, line_gap_jitter: size * 0.08 }),
	}
}

// The same script small and close, in sepia: the writing a page is
// walled with, round and between its figures.
fn wall_hand(size) {
	#{
		style: new_style(new_brush_style(sepia(), "Rough Ink", size * 0.04, #{
			min_pressure: 0.3, max_pressure: 0.62, pressure_profile: "taper", pressure_wander: 0.2,
			color_jitter: 0.08, ink_load: 0.45, redip_every: 90,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.05, glyph_position_jitter: [size * 0.04, size * 0.06], glyph_scale_jitter: 0.08, baseline_wander: size * 0.1 })),
		type: new_type_style(glyphset("eldritch"), size, #{ glyph_tracking: 0.03 }),
		paragraph: new_paragraph_style(#{ line_height: 1.1, clearance: size * 0.35, line_start_jitter: size * 0.2, line_gap_jitter: size * 0.06 }),
	}
}

// The same hand in another ink, a heavier pen: for what's written to be
// seen, on ribbons and rings.
fn loud_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Rough Ink", size * 0.05, #{
			min_pressure: 0.5, max_pressure: 0.8, pressure_profile: "taper", pressure_wander: 0.2, color_jitter: 0.04,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.03, glyph_scale_jitter: 0.04 })),
		type: new_type_style(glyphset("eldritch"), size, #{ glyph_tracking: 0.04 }),
		paragraph: new_paragraph_style(#{ line_height: 1.0 }),
	}
}

// The hand of his notes: a quick cursive, in `ink`, the pen pressed and
// lifted as a writer's is, the lines not quite level.
fn note_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Rough Ink", size * 0.032, #{
			min_pressure: 0.3, max_pressure: 0.75, pressure_profile: "taper", pressure_wander: 0.25, color_jitter: 0.05,
			ink_load: 0.6, redip_every: 120,
		}), new_handwriting_style(#{ glyph_rotation_jitter: 0.03, glyph_position_jitter: [size * 0.02, size * 0.03], glyph_scale_jitter: 0.04, baseline_wander: size * 0.1 })),
		type: new_type_style(glyphset("ayan"), size, #{ glyph_tracking: 0.0 }),
		paragraph: new_paragraph_style(#{ line_height: 1.3, line_start_jitter: size * 0.25, line_gap_jitter: size * 0.08 }),
	}
}

// Plain figures, for numbers that mustn't be misread: the plain glyphs
// (which have every sign the cursive lacks), in a steady pen whose stops
// don't taper away.
fn figures_hand(size, ink) {
	#{
		style: new_style(new_brush_style(ink, "Rough Ink", size * 0.036, #{ min_pressure: 0.55, max_pressure: 0.75, pressure_profile: "constant", color_jitter: 0.05 }),
			new_handwriting_style(#{ glyph_rotation_jitter: 0.04, glyph_position_jitter: [size * 0.028, size * 0.042], baseline_wander: size * 0.083 })),
		type: new_type_style(glyphset("plain"), size, #{ glyph_tracking: 0.05 }),
		paragraph: new_paragraph_style(#{ line_height: 1.3, line_start_jitter: size * 0.25, line_gap_jitter: size * 0.08 }),
	}
}
