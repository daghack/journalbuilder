import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import { linkedFiles, linkedPath, linksAFile } from '../journal/linked-content';
import { idbGet, idbSet } from './idb';
import { JournalStore } from './journal-store.service';

// The File System Access API's handles (Chromium only), as far as links use them.
interface FileHandle {
  kind: 'file';
  name: string;
  getFile(): Promise<File>;
}
interface DirectoryHandle {
  kind: 'directory';
  name: string;
  values(): AsyncIterable<FileHandle | DirectoryHandle>;
}
type Handle = (FileHandle | DirectoryHandle) & {
  queryPermission?(options: { mode: 'read' }): Promise<PermissionState>;
  requestPermission?(options: { mode: 'read' }): Promise<PermissionState>;
};
type PickerWindow = Window & {
  showOpenFilePicker?: (options?: object) => Promise<FileHandle[]>;
  showDirectoryPicker?: (options?: object) => Promise<DirectoryHandle>;
};

/** Where each journal's links are kept: by journal name, each link's handle by its path. */
const KEY = 'links';
/** How often linked files are looked at for changes, while the page is showing. */
const POLL_MS = 1000;

/**
 * A link's state: read (connected), waiting for the user to let the page
 * read it again (permission), not yet picked here (unconnected), gone or
 * unreadable (missing), or not readable in this browser (unsupported).
 */
export type LinkState = 'connected' | 'permission' | 'unconnected' | 'missing' | 'unsupported';

export interface Link {
  path: string;
  state: LinkState;
  /** What it was picked as: its file's or folder's name. */
  picked?: string;
  /** Its files, by their path in the journal. */
  files: Record<string, string>;
}

/** A file or folder picked to link, not yet linked. */
export interface PickedLink {
  as: 'file' | 'folder';
  name: string;
  handle: unknown;
}

interface Read {
  handle: Handle;
  /** Its files' names, times and sizes, to tell when it's changed. */
  stamp: string;
  files: Record<string, string>;
}

/**
 * The journal's linked content (journal.json's linked_content): each link
 * a file or folder the user picks on their disk, kept between visits,
 * read before the journal's scripts run and again whenever it changes,
 * in front of the journal's own content.
 */
@Injectable({ providedIn: 'root' })
export class LinkedContentService {
  private readonly store = inject(JournalStore);
  private readonly picker = window as PickerWindow;
  /** Whether this browser can link: it has the File System Access API. */
  readonly supported = typeof this.picker.showDirectoryPicker === 'function';

  private readonly handles = signal<Record<string, Handle>>({});
  private readonly states = signal<Record<string, LinkState>>({});
  private readonly reads = signal<Record<string, Read>>({});
  /** Bumped by each journal load, so reads for one journal don't land on the next. */
  private generation = 0;

  /** Each link the manifest lists, in its order. */
  readonly links = computed<Link[]>(() => {
    const handles = this.handles();
    const states = this.states();
    const reads = this.reads();
    return (this.store.manifest().linked_content ?? []).map((path) => ({
      path,
      state: this.supported ? states[path] ?? 'unconnected' : 'unsupported',
      picked: handles[path]?.name,
      files: reads[path]?.files ?? {},
    }));
  });

  constructor() {
    effect(() => {
      if (!this.store.loads()) return;
      void untracked(() => this.restore(this.store.name()));
    });
    // A journal retitled is renamed: its links are kept under its new name.
    let named = this.store.name();
    effect(() => {
      const name = this.store.name();
      if (name === named) return;
      named = name;
      if (Object.keys(untracked(() => this.handles())).length) void this.keep();
    });
    effect(() => {
      const links = this.links();
      untracked(() => this.store.setLinked(linkedFiles(links.filter((l) => l.state === 'connected').map((l) => l.files))));
    });
    setInterval(() => {
      if (document.visibilityState === 'visible') void this.refresh();
    }, POLL_MS);
  }

  /** The journal `name`'s links, as kept from the last visit, read if the page may still read them. */
  private async restore(name: string) {
    const generation = ++this.generation;
    this.handles.set({});
    this.states.set({});
    this.reads.set({});
    if (!this.supported) return;
    const kept = (await idbGet<Record<string, Record<string, Handle>>>(KEY).catch(() => undefined))?.[name] ?? {};
    if (generation !== this.generation) return;
    this.handles.set(kept);
    for (const [path, handle] of Object.entries(kept)) {
      const permission = await handle.queryPermission?.({ mode: 'read' }).catch(() => 'denied' as const) ?? 'granted';
      if (generation !== this.generation) return;
      if (permission === 'granted') await this.read(path, handle, generation);
      else this.setState(path, 'permission');
    }
  }

  /** Keep the journal's handles for the next visit. */
  private async keep() {
    const all = (await idbGet<Record<string, Record<string, Handle>>>(KEY).catch(() => undefined)) ?? {};
    all[this.store.name()] = this.handles();
    await idbSet(KEY, all).catch(() => {});
  }

