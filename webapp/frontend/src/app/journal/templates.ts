// What a new file of each kind starts as: something that works, to change.

import { BLANK_ENTRY, type Kind } from './journal-files';

/** A new file of `kind`, called `name`, with extension `ext`. */
export function template(kind: Kind, name: string, ext: string): string {
  switch (kind) {
    case 'entries':
      return BLANK_ENTRY;
    case 'chains':
      return `// ${name}: what the chain locks, and how.

fn chain(o) {
\t#{
\t\tlocks: #{},
\t\tconnect: [],
\t\tfixed: #{},
\t}
}
`;
    case 'locks':
      return `// ${name}: every symbol moved \`shift\` places along the base, wrapping
// around.

fn options() {
\t#{ base: integer(27), shift: integer(1) }
}

fn inputs(o) {
\t#{ plain_text: symbol_type(o.base) }
}

fn outputs(o) {
\t#{ cipher_text: symbol_type(o.base) }
}

fn lock(o, i) {
\t#{ cipher_text: i.plain_text.map(|s| ((s + o.shift) % o.base + o.base) % o.base) }
}

fn unlock(o, out) {
\t#{ plain_text: out.cipher_text.map(|s| ((s - o.shift) % o.base + o.base) % o.base) }
}

fn examples() {
\t[
\t\t#{ options: #{ base: 27, shift: 1 }, inputs: #{ plain_text: symbols(27, "HAL") }, outputs: #{ cipher_text: symbols(27, "IBM") } },
\t]
}
`;
    case 'helpers':
      return `// ${name}: a text's symbols in reverse order.

fn options() {
\t#{ base: integer(27) }
}

fn inputs(o) {
\t#{ text: symbol_type(o.base) }
}

fn outputs(o) {
\t#{ reversed: symbol_type(o.base) }
}

fn lock(o, i) {
\tlet reversed = i.text;
\treversed.reverse();
\t#{ reversed: reversed }
}

fn unlock(o, out) {
\tlet text = out.reversed;
\ttext.reverse();
\t#{ text: text }
}

fn examples() {
\t[
\t\t#{ options: #{ base: 27 }, inputs: #{ text: symbols(27, "STOP") }, outputs: #{ reversed: symbols(27, "POTS") } },
\t]
}
`;
    case 'illustrations':
      return `// ${name}: what it draws, and how.

fn options() {
\t#{ size: 40.0 }
}

fn inputs(o) {
\t#{ text: "Base27Symbol" }
}

fn render(o, i) {
\tlet c = new_canvas();
\tc.add(write_line(\`\${i.text.len()} symbols\`, vec2(0, 0), #{ style: default_style(), type: new_type_style(glyphset("plain"), o.size) }));
\tc
}

// What the editor previews it with.
fn example() {
\t#{ text: symbols(27, "HELLO").symbols() }
}
`;
    case 'layouts':
      return `// ${name}: a heading over a column of text.
//
// Flows: "title", "body". Roles: "heading", "body".

fn options() {
\t#{ width: 1200.0, height: 1600.0, margin: 100.0 }
}

fn page_size(o) {
\t#{ width: o.width, height: o.height }
}

fn page(n, info) {
\tlet o = info.options;
\tcolumn([
\t\tif n == 0 { flow("title", #{ role: "heading", flex: #{ height: 120 } }) } else { spacer(#{ flex: #{ height: 0 } }) },
\t\tflow("body", #{ flex: #{ grow: 1 } }),
\t], #{ flex: #{ padding: o.margin, gap: 30 } })
}

// What the editor previews it with.
fn example() {
\t#{
\t\tflows: #{ title: "Lorem Ipsum", body: content("lorem") + "\\n\\n" + content("lorem") },
\t\troles: #{ heading: text_style("copper_heading"), body: text_style("plain_hand") },
\t}
}
`;
    case 'styles':
      return `// ${name}: how a kind of text looks.

fn options() {
\t#{ size: 30.0, color: "#2b1d12" }
}

fn style(o) {
\t#{
\t\tstyle: new_style(new_brush_style(o.color, "Ink Pen", 1.4), new_handwriting_style(#{ glyph_rotation_jitter: 0.02 })),
\t\ttype: new_type_style(glyphset("plain"), o.size),
\t\tparagraph: #{ line_height: 1.4 },
\t}
}

// The text the editor previews it with.
fn example() {
\tcontent("lorem")
}
`;
    case 'content':
      return ext === 'json' ? '{ "base": 27, "symbols": [] }\n' : '';
    case 'glyphsets':
      return '{\n\t"space_width": 0.5,\n\t"glyphs": {}\n}\n';
    case 'lib':
      return `// ${name}: shared functions, for import "lib/${name}" as ${name};

fn twice(x) {
\tx * 2
}
`;
  }
}
