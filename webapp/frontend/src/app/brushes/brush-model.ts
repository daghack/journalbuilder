// A brush as the brush editor changes it: a brush file (Darkly's portable
// brush, see the repository's src/journal/brushes.rs) as JSON, and the node
// types Darkly makes brushes of. Each change gives a new brush; nothing here
// changes the one it's given.

import type { BrushFile, BrushFileNode, BrushNodeType, BrushPortDef, BrushValue, WireType } from '../scribe';

/** A wire: from a node's output to another's input. */
export interface Wire {
  from: string;
  fromPort: string;
  to: string;
  toPort: string;
}

/** A wire as a brush file writes it: "node.port -> node.port". */
export function wireText(wire: Wire): string {
  return `${wire.from}.${wire.fromPort} -> ${wire.to}.${wire.toPort}`;
}

/** A wire in a brush file; null if it isn't one. */
export function parseWire(text: string): Wire | null {
  const match = /^\s*([^.\s]+)\.(\S+)\s*->\s*([^.\s]+)\.(\S+)\s*$/.exec(text);
  return match ? { from: match[1], fromPort: match[2], to: match[3], toPort: match[4] } : null;
}

/** Every wire of `brush`. */
export function wires(brush: BrushFile): Wire[] {
  return (brush.connections ?? []).map(parseWire).filter((w): w is Wire => w !== null);
}

/** Whether a wire can carry `from` into `to`: the same, or a number into a whole number and back, as Darkly has it. */
export function compatible(from: WireType, to: WireType): boolean {
  return from === to || (from === 'Scalar' && to === 'Int') || (from === 'Int' && to === 'Scalar');
}

/** The node types, by type. */
export class NodeTypes {
  private readonly byId: Map<string, BrushNodeType>;

  constructor(readonly all: BrushNodeType[]) {
    this.byId = new Map(all.map((t) => [t.type_id, t]));
  }

  get(type: string): BrushNodeType | undefined {
    return this.byId.get(type);
  }

  /** Node type `type`'s port `name` of direction `dir`. */
  port(type: string, name: string, dir: 'Input' | 'Output'): BrushPortDef | undefined {
    return this.get(type)?.ports.find((p) => p.name === name && p.dir === dir);
  }

  /** The categories, in the order a palette shows them, each with its types. */
  categories(): { category: string; types: BrushNodeType[] }[] {
    const order = ['input', 'math', 'modulate', 'color', 'shape', 'texture', 'output'];
    const groups = new Map<string, BrushNodeType[]>();
    for (const t of this.all) groups.set(t.category, [...(groups.get(t.category) ?? []), t]);
    const rank = (c: string) => (order.includes(c) ? order.indexOf(c) : order.length);
    return [...groups.entries()]
      .sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b))
      .map(([category, types]) => ({ category, types: types.sort((a, b) => a.display_name.localeCompare(b.display_name)) }));
  }
}

/** Whether `a` and `b` are the same value. */
export function sameValue(a: unknown, b: unknown): boolean {
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((v, i) => sameValue(v, b[i]));
  if (typeof a === 'boolean' || typeof b === 'boolean') return Boolean(a) === Boolean(b) && typeof a === typeof b;
  return a === b;
}

/** Node `id`'s input `port`: what the brush sets it to, or its type's default. */
export function inputValue(brush: BrushFile, types: NodeTypes, id: string, port: string): BrushValue | undefined {
  const node = brush.nodes[id];
  if (!node) return undefined;
  return node.inputs?.[port] ?? types.port(node.type, port, 'Input')?.value;
}

/** A toggle's value as true or false, whether it's kept as a boolean or a number. */
export function truthy(value: unknown): boolean {
  return typeof value === 'boolean' ? value : Number(value) >= 0.5;
}

/** Whether input `port` of node `id` shows, as its type's visible_when says. */
export function inputShows(brush: BrushFile, types: NodeTypes, id: string, def: BrushPortDef): boolean {
  if (!def.visible_when) return true;
  const [other, values] = def.visible_when;
  const value = inputValue(brush, types, id, other);
  const n = typeof value === 'boolean' ? (value ? 1 : 0) : Number(value);
  return values.includes(n);
}

