// What the style form sets, and the text style file it's kept as.

import { isGradient, type Paint } from '../shared/gradient';
import { rhaiFloat, rhaiString } from '../shared/rhai-literal';

/** Lines under or through text, as a file holds them: none, on, how many, or `{ count, spacing, style }`. */
export type RuleValue = boolean | number | { count?: number; spacing?: number; style?: Record<string, unknown> };

/** A span, as a role takes it: settings over the text's. */
export interface SpanSettings {
  style?: Record<string, unknown>;
  type?: Record<string, unknown>;
}

/**
 * A text style as the form shows it, and its file holds it: every setting of
 * its ink, hand, type and paragraph, and its spans. A null is unset: what
 * it comes to is the glyphset's, or the call's.
 */
export interface StyleSettings {
  description: string;
  color: Paint;
  brush: string;
  brush_size: number;
  min_pressure: number;
  max_pressure: number;
  pressure_profile: string;
  pressure_wander: number;
  pressure_wander_length: number;
  pool: number;
  pool_depth: number;
  color_jitter: number;
  passes: number;
  pass_end_jitter: number;
  stroke_point_jitter: number;
  /** In glyph scales, so it follows the size; one for both ways, or [across, down]. */
  glyph_position_jitter: number | [number, number];
  glyph_rotation_jitter: number;
  glyph_scale_jitter: number;
  /** In glyph scales, so it follows the size. */
  baseline_wander: number;
  glyphset: string;
  size: number;
  glyph_scale_factor: number;
  glyph_tracking: number;
  glyph_outline_gap: number | null;
  space_width: number | null;
  italic: boolean;
  skew: number | null;
  joined: boolean | null;
  underline: RuleValue;
  strike: RuleValue;
  line_height: number | null;
  paragraph_gap: number;
  indent: number;
  align: string;
  valign: string;
  fit: boolean;
  layout: string | null;
  angle: number | null;
  /** Off, or the fewest glyphs a piece may have (on is 2). */
  split_words: boolean | number;
  split_padding: number;
  /** Off, or the fewest glyphs a piece may have (on is 2). */
  break_words: boolean | number;
  clearance: number;
  line_start_jitter: number;
  line_gap_jitter: number;
  overhang: number;
  /** Named spans: *em*, **strong** and [text]{name}. */
  spans: Record<string, SpanSettings>;
  /** The text to preview it with; blank for lorem ipsum. */
  example: string;
}

export type SettingKey = Exclude<keyof StyleSettings, 'description' | 'example' | 'spans'>;

/** How the preview writes: as write_block does, or typeset as a layout does. */
export type PreviewAs = 'block' | 'typeset';

/** One control: a slider, a choice, a colour, a switch, two sliders or lines of text. */
export interface SettingField {
  /** Besides the file's: a span's type has the glyph scale itself, which a file calls size; lines have their spacing. */
  key: SettingKey | 'glyph_scale' | 'spacing';
  label: string;
  title: string;
  kind: 'range' | 'choice' | 'colour' | 'switch' | 'pair' | 'rule';
  min?: number;
  max?: number;
  step?: number;
  /** A slider's value is a whole number. */
  whole?: boolean;
  /** A choice's options; brushes and glyphsets come from the engine. */
  choices?: readonly string[] | 'brushes' | 'glyphsets';
  /** What it comes to unset, for a setting that can be: a slider's or choice's value can be cleared. */
  unset?: string;
  /** What it's kept as unset; null unless it says. */
  unsetValue?: false;
  /** What's only of use with this set. */
  needs?: SettingKey;
  /** The preview that shows it, if only one does. */
  preview?: PreviewAs;
}

export interface SettingGroup {
  label: string;
  fields: SettingField[];
}

