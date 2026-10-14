// A chain written as a composite lock file (locks/README.md), and as
// journal script.

import { rhaiKey, rhaiString, rhaiValue } from '../shared/rhai-literal';
import type { LockOption } from '../scribe';
import { openInputs, openOutputs, portType, symbolBase, type ChainModel, type ChainNode, type ChainOption, type PortsByNode, type TestValue } from './chain-model';

/** What a lock file's options() writes for an option: `integer(3)`, `one_of(["a", "b"], "a")`. */
export function optionDeclaration(option: ChainOption): string {
  const constructor = option.kind;
  const value = (v: unknown) => rhaiValue(v, option.kind === 'number');
  const has = option.default !== undefined;
  if (option.kind === 'one_of') {
    const choices = `[${(option.choices ?? []).map((c) => rhaiValue(c)).join(', ')}]`;
    return has ? `one_of(${choices}, ${value(option.default)})` : `one_of(${choices})`;
  }
  return has ? `${constructor}(${value(option.default)})` : `${constructor}()`;
}

/** An option's value as rhai, as its kind wants it (a number as a float). */
function optionValue(value: unknown, kind: LockOption['kind'] | undefined): string {
  return rhaiValue(value, kind === 'number');
}

/** A test or fixed value as rhai, for a port of `type`. */
export function valueSource(value: TestValue, type: string | null): string {
  if ('text' in value) {
    const base = type ? symbolBase(type) : null;
    return base !== null ? `symbols(${base}, ${rhaiString(value.text)})` : rhaiString(value.text);
  }
  return rhaiValue(value.json);
}

/** A rhai map literal on one line, entries as given. */
function mapLiteral(entries: [string, string][]): string {
  return entries.length ? `#{ ${entries.map(([k, v]) => `${k}: ${v}`).join(', ')} }` : '#{}';
}

/** `// text`, wrapped to lines of at most `width` columns. */
function comment(text: string, width = 76): string[] {
  const lines: string[] = [];
  let line = '//';
  for (const word of text.split(/\s+/).filter(Boolean)) {
    if (line !== '//' && line.length + 1 + word.length > width) {
      lines.push(line);
      line = '//';
    }
    line += ` ${word}`;
  }
  lines.push(line);
  return lines;
}

/** A lock's options as chain(o) gives them: its own values, and `o.name` for those it takes from the chain. */
function nodeOptions(node: ChainNode, kinds: Record<string, LockOption['kind']>): string {
  const names = [...new Set([...Object.keys(node.options), ...Object.keys(node.promoted)])].sort();
  return mapLiteral(names.map((name) => [
    rhaiKey(name),
    name in node.promoted ? `o.${node.promoted[name]}` : optionValue(node.options[name], kinds[name]),
  ]));
}

export interface SourceContext {
  /** Each lock's ports, as the renderer gives them. */
  ports: PortsByNode;
  /** Each lock type's options, for how to write their values. */
  optionKinds: (type: string) => Record<string, LockOption['kind']>;
  /** The test case's outputs to pin in examples(), by the chain's output names; none to pin nothing. */
  outputs?: Record<string, TestValue>;
  /** Whether to write examples() (from the test case) at all. */
  examples?: boolean;
}

/** Why the test case can't be an example, or null if it can. */
export function exampleProblem(model: ChainModel, ports: PortsByNode): string | null {
  for (const input of openInputs(model, ports)) {
    const value = model.test[input.name];
    if (!value) return `the test has no value for '${input.name}'`;
    if (input.type === 'Maze') return `'${input.name}' takes a maze, which an example can't write`;
  }
  return null;
}

