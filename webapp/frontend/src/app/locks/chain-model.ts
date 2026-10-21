// A lock chain as the lock editor holds it: lock types placed as nodes,
// wires from outputs to inputs, values fixed, options of its own and a test
// case; and renderers attached to it, which draw its values. It's a chain
// file (see chain-source.ts and webapp/chains/README.md), which journals use
// with chain(name); its values are its locks' own ports, "lock.port".

import type { LockOption, LockPorts } from '../scribe';

/** A lock in the chain. */
export interface ChainNode {
  /** Its name in the chain, as wires write it ("shift" in "shift.cipher_text"). */
  id: string;
  type: string;
  /** Options given a value, by name. */
  options: Record<string, unknown>;
  /** Options that take one of the chain's own: option -> the chain's option. */
  promoted: Record<string, string>;
  x: number;
  y: number;
}

/**
 * A renderer attached to the chain: a renderers/*.rhai file that draws
 * values wired to its inputs as canvases. It has no outputs, and isn't part
 * of the lock: the values it draws stay the chain's.
 */
export interface RendererNode {
  id: string;
  type: string;
  /** Options given a value (over the renderer's defaults), by name. */
  options: Record<string, unknown>;
  /** Options that take one of the chain's own: option -> the chain's option. */
  promoted: Record<string, string>;
  x: number;
  y: number;
}

/** A wire: an output ("lock.port") feeding an input. */
export interface Wire {
  from: string;
  to: string;
}

/**
 * A test value: text for symbols or strings, JSON for anything, or rhai
 * that makes it in the lock files' engine, as a value kind's example does
 * (`huffman::code(symbols(27, "CORPUS"), 27, 3)`).
 */
export type TestValue = { text: string } | { json: unknown } | { rhai: string };

/** A test value as typed: text for symbols and strings, its rhai, else JSON. */
export function testText(value: TestValue | undefined): string {
  if (!value) return '';
  if ('text' in value) return value.text;
  if ('rhai' in value) return value.rhai;
  return JSON.stringify(value.json);
}

/** An option of the chain's own, which its locks' options can take. */
export interface ChainOption {
  name: string;
  kind: LockOption['kind'];
  default?: unknown;
  choices?: unknown[];
}

export interface ChainModel {
  /** The chain file it's saved as, and what journals call it: chain("name"). */
  name: string;
  description: string;
  options: ChainOption[];
  nodes: ChainNode[];
  wires: Wire[];
  /** Renderers attached, and the wires from locks' outputs to their inputs ("pages.chart"). */
  renderers: RendererNode[];
  renderWires: Wire[];
  /** Inputs given a value in the chain itself, by port. */
  fixed: Record<string, TestValue>;
  /** The test case's inputs, by port. */
  test: Record<string, TestValue>;
}

export function emptyChain(name = 'my_chain'): ChainModel {
  return { name, description: '', options: [], nodes: [], wires: [], renderers: [], renderWires: [], fixed: {}, test: {} };
}

/** A port of the chain open for the outside, "lock.port", and its type. */
export interface OpenPort {
  port: string;
  type: string;
}

/** Each lock's ports, as the renderer gives them (or why it can't). */
export type PortsByNode = Record<string, LockPorts | { error: string }>;

export const portsOf = (ports: PortsByNode, id: string): LockPorts | null => {
  const p = ports[id];
  return p && !('error' in p) ? p : null;
};

/** Whether a value of type `a` can go where `b` is wanted: the same type, or either is Any. */
export function compatible(a: string, b: string): boolean {
  return a === b || a === 'Any' || b === 'Any';
}

/** A symbol type's base, e.g. 27 for "Base27Symbol"; null for other types. */
export function symbolBase(type: string): number | null {
  const match = /^Base(\d+)Symbol$/.exec(type);
  return match ? Number(match[1]) : null;
}

/** The type of port "lock.port", input or output. */
export function portType(ports: PortsByNode, port: string, side: 'inputs' | 'outputs'): string | null {
  const [id, name] = splitPort(port);
  return portsOf(ports, id)?.[side].find(([n]) => n === name)?.[1] ?? null;
}

