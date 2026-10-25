import { Injectable, computed, effect, signal, untracked } from '@angular/core';
import {
  BLANK_ENTRY, MANIFEST_VERSION, VALID_NAME, blankJournal, checkEntries, checkManifest, entryPath, journalHash, kindOf, snakeCase, splitPath,
  type JournalData, type Manifest,
} from '../journal/journal-files';
import { idbGet, idbSet } from './idb';

const KEY = 'journal';
/** How long edits pause before the journal is kept. */
const KEEP_DELAY_MS = 300;

/** What's kept between visits: the journal, and its hash when last saved (exported, imported or loaded). */
interface Kept {
  journal: JournalData;
  saved: string | null;
}

/** Two sets of files alike, path for path. */
function sameFiles(a: Record<string, string>, b: Record<string, string>): boolean {
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every((key) => a[key] === b[key]);
}

/**
 * The journal being edited: its manifest and its files by path, one
 * journal at a time, kept in this browser (IndexedDB) between visits.
 * Everything that runs scripts runs them with its files over the library.
 */
@Injectable({ providedIn: 'root' })
export class JournalStore {
  readonly journal = signal<JournalData>(blankJournal());
  /** The journal's hash when last saved; null if it never was. */
  private readonly saved = signal<string | null>(journalHash(this.journal()));
  /** Whether the journal kept from the last visit has been read back. */
  readonly restored = signal(false);
  /** Counts journals loaded (new, imported, opened), for the editors to start afresh. */
  readonly loads = signal(0);

  readonly name = computed(() => this.journal().name);
  readonly manifest = computed(() => this.journal().manifest);
  readonly files = computed(() => this.journal().files);
  /** Whether it's changed since it was last saved (exported, or loaded). */
  readonly dirty = computed(() => journalHash(this.journal()) !== this.saved());
  /** Its files besides the entries, which every script runs with: changes only when they do. */
  readonly resources = computed(() => {
    const files = this.files();
    return Object.fromEntries(Object.entries(files).filter(([path]) => kindOf(path) !== 'entries'));
  }, { equal: sameFiles });
  /** Entry files the manifest doesn't list. */
  readonly unlisted = computed(() => Object.keys(this.files())
    .filter((path) => kindOf(path) === 'entries' && !this.manifest().entries.some((entry) => entryPath(entry) === path)));