/** The chain as a composite lock file. */
export function chainSource(model: ChainModel, context: SourceContext): string {
  const { ports } = context;
  const out: string[] = [];
  out.push(...comment(model.description ? `${model.name}: ${model.description}` : `${model.name}: a composite lock.`));
  out.push('');
  out.push('fn options() {');
  out.push(`\t${mapLiteral(model.options.map((o) => [rhaiKey(o.name), optionDeclaration(o)]))}`);
  out.push('}');
  out.push('');
  out.push('fn chain(o) {');
  out.push('\t#{');
  const nodes = [...model.nodes].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  out.push('\t\tlocks: #{');
  for (const node of nodes) {
    out.push(`\t\t\t${rhaiKey(node.id)}: #{ type: ${rhaiString(node.type)}, options: ${nodeOptions(node, context.optionKinds(node.type))} },`);
  }
  out.push('\t\t},');
  const wires = [...model.wires].sort((a, b) => (a.from + a.to < b.from + b.to ? -1 : 1));
  if (wires.length) {
    out.push('\t\tconnect: [');
    for (const wire of wires) out.push(`\t\t\t[${rhaiString(wire.from)}, ${rhaiString(wire.to)}],`);
    out.push('\t\t],');
  } else {
    out.push('\t\tconnect: [],');
  }
  const fixed = Object.entries(model.fixed).sort(([a], [b]) => (a < b ? -1 : 1));
  out.push(`\t\tfixed: ${mapLiteral(fixed.map(([port, value]) => [rhaiString(port), valueSource(value, portType(ports, port, 'inputs'))]))},`);
  const byName = (a: { name: string }, b: { name: string }) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
  const inputs = openInputs(model, ports).sort(byName);
  const outputs = openOutputs(model, ports).sort(byName);
  out.push(`\t\tinputs: ${mapLiteral(inputs.map((p) => [rhaiKey(p.name), rhaiString(p.port)]))},`);
  out.push(`\t\toutputs: ${mapLiteral(outputs.map((p) => [rhaiKey(p.name), rhaiString(p.port)]))},`);
  out.push('\t}');
  out.push('}');
  out.push('');
  out.push('// Where the web editor draws each lock.');
  out.push('fn layout() {');
  out.push(`\t${mapLiteral(nodes.map((node) => [rhaiKey(node.id), `[${Math.round(node.x)}, ${Math.round(node.y)}]`]))}`);
  out.push('}');
  if (context.examples !== false && exampleProblem(model, ports) === null) {
    out.push('');
    out.push('fn examples() {');
    out.push('\t[');
    out.push('\t\t#{');
    const options = model.options.filter((o) => o.default !== undefined);
    if (options.length) out.push(`\t\t\toptions: ${mapLiteral(options.map((o) => [rhaiKey(o.name), optionValue(o.default, o.kind)]))},`);
    out.push(`\t\t\tinputs: ${mapLiteral(inputs.map((p) => [rhaiKey(p.name), valueSource(model.test[p.name], p.type)]))},`);
    const pinned = outputs.filter((p) => context.outputs?.[p.name]);
    if (pinned.length) out.push(`\t\t\toutputs: ${mapLiteral(pinned.map((p) => [rhaiKey(p.name), valueSource(context.outputs![p.name], p.type)]))},`);
    out.push('\t\t},');
    out.push('\t]');
    out.push('}');
  }
  return out.join('\n') + '\n';
}

/** A journal script using the saved lock type: added as one lock, run on the test case. */
export function journalSnippet(model: ChainModel, ports: PortsByNode): string {
  const options = model.options.filter((o) => o.default !== undefined);
  const inputs = openInputs(model, ports);
  const outputs = openOutputs(model, ports);
  const lines = [
    `let lock = new_lock(${rhaiString(model.name)}).add("c", ${rhaiString(model.name)}, ${mapLiteral(options.map((o) => [rhaiKey(o.name), optionValue(o.default, o.kind)]))});`,
    `let result = lock.run(${mapLiteral(inputs.map((p) => [rhaiString(`c.${p.name}`), model.test[p.name] ? valueSource(model.test[p.name], p.type) : `/* ${p.type} */ ()`]))});`,
    ...outputs.map((p) => `// result.get(${rhaiString(`c.${p.name}`)}): ${p.type}`),
  ];
  return lines.join('\n') + '\n';
}

/** A journal script building the same chain lock by lock, with no lock file of its own. */
export function journalChain(model: ChainModel, context: SourceContext): string {
  const lines = [`let lock = new_lock(${rhaiString(model.name)})`];
  for (const node of model.nodes) {
    const kinds = context.optionKinds(node.type);
    const options = Object.entries({ ...node.options, ...Object.fromEntries(Object.entries(node.promoted).map(([k, v]) => [k, model.options.find((o) => o.name === v)?.default])) })
      .sort(([a], [b]) => (a < b ? -1 : 1));
    lines.push(`\t.add(${rhaiString(node.id)}, ${rhaiString(node.type)}, ${mapLiteral(options.map(([k, v]) => [rhaiKey(k), optionValue(v, kinds[k])]))})`);
  }
  for (const wire of model.wires) lines.push(`\t.connect(${rhaiString(wire.from)}, ${rhaiString(wire.to)})`);
  for (const [port, value] of Object.entries(model.fixed)) {
    lines.push(`\t.set_input(${rhaiString(port)}, ${valueSource(value, portType(context.ports, port, 'inputs'))})`);
  }
  lines[lines.length - 1] += ';';
  const inputs = openInputs(model, context.ports);
  lines.push(`let result = lock.run(${mapLiteral(inputs.map((p) => [rhaiString(p.port), model.test[p.name] ? valueSource(model.test[p.name], p.type) : `/* ${p.type} */ ()`]))});`);
  return lines.join('\n') + '\n';
}
