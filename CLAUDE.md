# Notes for Claude

## Journals and the library

A journal is a directory in `journals/`, or in `webapp/journals/` for the
one built into the web editor (`showcase`; nothing else goes there):
`journal.json` (title,
description, its entries in order), its entries in `entries/NAME.rhai`,
and files of its own in `locks/`, `helpers/`, `layouts/`,
`illustrations/`, `chains/`, `styles/`, `lib/`, `glyphsets/`,
`content/` and `images/` (PNG and JPEG pictures, which the web editor
and the compiled-in library hold as data URLs). These stand in
front of the built-in library, `webapp/library/`, which has the same
directories; a journal's file stands in for the library's of the same
name. `edits/ENTRY.json` holds the visual editor's edits to the
canvases an entry commits to it (`canvas.commit_to_editor(name)`),
applied at the call; the web editor writes it and drops edits for
names no longer committed. Anything any journal might use goes in the library; what only one
journal uses goes in that journal. `journalbuild NAME [--entry E]`
renders into `NAME_output/ENTRY/`.

## Keep the showcase up to date

`webapp/journals/showcase` is a journal of everything journals can draw with: a
page each for pens (Darkly brushes, pressure profiles, widths, colours),
glyphsets, every glyph, art tools, text layout, locks, pages typeset from
layouts, and a journal's files (text styles, content, helpers), then an
appendix of what's been added since. It's what's available, drawn, so it
must never fall behind.

It's only appended to. A new capability gets a page, or more, at the end
of the appendix (after the last appendix page, before `// ---- the title
page and the contents`): a `sheet(topic)` bound with
`book.bind("appendix", canvas)`. Earlier pages, their numbers and their
reference images stay as they are, unless a change alters what they draw;
don't fit new examples into them. The page list at the top names what
the appendix holds.

Whenever a change adds or changes a capability, add it to the appendix in
the same change:

- **A function scripts can call** (a constructor, a method, an option on
  an existing function, an `art::` tool, anything registered with the rhai
  engine): a labelled example. New options deserve an example too, even
  though the test below can't see them.
- **A glyphset or symbol set** (added to `webapp/library/glyphsets/`): its
  alphabet and a line. The glyphs pages draw every glyph of every
  glyphset by themselves.
- **A brush**: the pens page draws every `brush_names()` entry by itself;
  add examples only for new brush settings.
- **A lock or helper type** (`webapp/library/locks/`, `helpers/`): a row
  locking and unlocking an example.
- **An illustration** (`webapp/library/illustrations/*.rhai`): attach it
  and draw what it gives.
- **A chain file** (`webapp/library/chains/*.rhai`): use it with
  `chain(name)`.
- **A layout, text style or content file** (`webapp/library/layouts/`,
  `styles/`, `content/`): use it with `layout(name)`, `text_style(name)`
  or `content(name)`. Library layouts also define `example()`, and styles
  can give an `example`, which their tests preview.
- **A new kind of page or output** (layouts, splitting, ...): a page of
  its own.

The showcase is a sample book: every page is `W` x `H`, drawn on a
`sheet(section)` (running head) and bound with `book.bind(section,
canvas)` (folio, then `new_book`'s `book.add`: name, contents entry);
typeset pages go in with `book.add(section, pages)`. When a page fills
up, bind it and start another sheet. The title page and the contents are
drawn last, from a `seed` of their own, so pages added before them leave
their ink alone.

`tests/showcase.rs` fails when a function scripts can call, or a
built-in (a library lock or helper type, illustration, chain, layout, text style,
content file or glyphset of 20 glyphs or more), isn't in the showcase.
Files of a journal's own needn't be. Don't
silence it by adding to its `NOT_SHOWN` list unless the function truly
isn't a capability (printing, auto-generated fields).

Then check it quickly. Every page, every character having a glyph, as
SVG on the CPU (2s):

```bash
./target/release/journalbuild showcase --strict-glyphs --svg --scale 0.1 --output-dir /tmp/showcase
```

and the pages you added, by name, in Darkly at half scale (2s), to look
at:

```bash
./target/release/journalbuild showcase --page 73_appendix,74_appendix --scale 0.5 --supersample 1 --output-dir /tmp/showcase
```

(Renders land in `/tmp/showcase/showcase/`, the entry's directory.) Add
only the new pages' reference images:
`UPDATE_SVG_RENDERS=1 cargo test --release --test svg_renders showcase`,
which should add images and change none.

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

## Keep docs/api.md up to date

`docs/api.md` is the API reference: an entry for every function journal
scripts can call, each a `###` heading with its signature in code, what
it does and gives back, and tables of its arguments and options with
their defaults and the values they take, in the section it belongs to.
The showcase shows what calls draw; the reference says what they take. It
must match the API exactly.

Whenever a change adds, removes or changes anything scripts can call (a
function, a method, an overload, an argument, an option or its default,
a property, an operator, an `art::` tool, layout box styles or role
settings, markdown span settings, what lock, illustration, layout, style
or chain files define or can call, an illustration or its options, what
a journal's directory or manifest holds, a library file, or a
`journalbuild` flag), update its entry in the same change, or add one.
Remove entries for what's gone.

A call that draws gets a short example: a ```` ```rhai render=NAME ````
block drawing on `page` (the doc's ```` ```rhai prelude ```` sets it and
the inks up), followed by its image, `![NAME](images/NAME.svg)`. Make or
remake the images with:

```bash
UPDATE_DOC_IMAGES=1 cargo test --release --test api_doc
```

and look at the ones you changed (`rsvg-convert docs/images/NAME.svg -o
/tmp/NAME.png`). `tests/api_doc.rs` fails when a function scripts can call
isn't in any entry's heading, a library file isn't named in the doc, or
an example no longer runs or draws what its image shows; it can't see
options, defaults or changed behaviour: keep those right by hand.

## Checking changes

`./scripts/smoke.sh` builds in release mode, runs the tests (every entry
of every journal is evaluated), checks the lock and chain files (the
library's and each journal's own), and renders the smoke fixture, the
showcase and page 0 of every entry of every journal at reduced scale.

Without a GPU (a cloud sandbox), smoke.sh skips the Darkly renders, and
`tests/svg_renders.rs` is what checks drawing: it compares SVG renders of
the smoke fixture, the showcase and page 0 of every entry of every
journal, made on the CPU, with the images in
`tests/svg_renders/<journal>/<entry>/`. A change that means to
alter what's drawn will fail it: look at the diffs it writes to
`target/svg-render-diffs/`, then update the images and commit them with
the change (`UPDATE_SVG_RENDERS=1 cargo test --release --test
svg_renders`). A new journal goes in its `first_pages!` list.
