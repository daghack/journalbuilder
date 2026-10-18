# Notes for Claude

## Keep the showcase up to date

`webapp/journals/showcase` is a journal of everything journals can draw with: a
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
- **A font or symbol set** (a font added to `webapp/fonts/`): add it
  to the fonts pages with its alphabet and a line. The glyphs pages draw
  every glyph of every font by themselves.
- **A brush**: the pens page draws every `brush_names()` entry by itself;
  add examples only for new brush settings.
- **A lock type** (`webapp/locks/*.rhai`): add a row to the locks page.
- **A new kind of page or output** (layouts, splitting, ...): show it on
  its own page, and update the page list at the top of the showcase.

The showcase is a sample book: every page is `W` x `H`, drawn on a
`sheet(section)` (running head) and bound with `book.bind(section,
canvas)` (folio, then `new_book`'s `book.add`: name, contents entry);
typeset pages go in with `book.add(section, pages)`. When a page fills
up, bind it and start another sheet; the contents follow.

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

## Keep the API's shape

Everything scripts can call follows the same rules; new functions and
options should too:

- What a call needs comes first, positionally; everything optional goes
  in one trailing options map, which can always be left out (register the
  call with and without it). No positional optionals, no second overload
  doing the same thing another way.
- A constructor's options take every property its value can be set by,
  applied through the property's own setter (`options::apply_props`), so
  `new_X(..., #{ prop })` and `x.prop = ...` check alike. A new settable
  property goes in its constructor's list too.
- Text calls take one text-settings map with the layout role keys
  (`text_settings.rs`): role keys a call doesn't use are ignored, so a
  role can be passed straight in. A new text setting is a role key too.
- Things are put in containers with `add`, which returns the container.
- Cheap read-only facts are properties; computations are methods.

Lock-file option types (`integer(3)`, `one_of(choices, default)`) keep
their default positional: `default` is a reserved word in rhai.

## Keep the API reference up to date

`webapp/journals/api_reference` is a book with an entry for every function
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

Without a GPU (a cloud sandbox), smoke.sh skips the Darkly renders, and
`tests/svg_renders.rs` is what checks drawing: it compares SVG renders of
the smoke fixture, the showcase and page 0 of every journal, made on the
CPU, with the images in `tests/svg_renders/`. A change that means to
alter what's drawn will fail it: look at the diffs it writes to
`target/svg-render-diffs/`, then update the images and commit them with
the change (`UPDATE_SVG_RENDERS=1 cargo test --release --test
svg_renders`). A new journal goes in its `first_pages!` list. Run tests
with `--release`: debug builds reject the bigger scripts as too complex.
