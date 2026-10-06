# Issue list

Bugs, quirks and anti-patterns found while cleaning up `journalbuild` and
porting the legacy journals to rhai (Darkly's own internals excluded).
Items 1–17 come from the first review; 18 onwards turned up during the
port.

**Statuses:** **Open**, **Fixed** (with the commit), **Resolved** (went away
with other work), **Deferred** (postponed by choice), **Won't fix**
(documented instead), and **Needs decision** (analysed, but it changes the
journals' look or behaviour, so it waits for a call).

## Summary

| # | Item | Kind | Status |
|---|---|---|---|
| 1 | Ternary glyphs used only the first balanced-ternary digit | Bug | Fixed `9465169` |
| 2 | `StrokePath::from_str` handles only a subset of SVG path syntax | Bug | Fixed {{commit: Parse SVG path data properly}} |
| 3 | `bN:` text parsing silently dropped invalid characters | Bug | Fixed `1c93e9e` |
| 4 | Missing glyphs became `X` with only a stray message | Quirk | Fixed `8a1ca2e` |
| 5 | Darkly pressure ramps from min to max along every substroke | Quirk | Fixed {{commit: Add pressure profiles to brushes}} |
| 6 | A style's `width` is stroke width in SVG but a pressure multiplier in Darkly | Anti-pattern | Needs decision (saturation warning added {{commit: Warn when a style's width saturates Darkly pressure}}) |
| 7 | Rust-side randomness was unseeded | Quirk | Fixed `92d9eeb` (plain output: see 26) |
| 8 | Maze cells' colour and orientation are generated, then zeroed | Dead logic | Fixed {{commit: Drop the maze cells' unused colour and orientation}} |
| 9 | Plain-PNG style was hard-coded | Gap | Fixed `3275800` |
| 10 | Canvases are sized to loose stroke bounds, with no margin | Quirk | Deferred |
| 11 | The output folder is silently emptied before every render | Anti-pattern | Fixed {{commit: Clear only earlier renders from the output folder}} |
| 12 | `config/symbols.json` is opened relative to the working directory | Anti-pattern | Open |
| 13 | `&'j mut Journal<'j>` lifetime tangle | Anti-pattern | Resolved `3c547e8` |
| 14 | Two parallel pipelines, legacy JSON and rhai | Anti-pattern | Resolved `3c547e8` |
| 15 | Placeholder types with no content | Dead code | Open |
| 16 | `get_convex_hull()` clones a polygon to read its bounding box | Performance | Open |
| 17 | `unwrap`/`expect`/`panic` everywhere, clippy warnings, deprecations | Code health | Open |
| 18 | Offline Darkly renders piled up GPU readbacks (14 GB on journal 18) | Bug | Fixed `ece9149` |
| 19 | Glyph variant picks depended on hash-map order | Bug | Fixed `92d9eeb` |
| 20 | Glyph hulls used an unseeded jitter | Bug | Fixed `92d9eeb` |
| 21 | Darkly seeded each stroke from the wall clock | Quirk | Fixed `92d9eeb` |
| 22 | Legacy splat: lost jitter, a possible infinite loop, silent drops | Bug | Fixed `d1ff166` |
| 23 | Chained calls on a variable only apply the first call to it | Quirk | Open |
| 24 | A rhai `for` loop variable is a copy | Quirk | Won't fix (documented `76d8d84`) |
| 25 | Darkly renders each canvas separately; plain output merges them | Anti-pattern | Open |
| 26 | Plain-output roughening and noise are unseeded | Quirk | Open |
| 27 | `left_handed_maze_path` duplicates `new_maze(..).path()` | Redundancy | Open |
| 28 | `--page` takes only indices, not page names | Gap | Open |
| 29 | Font sets send digits and punctuation to the upper fonts | Quirk | Open |
| 30 | A glyph that is a single straight line panics when loaded | Bug | Open |
| 31 | The Darkly dependency follows a branch instead of a pinned revision | Anti-pattern | Open |
| 32 | The copper letters' brush sizes predate the fork's Calligraphy change | Quirk | Open |
| 33 | Rhai scripts can't emit warnings; `print` goes to stdout | Gap | Open |
| 34 | Rhai `fn`s can't read script globals | Quirk | Won't fix |
| 35 | Brush sizes are in output pixels, so each port repeats a 1080 px fit | Anti-pattern | Open |
| 36 | `LockConfig`'s string static inputs only served the JSON journals | Dead code | Open |
| 37 | Public library code left unused by the legacy removal | Dead code | Open |
| 38 | `fill_with_msg` loops forever when a glyph can't fit on the path | Bug | Open |
| 39 | Script errors surface as a panic with debug formatting | Bug | Open |
| 40 | An empty or zero-width canvas panics the Darkly render | Bug | Open |
| 41 | `new_brush` accepts nonsense pressures and sizes | Gap | Open |
| 42 | `splat_msgs` on an empty canvas gives a misleading error | Quirk | Open |
| 43 | Quadratic curves convert to cubics incorrectly, moving their end point | Bug | Open |

