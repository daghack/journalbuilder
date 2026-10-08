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
that wgpu can use (Metal on macOS).

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
- **Gradients.** In place of a colour, `new_gradient(["red", "gold",
  "blue"])` runs along each stroke from start to end; with `#{ from, to }`
  it's laid across the page between two points, and with `#{ center,
  radius }` out from a point. Colours are spaced evenly, or placed as
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
  characters it hasn't. `font("copper")` is a font as the file sets it;
  `font("loche_lower", #{ character_padding: 0.3 })` or
  `f.character_padding = 0.3` changes a copy. A character is looked for as
  it is in the font and its fallbacks, then in the other case, so a font of
  capitals sets lowercase text in them. `font("plain")` is a simple font
  made to read easily. `f.glyphs(text)` gives a text's glyphs,
  `f.glyph("a")` one glyph, `f.variants("a")` all of a glyph's variants,
  and `font_names()` every font; `ternary_font("balanced_ternary")` maps
  letters to balanced-ternary digits. Text layout: `new_textbox` flows
  glyphs into a box, `canvas.scatter_notes` scatters notes across a
  canvas, and `path.fill_with_text` / `fit_text` write along a path.
- **Keeping text clear.** Every text layout takes `avoid` (a path, canvas,
  glyph, or an array of them) and `clearance`, and keeps each glyph's
  whole outline that far from it. For more control, build an avoidance
  tree: `new_avoidance_tree().add(border, 8.0).add_hull(drawing, 4.0)`
  keeps 8 from the border's lines and 4 from anywhere inside the drawing's
  outline; `add_region(path)` keeps out of a closed shape. Trees are
  shared, so `avoid.add(placed)` keeps later text off what's just been
  placed.
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
  text through columns and pages until it runs out:
  `journal_entry.add_pages(layout("two_column_article").typeset(#{ flows,
  roles, art }))`. Layouts are rhai files in `layouts/`; see
  [layouts/README.md](layouts/README.md).
- **Problems.** `warn("...")` reports something without stopping the
  script. Errors give the script line.

`journals/showcase` shows everything there is to draw with, each example
labelled with the calls that make it: render it with
`target/release/journalbuild showcase`.

The module docs in `src/journal/rhaientries/` cover each area in more
detail: `path.rs` (paths, hatching, text along paths), `text.rs` (text
boxes, scattered notes), `glyphs.rs` (glyphs and fonts), `avoid.rs`,
`layout.rs` (page layouts), `gradient.rs`, `random.rs`, `art.rs` and
`locks/mod.rs`. The journals in `journals/` and
`tests/smoke_journals/smoke` use most of it.

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
