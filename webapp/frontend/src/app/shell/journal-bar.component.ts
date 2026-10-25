import { Component, ElementRef, effect, inject, signal, viewChild } from '@angular/core';
import { JournalIoService } from '../services/journal-io.service';
import { JournalStore } from '../services/journal-store.service';

/** Across the top: the journal's title (click it to change it), and new, open, import (a zip or a directory) and export. */
@Component({
  selector: 'app-journal-bar',
  template: `
    <span class="name">
      @if (editing()) {
        <input #titleInput class="title-edit" [value]="store.manifest().title" aria-label="The journal's title"
               (keydown.enter)="commitTitle(titleInput.value)" (keydown.escape)="editing.set(false)" (blur)="commitTitle(titleInput.value)" />
      } @else {
        <button type="button" class="title" (click)="editTitle()" title="Retitle the journal: its directory, and an export's name, follow in snake_case">{{ store.manifest().title }}</button>
      }
      @if (store.dirty()) { <span class="dirty" title="Changed since it was last exported">●</span> }
    </span>
    <span class="spacer"></span>
    @if (message(); as m) { <span class="message" [class.error]="m.error">{{ m.text }}</span> }
    <button type="button" (click)="newJournal()" title="Start a new journal: one blank entry, and the library">New</button>
    <div class="menu">
      <button type="button" aria-haspopup="true" title="Open one of the journals built in">Open ▾</button>
      <ul class="list" aria-label="The journals built in">
        @for (journal of io.repository(); track journal.name) {
          <li><button type="button" (click)="open(journal.name)" [title]="journal.description ?? ''">
            {{ journal.title }} <em>{{ journal.name }}</em>
          </button></li>
        } @empty {
          <li><span class="none">No journals to open</span></li>
        }
      </ul>
    </div>
    <div class="menu">
      <button type="button" aria-haspopup="true" title="Load a journal, in place of this one">Import ▾</button>
      <ul class="list" aria-label="Import a journal">
        <li><button type="button" (click)="importZip()" title="Load a zipped journal, as Export makes">Zipped journal…</button></li>
        <li><button type="button" (click)="importDirectory()" title="Load a journal's directory, unzipped: one of the repository's journals/, say">Journal folder…</button></li>
      </ul>
    </div>
    <button type="button" class="primary" (click)="exportZip()" [disabled]="io.exporting()"
            title="Download the journal as a zip, with the library files it uses">{{ io.exporting() ? 'Exporting…' : 'Export' }}</button>
  `,
  styles: `
    :host { display: flex; align-items: center; gap: 8px; padding: 4px 10px; background: var(--rail-bg); color: var(--rail-fg); font-size: 12px; }
    .name { display: inline-flex; align-items: baseline; gap: 6px; min-width: 0; }
    .title { background: transparent; border: 0; color: var(--rail-fg); font: inherit; font-weight: 600; padding: 2px 0; cursor: text;
             white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .title:hover { text-decoration: underline dotted; }
    .title-edit { font: inherit; font-weight: 600; padding: 1px 4px; min-width: 16ch; }
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
  /** The title being edited, in place. */
  protected readonly editing = signal(false);
  private readonly titleInput = viewChild<ElementRef<HTMLInputElement>>('titleInput');

  constructor() {
    effect(() => {
      const input = this.titleInput()?.nativeElement;
      if (input) {
        input.focus();
        input.select();
      }
    });
  }

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

  protected importZip() {
    return this.import(() => this.io.importZip());
  }

  protected importDirectory() {
    return this.import(() => this.io.importDirectory());
  }

  private async import(load: () => Promise<boolean>) {
    (document.activeElement as HTMLElement | null)?.blur();
    if (!this.discard('Import another journal')) return;
    try {
      if (await load()) this.say(`Imported ${this.store.name()}.`);
    } catch (err) {
      this.say(`Couldn't import it: ${(err as Error).message}`, true);
    }
  }

  protected async exportZip() {
    try {
      const exported = await this.io.exportZip();
      if (!exported) return;
      const { saved, bundled } = exported;
      this.say(bundled.length
        ? `Exported ${saved}, with ${bundled.length} library file${bundled.length === 1 ? '' : 's'} it uses.`
        : `Exported ${saved}.`);
    } catch (err) {
      this.say(`Couldn't export it: ${(err as Error).message}`, true);
    }
  }

  protected editTitle() {
    this.editing.set(true);
  }

  protected commitTitle(value: string) {
    if (!this.editing()) return;
    this.editing.set(false);
    const title = value.trim();
    if (title && title !== this.store.manifest().title) this.store.setTitle(title);
  }
}
