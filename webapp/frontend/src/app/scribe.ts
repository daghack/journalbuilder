// What the renderer (webapp/wasm, run in the workers) gives back, and the
// messages the app and its workers send each other.

/** A page of an evaluated journal. Its SVG arrives separately. */
export interface PageView {
  index: number;
  label: string;
  /** The box of canvas units its SVG shows, `[x, y, width, height]`; null if nothing's drawn on it. */
  view: [number, number, number, number] | null;
}

export interface ScriptError {
  message: string;
  line: number | null;
  column: number | null;
}

/** What running a script made, and what it said along the way. */
export interface Evaluation {
  pages: PageView[];
  warnings: string[];
  prints: string[];
  missingGlyphs: string | null;
  error: ScriptError | null;
  millis: number;
  /** What the workspace changed: edited fonts, lock files of its own. */
  workspace?: { editedFonts: boolean; locks: string[]; renderers?: string[]; chains?: string[] };
}

/** What the workers are sent of the workspace (see WorkspaceService). */
export interface WorkspacePayload {
  fonts: string | null;
  locks: Record<string, string>;
  renderers: Record<string, string>;
  chains: Record<string, string>;
}

/** How Darkly renders pages, as journalbuild's flags set it. */
export interface RenderOptions {
  scale: number;
  supersample?: number;
  parchment: boolean;
}

// ---- the eval worker ----

export type EvalRequest =
  | { type: 'init'; wasmUrl: string }
  | { type: 'workspace'; workspace: WorkspacePayload }
  | { type: 'evaluate'; generation: number; source: string; first: number }
  | { type: 'focus'; generation: number; index: number };

export type EvalReply =
  | { type: 'ready'; apiNames: string[] }
  | { type: 'failed'; message: string }
  | { type: 'evaluated'; generation: number; evaluation: Evaluation }
  | { type: 'svg'; generation: number; index: number; svg: string | null }
  /** The renderer panicked: the worker has to be replaced. */
  | { type: 'crashed'; message: string };

// ---- the Darkly worker ----

export type DarklyRequest =
  | { type: 'init'; wasmUrl: string }
  | { type: 'workspace'; workspace: WorkspacePayload }
  | { type: 'renderPages'; id: number; source: string; indices: number[]; options: RenderOptions }
  | { type: 'exportPdf'; id: number; source: string; title: string; options: RenderOptions };

export type DarklyReply =
  | { type: 'progress'; id: number; page: number; pages: number; fraction: number }
  /** A page rendered by renderPages: a PNG, or null if nothing's drawn on it. */
  | { type: 'page'; id: number; index: number; png: ArrayBuffer | null }
  | { type: 'done'; id: number; pdf?: ArrayBuffer }
  | { type: 'error'; id: number; message: string };

// ---- the tools worker: font previews and the lock editor ----

/** The renderer functions the tools worker calls (see webapp/wasm). */
export type ToolFunction = 'previewSvg' | 'builtinFonts' | 'lockTypes' | 'lockPorts' | 'chainTypes' | 'runChain' | 'readChain' | 'valueKinds'
  | 'rendererTypes' | 'rendererPorts' | 'renderPreview' | 'rendererSource';

/** A chain file: chains/*.rhai, lock types wired together that journals use with chain(name). */
export interface ChainType {
  name: string;
  description?: string;
  /** Whether it's one of the workspace's chain files. */
  workspace: boolean;
  /** Whether it's compiled in, from webapp/chains/ (a workspace copy stands in for it). */
  builtin?: boolean;
  /** Why it doesn't load, if it doesn't. */
  error?: string;
}

/** A renderer type: a renderers/*.rhai file that draws lock values as canvases. */
export interface RendererType {
  name: string;
  file?: string;
  description?: string;
  /** Its options' defaults: every option is optional, its type its default's. */
  options?: Record<string, unknown>;
  /** Its inputs, `[name, type]`, with its default options. */
  inputs?: [string, string][];
  /** Whether it has an example() to preview with. */
  example?: boolean;
  /** Whether it's one of the workspace's renderer files. */
  workspace: boolean;
  /** Why it doesn't load, if it doesn't. */
  error?: string;
}

/** What a renderer draws, a page per canvas, as SVG. */
export interface RenderPreview {
  pages: { svg: string | null; view: [number, number, number, number] | null }[];
}

/** A value kind: a lock value type of plain data, and rhai that makes one. */
export interface ValueKind {
  name: string;
  example: string | null;
}

export type ToolsRequest =
  | { type: 'init'; wasmUrl: string }
  | { type: 'workspace'; workspace: WorkspacePayload }
  | { type: 'call'; id: number; fn: ToolFunction; args: unknown[] };

export type ToolsReply =
  | { type: 'ready' }
  | { type: 'failed'; message: string }
  | { type: 'result'; id: number; value: unknown }
  | { type: 'error'; id: number; message: string }
  /** The renderer panicked: the worker has to be replaced. */
  | { type: 'crashed'; message: string };

/** A script's first page as SVG, for the font editor's text preview. */
export interface Preview {
  svg: string | null;
  view: [number, number, number, number] | null;
  warnings: string[];
  missingGlyphs: string | null;
  error: { message: string; line?: number | null; column?: number | null } | null;
}

/** An option a lock type declares. */
export interface LockOption {
  name: string;
  kind: 'integer' | 'number' | 'string' | 'bool' | 'array' | 'one_of';
  choices?: unknown[];
  default?: unknown;
}

/** A lock type, as the lock editor's palette lists it. */
export interface LockType {
  name: string;
  file?: string;
  description?: string;
  composite?: boolean;
  options?: LockOption[];
  /** Whether it's one of the workspace's lock files. */
  workspace: boolean;
  /** Why it doesn't load, if it doesn't. */
  error?: string;
}

/** A lock's ports, `[name, type]`, for some options, with defaults filled in. */
export interface LockPorts {
  inputs: [string, string][];
  outputs: [string, string][];
  options: Record<string, unknown>;
}

/** A value from a lock run. */
export interface LockValue {
  type: string;
  text?: string;
  json: unknown;
  /** A value kind's default drawing (a StarChart's pages, a HuffmanCode's tree). */
  art?: {
    viewBox: [number, number, number, number];
    parts: { name: string; d: string }[];
    labels: { x: number; y: number; text: string }[];
    labelSize: number;
  };
}

export interface ChainRun {
  outputs: [string, LockValue][];
  trace: [string, LockValue][];
  recovered: [string, LockValue][] | null;
  roundTrip: boolean | null;
  unlockError: string | null;
}

/** A chain file read back: `chain` is what its chain(o) returned. */
export interface ChainFile {
  name: string;
  description: string;
  options: LockOption[];
  chain: {
    locks: Record<string, { type: string; options?: Record<string, unknown> }>;
    connect?: [string, string][];
    fixed?: Record<string, unknown>;
    /** Renderers attached, each input drawing a port, "lock.port". */
    renderers?: Record<string, { type: string; options?: Record<string, unknown>; inputs?: Record<string, string> }>;
  };
  layout: Record<string, [number, number]> | null;
  /** What its example() returns, if it has one: a value for each open input, by port. */
  example: Record<string, unknown> | null;
  promotionsLost: boolean;
}

/** What a tool answers when it can't. */
export interface ToolError {
  error: string;
}
