// What the engine (webapp/wasm, run in the workers) gives back, and the
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
  /** The journal's files that stand in for the library's, as paths. */
  overrides?: string[];
}

/** What the workers are sent of the journal: its files besides the entries, by path (see JournalStore). */
export type JournalPayload = Record<string, string>;

/** How Darkly renders pages, as journalbuild's flags set it. */
export interface RenderOptions {
  scale: number;
  supersample?: number;
  parchment: boolean;
  /** For PDFs, how many pages side by side to a PDF page; none for a page each. */
  stitch?: number;
}

// ---- the eval worker ----

export type EvalRequest =
  | { type: 'init'; wasmUrl: string }
  | { type: 'journal'; files: JournalPayload }
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
  | { type: 'journal'; files: JournalPayload }
  | { type: 'renderPages'; id: number; source: string; indices: number[]; options: RenderOptions }
  | { type: 'exportPdf'; id: number; source: string; title: string; options: RenderOptions };

export type DarklyReply =
  | { type: 'progress'; id: number; page: number; pages: number; fraction: number }
  /** A page rendered by renderPages: a PNG, or null if nothing's drawn on it. */
  | { type: 'page'; id: number; index: number; png: ArrayBuffer | null }
  | { type: 'done'; id: number; pdf?: ArrayBuffer }
  | { type: 'error'; id: number; message: string };

// ---- the tools worker: previews, and the editors' questions ----

/** The engine functions the tools worker calls (see webapp/wasm). */
export type ToolFunction = 'previewSvg' | 'libraryFiles' | 'usedLibrary' | 'lockTypes' | 'lockPorts' | 'lockExamples' | 'chainTypes' | 'runChain' | 'readChain'
  | 'valueKinds' | 'illustrationTypes' | 'illustrationPorts' | 'illustrationPreview' | 'layoutTypes' | 'layoutPreview' | 'styleTypes' | 'stylePreview';

/** Whose a file is: the journal's own, the library's, or both (the journal's standing in for the library's). */
export interface Whose {
  own: boolean;
  builtin: boolean;
}

/** A chain file: chains/*.rhai, lock types wired together that journals use with chain(name). */
export interface ChainType extends Whose {
  name: string;
  description?: string;
  /** Why it doesn't load, if it doesn't. */
  error?: string;
}

/** An illustration type: an illustrations/*.rhai file that draws lock values as canvases. */
export interface IllustrationType extends Whose {
  name: string;
  file?: string;
  description?: string;
  /** Its options' defaults: every option is optional, its type its default's. */
  options?: Record<string, unknown>;
  /** Its inputs, `[name, type]`, with its default options. */
  inputs?: [string, string][];
  /** Whether it has an example() to preview with. */
  example?: boolean;
  /** Why it doesn't load, if it doesn't. */
  error?: string;
}

/** A layout file, layouts/*.rhai, or a text style file, styles/*.rhai. */
export interface LayoutType extends Whose {
  name: string;
  description?: string;
  /** Its options' defaults. */
  options?: Record<string, unknown>;
  /** A layout: whether it has an example() to preview with. A style: the text it previews with, if its file gives one. */
  example?: boolean | string | null;
  error?: string;
}

/** What a preview draws, a page per canvas, as SVG. */
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
  | { type: 'journal'; files: JournalPayload }
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

/** A lock type, as the chain editor's palette lists it. */
export interface LockType extends Whose {
  name: string;
  file?: string;
  description?: string;
  composite?: boolean;
  options?: LockOption[];
  /** Why it doesn't load, if it doesn't. */
  error?: string;
}

/** A lock's ports, `[name, type]`, for some options, with defaults filled in. */
export interface LockPorts {
  inputs: [string, string][];
  outputs: [string, string][];
  options: Record<string, unknown>;
  /** An illustration's inputs that draw with its own value when nothing's wired to them. */
  fallbacks?: string[];
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
    /** The journal's files it draws on, each a node with one output, "id.value". */
    sources?: Record<string, { type: 'content' | 'style' | 'glyphset' | 'layout'; name: string; options?: Record<string, unknown>; base?: number }>;
    /** Illustrations attached, each input drawing a port, "lock.port". */
    illustrations?: Record<string, { type: string; options?: Record<string, unknown>; inputs?: Record<string, string> }>;
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
