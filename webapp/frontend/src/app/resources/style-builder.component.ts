import { Component, ElementRef, computed, inject, output, signal, viewChild } from '@angular/core';
import { VALID_NAME, pathOf, splitPath } from '../journal/journal-files';
import { DEFAULT_STYLE, copyName, readStyle, styleFile, type StyleSettings } from '../journal/style-builder';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { StyleDesignerComponent } from './style-designer.component';

/** What the preview is drawn as when the name isn't one yet. */
const PLACEHOLDER_NAME = 'new_style';

/** A style there is to start from. */
interface Source {
  name: string;
  settings: StyleSettings;
}

/** The styles in `files` that can be read. */
function sourcesIn(files: Record<string, string>): Source[] {
  const sources: Source[] = [];
  for (const path of Object.keys(files).sort()) {
    const split = splitPath(path);
    if (split?.kind !== 'styles') continue;
    try {
      sources.push({ name: split.name, settings: readStyle(files[path]) });
    } catch {
      // Not one to start from, then.
    }
  }
  return sources;
}

/**
 * New style: a dialog of the ink, hand, type and paragraph a text style
 * is made of, previewed as they're set, that writes the style file. It
 * starts from the defaults or as a copy of another style.
 */
@Component({
  selector: 'app-style-builder',
  imports: [StyleDesignerComponent],
  template: `
    <dialog #dialog (close)="isOpen.set(false)" aria-label="New style">
      <form method="dialog" (submit)="$event.preventDefault(); create()">
        <header>
          <h2>New style</h2>
          <label>Start from
            <select (change)="startFrom($any($event.target).value)">
              <option value="" [selected]="!from()">The defaults</option>
              @if (own().length) {
                <optgroup label="The journal's">
                  @for (source of own(); track source.name) { <option [value]="source.name" [selected]="source.name === from()">{{ source.name }}</option> }
                </optgroup>
              }
              @if (theirs().length) {
                <optgroup label="The library's">
                  @for (source of theirs(); track source.name) { <option [value]="source.name" [selected]="source.name === from()">{{ source.name }}</option> }
                </optgroup>
              }
            </select>
          </label>
          <label class="name">Name <input #nameInput type="text" spellcheck="false" placeholder="letters, digits and _" [value]="name()" (input)="name.set($any($event.target).value)" /></label>
          <label class="about">About <input type="text" placeholder="how a kind of text looks." [value]="settings().description" (input)="setDescription($any($event.target).value)" /></label>
        </header>
        <app-style-designer [name]="previewName()" [(settings)]="settings" [showing]="isOpen()" />
        <footer>
          <button type="button" (click)="reset()" [title]="'Back to ' + (from() || 'the defaults')">Reset</button>
          @if (problem(); as p) { <p class="error">{{ p }}</p> } @else { <span class="spacer"></span> }
          <button type="button" (click)="dialog.close()">Cancel</button>
          <button type="submit" class="primary" [disabled]="!!problem()">Create</button>
        </footer>
      </form>
    </dialog>
  `,
  styles: `
    dialog { border: 1px solid var(--border); border-radius: 10px; background: var(--surface); color: var(--fg); padding: 0;
             width: min(1100px, calc(100vw - 32px)); height: min(780px, calc(100vh - 32px)); font-size: 12px; }
    dialog[open] { display: flex; }
    dialog::backdrop { background: rgb(0 0 0 / 0.45); }
    form { display: flex; flex-direction: column; flex: 1; min-height: 0; min-width: 0; }
    header { display: flex; flex-wrap: wrap; align-items: end; gap: 12px; padding: 14px 16px 10px; border-bottom: 1px solid var(--border); }
    h2 { margin: 0 8px 4px 0; font-size: 16px; }
    header label { display: flex; flex-direction: column; gap: 3px; font-size: 12px; color: var(--muted); }
    .name input { font-family: var(--mono); width: 180px; }
    .about { flex: 1; min-width: 200px; }
    .error { margin: 0; color: var(--danger); white-space: pre-wrap; }
    footer { display: flex; align-items: center; gap: 8px; padding: 10px 16px; border-top: 1px solid var(--border); }
    footer .error, .spacer { flex: 1; }
    .primary { background: var(--accent); color: var(--accent-fg); border-color: var(--accent); }
    .primary:disabled { opacity: 0.5; }
  `,
})
export class StyleBuilderComponent {
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);
  private readonly dialogRef = viewChild.required<ElementRef<HTMLDialogElement>>('dialog');
  private readonly nameInput = viewChild.required<ElementRef<HTMLInputElement>>('nameInput');

  /** The style file to make, called name: styles/NAME.json. */
  readonly created = output<{ name: string; text: string }>();

  protected readonly name = signal('');
  protected readonly settings = signal<StyleSettings>({ ...DEFAULT_STYLE });
  protected readonly isOpen = signal(false);
  /** The style it starts from; empty for the defaults. */
  protected readonly from = signal('');
  /** The name last given to a copy: starting from another style replaces it, as it does no name. */
  private suggested = '';

  /** The journal's styles to start from. */
  protected readonly own = computed(() => sourcesIn(this.store.files()));
  /** The library's, but for those the journal's stand in for. */
  protected readonly theirs = computed(() => {
    const files = this.store.files();
    return sourcesIn(Object.fromEntries(Object.entries(this.library.files()).filter(([path]) => !(path in files))));
  });

  protected readonly previewName = computed(() => {
    const name = this.name().trim();
    return VALID_NAME.test(name) ? name : PLACEHOLDER_NAME;
  });

  protected readonly problem = computed(() => {
    const name = this.name().trim();
    if (!name) return 'Give it a name.';
    if (!VALID_NAME.test(name)) return 'A name is letters, digits and _ only.';
    if (pathOf('styles', name) in this.store.files()) return `There's a styles/${name}.json already.`;
    return null;
  });

  /** Show the builder, its name blank and its settings as last left. */
  open() {
    if (!this.source(this.from())) this.from.set('');
    this.show('');
  }

  /** Show the builder starting as a copy of the style `name`, or say why it can't. */
  copy(name: string): string | null {
    const source = this.source(name);
    if (!source) return `${pathOf('styles', name)} can't be read, so it can't be copied.`;
    this.from.set(name);
    this.settings.set(source.settings);
    this.show(this.nameForCopy(name));
    return null;
  }

  private show(name: string) {
    this.name.set(name);
    this.suggested = name;
    this.isOpen.set(true);
    this.dialogRef().nativeElement.showModal();
    const input = this.nameInput().nativeElement;
    input.value = name;
    input.focus();
    input.select();
  }

  private source(name: string): Source | undefined {
    return [...this.own(), ...this.theirs()].find((source) => source.name === name);
  }

  private nameForCopy(name: string): string {
    return copyName(name, (copy) => pathOf('styles', copy) in this.store.files() || this.library.has(pathOf('styles', copy)));
  }

  protected get dialog(): HTMLDialogElement {
    return this.dialogRef().nativeElement;
  }

  protected setDescription(description: string) {
    this.settings.update((s) => ({ ...s, description }));
  }

  /** Start again from the style `name`, or the defaults: what's been set goes, and a name given is kept. */
  protected startFrom(name: string) {
    this.from.set(name);
    this.reset();
    const typed = this.name().trim();
    if (typed && typed !== this.suggested) return;
    this.suggested = name ? this.nameForCopy(name) : '';
    this.name.set(this.suggested);
  }

  protected reset() {
    this.settings.set(this.source(this.from())?.settings ?? { ...DEFAULT_STYLE });
  }

  protected create() {
    if (this.problem()) return;
    this.created.emit({ name: this.name().trim(), text: styleFile(this.settings()) });
    this.dialog.close();
  }
}
