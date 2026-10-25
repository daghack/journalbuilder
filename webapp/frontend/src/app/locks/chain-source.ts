// A chain written as a chain file (webapp/library/chains/README.md), and as
// journal script.

import { rhaiKey, rhaiString, rhaiValue } from '../shared/rhai-literal';
import type { LockOption } from '../scribe';
import { openInputs, openOutputs, portType, rendererInputs, symbolBase, type ChainModel, type ChainNode, type ChainOption, type PortsByNode, type RendererNode, type SourceNode, type TestValue } from './chain-model';

/** What a chain file's options() writes for an option: `integer(3)`, `one_of(["a", "b"], "a")`. */
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
  if ('rhai' in value) return value.rhai;
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

const byId = <T extends { id: string }>(a: T, b: T) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);

/** A lock's or renderer's options as chain(o) gives them: its own values, and `o.name` for those it takes from the chain. */
function nodeOptions(node: ChainNode | RendererNode, kinds: Record<string, LockOption['kind']>): string {
  const names = [...new Set([...Object.keys(node.options), ...Object.keys(node.promoted)])].sort();
  return mapLiteral(names.map((name) => [
    rhaiKey(name),
    name in node.promoted ? `o.${node.promoted[name]}` : optionValue(node.options[name], kinds[name]),
  ]));
}

/** A source's settings besides its type and name: a style's or layout's options, text content's base. */
function sourceSettings(source: SourceNode): [string, string][] {
  const entries: [string, string][] = [];
  if ((source.kind === 'style' || source.kind === 'layout') && Object.keys(source.options).length) {
    const options = Object.entries(source.options).sort(([a], [b]) => (a < b ? -1 : 1));
    entries.push(['options', mapLiteral(options.map(([k, v]) => [rhaiKey(k), rhaiValue(v, typeof v === 'number' && !Number.isInteger(v))]))]);
  }
  if (source.kind === 'content' && source.base !== undefined) entries.push(['base', String(source.base)]);
  return entries;
}

/** A source as a chain file's sources write it: #{ type, name, options or base }. */
export function sourceLiteral(source: SourceNode): string {
  return mapLiteral([['type', rhaiString(source.kind)], ['name', rhaiString(source.name)], ...sourceSettings(source)]);
}

/** A source's settings as lock.source takes them, after its name: `, #{ options, base }`, or nothing. */
function sourceOptions(source: SourceNode): string {
  const settings = sourceSettings(source);
  return settings.length ? `, ${mapLiteral(settings)}` : '';
}

export interface SourceContext {
  /** Each lock's ports, as the renderer gives them. */
  ports: PortsByNode;
  /** Each lock type's options, for how to write their values. */
  optionKinds: (type: string) => Record<string, LockOption['kind']>;
}

/** Whether every input running the chain needs has a test value, so it can be the file's example(). */
export function hasExample(model: ChainModel, ports: PortsByNode): boolean {
  return openInputs(model, ports).every((input) => model.test[input.port]);
}

/** The chain as a chain file. */
export function chainSource(model: ChainModel, context: SourceContext): string {
  const { ports } = context;
  const out: string[] = [];
  out.push(...comment(model.description ? `${model.name}: ${model.description}` : `${model.name}: a chain of locks.`));
  if (model.options.length) {
    out.push('');
    out.push('fn options() {');
    out.push(`\t${mapLiteral(model.options.map((o) => [rhaiKey(o.name), optionDeclaration(o)]))}`);
    out.push('}');
  }
  out.push('');
  out.push('fn chain(o) {');
  out.push('\t#{');
  const sources = [...model.sources].sort(byId);
  if (sources.length) {
    out.push('\t\tsources: #{');
    for (const source of sources) out.push(`\t\t\t${rhaiKey(source.id)}: ${sourceLiteral(source)},`);
    out.push('\t\t},');
  }
  const nodes = [...model.nodes].sort(byId);
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
  const renderers = [...model.renderers].sort(byId);
  if (renderers.length) {
    out.push('\t\tillustrations: #{');
    for (const renderer of renderers) {
      const inputs = Object.entries(rendererInputs(model, renderer.id)).sort(([a], [b]) => (a < b ? -1 : 1));
      out.push(`\t\t\t${rhaiKey(renderer.id)}: #{ type: ${rhaiString(renderer.type)}, options: ${nodeOptions(renderer, {})}, inputs: ${mapLiteral(inputs.map(([k, v]) => [rhaiKey(k), rhaiString(v)]))} },`);
    }
    out.push('\t\t},');
  }
  out.push('\t}');
  out.push('}');
  out.push('');
  out.push(renderers.length ? '// Where the web editor draws each lock and illustration.' : '// Where the web editor draws each lock.');
  out.push('fn layout() {');
  out.push(`\t${mapLiteral([...sources, ...nodes, ...renderers].map((node) => [rhaiKey(node.id), `[${Math.round(node.x)}, ${Math.round(node.y)}]`]))}`);
  out.push('}');
  const inputs = openInputs(model, ports).sort((a, b) => (a.port < b.port ? -1 : 1));
  if (inputs.length && hasExample(model, ports)) {
    out.push('');
    out.push('fn example() {');
    out.push('\t#{');
    for (const input of inputs) out.push(`\t\t${rhaiString(input.port)}: ${valueSource(model.test[input.port], input.type)},`);
    out.push('\t}');
    out.push('}');
  }
  return out.join('\n') + '\n';
}