export function splitPort(port: string): [string, string] {
  const dot = port.indexOf('.');
  return dot < 0 ? [port, ''] : [port.slice(0, dot), port.slice(dot + 1)];
}

/** The inputs no wire feeds and no value's fixed for: what running the chain needs. */
export function openInputs(model: ChainModel, ports: PortsByNode): OpenPort[] {
  const wired = new Set(model.wires.map((w) => w.to));
  return model.nodes.flatMap((node) =>
    (portsOf(ports, node.id)?.inputs ?? [])
      .map(([name, type]) => ({ port: `${node.id}.${name}`, type }))
      .filter(({ port }) => !wired.has(port) && !(port in model.fixed)));
}

/**
 * The outputs no wire takes: what running the chain gives. An output
 * passing a fixed input straight through isn't shown.
 */
export function openOutputs(model: ChainModel, ports: PortsByNode): OpenPort[] {
  const wired = new Set(model.wires.map((w) => w.from));
  return model.nodes.flatMap((node) =>
    (portsOf(ports, node.id)?.outputs ?? [])
      .map(([name, type]) => ({ port: `${node.id}.${name}`, type }))
      .filter(({ port, type }) => !wired.has(port) && !(port in model.fixed && portType(ports, port, 'inputs') === type)));
}

/** The value of an option a lock (or renderer) is given: its own, or (promoted) the chain option's default. */
export function resolvedOptions(model: ChainModel, node: ChainNode | RendererNode): Record<string, unknown> {
  const options = { ...node.options };
  for (const [option, chainOption] of Object.entries(node.promoted)) {
    const found = model.options.find((o) => o.name === chainOption);
    if (found && found.default !== undefined) options[option] = found.default;
  }
  return options;
}

/** Whether wiring `from` to `to` would feed a lock back into itself. */
export function wouldLoop(model: ChainModel, from: string, to: string): boolean {
  const [source] = splitPort(from);
  const [target] = splitPort(to);
  // Does `target` already reach `source`?
  const next = new Map<string, string[]>();
  for (const w of model.wires) {
    const [a] = splitPort(w.from), [b] = splitPort(w.to);
    next.set(a, [...(next.get(a) ?? []), b]);
  }
  const seen = new Set<string>();
  const stack = [target];
  while (stack.length) {
    const id = stack.pop()!;
    if (id === source) return true;
    if (seen.has(id)) continue;
    seen.add(id);
    stack.push(...(next.get(id) ?? []));
  }
  return false;
}

/** Whether `id` is one of the chain's renderers (not a lock). */
export function isRenderer(model: ChainModel, id: string): boolean {
  return model.renderers.some((r) => r.id === id);
}

/** Why `from` can't be wired to `to`, or null if it can. */
export function wireProblem(model: ChainModel, ports: PortsByNode, from: string, to: string): string | null {
  const a = portType(ports, from, 'outputs');
  const b = portType(ports, to, 'inputs');
  if (isRenderer(model, splitPort(from)[0])) return 'a renderer has no outputs to wire';
  if (a === null) return `${from} isn't an output`;
  if (b === null) return `${to} isn't an input`;
  if (!compatible(a, b)) return `${from} gives ${a}, but ${to} takes ${b}`;
  if (!isRenderer(model, splitPort(to)[0]) && wouldLoop(model, from, to)) return `that would feed ${splitPort(to)[0]} back into itself`;
  return null;
}

/** `model` with `from` wired to renderer input `to`, in place of any wire into it. */
export function connectRenderer(model: ChainModel, from: string, to: string): ChainModel {
  return { ...model, renderWires: [...model.renderWires.filter((w) => w.to !== to), { from, to }] };
}

/** What each renderer draws: its input -> the port wired to it. */
export function rendererInputs(model: ChainModel, id: string): Record<string, string> {
  return Object.fromEntries(model.renderWires.filter((w) => splitPort(w.to)[0] === id).map((w) => [splitPort(w.to)[1], w.from]));
}