/** A slider's range for input `def` of node `id`: the brush's own, else the port's natural range, else its bounds. */
export function inputRange(brush: BrushFile, id: string, def: BrushPortDef): [number, number] {
  return brush.nodes[id]?.ranges?.[def.name] ?? def.natural_range ?? [def.min, def.max];
}

/** How an input of `unit` shows its value: as a percentage, in degrees, or as it is. */
export function toDisplay(unit: BrushPortDef['unit_type'], value: number): number {
  return unit === 'Percent' ? value * 100 : unit === 'Degrees' ? (value * 180) / Math.PI : value;
}

export function fromDisplay(unit: BrushPortDef['unit_type'], value: number): number {
  return unit === 'Percent' ? value / 100 : unit === 'Degrees' ? (value * Math.PI) / 180 : value;
}

export function unitSuffix(unit: BrushPortDef['unit_type']): string {
  return unit === 'Percent' ? '%' : unit === 'Degrees' ? '°' : unit === 'Pixels' ? 'px' : '';
}

/** A new node's id: its type's, or with _2, _3... after it once that's taken, as Darkly names them. */
export function newNodeId(brush: BrushFile, type: string): string {
  if (!(type in brush.nodes)) return type;
  for (let n = 2; ; n++) if (!(`${type}_${n}` in brush.nodes)) return `${type}_${n}`;
}

/** The brush with a node of `type` added, its type's brush-bar ports exposed as Darkly exposes them; and its id. */
export function addNode(brush: BrushFile, types: NodeTypes, type: string): { brush: BrushFile; id: string } {
  const id = newNodeId(brush, type);
  const exposed = { ...brush.exposed_ports };
  for (const port of types.get(type)?.ports ?? []) {
    if (port.dir === 'Input' && port.exposed) exposed[`${id}.${port.name}`] = {};
  }
  const next: BrushFile = { ...brush, nodes: { ...brush.nodes, [id]: { type } } };
  if (Object.keys(exposed).length) next.exposed_ports = exposed;
  return { brush: next, id };
}

/** The brush without node `id`, its wires or its exposed ports. */
export function removeNode(brush: BrushFile, id: string): BrushFile {
  const { [id]: _, ...nodes } = brush.nodes;
  const next: BrushFile = { ...brush, nodes };
  next.connections = wires(brush).filter((w) => w.from !== id && w.to !== id).map(wireText);
  next.exposed_ports = Object.fromEntries(Object.entries(brush.exposed_ports ?? {}).filter(([key]) => !key.startsWith(`${id}.`)));
  return tidy(next);
}

/** Why `wire` can't be made in `brush`, or null if it can. */
export function wireProblem(brush: BrushFile, types: NodeTypes, wire: Wire): string | null {
  if (wire.from === wire.to) return "a node can't be wired to itself";
  const from = brush.nodes[wire.from];
  const to = brush.nodes[wire.to];
  if (!from || !to) return 'there is no such node';
  const out = types.port(from.type, wire.fromPort, 'Output');
  const into = types.port(to.type, wire.toPort, 'Input');
  if (!out) return `${wire.from} has no output ${wire.fromPort}`;
  if (!into) return `${wire.to} has no input ${wire.toPort}`;
  if (!into.wirable) return `${wire.to}.${wire.toPort} is set on the node, not wired`;
  if (!compatible(out.wire_type, into.wire_type)) return `${wire.from}.${wire.fromPort} gives ${label(out.wire_type)}, and ${wire.to}.${wire.toPort} takes ${label(into.wire_type)}`;
  if (reaches(brush, wire.to, wire.from)) return 'that would make a loop';
  return null;
}

