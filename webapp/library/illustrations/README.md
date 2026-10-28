# Illustration files

An illustration draws lock values as canvases: a StarChart as its pages, a Maze
with a glyph in each cell, a cipher text in balanced-ternary marks. Locks
stay plain data and never draw (see locks/README.md); an illustration attached
to a chain draws what it's wired to, and a journal adds the canvases to
its pages:

```rhai
let lock = new_lock("raven")
	.add("enc", "huffman_encode", #{ input_base: 27, output_base: 3 })
	.add("sky", "star_chart", #{ input_base: 27, output_base: 3 })
	.connect("enc.code", "sky.code")
	.attach("pages", "star_pages", #{ large_star: 33.0 })
	.connect("sky.chart", "pages.chart");
// or, from chains/raven_sky.rhai, illustrations and all:
// let lock = chain("raven_sky");
let result = lock.run(#{ "enc.plain_text": text });
for canvas in result.canvases("pages") {
	let page = new_page(#{ size: vec2(1200, 1600) });
	page.add(canvas);
	journal_entry.add(page);
}
```

`lock.attach("pages", "star_pages", ...)` loads `star_pages.rhai` from
the journal's `illustrations/`, or else from here, the library's. Wiring
a value to an illustration leaves the chain as it was: the value is still an output, or an
input still needing its value, and locking and unlocking never run the
illustration. A chain file attaches illustrations too (see chains/README.md).
Adding an illustration means adding a file; nothing needs recompiling.

## What an illustration file defines

| function | returns |
|---|---|
| `options()` *(optional)* | `#{ NAME: default }`: its options. `lock.attach` can give only these, each like its default (a whole number is fine for a number, and comes as one). |
| `inputs(o)` | `#{ NAME: type }`: the values it draws, typed as lock files type them (`Base27Symbol`, `HuffmanCode`, `Maze`, `StarChart`, `Any`). They can follow the options: `trit_message`'s input is `` `Base${o.base}Symbol` ``. |
| `render(o, i)` | a canvas, or an array of canvases |
| `fallbacks(o)` *(optional)* | `#{ NAME: value }`: what inputs nothing's wired to draw with. The library's illustrations take each kind of text they write as a `TextStyle` input, so a chain can wire in a style source, and give fallbacks for them made from their options (a `<role>_size` option for each one's size). A `TextStyle` input comes whole, `#{ style, type, paragraph }`, and what a wired style leaves out (a pen-only style's glyphset and size, say) is taken from the fallback's. An input with no fallback must be wired. |
| `example()` *(optional)* | `#{ NAME: value }`: values to draw when nothing's wired, which the web editor previews and `cargo test` checks it draws |

`o` is the options, its defaults with what `lock.attach` gave over them.
`i` holds the values as lock files see them: symbols as arrays of whole
numbers, a value kind's data as its map. An illustration runs in the journal's
engine, so it can call everything a journal can: `new_canvas`, glyphsets,
brushes, `art::`, and a value kind's functions (`maze.path()`,
`maze.cells()`).

```rhai
// maze_glyphs: a Maze drawn on a grid of cell by cell.

fn options() {
	#{ cell: 70.0, corridors: "#9b2318", glyphs: "#2b1d12" }
}

fn inputs(o) {
	#{ maze: "Maze", corridor_style: "TextStyle", glyph_style: "TextStyle" }
}

// A style source gives a text style's bundle, #{ style, type, paragraph }:
// these give what render uses, and fill in what a wired style leaves out.
fn fallbacks(o) {
	let hand = new_handwriting_style();
	#{
		corridor_style: #{ style: new_style(new_brush_style(o.corridors, "Ink Pen", 1.6), hand) },
		glyph_style: #{
			style: new_style(new_brush_style(o.glyphs, "Ink Pen", 2.2), hand),
			type: new_type_style(glyphset("balanced_ternary"), o.cell * 0.8),
		},
	}
}

fn render(o, i) {
	let c = new_canvas();
	c.add(i.maze.path().scale_by(o.cell), i.corridor_style.style);
	let t = i.glyph_style.type;
	// ... a glyph of t.glyphset in each of i.maze.cells(), at its size (no
	// bigger than a cell holds), in i.glyph_style.style
	c
}

fn example() {
	let maze = new_maze(3, 3, #{ seed: 4 });
	maze.fill(symbols(27, "HELLO WORLD").symbols());
	#{ maze: maze }
}
```

The file's opening comment is its description, shown in the web editor's
palette.

## In the web editor

The lock editor's palette lists every illustration. Wire a chain's value to
one to see what it draws of the test run; New and Edit open an illustration
file in the middle, drawing its example as you type. An illustration written
there is used, by chains and journals in this browser, as soon as it
draws; to add it to the repository, put the file here, give it an example
on the showcase's illustrations page and an entry in the API reference, and
rebuild the illustration.