export const SETTING_GROUPS: SettingGroup[] = [
  {
    label: 'Ink',
    fields: [
      { key: 'color', label: 'Colour', title: 'The ink, a colour or a gradient: an option of the style, so journals can change it', kind: 'colour' },
      { key: 'brush', label: 'Brush', title: 'Which Darkly brush draws (its texture shows in the Darkly preview)', kind: 'choice', choices: 'brushes' },
      { key: 'brush_size', label: 'Brush size', title: 'How wide its marks are, in canvas units', kind: 'range', min: 0.2, max: 8, step: 0.1 },
      { key: 'min_pressure', label: 'Min pressure', title: 'The least pressure along a stroke', kind: 'range', min: 0, max: 1, step: 0.01 },
      { key: 'max_pressure', label: 'Max pressure', title: 'The most pressure along a stroke', kind: 'range', min: 0, max: 1, step: 0.01 },
      { key: 'pressure_profile', label: 'Pressure profile', title: 'How pressure runs along each stroke', kind: 'choice', choices: ['ramp', 'taper', 'constant', 'pointed'] },
      { key: 'pressure_wander', label: 'Pressure wander', title: 'How far pressure wanders either way, as a fraction of it (Darkly)', kind: 'range', min: 0, max: 0.95, step: 0.01 },
      { key: 'pressure_wander_length', label: 'Wander length', title: 'About how far along a stroke pressure takes to rise and fall, in canvas units (Darkly)', kind: 'range', min: 1, max: 60, step: 0.5 },
      { key: 'pool', label: 'Ink pooling', title: 'How much heavier strokes are at the bottom of each path (Darkly)', kind: 'range', min: 0, max: 2, step: 0.05 },
      { key: 'pool_depth', label: 'Pool depth', title: 'How far up each path ink pools, as a fraction of its height (Darkly)', kind: 'range', min: 0.05, max: 1, step: 0.05 },
      { key: 'color_jitter', label: 'Colour jitter', title: 'How far each stroke\'s lightness strays from the colour', kind: 'range', min: 0, max: 0.3, step: 0.005 },
    ],
  },
  {
    label: 'Hand',
    fields: [
      { key: 'passes', label: 'Passes', title: 'How many times each stroke is drawn over', kind: 'range', min: 1, max: 8, step: 1, whole: true },
      { key: 'pass_end_jitter', label: 'Pass end jitter', title: 'How far each pass\'s ends stray, in canvas units', kind: 'range', min: 0, max: 4, step: 0.05 },
      { key: 'stroke_point_jitter', label: 'Stroke point jitter', title: 'How far every point of every pass strays, in canvas units', kind: 'range', min: 0, max: 4, step: 0.05 },
      { key: 'glyph_position_jitter', label: 'Glyph position jitter', title: 'How far each glyph strays, across and down, in glyph scales, so it follows the size', kind: 'pair', min: 0, max: 0.2, step: 0.005 },
      { key: 'glyph_rotation_jitter', label: 'Glyph rotation jitter', title: 'How far each glyph turns, in radians', kind: 'range', min: 0, max: 0.3, step: 0.005 },
      { key: 'glyph_scale_jitter', label: 'Glyph scale jitter', title: 'How much each glyph\'s size changes, as a fraction', kind: 'range', min: 0, max: 0.3, step: 0.005 },
      { key: 'baseline_wander', label: 'Baseline wander', title: 'How far lines wander up and down, in glyph scales, so it follows the size (write_line and write_block)', kind: 'range', min: 0, max: 0.5, step: 0.01, preview: 'block' },
    ],
  },
  {
    label: 'Type',
    fields: [
      { key: 'glyphset', label: 'Glyphset', title: 'The glyphs text is set in', kind: 'choice', choices: 'glyphsets' },
      { key: 'size', label: 'Size', title: 'The glyph scale: an option of the style, so journals can change it', kind: 'range', min: 6, max: 120, step: 1 },
      { key: 'glyph_scale_factor', label: 'Scale factor', title: 'Times the size', kind: 'range', min: 0.2, max: 3, step: 0.05 },
      { key: 'glyph_tracking', label: 'Tracking', title: 'Extra room after each glyph, in glyph scales', kind: 'range', min: -0.3, max: 1, step: 0.01 },
      { key: 'glyph_outline_gap', label: 'Outline gap', title: 'Room between glyphs\' outlines, in glyph scales', kind: 'range', min: 0, max: 1, step: 0.01, unset: 'the glyphset\'s' },
      { key: 'space_width', label: 'Space width', title: 'A space\'s width, in glyph scales', kind: 'range', min: 0.05, max: 2, step: 0.01, unset: 'the glyphset\'s' },
      { key: 'italic', label: 'Italic', title: 'Leans the glyphs 0.25 radians', kind: 'switch' },
      { key: 'skew', label: 'Skew', title: 'How far the glyphs lean, tops forward, in radians, in place of Italic', kind: 'range', min: -1.2, max: 1.2, step: 0.01, unset: 'the glyphset\'s' },
      { key: 'joined', label: 'Joined', title: 'Whether a cursive glyphset\'s words are written joined up', kind: 'switch', unset: 'the glyphset\'s' },
      { key: 'underline', label: 'Underline', title: 'Lines under the text, where it\'s typeset', kind: 'rule', preview: 'typeset' },
      { key: 'strike', label: 'Strike', title: 'Lines through the text, where it\'s typeset', kind: 'rule', preview: 'typeset' },
    ],
  },
  {
    label: 'Paragraph',
    fields: [
      { key: 'line_height', label: 'Line height', title: 'How far apart lines are, in glyph scales. Unset, text boxes and write_inside_path space lines by their ink, and other text 1.25 apart', kind: 'range', min: 0.8, max: 3, step: 0.05, unset: 'the call\'s' },
      { key: 'paragraph_gap', label: 'Paragraph gap', title: 'Extra room between paragraphs, in lines, where text is typeset', kind: 'range', min: 0, max: 3, step: 0.05, preview: 'typeset' },
      { key: 'indent', label: 'Indent', title: 'How much further in a paragraph\'s first line starts, in glyph scales, where text is typeset', kind: 'range', min: 0, max: 6, step: 0.1, preview: 'typeset' },
      { key: 'align', label: 'Align', title: 'Where each line\'s glyphs go along it', kind: 'choice', choices: ['left', 'center', 'right', 'justify'] },
      { key: 'valign', label: 'Vertical align', title: 'Where text sits down a text box or a typeset slot', kind: 'choice', choices: ['top', 'center', 'bottom'], preview: 'typeset' },
      { key: 'fit', label: 'Fit', title: 'Shrinks the text until all of it fits its slot, where it\'s typeset or in write_inside_path', kind: 'switch', preview: 'typeset' },
      { key: 'layout', label: 'Layout', title: 'How glyphs are placed: glyph by glyph, or words kept whole, wrapping at spaces', kind: 'choice', choices: ['flat', 'words'], unset: 'the call\'s' },
      { key: 'angle', label: 'Angle', title: 'In a text box, which way its lines run; along a path, the angle every glyph is held at, in radians (not shown here)', kind: 'range', min: -3.14, max: 3.14, step: 0.01, unset: 'none' },
      { key: 'split_words', label: 'Split words', title: 'With the words layout, breaks words round what\'s avoided: the fewest glyphs a piece may have (not shown here, with nothing to avoid)', kind: 'range', min: 1, max: 8, step: 1, whole: true, unset: 'off', unsetValue: false },
      { key: 'split_padding', label: 'Split padding', title: 'How much further split words\' pieces keep from what\'s avoided, in glyph scales (not shown here)', kind: 'range', min: 0, max: 3, step: 0.05, needs: 'split_words' },
      { key: 'break_words', label: 'Break words', title: 'With the words layout, cuts a word that doesn\'t fit at the end of a line: the fewest glyphs a piece may have', kind: 'range', min: 1, max: 8, step: 1, whole: true, unset: 'off', unsetValue: false },
      { key: 'clearance', label: 'Clearance', title: 'How far glyphs keep from what they avoid, in canvas units (not shown here)', kind: 'range', min: 0, max: 30, step: 0.5 },
      { key: 'line_start_jitter', label: 'Line start jitter', title: 'How much further each line starts, at most, in canvas units', kind: 'range', min: 0, max: 40, step: 0.5 },
      { key: 'line_gap_jitter', label: 'Line gap jitter', title: 'How much each line\'s distance from the last varies, in canvas units', kind: 'range', min: 0, max: 20, step: 0.25 },
      { key: 'overhang', label: 'Overhang', title: 'How far a text box\'s glyphs may reach past its edge, in canvas units (not shown here)', kind: 'range', min: 0, max: 20, step: 0.25 },
    ],
  },
];

