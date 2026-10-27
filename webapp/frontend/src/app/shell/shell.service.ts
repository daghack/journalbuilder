import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import { entryPath, splitPath } from '../journal/journal-files';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';

function fromHash(): string | null {
  const path = decodeURIComponent(location.hash.slice(1));
  return splitPath(path) ? path : null;
}

/**
 * Which file the editor shows: one of the journal's, or one of the
 * library's (to read, or copy in), kept in the address's hash
 * (#entries/letter_1.rhai) so a reload keeps it.
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
      const files = this.store.files();
      const restored = this.store.restored();
      const libraryLoaded = this.library.loaded();
      const library = this.library.files();
      untracked(() => {
        const path = this.selected();
        if (!restored) return;
        if (path && (path in files || (path in library) || (!libraryLoaded && splitPath(path)?.kind !== 'entries'))) return;
        const first = this.store.manifest().entries[0];
        this.selected.set(first ? entryPath(first) : null);
      });
    });
    // Another journal loaded: start at its first entry.
    effect(() => {
      if (!this.store.loads()) return;
      untracked(() => {
        const path = this.selected();
        if (path && path in this.store.files()) return;
        const first = this.store.manifest().entries[0];
        this.selected.set(first ? entryPath(first) : null);
      });
    });
  }

  open(path: string) {
    this.selected.set(path);
  }
}