  private setState(path: string, state: LinkState) {
    this.states.update((states) => ({ ...states, [path]: state }));
  }

  /** Read link `path` from `handle`, if it's changed since it was last read. */
  private async read(path: string, handle: Handle, generation = this.generation) {
    try {
      const entries = handle.kind === 'file' ? [handle] : await listFiles(handle);
      const found = await Promise.all(entries.map(async (entry) => ({ entry, file: await entry.getFile() })));
      const stamp = found.map(({ entry, file }) => `${entry.name}:${file.lastModified}:${file.size}`).join('|');
      if (generation !== this.generation) return;
      if (this.reads()[path]?.stamp === stamp && this.states()[path] === 'connected') return;
      const files: Record<string, string> = {};
      for (const { entry, file } of found) {
        const at = linkedPath(entry.name);
        if (at) files[at] = await file.text();
      }
      if (generation !== this.generation) return;
      this.reads.update((reads) => ({ ...reads, [path]: { handle, stamp, files } }));
      this.setState(path, 'connected');
    } catch {
      if (generation !== this.generation) return;
      this.reads.update(({ [path]: _, ...reads }) => reads);
      this.setState(path, 'missing');
    }
  }

  /** Read every connected link again that's changed. */
  async refresh() {
    const handles = this.handles();
    for (const link of this.links()) {
      const handle = handles[link.path];
      if (handle && (link.state === 'connected' || link.state === 'missing')) await this.read(link.path, handle);
    }
  }

  /** Let the user pick a file or folder for link `path` (one it names by extension is a file), and read it. */
  async connect(path: string): Promise<string | null> {
    const handle = await this.pick(linksAFile(path) ? 'file' : 'folder');
    if (!handle) return null;
    return this.attach(path, handle);
  }

  /** Ask again to read link `path`, which the page may no longer read: a click must call it. */
  async reconnect(path: string): Promise<string | null> {
    const handle = this.handles()[path];
    if (!handle) return this.connect(path);
    const permission = await handle.requestPermission?.({ mode: 'read' }).catch(() => 'denied' as const) ?? 'granted';
    if (permission !== 'granted') return "The browser wasn't allowed to read it.";
    await this.read(path, handle);
    return null;
  }

  /** Let the user pick a file or folder to link; null if they don't. */
  async pickNew(as: 'file' | 'folder'): Promise<PickedLink | null> {
    const handle = await this.pick(as);
    return handle ? { as, name: handle.name, handle } : null;
  }

  /**
   * Link `picked`, listed in journal.json as `path`, where journalbuild
   * finds it, and read it. Returns why it wasn't linked, or null.
   */
  async add(picked: PickedLink, path: string): Promise<string | null> {
    path = path.trim();
    if (!path) return 'Give the path journalbuild finds it at.';
    if (picked.as === 'file' && !linksAFile(path)) return "A linked file's path ends in its extension: .txt, .md, .json or .graph.";
    if (picked.as === 'folder' && linksAFile(path)) return "A linked folder's path can't end in a content file's extension.";
    const linked = this.store.manifest().linked_content ?? [];
    if (linked.includes(path)) return `${path} is linked already.`;
    this.store.setManifest({ linked_content: [...linked, path] });
    return this.attach(path, picked.handle as Handle);
  }

  /** Stop linking `path`: it's taken out of journal.json, and its files with it. */
  unlink(path: string) {
    this.store.setManifest({ linked_content: (this.store.manifest().linked_content ?? []).filter((p) => p !== path) });
    this.handles.update(({ [path]: _, ...handles }) => handles);
    this.reads.update(({ [path]: _, ...reads }) => reads);
    this.states.update(({ [path]: _, ...states }) => states);
    void this.keep();
  }

  private async attach(path: string, handle: Handle): Promise<string | null> {
    this.handles.update((handles) => ({ ...handles, [path]: handle }));
    await this.keep();
    await this.read(path, handle);
    return this.states()[path] === 'connected' ? null : `${handle.name} couldn't be read.`;
  }

  private async pick(as: 'file' | 'folder'): Promise<Handle | null> {
    if (!this.supported) return null;
    try {
      if (as === 'folder') return await this.picker.showDirectoryPicker!({ mode: 'read' });
      const [handle] = await this.picker.showOpenFilePicker!({
        types: [{ description: 'Content', accept: { 'text/plain': ['.txt', '.md', '.json', '.graph'] } }],
      });
      return handle ?? null;
    } catch (err) {
      if (err instanceof DOMException && err.name === 'AbortError') return null;
      throw err;
    }
  }
}

/** The files directly in `dir`, sorted by name. */
async function listFiles(dir: DirectoryHandle): Promise<FileHandle[]> {
  const files: FileHandle[] = [];
  for await (const entry of dir.values()) if (entry.kind === 'file') files.push(entry);
  return files.sort((a, b) => a.name.localeCompare(b.name));
}
