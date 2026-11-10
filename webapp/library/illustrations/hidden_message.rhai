// hidden_message: a secret picked out of a plain text, its letters found
// in order wherever they come ("secret" in "SECts RETurn"), in either
// case, its spaces skipped. The plain text is written in plain_style,
// wrapped into a block, and the secret's letters in secret_style: its
// pen, whole, and what its type sets over the plain text's (by default,
// a line under them), so they keep the text's glyphs and size unless it
// gives its own. With placement "first" each letter is the first that
// fits; with "spread" they're spaced through the text. One canvas, width
// across, the first line a glyph scale down. Draws plain_text and secret,
// Strings, in plain_style and secret_style, TextStyles.

fn options() {
	#{ width: 900.0, placement: "first", plain_size: 30.0, plain_ink: "#2b1d12", secret_ink: "#9b2318" }
}

fn inputs(o) {
	#{ plain_text: "String", secret: "String", plain_style: "TextStyle", secret_style: "TextStyle" }
}

// Unwired, near-black and red ink pens in plain, the secret underlined.
fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		plain_style: #{ style: new_style(new_brush_style(o.plain_ink, "Ink Pen", 1.6), hand), type: new_type_style(glyphset("plain"), o.plain_size) },
		secret_style: #{ style: new_style(new_brush_style(o.secret_ink, "Ink Pen", 1.6), hand), type: #{ underline: true } },
	}
}

fn render(o, i) {
	if o.placement != "first" && o.placement != "spread" {
		throw `placement should be "first" or "spread", got "${o.placement}"`;
	}
	let text = [];
	for c in i.plain_text.chars() { text.push(c); }
	let marked = places(text, i.secret, o.placement);
	// The text marked up, each run of the secret's letters one span, spaces
	// between them included, so lines under or through them run on across
	// the run ("dolor sit" one line, not two).
	let markup = "";
	let plain = "";
	let run = "";
	let gap = "";
	let next = 0;
	for (c, at) in text {
		if next < marked.len() && marked[next] == at {
			if plain != "" { markup += escape_markdown(plain); plain = ""; }
			run += gap + c;
			gap = "";
			next += 1;
		} else if run != "" && blank(c) {
			gap += c;
		} else {
			if run != "" { markup += `[${escape_markdown(run)}]{secret}`; run = ""; }
			plain += gap + c;
			gap = "";
		}
	}
	if run != "" { markup += `[${escape_markdown(run)}]{secret}`; }
	markup += escape_markdown(plain + gap);
	let span = #{ style: i.secret_style.style, type: set_in(i.secret_style.type) };
	let settings = i.plain_style;
	let spans = if "spans" in settings { settings.spans } else { #{} };
	spans.secret = span;
	settings.spans = spans;
	let t = settings.type;
	let top = (t.glyph_scale ?? 0.0) * t.glyph_scale_factor;
	new_canvas().add(write_block(markdown(markup), vec2(0, top), o.width, settings).canvas)
}

// What the editor previews it with.
fn example() {
	#{ plain_text: "Sects return to the stones each winter, and the circle keeps what they bring.", secret: "secret" }
}

// Where in `text` (an array of characters) the secret's letters go, in
// order: each the first that fits, or with "spread" the first at or past
// its share of the text that still leaves room for the rest.
fn places(text, secret, placement) {
	let want = [];
	for c in secret.chars() {
		if !blank(c) { want.push(c.to_lower()); }
	}
	let n = want.len();
	let lower = text.map(|c| c.to_lower());
	let first = [];
	let at = -1;
	for k in 0..n {
		at += 1;
		while at < lower.len() && lower[at] != want[k] { at += 1; }
		if at >= lower.len() {
			throw `the plain text doesn't hold the secret "${secret}": no '${want[k]}' for its letter ${k + 1}`;
		}
		first.push(at);
	}
	if placement == "first" || n == 0 { return first; }
	// The last place each letter can go and leave room for the rest.
	let latest = first.map(|at| 0);
	at = lower.len();
	let k = n;
	while k > 0 {
		k -= 1;
		at -= 1;
		while lower[at] != want[k] { at -= 1; }
		latest[k] = at;
	}
	let spread = [];
	let prev = -1;
	for k in 0..n {
		let aim = (2 * k + 1) * lower.len() / (2 * n);
		let from = if aim > prev + 1 { aim } else { prev + 1 };
		let at = latest[k];
		if from <= at {
			at = from;
			while lower[at] != want[k] { at += 1; }
		}
		spread.push(at);
		prev = at;
	}
	spread
}

fn blank(c) {
	c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

// The settings a type sets, as a map: a span's type over the text's, so
// what it leaves unset (a pen-only style's glyphset and size) is the
// text's.
fn set_in(t) {
	let m = #{};
	if type_of(t.glyphset) != "()" { m.glyphset = t.glyphset; }
	if type_of(t.glyph_scale) != "()" { m.glyph_scale = t.glyph_scale; }
	if t.glyph_scale_factor != 1.0 { m.glyph_scale_factor = t.glyph_scale_factor; }
	if type_of(t.glyph_outline_gap) != "()" { m.glyph_outline_gap = t.glyph_outline_gap; }
	if t.glyph_tracking != 0.0 { m.glyph_tracking = t.glyph_tracking; }
	if type_of(t.space_width) != "()" { m.space_width = t.space_width; }
	if type_of(t.skew) != "()" { m.skew = t.skew; }
	if type_of(t.joined) != "()" { m.joined = t.joined; }
	if type_of(t.underline) != "bool" { m.underline = t.underline; }
	if type_of(t.strike) != "bool" { m.strike = t.strike; }
	m
}
