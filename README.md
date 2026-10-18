# journalbuilder

Renders handwritten-looking journal pages from rhai scripts. Each journal
is a script that lays out strokes, glyphs from hand-drawn symbol sets, and
text enciphered through chains of locks; `journalbuild` runs it and renders
every page with [Darkly](https://github.com/daghack/darkly), a GPU brush
engine, and optionally as SVG.

## Quick start

```bash
cargo build --release
```

```bash
target/release/journalbuild journal_entry_17
```

Renders go to `journal_entry_17_output/` in the current directory (emptied first). Rendering needs a GPU
that wgpu can use (Metal on macOS). While it runs, one line of the
terminal shows its progress: a spinner while the script runs, then a bar
of the pages rendered, the page it's on and the time left
(`NO_PROGRESS=1` turns it off; it's off anyway when output isn't a
terminal).

Useful flags (`journalbuild` with no arguments lists them all):

- `--page 0,maze`: only these pages, by index or name.
- `--scale 0.25`: smaller output, much faster to render.
- `--svg`: also render each canvas as an SVG, and that SVG as a PNG.
- `--strict-glyphs`: fail if any character has no glyph, instead of
  drawing an X and warning.
- `--parchment`: render each page as one image, `darkly_{page}.png`, on
  procedural parchment: a mottled scroll browning toward ragged edges.
  Pages get a border of parchment round their strokes; typeset pages are
  a sheet the size of the page, and each tile of a split page a sheet of
  its own. Light inks are drawn as dark as they were light, hue kept, so
  they show.
- `--pdf`: put the pages in one PDF, `{journal}.pdf`, instead of images:
  each page's canvases layered into one image (on parchment, with
  `--parchment`), at 150 canvas units to the inch whatever the scale.
