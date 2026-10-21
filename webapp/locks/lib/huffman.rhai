// Reading and writing text with a Huffman code given as its tree, a
// HuffmanCode value (huffman::code builds one).

// Throws unless the code rewrites o.input_base in o.output_base.
fn check(code, o) {
	if code.input_base != o.input_base || code.output_base != o.output_base {
		throw `the code is base ${code.input_base} to base ${code.output_base}, but the lock is base ${o.input_base} to base ${o.output_base}`;
	}
}

// Each input symbol's code, an array of output digits, or () for a symbol
// with no leaf. A node's digits are its parent's and its own.
fn codes(code) {
	let digits = [];
	digits.pad(code.input_base, ());
	let paths = [];
	for node in code.nodes {
		let path = [];
		if node.parent != () {
			path = paths[node.parent];
			path.push(node.digit);
		}
		if node.symbol != () {
			digits[node.symbol] = path;
		}
		paths.push(path);
	}
	digits
}

// The symbols' codes, one after another.
fn encode(code, symbols) {
	let digits = codes(code);
	let out = [];
	for s in symbols {
		let d = digits[s];
		if d == () {
			throw `symbol ${s} has no code`;
		}
		out += d;
	}
	out
}

// The digits read back down the tree: from the root, each digit takes the
// child it names, and a leaf gives its symbol and starts again.
fn decode(code, digits) {
	let children = [];
	for node in code.nodes {
		children.push(#{});
	}
	for (node, i) in code.nodes {
		if node.parent != () {
			children[node.parent][`${node.digit}`] = i;
		}
	}
	let symbols = [];
	let at = 0;
	for d in digits {
		let next = children[at][`${d}`];
		if next == () {
			throw `the cipher text has a ${d} where the code has no such branch`;
		}
		at = next;
		let symbol = code.nodes[at].symbol;
		if symbol != () {
			symbols.push(symbol);
			at = 0;
		}
	}
	if at != 0 {
		throw "the cipher text ends partway through a code";
	}
	symbols
}
