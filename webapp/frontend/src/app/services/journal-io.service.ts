import { Injectable, inject, signal } from '@angular/core';
import { checkManifest, entryPath, journalName, packZip, unpackZip } from '../journal/journal-files';
import { readDirectory } from '../journal/journal-folder';
import { FileService, ZIP_FILES } from './file.service';
import { JournalStore } from './journal-store.service';
import { LibraryService } from './library.service';
import { ToolsService, failed } from './tools.service';

/** A journal built into the editor, as the Open menu lists it. */
export interface RepositoryJournal {
  name: string;
  title: string;
  description?: string;
  entries: string[];
}

/**
 * Journals in and out: the built-in ones (copied into the site by
 * scripts/examples.mjs), zips and journals' directories imported, zips
 * exported. Nothing is merged: a journal loaded replaces the one being
 * edited.
 */
@Injectable({ providedIn: 'root' })
export class JournalIoService {
  private readonly store = inject(JournalStore);
  private readonly tools = inject(ToolsService);
  private readonly library = inject(LibraryService);
  private readonly files = inject(FileService);

  /** The built-in journals; empty until fetched. */
  readonly repository = signal<RepositoryJournal[]>([]);
  /** An export under way. */
  readonly exporting = signal(false);

  constructor() {
    fetch('journals/index.json')
      .then((r) => (r.ok ? r.json() : []))
      .then((journals: RepositoryJournal[]) => this.repository.set(journals))
      .catch(() => this.repository.set([]));
  }

  /** Load the built-in journal `name`. */
  async openRepository(name: string) {
    const response = await fetch(`journals/${encodeURIComponent(name)}.json`);
    if (!response.ok) throw new Error(`couldn't fetch the journal ${name}: ${response.status}`);
    const data = await response.json() as { manifest: unknown; files: Record<string, string> };
    this.store.load({ name, manifest: checkManifest(data.manifest), files: data.files });
  }

  /** Let the user pick a zipped journal, and load it. Returns false if they don't pick one. */
  async importZip(): Promise<boolean> {
    const picked = await this.files.openBytes(ZIP_FILES);
    if (!picked) return false;
    const journal = unpackZip(picked.bytes, journalName(picked.name.replace(/\.zip$/i, ''), 'imported'));
    this.store.load(journal);
    return true;
  }

  /**
   * Let the user pick a journal's directory, unzipped (one of the
   * repository's journals/, say), and load it. Returns false if they don't
   * pick one.
   */
  async importDirectory(): Promise<boolean> {
    const picked = await this.files.openDirectory();
    if (!picked) return false;
    this.store.load(await readDirectory(picked));
    return true;
  }

  /**
   * The journal as a zip, to download: its files, and the library files its
   * entries use, so it renders as it does here wherever the library has
   * changed. Returns the library files bundled.
   */
  async exportZip(): Promise<string[]> {
    this.exporting.set(true);
    try {
      const journal = this.store.journal();
      const entries = journal.manifest.entries.map((entry) => journal.files[entryPath(entry)] ?? '');
      const used = await this.tools.usedLibrary(entries);
      if (failed(used)) throw new Error(`couldn't tell which library files the journal uses: ${used.error}`);
      const library = this.library.files();
      const bundled = Object.fromEntries(used.paths.filter((path) => path in library).map((path) => [path, library[path]]));
      const zip = packZip(journal, bundled);
      this.files.download(new Blob([zip as BlobPart], { type: 'application/zip' }), `${journal.name}.zip`);
      this.store.markSaved();
      return Object.keys(bundled).filter((path) => !(path in journal.files));
    } finally {
      this.exporting.set(false);
    }
  }
}
