# Performance issues outside this repository

Found while profiling `journalbuild` (October 2026, macOS on Apple silicon,
`sample` on release builds with debug info). Fixes for code in this
repository went in with the profiling; what's here needs changes in
dependencies, so it's written down rather than fixed.

## Where the time goes

Rendering every journal takes about 550 s on Darkly's `dev` and about
210 s on its `offline-render-perf` branch (below), with the same pixels. A
render of `journal_entry_18` (one 2,800 x 3,200 page, ~6,000 glyph strokes,
default `--supersample 1.2`) takes about 19 s on `dev` and 8 s on the
branch. Its main thread on the branch:

| | share |
|---|---|
| Darkly finishing and submitting command encoders, about four per stroke (`BrushGpuContext::submit_final`) | 34% |
| Darkly rendering the dabs (`render_from_stabilized_range_to`) | 21% |
| Darkly formatting strings per dab (`BrushGraphRunner::build_slot_outputs` → `format!`) | 11% |
| Darkly committing each stroke's undo region (`commit_undo_region`) | 7% |
| Darkly creating staging buffers and a scratch texture per stroke (`create_buffer`, `create_texture`) | 8% |
| This repository's code (sampling strokes, filtering, PNG encoding) | <1% |

Other threads are idle or submitting Metal command buffers. Script
evaluation for every journal together takes about 0.2 s.

## Darkly

### Fixed on the `offline-render-perf` branch

Not yet in a pinned Darkly; once it lands on the fork's `dev`, move the pin
in `Cargo.toml` to it.

- **Every stroke read the whole layer back for undo** (60% on `dev`). The
  undo rect came from a GPU diff that only a frame collects, so back-to-back
  offline strokes fell back to committing the whole layer. Each stroke now
  commits the union of its dabs' footprints when it ends.
- **The brush graph compiled for every stroke** (18% of the main thread
  once the above was fixed). It's now compiled once per change. That saves
  CPU but no wall time here: the render is bound by the submissions below.

### Stabilization is tuned for the screen

Darkly's stabilizer resamples a stroke every 6 screen pixels and smooths it
at that scale. A headless engine has no screen (zoom 1), so glyph-sized
strokes at a brush's own `stabilize` (0.6 for Rough Ink) came out as short
straight lines. Our points are already smooth curves, so `Pen::draw` turns
stabilization off for every brush.

### Remaining

1. **A command encoder and submit per stroke phase** (34%). Each stroke
   submits about four command buffers (`stroke-buffer-init`,
   `brush-begin-stroke`, `brush-render-tail`, `brush-commit`). One encoder
   per pen-up flush would cut that.
2. **Per-dab string formatting** (11%). `build_slot_outputs` builds a map of
   `format!`ed names for every dab; resolving them to slot indices once per
   compile would remove both the strings and the map.
3. **Per-stroke textures and copies** (8%). Each stroke creates a
   layer-sized scratch and pre-stroke texture, and copies the whole layer
   twice (once for undo, once for the commit). Reusing the textures and
   sharing one copy would cut both.
4. **Peak memory.** With the readback copies gone, the CPU runs further
   ahead of the GPU, and peak memory rose on several journals (1.5 to 2.7
   GB for `journal_entry_18`). `catch_up` waits for the GPU only in a
   browser; waiting natively too every so many strokes would bound it.
5. **SipHash in hot maps** (<1%). Darkly's id-keyed maps use the default
   hasher; FxHash or ahash would help a little.

Thumbnails, which cost 2% on `dev`, no longer register.

## resvg (`--svg` output)

Writing a journal's SVG output takes about 0.5 s per page, ~60% of it in
`resvg::render` rasterizing the stylized strokes, and ~10% in usvg parsing
the SVG text. That's resvg doing what it's asked: the stylized strokes are
filled outlines sampled every canvas unit, so they have many points. The
lever is on this side (sampling outlines more coarsely, writing fewer
decimal places), which would change the SVG output, so it's been left
alone.