const FIELDS = SETTING_GROUPS.flatMap((group) => group.fields);
const FIELD_OF = new Map<string, SettingField>(FIELDS.map((field) => [field.key, field]));
const groupFields = (label: string) => SETTING_GROUPS.find((group) => group.label === label)!.fields;

/** What a line under or through text, or a span, can draw differently: a style's settings. */
export const STYLE_FIELDS: readonly SettingField[] = [...groupFields('Ink'), ...groupFields('Hand')];

const SPAN_TYPE_FIELDS: readonly SettingField[] = groupFields('Type').map((field) => field.key === 'size'
  ? { ...field, key: 'glyph_scale', title: 'The glyph scale, in place of the text\'s' }
  : field);

/** What a span can change: a style's settings and a type's, none of them unset. */
export const SPAN_FIELDS: readonly SettingField[] = [...STYLE_FIELDS, ...SPAN_TYPE_FIELDS].map((field) => ({ ...field, unset: undefined, unsetValue: undefined }));

/** A line's own spacing, as a setting. */
export const RULE_SPACING_FIELD: SettingField = {
  key: 'spacing', label: 'Spacing', title: 'How far apart the lines are, in glyph scales', kind: 'range', min: 0, max: 2, step: 0.01, unset: 'the pen\'s',
};

