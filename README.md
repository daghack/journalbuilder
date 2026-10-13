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

Renders go to `journals/journal_entry_17/output/`. Rendering needs a GPU
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
- `--journals-dir`, `--output-dir`, `--fonts`, `--locks-dir`,
  `--layouts-dir`: read or write somewhere other than the repository's
  defaults.
- `--check-locks`: run every lock file's examples instead of building a
  journal.

## Layout

| path | what's there |
|---|---|
| `journals/<name>/journal_entry.rhai` | one journal per directory |
| `journals/showcase` | a page each of every brush, font, glyph, art tool, text layout and lock type, with the calls that make them |
| `locks/` | lock types, one rhai file each; see [locks/README.md](locks/README.md) |
| `layouts/` | page layouts, one rhai file each; see [layouts/README.md](layouts/README.md) |
| `config/fonts.json` | every font: its glyphs' SVG paths by character (a list for variants), `character_padding`, `space_width` and `fallback`; sets of symbols are fonts too |
| `src/journal/` | canvases, paths, glyph placement, fonts, hatching, gradients, parchment, and Darkly and SVG rendering |
| `src/journal/publish.rs` | writing an entry out: images per canvas or page, PDFs, SVGs |
| `src/journal/art/` | procedural path art: snakes and brambles, free or along a path |
| `src/journal/rhaientries/` | the rhai API journals and lock files use |
| `src/lockbuilder/` | runs lock chains forward (lock) and backward (unlock) |
| `src/bin/journalbuild/` | the command line |
| `tests/`, `scripts/smoke.sh` | tests, below |
| `scripts/plain_font.py` | draws the plain font into `config/fonts.json` |
| `assets/svgsets/` | source artwork for symbol sets (not read by the build) |

## Writing a journal

A journal script fills in `journal_entry` with pages, each holding one or
more canvases of strokes:

```rhai
let brush = new_brush("Calligraphy", 0.15, 0.25, 2.0);
let ink = new_style(2.5, "rgba(0, 0, 0, 0.8)", brush);

let canvas = new_canvas();
canvas.add_path(new_circle(new_vec(0.0, 0.0), 100.0), ink);

let font = font("copper");
for (g, i) in font.glyphs("ink") {
	g.scale = 50.0;
	g.position = new_vec(i * 50.0 - 50.0, 0.0);
	canvas.add_glyph(g, ink);
}

let page = new_page();
page.name = "hello";
page.add_canvas(canvas);
journal_entry.add_page(page);
```

This renders `darkly_hello_0.png`: one image per canvas, named
`darkly_{page}_{canvas}.png` after the page's and canvas's names, or their
indices. With `--svg` there's also `hello_0.svg` and `hello_0.png`.

What to know:

- **Units.** Coordinates are canvas units, one pixel at `--scale 1`. Each
  canvas is cropped to its strokes, plus `page.margin` (by default half the
  largest brush size).
- **Numbers and vectors.** Numbers can be whole or decimal anywhere
  (`new_circle(new_vec(0, 0), 20)`). Vecs add, subtract, negate, and
  multiply or divide by numbers (`(a + b) / 2`), and have `length()`,
  `distance(other)`, `dot(other)`, `angle()`, `normalized()` and
  `rotated(angle)`. `path.bounds()` and `canvas.bounds()` give
  `#{ min, max, center, width, height }`.
- **Brushes and widths.** `new_brush(name, min_pressure, max_pressure,
  size)` picks a Darkly brush; `size` is how wide its marks are. In
  `new_style(width, color, brush)`, `width` scales the brush's
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
- **Going over strokes.** `style.passes = 7` draws each stroke in the
  style seven times over, and `style.jitter = 9` lets each pass's ends
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
- **Fonts and glyphs.** Fonts come from `config/fonts.json`, each with
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
  `f.glyph("a")` one glyph, `f.variants("a")` all of a glyph's variants,
  and `font_names()` every font; `ternary_font("balanced_ternary")` maps
  letters to balanced-ternary digits. Text layout: `new_textbox` flows
  glyphs into a box, `canvas.scatter_notes` scatters notes across a
  canvas, and `path.fill_with_text` / `fit_text` write along a path,
  each glyph turned to it, or held at `angle` (radians, 0 upright) with
  `#{ angle: 0 }`.
