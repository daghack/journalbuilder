# Journal editor

A browser editor for journal scripts, and for the fonts and lock types they
draw with. Tabs along the top switch between the three editors (the address
keeps which: `#fonts`, `#locks`); each stays as you left it while another
shows.

## Journals

The code is on the right, a thumbnail of each page on the left, and the
page you pick in the middle. Pages redraw as you type.

- **Preview.** By default each page is drawn as SVG, from the same strokes
  `journalbuild --svg` draws. Switch the preview to **Darkly** to render the
  page with Darkly's brushes on the GPU (WebGPU), at a scale you pick and
  optionally on parchment. Darkly re-renders what's shown as it changes.
- **Editor.** Rhai is highlighted, and the functions journals add are
  marked and completed. The last run's error is marked on its line, with
  `warn()` warnings on theirs. The panel under the editor lists them, with
  missing glyphs and anything the script `print`s.
- **Files.** Open and save `.rhai` files (Ctrl/⌘ O, Ctrl/⌘ S, Ctrl/⌘
  Shift S). In Chromium browsers, Save writes back to the file you opened;
  elsewhere it downloads. The Examples menu opens the repository's
  journals. Your work is kept in the browser between visits.
- **Export.** One Export button opens a dialog: export as PDF (as
  `journalbuild --pdf` makes it) or as a zip of PNGs, one per page, rendered
  with Darkly, with a progress bar. The dialog sets the scale, supersample,
  parchment, and (for PNGs) `stitch`, as journalbuild's flags do. Cancel
  stops it.

Darkly previews and exports need WebGPU: a current Chrome, Edge or Safari.
The SVG preview works anywhere.

## Fonts

The whole of the library's `glyphsets/` (a file per glyphset), one font at a time: its glyphs on the
left, each with its variants; the variant you pick in the middle, to draw;
the font's options on the right.

