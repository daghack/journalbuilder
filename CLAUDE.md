# Notes for Claude

## Keep the showcase up to date

`journals/showcase` is a journal of everything journals can draw with: a
page each for pens (Darkly brushes, pressure profiles, widths, colours),
fonts, every glyph, art tools, text layout, and locks. It's the reference
for what's available, so it must never fall behind.

Whenever a change adds or changes a capability, update the showcase in
the same change:

- **A function scripts can call** (a constructor, a method, an option on
  an existing function, an `art::` tool, anything registered with the rhai
  engine): add a labelled example on the page it belongs to, or a new page
  if nothing fits. New options deserve an example too, even though the
  test below can't see them.
- **A font or symbol set** (glyphs added to `config/symbols.json`): add it
  to the fonts page with its alphabet and a sentence. The glyphs page draws
  every glyph by itself.
- **A brush**: the pens page draws every `brush_names()` entry by itself;
  add examples only for new brush settings.
- **A lock type** (`locks/*.rhai`): add a row to the locks page.
- **A new kind of page or output** (layouts, splitting, ...): show it on
  its own page, and update the page list at the top of the showcase.

`tests/showcase.rs` fails when a function scripts can call, a font (a
glyph family of 20 or more), or a lock type isn't in the showcase. Don't
silence it by adding to its `NOT_SHOWN` list unless the function truly
isn't a capability (printing, auto-generated fields).

Then check the showcase still renders, with every character having a
glyph, and look at the pages you changed:

```bash
./target/release/journalbuild showcase --strict-glyphs --output-dir /tmp/showcase
```

Conventions there: labels are lowercase loche (`label_font`), which has
punctuation but no digits; strokes are light colours, since renders are
viewed on a dark background; sections are labelled with the calls they
show.

## Checking changes

`./scripts/smoke.sh` builds in release mode, runs the tests (every journal
script is evaluated), checks the lock files, and renders the smoke fixture,
the showcase and page 0 of every journal at reduced scale.
