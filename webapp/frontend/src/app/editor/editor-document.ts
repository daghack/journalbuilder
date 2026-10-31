import { InjectionToken, computed, inject, type Signal } from '@angular/core';
import { entryPath } from '../journal/journal-files';
import { JournalService } from '../services/journal.service';

/** Where an error or warning is, and what it says. */
export interface DocumentProblem {
  message: string;
  line?: number | null;
  column?: number | null;
}

/** What the script editor edits: its text, and the last run's problems with it. */
export interface EditorDocument {
  /** The file it is, which keeps its cursor and undo history while another shows. */
  path: Signal<string | null>;
  source: Signal<string>;
  setSource(source: string): void;
  error: Signal<DocumentProblem | null>;
  warnings: Signal<string[]>;
}

/** The document an editor edits: the journal's, unless a component provides another. */
export const EDITOR_DOCUMENT = new InjectionToken<EditorDocument>('the editor\'s document', {
  providedIn: 'root',
  factory: () => {
    const journal = inject(JournalService);
    return {
      path: computed(() => {
        const entry = journal.entry();
        return entry === null ? null : entryPath(entry);
      }),
      source: journal.source,
      setSource: (source: string) => journal.setSource(source),
      error: journal.error,
      warnings: computed(() => journal.evaluation()?.warnings ?? []),
    };
  },
});

/** Where a rhai error message says it is: "(line 3, position 7)". */
export function problemAt(message: string): DocumentProblem {
  const at = /\(line (\d+), position (\d+)\)/.exec(message);
  return { message, line: at ? Number(at[1]) : null, column: at ? Number(at[2]) : null };
}
