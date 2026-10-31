import { Injectable, inject, signal } from '@angular/core';
import { checkManifest, entryPath, journalName, packZip, snakeCase, unpackZip, type JournalData } from '../journal/journal-files';
import { readDirectory } from '../journal/journal-folder';
import { FileService, ZIP_FILES } from './file.service';
import { JournalStore } from './journal-store.service';
import { LibraryService } from './library.service';
import { ToolsService } from './tools.service';

/** A journal built into the editor, as the Open menu lists it. */
export interface RepositoryJournal {
  name: string;
  title: string;
  description?: string;
  entries: string[];
}

/** An entry whose script failed, by name, and why. */
export interface EntryFailure {
  entry: string;
  message: string;
}

/** A zip exported: the name it was saved as, the library files bundled, and the entries that failed. */
export interface ExportedZip {
  saved: string;
  bundled: string[];
  failures: EntryFailure[];
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
   * changed. Entries whose scripts fail don't stop it: every file of the
   * journal goes in, theirs too, and of the library files they use, those
   * they got to. It's saved where the user picks, named for the journal's
   * title in snake_case to begin with. Returns what it did, or null if the
   * user didn't pick anywhere.
   */
  async exportZip(): Promise<ExportedZip | null> {
    this.exporting.set(true);
    try {
      const journal = this.store.journal();
      // Where it goes first, while the click is recent enough for the browser to show its picker: finding the library files takes a while.
      const target = await this.files.saveTarget(`${snakeCase(journal.manifest.title, journal.name)}.zip`, ZIP_FILES);
      if (!target) return null;
      const { paths, failures } = await this.usedLibrary(journal);
      const library = this.library.files();
      const bundled = Object.fromEntries(paths.filter((path) => path in library).map((path) => [path, library[path]]));
      const zip = packZip(journal, bundled);
      await target.write(new Blob([zip as BlobPart], { type: 'application/zip' }));
      this.store.markSaved();
      return { saved: target.name, bundled: Object.keys(bundled).filter((path) => !(path in journal.files)), failures };
    } finally {
      this.exporting.set(false);
    }
  }

  /**
   * The library files `journal`'s entries use, and those that fail, and why.
   * Each is run in a call of its own, so one that crashes the engine or runs
   * too long costs only its own files.
   */
  private async usedLibrary(journal: JournalData): Promise<{ paths: string[]; failures: EntryFailure[] }> {
    const paths = new Set<string>();
    const failures: EntryFailure[] = [];
    for (const entry of journal.manifest.entries) {
      try {
        const used = await this.tools.usedLibrary(journal.files[entryPath(entry)] ?? '');
        for (const path of used.paths) paths.add(path);
        if (used.error) failures.push({ entry, message: used.error.message });
      } catch (err) {
        failures.push({ entry, message: err instanceof Error ? err.message : String(err) });
      }
    }
    return { paths: [...paths], failures };
  }
}
