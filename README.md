# journalbuilder

Renders handwritten-looking journal pages from rhai scripts. Each journal
is a directory of entries, scripts that lay out strokes, glyphs from
hand-drawn symbol sets, and text enciphered through chains of locks, with
the files they use; `journalbuild` runs its entries and renders every page
with [Darkly](https://github.com/daghack/darkly), a GPU brush engine, and
optionally as SVG.

## Quick start

```bash
cargo build --release
```

```bash
target/release/journalbuild journal_entries --entry entry_17
```

Renders go to `journal_entries_output/entry_17/` in the current directory
(emptied first), a directory for each entry built; leave out `--entry` for
every entry. Rendering needs a GPU
that wgpu can use (Metal on macOS). While it runs, one line of the
terminal shows its progress: a spinner while the script runs, then a bar
of the pages rendered, the page it's on and the time left
(`NO_PROGRESS=1` turns it off; it's off anyway when output isn't a
terminal).

Useful flags (`journalbuild` with no arguments lists them all):

- `--entry letter_1,response_1`: only these entries.
- `--page 0,maze`: only these pages of each entry, by index or name.
- `--scale 0.25`: smaller output, much faster to render.
- `--svg`: also render each canvas as an SVG, and that SVG as a PNG.
- `--strict-glyphs`: fail if any character has no glyph, instead of
  drawing an X and warning.
- `--parchment`: render each page as one image, `darkly_{page}.png`, on
  procedural parchment: a mottled scroll browning toward ragged edges.
  Pages get a border of parchment round their strokes; typeset pages are
  a sheet the size of the page, and each tile of a split page a sheet of
  its own. Light inks are drawn as dark as they were light, hue kept, so
  they show, and ink soaks in a little unevenly: very slightly thinner in
  places, in blotches and grain across the sheet.
- `--pdf`: put each entry's pages in one PDF, `{entry}.pdf`, instead of images:
  each page's canvases layered into one image (on parchment, with
  `--parchment`), at 150 canvas units to the inch whatever the scale. With
  `--stitch N`, each PDF page is N pages side by side.
- `--stitch N`: also lay every N pages side by side, tops level, named
  for the pages they hold: `page_0` to `page_5` as `darkly_page_0-5.png`
  (the last may have fewer; a page left alone isn't stitched). Pages are
  rendered one image each, as with `--parchment`, on parchment with it.
  With `--pdf`, the PDF's pages are the stitches instead (a page left
  alone at the end is a page of its own).
- `--journals-dir`, `--library`, `--output-dir`: read or write somewhere
  other than the repository's defaults. A journal can also be given as a
  path, `journalbuild path/to/journal`.
- `--check-locks`: run every lock and chain file's examples instead of
  building a journal: the library's, or with a journal's name, its own.
- `--check-gpu`: check that Darkly can render here (there's a GPU wgpu
  can use) instead of building a journal; fails if it can't.

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
| `journals/<name>/` | one journal per directory: `journal.json` (its title and entries in order), `entries/<entry>.rhai`, and files of its own in front of the library's (below) |
| `webapp/journals/` | the two journals built into the web editor, the showcase and the API reference; it imports the others from `journals/` |
| `webapp/journals/showcase` | a page each of every brush, glyphset, glyph, art tool, text layout and lock type, with the calls that make them |
| `webapp/journals/api_reference` | the API reference: an entry for every function scripts can call, with its arguments, options and defaults |
| `webapp/library/` | the built-in library every journal stands in front of, a directory for each kind of file: |
| `webapp/library/locks/` | lock types, one rhai file each; see [webapp/library/locks/README.md](webapp/library/locks/README.md) |
| `webapp/library/layouts/` | page layouts, one rhai file each; see [webapp/library/layouts/README.md](webapp/library/layouts/README.md) |
| `webapp/library/illustrations/` | what chains draw their values with; see [webapp/library/illustrations/README.md](webapp/library/illustrations/README.md) |
| `webapp/library/chains/` | locks wired together, for `chain(name)`; see [webapp/library/chains/README.md](webapp/library/chains/README.md) |
| `webapp/library/styles/` | text styles, for `text_style(name)`: `options()` and `style(o)`, giving `#{ style, type, paragraph, spans }` |
| `webapp/library/content/` | text (`.txt`, `.md`) and symbols (`.json`, `{ base, symbols }`), for `content(name)` |
| `webapp/library/lib/` | helpers any script imports: `import "lib/NAME" as NAME;` |
| `webapp/library/glyphsets/` | a `NAME.json` for each glyphset: its glyphs' SVG paths by character (a list for variants), `character_padding` (scripts' `glyph_outline_gap`), `space_width` and `fallback`; sets of symbols are glyphsets too |
| `src/journal/` | canvases, paths, glyph placement, glyphsets, hatching, gradients, parchment, and Darkly and SVG rendering |
| `src/journal/publish.rs` | writing an entry out: images per canvas or page, PDFs, SVGs |
| `src/journal/art/` | procedural path art: snakes and brambles, free or along a path |
| `src/journal/rhaientries/` | the rhai API journals and lock files use |
| `src/lockbuilder/` | runs lock chains forward (lock) and backward (unlock) |
| `src/bin/journalbuild/` | the command line |
| `webapp/` | the web editor: `wasm/` (the renderer for the browser) and `frontend/` (the Angular app), with the journals and library above it builds from; see [webapp/README.md](webapp/README.md) |
| `tests/`, `scripts/smoke.sh` | tests, below |