## Details

### 1. Ternary glyphs used only the first balanced-ternary digit
- **Status:** Fixed in `9465169`.
- **What changed:** letters map to the 27 balanced base-27 symbols directly (a..m → 1..13, n..z → −13..−1, everything else 0), case-insensitively; non-ASCII letters no longer index out of range.

### 2. `StrokePath::from_str` handles only a subset of SVG path syntax
- **Where:** `src/journal/paths.rs`
- **Status:** Open.
- **Problem:** only absolute `M L C Q Z` are understood; comma-separated numbers are silently skipped; too few arguments panic; the regex is recompiled on every call.

### 3. `bN:` text parsing silently dropped invalid characters
- **Status:** Fixed in `1c93e9e`; the JSON journals that used it are gone too (`3c547e8`).

### 4. Missing glyphs became `X` with only a stray message
- **Status:** Fixed in `8a1ca2e`: substitutions are collected, reported as one warning, available to scripts as `missing_glyphs()`, and fatal with `--strict-glyphs`.

### 5. Darkly pressure ramps from min to max along every substroke
- **Where:** `src/journal/darkly.rs`
- **Status:** Open.
- **Problem:** pressure rises linearly from the brush's min to its max over each substroke, so every line gets heavier toward its end, whatever its direction or length.

### 6. A style's `width` is stroke width in SVG but a pressure multiplier in Darkly
- **Where:** `src/journal/canvas/mod.rs`, `src/journal/darkly.rs`
- **Status:** Open.
- **Problem:** the same number means a geometric width in the plain output and a pressure scale in Darkly, so the two outputs can't be tuned together.

### 7. Rust-side randomness was unseeded
- **Status:** Fixed in `92d9eeb`: glyph picks, jitter, text-box edges, hatching and splatting draw from the engine's seeded generator. The plain output's roughening is split out as 26.

### 8. Maze cells' colour and orientation are generated, then zeroed
- **Where:** `src/lockbuilder/locks/maze.rs`
- **Status:** Open.
- **Problem:** `SymbolInfo` fills `color` and `orientation` randomly and the maze lock immediately zeroes both; nothing reads them.

### 9. Plain-PNG style was hard-coded
- **Status:** Fixed in `3275800`: `page.plain_style`.

### 10. Canvases are sized to loose stroke bounds, with no margin
- **Where:** `src/journal/canvas/mod.rs`
- **Status:** Deferred (exact ink bounds were judged too complex for now).
- **Problem:** outer strokes touch the image edge. The bounds are also loose in the other direction: they include Bézier control points and pad every point by the stroke width.

### 11. The output folder is silently emptied before every render
- **Where:** `src/bin/journalbuild/main.rs`
- **Status:** Open. (The duplicate copy went with the legacy code in `3c547e8`.)
- **Problem:** `create_dir` deletes every file in the output directory, including ones journalbuild didn't write, without saying so.

### 12. `config/symbols.json` is opened relative to the working directory
- **Where:** `src/bin/journalbuild/main.rs`, tests
- **Status:** Open.
- **Problem:** running journalbuild from anywhere but the repository root fails to find the symbol dictionary.