- `--stitch N`: also lay every N pages side by side, tops level, named
  for the pages they hold: `page_0` to `page_5` as `darkly_page_0-5.png`
  (the last may have fewer; a page left alone isn't stitched). Pages are
  rendered one image each, as with `--parchment`, on parchment with it.
  Not with `--pdf`.
- `--journals-dir`, `--output-dir`, `--fonts`, `--locks-dir`,
  `--layouts-dir`: read or write somewhere other than the repository's
  defaults.
- `--check-locks`: run every lock file's examples instead of building a
  journal.

## Web editor

[webapp/](webapp/README.md) is a browser editor for journal scripts. Pages
redraw as you type, as SVG or rendered with Darkly over WebGPU, and export
as a PDF or a zip of PNGs. The renderer is this crate built to
WebAssembly. To run it:

```bash
cd webapp/frontend && npm install && npm start
```

## Layout

| path | what's there |
|---|---|
| `webapp/journals/<name>/journal_entry.rhai` | one journal per directory |
| `webapp/journals/showcase` | a page each of every brush, font, glyph, art tool, text layout and lock type, with the calls that make them |
| `webapp/journals/api_reference` | the API reference: an entry for every function scripts can call, with its arguments, options and defaults |
| `webapp/locks/` | lock types, one rhai file each; see [webapp/locks/README.md](webapp/locks/README.md) |
| `webapp/layouts/` | page layouts, one rhai file each; see [webapp/layouts/README.md](webapp/layouts/README.md) |
| `webapp/config/fonts.json` | every font: its glyphs' SVG paths by character (a list for variants), `character_padding`, `space_width` and `fallback`; sets of symbols are fonts too |
| `src/journal/` | canvases, paths, glyph placement, fonts, hatching, gradients, parchment, and Darkly and SVG rendering |
| `src/journal/publish.rs` | writing an entry out: images per canvas or page, PDFs, SVGs |
| `src/journal/art/` | procedural path art: snakes and brambles, free or along a path |
| `src/journal/rhaientries/` | the rhai API journals and lock files use |
| `src/lockbuilder/` | runs lock chains forward (lock) and backward (unlock) |
| `src/bin/journalbuild/` | the command line |
| `webapp/` | the web editor: `wasm/` (the renderer for the browser) and `frontend/` (the Angular app), with the journals, locks, layouts and fonts above it builds from; see [webapp/README.md](webapp/README.md) |
| `tests/`, `scripts/smoke.sh` | tests, below |

## Writing a journal

A journal script fills in `journal_entry` with pages, each holding one or
more canvases of strokes:

```rhai
let ink = new_style(new_brush("Calligraphy", 0.15, 0.25, 2.0), #{ width: 2.5 }, #{ width: "rgba(0, 0, 0, 0.8)" });

let canvas = new_canvas()
	.add(new_circle(vec2(0.0, 0.0), 100.0), ink)
	.add(write_line("ink", vec2(-50.0, 0.0), #{ font: font("copper"), size: 50.0, style: ink }));

journal_entry.add(new_page(#{ name: "hello" }).add(canvas));
```

This renders `darkly_hello_0.png`: one image per canvas, named
`darkly_{page}_{canvas}.png` after the page's and canvas's names, or their
indices. With `--svg` there's also `hello_0.svg` and `hello_0.png`.

What to know:

- **One shape for every call.** What a call needs comes first, in order,
  then one map of everything optional, which can be left out:
  `path.hatch(4, #{ angle: 0.3 })`. Constructors' options also set any
  property of what they make, checked as setting it is:
  `new_brush("Ink Pen", 0.4, 0.8, 1.6, #{ pressure_profile: "taper" })`.
  Text calls take one map of text settings with the keys layout roles
  take (`font`, `size`, `style`, `spacing`, ...), so a role's settings can
  be passed straight in. Canvases, pages, the entry, avoidance trees and
  lock chains all take things with `add`, which returns what it was
  called on, so calls chain.
- **Units.** Coordinates are canvas units, one pixel at `--scale 1`. Each
  canvas is cropped to its strokes, plus `page.margin` (by default half the
  largest brush size).
- **Numbers and vectors.** Numbers can be whole or decimal anywhere
  (`new_circle(vec2(0, 0), 20)`). Vecs add, subtract, negate, and
  multiply or divide by numbers (`(a + b) / 2`), and have `length()`,
  `distance(other)`, `dot(other)`, `angle()`, `normalized()` and
  `rotated(angle)`; `vec3` and `vec4` do the same in more coordinates.
  Angles are radians: `degrees(45)` converts. `path.bounds()` and `canvas.bounds()` give
  `#{ min, max, center, width, height }`.
- **Brushes and widths.** `new_brush(name, min_pressure, max_pressure,
  size)` picks a Darkly brush; `size` is how wide its marks are. In
  `new_style(brush, #{ width }, #{ width: color })` (width 1 by default), `width` scales the brush's
  pressure in Darkly renders, so past `1 / max_pressure` a stroke gets no
  heavier (the render warns), but it's the stroke's geometric width in SVG.
- **Pressure.** `brush.pressure_profile` runs pressure along each stroke
  (`"ramp"`, `"taper"` or `"constant"`). `brush.pressure_variation = 0.2`
  lets it wander up to 20% either way, rising and falling about every
  `brush.variation_length` units (default 12), so ramps and tapers look
  less mechanical. `brush.pool = 0.4` pools ink toward the bottom of each
  path, each glyph for text: 40% heavier at the very bottom, rising over
  its lowest `brush.pool_depth` (default 0.3 of its height). Both are
  pressure in Darkly renders, so capped as width is, and width in the
  styled SVG output.
- **Going over strokes.** `#{ passes: 7 }` (or `style.passes = 7`) draws each stroke in the
  style seven times over, and `#{ jitter: 9 }` lets each pass's ends
  stray by up to 9 units, so a straight stroke turns, stretches and
  shifts a little each time: the scored, manic look of writing gone over
  again and again. One pass with a little jitter just roughens a hand.
  Passes are drawn whenever the canvas is, so whatever is in the style is
  gone over, text included, and bounds and avoidance see every pass. Their
  jitter is seeded from each stroke's own points, so it's the same every
  run.
- **Gradients.** In place of a colour, `new_gradient(["red", "gold",
  "blue"])` runs along each stroke from start to end; with `#{ from, to }`
  it's laid across the page between two points, and with `#{ center,
  radius }` out from a point. `#{ vertical: true }` runs down each path
  (each glyph) from its top to its bottom, for ink darker where it pools.
  Colours are spaced evenly, or placed as
  `[position, colour]`. `style.color = gradient` works too.
