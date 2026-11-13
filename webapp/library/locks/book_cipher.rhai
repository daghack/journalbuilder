// book_cipher: each letter written as where to find it in a book, a text
// both sides hold (a page of the journal, say): WORD.LETTER, the word
// counted from the book's first and the letter from the word's first, so
// in "The stars are right" 2.3 is the A of stars. With mode "initial" a
// reference is a word alone, standing for its first letter: 3 is A. A
// space is written /. Words are what blanks part, and count only if they
// have a letter; only letters are counted in them (in "don't", T is 4),
// in either case.
//
// A letter the book holds more than once takes any of its places, picked
// by the seed, so a letter's references differ through the text. A letter
// the book doesn't hold is an error naming it.
//
// The text is base 27 (space, A to Z) or 26 (A to Z).

fn options() {
	#{ base: integer(27), mode: one_of(["letter", "initial"], "letter"), seed: integer(0) }
}

fn check(o) {
	if o.base != 26 && o.base != 27 {
		throw `base must be 26 (A to Z) or 27 (space, A to Z), got ${o.base}`;
	}
}

fn inputs(o) {
	#{ plain_text: symbol_type(o.base), book: "String" }
}

fn outputs(o) {
	// The book is passed through: unlocking needs it.
	#{ references: "String", book: "String" }
}

// The book's words, each its letters alone, in capitals.
fn words(book) {
	let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
	let words = [];
	let word = "";
	for c in (book.to_upper() + " ").chars() {
		if c == ' ' || c == '\n' || c == '\t' || c == '\r' {
			if word != "" { words.push(word); }
			word = "";
		} else if letters.index_of(c) >= 0 {
			word += c;
		}
	}
	words
}

fn lock(o, i) {
	let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
	// Every reference there is to each letter.
	let places = [];
	for l in 0..26 { places.push([]); }
	for (word, w) in words(i.book) {
		for (c, k) in word.chars() {
			if o.mode == "letter" {
				places[letters.index_of(c)].push(`${w + 1}.${k + 1}`);
			} else if k == 0 {
				places[letters.index_of(c)].push(`${w + 1}`);
			}
		}
	}
	let rng = new_random(o.seed);
	let references = "";
	for s in i.plain_text {
		let l = s - (o.base - 26);
		if references != "" { references += " "; }
		if l < 0 {
			references += "/";
			continue;
		}
		if places[l].is_empty() {
			throw if o.mode == "letter" { `the book has no '${letters[l]}'` } else { `no word of the book begins with '${letters[l]}'` };
		}
		references += rng.pick(places[l]);
	}
	#{ references: references, book: i.book }
}

fn unlock(o, out) {
	let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
	let words = words(out.book);
	let plain = [];
	for token in out.references.split(" ") {
		if token == "" { continue; }
		if token == "/" {
			if o.base == 26 { throw "'/' is a space, which base 26 hasn't"; }
			plain.push(0);
			continue;
		}
		let parts = token.split(".");
		if parts.len() != (if o.mode == "letter" { 2 } else { 1 }) {
			throw `'${token}' isn't a reference: ${if o.mode == "letter" { "WORD.LETTER" } else { "a word's number" }}`;
		}
		let w = number(parts[0], token);
		let k = if o.mode == "letter" { number(parts[1], token) } else { 1 };
		if w < 1 || w > words.len() {
			throw `'${token}': the book has ${words.len()} words`;
		}
		if k < 1 || k > words[w - 1].len() {
			throw `'${token}': word ${w}, ${words[w - 1]}, has ${words[w - 1].len()} letters`;
		}
		plain.push(letters.index_of(words[w - 1][k - 1]) + (o.base - 26));
	}
	#{ plain_text: plain }
}

// Digits as a whole number; anything else isn't part of a reference.
fn number(digits, token) {
	let n = 0;
	for c in digits.chars() {
		let d = "0123456789".index_of(c);
		if d < 0 { throw `'${token}' isn't a reference: '${c}' isn't a digit`; }
		n = n * 10 + d;
	}
	if digits == "" { throw `'${token}' isn't a reference`; }
	n
}

fn examples() {
	let book = "The stars are right, and the moon is dark; quiet now: the black goat of the woods wakes.";
	[
		#{
			options: #{},
			inputs: #{ plain_text: symbols(27, "DC AB"), book: "ab cd" },
			outputs: #{ references: "2.2 2.1 / 1.1 1.2" },
		},
		#{
			// Words' first letters: Each Night I Count Them.
			options: #{ mode: "initial" },
			inputs: #{ plain_text: symbols(27, "NICE TIN"), book: "Each night — I count them!" },
			outputs: #{ references: "2 3 4 1 / 5 3 2" },
		},
		#{
			options: #{ seed: 7 },
			inputs: #{ plain_text: symbols(27, "THE STARS ARE RIGHT"), book: book },
		},
		#{
			options: #{ base: 26, mode: "initial", seed: 3 },
			inputs: #{ plain_text: symbols(26, "BADGOAT"), book: book },
		},
		#{
			options: #{},
			inputs: #{ plain_text: [], book: "" },
			outputs: #{ references: "" },
		},
		#{
			options: #{},
			inputs: #{ plain_text: symbols(27, "ZEAL"), book: book },
			error: "the book has no 'Z'",
		},
		#{
			options: #{ mode: "initial" },
			inputs: #{ plain_text: symbols(27, "SEAT"), book: book },
			error: "no word of the book begins with 'E'",
		},
		#{
			options: #{ base: 10 },
			error: "base must be 26 (A to Z) or 27 (space, A to Z), got 10",
		},
	]
}
