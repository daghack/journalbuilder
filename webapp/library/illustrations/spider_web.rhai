// spider_web: symbols caught in a spider's web, one in each gap of it:
// the web's thread winds out from the hub in a spiral, clockwise from the
// spoke nearest the top, crossing spokes spokes each turn, and the
// symbols follow it, the first in the gap just clockwise of that spoke on
// the first turn, the last on the outermost that has any. Read along a
// spoke instead, the text is scrambled: every spokes-th symbol.
//
// The web is an old one, and nothing in it is even: its hub is off the
// middle, its spokes are at odd angles and run to the canvas's edges, its
// turns are nearer and farther apart, each thread sags its own way (sag
// of a turn's width toward the hub, about), and outside the turns that
// hold the symbols the thread is torn, lengths of it missing or hanging
// loose. ragged is how uneven it all is, from 0, a web spun true, to 1;
// seed picks how. Nothing waits at the hub.
//
// The canvas is size across and down. thread_style, a TextStyle, a style
// source's, draws the web. The symbols are symbol_set's, a SymbolSet, each
// at glyph_size (the set's own size if that's 0) or smaller in a gap that
// hasn't room, turned to the web (its foot to the hub) unless turned is
// false; a blank symbol leaves its gap empty. Unwired, a near-black ink
// pen and the fine letters set in red. With editor, each symbol is committed
// to the visual editor as EDITOR_N, N its place along the thread.

import "lib/symbol_rings" as rings;
import "lib/chance" as chance;

fn options() {
	#{
		base: 27, spokes: 9, size: 900.0, sag: 0.25, ragged: 1.0, seed: 0, turned: true, glyph_size: 40.0, editor: "",
		thread_ink: "#2b1d12", glyph_ink: "#9b2318",
	}
}

fn inputs(o) {
	#{ message: `Base${o.base}Symbol`, thread_style: "TextStyle", symbol_set: "SymbolSet" }
}

fn fallbacks(o) {
	let hand = new_handwriting_style();
	let steady = #{ min_pressure: 0.4, max_pressure: 0.7, pressure_profile: "constant" };
	#{
		thread_style: #{ style: new_style(new_brush_style(o.thread_ink, "Ink Pen", 0.9, steady), hand) },
		symbol_set: symbol_set("fine_letters", #{ color: o.glyph_ink }),
	}
}

