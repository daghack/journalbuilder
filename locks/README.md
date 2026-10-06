# Lock files

Each lock type is a rhai file here: `.add("v", "vigenere_cipher", #{ ... })`
in a journal loads `vigenere_cipher.rhai`. Adding a lock type means adding a
file; nothing needs recompiling. `journalbuild --locks-dir DIR` uses another
directory.

## What a lock file defines

| function | runs at | returns |
|---|---|---|
| `options()` | first use | option name → default; `()` marks a required option |
| `check(o)` *(optional)* | `.add` | nothing; `throw` to reject the options |
| `inputs(o)`, `outputs(o)` | `.add` | value name → type, e.g. `#{ plain_text: symbol_type(27) }` |
| `lock(o, inputs)` | `run` | every declared output |
| `unlock(o, outputs)` | `unlock` | the inputs it recovers |
| `examples()` *(optional)* | `--check-locks`, `cargo test` | test cases, see below |

`o` is the options, checked first: unknown names, missing required ones and
values of a different type from their default are errors (an integer is fine
where the default is a number).

An input that is also an output, such as a cipher's `key`, is passed
through: `lock` returns it unchanged, and `unlock` needn't return it.

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
symbol or a missing output is reported with the lock's name and file.

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