### 13. `&'j mut Journal<'j>` lifetime tangle
- **Status:** Resolved in `3c547e8`: the legacy `Journal` type is gone.

### 14. Two parallel pipelines, legacy JSON and rhai
- **Status:** Resolved: the journals were ported (`cc878cd`, `5b655fa`, `f432b6c`) and the legacy pipeline removed (`3c547e8`).

### 15. Placeholder types with no content
- **Where:** `src/journal/rhaientries/mod.rs`
- **Status:** Open.
- **Problem:** `EntryMetadata`, `PageMetadata`, `MarkdownContent` and a one-variant `LayoutStyle` carry nothing.

### 16. `get_convex_hull()` clones a polygon to read its bounding box
- **Where:** `src/journal/symbol.rs`, `src/journal/canvas/textbox.rs`
- **Status:** Open.
- **Problem:** text layout clones each glyph's hull, in its inner loop, just to read its extents.

### 17. `unwrap`/`expect`/`panic` everywhere, clippy warnings, deprecations
- **Status:** Open.
- **Problem:** around 120 panicking calls and 120 clippy warnings, plus `std::f32::MAX`/`MIN` deprecation warnings in `paths.rs`.

### 18. Offline Darkly renders piled up GPU readbacks
- **Status:** Fixed in `ece9149`: a frame is ticked every 25 strokes, so stroke bookkeeping completes; journal 18 went from a 14 GB peak (killed by the OS) to under 1 GB.

### 19. Glyph variant picks depended on hash-map order
- **Status:** Fixed in `92d9eeb`: `symbols_with_prefix` sorts its names.

### 20. Glyph hulls used an unseeded jitter
- **Status:** Fixed in `92d9eeb`: the jitter uses a fixed seed.

### 21. Darkly seeded each stroke from the wall clock
- **Status:** Fixed in `92d9eeb`: strokes are seeded by index, so Darkly renders are byte-identical run to run.

### 22. Legacy splat: lost jitter, a possible infinite loop, silent drops
- **Status:** Fixed in `d1ff166` (native `splat_msgs`).

### 23. Chained calls on a variable only apply the first call to it
- **Where:** `src/journal/rhaientries/` (paths, glyphs, canvases)
- **Status:** Open.
- **Problem:** functions modify their target and return a copy, so `g.translate_by(v).scale_by(2.0);` changes `g` only by the translation; the scale applies to the copy and is lost. Chains on temporaries and `x = x.a().b()` work.

### 24. A rhai `for` loop variable is a copy
- **Status:** Won't fix: it's rhai's semantics. Documented in `glyphs.rs` (`76d8d84`) with the index-based alternative.

### 25. Darkly renders each canvas separately; plain output merges them
- **Where:** `src/journal/rhaientries/mod.rs`
- **Status:** Open.
- **Problem:** a page with two canvases gives one plain PNG but two Darkly PNGs, so "page" means different things per output.

### 26. Plain-output roughening and noise are unseeded
- **Where:** `src/journal/canvas/mod.rs`, `src/journal/paths.rs`
- **Status:** Open.
- **Problem:** the plain PNG/SVG differ on every run even with a seeded script.

### 27. `left_handed_maze_path` duplicates `new_maze(..).path()`
- **Status:** Open.

### 28. `--page` takes only indices, not page names
- **Status:** Open.

### 29. Font sets send digits and punctuation to the upper fonts
- **Where:** `src/journal/styles/mod.rs`
- **Status:** Open.
- **Problem:** `FontSet` picks the lower fonts only for lowercase letters, so digits and punctuation are looked up in the upper fonts first.

### 30. A glyph that is a single straight line panics when loaded
- **Where:** `src/journal/symbol.rs`
- **Status:** Open.
- **Problem:** a straight line's convex hull is degenerate and `ConvexPolygon::from_convex_hull` fails; the dictionary load panics.

### 31. The Darkly dependency follows a branch instead of a pinned revision
- **Where:** `Cargo.toml`
- **Status:** Open.

