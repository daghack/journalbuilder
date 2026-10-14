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
- **Export.** Export as PDF (as `journalbuild --pdf` makes it) or as a zip
  of PNGs, one per page, rendered with Darkly, with a progress bar. Cancel
  stops it.

Darkly previews and exports need WebGPU: a current Chrome, Edge or Safari.
The SVG preview works anywhere.

## Fonts

The whole of `config/fonts.json`, one font at a time: its glyphs on the
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
- **Export** writes the whole fonts.json as `scripts/plain_font.py` does,
  one glyph to a line, so a glyph changed is a line changed in the diff.
  Put it in `config/fonts.json`. A new font of 20 glyphs or more needs a
  line in the showcase.

## Locks

Lock chains, built as graphs: the lock types (every `locks/*.rhai`) on the
left, the chain in the middle, and on the right its name, the picked
lock's options, and the chain's inputs and outputs with a test case.

- **Building.** Drag a lock type into the middle (or click it). Drag from
  an output to an input to wire them: only ports of the same type (or
  `Any`) take a wire, an input takes one wire, and nothing can feed back
  into itself. One output can feed several inputs. A lock's ports follow
  its options (`merge_alpha`'s `input_count`, say).
- **Ports.** The inputs and outputs left unwired are the chain's own; name
  them as the chain should show them. Fix an input to give it its value in
  the chain itself. "Chain's" makes a lock's option one of the chain's own,
  set where the chain is used.
- **Testing.** Give each input a test value (text, for symbols) and the
  chain runs as you edit: its outputs, the values along each wire, and
  whether unlocking gives the inputs back.
- **Saving** checks the test case as the chain's example, then saves a
  composite lock file (see locks/README.md) and adds it to this browser's
  lock types: in the palette, to use in other chains, and in journals as
  `.add("x", "name", #{...})`. To add it to the repository, put the file
  in `locks/`, give it a row on the showcase's locks page, and rebuild the
  renderer. Open… reads a composite lock file back, as the palette's Open
  does one compiled in. The snippets copy journal script for the chain.

## The workspace

Fonts edited and lock types saved here are the workspace: every script,
in every editor, runs with them (the diagnostics say when), and Darkly
renders and exports do too. They're kept in this browser only until
exported into the repository; the tabs show what's in use. If
config/fonts.json changes after edits began, the font editor says so.

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
| `wasm/` | `scribe-wasm`, the renderer for the browser: the `scribe` library with the layout and lock files and fonts compiled in (its `embedded-assets` feature). `evaluate(source)` runs a script and lists its pages, `pageSvg(i)` draws one as SVG, and `Renderer` renders pages with Darkly (`renderPage`, `exportPdf`). `setWorkspace(fonts, locks)` has scripts run with edited fonts and extra lock files. For the font and lock editors: `previewSvg`, `builtinFonts`, `lockTypes`, `lockPorts`, `runChain`, `readChain` and `checkLock`. |
| `frontend/` | the Angular app (standalone components, signals, no zone.js) |
| `frontend/src/app/workers/` | the web workers the renderer runs in. `eval.worker` runs the script as you type, sends its pages, then sends each page's SVG, the page shown first. `darkly.worker` renders with Darkly. `tools.worker` answers the font and lock editors. Typing never waits on any of them. |
| `frontend/src/app/services/` | `JournalService` (the script, its pages, the file), `RenderService` (the script and Darkly workers), `ToolsService` (the tools worker), `WorkspaceService` (edited fonts and lock files), `FileService` (opening, saving, downloads) |
| `frontend/src/app/shell/`, `layout/` | the tabs, and the three resizable columns every editor has |
| `frontend/src/app/fonts/` | the font editor. `font-file.ts` reads and writes fonts.json; `glyph-path.ts` reads and writes path data; `glyph-edit.ts` is what the canvas does to a path. |
| `frontend/src/app/locks/` | the lock editor. `chain-model.ts` is the graph and its rules; `chain-source.ts` writes it as a composite lock file. |
| `frontend/src/app/editor/` | the CodeMirror editor and its rhai highlighting (`rhai-language.ts`) |
| `frontend/scripts/examples.mjs` | copies `journals/*/journal_entry.rhai` into `generated/examples/` for the Examples menu, and the repository files the unit tests check against into `generated/fixtures/` |

While one script runs, only the newest edit waits to run next. A script
that runs for more than 20 seconds (an endless loop, say) is stopped and
its worker restarted. A script with an error leaves the last good pages
showing.

The renderer is about 17 MB (9 MB gzipped), mostly Darkly's built-in
textures and fonts.