/** How many lines, as a setting. */
export const RULE_COUNT_FIELD: Pick<SettingField, 'min' | 'max' | 'step'> = { min: 0, max: 20, step: 1 };

/**
 * What a setting is when a style file leaves it out: the engine's default,
 * or for the glyphset and size, which it has none of, what the form starts
 * them at. In the order a file lists them.
 */
const LEFT_OUT: StyleSettings = {
  description: '',
  color: 'black',
  brush: 'Ink Pen',
  brush_size: 1.2,
  min_pressure: 0.3,
  max_pressure: 0.7,
  pressure_profile: 'ramp',
  pressure_wander: 0,
  pressure_wander_length: 12,
  pool: 0,
  pool_depth: 0.3,
  color_jitter: 0,
  passes: 1,
  pass_end_jitter: 0,
  stroke_point_jitter: 0,
  glyph_position_jitter: 0,
  glyph_rotation_jitter: 0,
  glyph_scale_jitter: 0,
  baseline_wander: 0,
  glyphset: 'plain',
  size: 26,
  glyph_scale_factor: 1,
  glyph_tracking: 0,
  glyph_outline_gap: null,
  space_width: null,
  italic: false,
  skew: null,
  joined: null,
  underline: false,
  strike: false,
  line_height: null,
  paragraph_gap: 0.5,
  indent: 0,
  align: 'left',
  valign: 'top',
  fit: false,
  layout: null,
  angle: null,
  split_words: false,
  split_padding: 0,
  break_words: false,
  clearance: 3,
  line_start_jitter: 0,
  line_gap_jitter: 0,
  overhang: 0,
  spans: {},
  example: '',
};

/** A hand a little unsteady, as plain_hand writes. */
export const DEFAULT_STYLE: StyleSettings = {
  ...LEFT_OUT,
  description: 'how a kind of text looks.',
  color: '#2b1d12',
  brush_size: 1.4,
  min_pressure: 0.4,
  max_pressure: 0.8,
  glyph_position_jitter: 0.02,
  glyph_rotation_jitter: 0.03,
  baseline_wander: 0.08,
  line_height: 1.4,
};

/** What a file always says, however it was made: what the style is, at a glance. */
const ALWAYS_WRITTEN: readonly (keyof StyleSettings)[] = ['color', 'brush', 'brush_size', 'glyphset', 'size'];