- **Keeping text clear.** Every text layout takes `avoid` (a path, canvas,
  glyph, or an array of them) and `clearance`, and keeps each glyph's
  whole outline that far from it. For more control, build an avoidance
  tree: `new_avoidance_tree().add(border, 8.0).add_hull(drawing, 4.0)`
  keeps 8 from the border's lines and 4 from anywhere inside the drawing's
  outline; `add_region(path)` keeps out of a closed shape. Trees are
  shared, so `avoid.add(placed)` keeps later text off what's just been
  placed. With `layout: "words"`, a word that runs into something moves
  past it whole; `split_words: true` breaks it round it instead, for a
  tighter fit (or `split_words: 3`: no piece shorter than 3 letters), and
  `split_padding` (in symbol scales) keeps the pieces further off it.
- **Hatching.** `path.hatch(spacing, angle, roughness)` shades a closed
  path's inside with lines, each a stroke of its own; shapes inside or
  across others in the same path (`ring.append(hole)`) cut holes in it.
  `path.hatch(#{ spacing, angle: [a, b], roughness, bow, inset,
  fill_rule })` cross-hatches, bends lines, keeps them off the outline, and
  picks how overlaps fill.
- **Procedural art.** The `art` module draws shapes from a few
  parameters and a seed: `art::snake_path(head_center, head_angle,
  tail_end, snake_length, max_width, random_seed)` is the outline of a
  snake seen from above, winding from its head to the tip of its tail and
  passing over and under itself. `art::bramble(base, angle, length,
  flower_size, random_seed)` grows a blackberry bramble and gives its
  `vines`, `leaves` and `flowers` as separate paths, to stroke in
  different colours; `art::bramble_along(path, flower_size, random_seed)`
  grows one along a path, such as a maze's, and `art::snake_along(path,
  max_width, random_seed)` makes a many-headed serpent of one.
- **Page sizes.** A page is as big as its strokes, plus `page.margin`;
  with `page.size = new_vec(1200, 1600)` it's exactly that, from the
  origin, and strokes past its edges are cut off. The showcase is a book
  of such pages.
- **Splitting pages.** `page.split(columns, rows)` cuts a named page into a
  grid of tiles, each a page of its own named `{name}_{row}_{column}`
  (`page.split(#{ tile_width: 1200, tile_height: 1600 })` makes tiles of a
  set size instead). Each tile renders exactly its share of the pixels of
  the whole page's render, with strokes that cross a seam drawn whole and
  cut off at it, so a canvas's tile images placed side by side in order
  make up that render; with ImageMagick, for a 2 x 2 split:

  ```bash
  magick \( darkly_big_0_0_0.png darkly_big_0_1_0.png +append \) \( darkly_big_1_0_0.png darkly_big_1_1_0.png +append \) -append +repage big.png
  ```

  `journal_entry.add_pages(page.split(2, 2))` adds them all.