/** What a wire type carries, said plainly. */
export function label(type: WireType): string {
  return { Scalar: 'a number', Int: 'a whole number', Bool: 'a toggle', Enum: 'a choice', String: 'text', Curve: 'a curve', Vec2: 'a point', Vec4: 'a colour' }[type];
}

/** Whether wires lead from node `from` to node `to`. */
function reaches(brush: BrushFile, from: string, to: string): boolean {
  const all = wires(brush);
  const seen = new Set<string>();
  const stack = [from];
  while (stack.length) {
    const id = stack.pop()!;
    if (id === to) return true;
    if (seen.has(id)) continue;
    seen.add(id);
    for (const w of all) if (w.from === id) stack.push(w.to);
  }
  return false;
}

/** The brush with `wire` made, replacing what was wired into its input; or why it can't be. */
export function connect(brush: BrushFile, types: NodeTypes, wire: Wire): BrushFile | string {
  const problem = wireProblem(brush, types, wire);
  if (problem) return problem;
  const kept = wires(brush).filter((w) => !(w.to === wire.to && w.toPort === wire.toPort));
  return { ...brush, connections: [...kept, wire].map(wireText) };
}

/** The brush with nothing wired into node `id`'s input `port`. */
export function disconnect(brush: BrushFile, id: string, port: string): BrushFile {
  return tidy({ ...brush, connections: wires(brush).filter((w) => !(w.to === id && w.toPort === port)).map(wireText) });
}

/** What's wired into node `id`'s input `port`, if anything is. */
export function wiredInto(brush: BrushFile, id: string, port: string): Wire | undefined {
  return wires(brush).find((w) => w.to === id && w.toPort === port);
}

/** The brush with node `id`'s input `port` set to `value`: kept only while it isn't its type's default. */
export function setInput(brush: BrushFile, types: NodeTypes, id: string, port: string, value: BrushValue): BrushFile {
  const node = brush.nodes[id];
  if (!node) return brush;
  const def = types.port(node.type, port, 'Input');
  const { [port]: _, ...rest } = node.inputs ?? {};
  const inputs = def && sameValue(value, def.value) ? rest : { ...rest, [port]: value };
  const changed: BrushFileNode = { ...node, inputs };
  if (!Object.keys(inputs).length) delete changed.inputs;
  return { ...brush, nodes: { ...brush.nodes, [id]: changed } };
}

/** Whether node `id`'s input `port` is on the brush bar. */
export function isExposed(brush: BrushFile, id: string, port: string): boolean {
  return `${id}.${port}` in (brush.exposed_ports ?? {});
}

/** The brush with node `id`'s input `port` put on the brush bar (at its end), or taken off it. */
export function toggleExposed(brush: BrushFile, id: string, port: string): BrushFile {
  const key = `${id}.${port}`;
  const exposed = { ...brush.exposed_ports };
  if (key in exposed) delete exposed[key];
  else exposed[key] = {};
  return tidy({ ...brush, exposed_ports: exposed });
}

/** The brush with node `id` given a name of its own (shown in place of its type's), or none. */
export function nameNode(brush: BrushFile, id: string, name: string): BrushFile {
  const node = brush.nodes[id];
  if (!node) return brush;
  const changed: BrushFileNode = { ...node, name: name.trim() };
  if (!changed.name) delete changed.name;
  return { ...brush, nodes: { ...brush.nodes, [id]: changed } };
}

/** The brush with `key` (name, description, author) set, left out if it's empty. */
export function setAbout(brush: BrushFile, key: 'name' | 'description' | 'author', value: string): BrushFile {
  const next: BrushFile = { ...brush, [key]: value };
  if (!value.trim()) delete next[key];
  return next;
}

/** `brush` without empty lists, as a file leaves them out. */
function tidy(brush: BrushFile): BrushFile {
  const next = { ...brush };
  if (!next.connections?.length) delete next.connections;
  if (!Object.keys(next.exposed_ports ?? {}).length) delete next.exposed_ports;
  return next;
}

// ---- how a node is laid out, for the graph and its wires ----

