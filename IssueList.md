# Issue list

Bugs, quirks and anti-patterns found while cleaning up `journalbuild` and
porting the legacy journals to rhai (Darkly's own internals excluded).
Items 1–17 come from the first review; 18 onwards turned up during the
port.

**Statuses:** **Open**, **Fixed** (with the commit), **Resolved** (went away
with other work), **Deferred** (postponed by choice), **Won't fix**
(documented instead), and **Needs decision** (analysed, but it changes the
journals' look or behaviour, so it waits for a call).

## Waiting on a decision

- **6:** make `width` geometric in Darkly too (recommended), or keep it as a pressure multiplier. Changes every journal's Darkly look.
- **32:** set `copper_letter_1_sharp`'s Calligraphy nib to 2.0 (recommended) so it renders solid instead of as hairlines.
- **10** stays deferred (exact ink bounds).

## Summary

| # | Item | Kind | Status |
|---|---|---|---|
| 1 | Ternary glyphs used only the first balanced-ternary digit | Bug | Fixed `9465169` |
| 2 | `StrokePath::from_str` handles only a subset of SVG path syntax | Bug | Fixed `184fed3` |
| 3 | `bN:` text parsing silently dropped invalid characters | Bug | Fixed `1c93e9e` |
| 4 | Missing glyphs became `X` with only a stray message | Quirk | Fixed `8a1ca2e` |
| 5 | Darkly pressure ramps from min to max along every substroke | Quirk | Fixed `aa79ac3` |
| 6 | A style's `width` is stroke width in SVG but a pressure multiplier in Darkly | Anti-pattern | Needs decision (saturation warning added `71e8f42`) |
| 7 | Rust-side randomness was unseeded | Quirk | Fixed `92d9eeb` (plain output: see 26) |
| 8 | Maze cells' colour and orientation are generated, then zeroed | Dead logic | Fixed `245a5a0` |
| 9 | Plain-PNG style was hard-coded | Gap | Fixed `3275800` |
| 10 | Canvases are sized to loose stroke bounds, with no margin | Quirk | Deferred |
| 11 | The output folder is silently emptied before every render | Anti-pattern | Fixed `5f2ff6a` |
| 12 | `config/symbols.json` is opened relative to the working directory | Anti-pattern | Fixed `7ca07bf` |
| 13 | `&'j mut Journal<'j>` lifetime tangle | Anti-pattern | Resolved `3c547e8` |
| 14 | Two parallel pipelines, legacy JSON and rhai | Anti-pattern | Resolved `3c547e8` |
| 15 | Placeholder types with no content | Dead code | Fixed `7e5f569` |
| 16 | `get_convex_hull()` clones a polygon to read its bounding box | Performance | Fixed `e96587f` |
| 17 | `unwrap`/`expect`/`panic` everywhere, clippy warnings, deprecations | Code health | Fixed `d42af3c` (panics: see 30, 38–41, 44) |
| 18 | Offline Darkly renders piled up GPU readbacks (14 GB on journal 18) | Bug | Fixed `ece9149` |
| 19 | Glyph variant picks depended on hash-map order | Bug | Fixed `92d9eeb` |
| 20 | Glyph hulls used an unseeded jitter | Bug | Fixed `92d9eeb` |
| 21 | Darkly seeded each stroke from the wall clock | Quirk | Fixed `92d9eeb` |
| 22 | Legacy splat: lost jitter, a possible infinite loop, silent drops | Bug | Fixed `d1ff166` |
| 23 | Chained calls on a variable only apply the first call to it | Quirk | Fixed {{commit: Share paths, glyphs and canvases between copies}} |
| 24 | A rhai `for` loop variable is a copy | Quirk | Fixed {{commit: Share paths, glyphs and canvases between copies}} |
| 25 | Darkly renders each canvas separately; plain output merges them | Anti-pattern | Fixed `3cd2cb2` |
| 26 | Plain-output roughening and noise are unseeded | Quirk | Fixed `35b0435` |
| 27 | `left_handed_maze_path` duplicates `new_maze(..).path()` | Redundancy | Fixed `234187a` |
| 28 | `--page` takes only indices, not page names | Gap | Fixed `68bc49b` |
| 29 | Font sets send digits and punctuation to the upper fonts | Quirk | Fixed `5219415` |
| 30 | A glyph that is a single straight line panics when loaded | Bug | Fixed `9c79679` |
| 31 | The Darkly dependency follows a branch instead of a pinned revision | Anti-pattern | Fixed `6e14f59` |
| 32 | The copper letters' brush sizes predate the fork's Calligraphy change | Quirk | Needs decision |
| 33 | Rhai scripts can't emit warnings; `print` goes to stdout | Gap | Fixed `8a177dc` |
| 34 | Rhai `fn`s can't read script globals | Quirk | Won't fix |
| 35 | Each ported journal repeated a 1080 px brush-size fit | Anti-pattern | Fixed `99d38c1` |
| 36 | `LockConfig`'s string static inputs only served the JSON journals | Dead code | Fixed `52640ed` |
| 37 | Public library code left unused by the legacy removal | Dead code | Fixed `bbdb500` |
| 38 | `fill_with_msg` loops forever when a glyph can't fit on the path | Bug | Fixed `997cb65` |
| 39 | Script errors surface as a panic with debug formatting | Bug | Fixed `5ac0e02` |
| 40 | An empty or zero-width canvas panics the Darkly render | Bug | Fixed `22d95b8` |
| 41 | `new_brush` accepts nonsense pressures and sizes | Gap | Fixed `0608f7c` |
| 42 | `splat_msgs` on an empty canvas gives a misleading error | Quirk | Fixed `f6986c1` |
| 43 | Quadratic curves convert to cubics incorrectly, moving their end point | Bug | Fixed `6b98c5b` |
| 44 | Non-finite numbers from scripts can panic layout and hatching | Bug | Fixed `8e631e6` |

## Details

### 1. Ternary glyphs used only the first balanced-ternary digit
- **Status:** Fixed in `9465169`.
- **What changed:** letters map to the 27 balanced base-27 symbols directly (a..m → 1..13, n..z → −13..−1, everything else 0), case-insensitively; non-ASCII letters no longer index out of range.

### 2. `StrokePath::from_str` handles only a subset of SVG path syntax
- **Where:** `src/journal/paths.rs`
- **Status:** Fixed in `184fed3`.
- **Problem:** only absolute `M L C Q Z` were understood; comma-separated numbers were silently skipped; too few arguments panicked; the regex was recompiled on every call.
- **Change:** `StrokePath` implements `FromStr` with a small hand-written parser for the SVG path grammar: every command except arcs, absolute and relative (`H`/`V`, smooth `S`/`T` with control-point reflection), commas, compact numbers (`.5.5`, `1-2`, exponents), and repeated argument groups (extra pairs after a moveto are linetos). Bad data is an error naming the byte offset, and the dictionary load reports which symbol it's in. The `regex` dependency is gone.
- **Why:** the dictionary happened to use only the supported subset, but any symbol exported with relative commands or commas would have loaded wrong or crashed. All 249 existing symbols parse exactly as before (checked against the old parser's output).

### 3. `bN:` text parsing silently dropped invalid characters
- **Status:** Fixed in `1c93e9e`; the JSON journals that used it are gone too (`3c547e8`).

### 4. Missing glyphs became `X` with only a stray message
- **Status:** Fixed in `8a1ca2e`: substitutions are collected, reported as one warning, available to scripts as `missing_glyphs()`, and fatal with `--strict-glyphs`.

### 5. Darkly pressure ramps from min to max along every substroke
- **Where:** `src/journal/darkly.rs`
- **Status:** Fixed in `aa79ac3`.
- **Problem:** pressure rose linearly from the brush's min to its max over each substroke, so every line got heavier toward its end, whatever its direction or length, with no way to choose otherwise.
- **Change:** brushes have a `pressure_profile`: `"ramp"` (the old behaviour, still the default), `"taper"` (a sine swell, light at both ends, full at the middle) or `"constant"`. Set it in rhai with `brush.pressure_profile = "taper"`; unknown names fail at their line.
- **Why:** the ramp is part of how the existing journals look, so changing the default would have altered every render (renders with the default are byte-identical to before). Making it an explicit, documented option turns a hidden quirk into a choice.

### 6. A style's `width` is stroke width in SVG but a pressure multiplier in Darkly
- **Where:** `src/journal/canvas/mod.rs`, `src/journal/darkly.rs`
- **Status:** Needs decision. Partly addressed in `71e8f42`.
- **Problem:** the same number means a geometric width in the plain output and a pressure multiplier in Darkly, so the two outputs can't be tuned together. Worse, Darkly clamps pressure to 1, so in Darkly `width` stops having any effect past `1 / max_pressure`: journal 17's dial (width 6.75 × max 0.25 = 1.69) and maze glyphs (5 × 0.25), and `copper_letter_1_sharp`'s text (1.5 × 1.0), are saturated for much of each stroke.
- **Change so far:** `new_strokestyle` documents both meanings, and the Darkly render warns once per style when `width × max_pressure > 1`.
- **Options for the real fix:**
  1. *Width is geometric everywhere*: Darkly's brush size becomes `brush.size × width`, with pressure only from the brush's min/max and profile. One meaning, no clamping; but every journal's Darkly look changes and brush sizes need re-tuning (Darkly's size/pressure response is per brush, so there's no exact conversion).
  2. *Separate knobs*: keep `width` for the plain output only and ignore it in Darkly, so pressure is purely the brush's. Simple, but the journals that rely on width for Darkly weight (17's ring hierarchy) lose it.
  3. *Keep it, documented* (the current state).
- **Recommendation:** option 1, with a side-by-side re-tune of each journal's brush sizes. (Brush sizes are already in canvas units, see 35, so only the width/pressure meaning changes.)

### 7. Rust-side randomness was unseeded
- **Status:** Fixed in `92d9eeb`: glyph picks, jitter, text-box edges, hatching and splatting draw from the engine's seeded generator. The plain output's roughening is split out as 26.

### 8. Maze cells' colour and orientation are generated, then zeroed
- **Where:** `src/lockbuilder/locks/maze.rs`
- **Status:** Fixed in `245a5a0`.
- **Problem:** `SymbolInfo` filled `color` and `orientation` randomly and the maze lock immediately zeroed both; the legacy journal 17 code that mapped them to colours and rotations never had any effect.
- **Change:** both fields are gone; a cell holds only its symbol.
- **Why:** dead state that suggested a feature that didn't exist. Lock mazes are unaffected (the layout is generated before cells are filled, and the lock overwrites every symbol), so journal 17's maze renders byte-identically; only `new_maze`'s random placeholder symbols differ.

### 9. Plain-PNG style was hard-coded
- **Status:** Fixed in `3275800`: `page.plain_style`.

### 10. Canvases are sized to loose stroke bounds, with no margin
- **Where:** `src/journal/canvas/mod.rs`
- **Status:** Deferred (exact ink bounds were judged too complex for now).
- **Problem:** outer strokes touch the image edge. The bounds are also loose in the other direction: they include Bézier control points and pad every point by the stroke width.

### 11. The output folder is silently emptied before every render
- **Where:** `src/bin/journalbuild/main.rs`
- **Status:** Fixed in `5f2ff6a`. (The duplicate copy went with the legacy code in `3c547e8`.)
- **Problem:** `create_dir` deleted every file in the output directory, including ones journalbuild didn't write, without saying so.
- **Change:** `prepare_output_dir` removes only `.png` and `.svg` files (the only things journalbuild writes), reports how many it cleared, and leaves everything else. `--page` runs still clear nothing. The usage text says so.
- **Why:** clearing is still wanted, so renamed or removed pages don't leave stale renders behind, but notes or other files kept next to the renders shouldn't vanish.

### 12. `config/symbols.json` is opened relative to the working directory
- **Where:** `src/bin/journalbuild/main.rs`, tests
- **Status:** Fixed in `7ca07bf`.
- **Problem:** running journalbuild (or the tests) from anywhere but the repository root failed to find `config/symbols.json` and `journals/`, with a bare panic.
- **Change:** the defaults are resolved from the crate directory at build time (`symboldict::DEFAULT_SYMBOLS`, and the repository's `journals/`); `--symbols FILE` overrides the dictionary; a dictionary that won't load is a one-line error naming the path. The tests use the same constants.
- **Why:** journalbuild is run from the repository it's built in, so baking in its location is the simplest correct default; explicit flags cover anything else.

### 13. `&'j mut Journal<'j>` lifetime tangle
- **Status:** Resolved in `3c547e8`: the legacy `Journal` type is gone.

### 14. Two parallel pipelines, legacy JSON and rhai
- **Status:** Resolved: the journals were ported (`cc878cd`, `5b655fa`, `f432b6c`) and the legacy pipeline removed (`3c547e8`).

### 15. Placeholder types with no content
- **Where:** `src/journal/rhaientries/mod.rs`
- **Status:** Fixed in `7e5f569`.
- **Problem:** `EntryMetadata`, `PageMetadata`, `MarkdownContent` and a one-variant `LayoutStyle` carried nothing, and the page-to-canvas conversion matched on the single variant.
- **Change:** all four are gone. Page metadata that does exist now (`name`, `plain_style`) lives on `Page` itself. (The page-layering conversion went later, with 25.)
- **Why:** they suggested features that weren't there, and were registered as rhai types nobody could use. They're easy to reintroduce with real content if layouts or metadata are ever needed.

### 16. `get_convex_hull()` clones a polygon to read its bounding box
- **Where:** `src/journal/symbol.rs`, `src/journal/canvas/textbox.rs`
- **Status:** Fixed in `e96587f`.
- **Problem:** text layout cloned each glyph's convex hull, in its inner loop, just to read its extents.
- **Change:** each glyph base caches its hull's bounding box, read through `Symbol::hull_aabb()`. `get_convex_hull` is gone, along with the unused `intersects`, `placed_bounds` and `placed_convex_hull`; `intersects` was also wrong, scaling the other glyph's hull by this glyph's scale.
- **Why:** cheap and allocation-free. Evaluating every journal script got about 9% faster (0.69 s to 0.63 s, mostly journal 18's layout), with identical output.

### 17. `unwrap`/`expect`/`panic` everywhere, clippy warnings, deprecations
- **Status:** Fixed in `d42af3c`, for clippy and the deprecations; the panics that scripts can reach are tracked individually (30, 38–41, 44).
- **Problem:** about 120 clippy warnings, `std::f32::MAX`/`MIN` deprecation warnings in `paths.rs`, and around 120 `unwrap`/`expect`/`panic` calls.
- **Change:** `cargo clippy --all-targets` is clean. Clippy's machine fixes were reviewed and kept (`From` instead of `Into` impls, slices instead of `&Vec`, `Default` impls, collapsed `if let`s, redundant clones and conversions); by hand: the deprecated constants, `Display` instead of inherent `to_string`s, a `SymbolFilter` alias, and `Rc` instead of `Arc` for the symbol dictionary (it holds `Rc`s and a `RefCell`, so `Arc` gave no thread safety). The NaN-rejecting `!(x > 0.0)` argument checks are kept deliberately, with a crate-level `allow` explaining why. The panicking calls dropped from about 120 to 52 outside tests, most of the rest being internal invariants (lookups the code just populated).
- **Why:** a quiet build makes new warnings visible. All 20 Darkly renders across the journals and the fixture are byte-identical before and after.

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
- **Status:** Fixed in {{commit: Share paths, glyphs and canvases between copies}} (option 1). Earlier documented in `03eddbb`.
- **Problem:** path, glyph and canvas functions modified their target and returned a copy, so `g.translate_by(v).scale_by(2.0);` changed `g` only by the translation: rhai passes the variable by reference to the first call of a chain only, and later calls acted on the returned copy.
- **Change:** in scripts these three types are now shared handles (`Shared<T>`, an `Rc<RefCell<T>>`): every copy of one (assignment, argument, array element, `for` variable) is the same value, and mutators return that same handle, so chains on variables apply every call. `.copy()` makes an independent value. Adding to something takes a snapshot: `add_strokepath`, `add_glyph(s)`, `new_stroke` copy the path, and `page.add_canvas` copies the canvas, so changing a path or canvas afterwards doesn't reach what it was added to. Paths also got a real `to_string` (rhai had printed the type name, which made one glyph test vacuous).
- **Why:** chained calls behave the way they read, with no "assign the chain back" rule. The cost is that `let b = a;` aliases; the only script code that copied-then-modified was one test, now `ring.copy()`. All renders are byte-identical.

### 24. A rhai `for` loop variable is a copy
- **Status:** Fixed in {{commit: Share paths, glyphs and canvases between copies}} (with 23). Earlier documented in `76d8d84`.
- **Problem:** a rhai `for` loop variable was a copy, so `for g in glyphs { g.scale = 7.0; }` changed nothing.
- **Change:** glyphs, paths and canvases are shared handles (see 23), so a loop variable is the array element and changes reach it. (Plain values like numbers and maps still copy, as rhai does.)
- **Why:** the natural loop now works for the types scripts loop over.

### 25. Darkly renders each canvas separately; plain output merges them
- **Where:** `src/journal/rhaientries/mod.rs`
- **Status:** Fixed in `3cd2cb2`.
- **Problem:** a page with two canvases gave one plain PNG (its canvases layered) but two Darkly PNGs, so "page" meant different things per output, and canvas names only reached the Darkly files.
- **Change:** the plain output is one PNG per canvas too, `{page}_{canvas}.png` beside `darkly_{page}_{canvas}.png`; the page-layering conversion is gone. To draw several canvases as one image, merge them with `canvas.add_canvas(..)`, as the journals already do for splatted notes.
- **Why:** per-canvas images are what canvas names were added for, so the plain output follows Darkly rather than the other way round. Plain file names change (`page_0.png` becomes `page_0_0.png`; journalbuild clears the old ones on the next full render).

### 26. Plain-output roughening and noise are unseeded
- **Where:** `src/journal/canvas/mod.rs`, `src/journal/paths.rs`
- **Status:** Fixed in `35b0435`.
- **Problem:** the plain PNG's roughened strokes and Perlin texture used `rand::random`, so the plain output differed on every run even with a seeded script.
- **Change:** `Canvas::to_svg` and `to_png` take a seed, and the roughening and noise draw from a generator seeded with it. Each canvas is seeded with an FNV-1a hash of its output name (`{page}_{canvas}`), stable across runs and Rust versions, so canvases differ from each other but not from run to run. The unused `roughen_by`, `roughened_by`, `save_to_png` and `save_to_svg` went rather than being converted.
- **Why:** with this, every output of a journal (plain and Darkly) is byte-identical run to run.

### 27. `left_handed_maze_path` duplicates `new_maze(..).path()`
- **Status:** Fixed in `234187a`.
- **Problem:** `left_handed_maze_path(seed, w, h)` built the same maze as `new_maze(seed, w, h)` and returned its `path()`, so there were two spellings for one thing.
- **Change:** the copper letters use `new_maze(index, 10, 10).path()` and the old function is gone.
- **Why:** one maze API. The copper letters' renders are byte-identical (all 24 files).

### 28. `--page` takes only indices, not page names
- **Status:** Fixed in `68bc49b`.
- **Problem:** `--page` took only indices, though pages can now be named (`maze`, `balanced_ternary`), and output files are named after them.
- **Change:** `--page` takes indices or names, mixed (`--page 0,maze`). `Entry::page_index` resolves them; an unknown page is an error that lists the journal's pages with their indices.
- **Why:** names are what you see in the output folder, and they don't shift when pages are added or reordered.

### 29. Font sets send digits and punctuation to the upper fonts
- **Where:** `src/journal/styles/mod.rs`
- **Status:** Fixed in `5219415`.
- **Problem:** `FontSet` looked up lowercase letters only in the lower fonts and everything else (capitals, digits, punctuation) only in the upper fonts, so a character one set lacked became `X` even when the other set had it.
- **Change:** each character still tries its own set first (lowercase letters the lower fonts; everything else the upper fonts), then falls back to the other set, then to `X`. `missing_glyphs()` lists every font tried.
- **Why:** sending punctuation to the upper fonts is right for the existing fonts (only `loche_upper` has punctuation), so the priority stays; the fallback can only replace an `X` with a real glyph. No journal had missing glyphs, so no render changes.

### 30. A glyph that is a single straight line panics when loaded
- **Where:** `src/journal/symbol.rs`
- **Status:** Fixed in `9c79679`.
- **Problem:** a glyph whose points are collinear (a single straight stroke like `-` or `/`), a single point, or empty, has no convex hull, and loading the dictionary panicked with "Failed to generate convex hull".
- **Change:** when the points don't wrap into a hull, the glyph's hull is its bounding box padded by 0.02 (1% of a glyph's span), so layout still sees a sliver of width and height.
- **Why:** such glyphs are legitimate (dashes, slashes, `I`). Glyphs that already had a hull are untouched.

### 31. The Darkly dependency follows a branch instead of a pinned revision
- **Where:** `Cargo.toml`
- **Status:** Fixed in `6e14f59`.
- **Problem:** `Cargo.toml` followed the fork's `offline-stroke-path` branch, so a `cargo update` would pull whatever the branch held next, silently changing how journals render.
- **Change:** pinned with `rev = "3f99aea…"` (the commit already in `Cargo.lock`), with a comment naming the branch.
- **Why:** renders are now byte-reproducible, and the renderer version should only change deliberately. Moving to a newer fork commit, or back to upstream once `stroke_path` lands, is a one-line edit.

### 32. The copper letters' brush sizes predate the fork's Calligraphy change
- **Where:** `journals/copper_letter_1*/journal_entry.rhai`
- **Status:** Needs decision.
- **Problem:** the fork treats Calligraphy's size as the nib's long axis, 2.5× its old meaning. The legacy default brush was rescaled for it (0.7 to 1.75), but `copper_letter_1_sharp`'s Calligraphy brush (size 0.8) wasn't. (`copper_letter_1` uses Rough Ink, which the fork didn't change.)
- **Finding:** at 0.8 the letter's strokes render as broken, striped hairlines; at 2.0 (0.8 × 2.5, the fork's equivalent of the old size) they're solid calligraphy again.
- **Recommendation:** change the brush to `new_brush("Calligraphy", 1.0, 1.0, 2.0)`. It's a one-line edit, left undone because it changes a finished journal's look.

### 33. Rhai scripts can't emit warnings; `print` goes to stdout
- **Status:** Fixed in `8a177dc`.
- **Problem:** a script could only `print` (stdout, mixed into progress output) or `throw` (stopping the render); there was no way to flag a problem and carry on.
- **Change:** `warn(message)` writes `warning: message (line N)` to stderr, like journalbuild's own warnings (so smoke.sh shows it), and records it for `JournalEngine::warnings()`. Journal 17 uses it for key symbols that don't fit around its maze.
- **Why:** the journals already wanted it (17 was printing a fake "warning:" to stdout).

### 34. Rhai `fn`s can't read script globals
- **Status:** Won't fix: rhai functions are pure by design; pass values in, or use `this` for the method target.

### 35. Each ported journal repeated a 1080 px brush-size fit
- **Where:** `journals/*/journal_entry.rhai`, `src/journal/darkly.rs`
- **Status:** Fixed in `99d38c1`.
- **Problem:** each ported journal computed its Calligraphy nib as `1.75 * extent / 1080`, journal 17 through two helper functions, which looked like brush sizes were in output pixels and had to be fitted to each page.
- **Finding:** they aren't. A canvas unit is a pixel at `--scale 1` and Darkly multiplies brush sizes by the scale, so brush sizes are already in canvas units like everything else. The formula only reproduced the legacy renderer, which fit every page to 1080 px.
- **Change:** the journals use plain nib sizes with a one-line note on where they came from: 2.15 (first_codex), 1.65 and 2.05 (journal 17's maze and dial), 2.43 (journal 18), the formula's values to two decimals.
- **Why:** the scripts get simpler and say what they mean; the look is unchanged.

### 36. `LockConfig`'s string static inputs only served the JSON journals
- **Where:** `src/lockbuilder/mod.rs`
- **Status:** Fixed in `52640ed`.
- **Problem:** `LockConfig.static_inputs` held `"bN:..."` strings, parsed by `parse_info_var` when the lock was built. Only the JSON journals wrote them; rhai passes typed values with `set_input` instead.
- **Change:** the field, its parsing and `parse_info_var` are gone. Fixed inputs are only `CompositeLock::add_static_input` with an `InfoVar`. The validation tests moved onto `base_n_symbols`, which rhai's `symbols()` uses.
- **Why:** one way to fix a lock input, and no stringly-typed parsing step left over from the JSON format.

### 37. Public library code left unused by the legacy removal
- **Status:** Fixed in `bbdb500`.
- **Problem:** the library's `pub` items aren't checked by the dead-code lint, so helpers only the legacy code used lingered.
- **Change:** with the library's modules made private for one build (exporting only what journalbuild calls), the compiler listed everything unreachable, and it's gone: `CanvasShape`; a dozen `Canvas` transforms and fitting helpers; `TextBox::place_all_symbols` (rhai's `place_all` replaced it); unused `Symbol` constructors and accessors, its `color` field, and the stored hull (only its bounding box is read); assorted `PathCmd`/`StrokePath` point and transform helpers; the symbol builder's filter hook; `SymbolPathDict::new_symbol`; `string_infovar_set`. About 320 lines. `Canvas::to_svg` stays for the tests. Earlier items removed more of the same (`intersects`, `roughen_by`, `save_to_png`, `parse_info_var`, ...).
- **Why:** code nobody calls still has to be read and kept compiling. All 28 renders across the journals and fixture are byte-identical.

### 38. `fill_with_msg` loops forever when a glyph can't fit on the path
- **Where:** `src/journal/rhaientries/mod.rs`
- **Status:** Fixed in `997cb65`.
- **Problem:** `fill_with_msg` lays the message along fresh copies of the path until every glyph is placed; when a glyph didn't fit at all (a short path, a big font) no copy placed anything and it looped forever. `fit_msg_to` had the quiet version of the same bug: if the message didn't fit even at its smallest font size it returned whatever fitted.
- **Change:** `fill_with_msg` stops with an error naming the glyph that can't fit (and rejects a font size that isn't positive); `fit_msg_to` errors when glyphs are left over at its smallest size.
- **Why:** a hung render is the worst way to learn a path is too short. Every journal still renders as before.

### 39. Script errors surface as a panic with debug formatting
- **Where:** `src/bin/journalbuild/main.rs`
- **Status:** Fixed in `5ac0e02`.
- **Problem:** a script error printed `Failed to execute result.: Custom { kind: Other, error: "Failed to Evaluate Rhai: ..." }` and a panic note, with exit status 101; a journal with no pages silently rendered nothing; failing to write a render, read the journal or clear the output folder panicked.
- **Change:** journalbuild's work runs in `run() -> Result`, and every failure is one `error:` line with exit status 1: `error: journals/x/journal_entry.rhai: Runtime error: oops (line 1, position 1)`. `build_entry_*` return rhai's message with the file name; the publishing functions and `prepare_output_dir` return errors naming the file; an entry with no pages is an error. The `EntryPath`/`JournalPath` debug prints are gone, and the GPU is only set up once the script has succeeded.
- **Why:** script errors are the common case while writing a journal, so they should read like compiler errors, not crashes.

### 40. An empty or zero-width canvas panics the Darkly render
- **Where:** `src/journal/rhaientries/mod.rs`, `src/journal/darkly.rs`
- **Status:** Fixed in `22d95b8`, tested in `5327540`.
- **Problem:** an empty canvas, or one with no width or height (a vertical hairline with width 0) or under a pixel at the render scale, rounded to a 0-pixel render and failed Darkly's resize assertion; the plain output failed to parse its zero-size SVG.
- **Change:** canvases render at `Canvas::render_extents()`, their bounds' extents but at least one unit a side (the bounds centred in it), for both outputs, and Darkly renders are at least 1×1 px. Canvases with no strokes are skipped, with one warning listing them (`page/canvas`).
- **Why:** an empty or degenerate canvas is a script-level mistake at worst, worth a warning, not a crash mid-render. Every existing render is byte-identical.

### 41. `new_brush` accepts nonsense pressures and sizes
- **Status:** Fixed in `0608f7c`.
- **Problem:** `new_brush` accepted a min pressure above the max, pressures outside Darkly's 0–1 range, and zero or negative sizes, which rendered as nothing or as nonsense without an error.
- **Change:** `new_brush` requires `0 <= min <= max <= 1` and a positive size, failing at the script line otherwise. Equal pressures (a fixed pressure) are still fine.
- **Why:** mistakes in brush settings show up at the line that made them. Every existing journal's brushes are within range.

### 42. `splat_msgs` on an empty canvas gives a misleading error
- **Status:** Fixed in `f6986c1`.
- **Problem:** messages are placed in boxes within the canvas's bounds; an empty canvas has none, and after 100 hopeless tries the error said "message 0 doesn't fit".
- **Change:** `splat_msgs` on a canvas with no strokes fails straight away with "the canvas is empty, so there's nowhere to place messages; draw on it first".
- **Why:** the error now names the actual cause.

### 43. Quadratic curves convert to cubics incorrectly, moving their end point
- **Where:** `src/journal/paths.rs` (`PathCmd::to_cubic_curve`)
- **Status:** Fixed in `6b98c5b`.
- **Problem:** the plain output's roughening converts every segment to a cubic first, one command at a time, so a quadratic's conversion didn't know its start point: it placed the control points wrongly and moved the end point a third of the way past the control point. Quadratic strokes (the fixture's `quad_to`, and the 50 quadratic segments in the glyph dictionary) were distorted in the plain PNGs.
- **Change:** `StrokePath::to_cubics()` converts a whole path, tracking the current point, with the exact cubic for each quadratic (control points 2/3 of the way from each end to the quadratic's control point). `PathCmd::to_cubic_curve` is gone.
- **Why:** the plain output now draws the shapes the script describes, matching Darkly. Only plain renders containing quadratics changed; no Darkly render did.

### 44. Non-finite numbers from scripts can panic layout and hatching
- **Where:** `src/journal/paths.rs`, `src/journal/symbol.rs`, `src/journal/rhaientries/`
- **Status:** Fixed in `8e631e6`.
- **Problem:** an infinite or NaN coordinate (from dividing or scaling by zero in a script) crashed the render, since infinite bounds asked for an impossibly large image, or quietly drew garbage, and could reach the k-d tree (which panics on non-finite points) and the hatch's `partial_cmp(..).unwrap()` sort.
- **Change:** `new_vec` refuses non-finite coordinates, and `add_strokepath`, `add_stroke`, `add_glyph(s)` (and so `splat_msgs`) refuse paths with any; each fails at the script line. Inside, avoid trees skip non-finite points, `AvoidanceKdTree::add_point` skips infinities as well as NaN, and the hatch sorts with `total_cmp`.
- **Why:** the error appears at the line that made the bad number, rather than as a crash later. Renders are byte-identical.
