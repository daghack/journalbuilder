# Renderer files

A renderer draws lock values as canvases: a StarChart as its pages, a Maze
with a glyph in each cell, a cipher text in balanced-ternary marks. Locks
stay plain data and never draw (see locks/README.md); a renderer attached
to a chain draws what it's wired to, and a journal adds the canvases to
its pages:

```rhai
let lock = new_lock("raven")
	.add("enc", "huffman_encode", #{ input_base: 27, output_base: 3 })
	.add("sky", "star_chart", #{ input_base: 27, output_base: 3 })
	.connect("enc.code", "sky.code")
	.attach("pages", "star_pages", #{ large_star: 33.0 })
	.connect("sky.chart", "pages.chart");
// or, from chains/raven_sky.rhai, renderers and all:
// let lock = chain("raven_sky");
let result = lock.run(#{ "enc.plain_text": text });
for canvas in result.canvases("pages") {
	let page = new_page(#{ size: vec2(1200, 1600) });
	page.add(canvas);
	journal_entry.add(page);
}
```

`lock.attach("pages", "star_pages", ...)` loads `star_pages.rhai` from
here (or from `journalbuild --renderers-dir DIR`). Wiring a value to a
renderer leaves the chain as it was: the value is still an output, or an
input still needing its value, and locking and unlocking never run the
renderer. A chain file attaches renderers too (see chains/README.md).
Adding a renderer means adding a file; nothing needs recompiling.

## What a renderer file defines

| function | returns |
|---|---|
| `options()` *(optional)* | `#{ NAME: default }`: its options. `lock.attach` can give only these, each like its default (a whole number is fine for a number, and comes as one). |
| `inputs(o)` | `#{ NAME: type }`: the values it draws, typed as lock files type them (`Base27Symbol`, `HuffmanCode`, `Maze`, `StarChart`, `Any`). They can follow the options: `trit_message`'s input is `` `Base${o.base}Symbol` ``. |
| `render(o, i)` | a canvas, or an array of canvases |
| `example()` *(optional)* | `#{ NAME: value }`: values to draw when nothing's wired, which the web editor previews and `cargo test` checks it draws |

`o` is the options, its defaults with what `lock.attach` gave over them.
`i` holds the values as lock files see them: symbols as arrays of whole
numbers, a value kind's data as its map. A renderer runs in the journal's
engine, so it can call everything a journal can: `new_canvas`, fonts,
brushes, `art::`, and a value kind's functions (`maze.path()`,
`maze.cells()`).

```rhai
// maze_glyphs: a Maze drawn on a grid of cell by cell.

fn options() {
	#{ cell: 70.0, corridors: "#9b2318", glyphs: "#2b1d12" }
}

fn inputs(o) {
	#{ maze: "Maze" }
}

fn render(o, i) {
	let c = new_canvas();
	c.add(i.maze.path().scale_by(o.cell), new_style(o.corridors, new_brush("Ink Pen", 0.3, 0.7, 1.6)));
	// ... a glyph in each of i.maze.cells()
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

The lock editor's palette lists every renderer. Wire a chain's value to
one to see what it draws of the test run; New and Edit open a renderer
file in the middle, drawing its example as you type. A renderer written
there is used, by chains and journals in this browser, as soon as it
draws; to add it to the repository, put the file here, give it an example
on the showcase's renderers page and an entry in the API reference, and
rebuild the renderer.
