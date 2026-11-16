// homophonic: each symbol given several numbers, its homophones, as many
// as it's common, so no number is commoner than another; each time a
// symbol is written one of its numbers is picked. The numbers are digits
// wide, 00 to 99 by default, written as their digits (base 10). With base
// 27 or 26 the numbers are shared out by how often each symbol comes in
// English (a space about one symbol in six, E the commonest letter, Q and
// Z the rarest); other bases share them evenly. Which numbers go to which
// symbol, and which of a symbol's is picked each time, the seed decides.
//
// table is the key, a Homophones: #{ base, digits, homophones }, each
// symbol's numbers in a list, smallest first, the lists in symbol order.

fn options() {
	#{ base: integer(27), digits: integer(2), seed: integer(0) }
}

fn check(o) {
	if o.digits < 1 || o.digits > 3 {
		throw `digits must be from 1 to 3, got ${o.digits}`;
	}
	if 10 ** o.digits < o.base {
		throw `${10 ** o.digits} numbers can't give each of ${o.base} symbols one`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base) }
}

fn outputs(o) {
	#{ cipher_text: symbol_type(10), table: "Homophones" }
}

// How often each symbol comes, in a thousand.
fn weights(base) {
	let letters = [82, 15, 28, 43, 127, 22, 20, 61, 70, 2, 8, 40, 24, 67, 75, 19, 1, 60, 63, 91, 28, 10, 24, 2, 20, 1];
	if base == 27 { return [180] + letters; }
	if base == 26 { return letters; }
	let even = [];
	even.pad(base, 1);
	even
}

fn table(o) {
	let count = 10 ** o.digits;
	let w = weights(o.base);
	let total = 0;
	for x in w { total += x; }
	let spare = count - o.base;
	// One each, the spare shared by weight, what's left over handed out
	// heaviest first.
	let shares = w.map(|x| 1 + x * spare / total);
	let given = 0;
	for x in shares { given += x; }
	let heaviest = [];
	for s in 0..o.base { heaviest.push(s); }
	heaviest.sort(|a, b| if w[b] != w[a] { w[b] - w[a] } else { a - b });
	let k = 0;
	while given < count {
		shares[heaviest[k % o.base]] += 1;
		given += 1;
		k += 1;
	}
	// The numbers shuffled, then dealt out in symbol order.
	let rng = new_random(o.seed);
	let numbers = [];
	for n in 0..count { numbers.push(n); }
	for n in 0..count - 1 {
		let j = n + (rng.next() * (count - n)).floor().to_int();
		if j >= count { j = count - 1; }
		let x = numbers[n];
		numbers[n] = numbers[j];
		numbers[j] = x;
	}
	let homophones = [];
	let at = 0;
	for s in 0..o.base {
		let mine = numbers.extract(at, shares[s]);
		mine.sort();
		homophones.push(mine);
		at += shares[s];
	}
	#{ base: o.base, digits: o.digits, homophones: homophones }
}

fn lock(o, i) {
	let t = table(o);
	let rng = new_random(o.seed + 1);
	let out = [];
	for s in i.plain_text {
		let n = rng.pick(t.homophones[s]);
		let group = [];
		group.pad(o.digits, 0);
		for k in 0..o.digits {
			group[o.digits - 1 - k] = n % 10;
			n /= 10;
		}
		out += group;
	}
	#{ cipher_text: out, table: t }
}

fn unlock(o, out) {
	let t = out.table;
	let count = 10 ** t.digits;
	let owner = [];
	owner.pad(count, -1);
	for (numbers, s) in t.homophones {
		for n in numbers {
			if n < 0 || n >= count {
				throw `the table gives ${n}, which isn't ${t.digits} digits`;
			}
			if owner[n] >= 0 {
				throw `the table gives ${n} to two symbols`;
			}
			owner[n] = s;
		}
	}
	let digits = out.cipher_text;
	if digits.len() % t.digits != 0 {
		throw `the digits, ${digits.len()} of them, don't make whole numbers of ${t.digits}`;
	}
	let text = [];
	for g in 0..digits.len() / t.digits {
		let n = 0;
		for k in 0..t.digits { n = n * 10 + digits[g * t.digits + k]; }
		if owner[n] < 0 {
			throw `${n} is no symbol's number`;
		}
		text.push(owner[n]);
	}
	#{ plain_text: text }
}

fn examples() {
	[
		#{
			options: #{ base: 27, seed: 3 },
			inputs: #{ plain_text: symbols(27, "THE DROWNED KEEP THEIR OWN COUNT") },
		},
		#{
			options: #{ base: 26, seed: 7 },
			inputs: #{ plain_text: symbols(26, "EEEEEEEEEEQZ") },
		},
		#{
			// Evenly shared: ten digits, a hundred numbers.
			options: #{ base: 10, seed: 1 },
			inputs: #{ plain_text: symbols(10, "0000111") },
		},
		#{
			options: #{ base: 27, digits: 3, seed: 2 },
			inputs: #{ plain_text: symbols(27, "A THOUSAND NAMES") },
		},
		#{
			options: #{ base: 27, digits: 1 },
			error: "10 numbers can't give each of 27 symbols one",
		},
	]
}
