# Helper files

A helper type is a lock file that transforms data rather than locking it:
it splits a text into pieces (`split_symbols`), joins texts
(`merge_alpha`) or reads a string as symbols (`string_to_symbols`). Locks,
in `locks/`, are the ciphers and puzzles; helpers are what wires them
together.

A helper file is written exactly as a lock file is (see
[locks/README.md](../locks/README.md)): `options()`, `inputs(o)`,
`outputs(o)`, `lock(o, inputs)`, `unlock(o, outputs)` and `examples()`,
and it must unlock what it locks. It's used the same way, by name:

```rhai
let lock = new_lock("split")
	.add("s", "split_symbols", #{ base_n: 27, output_count: 2 })
	.add("v", "vigenere_cipher", #{ plain_base: 27, key_base: 27, cipher_base: 27 })
	.connect("s.output_0", "v.plain_text");
```

A type name is looked up in `locks/` first, then here; a journal's own
`helpers/` stand in front of these, the library's. Adding a helper means
adding a file; nothing needs recompiling. `journalbuild --check-locks` and
`cargo test` run helpers' examples as they do locks'.

Functions a helper file imports live in `lib/`, as for lock files; those
are shared code, not node types.
