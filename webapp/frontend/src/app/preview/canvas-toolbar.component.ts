import { Component, computed, inject } from '@angular/core';
import type { Alignment, Bounds, CanvasEdit } from '../journal/canvas-edits';
import { editsPath } from '../journal/journal-files';
import { JournalService } from '../services/journal.service';
import { VisualEditService } from '../services/visual-edit.service';

/**
 * The visual editor's side panel: the canvases the entry commits to it
 * (shown or hidden, locked or not), the tools, and the selected
 * canvases' placing, arranging and ink, with undo and redo.
 */
@Component({
  selector: 'app-canvas-toolbar',
  template: `
    <section class="panel-section">
      <h3>Canvases</h3>
      @if (!edit.readable()) {
        <p class="note-warning">{{ editsFile() }} isn't an edits file this editor reads: fix it or delete it to edit here.</p>
      }
      @for (c of edit.committed(); track c.name) {
        <div class="canvas" [class.on]="selected().has(c.name)">
          <button type="button" class="name" (click)="edit.select(c.name, $event.shiftKey)" [title]="'Select ' + c.name + ' (Shift adds)'">{{ c.name }}</button>
          @if (c.line !== null) { <span class="hint" title="The script line that commits it">L{{ c.line }}</span> }
          @if (!c.shown.length) {
            <span class="hint" title="It isn't drawn on any page (passed through a layout, say): edit it by number">not on a page</span>
          } @else if (!onPage(c.shown)) {
            <button type="button" class="link" (click)="journal.select(c.shown[0].page)" title="Show the page it's on">page {{ c.shown[0].page + 1 }}</button>
          }
          <span class="spacer"></span>
          <button type="button" class="icon" [class.off]="edit.edit(c.name).hidden" [disabled]="!edit.readable()"
                  (click)="toggle(c.name, 'hidden')" [title]="edit.edit(c.name).hidden ? 'Show it' : 'Hide it'"
                  [attr.aria-label]="(edit.edit(c.name).hidden ? 'Show ' : 'Hide ') + c.name">{{ edit.edit(c.name).hidden ? '◌' : '●' }}</button>
          <button type="button" class="icon" [class.off]="!edit.edit(c.name).locked" [disabled]="!edit.readable()"
                  (click)="toggle(c.name, 'locked')" [title]="edit.edit(c.name).locked ? 'Unlock it' : 'Lock it: it can\\'t be picked on the page'"
                  [attr.aria-label]="(edit.edit(c.name).locked ? 'Unlock ' : 'Lock ') + c.name">{{ edit.edit(c.name).locked ? '🔒' : '🔓' }}</button>
        </div>
      } @empty {
        <p class="hint">The entry commits no canvases. Hand one to the editor with <code>canvas.commit_to_editor("name")</code>, then move, turn, scale and re-ink it here.</p>
      }
    </section>

    <section class="panel-section">
      <h3>Tools</h3>
      <div class="row">
        <button type="button" [class.on]="edit.tool() === 'select'" (click)="edit.tool.set('select')" title="Move, scale, stretch (Ctrl/⌘ skews) and turn (Shift snaps 15°) with the box's handles (V)">Select</button>
        <button type="button" [class.on]="edit.tool() === 'warp'" (click)="edit.tool.set('warp')" title="Pin the box's corners where you drag them (W)">Warp</button>
        <span class="spacer"></span>
        <button type="button" (click)="edit.undo()" [disabled]="!edit.canUndo()" title="Undo (Ctrl/⌘+Z)">Undo</button>
        <button type="button" (click)="edit.redo()" [disabled]="!edit.canRedo()" title="Redo (Ctrl/⌘+Shift+Z)">Redo</button>
      </div>
      <p class="hint">Drag to move (Alt: no snapping, Shift: one axis), arrows nudge (Shift ×10), Shift-click or drag a box round several.</p>
    </section>

    @if (one(); as name) {
      <section class="panel-section">
        <h3>Placing {{ name }}</h3>
        @let e = edit.edit(name);
        <div class="grid">
          <label>Move x <input type="number" step="1" [value]="fixed(e.move?.[0] ?? 0)" (change)="setMove(name, 0, $event)" /></label>
          <label>Move y <input type="number" step="1" [value]="fixed(e.move?.[1] ?? 0)" (change)="setMove(name, 1, $event)" /></label>
          <label>Turn ° <input type="number" step="1" [value]="fixed(e.turn ?? 0)" (change)="set(name, 'turn', $event)" /></label>
          <label>Skew ° <input type="number" step="1" min="-85" max="85" [value]="fixed(e.skew ?? 0)" (change)="set(name, 'skew', $event)" /></label>
          <label>Scale x % <input type="number" step="5" [value]="fixed((e.scale?.[0] ?? 1) * 100)" (change)="setScale(name, 0, $event)" /></label>
          <label>Scale y % <input type="number" step="5" [value]="fixed((e.scale?.[1] ?? 1) * 100)" (change)="setScale(name, 1, $event)" /></label>
        </div>
        <div class="row">
          <button type="button" (click)="edit.flip(sel(), 'x')" [disabled]="!onPageSelection()" title="Flip it left to right on the page">Flip ↔</button>
          <button type="button" (click)="edit.flip(sel(), 'y')" [disabled]="!onPageSelection()" title="Flip it top to bottom on the page">Flip ↕</button>
          <span class="spacer"></span>
          <button type="button" (click)="edit.reset(name)" title="Back to where the script put it, in its own ink (staying hidden or locked)">Reset</button>
        </div>
      </section>
    } @else if (sel().length > 1) {
      <section class="panel-section">
        <h3>{{ sel().length }} canvases</h3>
        <div class="row">
          <button type="button" (click)="edit.flip(sel(), 'x')" title="Flip each left to right">Flip ↔</button>
          <button type="button" (click)="edit.flip(sel(), 'y')" title="Flip each top to bottom">Flip ↕</button>
          <span class="spacer"></span>
          <button type="button" (click)="resetAll()" title="Each back to where the script put it, in its own ink">Reset</button>
        </div>
      </section>
    }

    @if (onPageSelection()) {
      <section class="panel-section">
        <h3>Arrange</h3>
        <p class="hint">{{ sel().length === 1 ? 'With the page' : 'With each other' }}</p>
        <div class="row">
          @for (a of alignments; track a.how) {
            <button type="button" class="icon" (click)="alignTo(a.how)" [title]="a.title">{{ a.label }}</button>
          }
        </div>
        @if (sel().length > 2) {
          <div class="row">
            <button type="button" (click)="edit.distribute(sel(), 'x')" title="Space them evenly across">Distribute ↔</button>
            <button type="button" (click)="edit.distribute(sel(), 'y')" title="Space them evenly down">Distribute ↕</button>
          </div>
        }
        @if (one(); as name) {
          <div class="row">
            <button type="button" (click)="centre(name, false)" title="Move it to the page's centre">Centre on page</button>
            <button type="button" (click)="centre(name, true)" title="Scale it to fill the page as far as it can, centred">Fit page</button>
          </div>
        }
      </section>
    }

    @if (sel().length) {
      <section class="panel-section">
        <h3>Ink</h3>
        @let ink = firstInk();
        <div class="field">
          <span>Colour</span>
          <span class="row">
            <input type="color" [value]="ink.color ?? '#1f1b16'" (input)="setInk('color', $any($event.target).value)" (change)="edit.settle()"
                   title="Draw it in one colour instead of its own" />
            @if (ink.color) {
              <button type="button" (click)="setInk('color', undefined); edit.settle()" title="Back to its own colours">Own</button>
            } @else {
              <span class="hint">its own</span>
            }
          </span>
          <span>Opacity</span>
          <span class="row">
            <input type="range" min="0" max="1" step="0.01" [value]="ink.opacity ?? 1" (input)="setInk('opacity', +$any($event.target).value)" (change)="edit.settle()" />
            <span class="hint num">{{ fixed((ink.opacity ?? 1) * 100) }}%</span>
          </span>
          <span>Weight</span>
          <span class="row">
            <input type="range" min="-2" max="2" step="0.05" [value]="log2(ink.weight ?? 1)" (input)="setInk('weight', 2 ** +$any($event.target).value)" (change)="edit.settle()"
                   title="Its lines' width, times this" />
            <span class="hint num">×{{ fixed(ink.weight ?? 1) }}</span>
          </span>
        </div>
      </section>
    }
  `,
  styles: `
    :host { display: block; overflow-y: auto; background: var(--surface); border-right: 1px solid var(--border); font-size: 12px; }
    .canvas { display: flex; align-items: center; gap: 4px; padding: 1px 4px; border-radius: 4px; }
    .canvas.on { background: var(--editor-selection); }
    .name { background: transparent; border: 0; padding: 2px 0; color: inherit; font-family: var(--mono); font-size: 12px; text-align: left;
            overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; cursor: pointer; }
    .canvas.on .name { font-weight: 600; }
    .link { background: transparent; border: 0; padding: 0; color: var(--accent); text-decoration: underline; font-size: 11px; cursor: pointer; }
    .spacer { flex: 1; }
    .icon { min-width: 26px; padding: 2px 4px; }
    .icon.off { opacity: 0.5; }
    button.on { background: var(--accent); border-color: var(--accent); color: var(--accent-fg); }
    .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
    .grid label { display: flex; flex-direction: column; gap: 2px; color: var(--muted); }
    .grid input { width: 100%; }
    .field { grid-template-columns: 4.5em 1fr; }
    .field input[type='range'] { flex: 1; min-width: 0; }
    .num { min-width: 3.5em; text-align: right; }
    code { font-family: var(--mono); }
    p { margin: 0; }
  `,
})
export class CanvasToolbarComponent {
  protected readonly edit = inject(VisualEditService);
  protected readonly journal = inject(JournalService);

