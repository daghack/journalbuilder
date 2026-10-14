# Notes for Claude

## Keep the showcase up to date

`journals/showcase` is a journal of everything journals can draw with: a
page each for pens (Darkly brushes, pressure profiles, widths, colours),
fonts, every glyph, art tools, text layout, locks, and pages typeset from
layouts. It's the reference for what's available, so it must never fall
behind.

Whenever a change adds or changes a capability, update the showcase in
the same change:

- **A function scripts can call** (a constructor, a method, an option on
  an existing function, an `art::` tool, anything registered with the rhai
  engine): add a labelled example on the page it belongs to, or a new page
  if nothing fits. New options deserve an example too, even though the
  test below can't see them.
- **A font or symbol set** (a font added to `config/fonts.json`): add it
  to the fonts pages with its alphabet and a line. The glyphs pages draw
  every glyph of every font by themselves.
- **A brush**: the pens page draws every `brush_names()` entry by itself;
  add examples only for new brush settings.
- **A lock type** (`locks/*.rhai`): add a row to the locks page.
- **A new kind of page or output** (layouts, splitting, ...): show it on
  its own page, and update the page list at the top of the showcase.

The showcase is a sample book: every page is `W` x `H`, drawn on a
`sheet(section)` (running head) and bound with `book.bind(section,
canvas)` (folio, name, contents entry); typeset pages go in with
`book.bind_page`. When a page fills up, bind it and start another sheet;
the contents follow.

`tests/showcase.rs` fails when a function scripts can call, a font (of
20 glyphs or more), or a lock type isn't in the showcase. Don't
silence it by adding to its `NOT_SHOWN` list unless the function truly
isn't a capability (printing, auto-generated fields).

Then check the showcase still renders, with every character having a
glyph, and look at the pages you changed:

```bash
./target/release/journalbuild showcase --strict-glyphs --output-dir /tmp/showcase
```

Conventions there: labels and body text are in plain (`PLAIN`), which
has capitals, digits and punctuation, and headings in loche (`LOCHE`);
inks are red and near-black; text blocks are lorem ipsum; sections are
labelled with the calls they show.

## Keep the API reference up to date

`journals/api_reference` is a book with an entry for every function
journal scripts can call: its signature, its arguments and options with
their defaults, and what it gives back. The showcase shows what calls
draw; the reference says what they take. It must match the API exactly.

Whenever a change adds, removes or changes anything scripts can call (a
function, a method, an overload, an argument, an option or its default,
a property, an operator, an `art::` tool, layout box styles or role
settings, markdown span settings, what lock or layout files define or
can call, or a `journalbuild` flag), update its `api("signature",
"description")` entry in the same change, or add one in the section it
belongs to. Remove entries for what's gone. Signatures can't contain a
double quote, and both parts are written as plain text (markdown marks
are escaped for you).

`tests/api_reference.rs` fails when a function scripts can call isn't in
any entry's signature, but it can't see options, defaults or changed
behaviour: keep those right by hand.

Then render it and look at the pages you changed (it uses only `plain`
and `loche`, so every character must have a glyph):

```bash
./target/release/journalbuild api_reference --strict-glyphs --parchment --output-dir /tmp/api_reference
```

Body text is plain at size 26 with a constant-pressure pen: smaller, the
plain font's dots and colons don't render.

## Checking changes

`./scripts/smoke.sh` builds in release mode, runs the tests (every journal
script is evaluated), checks the lock files, and renders the smoke fixture,
the showcase and page 0 of every journal at reduced scale.
