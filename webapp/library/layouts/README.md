# Layout files

A layout lays out pages: each page is a tree of boxes, sized and placed by
flexbox rules (with [stretch](https://github.com/vislyhq/stretch)), with
slots that text, art and ornaments are poured into. A journal gives a
layout its text, the look of each kind of text (a style, a type and a
paragraph) and its art, and gets back as many pages as the text needs:

```rhai
journal_entry.add(layout("two_column_article").typeset(#{
	name: "lunar",
	flows: #{ title: "Lunar Influence", body: text, caption: "the moon" },
	roles: #{
		body: #{ style: ink, type: new_type_style(lang, 34.0), paragraph: #{ align: "justify", indent: 1.5 } },
		heading: #{ style: red, type: new_type_style(caps, 90.0) },
		caption: #{ style: red, type: new_type_style(caps, 46.0) },
		initial: #{ style: red, type: #{ glyphset: caps } },
		folio: #{ style: red, type: new_type_style(caps, 30.0) },
		rule: #{ style: thin_red },
		divider: #{ style: thin_red },
	},
	art: #{ divider: diamond, illustration: moon_canvas },
}));
```

`layout("two_column_article")` loads `two_column_article.rhai` from the
journal's `layouts/`, or else from here, the library's;
`layout("two_column_article", #{ gap: 60 })` gives it options. Adding a
layout means adding a file; nothing needs recompiling.
`journals/showcase` uses every layout here.

## What a layout file defines

| function | returns |
|---|---|
| `options()` *(optional)* | its options and their defaults, `#{ NAME: value }`; `layout(name, options)` can give only these, each like its default |
| `page(n, info)` | page `n`'s tree of boxes (from 0) |
| `page_size(o)` or `page_size()` *(optional)* | `#{ width, height }`, given the options, unless the journal gives them |
| `example()` *(optional)* | what to typeset to preview it, `#{ flows, roles, art }`: the web editor previews it, and `cargo test` checks it typesets |

`info` has `flows` (how many characters each flow has left), `art` (the
art's names), `width` and `height`, and `options`. A layout can use them
to change as the text runs out, or to leave out art it wasn't given.
Library layouts' examples write in the library's text styles and content
(`text_style("plain_hand")`, `content("lorem")`).

A layout file runs in the journal's engine, so it can call everything a
journal can, `art::bramble` included.

## Boxes

Every box takes an options map, which can be left out: `#{ flex }`, its
flex style, and for slots (every box but rows and columns) `role` and a
`style`, `type`, `paragraph` or `spans` of their own.

- `row(children, options)` and `column(children, options)` lay out their
  children across or down.
- `flow(name, options)`: text from the flow `name`. Each flow runs through
  its slots in the order they come in the tree, then on to the next page:
  what doesn't fit one slot goes on in the next. Typesetting stops when
  every flow has run out.
- `label(text, options)`: fixed text, such as a page number.
- `initial(name, options)`: a drop cap, the next letter of a flow, filling
  its box. With `position: "absolute"` in a column alongside the flow's
  slot, the text wraps around it.
- `figure(name, options)`: art, fitted into the box.
- `band(name, options)`: art repeated along the box, for ornamental strips;
  each copy is scaled to the band's breadth and turned to run along it
  (the flex style's `turn: radians` turns it yourself).
- `rule(options)`: a line along the box. `spacer(options)`: nothing.

## Flex styles

`#{ flex: ... }` takes a `new_flex_style(#{ ... })` or a map of its
settings. Lengths are canvas units, `"N%"` of the parent's size, or
`"auto"`.

| key | on | |
|---|---|---|
| `width`, `height`, `min_width`, `min_height`, `max_width`, `max_height` | any | its size |
| `grow`, `shrink`, `basis` | any | how it shares out its parent's space; a box that grows with no size of its own starts from nothing, so growing boxes share evenly |
| `margin`, `padding` | any | one value, `[vertical, horizontal]`, `[top, right, bottom, left]`, or `#{ top, right, bottom, left }` |
| `position: "absolute"`, `top`, `left`, `right`, `bottom` | any | out of the flow, placed in its parent |
| `aspect` | any | width / height |
| `align_self` | any | "auto", "start", "end", "center", "stretch" |
| `border` | any | a style that outlines the box, or a role's name (a layout file can't see the journal's styles) |
| `text_wrap` | leaves | what text wrapping around it keeps clear of: "box", "hull" (what's drawn), or "none"; absolutely placed boxes default to "box", others "none" |
| `turn` | bands | how far to turn each copy, in radians |
| `gap` | rows, columns | space between children |
| `justify` | rows, columns | "start", "end", "center", "space_between", "space_around", "space_evenly" |
| `align`, `align_content` | rows, columns | "start", "end", "center", "stretch" |
| `wrap` | rows, columns | `true` to wrap children onto more lines |

Text slots with no height (and no `grow`) are as tall as their text; with
no width in a row, as wide.

## Roles

A slot's role is how its text or strokes look. It's the role named after
the slot (a flow's or art's name, or `initial`, `label`, `rule`) unless it
names another with `role`. A role is `#{ style, type, paragraph, spans }`,
each a whole style or a map of its settings over the defaults, and a slot
can override any of them with its own, a whole one or a map over its
role's: `flow("body", #{ flex: #{ grow: 1 }, paragraph: #{ align: "center" } })`.

| key | | |
|---|---|---|
| `style` | any | what text and strokes are drawn in, and the hand that sets the glyphs: a style, or a map of style settings over `default_style()` |
| `type` | text | the glyphset and glyph scale, and how glyphs are shaped and spaced: `new_type_style(glyphset, glyph_scale, #{ glyph_tracking, glyph_outline_gap, space_width, skew, italic, glyph_scale_factor, underline, strike })`; initials need only a glyphset |
| `paragraph` | text | how lines are built and set: `line_height` (in glyph scales, default 1.25), `align`, `valign`, `indent` (in glyph scales), `paragraph_gap` (in lines, default 0.5), `clearance` (from what it wraps around, default 3), `line_start_jitter`, `line_gap_jitter` (canvas units), `split_words`, `split_padding`, `fit` (shrink the text until all of it fits its box) |
| `spans` | text | named spans for markdown, `em` and `strong` among them (below) |

In text, `"\n"` breaks a line and a blank line starts a paragraph.

## Marked-up text

A flow can be `markdown(text)` instead of a string: text styled as it's
set, much as markdown styles it.

```rhai
flows: #{ body: markdown("Plain, *emphasis*, **strong**, ~~struck out~~, [in blue]{color: blue}, [a warning]{warn}, "
    + "[scribbled out]{color: black, strike: {count: 3, style: {color: hsla(0, 80%, 40%, 0.9)}}}.",
    #{ spans: #{ warn: #{ style: red_ink } } }) }
```

- `*text*` or `_text_` is the span `em`, `**text**` or `__text__` the span
  `strong`, and `~~text~~` struck through.
- `[text]{settings}` is text with settings of its own: span names, and
  `key: value` pairs separated by commas, a key on its own being `true`.
  A word on its own is a setting if it's one, and a span's name otherwise;
  the named spans apply first, then the settings. Values are numbers,
  `true`/`false`, text (`hsla(0, 80%, 40%, 0.9)` is one value), or more
  settings in braces.
- A backslash writes a mark as it is: `\*`, `\[`, `\~`, ...

A span is `#{ style, type }`, each a whole one or a map of settings over
the text it's in. Spans are named in a role's `spans` and in `markdown(text,
#{ spans })`, which win. `em` defaults to `#{ type: #{ italic: true } }` and
`strong` to `#{ style: #{ passes: 2, pass_end_jitter: 1.3 } }`.

Settings in braces are a style's and a type's: `color`, `passes`,
`pass_end_jitter`, `stroke_point_jitter` and the rest; `italic`, `skew`,
`glyph_scale_factor` (times the size of the text around it: lines grow to
fit, and smaller text sits on the same baseline), `glyph_outline_gap`,
`space_width`, `underline` and `strike`. Spans nest, each changing what
it's in.

Lines under and through text take `true`, a number of lines, `false`
(none, inside text that has them), or `{count, spacing, style}`: how many,
how far apart (in glyph scales), and the style they're drawn in, by
default the text's. Lines under text go one below another; lines through
it go back and forth as one stroke, scribbling it out. They run across the
spaces between the words they mark, and break where the text does: at the
ends of lines, either side of what the text wraps round, and wherever
they'd come within its `clearance` of it.

## Art

Art is a canvas (drawn with its own strokes' styles), a path or glyph
(drawn in the slot's role's style, `default_style()` without one), or a
function. A function is given the
box, `#{ x, y, width, height, center }`, and returns a canvas or path drawn
as it is, for art made to measure:

```rhai
fn bramble_in(b) {
	let plant = art::bramble(vec2(b.center.x, b.y + b.height), b.height, #{ angle: -PI() / 2.0, flower_size: b.height * 0.2, seed: 5 });
	...
}
art: #{ illustration: Fn("bramble_in") }
```

## Pages

Typeset pages are named `{name}_{n}` (`name` defaults to the layout's) and
render exactly `width` x `height`, so they line up with each other; they
split into tiles like any page. Typesetting fails if a page sets none of
what's left of the text (a flow with no slot, or no room), or runs past
`max_pages` (default 200).