  private keepTimer: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    void this.restore();
    effect(() => {
      const kept: Kept = { journal: this.journal(), saved: this.saved() };
      if (!untracked(() => this.restored())) return;
      if (this.keepTimer) clearTimeout(this.keepTimer);
      this.keepTimer = setTimeout(() => void idbSet(KEY, kept).catch(() => {}), KEEP_DELAY_MS);
    });
    addEventListener('beforeunload', (event) => {
      if (this.dirty()) event.preventDefault();
    });
  }

  /** The journal from the last visit; failing that, a new one with the old editor's script and files in it. */
  private async restore() {
    let kept: Kept | undefined;
    try {
      kept = await idbGet<Kept>(KEY);
    } catch {
      kept = undefined;
    }
    if (kept?.journal?.manifest && kept.journal.files) {
      this.journal.set(kept.journal);
      this.saved.set(kept.saved ?? null);
    } else {
      const old = fromOldEditor();
      if (old) {
        this.journal.set(old);
        this.saved.set(null);
      }
    }
    this.restored.set(true);
    this.loads.update((n) => n + 1);
  }

  /** Edit `journal` from now on, as saved if `saved`. */
  load(journal: JournalData, saved = true) {
    checkEntries(journal.manifest, journal.files);
    this.journal.set(journal);
    this.saved.set(saved ? journalHash(journal) : null);
    this.loads.update((n) => n + 1);
  }

  /** Start a new journal: one blank entry, and the library. */
  newJournal() {
    this.load(blankJournal());
  }

  /** The journal as it now is has been saved (exported). */
  markSaved() {
    this.saved.set(journalHash(this.journal()));
  }

  private update(change: (journal: JournalData) => JournalData) {
    this.journal.update(change);
  }

  /** Write file `path`, adding it if it's new. */
  setFile(path: string, text: string) {
    if (this.files()[path] === text) return;
    this.update((j) => ({ ...j, files: { ...j.files, [path]: text } }));
  }

  /** Remove file `path` (an entry's too, from the manifest). */
  removeFile(path: string) {
    if (!(path in this.files())) return;
    const split = splitPath(path);
    this.update((j) => {
      const { [path]: _, ...files } = j.files;
      const entries = split?.kind === 'entries' ? j.manifest.entries.filter((e) => e !== split.name) : j.manifest.entries;
      return { ...j, files, manifest: { ...j.manifest, entries } };
    });
  }

  /** Move file `from` to `to`, an entry's in the manifest too. Returns why it can't, or null. */
  renameFile(from: string, to: string): string | null {
    if (from === to) return null;
    const files = this.files();
    if (!(from in files)) return `there's no ${from}`;
    if (to in files) return `there's a ${to} already`;
    const a = splitPath(from);
    const b = splitPath(to);
    if (!a || !b || a.kind !== b.kind) return `${to} isn't a ${a?.kind ?? ''} file's path`;
    if (!VALID_NAME.test(b.name)) return 'a name is letters, digits and _ only';
    this.update((j) => {
      const renamed = Object.fromEntries(Object.entries(j.files).map(([path, text]) => [path === from ? to : path, text]));
      const entries = a.kind === 'entries' ? j.manifest.entries.map((e) => (e === a.name ? b.name : e)) : j.manifest.entries;
      return { ...j, files: renamed, manifest: { ...j.manifest, entries } };
    });
    return null;
  }

  /** Add an entry, `name`, last. Returns why it can't, or null. */
  addEntry(name: string, source = BLANK_ENTRY): string | null {
    if (!VALID_NAME.test(name)) return 'an entry\'s name is letters, digits and _ only';
    if (entryPath(name) in this.files()) return `there's an entry ${name} already`;
    this.update((j) => ({
      ...j,
      files: { ...j.files, [entryPath(name)]: source },
      manifest: { ...j.manifest, entries: [...j.manifest.entries.filter((e) => e !== name), name] },
    }));
    return null;
  }

  /** List an entry file the manifest doesn't, last. */
  listEntry(name: string) {
    if (this.manifest().entries.includes(name)) return;
    this.update((j) => ({ ...j, manifest: { ...j.manifest, entries: [...j.manifest.entries, name] } }));
  }

  /** Move entry `name` `by` places in the manifest's order. */
  moveEntry(name: string, by: number) {
    this.update((j) => {
      const entries = [...j.manifest.entries];
      const from = entries.indexOf(name);
      const to = Math.max(0, Math.min(entries.length - 1, from + by));
      if (from < 0 || from === to) return j;
      entries.splice(to, 0, ...entries.splice(from, 1));
      return { ...j, manifest: { ...j.manifest, entries } };
    });
  }

  setManifest(patch: Partial<Pick<Manifest, 'title' | 'description'>>) {
    this.update((j) => ({ ...j, manifest: checkManifest({ ...j.manifest, ...patch }) }));
  }

  /** Retitle the journal, and name its directory (and an export's) for the title, in snake_case. */
  setTitle(title: string) {
    this.update((j) => ({ ...j, name: snakeCase(title, j.name), manifest: checkManifest({ ...j.manifest, title }) }));
  }
}

/**
 * What the editor kept before journals were directories: its script, and
 * the lock, chain and illustration files only this browser had, as a
 * journal. Null if it kept nothing.
 */
function fromOldEditor(): JournalData | null {
  const read = (key: string) => {
    try {
      return JSON.parse(localStorage.getItem(key) ?? 'null');
    } catch {
      return null;
    }
  };
  const draft = read('journal-editor.draft');
  const workspace = read('journal-editor.workspace');
  if (!draft && !workspace) return null;
  const files: Record<string, string> = {};
  const name = typeof draft?.name === 'string' ? draft.name.replace(/\.rhai$/, '').replace(/[^A-Za-z0-9_]/g, '_') || 'entry_1' : 'entry_1';
  files[entryPath(name)] = typeof draft?.source === 'string' ? draft.source : BLANK_ENTRY;
  for (const [kind, key] of [['locks', 'locks'], ['chains', 'chains'], ['illustrations', 'renderers']] as const) {
    const kept = workspace?.[key];
    if (kept && typeof kept === 'object') {
      for (const [file, text] of Object.entries(kept)) if (typeof text === 'string' && VALID_NAME.test(file)) files[`${kind}/${file}.rhai`] = text;
    }
  }
  return { name: 'untitled', manifest: { version: MANIFEST_VERSION, title: 'Untitled', entries: [name] }, files };
}
