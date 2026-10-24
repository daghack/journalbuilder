// huffman: a text in input_base rewritten in output_base by a Huffman code
// built from a sample of input_base text, the corpus. Symbols the corpus
// uses often get short codes, rare ones long, so text like the corpus comes
// out short. Any pair of bases works: base 27 to binary, base 27 to
// ternary, digits to letters.
//
// Every symbol counts `smoothing` times besides its uses in the corpus, so
// text with symbols the corpus never uses still encodes. With smoothing 0
// those symbols have no code, and locking a text with one fails.
//
// The corpus is passed through: unlocking rebuilds the same code from it.
// Ties are broken by symbol, then by the order nodes were made, so the
// same corpus always gives the same code.

fn options() {
	#{ input_base: integer(), output_base: integer(), smoothing: integer(1) }
}

fn check(o) {
	if o.smoothing < 0 {
		throw `smoothing can't be negative, got ${o.smoothing}`;
	}
}

fn inputs(o) {
	#{ corpus: symbol_type(o.input_base), plain_text: symbol_type(o.input_base) }
}

fn outputs(o) {
	#{ corpus: symbol_type(o.input_base), cipher_text: symbol_type(o.output_base) }
}

fn lock(o, i) {
	let codes = code_table(o, i.corpus);
	let cipher = [];
	for s in i.plain_text {
		let code = codes[s];
		if code == () {
			throw `symbol ${s} isn't in the corpus, and smoothing is 0, so it has no code`;
		}
		cipher.append(code);
	}
	#{ corpus: i.corpus, cipher_text: cipher }
}

fn unlock(o, out) {
	// Codes are prefix-free: read digits until they spell one.
	let decode = #{};
	for (code, s) in code_table(o, out.corpus) {
		if code != () { decode[key(code)] = s; }
	}
	let plain = [];
	let read = [];
	for d in out.cipher_text {
		read.push(d);
		let k = key(read);
		if k in decode {
			plain.push(decode[k]);
			read = [];
		}
	}
	if read.len() > 0 {
		throw `the cipher text ends partway through a code`;
	}
	#{ plain_text: plain }
}

// Each input symbol's code, an array of output digits, or () for a symbol
// with no weight.
fn code_table(o, corpus) {
	let counts = [];
	counts.pad(o.input_base, o.smoothing);
	for s in corpus { counts[s] += 1; }

	// Nodes are #{ weight, symbol, children }; leaves first, by symbol.
	let nodes = [];
	let queue = [];
	for s in 0..o.input_base {
		if counts[s] > 0 {
			queue.push([counts[s], nodes.len()]);
			nodes.push(#{ weight: counts[s], symbol: s, children: [] });
		}
	}
	if queue.len() == 0 {
		throw "the corpus is empty and smoothing is 0, so there's nothing to build a code from";
	}
	// An M-ary tree merges M nodes at a time, so pad with weightless
	// leaves until every merge is full; there are at least two leaves, so
	// even one symbol gets a digit.
	let m = o.output_base;
	while queue.len() < 2 || (queue.len() - 1) % (m - 1) != 0 {
		queue.push([0, nodes.len()]);
		nodes.push(#{ weight: 0, symbol: -1, children: [] });
	}
	while queue.len() > 1 {
		queue.sort(|a, b| if a[0] != b[0] { a[0] - b[0] } else { a[1] - b[1] });
		let children = queue.extract(0, m);
		queue = queue.extract(m);
		let weight = 0;
		let ids = [];
		for c in children {
			weight += c[0];
			ids.push(c[1]);
		}
		queue.push([weight, nodes.len()]);
		nodes.push(#{ weight: weight, symbol: -1, children: ids });
	}

	// Walk down from the root, child d adding digit d.
	let codes = [];
	codes.pad(o.input_base, ());
	let stack = [[queue[0][1], []]];
	while stack.len() > 0 {
		let top = stack.pop();
		let node = nodes[top[0]];
		if node.children.len() == 0 {
			if node.symbol >= 0 { codes[node.symbol] = top[1]; }
		} else {
			for (child, d) in node.children {
				let code = top[1];
				code.push(d);
				stack.push([child, code]);
			}
		}
	}
	codes
}

// A code as a map key.
fn key(code) {
	code.reduce(|k, d| k + d + ".", "")
}

fn examples() {
	[
		#{
			// Base 27 to binary: E and space, the corpus's commonest, get
			// the shortest codes.
			options: #{ input_base: 27, output_base: 2 },
			inputs: #{ corpus: symbols(27, "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG"), plain_text: symbols(27, "EVERY LOCK OPENS") },
		},
		#{
			// Base 27 to ternary.
			options: #{ input_base: 27, output_base: 3 },
			inputs: #{ corpus: symbols(27, "SEE THE SEA"), plain_text: symbols(27, "HELLO WORLD") },
		},
		#{
			// Down to a bigger base: digits to letters.
			options: #{ input_base: 10, output_base: 26 },
			inputs: #{ corpus: symbols(10, "31415926535"), plain_text: symbols(10, "2718281828") },
		},
		#{
			// One symbol in the corpus and no smoothing: it gets one digit.
			options: #{ input_base: 10, output_base: 2, smoothing: 0 },
			inputs: #{ corpus: symbols(10, "7"), plain_text: symbols(10, "777") },
			outputs: #{ corpus: symbols(10, "7"), cipher_text: [1, 1, 1] },
		},
		#{
			// A, B and C equally common, base 3: one digit each.
			options: #{ input_base: 3, output_base: 3, smoothing: 0 },
			inputs: #{ corpus: [0, 1, 2], plain_text: [2, 0, 1] },
			outputs: #{ corpus: [0, 1, 2], cipher_text: [2, 0, 1] },
		},
		#{
			options: #{ input_base: 27, output_base: 2, smoothing: 0 },
			inputs: #{ corpus: symbols(27, "ABC"), plain_text: symbols(27, "ABD") },
			error: "isn't in the corpus",
		},
		#{
			options: #{ input_base: 27, output_base: 2, smoothing: -1 },
			error: "smoothing can't be negative",
		},
	]
}
