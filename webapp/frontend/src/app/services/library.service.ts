import { Injectable, computed, inject, signal } from '@angular/core';
import { kindOf, type Kind } from '../journal/journal-files';
import { ToolsService } from './tools.service';

/**
 * The built-in library every journal stands in front of: its files by
 * path, as the engine has them compiled in, read once.
 */
@Injectable({ providedIn: 'root' })
export class LibraryService {
  private readonly tools = inject(ToolsService);
  /** The library's files by path; empty until read. */
  readonly files = signal<Record<string, string>>({});
  readonly loaded = signal(false);
  readonly error = signal<string | null>(null);

  /** The library's paths, by kind. */
  readonly byKind = computed(() => {
    const kinds = new Map<Kind, string[]>();
    for (const path of Object.keys(this.files()).sort()) {
      const kind = kindOf(path);
      if (kind) kinds.set(kind, [...(kinds.get(kind) ?? []), path]);
    }
    return kinds;
  });

  constructor() {
    this.tools.libraryFiles().then(
      (files) => {
        this.files.set(files);
        this.loaded.set(true);
      },
      (err: Error) => this.error.set(err.message),
    );
  }

  /** Whether the library has a file at `path`. */
  has(path: string): boolean {
    return path in this.files();
  }
}
