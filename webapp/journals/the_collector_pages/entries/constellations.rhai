const TEXT = "You and I are not the same, we eat each other both night and die.";

const W = 1200.0;
const H = 1600.0;

// ---- the lock ----

let text = symbols(27, TEXT, #{ lenient: true });
let lock = chain("raven_sky");
let result = lock.run(#{ "enc.plain_text": text, "msg.plain_text": text });
let unlocked = lock.unlock(result);

for (canvas, p) in result.canvases("pages") {
	let sheet = new_page(#{ name: `constellation_${p + 1}`, size: vec2(W, H) });
	sheet.add(canvas);
	journal_entry.add(sheet);
}
let sheet = new_page(#{ name: "message", size: vec2(W, H) });
sheet.add(result.canvases("message")[0]);
journal_entry.add(sheet);
