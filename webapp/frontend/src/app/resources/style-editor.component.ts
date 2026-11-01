import { Component, computed, inject } from '@angular/core';
import { splitPath } from '../journal/journal-files';
import { DEFAULT_STYLE, readStyle, styleFile, type StyleSettings } from '../journal/style-builder';
import { ThreePaneComponent } from '../layout/three-pane.component';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { ShellService } from '../shell/shell.service';
import { LibraryBannerComponent } from './library-banner.component';
import { StyleDesignerComponent } from './style-designer.component';

/**
 * A text style file: the settings New style shows, changing the file as
 * they're set (the journal's, or the library's until it's changed, when the
 * journal gets a copy that stands in for it). Nobody edits the JSON it's
 * kept as.
 */
@Component({
  selector: 'app-style-editor',
  imports: [ThreePaneComponent, LibraryBannerComponent, StyleDesignerComponent],
  template: `
    <app-three-pane key="style" [right]="false">
      <section center class="centre" aria-label="Style">
        <app-library-banner [path]="path()" [own]="own()" [builtin]="builtin()" (copy)="copyIn()" (revert)="revert()" />
        @if (read(); as read) {
          @if (read.settings; as settings) {
            <header>
              <label class="about">About <input type="text" placeholder="how a kind of text looks." [value]="settings.description" (input)="write({ ...settings, description: $any($event.target).value })" /></label>
              <code>text_style("{{ name() }}", #{{ '{' }} size, color {{ '}' }})</code>
            </header>
            <app-style-designer [name]="name()" [settings]="settings" [showing]="showing()" (settingsChange)="write($event)" />
          } @else {
            <div class="unreadable">
              <p class="error">{{ path() }} can't be read. {{ read.error }}</p>
              <button type="button" (click)="write(defaults)" title="Replace the file with New style's defaults">Start again from the defaults</button>
            </div>
          }
        }
      </section>
    </app-three-pane>
  `,
  styles: `
    :host { display: flex; flex-direction: column; min-height: 0; height: 100%; }
    .centre { display: flex; flex-direction: column; flex: 1; min-height: 0; min-width: 0; }
    header { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 16px; padding: 8px 12px; border-bottom: 1px solid var(--border); background: var(--surface); font-size: 12px; }
    .about { display: flex; align-items: center; gap: 8px; flex: 1; min-width: 200px; color: var(--muted); }
    .about input { flex: 1; min-width: 0; }
    code { font-family: var(--mono); color: var(--muted); }
    .unreadable { display: flex; flex-direction: column; align-items: start; gap: 10px; padding: 14px; font-size: 12px; }
    .error { margin: 0; color: var(--danger); white-space: pre-wrap; }
  `,
})
export class StyleEditorComponent {
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);
  private readonly shell = inject(ShellService);

  protected readonly defaults = DEFAULT_STYLE;
  /** The file edited, which stays while another kind's shows. */
  protected readonly path = this.shell.lastOpened(['styles']);
  protected readonly name = computed(() => splitPath(this.path() ?? '')?.name ?? '');
  protected readonly own = computed(() => (this.path() ?? '') in this.store.files());
  protected readonly builtin = computed(() => this.library.has(this.path() ?? ''));
  protected readonly showing = computed(() => this.shell.selected() === this.path());
  private readonly text = computed(() => {
    const path = this.path();
    return path ? (this.store.files()[path] ?? this.library.files()[path] ?? '') : '';
  });

  /** The file's settings, or why it can't be read; null until there's a file to read. */
  protected readonly read = computed<{ settings?: StyleSettings; error?: string } | null>(() => {
    const text = this.text();
    if (!text) return null;
    try {
      return { settings: readStyle(text) };
    } catch (err) {
      return { error: (err as Error).message };
    }
  });

  /** The settings changed: the journal's copy is changed, made if it hasn't one. */
  protected write(settings: StyleSettings) {
    const path = this.path();
    if (path) this.store.setFile(path, styleFile(settings));
  }

  /** Copy the library's file into the journal, to change. */
  protected copyIn() {
    const path = this.path();
    if (path && !this.own()) this.store.setFile(path, this.text());
  }

  /** Remove the journal's copy: the library's is used again. */
  protected revert() {
    const path = this.path();
    if (path && this.own() && this.builtin()) this.store.removeFile(path);
  }
}