  protected readonly sel = this.edit.selection;
  protected readonly selected = computed(() => new Set(this.sel()));
  /** The one canvas selected, if just one is. */
  protected readonly one = computed(() => (this.sel().length === 1 ? this.sel()[0] : null));
  protected readonly editsFile = computed(() => editsPath(this.journal.entry() ?? 'entry'));
  /** Whether a selected canvas is drawn on the page shown, to arrange there. */
  protected readonly onPageSelection = computed(() => this.edit.placedOf(this.sel()).length > 0);
  protected readonly firstInk = computed(() => (this.sel().length ? this.edit.edit(this.sel()[0]).ink ?? {} : {}));

  protected readonly alignments: { how: Alignment; label: string; title: string }[] = [
    { how: 'left', label: '⇤', title: 'Line up the left edges' },
    { how: 'centre', label: '↔', title: 'Line up the centres across' },
    { how: 'right', label: '⇥', title: 'Line up the right edges' },
    { how: 'top', label: '⤒', title: 'Line up the tops' },
    { how: 'middle', label: '↕', title: 'Line up the middles down' },
    { how: 'bottom', label: '⤓', title: 'Line up the bottoms' },
  ];

  protected onPage(shown: { page: number }[]): boolean {
    const index = this.journal.selected()?.index;
    return shown.some((s) => s.page === index);
  }

