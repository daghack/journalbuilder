# Lock files

Each lock type is a rhai file here: `.add("v", "vigenere_cipher", #{ ... })`
in a journal loads `vigenere_cipher.rhai`. Adding a lock type means adding a
file; nothing needs recompiling. `journalbuild --locks-dir DIR` uses another
directory.

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
- `"Maze"`: a maze, from `new_maze`.
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
- `symbols(base, text)`: text as symbols, `symbols(27, "HI")` is `[8, 9]`.
- `symbol_text(base, symbols)`: the reverse.
- `new_maze(seed, width, height[, strategy])`: a maze of width x height
  nodes, walked "LeftHanded" (the default) or "RightHanded". Its `cells()`
  are `#{ x, y, step, symbol }` in walking order, and `fill(symbols)` writes
  symbols into them in that order, 0 into any left over. `width()` and
  `height()` are its size in cells.

There's nothing random and nothing to draw with: a lock is a function of its
options and values alone.

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

`fn layout()`, if there is one, is where the web editor's lock editor draws
each part; the lock file is otherwise free to ignore it. The lock editor
writes composite lock files like this one.

## Shared helpers

Functions several lock files need live in `lib/`, which isn't a lock type
itself. Import one at the top of a lock file:

```rhai
import "lib/permutations" as perm;

fn lock(o, i) {
	perm::check(i.ordering, o.base_n, "the ordering");
	#{ cipher_text: perm::apply(i.plain_text, i.ordering), ordering: i.ordering }
}
```

Import paths are relative to this directory, in lib files too. An error in
a helper is reported at its own file and line, e.g. `lib/permutations.rhai,
line 15`.

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