- **Drawing.** Glyphs are pen strokes, y down, roughly −1 to 1, with the
  font's cap height, x-height, baseline and descender worked out from its
  glyphs as guides. Select picks points (anchors square, controls round;
  a stroke's start is filled), curves, or what you drag a box around;
  drag them, or nudge with the arrow keys (Shift for bigger steps).
  Double-click a curve to add a point. The tools add a new stroke, a
  line, quadratic or cubic curve to the stroke picked, a dot, or an
  ellipse (drag from its centre; Shift for a circle). Picked curves can be
  made lines, quadratics or cubics, or split; strokes closed or reversed.
  Snap keeps points on a 0.05 grid (hold Alt not to). The variant's path
  data is in the panel to edit by hand too. Ctrl/⌘ Z undoes.
- **Preview.** Under the glyph, type text to see it set in the font exactly
  as a journal sets it (`fill_with_text`): spacing, fallbacks and variant
  picks included. Reroll picks other variants.
- **Fonts and options.** Pick, add, copy, rename or delete fonts; set
  `character_padding`, `space_width` and `fallback`. What fonts.rs would
  reject is listed, and the journals keep the last good fonts until it's
  fixed.
- **Export** writes the whole fonts.json in the layout it is kept in,
  one glyph to a line, so a glyph changed is a line changed in the diff.
  Split it into `library/glyphsets/` with `npm run split-fonts -- exported.json`. A new font of 20 glyphs or more needs a
  line in the showcase.

## Locks

Chains, built as graphs: the chains (every `library/chains/*.rhai`, and this
browser's), lock types (every `library/locks/*.rhai`) and illustrations
(every `library/illustrations/*.rhai`) on the left, the chain in the middle, and on the right
its name, the picked lock's or illustration's options, and the chain's inputs
and outputs with a test case. A journal uses a chain by name:

```rhai
let c = chain("my_chain_1");
let result = c.run(#{ "ml.maze_text": symbols(27, "HELLO"), "ml.random_seed": symbols(10, "4") });
let pages = result.canvases("mg");
```

- **Live.** As you edit, the chain is a chain file in this browser under
  its name, so a journal using `chain("name")` follows along, as one using
  an edited font does, mistakes and all. Click a chain on the left to edit
  it (a compiled-in one too: the browser's copy then stands in for it until
  removed with ×); New starts another, and the last one stays. Renamed, a
  chain moves.
- **Building.** Drag a lock type into the middle (or click it). Drag from
  an output to an input to wire them: only ports of the same type (or
  `Any`) take a wire, an input takes one wire, and nothing can feed back
  into itself. One output can feed several inputs. A lock's ports follow
  its options (`merge_alpha`'s `input_count`, say).
- **Ports.** The inputs and outputs left unwired are the chain's, named
  as its locks name them, `lock.port`. Fix an input to give it its value
  in the chain itself. "Chain's" makes a lock's or illustration's option one
  of the chain's own, which a journal gives as `chain("name", #{...})`.
- **Testing.** Give each input a test value and the chain runs as you
  edit: its outputs, the values along each wire, and whether unlocking
  gives the inputs back. Symbols take text. A value kind (a HuffmanCode, a
  Maze) takes JSON, or the rhai that makes it, run as lock files run it;
  the box suggests one, e.g. `huffman::code(symbols(27, "THE QUICK BROWN
  FOX"), 27, 3)`. A value kind with a sketch, such as a StarChart from
  `star_chart`, is drawn under its output. The test case is the chain
  file's `example()`.
- **Illustrations.** An illustration draws the chain's values as canvases: a star
  chart's pages, a maze with its glyphs. Drag one in (or click it) and wire
  a value to its inputs; the chain itself is left as it was. Pick it to set
  its options and see what it draws of the test run. New writes an illustration
  file in the browser and Edit opens one; the middle becomes its editor,
  previewing its `example()` as you type, and it's usable as it draws (see
  library/illustrations/README.md). A journal's `result.canvases("id")` is what it draws.
- **Saving** writes the chain file. To add it to the repository, put it in
  `library/chains/` (see library/chains/README.md), use it in the showcase, and rebuild
  the renderer. Open… reads a chain file back. The snippets copy journal
  script: `chain("name")` in use, or the chain built lock by lock with
  `new_lock`.

## The workspace

Fonts edited, and chains, illustrations and lock types made here, are the
workspace: every script, in every editor, runs with them (the diagnostics
say when), and Darkly renders and exports do too. They're kept in this
browser only until exported into the repository; the tabs show what's in
use, and Clear workspace, beside them, throws it all away (and the
editors' drafts) to use the repository's files again. If the files in
library/glyphsets/ change after edits began, the font editor says so.

## Running it

You need Rust with the `wasm32-unknown-unknown` target, [wasm-pack], and
Node.js 24 (`webapp/frontend/.nvmrc`; with nvm, `nvm use` there).

```bash
rustup target add wasm32-unknown-unknown
```

```bash
cd webapp/frontend && npm install
```

```bash
cd webapp/frontend && npm start
```

`npm start` builds the renderer (`npm run wasm`, a release build that takes
a few minutes the first time) and serves the editor at
http://localhost:4200. After changing only the frontend, `npm run serve`
skips the wasm build. `npm run build` puts a static site in
`webapp/frontend/dist/journal-editor/browser`, which any static file
server can host. `npm test` runs the frontend's unit tests.

[wasm-pack]: https://rustwasm.github.io/wasm-pack/installer/

### In Docker

`webapp/Dockerfile` builds the renderer and the site and serves them with
nginx; it needs only Docker. Build from the repository root:

```bash
docker build -f webapp/Dockerfile -t journal-editor .
```

```bash
docker run --rm -p 8080:80 journal-editor
```

Then open http://localhost:8080. Browsers offer WebGPU (for Darkly) only on
localhost or over HTTPS, so anywhere else, serve it behind TLS. The first
build compiles the whole crate for wasm and takes a while; later builds
reuse BuildKit's caches.

## How it fits together

| path | what's there |
|---|---|
| `wasm/` | `scribe-wasm`, the renderer for the browser: the `scribe` library with the built-in library (`library/`) compiled in (its `embedded-assets` feature). `evaluate(source)` runs a script and lists its pages, `pageSvg(i)` draws one as SVG, and `Renderer` renders pages with Darkly (`renderPage`, `exportPdf`). `setWorkspace(fonts, locks, renderers, chains)` has scripts run with edited fonts and extra lock, renderer and chain files. For the font and lock editors: `previewSvg`, `builtinFonts`, `lockTypes`, `lockPorts`, `chainTypes`, `runChain`, `readChain`, `valueKinds`, `rendererTypes`, `rendererPorts`, `renderPreview` and `rendererSource`. |
| `frontend/` | the Angular app (standalone components, signals, no zone.js) |
| `frontend/src/app/workers/` | the web workers the renderer runs in. `eval.worker` runs the script as you type, sends its pages, then sends each page's SVG, the page shown first. `darkly.worker` renders with Darkly. `tools.worker` answers the font and lock editors. Typing never waits on any of them. |
| `frontend/src/app/services/` | `JournalService` (the script, its pages, the file), `RenderService` (the script and Darkly workers), `ToolsService` (the tools worker), `WorkspaceService` (edited fonts, and lock, renderer and chain files), `FileService` (opening, saving, downloads) |
| `frontend/src/app/shell/`, `layout/` | the tabs, and the three resizable columns every editor has |
| `frontend/src/app/fonts/` | the font editor. `font-file.ts` reads and writes fonts.json; `glyph-path.ts` reads and writes path data; `glyph-edit.ts` is what the canvas does to a path. |
| `frontend/src/app/locks/` | the lock editor. `chain-model.ts` is the graph and its rules; `chain-source.ts` writes it as a chain file; `renderer-editor.component.ts` edits renderer files. |
| `frontend/src/app/editor/` | the CodeMirror editor and its rhai highlighting (`rhai-language.ts`) |
| `frontend/scripts/examples.mjs` | copies every entry of every journal, `journals/*/entries/*.rhai`, into `generated/examples/` for the Examples menu, and the repository files the unit tests check against into `generated/fixtures/` |

While one script runs, only the newest edit waits to run next. A script
that runs for more than 20 seconds (an endless loop, say) is stopped and
its worker restarted. A script with an error leaves the last good pages
showing.

The renderer is about 17 MB (9 MB gzipped), mostly Darkly's built-in
textures and fonts.
