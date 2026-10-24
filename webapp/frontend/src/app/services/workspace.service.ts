import { Injectable, computed, signal } from '@angular/core';

const KEY = 'journal-editor.workspace';
/** The editors' drafts of what they're editing, which clearing throws away too. */
const DRAFT_KEYS = ['journal-editor.fonts-draft', 'journal-editor.lock-draft', 'journal-editor.renderer-draft'];

/** What's kept between visits. */
interface Stored {
  /** The edited fonts.json, or null for the built-in one. */
  fonts: string | null;
  /** A hash of the built-in fonts.json the edits started from. */
  fontsBase: string | null;
  /** Lock files by type name, not yet in the repository. */
  locks: Record<string, string>;
  /** Renderer files by type name, likewise. */
  renderers?: Record<string, string>;
  /** Chain files by name, likewise. */
  chains?: Record<string, string>;
}

/**
 * Edits not yet saved into the repository, which every script runs with:
 * fonts from the font editor in place of webapp/library/glyphsets/, chain files from
 * the lock editor added to webapp/library/chains/ (the chain being edited among
 * them, as it's edited), renderer files added to webapp/library/illustrations/, and
 * lock files added to webapp/library/locks/. The renderer's workers are sent them
 * whenever they change.
 */
@Injectable({ providedIn: 'root' })
export class WorkspaceService {
  readonly fonts = signal<string | null>(null);
  readonly fontsBase = signal<string | null>(null);
  readonly locks = signal<Record<string, string>>({});
  readonly renderers = signal<Record<string, string>>({});
  readonly chains = signal<Record<string, string>>({});
  /** Counts clears, for the editors to start again from the repository's files. */
  readonly cleared = signal(0);
  /** Counts changes, so what was made with an older workspace can tell. */
  readonly version = signal(0);
  /** What the workers are sent. */
  readonly payload = computed(() => ({ fonts: this.fonts(), locks: this.locks(), renderers: this.renderers(), chains: this.chains() }));

  constructor() {
    this.load();
    // Another tab changed it.
    addEventListener('storage', (event) => {
      if (event.key === KEY) this.load(true);
    });
  }

  /** Use `fonts` (a fonts.json) in place of the built-in one, or the built-in one again with null. */
  setFonts(fonts: string | null, base: string | null = this.fontsBase()) {
    if (fonts === this.fonts()) return;
    this.fonts.set(fonts);
    this.fontsBase.set(fonts === null ? null : base);
    this.changed();
  }

  /** Add the lock file `source` as the type `name`, or replace it. */
  setLock(name: string, source: string) {
    if (this.locks()[name] === source) return;
    this.locks.update((locks) => ({ ...locks, [name]: source }));
    this.changed();
  }

  removeLock(name: string) {
    if (!(name in this.locks())) return;
    this.locks.update(({ [name]: _, ...rest }) => rest);
    this.changed();
  }

  /** Add the renderer file `source` as the type `name`, or replace it. */
  setRenderer(name: string, source: string) {
    if (this.renderers()[name] === source) return;
    this.renderers.update((renderers) => ({ ...renderers, [name]: source }));
    this.changed();
  }

  removeRenderer(name: string) {
    if (!(name in this.renderers())) return;
    this.renderers.update(({ [name]: _, ...rest }) => rest);
    this.changed();
  }

  /** Add the chain file `source` as the chain `name`, or replace it. */
  setChain(name: string, source: string) {
    if (this.chains()[name] === source) return;
    this.chains.update((chains) => ({ ...chains, [name]: source }));
    this.changed();
  }

  removeChain(name: string) {
    if (!(name in this.chains())) return;
    this.chains.update(({ [name]: _, ...rest }) => rest);
    this.changed();
  }

  /** Whether anything differs from the repository's files. */
  readonly empty = computed(() => this.fonts() === null && !Object.keys(this.locks()).length
    && !Object.keys(this.renderers()).length && !Object.keys(this.chains()).length);

  /**
   * Throw away every edit: fonts, chains, renderers and lock files, and the
   * editors' drafts, so every script runs with the repository's files again.
   */
  clear() {
    this.fonts.set(null);
    this.fontsBase.set(null);
    this.locks.set({});
    this.renderers.set({});
    this.chains.set({});
    for (const key of DRAFT_KEYS) {
      try {
        localStorage.removeItem(key);
      } catch {
        // Nothing kept, then.
      }
    }
    this.changed();
    this.cleared.update((n) => n + 1);
  }

  private changed() {
    this.version.update((v) => v + 1);
    const stored: Stored = { fonts: this.fonts(), fontsBase: this.fontsBase(), locks: this.locks(), renderers: this.renderers(), chains: this.chains() };
    try {
      localStorage.setItem(KEY, JSON.stringify(stored));
    } catch {
      // No storage (or it's full): the workspace lasts this visit only.
    }
  }

  private load(bump = false) {
    let stored: Partial<Stored> = {};
    try {
      stored = JSON.parse(localStorage.getItem(KEY) ?? '{}') ?? {};
    } catch {
      stored = {};
    }
    this.fonts.set(typeof stored.fonts === 'string' ? stored.fonts : null);
    this.fontsBase.set(typeof stored.fontsBase === 'string' ? stored.fontsBase : null);
    this.locks.set(stored.locks && typeof stored.locks === 'object' ? stored.locks : {});
    this.renderers.set(stored.renderers && typeof stored.renderers === 'object' ? stored.renderers : {});
    this.chains.set(stored.chains && typeof stored.chains === 'object' ? stored.chains : {});
    if (bump) this.version.update((v) => v + 1);
  }
}

/** FNV-1a of `text`, to tell versions of a file apart. */
export function hash(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(36) + text.length.toString(36);
}
