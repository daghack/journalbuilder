import { signal } from '@angular/core';

// What the editors' panes keep between editors and reloads.

export function stored<T>(key: string, fallback: T): T {
  try {
    return JSON.parse(localStorage.getItem(`journal-editor.panes.${key}`) ?? 'null') ?? fallback;
  } catch {
    return fallback;
  }
}

export function keep(key: string, value: unknown) {
  try {
    localStorage.setItem(`journal-editor.panes.${key}`, JSON.stringify(value));
  } catch {
    // No storage: it just isn't kept.
  }
}

/** Whether the left pane shows the journal's files, in every editor. */
export const filesShown = signal<boolean>(stored('files', true));

/** Leave the Files tab for the editor's own left tab: a file was picked to edit. */
export function showEditorTab() {
  filesShown.set(false);
  keep('files', false);
}
