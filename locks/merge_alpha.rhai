// merge_alpha: input_count texts, input_0 to input_<n-1>, joined end to
// end. location_info keeps their lengths, so unlocking can split them
// apart again.

fn options() {
	#{ base_n: (), input_count: () }
}

fn check(o) {
	if type_of(o.input_count) != "i64" || o.input_count < 1 {
		throw `input_count must be a positive integer, got ${o.input_count}`;
	}
}

fn inputs(o) {
	let inputs = #{};
	for n in 0..o.input_count {
		inputs[`input_${n}`] = symbol_type(o.base_n);
	}
	inputs
}

fn outputs(o) {
	#{ merged_output: symbol_type(o.base_n), location_info: "LockProcessInfo" }
}

fn lock(o, i) {
	let merged = [];
	let lengths = [];
	for n in 0..o.input_count {
		let text = i[`input_${n}`];
		lengths.push(text.len());
		merged.append(text);
	}
	#{ merged_output: merged, location_info: lengths }
}

fn unlock(o, out) {
	let merged = out.merged_output;
	let lengths = out.location_info;
	check_lengths(lengths, o.input_count, merged.len());
	let inputs = #{};
	let start = 0;
	for n in 0..o.input_count {
		inputs[`input_${n}`] = merged.extract(start, lengths[n]);
		start += lengths[n];
	}
	inputs
}

// Throws unless `lengths` is `count` lengths adding up to `total`.
fn check_lengths(lengths, count, total) {
	if type_of(lengths) != "array" || lengths.len() != count {
		throw `location_info should be the ${count} input lengths, got ${lengths}`;
	}
	let sum = 0;
	for length in lengths {
		if type_of(length) != "i64" || length < 0 {
			throw `location_info should be the ${count} input lengths, got ${lengths}`;
		}
		sum += length;
	}
	if sum != total {
		throw `location_info's lengths add up to ${sum}, but the merged text has ${total} symbols`;
	}
}

fn examples() {
	[
		#{
			options: #{ base_n: 27, input_count: 3 },
			inputs: #{ input_0: symbols(27, "AB"), input_1: [], input_2: symbols(27, "CDE") },
			outputs: #{ merged_output: symbols(27, "ABCDE"), location_info: [2, 0, 3] },
		},
		#{
			options: #{ base_n: 10, input_count: 1 },
			inputs: #{ input_0: symbols(10, "1084") },
		},
		#{
			options: #{ base_n: 27, input_count: 0 },
			error: "input_count must be a positive integer, got 0",
		},
	]
}
