import { Component, input, signal, type OnInit } from '@angular/core';

type Splitter = 'left' | 'right';

function stored(key: string): { left?: number; right?: number } {
  try {
    return JSON.parse(localStorage.getItem(`journal-editor.panes.${key}`) ?? '{}') ?? {};
  } catch {
    return {};
  }
}

/**
 * Three columns with splitters between them: what's projected as `[left]`,
 * `[center]` and `[right]`. Each editor has its own widths (`key`), kept
 * between visits.
 */
@Component({
  selector: 'app-three-pane',
  host: {
    '[style.--left-width.px]': 'leftWidth()',
    '[style.--right-width.px]': 'rightWidth()',
    '(window:pointermove)': 'drag($event)',
    '(window:pointerup)': 'endDrag()',
  },
  template: `
    <ng-content select="[left]" />
    <div class="split" role="separator" aria-label="Resize the left pane" (pointerdown)="startDrag($event, 'left')"></div>
    <ng-content select="[center]" />
    <div class="split" role="separator" aria-label="Resize the right pane" (pointerdown)="startDrag($event, 'right')"></div>
    <ng-content select="[right]" />
  `,
  styles: `
    :host {
      display: grid; height: 100%; min-height: 0;
      grid-template-columns: var(--left-width) 4px minmax(240px, 1fr) 4px var(--right-width);
    }
    :host > ::ng-deep * { min-height: 0; min-width: 0; }
    .split { cursor: col-resize; background: var(--border); }
    .split:hover { background: var(--accent); }
    @media (max-width: 760px) {
      :host { grid-template-columns: 1fr; grid-template-rows: 120px 45vh auto; height: auto; min-height: 100%; }
      .split { display: none; }
    }
  `,
})
export class ThreePaneComponent implements OnInit {
  /** Names the widths kept for this editor. */
  readonly key = input.required<string>();
  readonly leftDefault = input(190);
  /** The right pane's width, as a fraction of the window, and at least `rightMin`. */
  readonly rightFraction = input(0.4);
  readonly rightMin = input(420);

  protected readonly leftWidth = signal(190);
  protected readonly rightWidth = signal(420);
  private dragging: { which: Splitter; x: number; width: number } | null = null;
  private initialised = false;

  ngOnInit() {
    const saved = stored(this.key());
    this.leftWidth.set(saved.left ?? this.leftDefault());
    this.rightWidth.set(saved.right ?? Math.round(Math.max(this.rightMin(), window.innerWidth * this.rightFraction())));
    this.initialised = true;
  }

  protected startDrag(event: PointerEvent, which: Splitter) {
    event.preventDefault();
    this.dragging = { which, x: event.clientX, width: which === 'left' ? this.leftWidth() : this.rightWidth() };
  }

  protected drag(event: PointerEvent) {
    if (!this.dragging) return;
    const { which, x, width } = this.dragging;
    const dx = event.clientX - x;
    if (which === 'left') this.leftWidth.set(Math.min(400, Math.max(110, width + dx)));
    else this.rightWidth.set(Math.min(window.innerWidth - 400, Math.max(300, width - dx)));
  }

  protected endDrag() {
    if (!this.dragging || !this.initialised) return;
    this.dragging = null;
    try {
      localStorage.setItem(`journal-editor.panes.${this.key()}`, JSON.stringify({ left: this.leftWidth(), right: this.rightWidth() }));
    } catch {
      // No storage: the widths just aren't kept.
    }
  }
}