/** `model` with `from` wired to `to`, in place of any wire into `to`. */
export function connect(model: ChainModel, from: string, to: string): ChainModel {
  const fixed = { ...model.fixed };
  delete fixed[to];
  return { ...model, fixed, wires: [...model.wires.filter((w) => w.to !== to), { from, to }] };
}

/** A name for a new lock (or renderer) of `type` not yet in the chain. */
export function freshId(model: ChainModel, type: string): string {
  const base = type.split('_').map((word) => word[0]).join('') || 'lock';
  for (let n = 1; ; n++) {
    const id = n === 1 ? base : `${base}${n}`;
    if (!model.nodes.some((node) => node.id === id) && !model.renderers.some((r) => r.id === id)) return id;
  }
}

/** `model` with lock `id` renamed: its wires, fixed values and test values follow. */
export function renameNode(model: ChainModel, id: string, to: string): ChainModel {
  const port = (p: string) => (splitPort(p)[0] === id ? `${to}.${splitPort(p)[1]}` : p);
  const rekey = <T>(record: Record<string, T>) => Object.fromEntries(Object.entries(record).map(([k, v]) => [port(k), v]));
  return {
    ...model,
    nodes: model.nodes.map((node) => (node.id === id ? { ...node, id: to } : node)),
    wires: model.wires.map((w) => ({ from: port(w.from), to: port(w.to) })),
    renderers: model.renderers.map((r) => (r.id === id ? { ...r, id: to } : r)),
    renderWires: model.renderWires.map((w) => ({ from: port(w.from), to: port(w.to) })),
    fixed: rekey(model.fixed),
    test: rekey(model.test),
  };
}

/** `model` without lock (or renderer) `id` and what touches it. */
export function removeNode(model: ChainModel, id: string): ChainModel {
  const keep = (p: string) => splitPort(p)[0] !== id;
  const filter = <T>(record: Record<string, T>) => Object.fromEntries(Object.entries(record).filter(([k]) => keep(k)));
  return {
    ...model,
    nodes: model.nodes.filter((node) => node.id !== id),
    wires: model.wires.filter((w) => keep(w.from) && keep(w.to)),
    renderers: model.renderers.filter((r) => r.id !== id),
    renderWires: model.renderWires.filter((w) => keep(w.from) && keep(w.to)),
    fixed: filter(model.fixed),
    test: filter(model.test),
  };
}

// ---- symbols, as lockbuilder writes and reads them ----

/** Text as symbols in `base`: base 10 digits; base 27 space and A-Z; other bases A on. Throws on what isn't one. */
export function textSymbols(base: number, text: string): number[] {
  return [...text].map((c) => {
    const upper = c.toUpperCase();
    const letter = upper >= 'A' && upper <= 'Z' && upper.length === 1 ? upper.charCodeAt(0) - 65 : -1;
    let symbol: number | null = null;
    if (base === 10) symbol = /^\d$/.test(c) ? Number(c) : null;
    else if (base === 27) symbol = c === ' ' ? 0 : letter >= 0 ? letter + 1 : null;
    else symbol = letter >= 0 && letter < base ? letter : null;
    if (symbol === null) throw new Error(`'${c}' isn't a base-${base} symbol`);
    return symbol;
  });
}

/** Symbols in `base` as text, the reverse of textSymbols. */
export function symbolsText(base: number, symbols: number[]): string {
  if (base === 10) return symbols.join('');
  if (base === 27) return symbols.map((s) => (s === 0 ? ' ' : String.fromCharCode(64 + s))).join('');
  return symbols.map((s) => String.fromCharCode(65 + s)).join('');
}

/** A value read back from a lock file (JSON) as a test value of `type`: symbols as text, when they have it. */
export function testValueOf(json: unknown, type: string | null): TestValue {
  const base = type ? symbolBase(type) : null;
  if (base !== null && base <= 27 && Array.isArray(json) && json.every((s) => Number.isInteger(s) && s >= 0 && s < base)) {
    return { text: symbolsText(base, json as number[]) };
  }
  if (type === 'String' && typeof json === 'string') return { text: json };
  return { json };
}
