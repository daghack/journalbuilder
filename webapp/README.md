# Journal editor

A browser editor for journal scripts. The code is on the right, a
thumbnail of each page on the left, and the page you pick in the middle.
Pages redraw as you type.

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

## How it fits together

| path | what's there |
|---|---|
| `wasm/` | `scribe-wasm`, the renderer for the browser: the `scribe` library with the layout and lock files and fonts compiled in (its `embedded-assets` feature). `evaluate(source)` runs a script and lists its pages, `pageSvg(i)` draws one as SVG, and `Renderer` renders pages with Darkly (`renderPage`, `exportPdf`). |
| `frontend/` | the Angular app (standalone components, signals, no zone.js) |
| `frontend/src/app/workers/` | the two web workers the renderer runs in. `eval.worker` runs the script as you type, sends its pages, then sends each page's SVG, the page shown first. `darkly.worker` renders with Darkly. Typing never waits on either. |
| `frontend/src/app/services/` | `JournalService` (the script, its pages, the file), `RenderService` (the workers), `FileService` (opening, saving, downloads) |
| `frontend/src/app/editor/` | the CodeMirror editor and its rhai highlighting (`rhai-language.ts`) |
| `frontend/scripts/examples.mjs` | copies `journals/*/journal_entry.rhai` into `generated/examples/` for the Examples menu |

While one script runs, only the newest edit waits to run next. A script
that runs for more than 20 seconds (an endless loop, say) is stopped and
its worker restarted. A script with an error leaves the last good pages
showing.

The renderer is about 17 MB (9 MB gzipped), mostly Darkly's built-in
textures and fonts.
