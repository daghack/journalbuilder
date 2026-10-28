// What the style builder sets, and the text style file it writes from it.

import { rhaiFloat, rhaiString } from '../shared/rhai-literal';

/** A text style as the builder shows it: the ink, the hand, the type and the paragraph. */
export interface StyleSettings {
  description: string;
  color: string;
  brush: string;
  brush_size: number;
  min_pressure: number;
  max_pressure: number;
  pressure_profile: string;
  pressure_wander: number;
  pool: number;
  color_jitter: number;
  passes: number;
  pass_end_jitter: number;
  stroke_point_jitter: number;
  /** In glyph scales: written as o.size times it. */
  glyph_position_jitter: number;
  glyph_rotation_jitter: number;
  glyph_scale_jitter: number;
  /** In glyph scales: written as o.size times it. */
  baseline_wander: number;
  glyphset: string;
  size: number;
  glyph_tracking: number;
  italic: boolean;
  line_height: number;
  align: string;
  line_start_jitter: number;
  line_gap_jitter: number;
  /** The text to preview it with; blank for lorem ipsum. */
  example: string;
}

export type SettingKey = Exclude<keyof StyleSettings, 'description' | 'example'>;

/** One control: a slider, a choice, a colour or a switch. */
export interface SettingField {
  key: SettingKey;
  label: string;
  title: string;
  kind: 'range' | 'choice' | 'colour' | 'switch';
  min?: number;
  max?: number;
  step?: number;
  /** A choice's options; brushes and glyphsets come from the engine. */
  choices?: string[] | 'brushes' | 'glyphsets';
}

export interface SettingGroup {
  label: string;
  fields: SettingField[];
}

/** A hand a little unsteady, as plain_hand writes. */
export const DEFAULT_STYLE: StyleSettings = {
  description: 'how a kind of text looks.',
  color: '#2b1d12',
  brush: 'Ink Pen',
  brush_size: 1.4,
  min_pressure: 0.4,
  max_pressure: 0.8,
  pressure_profile: 'ramp',
  pressure_wander: 0,
  pool: 0,
  color_jitter: 0,
  passes: 1,
  pass_end_jitter: 0,
  stroke_point_jitter: 0,
  glyph_position_jitter: 0.02,
  glyph_rotation_jitter: 0.03,
  glyph_scale_jitter: 0,
  baseline_wander: 0.08,
  glyphset: 'plain',
  size: 26,
  glyph_tracking: 0,
  italic: false,
  line_height: 1.4,
  align: 'left',
  line_start_jitter: 0,
  line_gap_jitter: 0,
  example: '',
};

export const SETTING_GROUPS: SettingGroup[] = [
  {
    label: 'Ink',
    fields: [
      { key: 'color', label: 'Colour', title: 'The ink: an option of the style, so journals can change it', kind: 'colour' },
      { key: 'brush', label: 'Brush', title: 'Which Darkly brush draws (its texture shows in the Darkly preview)', kind: 'choice', choices: 'brushes' },
      { key: 'brush_size', label: 'Brush size', title: 'How wide its marks are, in canvas units', kind: 'range', min: 0.2, max: 8, step: 0.1 },
      { key: 'min_pressure', label: 'Min pressure', title: 'The least pressure along a stroke', kind: 'range', min: 0, max: 1, step: 0.01 },
      { key: 'max_pressure', label: 'Max pressure', title: 'The most pressure along a stroke', kind: 'range', min: 0, max: 1, step: 0.01 },
      { key: 'pressure_profile', label: 'Pressure profile', title: 'How pressure runs along each stroke', kind: 'choice', choices: ['ramp', 'taper', 'constant', 'pointed'] },
      { key: 'pressure_wander', label: 'Pressure wander', title: 'How far pressure wanders either way, as a fraction of it', kind: 'range', min: 0, max: 0.95, step: 0.01 },
      { key: 'pool', label: 'Ink pooling', title: 'How much heavier strokes are at the bottom of each glyph', kind: 'range', min: 0, max: 2, step: 0.05 },
      { key: 'color_jitter', label: 'Colour jitter', title: 'How far each stroke\'s lightness strays from the colour', kind: 'range', min: 0, max: 0.3, step: 0.005 },
    ],
  },
  {
    label: 'Hand',
    fields: [
      { key: 'passes', label: 'Passes', title: 'How many times each stroke is drawn over', kind: 'range', min: 1, max: 8, step: 1 },
      { key: 'pass_end_jitter', label: 'Pass end jitter', title: 'How far each pass\'s ends stray, in canvas units', kind: 'range', min: 0, max: 4, step: 0.05 },
      { key: 'stroke_point_jitter', label: 'Stroke point jitter', title: 'How far every point of every pass strays, in canvas units', kind: 'range', min: 0, max: 4, step: 0.05 },
      { key: 'glyph_position_jitter', label: 'Glyph position jitter', title: 'How far each glyph strays, in glyph scales (written as o.size times it)', kind: 'range', min: 0, max: 0.2, step: 0.005 },
      { key: 'glyph_rotation_jitter', label: 'Glyph rotation jitter', title: 'How far each glyph turns, in radians', kind: 'range', min: 0, max: 0.3, step: 0.005 },
      { key: 'glyph_scale_jitter', label: 'Glyph scale jitter', title: 'How much each glyph\'s size changes, as a fraction', kind: 'range', min: 0, max: 0.3, step: 0.005 },
      { key: 'baseline_wander', label: 'Baseline wander', title: 'How far lines wander up and down, in glyph scales (written as o.size times it)', kind: 'range', min: 0, max: 0.5, step: 0.01 },
    ],
  },
  {
    label: 'Type',
    fields: [
      { key: 'glyphset', label: 'Glyphset', title: 'The glyphs text is set in', kind: 'choice', choices: 'glyphsets' },
      { key: 'size', label: 'Size', title: 'The glyph scale: an option of the style, so journals can change it', kind: 'range', min: 6, max: 120, step: 1 },
      { key: 'glyph_tracking', label: 'Tracking', title: 'Extra room after each glyph, in glyph scales', kind: 'range', min: -0.3, max: 1, step: 0.01 },
      { key: 'italic', label: 'Italic', title: 'Leans the glyphs 0.25 radians', kind: 'switch' },
    ],
  },
  {
    label: 'Paragraph',
    fields: [
      { key: 'line_height', label: 'Line height', title: 'How far apart lines are, in glyph scales', kind: 'range', min: 0.8, max: 3, step: 0.05 },
      { key: 'align', label: 'Align', title: 'Where each line\'s glyphs go along it', kind: 'choice', choices: ['left', 'center', 'right', 'justify'] },
      { key: 'line_start_jitter', label: 'Line start jitter', title: 'How much further each line starts, at most, in canvas units', kind: 'range', min: 0, max: 40, step: 0.5 },
      { key: 'line_gap_jitter', label: 'Line gap jitter', title: 'How much each line\'s distance from the last varies, in canvas units', kind: 'range', min: 0, max: 20, step: 0.25 },
    ],
  },
];

