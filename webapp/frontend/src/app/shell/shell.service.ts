import { Injectable, computed, effect, inject, linkedSignal, signal, untracked, type Signal } from '@angular/core';
import { entryPath, kindOf, splitPath, type Kind } from '../journal/journal-files';
import { keep, stored } from '../layout/pane-state';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { keptTabs, lastUsed, renameTab, showTab, type FileTab } from './file-tabs';

function fromHash(): string | null {
  const path = decodeURIComponent(location.hash.slice(1));
  return splitPath(path) ? path : null;
}

/**
 * Which file the editor shows: one of the journal's, or one of the
 * library's (to read, or copy in), kept in the address's hash
 * (#entries/letter_1.rhai) so a reload keeps it; and the files opened
 * lately, each with a tab, which a reload keeps too.
 */
@Injectable({ providedIn: 'root' })
export class ShellService {
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);

  readonly selected = signal<string | null>(fromHash());
  /** Whether the file shown is the journal's own (else it's the library's). */
  readonly own = computed(() => {
    const path = this.selected();
    return path !== null && path in this.store.files();
  });
  /** Whether the last entry's pages show over whatever file is open (Ctrl/⌘+P). */
  readonly previewOpen = signal(false);
  /** The files with tabs, left to right as they were opened. */
  readonly tabs = signal<FileTab[]>(keptTabs(stored('tabs', [])));

  /** Whether there's a file at a path, as far as is known: the library may not be read yet. */
  private readonly exists = computed(() => {
    const files = this.store.files();
    const library = this.library.files();
    const loaded = this.library.loaded();
    return (path: string) => path in files || path in library || (!loaded && splitPath(path)?.kind !== 'entries');
  });

  constructor() {
    addEventListener('hashchange', () => {
      const path = fromHash();
      if (path) this.selected.set(path);
    });
    effect(() => {
      const path = this.selected();
      const hash = path ? `#${encodeURI(path)}` : '';
      if (location.hash !== hash) history.replaceState(null, '', hash || location.pathname + location.search);
    });
    // A file that's gone (removed, or another journal loaded): show the first entry.
    effect(() => {
      const exists = this.exists();
      const restored = this.store.restored();
      untracked(() => {
        const path = this.selected();
        if (!restored || (path && exists(path))) return;
        const first = this.store.manifest().entries[0];
        this.selected.set(first ? entryPath(first) : null);
      });
    });
    // Another journal loaded (the first load is the one kept from the last
    // visit, whose file and tabs stay): start at its first entry, with no tabs of the last one's.
    effect(() => {
      if (this.store.loads() <= 1) return;
      untracked(() => {
        this.tabs.set([]);
        const path = this.selected();
        if (path && path in this.store.files()) return;
        const first = this.store.manifest().entries[0];
        this.selected.set(first ? entryPath(first) : null);
      });
    });
    // The file showing has a tab, and a file that's gone hasn't.
    effect(() => {
      const path = this.selected();
      const tabs = this.tabs();
      const exists = this.exists();
      if (!this.store.restored()) return;
      const there = tabs.every((tab) => exists(tab.path)) ? tabs : tabs.filter((tab) => exists(tab.path));
      this.tabs.set(path && exists(path) ? showTab(there, path) : there);
    });
    effect(() => keep('tabs', this.tabs()));
    this.store.onRename((from, to) => this.tabs.update((tabs) => renameTab(tabs, from, to)));
  }

  open(path: string) {
    this.selected.set(path);
  }

  /** Close the tab of `path` (the file stays), unless it's the only one. If it's showing, the file used before it shows. */
  closeTab(path: string) {
    const tabs = this.tabs().filter((tab) => tab.path !== path);
    if (!tabs.length) return;
    this.tabs.set(tabs);
    if (this.selected() === path) this.selected.set(lastUsed(tabs));
  }

  /** The last file opened of `kinds`, which stays while a file of another kind shows, for the editor that stays as it was. */
  lastOpened(kinds: readonly Kind[]): Signal<string | null> {
    return linkedSignal<string | null, string | null>({
      source: this.selected,
      computation: (path, previous) => {
        const kind = path ? kindOf(path) : null;
        return kind && kinds.includes(kind) ? path : (previous?.value ?? null);
      },
    });
  }
}