/** A setting as a file writes it; undefined if it's not to be. */
function asWritten(key: keyof StyleSettings, s: StyleSettings): unknown {
  const value = s[key];
  if (key === 'spans') return Object.keys(s.spans).length ? s.spans : undefined;
  // Of no use unless words are split, as the engine says if it's given.
  if (key === 'split_padding' && !s.split_words) return undefined;
  if (Array.isArray(value)) {
    const [across, down] = value.map((n) => +n.toFixed(4));
    return across === down ? across : [across, down];
  }
  if (typeof value === 'number') return FIELD_OF.get(key)?.whole ? Math.round(value) : +value.toFixed(4);
  return value;
}

/** The style file the settings make: what's as if left out isn't written. */
export function styleFile(s: StyleSettings): string {
  const file: Record<string, unknown> = {};
  for (const key of Object.keys(LEFT_OUT) as (keyof StyleSettings)[]) {
    const value = asWritten(key, s);
    if (value !== undefined && !(value === LEFT_OUT[key] && !ALWAYS_WRITTEN.includes(key))) file[key] = value;
  }
  return JSON.stringify(file, null, 2) + '\n';
}

/** What a setting should be if `given` isn't that, or null. */
function misfit(field: SettingField, given: unknown): string | null {
  const isNumber = typeof given === 'number';
  switch (field.kind) {
    case 'colour': return typeof given === 'string' || isGradient(given) ? null : 'a colour or a gradient';
    case 'choice': return typeof given === 'string' ? null : 'a string';
    case 'switch': return typeof given === 'boolean' ? null : 'a boolean';
    case 'pair': return isNumber || (Array.isArray(given) && given.length === 2 && given.every((n) => typeof n === 'number')) ? null : 'a number or [across, down]';
    case 'rule': return isNumber || typeof given === 'boolean' || isObject(given) ? null : 'true, false, a number of lines or { count, spacing, style }';
    default: return isNumber || (field.unsetValue === false && typeof given === 'boolean') ? null : 'a number';
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

/** The settings in the style file `text`, or what's wrong with it. */
export function readStyle(text: string): StyleSettings {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (err) {
    throw new Error(`It isn't JSON: ${(err as Error).message}`);
  }
  if (!isObject(value)) throw new Error('It should be a JSON object.');
  for (const [key, given] of Object.entries(value)) {
    if (!(key in LEFT_OUT)) throw new Error(`It has '${key}', which a style file doesn't.`);
    const field = FIELD_OF.get(key);
    const wanted = field ? misfit(field, given) : key === 'spans' ? (isObject(given) ? null : 'an object of span names') : typeof given === 'string' ? null : 'a string';
    if (wanted) throw new Error(`Its '${key}' should be ${wanted}.`);
  }
  return { ...LEFT_OUT, ...value };
}

/** A name for a copy of the style `name`, one that isn't `taken`: NAME_copy, then NAME_copy_2 and on. */
export function copyName(name: string, taken: (name: string) => boolean): string {
  let copy = `${name}_copy`;
  for (let n = 2; taken(copy); n++) copy = `${name}_copy_${n}`;
  return copy;
}

/** The settings with `key` set to `value`, what goes with it keeping up. */
export function withSetting(s: StyleSettings, key: SettingKey, value: unknown): StyleSettings {
  const next = { ...s, [key]: value } as StyleSettings;
  // min_pressure and max_pressure keep in order, the other giving way.
  if (key === 'min_pressure' && (value as number) > s.max_pressure) next.max_pressure = value as number;
  if (key === 'max_pressure' && (value as number) < s.min_pressure) next.min_pressure = value as number;
  // Italic is a skew: the last one set says.
  if (key === 'italic' && value) next.skew = null;
  if (key === 'skew' && value !== null) next.italic = false;
  return next;
}

/** A slider's number: on, for what's on or off, being 2. */
export function numberOf(value: unknown): number | null {
  return typeof value === 'number' ? value : value === true ? 2 : null;
}

/** A setting of one number or two, as two. */
export function pairOf(value: unknown): [number, number] {
  if (Array.isArray(value)) return [Number(value[0]), Number(value[1])];
  const n = typeof value === 'number' ? value : 0;
  return [n, n];
}

/** Lines under or through text, each part of them apart. */
export interface Rule {
  count: number;
  spacing: number | null;
  style: Record<string, unknown>;
}

export function readRule(value: unknown): Rule {
  if (typeof value === 'number') return { count: value, spacing: null, style: {} };
  if (!isObject(value)) return { count: value === true ? 1 : 0, spacing: null, style: {} };
  return { count: typeof value['count'] === 'number' ? value['count'] : 1, spacing: typeof value['spacing'] === 'number' ? value['spacing'] : null, style: isObject(value['style']) ? value['style'] : {} };
}

/** Lines as a file holds them, as briefly as it can. */
export function ruleValue(rule: Rule): RuleValue {
  if (!rule.count) return false;
  const styled = Object.keys(rule.style).length > 0;
  if (rule.spacing === null && !styled) return rule.count;
  return { count: rule.count, ...(rule.spacing === null ? {} : { spacing: rule.spacing }), ...(styled ? { style: rule.style } : {}) };
}

/** What a setting starts at, when something sets it that didn't. */
export function startingValue(field: SettingField): unknown {
  if (field.key === 'glyph_scale') return DEFAULT_STYLE.size;
  const value = DEFAULT_STYLE[field.key as SettingKey];
  switch (field.kind) {
    case 'switch': return true;
    case 'rule': return 1;
    case 'range': return value ?? Math.max(field.min ?? 0, 0);
    default: return value;
  }
}

/** What a span can't be called: a setting's name, which `[text]{name}` would take as the setting. */
export function spanNameProblem(name: string, taken: readonly string[]): string | null {
  if (!name) return null;
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) return 'A span\'s name is letters, digits and _.';
  if (FIELD_OF.has(name) || name === 'glyph_scale') return `'${name}' is a setting's name.`;
  if (taken.includes(name)) return `There's a '${name}' already.`;
  return null;
}

/** Whether a span's setting is a type's, not a style's. */
export function isTypeSetting(key: string): boolean {
  return SPAN_TYPE_FIELDS.some((field) => field.key === key);
}

/** Marks to preview, with the lorem ipsum, how typeset text changes as they do. */
const MARKED = 'Lorem ipsum *dolor sit amet*, consectetur **adipiscing** elit, sed do ~~eiusmod~~ tempor.';

function previewText(s: StyleSettings, as: PreviewAs): string {
  if (s.example.trim()) return rhaiString(s.example);
  if (as === 'block') return 'content("lorem")';
  const named = Object.keys(s.spans).filter((name) => name !== 'em' && name !== 'strong').map((name) => `[incididunt ut labore]{${name}}`);
  return `content("lorem") + "\\n\\n" + ${rhaiString([MARKED, ...named].join(' '))}`;
}

/**
 * A journal entry that writes with style `name` in a block `width` wide:
 * as write_block does, or typeset, its text as markdown, as layouts do. Each
 * setting shows in one or the other.
 */
export function stylePreviewScript(name: string, s: StyleSettings, width: number, as: PreviewAs = 'block'): string {
  if (as === 'typeset') {
    return `let pages = new_layout("style", |n, info| column([flow("text", #{ flex: #{ grow: 1 } })], #{ flex: #{ padding: [40, 40, 40, 40] } })).typeset(#{
  width: ${rhaiFloat(width)},
  height: ${rhaiFloat(width * 0.75)},
  flows: #{ text: markdown(${previewText(s, as)}) },
  roles: #{ text: text_style(${rhaiString(name)}) },
});
journal_entry.add(pages[0]);
`;
  }
  return `let settings = text_style(${rhaiString(name)});
let size = settings.type.glyph_scale ?? 30.0;
let page = new_page();
page.add(write_block(${previewText(s, as)}, vec2(size, size * 2), ${rhaiFloat(width)} - size * 2, settings).canvas);
journal_entry.add(page);
`;
}
