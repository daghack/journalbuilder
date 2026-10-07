# Performance issues outside this repository

Found while profiling `journalbuild` (October 2026, macOS on Apple silicon,
`sample` on release builds with debug info). Fixes for code in this
repository went in with the profiling; what's here needs changes in
dependencies, so it's written down rather than fixed.

## Where the time goes

A full render of `journal_entry_18` (one 2,800 x 3,200 page, ~6,000 glyph
strokes, default `--supersample 1.2`) takes about 18 s. Of the main
thread's ~11,500 samples:

| | samples | share |
|---|---|---|
| Darkly reading back undo snapshots (`DarklyEngine::render` → `drain_readbacks` → `ReadbackRequest::extract_pixels`, nearly all `memmove`) | 6,860 | 60% |
| Darkly committing undo regions (`commit_undo_region`: buffer creation, encoding, submit) and freeing them (`IOGPUMetalBuffer dealloc`) | ~760 | 7% |
| Darkly compiling the brush graph for every stroke (`brush::compile_graph`) | ~620 | 5% |
| Darkly finishing and submitting a command encoder per stroke and dab batch | ~565 | 5% |
| Darkly formatting strings per dab (`BrushGraphRunner::build_slot_outputs` → `format!`) | ~220 | 2% |
| Darkly diffing each stroke for its undo rect (`end_stroke` → `DiffRectPass::request`) | ~190 | 2% |
| Darkly requesting layer thumbnails (`request_thumbnail_readbacks`) | ~180 | 2% |
| Darkly creating a scratch texture per stroke (`StrokeBuffer::new` → `create_texture`) | ~140 | 1% |
| This repository's code (sampling strokes, filtering, PNG encoding) | <100 | <1% |

Other threads are idle or submitting Metal command buffers. Script
evaluation for every journal together takes about 0.2 s.

## Darkly

All of these are in Darkly's interactive-editor machinery, which an offline
batch render doesn't need. Most would go away with a headless "batch"
mode on `DarklyEngine` (no undo, no thumbnails), the single biggest win.

### 1. Every stroke reads the whole canvas back for undo (60%)

`stroke_path` ends each stroke by committing an undo entry. The undo rect
comes from a GPU diff (`DiffRectPass`) that's polled without waiting
(`flush_pending_undo_commit`, `painting.rs`): when the next stroke starts
before the GPU has finished, it falls back to the whole saved area. Offline
strokes arrive back to back, so it nearly always falls back, and every
stroke copies the whole canvas into a staging buffer and then into memory
(~4 MB per stroke on a 1,000 x 1,000 canvas; this is also why renders used
to run out of memory until `render_canvas` ticked a frame every 25
strokes).

Waiting on the device before each stroke (tried from this repo, which owns
the `wgpu::Device`) makes the diff ready but doubles the render time with
stalls. Ticking frames more or less often doesn't help either.

Fixes, in Darkly:
- An option to turn undo history off, skipping `commit_undo_region` and
  `push_undo` for strokes. The cheapest and biggest fix.
- Otherwise, take the undo rect from the stroke engine's own bounds of the
  dabs it placed (it knows them on the CPU) rather than a GPU diff, so it's
  never the whole canvas.
- Reuse staging buffers for undo readbacks rather than creating and
  freeing one per stroke (the `create_buffer`/`dealloc` cost above).

### 2. The brush graph is compiled for every stroke (5%)

`brush_stroke_to` calls `brush::compile_graph` (and builds a
`BrushGraphRunner`) for each stroke, though the brush and its settings
rarely change between strokes. Caching the compiled graph per brush,
invalidated by `brush_load` and `brush_set_exposed_port`, would remove it.

### 3. Per-dab string formatting (2%)

`BrushGraphRunner::build_slot_outputs` builds output names with `format!`
for every dab. Precomputing the slot keys when the graph is compiled (or
using interned ids) would remove the allocations.

### 4. A command encoder and submit per stroke (5%)

Each stroke finishes and submits its own command encoders
(`BrushGpuContext::submit_final`, `CommandEncoder::finish`), and creates a
scratch texture (`Scratch::create_write_texture`). A batch API (several
strokes, one submission) and a pool of scratch textures by size would cut
both.

### 5. Thumbnails nobody looks at (2%)

`DarklyEngine::render` requests layer thumbnail readbacks every frame. A
headless engine never shows them; making thumbnails opt-in (or off with no
display) would skip them.

### 6. SipHash in hot maps (<1%)

Darkly's internal hash maps keyed by ids use the default SipHash
(`Sip13Rounds` shows up on its own). A faster hasher (FxHash, ahash) for
id-keyed maps would help a little.

## resvg (`--svg` output)

Writing a journal's SVG output takes about 0.5 s per page, ~60% of it in
`resvg::render` rasterizing the stylized strokes, and ~10% in usvg parsing
the SVG text. That's resvg doing what it's asked: the stylized strokes are
filled outlines sampled every canvas unit, so they have many points. The
lever is on this side (sampling outlines more coarsely, writing fewer
decimal places), which would change the SVG output, so it's been left
alone.