- **Page layouts.** A layout lays out pages as trees of flexbox boxes, and
  `typeset` pours text, art and ornaments into their slots, chaining the
  text through columns and pages until it runs out; see
  [Using layouts](#using-layouts) below.
- **Problems.** `warn("...")` reports something without stopping the
  script. Errors give the script line.

`journals/showcase` shows everything there is to draw with, each example
labelled with the calls that make it: render it with
`target/release/journalbuild showcase`.

The module docs in `src/journal/rhaientries/` cover each area in more
detail: `style.rs` (brushes and styles, passes and jitter), `path.rs`
(paths, hatching, text along paths), `text.rs` (text boxes, scattered
notes), `glyphs.rs` (glyphs and fonts), `avoid.rs`, `layout.rs` (page
layouts), `markdown.rs` (marked-up text for layouts), `gradient.rs`, `random.rs`, `art.rs` and `locks/mod.rs`. The journals in `journals/` and
`tests/smoke_journals/smoke` use most of it.

## Using layouts

Text boxes and text along paths place glyphs where you say. For pages of
running text (articles, title pages, captioned plates) a layout does the
placing: it describes a page as boxes, and `typeset` pours the text into
them, column after column and page after page, until it runs out.

### Typesetting with a layout file

The layouts in `layouts/` are ready to use:

| layout | flows | art |
|---|---|---|
| `two_column_article` | `title`, `body`, `caption` | `divider` (a band's motif), `illustration` (optional) |
| `bordered_title` | `title`, `subtitle`, `credits` | `border`, `corner`, `flourish` |
| `captioned_figure` | `caption` | `figure` |

Give one the text (**flows**), how each kind of text and stroke looks
(**roles**) and the drawings (**art**), and add the pages it returns:

```rhai
let ink = new_style(1.0, "rgba(30, 20, 20, 0.9)", new_brush("Ink Pen", 0.4, 0.9, 3.0));
let red = new_style(1.0, "darkred", new_brush("Ink Pen", 0.4, 0.9, 3.0));

journal_entry.add_pages(layout("two_column_article").typeset(#{
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
	art: #{ divider: new_circle(new_vec(0, 0), 10), illustration: moon_canvas },
}));
```

- **Flows** are strings. `"\n"` breaks a line and a blank line starts a
  paragraph. A flow that doesn't fit its box carries on in the next box
  of the same name, then onto the next page. A flow can also be
  `markdown(text)`: `*emphasis*`, `**strong**`, `~~struck out~~`, and
  `[text]{settings}` for colour, size, lean, passes, spacing, and lines
  under and through, which break where the text does (see
  [layouts/README.md](layouts/README.md#marked-up-text)).
- **Roles** are maps of settings, by name. Text roles need `font`,
  `style` and `size` (the symbol scale, in canvas units); the rest are
  optional: `line_height`, `align`, `valign`, `indent`, `paragraph_gap`,
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
	column(#{ padding: 60, gap: 30 }, [
		if n == 0 { flow("title", #{ role: "heading", height: 80, fit: true, align: "center" }) } else { spacer() },
		row(#{ grow: 1, gap: 40 }, [
			flow("body", #{ grow: 1 }),
			rule(#{ width: 4 }),
			flow("body", #{ grow: 1 }),
		]),
		label(`${n + 1}`, #{ role: "folio", height: 30, align: "center" }),
	])
}
let pages = new_layout("two_up", Fn("two_up")).typeset(#{ width: 1200, height: 1600, flows: flows, roles: roles });
```

To reuse it, put `page(n, info)` (and, optionally, `page_size()`) in
`layouts/<name>.rhai` and load it with `layout("<name>")`; no rebuild is
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
column(#{ grow: 1 }, [
	figure("sigil", #{ position: "absolute", top: 200, left: 150, width: 300, height: 300, wrap: "hull" }),
	flow("body", #{ grow: 1, clearance: 12, split_words: true, split_padding: 0.4 }),
])
```

`clearance` is how far text keeps from what it wraps round. By default a
word that doesn't fit before it goes on after it, whole; `split_words`
breaks it round the figure instead, so lines hug the outline more
tightly, and `split_padding` (in sizes) adds room at the breaks. An
`initial` placed the same way is a drop cap the text wraps round.

[layouts/README.md](layouts/README.md) has every box, style key and role
setting; `journals/showcase` uses each layout, and an inline one.

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

Each lock type is a rhai file in `locks/`, so adding one doesn't need a
rebuild. [locks/README.md](locks/README.md) describes what a lock file
defines; `journalbuild --check-locks` tests them.

## Tests

```bash
./scripts/smoke.sh
```

`smoke.sh` builds in release mode and runs `cargo test` (which also
evaluates every journal script and every lock file's examples). It then runs
`journalbuild --check-locks`, renders the smoke fixture in full, and renders
page 0 of each journal at quarter scale. Renders go to a temporary directory;
`--keep` keeps them.