## Writing a journal

A journal is a directory, `journals/<name>/`, with a manifest,
`journal.json`:

```json
{
  "version": 1,
  "title": "Nivelia's Letters",
  "description": "Her letter, and the reply.",
  "entries": ["letter_1", "response_1"]
}
```

Each entry is a script, `entries/<entry>.rhai`, run on its own. Beside
them, a journal can have files of its own, each kind in a directory:
`locks/`, `layouts/`, `illustrations/`, `chains/`, `styles/` (text
styles, for `text_style(name)`), `lib/` (helpers, `import "lib/name" as
name;`), `glyphsets/` and `content/` (text and symbols, for
`content(name)`). They stand in front of the built-in library,
`webapp/library/`: a journal's file stands in for the library's of the
same name, and journalbuild notes when one does. Entries share these
files, not code.

An entry script fills in `journal_entry` with pages, each holding one or
more canvases of strokes:

```rhai
let ink = new_style(new_brush_style("rgba(0, 0, 0, 0.8)", "Calligraphy", 2.0, #{ min_pressure: 0.4, max_pressure: 0.6 }), new_handwriting_style());

let canvas = new_canvas()
	.add(new_circle(vec2(0.0, 0.0), 100.0), ink)
	.add(write_line("ink", vec2(-50.0, 0.0), #{ style: ink, type: new_type_style(glyphset("copper"), 50.0) }));

journal_entry.add(new_page(#{ name: "hello" }).add(canvas));
```

