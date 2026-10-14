import { Component, inject, signal } from '@angular/core';
import { JournalService } from '../services/journal.service';
import { ShellService } from '../shell/shell.service';

/** Above the editor: the file's name, new, open and save, and the repository's journals as examples. */
@Component({
  selector: 'app-top-bar',
  host: { '(window:keydown)': 'onKey($event)' },
  template: `
    <span class="name" [title]="journal.fileName()">
      {{ journal.fileName() }}@if (journal.dirty()) {<span class="dirty" title="Unsaved changes"> ●</span>}
    </span>
    @if (journal.evaluating()) { <span class="busy" title="Running the script">running…</span> }
    <button type="button" (click)="newJournal()" title="Start a new, blank journal">New</button>
    <button type="button" (click)="journal.open()" title="Open a script (Ctrl/⌘ O)">Open</button>
    <button type="button" (click)="journal.save()" title="Save to a file you pick (Ctrl/⌘ S)">Save</button>
    <select aria-label="Open an example" (change)="pick($any($event.target))">
      <option value="">Examples…</option>
      @for (name of examples(); track name) { <option [value]="name">{{ name }}</option> }
    </select>
  `,
  styles: `
    :host { display: flex; align-items: center; gap: 6px; padding: 6px 10px; background: var(--surface); border-bottom: 1px solid var(--border); font-size: 12px; }
    .name { flex: 1; font-weight: 600; font-family: var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
    .dirty { color: var(--accent); }
    button, select { white-space: nowrap; flex: none; }
    .busy { color: var(--muted); }
  `,
})
export class TopBarComponent {
  protected readonly journal = inject(JournalService);
  private readonly shell = inject(ShellService);
  protected readonly examples = signal<string[]>([]);

  constructor() {
    fetch('examples/index.json')
      .then((r) => (r.ok ? r.json() : []))
      .then((names: string[]) => this.examples.set(names))
      .catch(() => this.examples.set([]));
  }

  protected newJournal() {
    if (this.discard()) this.journal.newJournal();
  }

  protected async pick(select: HTMLSelectElement) {
    const name = select.value;
    select.value = '';
    if (!name) return;
    if (!this.discard()) return;
    const source = await loadExample(name);
    if (source !== null) this.journal.load(`${name}.rhai`, source);
  }

  /** Whether it's fine to replace the script: it's saved, or the user says to throw its changes away. */
  private discard(): boolean {
    return !this.journal.dirty() || confirm(`Discard unsaved changes to ${this.journal.fileName()}?`);
  }

  protected onKey(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey) || this.shell.mode() !== 'journal') return;
    const key = event.key.toLowerCase();
    if (key === 's') {
      event.preventDefault();
      void this.journal.save();
    } else if (key === 'o') {
      event.preventDefault();
      void this.journal.open();
    }
  }
}

/** The repository's journal `name`, or null if it can't be fetched. */
export async function loadExample(name: string): Promise<string | null> {
  const response = await fetch(`examples/${encodeURIComponent(name)}.rhai`).catch(() => null);
  return response?.ok ? response.text() : null;
}
