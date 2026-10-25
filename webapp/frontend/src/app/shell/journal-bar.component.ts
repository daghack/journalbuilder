import { Component, inject, signal } from '@angular/core';
import { JournalIoService } from '../services/journal-io.service';
import { JournalStore } from '../services/journal-store.service';

/** Across the top: the journal's name, and new, open, import and export. */
@Component({
  selector: 'app-journal-bar',
  template: `
    <span class="name" [title]="'The journal: ' + store.manifest().title">
      <button type="button" class="dir" (click)="rename()" title="Rename the journal's directory (its zip's name)">{{ store.name() }}/</button>
      <span class="title">{{ store.manifest().title }}</span>
      @if (store.dirty()) { <span class="dirty" title="Changed since it was last exported">●</span> }
    </span>
    <span class="spacer"></span>
    @if (message(); as m) { <span class="message" [class.error]="m.error">{{ m.text }}</span> }
    <button type="button" (click)="newJournal()" title="Start a new journal: one blank entry, and the library">New</button>
    <div class="menu">
      <button type="button" aria-haspopup="true" title="Open one of the repository's journals">Open ▾</button>
      <ul class="list" aria-label="The repository's journals">
        @for (journal of io.repository(); track journal.name) {
          <li><button type="button" (click)="open(journal.name)" [title]="journal.description ?? ''">
            {{ journal.title }} <em>{{ journal.name }}</em>
          </button></li>
        } @empty {
          <li><span class="none">No journals to open</span></li>
        }
      </ul>
    </div>
    <button type="button" (click)="importZip()" title="Load a zipped journal, in place of this one">Import…</button>
    <button type="button" class="primary" (click)="exportZip()" [disabled]="io.exporting()"
            title="Download the journal as a zip, with the library files it uses">{{ io.exporting() ? 'Exporting…' : 'Export' }}</button>
  `,
  styles: `
    :host { display: flex; align-items: center; gap: 8px; padding: 4px 10px; background: var(--rail-bg); color: var(--rail-fg); font-size: 12px; }
    .name { display: inline-flex; align-items: baseline; gap: 6px; min-width: 0; }
    .dir { background: transparent; border: 0; color: var(--rail-fg); font-family: var(--mono); padding: 2px 0; cursor: pointer; opacity: 0.75; }
    .dir:hover { opacity: 1; text-decoration: underline; }
    .title { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .dirty { color: var(--accent); }
    .spacer { flex: 1; }
    .message { opacity: 0.85; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 40vw; }
    .message.error { color: var(--danger); opacity: 1; }
    button { white-space: nowrap; }
    .menu { position: relative; }
    .list { display: none; position: absolute; right: 0; top: 100%; z-index: 30; margin: 0; padding: 4px; list-style: none; min-width: 240px; max-height: 70vh; overflow: auto;
            background: var(--surface); color: var(--fg); border: 1px solid var(--border); border-radius: 6px; box-shadow: 0 4px 14px rgb(0 0 0 / 0.25); }
    .list::before { content: ''; position: absolute; left: 0; right: 0; top: -6px; height: 6px; }
    .menu:hover .list, .menu:focus-within .list { display: block; }
    .list button, .list .none { display: block; width: 100%; text-align: left; white-space: nowrap; padding: 4px 10px; }
    .list button { background: transparent; border: 0; color: inherit; cursor: pointer; border-radius: 4px; }
    .list button:hover, .list button:focus-visible { background: var(--surface-2); }
    .list .none { color: var(--muted); }
    em { font-style: normal; color: var(--muted); font-family: var(--mono); margin-left: 6px; }
  `,
})
export class JournalBarComponent {
  protected readonly store = inject(JournalStore);
  protected readonly io = inject(JournalIoService);
  protected readonly message = signal<{ text: string; error: boolean } | null>(null);
  private messageTimer: ReturnType<typeof setTimeout> | null = null;

  /** Whether it's fine to replace the journal: it's saved, or the user says to throw its changes away. */
  private discard(doing: string): boolean {
    return !this.store.dirty()
      || confirm(`${this.store.manifest().title} has changes not yet exported. ${doing} anyway, and lose them? (Export first to keep them.)`);
  }

  private say(text: string, error = false) {
    this.message.set({ text, error });
    if (this.messageTimer) clearTimeout(this.messageTimer);
    this.messageTimer = setTimeout(() => this.message.set(null), error ? 12_000 : 6_000);
  }

  protected newJournal() {
    if (this.discard('Start a new journal')) this.store.newJournal();
  }

  protected async open(name: string) {
    (document.activeElement as HTMLElement | null)?.blur();
    if (!this.discard(`Open ${name}`)) return;
    try {
      await this.io.openRepository(name);
    } catch (err) {
      this.say((err as Error).message, true);
    }
  }

  protected async importZip() {
    if (!this.discard('Import another journal')) return;
    try {
      if (await this.io.importZip()) this.say(`Imported ${this.store.name()}.`);
    } catch (err) {
      this.say(`Couldn't import it: ${(err as Error).message}`, true);
    }
  }

  protected async exportZip() {
    try {
      const bundled = await this.io.exportZip();
      this.say(bundled.length
        ? `Exported ${this.store.name()}.zip, with ${bundled.length} library file${bundled.length === 1 ? '' : 's'} it uses.`
        : `Exported ${this.store.name()}.zip.`);
    } catch (err) {
      this.say(`Couldn't export it: ${(err as Error).message}`, true);
    }
  }

  protected rename() {
    const name = prompt("The journal's directory name (letters, digits and _):", this.store.name())?.trim();
    if (!name || name === this.store.name()) return;
    const problem = this.store.setName(name);
    if (problem) this.say(problem, true);
  }
}
