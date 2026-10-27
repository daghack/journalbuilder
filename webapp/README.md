# Journal editor

A browser editor for a journal: its entries, and the files they draw with,
over the built-in library. One journal at a time: the bar along the top
names it and starts a new one, opens one of those built in, imports a
journal (zipped, or its directory) or exports this one. The sidebar lists its files, kind by
kind, each kind with the library's files beside the journal's; picking one
opens it in the editor for its kind. The address keeps the file open
(`#entries/letter_1.rhai`).

## The journal

- **New** starts a journal of one blank entry, with the library. **Open**
  loads one of the two journals built in, the showcase and the API
  reference (`webapp/journals/`).
- **Import** loads a journal in place of this one: a zipped journal, or
  a journal's directory as journalbuild reads it (one of the repository's
  `journals/`, say). **Export** downloads
  this one as a zip, with the library files its entries use bundled in and
  listed in its `journal.json`, so it renders the same wherever the
  library has changed; importing drops them again. Nothing is merged:
  New, Open and Import ask first if the journal has changes not exported.
- The journal is kept in this browser between visits.
- **Library files.** Any of the library's files opens, to read. Changing
  one gives the journal a copy, marked ⚠, which stands in for it; the
  banner over its editor removes the copy, to use the library's again.

## Entries

The code is on the right, a thumbnail of each page on the left, and the
page you pick in the middle. Pages redraw as you type, and when any of the
journal's other files changes.

- **Preview.** By default each page is drawn as SVG, from the same strokes
  `journalbuild --svg` draws. Switch the preview to **Darkly** to render the
  page with Darkly's brushes on the GPU (WebGPU), at a scale you pick and
  optionally on parchment. Darkly re-renders what's shown as it changes.
- **Editor.** Rhai is highlighted, and the functions journals add are
  marked and completed. The last run's error is marked on its line, with
  `warn()` warnings on theirs. The panel under the editor lists them, with
  missing glyphs and anything the script `print`s.
- **Export.** One Export button opens a dialog: export as PDF (as
  `journalbuild --pdf` makes it) or as a zip of PNGs, one per page, rendered
  with Darkly, with a progress bar. The dialog sets the scale, supersample,
  parchment, and `stitch`, as journalbuild's flags do: PNGs get stitched
  images besides the pages, and a PDF's pages are the stitches. Cancel
  stops it.

Darkly previews and exports need WebGPU: a current Chrome, Edge or Safari.
The SVG preview works anywhere.

## Glyphsets

Every glyphset the journal has, its own files over the library's, one at a
time: its glyphs on the left, each with its variants; the variant you pick
in the middle, to draw; the glyphset's options on the right. Each glyphset
that differs from the library's is a file of the journal's,
`glyphsets/NAME.json`, written as you edit.

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
- **Glyphsets and options.** Pick, add, duplicate, rename or delete the
  journal's glyphsets (the library's can't be renamed or deleted, only
  copied); set `character_padding`, `space_width` and `fallback`. What
  fonts.rs would reject is listed, and the journal keeps its last good
  glyphsets until it's fixed. Files are written one glyph to a line, so a
  glyph changed is a line changed in a diff.

## Chains