- **Shared values.** Paths, glyphs and canvases are shared: assigning one,
  passing it or looping over it gives the same value, so changes through any
  copy reach it. `.copy()` makes an independent one.
- **Randomness is seeded.** `random()`, `random_range(a, b)`, glyph variant
  picks, text layout and hatching all draw from one generator with a fixed
  seed, so a journal renders the same every run. `seed(n)` changes it.
- **Fonts and glyphs.** Fonts come from `webapp/config/fonts.json`, each with
  its glyphs and how text is set in it: `character_padding` between
  glyphs and `space_width` (both in font sizes), and a `fallback` font for
  characters it hasn't. Text is set with each glyph as close to the last
  as their outlines (the convex hulls of their strokes, as drawn) allow,
  plus the padding, so a `V` tucks in beside an `A` and a `.` under a
  `T`'s arm, while glyphs that share no height (an apostrophe after an
  `n`) only tuck in a little. `font("copper")` is a font as the file sets it;
  `font("loche_lower", #{ character_padding: 0.3 })` or
  `f.character_padding = 0.3` changes a copy. `skew` leans a font's
  glyphs into italics (`font("plain", #{ skew: 0.25 })`, in radians, tops
  forward), its fallbacks' too, without opening up its spacing; a glyph
  has its own `g.skew`. A character is looked for as
  it is in the font and its fallbacks, then in the other case, so a font of
  capitals sets lowercase text in them. `font("plain")` is a simple font
  made to read easily. `f.glyphs(text)` gives a text's glyphs,
  `f.glyph("a", #{ position, scale, rotation })` one glyph, placed,
  `f.variants("a")` all of a glyph's variants, and `font_names()` every
  font; `ternary_font("balanced_ternary")` maps letters to
  balanced-ternary digits. Text layout: `write_line(text, at, settings)`
  writes a line from a point, `write_block(text, at, width, settings)`
  wraps text to a width and says where the next goes (both take
  `path_jitter`, lines wandering up and down from straight), `new_textbox` flows
  glyphs into a box or any shape, `canvas.scatter_notes` scatters notes
  across a canvas, and `path.fill_with_text` / `fit_text` write along a
  path, each glyph turned to it, or held at `angle` (radians, 0 upright)
  with `#{ angle: 0 }`. Every one, roles too, takes `position_jitter`,
  `rotation_jitter` and `scale_jitter`, setting each glyph a little off
  true as a hand does.
- **Balanced ternary.** `balanced_ternary(2026)` (or `"+-0"` text, or an
  array of digits) is a value: `.value`, `.digits()`, and `.digits(3)` in
  chunks of three, padded in front. `font.glyph(bt)` draws a small one as
  a glyph, and `path.fill_with_text(bt, #{ style, size })` /
  `fit_text(bt, #{ style })` write its digits as marks along a path.
- **Keeping text clear.** Every text layout takes `avoid` (a path, canvas,
  glyph, or an array of them) and `clearance`, and keeps each glyph's
  whole outline that far from it. For more control, build an avoidance
  tree: `new_avoidance_tree().add(border, #{ clearance: 8 }).add_hull(drawing, #{ clearance: 4 })`
  keeps 8 from the border's lines and 4 from anywhere inside the drawing's
  outline; `add_region(path)` keeps out of a closed shape. Trees are
  shared, so `avoid.add(placed)` keeps later text off what's just been
  placed. With `layout: "words"`, a word that runs into something moves
  past it whole; `split_words: true` breaks it round it instead, for a
  tighter fit (or `split_words: 3`: no piece shorter than 3 letters), and
  `split_padding` (in sizes) keeps the pieces further off it.