  protected fixed(x: number): number {
    return Math.round(x * 100) / 100;
  }

  protected log2(x: number): number {
    return Math.log2(x);
  }

  private number(event: Event): number | null {
    const value = (event.target as HTMLInputElement).valueAsNumber;
    return Number.isFinite(value) ? value : null;
  }

  protected toggle(name: string, what: 'hidden' | 'locked') {
    this.edit.updateOne(name, (e) => ({ ...e, [what]: !e[what] }));
    if (what === 'locked' && this.edit.edit(name).locked) this.edit.selection.update((s) => s.filter((n) => n !== name));
  }

  protected set(name: string, what: 'turn' | 'skew', event: Event) {
    const value = this.number(event);
    if (value !== null) this.edit.updateOne(name, (e) => ({ ...e, [what]: value }));
  }

  protected setMove(name: string, axis: 0 | 1, event: Event) {
    const value = this.number(event);
    if (value === null) return;
    this.edit.updateOne(name, (e) => {
      const move: [number, number] = [...(e.move ?? [0, 0])];
      move[axis] = value;
      return { ...e, move };
    });
  }

  protected setScale(name: string, axis: 0 | 1, event: Event) {
    const value = this.number(event);
    if (value === null || value === 0) return;
    this.edit.updateOne(name, (e) => {
      const scale: [number, number] = [...(e.scale ?? [1, 1])];
      scale[axis] = value / 100;
      return { ...e, scale };
    });
  }

  /** Set an ink setting on every canvas selected; a drag of a slider or the colour is one undo step. */
  protected setInk<K extends keyof NonNullable<CanvasEdit['ink']>>(what: K, value: NonNullable<CanvasEdit['ink']>[K] | undefined) {
    const names = this.sel();
    const key = `ink-${what}-${names.join(' ')}`;
    this.edit.layers.set([]);
    this.edit.update((canvases) => {
      for (const name of names) {
        const edit = canvases[name] ?? {};
        canvases[name] = { ...edit, ink: { ...edit.ink, [what]: value } };
      }
    }, key);
  }

  protected resetAll() {
    this.edit.layers.set([]);
    this.edit.update((canvases) => {
      for (const name of this.sel()) canvases[name] = { hidden: canvases[name]?.hidden, locked: canvases[name]?.locked };
    });
  }

  private pageBox(): Bounds | null {
    const page = this.journal.selected();
    if (!page?.view) return null;
    if (page.size) return [0, 0, page.size[0], page.size[1]];
    const [x, y, w, h] = page.view;
    return [x, y, x + w, y + h];
  }

  protected alignTo(how: Alignment) {
    this.edit.align(this.sel(), how, this.pageBox());
  }

  protected centre(name: string, fit: boolean) {
    const page = this.pageBox();
    if (page) this.edit.centre(name, page, fit);
  }
}