A chain file, built as a graph: the chains (the journal's and the
library's), the sources a chain can draw on, the lock types and the
illustrations on the left; the chain in the middle; and on the right its
name, the picked node's settings, and the chain's inputs and outputs with
a test case. An entry uses a chain by name:

```rhai
let c = chain("my_chain_1");
let result = c.run(#{ "ml.maze_text": symbols(27, "HELLO"), "ml.random_seed": symbols(10, "4") });
let pages = result.canvases("mg");
```

- **Live.** As you edit, the chain is written into its file,
  `chains/NAME.rhai`, so entries using `chain("name")` follow along,
  mistakes and all. Click a chain on the left to edit it; a library chain
  changed gives the journal a copy that stands in for it. Renamed, a
  chain's file moves.
- **Building.** Drag a lock type into the middle (or click it). Drag from
  an output to an input to wire them: only ports of the same type (or
  `Any`) take a wire, an input takes one wire, and nothing can feed back
  into itself. One output can feed several inputs: hold Shift as you wire
  it and the wire stays in hand, to click on to the next input. A lock's
  ports follow its options (`merge_alpha`'s `input_count`, say). Lock
  types (ciphers and puzzles) and helper types (`helpers/`, which
  transform data between them) are listed apart. Ctrl/⌘ C copies the
  picked node, options, fixed values and all; Ctrl/⌘ V pastes it where
  the pointer is.
- **Sources.** The journal's files a chain draws on: content, styles,
  glyphsets and layouts, each a node with one output, `value` (see
  library/chains/README.md). Content feeds locks (text as symbols in a base
  set on it, or as text) and illustrations; styles, glyphsets and layouts
  only illustrations. Pick one to change its file, base or options.
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
- **Illustrations.** An illustration draws the chain's values as canvases:
  a star chart's pages, a maze with its glyphs. Drag one in (or click it)
  and wire a value to its inputs; the chain itself is left as it was. Pick
  it to set its options and see what it draws of the test run. New and
  Open open an illustration file in its own editor.
- **⚠** marks a node whose file is the journal's, standing in for the
  library's of the same name. It's a note, not an error.
- The snippets copy entry script: `chain("name")` in use, or the chain
  built node by node with `new_lock`.

## Locks, helpers, illustrations, layouts, styles and lib

Each opens in three panes: on the left the options its file declares, to
give values to preview with; in the middle the preview (a lock's examples,
run, and its ports; what an illustration draws of its `example()`; a
layout typesetting its `example()`; a style writing its example text); on
the right its code. Helper types run their examples as locks do. Lib
files (`lib/`), shared functions, have only their code.

## Content

New content asks for its name and its type: String, Markdown or Symbols
(kept as `.txt`, `.md` and `.json`; nobody types the extension). Strings
and Markdown are typed beside a page writing them in the library's plain
hand. Symbols are typed as text, kept as typed, and made to fit their
base (picked beside them) as they're read: in capitals, what the base
can't write dropped. Strict, the text must be written in the base
already. Nobody edits the JSON a symbols file is kept as.

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

`npm start` builds the engine (`npm run wasm`, a release build that takes
a few minutes the first time) and serves the editor at
http://localhost:4200. After changing only the frontend, `npm run serve`
skips the wasm build. `npm run build` puts a static site in
`webapp/frontend/dist/journal-editor/browser`, which any static file
server can host. `npm test` runs the frontend's unit tests.

[wasm-pack]: https://rustwasm.github.io/wasm-pack/installer/

### In Docker

The `Dockerfile` at the repository root builds the engine and the site and
serves them with nginx; it needs only Docker. Build from the repository
root:

```bash
docker build -t journal-editor .
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
| `wasm/` | `scribe-wasm`, the engine for the browser: the `scribe` library with the built-in library (`library/`) compiled in (its `embedded-assets` feature). `setJournal(files)` gives it the journal's own files, by path, in front of the library's. `evaluate(source)` runs an entry and lists its pages, `pageSvg(i)` draws one as SVG, and `Renderer` renders pages with Darkly (`renderPage`, `exportPdf`). For the other editors: `previewSvg`, `libraryFiles`, `usedLibrary` (what an export bundles), `lockTypes`, `lockPorts`, `lockExamples`, `chainTypes`, `runChain`, `readChain`, `valueKinds`, `illustrationTypes`, `illustrationPorts`, `illustrationPreview`, `layoutTypes`, `layoutPreview`, `styleTypes` and `stylePreview`. |
| `frontend/` | the Angular app (standalone components, signals, no zone.js) |
| `frontend/src/app/journal/` | the journal as files by path (`journal-files.ts`: kinds, the manifest, zips; `journal-folder.ts`: journals' directories), new files' templates, symbols content, and the entry editor |
| `frontend/src/app/workers/` | the web workers the engine runs in. `eval.worker` runs the entry as you type, sends its pages, then sends each page's SVG, the page shown first. `darkly.worker` renders with Darkly. `tools.worker` answers the other editors. Typing never waits on any of them. |
| `frontend/src/app/services/` | `JournalStore` (the journal, kept in IndexedDB), `JournalIoService` (open, import, export), `LibraryService` (the library's files), `JournalService` (the entry shown, its pages), `RenderService` (the script and Darkly workers), `ToolsService` (the tools worker), `FileService` (picking files and directories, downloads) |
| `frontend/src/app/shell/`, `layout/` | the journal bar, the files sidebar, which file shows, and the three resizable columns the editors have |
| `frontend/src/app/fonts/` | the glyphset editor. `font-file.ts` reads and writes glyphset files; `glyph-path.ts` reads and writes path data; `glyph-edit.ts` is what the canvas does to a path. |
| `frontend/src/app/locks/` | the chain editor. `chain-model.ts` is the graph and its rules; `chain-source.ts` writes it as a chain file. |
| `frontend/src/app/resources/` | the lock, illustration, layout, style and helper editor, the content editor, and the library banner |
| `frontend/src/app/editor/` | the CodeMirror editor and its rhai highlighting (`rhai-language.ts`) |
| `frontend/scripts/examples.mjs` | copies the built-in journals, `showcase` and `api_reference` (their manifests and files), into `generated/journals/` for the Open menu, and the repository files the unit tests check against into `generated/fixtures/` |

While one script runs, only the newest edit waits to run next. A script
that runs for more than 20 seconds (an endless loop, say) is stopped and
its worker restarted. A script with an error leaves the last good pages
showing.

The engine is about 17 MB (9 MB gzipped), mostly Darkly's built-in
textures and fonts.
