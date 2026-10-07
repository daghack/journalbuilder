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
- `--journals-dir`, `--output-dir`, `--symbols`, `--locks-dir`: read or
  write somewhere other than the repository's defaults.
- `--check-locks`: run every lock file's examples instead of building a
  journal.

## Layout

| path | what's there |
|---|---|
| `journals/<name>/journal_entry.rhai` | one journal per directory |
| `locks/` | lock types, one rhai file each; see [locks/README.md](locks/README.md) |
| `config/symbols.json` | the symbol dictionary: every glyph's SVG path, by name |
| `src/journal/` | canvases, paths, glyph placement, and Darkly and SVG rendering |
| `src/journal/art/` | procedural path art: snakes and brambles, free or along a path |
| `src/journal/rhaientries/` | the rhai API journals and lock files use |
| `src/lockbuilder/` | runs lock chains forward (lock) and backward (unlock) |
| `src/bin/journalbuild/` | the command line |
| `tests/`, `scripts/smoke.sh` | tests, below |
| `assets/svgsets/` | source artwork for symbol sets (not read by the build) |

## Writing a journal

A journal script fills in `journal_entry` with pages, each holding one or
more canvases of strokes:

```rhai
let brush = new_brush("Calligraphy", 0.15, 0.25, 2.0);
let ink = new_strokestyle(2.5, "rgba(0, 0, 0, 0.8)", brush);

let canvas = new_canvas();
canvas.add_strokepath(new_circle(new_vec(0.0, 0.0), 100.0), ink);

let font = new_fontset(["copper_upper"], ["copper_lower"]);
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
- **Brushes and widths.** `new_brush(name, min_pressure, max_pressure,
  size)` picks a Darkly brush; `size` is how wide its marks are. In
  `new_strokestyle(width, color, brush)`, `width` scales the brush's
  pressure in Darkly renders, so past `1 / max_pressure` a stroke gets no
  heavier (the render warns), but it's the stroke's geometric width in SVG.
- **Shared values.** Paths, glyphs and canvases are shared: assigning one,
  passing it or looping over it gives the same value, so changes through any
  copy reach it. `.copy()` makes an independent one.
- **Randomness is seeded.** `random()`, `random_range(a, b)`, glyph variant
  picks, text layout and hatching all draw from one generator with a fixed
  seed, so a journal renders the same every run. `seed(n)` changes it.
- **Glyphs and fonts.** `glyph("balanced_ternary_3")` loads a symbol by
  name; `new_fontset`, `ternary_font` and `lowercase_font` map text to
  glyphs. Text layout: `new_textbox` flows glyphs into a box,
  `canvas.splat_msgs` scatters messages across a canvas, and
  `path.fill_with_msg` / `fit_msg_to` write along a path.
- **Keeping text clear.** Every text layout takes `avoid` (a path, canvas,
  glyph, or an array of them) and `clearance`, and keeps each glyph's
  whole outline that far from it. For more control, build an avoidance
  tree: `new_avoidance_tree().add(border, 8.0).add_hull(drawing, 4.0)`
  keeps 8 from the border's lines and 4 from anywhere inside the drawing's
  outline; `add_region(path)` keeps out of a closed shape. Trees are
  shared, so `avoid.add(placed)` keeps later text off what's just been
  placed.
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
- **Problems.** `warn("...")` reports something without stopping the
  script. Errors give the script line.

The module docs in `src/journal/rhaientries/` (`glyphs.rs`, `layout.rs`,
`avoid.rs`, `random.rs`, `art.rs`, `locks/mod.rs`) cover each area in more
detail, and
the journals in `journals/` and `tests/smoke_journals/smoke` use most of it.

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