This renders `darkly_hello_0.png` (in the entry's output directory): one image per canvas, named
`darkly_{page}_{canvas}.png` after the page's and canvas's names, or their
indices. With `--svg` there's also `hello_0.svg` and `hello_0.png`.

What to know:

- **One shape for every call.** What a call needs comes first, in order,
  then one map of everything optional, which can be left out:
  `path.hatch(4, #{ angle: 0.3 })`. Constructors' options also set any
  property of what they make, checked as setting it is:
  `new_brush_style("black", "Ink Pen", 1.6, #{ pressure_profile: "taper" })`.
  Text calls take one settings map, `#{ style, type, paragraph }` and
  the call's own options, as layout roles do, so a role can be passed
  straight in. Canvases, pages, the entry, avoidance trees and lock
  chains all take things with `add`, which returns what it was called on,
  so calls chain.
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
- **Styles.** A style is what strokes are drawn in:
  `new_style(brush_style, handwriting_style)`, the ink and the hand, their
  settings read and set on the style as one (`style.color`,
  `style.passes`). `default_style()` is black, in an Ink Pen 1.2 wide.
- **Brush styles.** `new_brush_style(color, brush, brush_size)` picks a
  Darkly brush; `brush_size` is how wide its marks are, and pressure, from
  `min_pressure` to `max_pressure` (0.3 to 0.7 by default), how heavy. In
  SVG output a stroke is `brush_size * max_pressure` wide.
  `pressure_profile` runs pressure along each stroke (`"ramp"`, `"taper"`
  or `"constant"`). `pressure_wander: 0.2` lets it wander up to 20% either
  way, rising and falling about every `pressure_wander_length` units
  (default 12), so ramps and tapers look less mechanical. `pool: 0.4`
  pools ink toward the bottom of each path, each glyph for text: 40%
  heavier at the very bottom, rising over its lowest `pool_depth`
  (default 0.3 of its height). `color_jitter: 0.05` draws each stroke (each
  pass of it, too) a little off its colour: lightness moved up to 0.05
  either way in OKLab, hue and chroma up to half that (less in dark
  colours, so they keep their hue); gradients are moved
  point by point. Default 0.
- **Handwriting.** `new_handwriting_style(#{ passes: 7, pass_end_jitter: 9 })`
  draws each stroke seven times over, each pass's ends straying by up to 9
  units, so a straight stroke turns, stretches and shifts a little each
  time: the scored, manic look of writing gone over again and again. One
  pass with a little jitter just roughens a hand. `stroke_point_jitter`
  moves every point of every pass, so curves change shape too.
  `glyph_position_jitter`, `glyph_rotation_jitter` and `glyph_scale_jitter`
  set each glyph text calls place a little off true, and `baseline_wander`
  bends the lines `write_line` and `write_block` write along. Every jitter
  is in canvas units, drawn from the journal's generator as strokes are
  added to a canvas, so bounds and avoidance see every pass.
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
  picks, text layout, hatching, handwriting and procedural art without a
  seed of its own all draw from one generator with a fixed seed, so a
  journal renders the same every run. `seed(n)` changes it.
- **Glyphsets and glyphs.** Glyphsets come from `glyphsets/`, the
  journal's and the library's, one file each, with its glyphs and how text is set in it: `glyph_outline_gap`
  between glyphs and `space_width` (both in glyph scales), and a
  `fallback` glyphset for characters it hasn't. Text is set with each
  glyph as close to the last as their outlines (the convex hulls of their
  strokes, as drawn) allow, plus the gap, so a `V` tucks in beside an `A`
  and a `.` under a `T`'s arm, while glyphs that share no height (an
  apostrophe after an `n`) only tuck in a little. `glyphset("copper")` is
  a glyphset as the file sets it; `glyphset("loche_lower", #{ glyph_outline_gap: 0.3 })`
  or `g.glyph_outline_gap = 0.3` changes a copy. `skew` leans its glyphs
  into italics (`glyphset("plain", #{ skew: 0.25 })`, in radians, tops
  forward), its fallbacks' too, without opening up its spacing; a glyph
  has its own `g.skew`. A character is looked for as it is in the
  glyphset and its fallbacks, then in the other case, so a glyphset of
  capitals sets lowercase text in them. `glyphset("plain")` is made to
  read easily. `g.glyphs(text)` gives a text's glyphs,
  `g.glyph("a", #{ position, scale, rotation })` one glyph, placed,
  `g.variants("a")` all of a glyph's variants, and `glyphset_names()`
  every glyphset; `ternary_glyphset("balanced_ternary")` maps letters to
  balanced-ternary digits.
- **Text.** Every text call takes a style, a type
  (`new_type_style(glyphset, glyph_scale, #{ glyph_tracking, glyph_outline_gap, space_width, skew, italic, glyph_scale_factor, underline, strike })`)
  and a paragraph (`new_paragraph_style(#{ line_height, align, valign, layout, angle, clearance, line_start_jitter, line_gap_jitter, ... })`),
  each whole or a map of some settings. `write_line(text, at, settings)`
  writes a line from a point, `write_block(text, at, width, settings)`
  wraps text to a width and says where the next goes, `new_textbox` flows
  glyphs into a box or any shape, `canvas.scatter_notes` scatters notes
  across a canvas, and `path.fill_with_text` / `fit_text` write along a
  path, each glyph turned to it, or held at the paragraph's `angle`
  (radians, 0 upright).
- **Balanced ternary.** `balanced_ternary(2026)` (or `"+-0"` text, or an
  array of digits) is a value: `.value`, `.digits()`, and `.digits(3)` in
  chunks of three, padded in front. `glyphset.glyph(bt)` draws a small one
  as a glyph, and `path.fill_with_text(bt, #{ style, type: #{ glyph_scale } })` /
  `fit_text(bt, #{ style })` write its digits as marks along a path.