/** What the engine takes when a setting's left out: those aren't written. */
const ENGINE_DEFAULTS: Partial<StyleSettings> = {
  min_pressure: 0.3, max_pressure: 0.7, pressure_profile: 'ramp', pressure_wander: 0, pool: 0, color_jitter: 0,
  passes: 1, pass_end_jitter: 0, stroke_point_jitter: 0, glyph_position_jitter: 0, glyph_rotation_jitter: 0, glyph_scale_jitter: 0, baseline_wander: 0,
  glyph_tracking: 0, italic: false, line_height: 1.25, align: 'left', line_start_jitter: 0, line_gap_jitter: 0,
};

/** A number rounded as a slider steps, as a rhai float. */
function num(n: number): string {
  return rhaiFloat(+n.toFixed(4));
}

/** `key: value` for each setting of `keys` that isn't the engine's default, as a rhai map, or null if none is. */
function options(s: StyleSettings, keys: SettingKey[], write: (key: SettingKey) => string = (key) => value(s[key])): string | null {
  const set = keys.filter((key) => s[key] !== ENGINE_DEFAULTS[key]);
  return set.length ? `#{ ${set.map((key) => `${key}: ${write(key)}`).join(', ')} }` : null;
}

function value(v: string | number | boolean): string {
  return typeof v === 'number' ? num(v) : typeof v === 'string' ? rhaiString(v) : String(v);
}

/** `text` as `//` comment lines no wider than `width`. */
function comment(text: string, width = 76): string {
  const lines: string[] = [];
  let line = '//';
  for (const word of text.split(/\s+/).filter(Boolean)) {
    if (line.length > 2 && line.length + 1 + word.length > width) {
      lines.push(line);
      line = '//';
    }
    line += ` ${word}`;
  }
  return [...lines, line].join('\n');
}

/** The text style file `name` the builder's settings make: size and colour its options, the rest written in. */
export function styleSource(name: string, s: StyleSettings): string {
  const brush = options(s, ['min_pressure', 'max_pressure', 'pressure_profile', 'pressure_wander', 'pool', 'color_jitter']);
  const hand = options(s, ['passes', 'pass_end_jitter', 'stroke_point_jitter', 'glyph_position_jitter', 'glyph_rotation_jitter', 'glyph_scale_jitter', 'baseline_wander'],
    (key) => key === 'passes' ? String(Math.round(s.passes))
      : key === 'glyph_position_jitter' || key === 'baseline_wander' ? `o.size * ${num(s[key])}` : value(s[key]));
  const type = options(s, ['glyph_tracking', 'italic']);
  const paragraph = options(s, ['line_height', 'align', 'line_start_jitter', 'line_gap_jitter']);
  const example = s.example.trim() ? rhaiString(s.example) : 'content("lorem")';
  return `${comment(`${name}: ${s.description.trim() || 'how a kind of text looks.'}`)}

fn options() {
\t#{ size: ${num(s.size)}, color: ${rhaiString(s.color)} }
}

fn style(o) {
\t#{
\t\tstyle: new_style(
\t\t\tnew_brush_style(o.color, ${rhaiString(s.brush)}, ${num(s.brush_size)}${brush ? `, ${brush}` : ''}),
\t\t\tnew_handwriting_style(${hand ?? ''}),
\t\t),
\t\ttype: new_type_style(glyphset(${rhaiString(s.glyphset)}), o.size${type ? `, ${type}` : ''}),${paragraph ? `\n\t\tparagraph: ${paragraph},` : ''}
\t}
}

// The text the editor previews it with.
fn example() {
\t${example}
}
`;
}

/**
 * A journal entry that writes the style file `source`'s example in a
 * block `width` wide, as the style editor's preview does: for Darkly,
 * which renders entries, before the file is in the journal.
 */
export function stylePreviewScript(source: string, width: number): string {
  return `${source}
let settings = style(options());
let size = settings.type.glyph_scale;
let page = new_page();
page.add(write_block(example(), vec2(size, size * 2), ${num(width)} - size * 2, settings).canvas);
journal_entry.add(page);
`;
}