fn render(o, i) {
	rings::check_base(o.base);
	if o.spokes < 5 || o.spokes > 32 { throw `spokes should be from 5 to 32, got ${o.spokes}`; }
	if o.sag < 0.0 || o.sag > 0.5 { throw `sag should be from 0 to 0.5, got ${o.sag}`; }
	if o.ragged < 0.0 || o.ragged > 1.0 { throw `ragged should be from 0 to 1, got ${o.ragged}`; }
	let n = o.spokes;
	let count = i.message.len();
	let centre = vec2(o.size / 2.0, o.size / 2.0);
	let hub_at = centre + vec2(chance::either_way(o.seed, 0, 10), chance::either_way(o.seed, 0, 11)) * o.size * 0.09 * o.ragged;
	let hub = o.size * 0.135;
	// Each spoke: which way it runs, how far to the canvas's edge, where
	// the web's frame crosses it, and where its first turn does.
	let ways = [];
	let ends = [];
	let frames = [];
	let firsts = [];
	for k in 0..n {
		let a = -PI() / 2.0 + 2.0 * PI() * (k + chance::either_way(o.seed, k, 12) * 0.28 * o.ragged) / n;
		let way = vec2(a.cos(), a.sin());
		let end = to_edge(hub_at, way, o.size * 0.015, o.size * 0.985);
		ways.push(way);
		ends.push(end);
		frames.push(end * (0.92 - 0.12 * chance::of(o.seed, k, 13) * o.ragged));
		firsts.push(hub * (1.0 + 0.3 * chance::either_way(o.seed, k, 14) * o.ragged));
	}
	// Turns of gaps, the thread one turn more to close the last, and one
	// beyond that to tear.
	let turns = max(1, (count + n - 1) / n);
	let whole = turns + 1;
	let torn = if o.ragged > 0.0 { 1 } else { 0 };
	// Where the thread crosses its m-th spoke, counting on round the web:
	// a little before or after its turn.
	let crossing = |m| {
		let k = m % n;
		let turn = m * 1.0 / n + (if m < n { 0.0 } else { chance::either_way(o.seed, m, 15) * 0.2 * o.ragged });
		hub_at + ways[k] * (firsts[k] + (frames[k] - firsts[k]) * turn / (whole + torn + 0.3))
	};
	let web = new_path();
	for k in 0..n {
		web.append(new_line(hub_at + ways[k] * hub * 0.2, hub_at + ways[k] * ends[k]));
	}
	// How far each length of the thread hangs toward the hub.
	let hangs = |m| o.sag * (crossing.call(m + n) - crossing.call(m)).length() * (1.0 + chance::either_way(o.seed, m, 16) * 0.7 * o.ragged);
	for m in 0..(whole + torn) * n {
		let from = crossing.call(m);
		let to = crossing.call(m + 1);
		if m >= count + n {
			// Past the symbols' gaps: torn away, or hanging from one end.
			let fate = chance::of(o.seed, m, 17);
			if fate < 0.3 * o.ragged { continue; }
			if fate < 0.5 * o.ragged {
				let part = 0.25 + 0.3 * chance::of(o.seed, m, 18);
				let loose = from + (to - from) * part + vec2(0, (to - from).length() * part * 0.7);
				web.append(new_path().move_to(from).quad_to(from + (to - from) * part, loose));
				continue;
			}
		}
		let length = new_path().move_to(from);
		hang(length, from, to, hub_at, hangs.call(m));
		web.append(length);
	}
	// The frame, from spoke to spoke, and a ragged ring or two at the hub.
	for ring in [0, 1, 2] {
		for k in 0..n {
			let next = (k + 1) % n;
			let at = |j| if ring == 0 { frames[j] } else { firsts[j] * (0.75 - 0.3 * ring + 0.2 * chance::of(o.seed, j, 19 + ring) * o.ragged) + hub * 0.2 };
			if ring == 2 && chance::of(o.seed, k, 22) < 0.35 * o.ragged { continue; }
			let from = hub_at + ways[k] * at.call(k);
			let to = hub_at + ways[next] * at.call(next);
			let length = new_path().move_to(from);
			hang(length, from, to, hub_at, if ring == 0 { (to - from).length() * 0.07 } else { 0.0 });
			web.append(length);
		}
	}
	let c = new_canvas();
	c.add(web, i.thread_style.style);
	// The symbols, each in the middle of its gap, as big as the gap holds.
	let set = i.symbol_set;
	if set.base < o.base { throw `spider_web: the symbol set '${set.name}' has ${set.base} symbols, fewer than the base, ${o.base}`; }
	let top = if o.glyph_size > 0.0 { o.glyph_size } else { set.size };
	for (s, m) in i.message {
		let corners = [crossing.call(m), crossing.call(m + 1), crossing.call(m + n), crossing.call(m + n + 1)];
		let middle = (corners[0] + corners[1] + corners[2] + corners[3]) / 4.0;
		let inward = (hub_at - middle).normalized();
		let high = min((corners[2] - corners[0]).length(), (corners[3] - corners[1]).length());
		let wide = (corners[1] - corners[0]).length();
		let at = middle + inward * (hangs.call(m) + hangs.call(m + n)) * 0.4;
		let turn = if o.turned { (-inward).angle() + PI() / 2.0 } else { 0.0 };
		let mark = write_at(set.text([s]), at, #{ type: #{ glyph_scale: min(top, min(high, wide * 0.9) * 0.68) }, turn: turn });
		if o.editor != "" { mark = mark.commit_to_editor(`${o.editor}_${m}`); }
		c.add(mark);
	}
	c
}

fn example() {
	#{ message: symbols(27, "WHAT THE SPIDER SPINS SHE SPINS FOR YOU").symbols() }
}

// How far from `from` along `way` the square from low to high, across
// and down, ends.
fn to_edge(from, way, low, high) {
	let far = 1.0e9;
	if way.x > 1.0e-6 { far = min(far, (high - from.x) / way.x); }
	if way.x < -1.0e-6 { far = min(far, (low - from.x) / way.x); }
	if way.y > 1.0e-6 { far = min(far, (high - from.y) / way.y); }
	if way.y < -1.0e-6 { far = min(far, (low - from.y) / way.y); }
	far
}

// A thread from `from` to `to` added to path (which is at `from`),
// hanging sag toward the hub in its middle.
fn hang(path, from, to, hub_at, sag) {
	let middle = (from + to) / 2.0;
	path.quad_to(middle + (hub_at - middle).normalized() * sag * 2.0, to);
}
