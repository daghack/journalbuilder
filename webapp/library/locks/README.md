# Lock files

Each lock type is a rhai file here: `.add("v", "vigenere_cipher", #{ ... })`
in a journal loads `vigenere_cipher.rhai`. Adding a lock type means adding a
file; nothing needs recompiling. A journal's own `locks/` stand in front
of these, the library's.

## What a lock file defines

| function | runs at | returns |
|---|---|---|
| `options()` | first use | option name → type, e.g. `#{ base: integer() }` (see below) |
| `check(o)` *(optional)* | `.add` | nothing; `throw` to reject the options |
| `inputs(o)`, `outputs(o)` | `.add` | value name → type, e.g. `#{ plain_text: symbol_type(27) }` |
| `lock(o, inputs)` | `run` | every declared output |
| `unlock(o, outputs)` | `unlock` | the inputs it recovers |
| `examples()` *(optional)* | `--check-locks`, `cargo test` | test cases, see below |

A composite lock file defines `options()` and `chain(o)` instead; see
[Composite lock files](#composite-lock-files).

`o` is the options, checked against their declared types first.

An input that is also an output, such as a cipher's `key`, is passed
through: `lock` returns it unchanged, and `unlock` needn't return it.

## Options

`options()` declares each option with its type:

- `integer()`, `number()`, `string()`, `bool()`, `array()`: a required
  option of that type. Given a default, e.g. `integer(0)` or
  `string("LeftHanded")`, the option is optional.
- `one_of(["LeftHanded", "RightHanded"])`: one of those values, also with an
  optional default: `one_of([...], "LeftHanded")`.

Unknown options, missing required ones and values of the wrong type are
errors (an integer is fine for a number). Anything beyond the type, such as
a base that must be even, goes in `check(o)`.

It's `integer()` rather than `int()` because rhai's own `int(x)` is the
integral part of a float.

## Values and types

Lock functions get and return plain rhai values. A value's type is a string:

- `symbol_type(base)`, e.g. `"Base27Symbol"`: an array of integers from 0
  to base - 1. Base 27 is space (0) and A-Z (1-26); base 10 is digits; other
  bases up to 26 are letters from A (0).
- A value kind's name, `"HuffmanCode"`, `"Maze"` or `"StarChart"`: plain
  data (see [Value kinds](#value-kinds)), checked against the kind's shape.
- `"String"`: a string.
- `"Any"`: matches every type.
- Any other name, e.g. `"KeySchedule"`: a value of the lock file's own
  making, which only connects to inputs of the same type.

What `lock` and `unlock` return is checked against these types, so a stray
symbol or a missing output is reported with the lock's name and file. In a
chain, an output only connects to an input of the same type (or `"Any"`).

## What a lock file can call

The standard rhai library (arrays, maps, strings, math), plus:

- `symbol_type(base)`: the type name above.
- `symbols(base, text[, #{ lenient }])`: text as symbols, `symbols(27, "HI")` is `[8, 9]`;
  with `lenient: true` any text is made to fit the base first.
- `symbol_text(base, symbols)`: the reverse.
- `new_maze(width, height[, #{ seed, strategy }])`: a Maze of width x height
  nodes, laid out by the seed (0 by default) and walked "LeftHanded" (the
  default) or "RightHanded". Its `cells()` are `#{ x, y, step, symbol }` in
  walking order, and `fill(symbols)` writes symbols into them in that
  order, 0 into any left over. `width` and `height` are its size in cells.

- Value kinds' functions: `huffman::code(symbols, input_base, output_base[,
  #{ smoothing }])`, `stars::layout(code[, options])` and
  `stars::split(points, edges, sizes[, options])`.
- `new_random(seed)`: a generator, `next()` from 0 up to 1, `range(low,
  high)` and `pick(items)`. Copies share it, so a helper handed one moves it
  on for its caller too.

There's nothing unseeded and nothing to draw with: a lock is a function of
its options and values alone.

## Value kinds

A value kind is a value type whose values are plain data: maps, arrays,
numbers and strings, tagged `kind: "<Name>"`. They cross between lock
files, journals and the web editor as they are, and are checked against
the kind's shape wherever they go into or out of a lock, with errors naming
the field (`pages[2].stars[0].point should be a point [x, y]`). Each kind
lives in one Rust file in `src/journal/kinds/`, which gives:

- its shape, exact: keys it doesn't declare are errors;
- its functions, the same in lock files and journals (`stars::layout`);
- its sketch, how it's drawn by default: `value.sketch()` and
  `art::sketch(data)` in journals, and the web editor's preview.

| kind | data | from |
|---|---|---|
| `HuffmanCode` | `#{ input_base, output_base, nodes: [#{ parent, digit, symbol }] }`, root first then breadth first | `huffman::code` |
| `Maze` | `#{ width, height, seed, strategy, symbols }`, its corridors from its seed | `new_maze` |
| `StarChart` | `#{ input_base, output_base, pages: [#{ name, stars: [#{ name, point, size, label }], links: [[a, b, digit]] }] }` | the `star_chart` lock |

A kind's value holds only what its drawing shows, plus geometry, so a lock
that unlocks from it unlocks from what a reader sees.

### Art nodes

An art node is a lock whose output is a drawing's data, a value kind with
a sketch: `star_chart` draws a `HuffmanCode` as a `StarChart`. It's an
ordinary lock, so it must unlock: its examples prove the drawing carries
what it draws. `star_chart`'s unlock joins pages by their shared star
names, reads leaf symbols from name initials and digits from links, and
rebuilds the code. Chained after `huffman_encode`, which gives the code
its text builds as its key, the pages become the key, which `huffman_apply`
can lock another message with:

```rhai
let lock = new_lock("sky")
	.add("enc", "huffman_encode", #{ input_base: 27, output_base: 3 })
	.add("msg", "huffman_apply", #{ input_base: 27, output_base: 3 })
	.add("sky", "star_chart", #{ input_base: 27, output_base: 3 })
	.connect("enc.code", "msg.code")
	.connect("enc.code", "sky.code");
```

A new visual lock is a kind file and a lock file; nothing else changes.

## Composite lock files

A lock type can be other lock types wired together: a chain, as
`new_lock(...).add(...).connect(...)` builds in a journal, saved as a lock
file of its own. It defines `options()` and `chain(o)` instead of `inputs`,
`outputs`, `lock` and `unlock`; `check(o)` and `examples()` work as above.
`shift_vigenere.rhai` is one:

```rhai
fn options() {
	#{ shift: integer(3) }
}

fn chain(o) {
	#{
		locks: #{
			shift: #{ type: "shift_cipher", options: #{ base: 27, shift: o.shift } },
			vig: #{ type: "vigenere_cipher", options: #{ cipher_base: 27, key_base: 27, plain_base: 27 } },
		},
		connect: [
			["shift.cipher_text", "vig.key"],
		],
		fixed: #{},
		inputs: #{ key: "shift.plain_text", plain_text: "vig.plain_text" },
		outputs: #{ cipher_text: "vig.cipher_text", shifted_key: "vig.key" },
	}
}
```

- `locks`: each part's name and its `type`, with its `options` (which can
  use the composite's own, `o`). A part can be a composite itself, but not
  one that includes the type being defined.
- `connect`: `[output, input]` pairs, as `.connect` takes them.
- `fixed`: values for inputs, as `.set_input` sets them.
- `inputs` and `outputs`: the lock type's own names for the chain's open
  ports, those neither connected nor fixed. Every open port needs one,
  except an output passing a fixed input straight through (a fixed key, say),
  which unlocking puts back by itself. An input and an output can share a
  name only when they're the same port, a value passed through.

Composite lock files are written by hand. A chain to use in a journal
(with its illustrations) is a chain file instead, which the web editor's lock
editor makes: see chains/README.md.

## Shared helpers

Functions several lock files need live in `lib/` (`webapp/library/lib/`,
or a journal's own), beside the lock files' directory. Any script imports
from there, entries and layout, illustration, chain and style files
included; a helper runs in the engine of the script importing it, so a
lock file's helpers can't draw. Import one at the top of a lock file:

```rhai
import "lib/permutations" as perm;

fn lock(o, i) {
	perm::check(i.ordering, o.base_n, "the ordering");
	#{ cipher_text: perm::apply(i.plain_text, i.ordering), ordering: i.ordering }
}
```

Import paths start `lib/`, in lib files too. An error in
a helper is reported at its own file and line, e.g. `lib/permutations.rhai,
line 15`.

- `lib/huffman.rhai`: `check(code, o)`, `codes(code)`, `encode(code,
  symbols)` and `decode(code, digits)`, for codes given as HuffmanCodes.
- `lib/names.rhai`: names from syllables, `sky(code, rng, count)` for a
  star chart's stars and constellations, and `symbol_of(base, name)`.
- `lib/permutations.rhai`: `check(ring, base, what)` throws unless `ring`
  holds each symbol once; `apply(symbols, table)` maps symbols through a
  table; `inverse(table)` undoes one; `pairs(ring)` swaps the ring's first
  half with its second.

## Examples

`examples()` returns an array of maps:

```rhai
#{
	options: #{ base: 27, shift: 3 },
	inputs: #{ plain_text: symbols(27, "HELLO WORLD") },
	outputs: #{ cipher_text: symbols(27, "KHOORCZRUOG") },   // optional
}
```

Each example is locked and then unlocked, and must give its inputs back.
`outputs`, if given, pins what locking produces. An example with
`error: "text"` must instead fail with an error containing that text.

`journalbuild --check-locks` runs every file's examples, and so does
`cargo test`, which also requires each file here to have some.

## Rhai gotchas

- Functions can't see variables from outside themselves; pass what they
  need as arguments.
- `%` keeps the sign of the left side: `-1 % 27` is `-1`. Wrap with
  `((n % base) + base) % base`.
- A closure can't capture a variable while a method is being called on part
  of it: `i.plain_text.map(|s| i.ordering[s])` fails with "Data race
  detected". Copy it out first: `let ordering = i.ordering;`.
