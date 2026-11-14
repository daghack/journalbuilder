// The web (spider_web, book_cipher, vigenere_cipher): an old web drawn as
// he found it, a sentence caught in its gaps, enciphered with a word that
// is itself only given as places in the watcher's account (the content of
// this journal, on the first page). Drawn like a specimen: where and when
// it was found, a measure of its size, his notes on how to read it.

import "lib/inks" as inks;
import "lib/plates" as plates;
import "lib/book" as book;

const INK = inks::black();
const BLOOD = inks::blood();

seed(1616);

// The sentence, enciphered with LAKE; and LAKE, as where to find its
// letters in the account.
let account = content("watch");
let key = new_lock("key").add("b", "book_cipher", #{ seed: 7 })
	.run(#{ plain_text: symbols(27, "LAKE"), book: account });
let caught = new_lock("web").add("v", "vigenere_cipher", #{ plain_base: 27, key_base: 27, cipher_base: 27 })
	.run(#{ plain_text: symbols(27, "THE TOWERS IN THE WATER ARE LIT"), key: symbols(27, "LAKE") }).get("cipher_text").value();

let leaf = book::leaf("web", 16, #{ fold: "across", foxing: 170 });

// Where it was, when.
book::jot(leaf, "Under the east window, the ninth night. It was not there the night before, and it was finished.", vec2(70, 52), 1060, #{ size: 34, turn: -0.008 });

// The web, off the middle and torn, a heavier thread than the fallback.
let web = illustration("spider_web", #{ size: 1080.0, spokes: 9, ragged: 1.0, seed: 4, glyph_size: 46.0, editor: "web" })
	.run(#{ message: caught, thread_style: #{ style: inks::detail(INK, 1.5) } })[0];
book::place(leaf, web.translate_by(vec2(60, 170)), 10);

// A measure of it, as a naturalist leaves one: a hand's breadth in ten.
let rule_at = vec2(830, 1300);
leaf.figures.add(plates::ruler(rule_at, rule_at + vec2(300, 0), 10, 8.0, 5, 16.0), inks::contour(INK, 1.3));
leaf.figures.add(book::note("a hand's breadth", rule_at + vec2(70, 6), 240, #{ size: 24 }).canvas);
book::clear(leaf, new_rectangle(rule_at + vec2(150, 4), 330, 70), 4);

// How to read it, and where to find the word: the places in figures
// plainer than his cursive, so none is misread.
book::jot(leaf, "Begin at the highest spoke and go with the sun. The word is in the account: count the words, then the letters.", vec2(70, 1300), 680, #{ turn: -0.012 });
let places = write_block(key.text("references"), vec2(84, 1460), 640, book::figures_hand(36.0, BLOOD)).canvas.rotate_around(vec2(84, 1440), 0.01);
book::place(leaf, places, 10);

// The account, round the web and under it.
journal_entry.add(book::bind(leaf, account));
