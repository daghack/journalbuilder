// symbols_to_digits: each symbol written as digits in another base, the
// biggest first, width digits to a symbol: in bases 27 and 3, H (8) is
// 0 2 2, and in bases 27 and 8, 1 0: what moon_phases, gazing_eyes and
// tentacles draw a text's letters with. width 0 is as few digits as hold
// every symbol of input_base. Unlocking reads the digits back, width at a
// time.

fn options() {
	#{ input_base: integer(), output_base: integer(), width: integer(0) }
}

fn check(o) {
	for base in [o.input_base, o.output_base] {
		if base < 2 || base > 27 {
			throw `the bases must be from 2 to 27, got ${base}`;
		}
	}
	if o.width < 0 {
		throw `width must be 0 (as few digits as it takes) or more, got ${o.width}`;
	}
	if o.width > 0 && o.width < width_of(o) {
		throw `width ${o.width} is too few base-${o.output_base} digits for a base-${o.input_base} symbol: it takes ${width_of(o)}`;
	}
}

fn inputs(o) {
	#{ symbols: symbol_type(o.input_base) }
}

fn outputs(o) {
	#{ digits: symbol_type(o.output_base) }
}

// How many digits a symbol takes: as few as hold every symbol.
fn width_of(o) {
	let w = 1;
	let holds = o.output_base;
	while holds < o.input_base {
		holds *= o.output_base;
		w += 1;
	}
	w
}

fn lock(o, i) {
	let w = if o.width > 0 { o.width } else { width_of(o) };
	let digits = [];
	for s in i.symbols {
		let these = [];
		let n = s;
		for k in 0..w {
			these.insert(0, n % o.output_base);
			n /= o.output_base;
		}
		digits += these;
	}
	#{ digits: digits }
}

fn unlock(o, out) {
	let w = if o.width > 0 { o.width } else { width_of(o) };
	if out.digits.len() % w != 0 {
		throw `${out.digits.len()} digits aren't a whole number of symbols, ${w} digits each`;
	}
	let symbols = [];
	let n = 0;
	for (d, k) in out.digits {
		n = n * o.output_base + d;
		if k % w == w - 1 {
			if n >= o.input_base {
				throw `the digits at ${k - w + 1} write ${n}, which isn't a base-${o.input_base} symbol`;
			}
			symbols.push(n);
			n = 0;
		}
	}
	#{ symbols: symbols }
}

fn examples() {
	[
		#{
			options: #{ input_base: 27, output_base: 3 },
			inputs: #{ symbols: symbols(27, "HI Z") },
			outputs: #{ digits: [0, 2, 2, 1, 0, 0, 0, 0, 0, 2, 2, 2] },
		},
		#{
			options: #{ input_base: 27, output_base: 8 },
			inputs: #{ symbols: symbols(27, "MOON") },
			outputs: #{ digits: [1, 5, 1, 7, 1, 7, 1, 6] },
		},
		#{
			// A width wider than it takes: leading zeros.
			options: #{ input_base: 10, output_base: 9, width: 3 },
			inputs: #{ symbols: symbols(10, "907") },
			outputs: #{ digits: [0, 1, 0, 0, 0, 0, 0, 0, 7] },
		},
		#{
			options: #{ input_base: 5, output_base: 27 },
			inputs: #{ symbols: [] },
			outputs: #{ digits: [] },
		},
		#{
			options: #{ input_base: 27, output_base: 3, width: 2 },
			error: "width 2 is too few base-3 digits for a base-27 symbol: it takes 3",
		},
		#{
			options: #{ input_base: 27, output_base: 1 },
			error: "the bases must be from 2 to 27, got 1",
		},
	]
}
