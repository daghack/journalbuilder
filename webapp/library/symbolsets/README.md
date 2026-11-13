# Symbol set files

A symbol set is a glyphset's glyphs as the symbols of a lock, and how each
is drawn: a value of symbols reaches the page without being turned into a
string and then glyphs by hand. `symbol_set("name")` loads one from here (a
journal's own `symbolsets/` stand in front of the library's).

## Simple: `NAME.json`

| key | what |
|---|---|
| `description` | what it's for |
| `glyphset` | the glyphset its glyphs come from (required) |
| `glyphs` | the glyphs' names in symbol order, `null` a blank (a space); left out, all the glyphset's own |
| `style` | a `styles/` file whose pen, hand and size draw them (required) |

## Scripted: `NAME.rhai`

Its opening comment is its description.

| function | gives |
|---|---|
| `options()` *(optional)* | `#{ NAME: default }`, as an illustration's |
| `symbols(o)` | `#{ glyphset, glyphs, style, size }`: as the JSON file, or `glyph` and `count` for the same glyph that many times; `style` and `size` optional, one of them must give a size |
| `draw(o, symbol, glyph)` | a canvas for each non-blank symbol: `symbol` the whole number, `glyph` its glyph as placed |

The glyph is what layout measures and places: pick one whose box covers
what `draw` draws.
