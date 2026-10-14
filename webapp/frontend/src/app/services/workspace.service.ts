import { Injectable, computed, signal } from '@angular/core';

const KEY = 'journal-editor.workspace';

/** What's kept between visits. */
interface Stored {
  /** The edited fonts.json, or null for the built-in one. */
  fonts: string | null;
  /** A hash of the built-in fonts.json the edits started from. */
  fontsBase: string | null;
  /** Lock files by type name, not yet in the repository. */
  locks: Record<string, string>;
}

/**
 * Edits not yet saved into the repository, which every script runs with:
 * fonts from the font editor in place of config/fonts.json, and lock files
 * from the lock editor added to locks/. The renderer's workers are sent
 * them whenever they change.
 */
@Injectable({ providedIn: 'root' })
export class WorkspaceService {
  readonly fonts = signal<string | null>(null);
  readonly fontsBase = signal<string | null>(null);
  readonly locks = signal<Record<string, string>>({});
  /** Counts changes, so what was made with an older workspace can tell. */
  readonly version = signal(0);
  /** What the workers are sent. */
  readonly payload = computed(() => ({ fonts: this.fonts(), locks: this.locks() }));

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

  private changed() {
    this.version.update((v) => v + 1);
    const stored: Stored = { fonts: this.fonts(), fontsBase: this.fontsBase(), locks: this.locks() };
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
