import { Component, Directive, computed, inject, input, signal, type OnInit } from '@angular/core';
import { FileSidebarComponent } from '../shell/file-sidebar.component';

type Splitter = 'left' | 'right';

/** One of the left pane's tabs, beside the journal's files. */
export interface PaneTab {
  id: string;
  label: string;
  title: string;
}

const FILES = 'files';

function stored<T>(key: string, fallback: T): T {
  try {
    return JSON.parse(localStorage.getItem(`journal-editor.panes.${key}`) ?? 'null') ?? fallback;
  } catch {
    return fallback;
  }
}

function keep(key: string, value: unknown) {
  try {
    localStorage.setItem(`journal-editor.panes.${key}`, JSON.stringify(value));
  } catch {
    // No storage: it just isn't kept.
  }
}

/** The left pane's width, the same in every editor so it holds still as files open. */
const leftWidth = signal<number>(stored('left', 240));
/** Whether the left pane shows the journal's files, in every editor: picking a file keeps them showing. */
const filesShown = signal<boolean>(stored('files', true));

/**
 * Three columns with splitters between them. The left is tabbed: the
 * journal's files first, then what's projected with `[appPaneTab]="id"`
 * for each of `tabs`. Then `[center]`, and `[right]` unless `right` is
 * false. Each editor keeps its right width and its tab (`key`).
 */
@Component({
  selector: 'app-three-pane',
  imports: [FileSidebarComponent],
  host: {
    '[class.no-right]': '!right()',
    '[style.--left-width.px]': 'leftWidth()',
    '[style.--right-width.px]': 'rightWidth()',
    '(window:pointermove)': 'drag($event)',
    '(window:pointerup)': 'endDrag()',
  },
  template: `
    <div class="left">
      <div class="tabs" role="tablist">
        <button type="button" role="tab" [class.on]="tab() === files" [attr.aria-selected]="tab() === files"
                (click)="pick(files)" title="The journal's files, and the library's: pick one to edit">Files</button>
        @for (t of tabs(); track t.id) {
          <button type="button" role="tab" [class.on]="t.id === tab()" [attr.aria-selected]="t.id === tab()"
                  (click)="pick(t.id)" [title]="t.title">{{ t.label }}</button>
        }
      </div>
      <app-file-sidebar role="tabpanel" [hidden]="tab() !== files" />
      <ng-content select="[appPaneTab]" />
    </div>
    <div class="split" role="separator" aria-label="Resize the left pane" (pointerdown)="startDrag($event, 'left')"></div>
    <ng-content select="[center]" />
    @if (right()) {
      <div class="split" role="separator" aria-label="Resize the right pane" (pointerdown)="startDrag($event, 'right')"></div>
      <ng-content select="[right]" />
    }
  `,
  styles: `
    :host {
      display: grid; height: 100%; min-height: 0;
      grid-template-columns: var(--left-width) 4px minmax(240px, 1fr) 4px var(--right-width);
    }
    :host(.no-right) { grid-template-columns: var(--left-width) 4px minmax(240px, 1fr); }
    :host > ::ng-deep * { min-height: 0; min-width: 0; }
    .split { cursor: col-resize; background: var(--border); }
    .split:hover { background: var(--accent); }
    .left { display: flex; flex-direction: column; min-height: 0; min-width: 0; background: var(--surface); }
    .left > ::ng-deep :not(.tabs) { flex: 1; min-height: 0; }
    .left > ::ng-deep [hidden] { display: none !important; }
    .left > app-file-sidebar { border-right: none; }
    .tabs { display: flex; flex: none; gap: 2px; padding: 4px 6px 0; background: var(--rail-bg); border-bottom: 1px solid var(--border); overflow-x: auto; }
    .tabs button { background: transparent; border: 1px solid transparent; border-bottom: none; border-radius: 6px 6px 0 0;
                   color: var(--rail-fg); padding: 4px 12px; font-size: 12px; white-space: nowrap; }
    .tabs button:hover { background: var(--rail-hover); }
    .tabs button.on { background: var(--rail-selected-bg); border-color: var(--rail-selected); font-weight: 600; }
    @media (max-width: 760px) {
      :host, :host(.no-right) { grid-template-columns: 1fr; grid-template-rows: minmax(0, 40vh) 45vh auto; height: auto; min-height: 100%; }
      .split { display: none; }
    }
  `,
})
export class ThreePaneComponent implements OnInit {
  /** Names the right width and tab kept for this editor. */
  readonly key = input.required<string>();
  /** The left pane's tabs after the journal's files, in order. */
  readonly tabs = input<PaneTab[]>([]);
  /** Whether there's a right pane. */
  readonly right = input(true);
  /** The right pane's width, as a fraction of the window, and at least `rightMin`. */
  readonly rightFraction = input(0.4);
  readonly rightMin = input(420);

  protected readonly files = FILES;
  protected readonly leftWidth = leftWidth;
  protected readonly rightWidth = signal(420);
  private readonly picked = signal<string | null>(null);
  /** The tab showing: the files, if they're showing, else the one picked, else the first. */
  readonly tab = computed(() => {
    const tabs = this.tabs();
    if (filesShown() || !tabs.length) return FILES;
    const picked = this.picked();
    return tabs.some((t) => t.id === picked) ? picked! : tabs[0].id;
  });
  private dragging: { which: Splitter; x: number; width: number } | null = null;
  private initialised = false;

  ngOnInit() {
    const saved = stored<{ right?: number; tab?: string }>(this.key(), {});
    this.rightWidth.set(saved.right ?? Math.round(Math.max(this.rightMin(), window.innerWidth * this.rightFraction())));
    this.picked.set(saved.tab ?? null);
    this.initialised = true;
  }

  /** Show the tab `id`: the files, or one of `tabs`. */
  pick(id: string) {
    filesShown.set(id === FILES);
    keep('files', id === FILES);
    if (id !== FILES) {
      this.picked.set(id);
      this.keepOwn();
    }
  }

  protected startDrag(event: PointerEvent, which: Splitter) {
    event.preventDefault();
    this.dragging = { which, x: event.clientX, width: which === 'left' ? leftWidth() : this.rightWidth() };
  }

  protected drag(event: PointerEvent) {
    if (!this.dragging) return;
    const { which, x, width } = this.dragging;
    const dx = event.clientX - x;
    if (which === 'left') leftWidth.set(Math.min(520, Math.max(160, width + dx)));
    else this.rightWidth.set(Math.min(window.innerWidth - 400, Math.max(300, width - dx)));
  }

  protected endDrag() {
    if (!this.dragging || !this.initialised) return;
    const { which } = this.dragging;
    this.dragging = null;
    if (which === 'left') keep('left', leftWidth());
    else this.keepOwn();
  }

  private keepOwn() {
    if (this.initialised) keep(this.key(), { right: this.rightWidth(), tab: this.picked() ?? undefined });
  }
}

/** Marks what shows under the left pane's tab `appPaneTab`. */
@Directive({
  selector: '[appPaneTab]',
  host: { '[hidden]': 'pane.tab() !== appPaneTab()', role: 'tabpanel' },
})
export class PaneTabDirective {
  readonly appPaneTab = input.required<string>();
  protected readonly pane = inject(ThreePaneComponent);
}
