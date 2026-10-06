// split_symbols: a text cut into output_count pieces, output_0 to
// output_<n-1>, each as long as the first; the last ones are shorter, or
// empty, when the length doesn't divide evenly. Unlocking joins them again.

fn options() {
	#{ base_n: integer(), output_count: integer() }
}

fn check(o) {
	// With no outputs, the lock would never run.
	if o.output_count < 1 {
		throw `output_count must be at least 1, got ${o.output_count}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base_n) }
}

fn outputs(o) {
	let outputs = #{};
	for n in 0..o.output_count {
		outputs[`output_${n}`] = symbol_type(o.base_n);
	}
	outputs
}

fn lock(o, i) {
	let text = i.plain_text;
	// The length divided by the count, rounded up.
	let size = (text.len() + o.output_count - 1) / o.output_count;
	let outputs = #{};
	for n in 0..o.output_count {
		let start = n * size;
		outputs[`output_${n}`] = if start < text.len() { text.extract(start, size) } else { [] };
	}
	outputs
}

fn unlock(o, out) {
	let text = [];
	for n in 0..o.output_count {
		text.append(out[`output_${n}`]);
	}
	#{ plain_text: text }
}

fn examples() {
	[
		#{
			options: #{ base_n: 27, output_count: 3 },
			inputs: #{ plain_text: symbols(27, "HELLO WORLD") },
			outputs: #{ output_0: symbols(27, "HELL"), output_1: symbols(27, "O WO"), output_2: symbols(27, "RLD") },
		},
		#{
			options: #{ base_n: 27, output_count: 4 },
			inputs: #{ plain_text: symbols(27, "ABC") },
			outputs: #{ output_0: symbols(27, "A"), output_1: symbols(27, "B"), output_2: symbols(27, "C"), output_3: [] },
		},
		#{
			options: #{ base_n: 10, output_count: 2 },
			inputs: #{ plain_text: [] },
		},
		#{
			options: #{ base_n: 27, output_count: 0 },
			error: "output_count must be at least 1, got 0",
		},
	]
}
