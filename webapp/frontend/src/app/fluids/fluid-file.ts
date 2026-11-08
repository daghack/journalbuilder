// A fluid file, fluids/NAME.json, as the Fluid tab edits it (see the
// repository's src/journal/fluid.rs): the box it lies over, its effects
// and its frozen field, which scripts read, and the flow simulator's
// scene, the canvases it flows round and the page it was shaped on, which
// only the tab reads.

import type { OptionField } from '../resources/options-form.component';

export const EFFECTS = ['position_distort', 'jitter_distort', 'weight_distort', 'opacity_distort', 'color_distort'] as const;
export type EffectName = (typeof EFFECTS)[number];

export interface FluidDoc {
  /** The box's top-left corner and size, in canvas units; null until fitted to a page. */
  at: [number, number] | null;
  size: [number, number] | null;
  /** The effects that are on, with the settings given (the rest are their defaults). */
  effects: Partial<Record<EffectName, Record<string, unknown>>>;
  /** The encoded field (flowsim's field_json); null until there is one. */
  field: unknown;
  /** The flow simulator's scene (its scene()); null for a new one. */
  scene: unknown;
  /** The canvases the fluid flows round, and how much they're grown by. */
  obstacles: { canvases: string[]; grow: number };
  /** The entry and page it's shaped on, and where it's added if the entry doesn't add it. */
  preview: { entry: string | null; page: number; target: string | null };
}

const CHANNELS = ['speed', 'vorticity', 'pressure', 'dye'];
/** A channel, or none for the same everywhere. */
const SOME_CHANNEL = [...CHANNELS, null];

const channel = (fallback: string | null): OptionField => ({ name: 'channel', kind: 'one_of', choices: SOME_CHANNEL, default: fallback });
const shaping: OptionField[] = [
  { name: 'range', kind: 'json', default: null },
  { name: 'curve', kind: 'number', default: 1 },
];

/** Each effect's settings, with their defaults (as fluid.rs has them). */
export const EFFECT_FIELDS: Record<EffectName, OptionField[]> = {
  position_distort: [
    { name: 'by', kind: 'one_of', choices: ['advect', 'offset', 'gradient'], default: 'advect' },
    { name: 'amount', kind: 'number', default: 1 },
    { name: 'channel', kind: 'one_of', choices: CHANNELS, default: 'dye' },
    { name: 'step', kind: 'number', default: 2 },
  ],
  jitter_distort: [channel('speed'), { name: 'amount', kind: 'number', default: 3 }, { name: 'length', kind: 'number', default: 20 }, ...shaping],
  weight_distort: [channel('speed'), { name: 'from', kind: 'number', default: 1 }, { name: 'to', kind: 'number', default: 2 }, ...shaping],
  opacity_distort: [channel('speed'), { name: 'from', kind: 'number', default: 0.3 }, { name: 'to', kind: 'number', default: 1 }, ...shaping],
  color_distort: [channel('dye'), { name: 'colors', kind: 'json', default: ['#1d1d1f', '#c0392b'] }, { name: 'mix', kind: 'number', default: 1 }, ...shaping],
};

export const EFFECT_INFO: Record<EffectName, { label: string; hint: string }> = {
  position_distort: { label: 'Position', hint: 'Moves strokes: carried along the flow for amount seconds (advect), pushed by the velocity (offset), or up a channel\'s slope (gradient).' },
  jitter_distort: { label: 'Jitter', hint: 'A smooth wobble, amount units at most, as strong as the channel is.' },
  weight_distort: { label: 'Weight', hint: 'How heavy the ink is: from times its weight where the channel is lowest, to times where it\'s highest.' },
  opacity_distort: { label: 'Opacity', hint: 'How strong the ink is, from and to, by the channel.' },
  color_distort: { label: 'Colour', hint: 'The ink mixed toward the colours, low to high, by the channel.' },
};

function pair(value: unknown): [number, number] | null {
  return Array.isArray(value) && value.length === 2 && value.every((v) => typeof v === 'number' && Number.isFinite(v)) ? [value[0], value[1]] : null;
}

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

/** A fluid file's text as the tab edits it; what doesn't read is left at its default. */
export function parseFluid(text: string | null | undefined): FluidDoc {
  let file: Record<string, unknown> = {};
  try {
    file = record(JSON.parse(text ?? '{}')) ?? {};
  } catch {
    // An unreadable file starts afresh; it's written over when the tab saves.
  }
  const effects: FluidDoc['effects'] = {};
  for (const name of EFFECTS) {
    const settings = record(file[name]);
    if (settings) effects[name] = settings;
  }
  const obstacles = record(file['obstacles']) ?? {};
  const preview = record(file['preview']) ?? {};
  return {
    at: pair(file['at']),
    size: pair(file['size']),
    effects,
    field: record(file['field']),
    scene: record(file['scene']),
    obstacles: {
      canvases: Array.isArray(obstacles['canvases']) ? obstacles['canvases'].filter((c): c is string => typeof c === 'string') : [],
      grow: typeof obstacles['grow'] === 'number' ? obstacles['grow'] : 2,
    },
    preview: {
      entry: typeof preview['entry'] === 'string' ? preview['entry'] : null,
      page: typeof preview['page'] === 'number' ? preview['page'] : 0,
      target: typeof preview['target'] === 'string' ? preview['target'] : null,
    },
  };
}

/** What scripts read of it: its box and effects, as JSON (FluidFile). */
export function fluidSettings(doc: FluidDoc): string {
  return JSON.stringify({ at: doc.at, size: doc.size, ...doc.effects });
}

/** The file's text: a line for each setting, the field and scene last. */
export function fluidText(doc: FluidDoc): string {
  const lines: [string, unknown][] = [['version', 1]];
  if (doc.at) lines.push(['at', doc.at]);
  if (doc.size) lines.push(['size', doc.size]);
  for (const name of EFFECTS) {
    if (doc.effects[name]) lines.push([name, doc.effects[name]]);
  }
  lines.push(['obstacles', doc.obstacles], ['preview', doc.preview]);
  if (doc.field) lines.push(['field', doc.field]);
  if (doc.scene) lines.push(['scene', doc.scene]);
  return `{\n${lines.map(([key, value]) => `\t${JSON.stringify(key)}: ${JSON.stringify(value)}`).join(',\n')}\n}\n`;
}