/** The chain's options as a journal gives them to chain(name, options): their defaults. */
function chainOptions(model: ChainModel): string {
  return mapLiteral(model.options.filter((o) => o.default !== undefined).map((o) => [rhaiKey(o.name), optionValue(o.default, o.kind)]));
}

/** The test case as lock.run takes it, open inputs by port. */
function runInputs(model: ChainModel, ports: PortsByNode): string {
  return mapLiteral(openInputs(model, ports).map((p) => [rhaiString(p.port), model.test[p.port] ? valueSource(model.test[p.port], p.type) : `/* ${p.type} */ ()`]));
}

/** A journal script using the chain: chain(name), run on the test case. */
export function journalSnippet(model: ChainModel, ports: PortsByNode): string {
  const options = model.options.some((o) => o.default !== undefined) ? `, ${chainOptions(model)}` : '';
  const lines = [
    `let c = chain(${rhaiString(model.name)}${options});`,
    `let result = c.run(${runInputs(model, ports)});`,
    ...openOutputs(model, ports).map((p) => `// result.get(${rhaiString(p.port)}): ${p.type}`),
    ...model.renderers.map((r) => `let ${r.id} = result.canvases(${rhaiString(r.id)});  // what ${r.type} draws, an array of canvases`),
  ];
  return lines.join('\n') + '\n';
}

/** A journal script building the same chain lock by lock with new_lock, with no chain file of its own. */
export function journalChain(model: ChainModel, context: SourceContext): string {
  const lines = [`let lock = new_lock(${rhaiString(model.name)})`];
  for (const source of model.sources) {
    lines.push(`\t.source(${rhaiString(source.id)}, ${rhaiString(source.kind)}, ${rhaiString(source.name)}${sourceOptions(source)})`);
  }
  const value = (name: string) => model.options.find((o) => o.name === name)?.default;
  const withDefaults = (node: ChainNode | RendererNode) => ({ ...node.options, ...Object.fromEntries(Object.entries(node.promoted).map(([k, v]) => [k, value(v)])) });
  for (const node of model.nodes) {
    const kinds = context.optionKinds(node.type);
    const options = Object.entries(withDefaults(node)).sort(([a], [b]) => (a < b ? -1 : 1));
    lines.push(`\t.add(${rhaiString(node.id)}, ${rhaiString(node.type)}, ${mapLiteral(options.map(([k, v]) => [rhaiKey(k), optionValue(v, kinds[k])]))})`);
  }
  for (const wire of model.wires) lines.push(`\t.connect(${rhaiString(wire.from)}, ${rhaiString(wire.to)})`);
  for (const [port, fixed] of Object.entries(model.fixed)) {
    lines.push(`\t.set_input(${rhaiString(port)}, ${valueSource(fixed, portType(context.ports, port, 'inputs'))})`);
  }
  for (const renderer of model.renderers) {
    const options = Object.entries(withDefaults(renderer)).sort(([a], [b]) => (a < b ? -1 : 1));
    lines.push(`\t.attach(${rhaiString(renderer.id)}, ${rhaiString(renderer.type)}, ${mapLiteral(options.map(([k, v]) => [rhaiKey(k), rhaiValue(v, false)]))})`);
    for (const wire of model.renderWires.filter((w) => w.to.startsWith(`${renderer.id}.`))) lines.push(`\t.connect(${rhaiString(wire.from)}, ${rhaiString(wire.to)})`);
  }
  lines[lines.length - 1] += ';';
  lines.push(`let result = lock.run(${runInputs(model, context.ports)});`);
  for (const renderer of model.renderers) lines.push(`let ${renderer.id} = result.canvases(${rhaiString(renderer.id)});`);
  return lines.join('\n') + '\n';
}
