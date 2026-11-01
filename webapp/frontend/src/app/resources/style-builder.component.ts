import { Component, ElementRef, computed, inject, output, signal, viewChild } from '@angular/core';
import { VALID_NAME, pathOf } from '../journal/journal-files';
import { DEFAULT_STYLE, styleFile, type StyleSettings } from '../journal/style-builder';
import { JournalStore } from '../services/journal-store.service';
import { StyleDesignerComponent } from './style-designer.component';

/** What the preview is drawn as when the name isn't one yet. */
const PLACEHOLDER_NAME = 'new_style';

/**
 * New style: a dialog of the ink, hand, type and paragraph a text style
 * is made of, previewed as they're set, that writes the style file.
 */
@Component({
  selector: 'app-style-builder',
  imports: [StyleDesignerComponent],
  template: `
    <dialog #dialog (close)="isOpen.set(false)" aria-label="New style">
      <form method="dialog" (submit)="$event.preventDefault(); create()">
        <header>
          <h2>New style</h2>
          <label class="name">Name <input #nameInput type="text" spellcheck="false" placeholder="letters, digits and _" [value]="name()" (input)="name.set($any($event.target).value)" /></label>
          <label class="about">About <input type="text" placeholder="how a kind of text looks." [value]="settings().description" (input)="setDescription($any($event.target).value)" /></label>
        </header>
        <app-style-designer [name]="previewName()" [(settings)]="settings" [showing]="isOpen()" />
        <footer>
          <button type="button" (click)="reset()" title="Back to the defaults">Reset</button>
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
  private readonly dialogRef = viewChild.required<ElementRef<HTMLDialogElement>>('dialog');
  private readonly nameInput = viewChild.required<ElementRef<HTMLInputElement>>('nameInput');

  /** The style file to make, called name: styles/NAME.json. */
  readonly created = output<{ name: string; text: string }>();

  protected readonly name = signal('');
  protected readonly settings = signal<StyleSettings>({ ...DEFAULT_STYLE });
  protected readonly isOpen = signal(false);

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
    this.name.set('');
    this.isOpen.set(true);
    this.dialogRef().nativeElement.showModal();
    const name = this.nameInput().nativeElement;
    name.value = '';
    name.focus();
  }

  protected get dialog(): HTMLDialogElement {
    return this.dialogRef().nativeElement;
  }

  protected setDescription(description: string) {
    this.settings.update((s) => ({ ...s, description }));
  }

  protected reset() {
    this.settings.set({ ...DEFAULT_STYLE });
  }

  protected create() {
    if (this.problem()) return;
    this.created.emit({ name: this.name().trim(), text: styleFile(this.settings()) });
    this.dialog.close();
  }
}