### 32. The copper letters' brush sizes predate the fork's Calligraphy change
- **Where:** `journals/copper_letter_1*/journal_entry.rhai`
- **Status:** Open.
- **Problem:** the fork treats Calligraphy's size as the nib's long axis (2.5× the old meaning); the legacy default was rescaled for it, the copper letters weren't.

### 33. Rhai scripts can't emit warnings; `print` goes to stdout
- **Status:** Open.
- **Problem:** a script can only `print` (stdout) or `throw`; warnings don't reach stderr, where smoke.sh and the user look.

### 34. Rhai `fn`s can't read script globals
- **Status:** Won't fix: rhai functions are pure by design; pass values in, or use `this` for the method target.

### 35. Brush sizes are in output pixels, so each port repeats a 1080 px fit
- **Where:** `journals/*/journal_entry.rhai`, `src/journal/darkly.rs`
- **Status:** Open.
- **Problem:** brush size is in render pixels while everything else is in canvas units, so the ported journals each compute `1.75 * extent / 1080` to keep the legacy look.

### 36. `LockConfig`'s string static inputs only served the JSON journals
- **Where:** `src/lockbuilder/mod.rs`
- **Status:** Open.
- **Problem:** static inputs as `"bN:..."` strings are parsed at build time; rhai passes typed values instead, so the string path is now unused.

### 37. Public library code left unused by the legacy removal
- **Status:** Open.
- **Problem:** the library's `pub` items aren't flagged by the compiler, so helpers only the legacy code used may linger.

### 38. `fill_with_msg` loops forever when a glyph can't fit on the path
- **Where:** `src/journal/rhaientries/mod.rs`
- **Status:** Open.
- **Problem:** it keeps placing the unplaced glyphs on new copies of the path; if a glyph never fits (a short path, a big font) no progress is made and it never returns.

### 39. Script errors surface as a panic with debug formatting
- **Where:** `src/bin/journalbuild/main.rs`
- **Status:** Open.
- **Problem:** a script error prints `Failed to execute result.: Custom { kind: Other, error: "..." }` and a panic note, exit status 101. A journal with no pages silently renders nothing.

### 40. An empty or zero-width canvas panics the Darkly render
- **Where:** `src/journal/rhaientries/mod.rs`, `src/journal/darkly.rs`
- **Status:** Open.
- **Problem:** the render size rounds to 0 and the canvas resize assertion fails.

### 41. `new_brush` accepts nonsense pressures and sizes
- **Status:** Open.
- **Problem:** min pressure above max, negative pressures and sizes are accepted.

### 42. `splat_msgs` on an empty canvas gives a misleading error
- **Status:** Fixed in {{commit: Parse SVG path data properly}}.
- **Problem:** only absolute `M L C Q Z` were understood; comma-separated numbers were silently skipped; too few arguments panicked; the regex was recompiled on every call.
- **Change:** `StrokePath` implements `FromStr` with a small hand-written parser for the SVG path grammar: every command except arcs, absolute and relative (`H`/`V`, smooth `S`/`T` with control-point reflection), commas, compact numbers (`.5.5`, `1-2`, exponents), and repeated argument groups (extra pairs after a moveto are linetos). Bad data is an error naming the byte offset, and the dictionary load reports which symbol it's in. The `regex` dependency is gone.
- **Why:** the dictionary happened to use only the supported subset, but any symbol exported with relative commands or commas would have loaded wrong or crashed. All 249 existing symbols parse exactly as before (checked against the old parser's output).

### 43. Quadratic curves convert to cubics incorrectly, moving their end point
- **Where:** `src/journal/paths.rs` (`PathCmd::to_cubic_curve`)
- **Status:** Fixed in {{commit: Clear only earlier renders from the output folder}}. (The duplicate copy went with the legacy code in `3c547e8`.)
- **Problem:** `create_dir` deleted every file in the output directory, including ones journalbuild didn't write, without saying so.
- **Change:** `prepare_output_dir` removes only `.png` and `.svg` files (the only things journalbuild writes), reports how many it cleared, and leaves everything else. `--page` runs still clear nothing. The usage text says so.
- **Why:** clearing is still wanted, so renamed or removed pages don't leave stale renders behind, but notes or other files kept next to the renders shouldn't vanish.