/** A node's width, its head's height, a port row's, and a curve's. */
export const NODE_WIDTH = 240;
export const HEAD = 30;
export const ROW = 24;
export const CURVE_ROW = 132;

/** A row of a node: an output, or an input with the widget it's set with. */
export interface NodeRow {
  dir: 'Input' | 'Output';
  def: BrushPortDef;
  /** Its top, from the node's. */
  y: number;
  height: number;
}

/** Node `id`'s rows, outputs first, then the inputs that show; and its height. */
export function nodeRows(brush: BrushFile, types: NodeTypes, id: string): { rows: NodeRow[]; height: number } {
  const node = brush.nodes[id];
  const type = node ? types.get(node.type) : undefined;
  const rows: NodeRow[] = [];
  let y = HEAD + 4;
  for (const def of type?.ports.filter((p) => p.dir === 'Output') ?? []) {
    rows.push({ dir: 'Output', def, y, height: ROW });
    y += ROW;
  }
  for (const def of type?.ports.filter((p) => p.dir === 'Input') ?? []) {
    if (!inputShows(brush, types, id, def)) continue;
    const height = def.wire_type === 'Curve' && !wiredInto(brush, id, def.name) ? CURVE_ROW : ROW;
    rows.push({ dir: 'Input', def, y, height });
    y += height;
  }
  return { rows, height: y + 6 };
}

/** Where node `id`'s port's dot is, from the node's top left. */
export function portOffset(brush: BrushFile, types: NodeTypes, id: string, port: string, dir: 'Input' | 'Output'): { x: number; y: number } | null {
  const row = nodeRows(brush, types, id).rows.find((r) => r.dir === dir && r.def.name === port);
  return row ? { x: dir === 'Input' ? 0 : NODE_WIDTH, y: row.y + ROW / 2 } : null;
}

export interface Point {
  x: number;
  y: number;
}

/**
 * Where each node goes: in columns from the pen's inputs on the left to
 * what paints on the right, each node a column past the nodes wired into
 * it, and down each column in the order of what feeds it.
 */
export function layout(brush: BrushFile, types: NodeTypes): Record<string, Point> {
  const ids = Object.keys(brush.nodes);
  const all = wires(brush);
  const column = new Map<string, number>();
  const columnOf = (id: string, seen: Set<string>): number => {
    if (column.has(id)) return column.get(id)!;
    if (seen.has(id)) return 0;
    seen.add(id);
    const feeds = all.filter((w) => w.to === id && w.from in brush.nodes);
    const c = feeds.length ? Math.max(...feeds.map((w) => columnOf(w.from, seen) + 1)) : 0;
    column.set(id, c);
    return c;
  };
  for (const id of ids) columnOf(id, new Set());
  // Nodes nothing is wired out of, nor into, sit in the first column; terminals in the last.
  const last = Math.max(0, ...column.values());
  for (const id of ids) {
    if (types.get(brush.nodes[id].type)?.is_terminal) column.set(id, Math.max(column.get(id)!, last));
  }
  const columns: string[][] = [];
  for (const id of ids) (columns[column.get(id)!] ??= []).push(id);
  const at: Record<string, Point> = {};
  const GAP_X = 90;
  const GAP_Y = 24;
  columns.forEach((members, c) => {
    // In the order of the rows they're wired into (or out of), so wires cross less.
    const order = (id: string) => {
      const out = all.filter((w) => w.from === id && at[w.to]).map((w) => at[w.to].y);
      const into = all.filter((w) => w.to === id && at[w.from]).map((w) => at[w.from].y);
      const ys = [...into, ...out];
      return ys.length ? ys.reduce((a, b) => a + b, 0) / ys.length : Number.MAX_SAFE_INTEGER;
    };
    members.sort((a, b) => order(a) - order(b) || a.localeCompare(b));
    let y = 0;
    for (const id of members) {
      at[id] = { x: c * (NODE_WIDTH + GAP_X), y };
      y += nodeRows(brush, types, id).height + GAP_Y;
    }
  });
  return at;
}
