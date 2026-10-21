// Names for stars and constellations, from syllables rather than after any
// language: a start, a vowel, another syllable or two, sometimes closed.
// Each takes a generator from new_random(seed), so a seed always gives the
// same names.

fn onsets() { ["b", "d", "g", "k", "l", "m", "n", "r", "s", "t", "v", "th", "sh", "dr", "tr", "br", "gr", "kh", "z", "ph", "mt", "lv"] }
fn vowels() { ["a", "e", "i", "o", "u", "a", "e", "i", "ia", "ae", "ei", "ou", "y"] }
fn codas() { ["", "", "", "l", "n", "r", "s", "th", "gal", "ri", "ra", "el", "iel", "ar"] }
fn initials() { ["A", "B", "D", "E", "I", "K", "L", "M", "N", "O", "R", "S", "T", "V"] }

// A vowel, then a syllable or two, sometimes closed.
fn tail(rng) {
	let s = rng.pick(vowels());
	let more = if rng.next() < 0.5 { 1 } else { 2 };
	for t in 0..more {
		s += rng.pick(onsets()) + rng.pick(vowels());
	}
	s + rng.pick(codas())
}

// A name starting with `letter`, at most nine letters, not in `used`.
fn name(rng, letter, used) {
	loop {
		let l = letter.to_lower();
		let body = if "aeiouy".contains(l) { l + rng.pick(onsets()) + tail(rng) } else { l + tail(rng) };
		let name = body.sub_string(0, 1).to_upper() + body.sub_string(1);
		if name.len() <= 9 && !(name in used) {
			return name;
		}
	}
}

// A name for each of a code's nodes, and `count` constellations', all
// different: the root is Shiathri, a leaf's name starts with its symbol's
// letter (the space's, in base 27, is Vacii), and the rest start anyhow.
fn sky(code, rng, count) {
	let used = #{ Shiathri: true, Vacii: true };
	let stars = [];
	for node in code.nodes {
		let n = if node.parent == () {
			"Shiathri"
		} else if node.symbol == () {
			name(rng, rng.pick(initials()), used)
		} else if code.input_base == 27 && node.symbol == 0 {
			"Vacii"
		} else {
			name(rng, symbol_text(code.input_base, [node.symbol]), used)
		};
		used[n] = true;
		stars.push(n);
	}
	let constellations = [];
	for c in 0..count {
		let n = name(rng, rng.pick(initials()), used);
		used[n] = true;
		constellations.push(n);
	}
	#{ stars: stars, constellations: constellations }
}

// The symbol a leaf's name stands for: its first letter's, or the space's
// for Vacii in base 27.
fn symbol_of(base, name) {
	if base == 27 && name == "Vacii" {
		return 0;
	}
	symbols(base, name.sub_string(0, 1).to_upper())[0]
}