- **Shapes.** Besides `new_path()` and its lines and curves, `new_line`,
  `new_circle`, `new_rectangle`, `new_arc`, `new_ellipse`, `new_polygon`
  (a star in one line with `#{ step: 2 }`), `new_star`, `new_wave` and
  `new_polyline(points)` make shapes, and `path.points_along(count)`
  gives evenly spaced points round one, with the way it runs: for rings
  of glyphs.
- **Hatching.** `path.hatch(spacing)` shades a closed
  path's inside with lines, each a stroke of its own; shapes inside or
  across others in the same path (`ring.append(hole)`) cut holes in it.
  `path.hatch(spacing, #{ angle: [a, b], roughness, bow, inset,
  fill_rule })` cross-hatches, bends lines, keeps them off the outline, and
  picks how overlaps fill.
- **Procedural art.** The `art` module draws shapes from a few
  parameters and a seed of their own (`#{ seed }`, 0 by default):
  `art::snake(head_center, tail_end, length)` gives a snake's `outline`,
  winding from its head to the tip of its tail and passing over and under
  itself, and its `spine`. `art::bramble(base, length)` grows a
  blackberry bramble and gives its `vines`, `leaves` and `flowers` as
  separate paths, which `canvas.add(plant, #{ vines: bark, leaves: green,
  flowers: ink })` strokes in different colours;
  `art::bramble_along(path, flower_size)` grows one along a path, such as
  a maze's, `art::snake_along(path, max_width)` makes a many-headed
  serpent of one, and `art::eye` and `art::moon` give the parts of eyes
  and moons to draw likewise.
- **Page sizes.** A page is as big as its strokes, plus `page.margin`;
  with `page.size = vec2(1200, 1600)` it's exactly that, from the
  origin, and strokes past its edges are cut off. The showcase is a book
  of such pages.
- **Splitting pages.** `page.split(#{ columns, rows })` cuts a named page into a
  grid of tiles, each a page of its own named `{name}_{row}_{column}`
  (`page.split(#{ tile_width: 1200, tile_height: 1600 })` makes tiles of a
  set size instead). Each tile renders exactly its share of the pixels of
  the whole page's render, with strokes that cross a seam drawn whole and
  cut off at it, so a canvas's tile images placed side by side in order
  make up that render; with ImageMagick, for a 2 x 2 split:

  ```bash
  magick \( darkly_big_0_0_0.png darkly_big_0_1_0.png +append \) \( darkly_big_1_0_0.png darkly_big_1_1_0.png +append \) -append +repage big.png
  ```

  `journal_entry.add(page.split(#{ columns: 2, rows: 2 }))` adds them all.
- **Books.** `new_book(#{ first: 3 })` numbers pages as they're added
  (`book.add(section, page)`), names them `03_section` and on, and keeps a
  `book.contents` of where each section starts; `book.next` is the next
  page's number, for its folio, and `roman(n)` writes it in numerals.
