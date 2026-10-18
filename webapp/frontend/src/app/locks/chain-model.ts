// A lock chain as the lock editor holds it: lock types placed as nodes,
// wires from outputs to inputs, values fixed, the chain's own names for the
// ports left open, options of its own, and a test case. Saved, it's a
// composite lock file (see chain-source.ts and webapp/locks/README.md).

import type { LockOption, LockPorts, LockValue } from '../scribe';

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

/** A wire: an output ("lock.port") feeding an input. */
export interface Wire {
  from: string;
  to: string;
}

/** A test value: text for symbols or strings, or JSON for anything. */
export type TestValue = { text: string } | { json: unknown };

/** An option of the chain's own, which its locks' options can take. */
export interface ChainOption {
  name: string;
  kind: LockOption['kind'];
  default?: unknown;
  choices?: unknown[];
}

export interface ChainModel {
  /** The lock type it's saved as. */
  name: string;
  description: string;
  options: ChainOption[];
  nodes: ChainNode[];
  wires: Wire[];
  /** Inputs given a value in the chain itself, by port. */
  fixed: Record<string, TestValue>;
  /** The chain's names for open ports, where they aren't the default, by port. */
  inputNames: Record<string, string>;
  outputNames: Record<string, string>;
  /** The test case's inputs, by the chain's input names. */
  test: Record<string, TestValue>;
}

export function emptyChain(): ChainModel {
  return { name: 'my_chain', description: '', options: [], nodes: [], wires: [], fixed: {}, inputNames: {}, outputNames: {}, test: {} };
}

/** A port of the chain open for the outside: its port inside, type and name. */
export interface OpenPort {
  port: string;
  type: string;
  name: string;
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

/** The inputs no wire feeds and no value's fixed for, named as the chain shows them. */
export function openInputs(model: ChainModel, ports: PortsByNode): OpenPort[] {
  const wired = new Set(model.wires.map((w) => w.to));
  const open = model.nodes.flatMap((node) =>
    (portsOf(ports, node.id)?.inputs ?? [])
      .map(([name, type]) => ({ port: `${node.id}.${name}`, type, short: name }))
      .filter(({ port }) => !wired.has(port) && !(port in model.fixed)));
  return named(open, model.inputNames, new Map());
}

/**
 * The outputs no wire takes, named as the chain shows them. An output
 * passing a fixed input straight through isn't shown: unlocking puts it back.
 */
export function openOutputs(model: ChainModel, ports: PortsByNode): OpenPort[] {
  const wired = new Set(model.wires.map((w) => w.from));
  const inputs = openInputs(model, ports);
  const open = model.nodes.flatMap((node) =>
    (portsOf(ports, node.id)?.outputs ?? [])
      .map(([name, type]) => ({ port: `${node.id}.${name}`, type, short: name }))
      .filter(({ port, type }) => !wired.has(port) && !(port in model.fixed && portType(ports, port, 'inputs') === type)));
  // An output may share an input's name only if it's the same port, passed through.
  const taken = new Map(inputs.map((input) => [input.name, input.port]));
  return named(open, model.outputNames, taken);
}

/**
 * Names for ports: the one given, else the port's own name if no other
 * port has it, else "lock_port". `taken` are names in use elsewhere, by
 * the port they're for.
 */
function named(ports: { port: string; type: string; short: string }[], given: Record<string, string>, taken: Map<string, string>): OpenPort[] {
  const count = new Map<string, number>();
  for (const p of ports) count.set(p.short, (count.get(p.short) ?? 0) + 1);
  return ports.map(({ port, type, short }) => {
    let name = given[port];
    if (!name) {
      const clash = taken.has(short) && taken.get(short) !== port;
      name = count.get(short) === 1 && !clash ? short : port.replace('.', '_');
    }
    return { port, type, name };
  });
}

/** The value of an option the lock is given: its own, or (promoted) the chain option's default. */
export function resolvedOptions(model: ChainModel, node: ChainNode): Record<string, unknown> {
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

/** Why `from` can't be wired to `to`, or null if it can. */
export function wireProblem(model: ChainModel, ports: PortsByNode, from: string, to: string): string | null {
  const a = portType(ports, from, 'outputs');
  const b = portType(ports, to, 'inputs');
  if (a === null) return `${from} isn't an output`;
  if (b === null) return `${to} isn't an input`;
  if (!compatible(a, b)) return `${from} gives ${a}, but ${to} takes ${b}`;
  if (wouldLoop(model, from, to)) return `that would feed ${splitPort(to)[0]} back into itself`;
  return null;
}

/** `model` with `from` wired to `to`, in place of any wire into `to`. */
export function connect(model: ChainModel, from: string, to: string): ChainModel {
  const fixed = { ...model.fixed };
  delete fixed[to];
  return { ...model, fixed, wires: [...model.wires.filter((w) => w.to !== to), { from, to }] };
}

/** A name for a new lock of `type` not yet in the chain. */
export function freshId(model: ChainModel, type: string): string {
  const base = type.split('_').map((word) => word[0]).join('') || 'lock';
  for (let n = 1; ; n++) {
    const id = n === 1 ? base : `${base}${n}`;
    if (!model.nodes.some((node) => node.id === id)) return id;
  }
}

/** `model` with lock `id` renamed: its wires, fixed values and names follow. */
export function renameNode(model: ChainModel, id: string, to: string): ChainModel {
  const port = (p: string) => (splitPort(p)[0] === id ? `${to}.${splitPort(p)[1]}` : p);
  const rekey = <T>(record: Record<string, T>) => Object.fromEntries(Object.entries(record).map(([k, v]) => [port(k), v]));
  return {
    ...model,
    nodes: model.nodes.map((node) => (node.id === id ? { ...node, id: to } : node)),
    wires: model.wires.map((w) => ({ from: port(w.from), to: port(w.to) })),
    fixed: rekey(model.fixed),
    inputNames: rekey(model.inputNames),
    outputNames: rekey(model.outputNames),
  };
}

/** `model` without lock `id` and what touches it. */
export function removeNode(model: ChainModel, id: string): ChainModel {
  const keep = (p: string) => splitPort(p)[0] !== id;
  const filter = <T>(record: Record<string, T>) => Object.fromEntries(Object.entries(record).filter(([k]) => keep(k)));
  return {
    ...model,
    nodes: model.nodes.filter((node) => node.id !== id),
    wires: model.wires.filter((w) => keep(w.from) && keep(w.to)),
    fixed: filter(model.fixed),
    inputNames: filter(model.inputNames),
    outputNames: filter(model.outputNames),
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

/** A run's value as a test value, to pin in an example; null for ones that can't be written (mazes). */
export function pinnable(value: LockValue): TestValue | null {
  if (value.maze || (typeof value.json === 'object' && value.json !== null && '$type' in value.json)) return null;
  if (value.text !== undefined && (symbolBase(value.type) !== null || value.type === 'String')) return { text: value.text };
  return { json: value.json };
}