- **Keeping text clear.** Every text layout takes `avoid` (a path, canvas,
  glyph, or an array of them), and keeps each glyph's whole outline the
  paragraph's `clearance` from it (3 by default). For more control, build
  an avoidance tree: `new_avoidance_tree().add(border, #{ margin: 8 }).add_hull(drawing, #{ margin: 4 })`
  keeps 8 more from the border's lines and 4 more from anywhere inside the
  drawing's outline; `add_region(path)` keeps out of a closed shape. Trees
  are shared, so `avoid.add(placed)` keeps later text off what's just been
  placed. With the paragraph's `layout: "words"`, a word that runs into
  something moves past it whole; `split_words: true` breaks it round it
  instead, for a tighter fit (or `split_words: 3`: no piece shorter than
  3 letters), and `split_padding` (in glyph scales) keeps the pieces
  further off it.
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
  parameters and a seed (`#{ seed }`; without one, it's drawn from the
  journal's generator):
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
detail: `style.rs` (brush, handwriting and stroke styles),
`text_settings.rs` (type and paragraph styles), `path.rs` (paths,
hatching, text along paths), `text.rs` (text boxes, scattered notes),
`glyphs.rs` (glyphs and glyphsets), `avoid.rs`, `layout.rs` (page
layouts), `markdown.rs` (marked-up text for layouts), `gradient.rs`, `random.rs`, `art.rs` and `locks/mod.rs`. The journals in `journals/` and `webapp/journals/` and
`tests/smoke_journals/smoke` use most of it.

## Using layouts

Text boxes and text along paths place glyphs where you say. For pages of
running text (articles, title pages, captioned plates) a layout does the
placing: it describes a page as boxes, and `typeset` pours the text into
them, column after column and page after page, until it runs out.

### Typesetting with a layout file

The layouts in `webapp/library/layouts/` are ready to use:

| layout | flows | art |
|---|---|---|
| `two_column_article` | `title`, `body`, `caption` | `divider` (a band's motif), `illustration` (optional) |
| `bordered_title` | `title`, `subtitle`, `credits` | `border`, `corner`, `flourish` |
| `captioned_figure` | `caption` | `figure` |

Give one the text (**flows**), how each kind of text and stroke looks
(**roles**) and the drawings (**art**), and add the pages it returns:

```rhai
let ink = new_style(new_brush_style("rgba(30, 20, 20, 0.9)", "Ink Pen", 3.0, #{ min_pressure: 0.4, max_pressure: 0.9 }), new_handwriting_style());
let red = ink;
red.color = "darkred";
let plain = glyphset("plain");
let caps = glyphset("loche_upper");

journal_entry.add(layout("two_column_article").typeset(#{
	name: "lunar",                   // pages lunar_0, lunar_1, ...
	flows: #{
		title: "Lunar Influence",
		body: "First paragraph...\n\nSecond paragraph...",
		caption: "the moon at its third quarter",
	},
	roles: #{
		body: #{ style: ink, type: new_type_style(plain, 24.0), paragraph: #{ align: "justify", indent: 1.5 } },
		heading: #{ style: red, type: new_type_style(caps, 80.0) },
		caption: #{ style: red, type: new_type_style(plain, 20.0), paragraph: #{ align: "center" } },
		initial: #{ style: red, type: #{ glyphset: caps } },
		folio: #{ style: red, type: new_type_style(plain, 20.0) },
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
  `[text]{settings}` and named spans for colour, size, lean, passes,
  spacing, and lines under and through, which break where the text does (see
  [webapp/library/layouts/README.md](webapp/library/layouts/README.md#marked-up-text)).
- **Roles** are `#{ style, type, paragraph, spans }`, by name. Text roles
  need a type with a glyphset and glyph scale; the style defaults to
  `default_style()`, the paragraph (`line_height`, `align`, `valign`,
  `indent`, `paragraph_gap`, `clearance`, `split_words`, `fit`, ...) to
  its defaults, and `spans` say how markdown's emphasis, strong text and
  named spans look. Art, rules and bands need only a `style`. Each layout file's
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
		if n == 0 { flow("title", #{ role: "heading", flex: #{ height: 80 }, paragraph: #{ fit: true, align: "center" } }) } else { spacer() },
		row([
			flow("body", #{ flex: #{ grow: 1 } }),
			rule(#{ flex: #{ width: 4 } }),
			flow("body", #{ flex: #{ grow: 1 } }),
		], #{ flex: #{ grow: 1, gap: 40 } }),
		label(`${n + 1}`, #{ role: "folio", flex: #{ height: 30 }, paragraph: #{ align: "center" } }),
	], #{ flex: #{ padding: 60, gap: 30 } })
}
let pages = new_layout("two_up", Fn("two_up")).typeset(#{ width: 1200, height: 1600, flows: flows, roles: roles });
```

To reuse it, put `page(n, info)` (and, optionally, `options()`,
`page_size(o)` and `example()`) in a journal's `layouts/<name>.rhai`, or
the library's, and load it with `layout("<name>")` or `layout("<name>",
options)`; no rebuild is needed. Every box takes its flex style, `#{ flex }` (a `new_flex_style`
or a map): rows and columns lay out their children with flexbox rules,
`grow` shares out the space left, `width`/`height` take units or
`"40%"`, and `gap`, `padding`, `margin`, `justify` and `align` work as in
CSS. The leaves are `flow`, `label`, `initial` (a drop cap), `figure`,
`band` (art repeated as an ornamental strip), `rule` and `spacer`. Any
slot can override its role's style, type, paragraph or spans:
`flow("body", #{ flex: #{ grow: 1 }, paragraph: #{ align: "center" } })`. `info.flows` says how much of each flow is left, so a layout
can change as the text runs out (a closing page, say), and `info.art`
which art it was given.

### Wrapping text round art

A box placed with `position: "absolute"` is taken out of the flow, and
text in its parent wraps round it: round its box (`text_wrap: "box"`,
the default), round the outline of what's drawn in it
(`text_wrap: "hull"`), or not at all (`text_wrap: "none"`).

```rhai
column([
	figure("sigil", #{ flex: #{ position: "absolute", top: 200, left: 150, width: 300, height: 300, text_wrap: "hull" } }),
	flow("body", #{ flex: #{ grow: 1 }, paragraph: #{ clearance: 12, split_words: true, split_padding: 0.4 } }),
], #{ flex: #{ grow: 1 } })
```

The paragraph's `clearance` is how far text keeps from what it wraps round. By default a
word that doesn't fit before it goes on after it, whole; `split_words`
breaks it round the figure instead, so lines hug the outline more
tightly, and `split_padding` (in glyph scales) adds room at the breaks. An
`initial` placed the same way is a drop cap the text wraps round.

[webapp/library/layouts/README.md](webapp/library/layouts/README.md) has every box, flex setting and role
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

Each lock type is a rhai file in `locks/`, the library's or a journal's,
so adding one doesn't need a rebuild.
[webapp/library/locks/README.md](webapp/library/locks/README.md) describes
what a lock file defines; `journalbuild --check-locks` tests them.

## Tests

```bash
./scripts/smoke.sh
```

`smoke.sh` builds in release mode and runs `cargo test` (which also
evaluates every entry of every journal and every lock file's examples). It
then runs `journalbuild --check-locks` for the library and each journal
with lock or chain files of its own, renders the smoke fixture in full,
and renders page 0 of each entry of each journal at quarter scale. Last, it checks that the web
editor's renderer still builds for the browser and, if the editor's npm
packages are installed and its wasm built, runs its unit tests and builds
it. Renders go to a temporary directory; `--keep` keeps them.

Where there's no GPU (a cloud sandbox, CI), `smoke.sh` skips the Darkly
renders, but what journals draw is still checked: `cargo test` renders
the smoke fixture, every page of the showcase and page 0 of every entry
of every other journal as SVG, rasterised on the CPU, and compares each
canvas with its image in `tests/svg_renders/<journal>/<entry>/`
(`tests/svg_renders.rs`). Small differences
between machines pass; moved, missing or restyled strokes don't. A
failure writes the new render and a diff (changed pixels in red) to
`target/svg-render-diffs/`. When a change means to alter what's drawn,
look at those, then update the images and commit them with it:

```bash
UPDATE_SVG_RENDERS=1 cargo test --release --test svg_renders
```
