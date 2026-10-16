# Layout files

A layout lays out pages: each page is a tree of boxes, sized and placed by
flexbox rules (with [stretch](https://github.com/vislyhq/stretch)), with
slots that text, art and ornaments are poured into. A journal gives a
layout its text, the look of each kind of text (a font, an ink, a size) and
its art, and gets back as many pages as the text needs:

```rhai
journal_entry.add_pages(layout("two_column_article").typeset(#{
	name: "lunar",
	flows: #{ title: "Lunar Influence", body: text, caption: "the moon" },
	roles: #{
		body: #{ font: lang, style: ink, size: 34.0, align: "justify", indent: 1.5 },
		heading: #{ font: caps, style: red, size: 90.0 },
		caption: #{ font: caps, style: red, size: 46.0 },
		initial: #{ font: caps, style: red },
		folio: #{ font: caps, style: red, size: 30.0 },
		rule: #{ style: thin_red },
		divider: #{ style: thin_red },
	},
	art: #{ divider: diamond, illustration: moon_canvas },
}));
```

`layout("two_column_article")` loads `two_column_article.rhai` from here (or from
`journalbuild --layouts-dir DIR`). Adding a layout means adding a file;
nothing needs recompiling. `journals/showcase` uses every layout here.

## What a layout file defines

| function | returns |
|---|---|
| `page(n, info)` | page `n`'s tree of boxes (from 0) |
| `page_size()` *(optional)* | `#{ width, height }`, unless the journal gives them |

`info` has `flows` (how many characters each flow has left), `art` (the
art's names), and `width` and `height`. A layout can use them to change as
the text runs out, or to leave out art it wasn't given.

A layout file runs in the journal's engine, so it can call everything a
journal can, `art::bramble` included.

## Boxes

- `row(style, children)` and `column(style, children)` lay out their
  children across or down. The style is optional.
- `flow(name, style)`: text from the flow `name`. Each flow runs through
  its slots in the order they come in the tree, then on to the next page:
  what doesn't fit one slot goes on in the next. Typesetting stops when
  every flow has run out.
- `label(text, style)`: fixed text, such as a page number.
- `initial(name, style)`: a drop cap, the next letter of a flow, filling
  its box. With `position: "absolute"` in a column alongside the flow's
  slot, the text wraps around it.
- `figure(name, style)`: art, fitted into the box.
- `band(name, style)`: art repeated along the box, for ornamental strips;
  each copy is scaled to the band's breadth and turned to run along it
  (`turn: radians` turns it yourself).
- `rule(style)`: a line along the box. `spacer(style)`: nothing.

## Style

Lengths are canvas units, `"N%"` of the parent's size, or `"auto"`.

| key | on | |
|---|---|---|
| `width`, `height`, `min_width`, `min_height`, `max_width`, `max_height` | any | its size |
| `grow`, `shrink`, `basis` | any | how it shares out its parent's space; a box that grows with no size of its own starts from nothing, so growing boxes share evenly |
| `margin`, `padding` | any | one value, `[vertical, horizontal]`, `[top, right, bottom, left]`, or `#{ top, right, bottom, left }` |
| `position: "absolute"`, `top`, `left`, `right`, `bottom` | any | out of the flow, placed in its parent |
| `aspect` | any | width / height |
| `align_self` | any | "auto", "start", "end", "center", "stretch" |
| `border` | any | a role whose stroke outlines the box |
| `wrap` | leaves | what text wrapping around it keeps clear of: "box", "hull" (what's drawn), or "none"; absolutely placed boxes default to "box", others "none" |
| `gap` | rows, columns | space between children |
| `justify` | rows, columns | "start", "end", "center", "space_between", "space_around", "space_evenly" |
| `align`, `align_content` | rows, columns | "start", "end", "center", "stretch" |
| `wrap` | rows, columns | `true` to wrap children onto more lines |
| `role` and any role setting | leaves | which role it uses, and settings of its own |

Text slots with no height (and no `grow`) are as tall as their text; with
no width in a row, as wide.

## Roles

A slot's role is how its text or strokes look. It's the role named after
the slot (a flow's or art's name, or `initial`, `label`, `rule`) unless it
names another with `role`. Settings:

| key | | |
|---|---|---|
| `font`, `style`, `size` | text | the font (leant for italics: `font("plain", #{ skew: 0.25 })`), stroke style and symbol scale |
| `line_height` | text | in sizes, default 1.25 |
| `align` | text | "left", "center", "right", "justify" (a paragraph's last line is left) |
| `valign` | text | "top", "center", "bottom" in its box |
| `indent` | text | a paragraph's first line, in sizes |
| `paragraph_gap` | text | in lines, default 0.5 |
| `spacing` | text | between glyphs, in sizes, on top of the font's padding between their outlines |
| `jagged` | text | lines start up to this much further in, at random, in sizes |
| `clearance` | text | how far it keeps from what it wraps around, default size / 4 |
| `split_words` | text | `true` breaks a word round what text wraps around, instead of moving it past whole, for a tighter fit; a number is the fewest letters a piece may have (`true` is 2) |
| `split_padding` | text | with `split_words`, how much further the pieces keep from what they're split round, in sizes |
| `fit` | text | shrink the text until all of it fits its box (for headings) |
| `em`, `strong` | text | what emphasis and strong text in markdown look like: maps of span settings (below), by default `#{ italic: true }` and `#{ passes: 2, jitter: size / 20 }` |
| `style` | art, bands, rules, borders | the stroke style, for art that's a path or glyph |

In text, `"\n"` breaks a line and a blank line starts a paragraph.

## Marked-up text

A flow can be `markdown(text)` instead of a string: text styled as it's
set, much as markdown styles it.

```rhai
flows: #{ body: markdown("Plain, *emphasis*, **strong**, ~~struck out~~, [in blue]{color: blue}, "
    + "[scribbled out]{color: black, strike: {count: 3, color: hsla(0, 80%, 40%, 0.9)}}.") }
```

- `*text*` or `_text_` is emphasis, `**text**` or `__text__` strong (as the
  role's `em` and `strong` say), and `~~text~~` struck through.
- `[text]{settings}` is text with settings of its own: `key: value` pairs
  separated by commas, a key on its own being `true`. Values are numbers,
  `true`/`false`, text (`hsla(0, 80%, 40%, 0.9)` is one value), or more
  settings in braces.
- A backslash writes a mark as it is: `\*`, `\[`, `\~`, ...

Spans nest, each changing what it's in. Settings (and `em` and `strong`):

| key | | |
|---|---|---|
| `color` (`colour`) | a CSS colour | the ink |
| `passes`, `jitter` | as a style's | gone over this many times, each pass astray by up to this much |
| `italic` | true or false | leant as italics, or upright |
| `skew` | radians | leant this far |
| `size` | a number | times the size of the text around it; lines grow to fit, and smaller text sits on the same baseline |
| `character_padding` | sizes | room between glyphs' outlines, in place of the font's |
| `word_spacing` | sizes | a space's width, in place of the font's `space_width` |
| `underline` | true, a count, or settings | lines under it, one below another |
| `strike` (`strikethrough`) | true, a count, or settings | lines through it; more than one go back and forth as one stroke, scribbling it out |

Lines' settings are `count`, `spacing` (how far apart, in sizes) and the
`color`, `passes` and `jitter` they're drawn in (by default, the text's);
`false` takes them away inside text that has them. They run across the
spaces between the words they mark, and break where the text does: at the
ends of lines, either side of what the text wraps round, and wherever they'd
come within its `clearance` of it. `label` text is plain.

## Art

Art is a canvas (drawn with its own strokes' styles), a path or glyph
(drawn in the slot's role's style), or a function. A function is given the
box, `#{ x, y, width, height, center }`, and returns a canvas or path drawn
as it is, for art made to measure:

```rhai
fn bramble_in(b) {
	let plant = art::bramble(vec2(b.center.x, b.y + b.height), -PI() / 2.0, b.height, b.height * 0.2, 5);
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
