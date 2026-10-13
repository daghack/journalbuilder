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
  | { type: 'renderPages'; id: number; source: string; indices: number[]; options: RenderOptions }
  | { type: 'exportPdf'; id: number; source: string; title: string; options: RenderOptions };

export type DarklyReply =
  | { type: 'progress'; id: number; page: number; pages: number; fraction: number }
  /** A page rendered by renderPages: a PNG, or null if nothing's drawn on it. */
  | { type: 'page'; id: number; index: number; png: ArrayBuffer | null }
  | { type: 'done'; id: number; pdf?: ArrayBuffer }
  | { type: 'error'; id: number; message: string };
