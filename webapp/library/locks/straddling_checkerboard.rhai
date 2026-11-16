// straddling_checkerboard: symbols as digits from a board of ten columns,
// 0 to 9. Its top row holds eight symbols, leaving two columns blank, and
// a symbol there is its column's digit alone; below it are two rows of
// ten, named by the blank columns' digits, and a symbol there is that
// digit then its column's. The board is filled in key order: the key's
// symbols, each the first time it comes, then the rest of the base's in
// order. So common symbols put first come out a digit long, and a reader
// can tell where each ends: a blank column's digit always has another
// after it.
//
// With the key ESTONIAR (base 26) and the blanks 2 and 6,
//
//        0 1 2 3 4 5 6 7 8 9
//        E S   T O N   I A R
//     2  B C D F G H J K L M
//     6  P Q U V W X Y Z
//
// ATTACK is 8 3 3 8 21 27. The board holds 28, enough for base 27 or less.

fn options() {
	#{ base: integer(27), first_blank: integer(2), second_blank: integer(6) }
}

fn check(o) {
	for b in [o.first_blank, o.second_blank] {
		if b < 0 || b > 9 {
			throw `the blank columns must be digits, 0 to 9, got ${b}`;
		}
	}
	if o.first_blank == o.second_blank {
		throw `the two blank columns must differ, both are ${o.first_blank}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base), key: symbol_type(o.base) }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(10), key: symbol_type(o.base) }
}

// The base's symbols in key order.
fn alphabet(key, base) {
	let order = [];
	let all = [];
	for s in 0..base { all.push(s); }
	for s in key + all {
		if !order.contains(s) { order.push(s); }
	}
	order
}

// The top row's columns: the digits but the blanks.
fn top(o) {
	let columns = [];
	for d in 0..10 {
		if d != o.first_blank && d != o.second_blank { columns.push(d); }
	}
	columns
}

fn lock(o, i) {
	let order = alphabet(i.key, o.base);
	let columns = top(o);
	let codes = [];
	codes.pad(o.base, []);
	for (s, n) in order {
		codes[s] = if n < 8 { [columns[n]] } else if n < 18 { [o.first_blank, n - 8] } else { [o.second_blank, n - 18] };
	}
	let out = [];
	for s in i.plain_text { out += codes[s]; }
	#{ cipher_text: out, key: i.key }
}

fn unlock(o, out) {
	let order = alphabet(out.key, o.base);
	let columns = top(o);
	let digits = out.cipher_text;
	let text = [];
	let k = 0;
	while k < digits.len() {
		let d = digits[k];
		let n = if d == o.first_blank || d == o.second_blank {
			if k + 1 >= digits.len() {
				throw `the digits end on ${d}, a row's digit with no column after it`;
			}
			k += 1;
			(if d == o.first_blank { 8 } else { 18 }) + digits[k]
		} else {
			columns.index_of(d)
		};
		if n >= order.len() {
			throw `row ${d} column ${digits[k]} is an empty square`;
		}
		text.push(order[n]);
		k += 1;
	}
	#{ plain_text: text }
}

fn examples() {
	[
		#{
			options: #{ base: 26 },
			inputs: #{ plain_text: symbols(26, "ATTACK"), key: symbols(26, "ESTONIAR") },
			outputs: #{ cipher_text: [8, 3, 3, 8, 2, 1, 2, 7] },
		},
		#{
			options: #{ base: 27, first_blank: 0, second_blank: 9 },
			inputs: #{ plain_text: symbols(27, "SIX FATHOMS AND FALLING"), key: symbols(27, "SOUNDING") },
		},
		#{
			// No key: the base's own order.
			options: #{ base: 27 },
			inputs: #{ plain_text: symbols(27, "ZERO"), key: [] },
		},
		#{
			options: #{ base: 27, first_blank: 4, second_blank: 4 },
			error: "must differ",
		},
	]
}
