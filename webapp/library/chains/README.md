# Chain files

A chain file is a chain of lock types, wired together, with the illustrations
that draw its values: what a journal would build with `new_lock(...)`,
`.add`, `.connect`, `.set_input` and `.attach`, kept as a file of its own
so journals can use it by name:

```rhai
let c = chain("my_sky", #{ output_base: 3 });
let result = c.run(#{ "enc.plain_text": symbols(27, "ONCE UPON A MIDNIGHT DREARY") });
let pages = result.canvases("pages");
```

`chain(name)` builds `name.rhai` from the journal's `chains/`, or else
from here, the library's, as `new_lock` would: its values are its locks' own
ports, `lock.port`, and a journal can add, connect and attach more. The
web editor's lock editor makes chain files; as you edit one there, it's in
the browser's workspace, so a journal using it follows along, as it does
an edited font. Adding a chain means adding a file; nothing needs
recompiling.

## What a chain file defines

```rhai
// my_sky: what it is (the editor shows this).

fn options() {
	#{ output_base: integer(3) }
}

fn chain(o) {
	#{
		locks: #{
			enc: #{ type: "huffman_encode", options: #{ input_base: 27, output_base: o.output_base } },
			sky: #{ type: "star_chart", options: #{ input_base: 27, output_base: o.output_base } },
		},
		connect: [
			["enc.code", "sky.code"],
		],
		fixed: #{},
		illustrations: #{
			message: #{ type: "trit_message", options: #{ base: o.output_base }, inputs: #{ message: "enc.cipher_text" } },
			pages: #{ type: "star_pages", options: #{}, inputs: #{ chart: "sky.chart" } },
		},
	}
}

fn layout() {
	#{ enc: [120, 60], sky: [480, 60], message: [480, 300], pages: [840, 60] }
}

fn example() {
	#{
		"enc.plain_text": symbols(27, "ONCE UPON A MIDNIGHT DREARY"),
	}
}
```

| function | gives |
|---|---|
| `options()` *(optional)* | the chain's options, declared as a lock file declares its own (`integer(3)`, `one_of([...], default)`; see locks/README.md), which `chain(name, options)` gives |
| `chain(o)` | `locks` (each id's `type` and `options`), `connect` (`[output, input]` pairs), `fixed` (values for inputs, by port) and `illustrations` (each id's `type`, `options` and `inputs`, each illustration input -> the port it draws) |
| `layout()` *(optional)* | where the lock editor draws each lock and illustration |
| `example()` *(optional)* | a value for every open input, by port: the editor's test case, which `journalbuild --check-locks` and `cargo test` lock and unlock, needing the inputs back |

A chain file runs in the lock files' engine, so it writes values as lock
files do: `symbols(27, "HI")` is an array, and value kinds' functions
(`huffman::code`) work. Its illustrations only load when a journal uses it,
in the journal's engine.

To add one to the repository, put it here and use it in the showcase
(`tests/showcase.rs` checks); the web editor needs the illustration rebuilt
(`npm run wasm`) to have it compiled in.