- **Page layouts.** A layout lays out pages as trees of flexbox boxes, and
  `typeset` pours text, art and ornaments into their slots, chaining the
  text through columns and pages until it runs out; see
  [Using layouts](#using-layouts) below.
- **Problems.** `warn("...")` reports something without stopping the
  script. Errors give the script line.

`webapp/journals/showcase` shows everything there is to draw with, each example
labelled with the calls that make it: render it with
`target/release/journalbuild showcase`.

`webapp/journals/api_reference` lists every function scripts can call, with its
arguments, options and defaults: render it with
`target/release/journalbuild api_reference --pdf`.

The module docs in `src/journal/rhaientries/` cover each area in more
detail: `style.rs` (brushes and styles, passes and jitter), `path.rs`
(paths, hatching, text along paths), `text.rs` (text boxes, scattered
notes), `glyphs.rs` (glyphs and fonts), `avoid.rs`, `layout.rs` (page
layouts), `markdown.rs` (marked-up text for layouts), `gradient.rs`, `random.rs`, `art.rs` and `locks/mod.rs`. The journals in `webapp/journals/` and
`tests/smoke_journals/smoke` use most of it.

## Using layouts

Text boxes and text along paths place glyphs where you say. For pages of
running text (articles, title pages, captioned plates) a layout does the
placing: it describes a page as boxes, and `typeset` pours the text into
them, column after column and page after page, until it runs out.

### Typesetting with a layout file

The layouts in `webapp/layouts/` are ready to use:

| layout | flows | art |
|---|---|---|
| `two_column_article` | `title`, `body`, `caption` | `divider` (a band's motif), `illustration` (optional) |
| `bordered_title` | `title`, `subtitle`, `credits` | `border`, `corner`, `flourish` |
| `captioned_figure` | `caption` | `figure` |

Give one the text (**flows**), how each kind of text and stroke looks
(**roles**) and the drawings (**art**), and add the pages it returns:

```rhai
let ink = new_style("rgba(30, 20, 20, 0.9)", new_brush("Ink Pen", 0.4, 0.9, 3.0));
let red = new_style("darkred", new_brush("Ink Pen", 0.4, 0.9, 3.0));

journal_entry.add(layout("two_column_article").typeset(#{
	name: "lunar",                   // pages lunar_0, lunar_1, ...
	flows: #{
		title: "Lunar Influence",
		body: "First paragraph...\n\nSecond paragraph...",
		caption: "the moon at its third quarter",
	},
	roles: #{
		body: #{ font: font("plain"), style: ink, size: 24.0, align: "justify", indent: 1.5 },
		heading: #{ font: font("loche_upper"), style: red, size: 80.0 },
		caption: #{ font: font("plain"), style: red, size: 20.0, align: "center" },
		initial: #{ font: font("loche_upper"), style: red },
		folio: #{ font: font("plain"), style: red, size: 20.0 },
		rule: #{ style: red },
		divider: #{ style: red },
	},
	art: #{ divider: new_circle(vec2(0, 0), 10), illustration: moon_canvas },
}));
```

- **Flows** are strings. `"\n"` breaks a line and a blank line starts a
  paragraph. A flow that doesn't fit its box carries on in the next box
  of the same name, then onto the next page. A flow can also be
  `markdown(text)`: `*emphasis*`, `**strong**`, `~~struck out~~`, and
  `[text]{settings}` for colour, size, lean, passes, spacing, and lines
  under and through, which break where the text does (see
  [webapp/layouts/README.md](webapp/layouts/README.md#marked-up-text)).
- **Roles** are maps of settings, by name. Text roles need `font`,
  `style` and `size` (the symbol scale, in canvas units); the rest are
  optional: `line_height`, `line_jitter`, `align`, `valign`, `indent`, `paragraph_gap`,
  `spacing`, `jagged`, `clearance`, `split_words`, `split_padding`,
  `fit`, and `em` and `strong` (how markdown's emphasis and strong text
  look). Art, rules and bands need only a `style`. Each layout file's
  header comment lists the roles it uses.
- **Art** is a canvas (drawn in its own styles), a path or glyph (drawn in
  the role's style), or a function given its box,
  `#{ x, y, width, height, center }`, that returns a canvas or path made
  to measure.

The pages come out exactly the layout's `page_size()` (or the `width` and
`height` you give in the map), so they line up, and they render, split
and go into PDFs like any other page. Typesetting fails, naming the flow,
if a page sets none of what's left of the text.

### Writing a layout

A layout is a function from a page number to that page's tree of boxes.
For a one-off, write it in the journal:

```rhai
fn two_up(n, info) {
	column([
		if n == 0 { flow("title", #{ role: "heading", height: 80, fit: true, align: "center" }) } else { spacer() },
		row([
			flow("body", #{ grow: 1 }),
			rule(#{ width: 4 }),
			flow("body", #{ grow: 1 }),
		], #{ grow: 1, gap: 40 }),
		label(`${n + 1}`, #{ role: "folio", height: 30, align: "center" }),
	], #{ padding: 60, gap: 30 })
}
let pages = new_layout("two_up", Fn("two_up")).typeset(#{ width: 1200, height: 1600, flows: flows, roles: roles });
```

To reuse it, put `page(n, info)` (and, optionally, `page_size()`) in
`webapp/layouts/<name>.rhai` and load it with `layout("<name>")`; no rebuild is
needed. Rows and columns lay out their children with flexbox rules:
`grow` shares out the space left, `width`/`height` take units or
`"40%"`, and `gap`, `padding`, `margin`, `justify` and `align` work as in
CSS. The leaves are `flow`, `label`, `initial` (a drop cap), `figure`,
`band` (art repeated as an ornamental strip), `rule` and `spacer`. Any box
can override its role's settings: `flow("body", #{ grow: 1, align:
"center" })`. `info.flows` says how much of each flow is left, so a layout
can change as the text runs out (a closing page, say), and `info.art`
which art it was given.

### Wrapping text round art

A box placed with `position: "absolute"` is taken out of the flow, and
text in its parent wraps round it: round its box (`wrap: "box"`, the
default), round the outline of what's drawn in it (`wrap: "hull"`), or
not at all (`wrap: "none"`).

```rhai
column([
	figure("sigil", #{ position: "absolute", top: 200, left: 150, width: 300, height: 300, wrap: "hull" }),
	flow("body", #{ grow: 1, clearance: 12, split_words: true, split_padding: 0.4 }),
], #{ grow: 1 })
```

`clearance` is how far text keeps from what it wraps round. By default a
word that doesn't fit before it goes on after it, whole; `split_words`
breaks it round the figure instead, so lines hug the outline more
tightly, and `split_padding` (in sizes) adds room at the breaks. An
`initial` placed the same way is a drop cap the text wraps round.

[webapp/layouts/README.md](webapp/layouts/README.md) has every box, style key and role
setting; `webapp/journals/showcase` uses each layout, and an inline one.

## Locks

Locks encipher text for a journal to draw. Chain them, run the chain, and
draw what comes out, here a maze of glyphs:

```rhai
let lock = new_lock("page_one_lock")
	.add("sub", "substitution_cipher", #{ base_n: 27 })
	.add("maze", "maze_lock", #{ input_base: 27 })
	.connect("sub.cipher_text", "maze.maze_text")
	.set_input("sub.ordering", symbols(27, "IKBNAZXYFTQWJ HRGLMOUVCPEDS"))
	.set_input("maze.random_seed", symbols(10, "10843"));
let result = lock.run(#{ "sub.plain_text": symbols(27, "A SECRET") });
for cell in result.get("maze.generated_maze").maze().cells() { /* ... */ }
lock.unlock(result).text("sub.plain_text");   // "A SECRET"
```

Each lock type is a rhai file in `webapp/locks/`, so adding one doesn't need a
rebuild. [webapp/locks/README.md](webapp/locks/README.md) describes what a lock file
defines; `journalbuild --check-locks` tests them.

## Tests

```bash
./scripts/smoke.sh
```

`smoke.sh` builds in release mode and runs `cargo test` (which also
evaluates every journal script and every lock file's examples). It then runs
`journalbuild --check-locks`, renders the smoke fixture in full, and renders
page 0 of each journal at quarter scale. Last, it checks that the web
editor's renderer still builds for the browser and, if the editor's npm
packages are installed and its wasm built, runs its unit tests and builds
it. Renders go to a temporary directory; `--keep` keeps them.
