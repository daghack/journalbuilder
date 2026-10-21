// A message locked with a text's Huffman code, and the code drawn as a sky
// of constellations, a page each: the pages are the key.
//
// The chain is webapp/chains/raven_sky.rhai. Its huffman_encode lock
// builds a Huffman code from the text (in base 27: letters and spaces;
// anything else is left out), rewriting it in the chain's output_base, and
// gives the code; huffman_apply locks the message, MESSAGE, with that
// code; star_chart, an art node, draws the code as named stars
// cut into constellations of at most most_stars stars, every one fitted to
// its page. Neighbouring constellations share a star under the same name:
// whoever knows the pages make one sky can join them on those stars, read
// the code off it, and so read the message on the last page. star_chart's
// own unlocking does just that, and this journal checks it gets the text
// and the message back.
//
// A letter's star is named for its letter, the space's star is Vacii and
// the root Shiathri. The locks only make values; the chain's star_pages
// renderer draws the chart, numbering stars page by page in balanced
// ternary and indexing them to their names, each link carrying its digit
// in balanced-ternary marks, and trit_message writes the message's cipher
// text in the same marks.
//
// To chart another text, change TEXT; to lock another message, MESSAGE. The chain's settings (the output
// base, how many stars a constellation has, the seeds) are its own: the
// lock editor edits them, and this journal follows along.

// The text: here The Raven (Poe, 1845), its first six stanzas.
const TEXT = "Once upon a midnight dreary, while I pondered, weak and weary, Over many a quaint and curious volume of forgotten lore, While I nodded, nearly napping, suddenly there came a tapping, As of some one gently rapping, rapping at my chamber door. Tis some visitor, I muttered, tapping at my chamber door, Only this and nothing more. Ah, distinctly I remember it was in the bleak December; And each separate dying ember wrought its ghost upon the floor. Eagerly I wished the morrow; vainly I had sought to borrow From my books surcease of sorrow, sorrow for the lost Lenore, For the rare and radiant maiden whom the angels name Lenore, Nameless here for evermore. And the silken, sad, uncertain rustling of each purple curtain Thrilled me, filled me with fantastic terrors never felt before; So that now, to still the beating of my heart, I stood repeating Tis some visitor entreating entrance at my chamber door, Some late visitor entreating entrance at my chamber door; This it is and nothing more. Presently my soul grew stronger; hesitating then no longer, Sir, said I, or Madam, truly your forgiveness I implore; But the fact is I was napping, and so gently you came rapping, And so faintly you came tapping, tapping at my chamber door, That I scarce was sure I heard you, here I opened wide the door; Darkness there and nothing more. Deep into that darkness peering, long I stood there wondering, fearing, Doubting, dreaming dreams no mortal ever dared to dream before; But the silence was unbroken, and the stillness gave no token, And the only word there spoken was the whispered word, Lenore? This I whispered, and an echo murmured back the word, Lenore! Merely this and nothing more.";
// The message, locked with the text's code.
const MESSAGE = "Quoth the Raven Nevermore";

const W = 1200.0;
const H = 1600.0;

// ---- the lock ----

let text = symbols(27, TEXT, #{ lenient: true });
let message = symbols(27, MESSAGE, #{ lenient: true });
let lock = chain("raven_sky");
let result = lock.run(#{ "enc.plain_text": text, "msg.plain_text": message });
let unlocked = lock.unlock(result);
assert(unlocked.text("enc.plain_text") == text.text() && unlocked.text("msg.plain_text") == message.text(), "the pages should unlock the text and the message");

// ---- pages ----

for (canvas, p) in result.canvases("pages") {
	let sheet = new_page(#{ name: `constellation_${p + 1}`, size: vec2(W, H) });
	sheet.add(canvas);
	journal_entry.add(sheet);
}
let sheet = new_page(#{ name: "message", size: vec2(W, H) });
sheet.add(result.canvases("message")[0]);
journal_entry.add(sheet);
