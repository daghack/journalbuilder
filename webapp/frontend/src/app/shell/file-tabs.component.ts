import { Component, ElementRef, computed, effect, inject, viewChildren } from '@angular/core';
import { splitPath } from '../journal/journal-files';
import { JournalStore } from '../services/journal-store.service';
import { ShellService } from './shell.service';

/** Above an editor's middle: a tab for each file opened lately, to go back to. */
@Component({
  selector: 'app-file-tabs',
  host: { role: 'tablist', 'aria-label': 'Files opened lately', '(wheel)': 'onWheel($event)' },
  template: `
    @for (tab of tabs(); track tab.path) {
      <div #tab class="tab" [class.on]="tab.path === shell.selected()" [class.library]="tab.library"
           (mousedown)="$event.button === 1 && $event.preventDefault()" (auxclick)="$event.button === 1 && shell.closeTab(tab.path)">
        <button type="button" role="tab" class="open" [attr.aria-selected]="tab.path === shell.selected()" (click)="shell.open(tab.path)"
                [title]="tab.library ? 'The library\\'s ' + tab.path : tab.path">{{ tab.label }}</button>
        @if (tabs().length > 1) {
          <button type="button" class="close" (click)="shell.closeTab(tab.path)" [attr.aria-label]="'Close the tab of ' + tab.path"
                  title="Close the tab (the file stays)">✕</button>
        }
      </div>
    }
  `,
  styles: `
    :host { display: flex; flex: none; gap: 2px; min-height: 28px; padding: 4px 6px 0; background: var(--rail-bg); border-bottom: 1px solid var(--border);
            overflow-x: auto; scrollbar-width: none; }
    .tab { display: flex; align-items: center; flex: none; max-width: 220px; border: 1px solid transparent; border-bottom: none; border-radius: 6px 6px 0 0; }
    .tab:hover { background: var(--rail-hover); }
    .tab.on { background: var(--rail-selected-bg); border-color: var(--rail-selected); }
    .open { flex: 1; min-width: 0; background: transparent; border: 0; border-radius: inherit; color: var(--rail-fg); padding: 4px 2px 4px 10px; font-family: var(--mono);
            text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .tab.on .open { font-weight: 600; }
    .tab.library .open { opacity: 0.7; }
    .close { flex: none; background: transparent; border: 0; color: var(--rail-fg); padding: 0 6px; font-size: 10px; line-height: 20px; opacity: 0; }
    .tab:hover .close, .tab.on .close, .close:focus-visible { opacity: 0.7; }
    .close:hover { opacity: 1 !important; color: var(--accent); }
  `,
})
export class FileTabsComponent {
  protected readonly shell = inject(ShellService);
  private readonly store = inject(JournalStore);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  private readonly items = viewChildren<ElementRef<HTMLElement>>('tab');

  /** The tabs as they show: a file's name, its path where two share a name. */
  protected readonly tabs = computed(() => {
    const files = this.store.files();
    const tabs = this.shell.tabs().map(({ path }) => ({ path, name: splitPath(path)?.name ?? path, library: !(path in files) }));
    return tabs.map((tab) => ({ ...tab, label: tabs.some((t) => t !== tab && t.name === tab.name) ? tab.path.replace(/\.[^.]*$/, '') : tab.name }));
  });

  constructor() {
    // Keep the file showing in view, when the row is too narrow for them all.
    effect(() => {
      const index = this.tabs().findIndex((tab) => tab.path === this.shell.selected());
      this.items()[index]?.nativeElement.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    });
  }

  /** The wheel scrolls the row sideways. */
  protected onWheel(event: WheelEvent) {
    if (event.deltaX || !event.deltaY) return;
    this.host.nativeElement.scrollLeft += event.deltaY;
  }
}
