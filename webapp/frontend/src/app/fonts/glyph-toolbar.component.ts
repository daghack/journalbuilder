import { Component, computed, inject, input } from '@angular/core';
import { FontEditorService, type Tool } from './font-editor.service';
import type { GlyphCanvasComponent } from './glyph-canvas.component';

/** Over the glyph canvas: the tools that add to a glyph, and what to do with what's picked. */
@Component({
  selector: 'app-glyph-toolbar',
  host: { class: 'toolbar' },
  template: `
    @for (t of tools; track t.tool) {
      <button type="button" [class.on]="editor.tool() === t.tool" (click)="editor.tool.set(t.tool)" [title]="t.title">{{ t.label }}</button>
    }
    <span class="sep"></span>
    @if (editor.pickedSegment()) {
      <span class="muted">Curve:</span>
      <button type="button" (click)="editor.convertPicked('L')" title="Make the picked curve a straight line">Line</button>
      <button type="button" (click)="editor.convertPicked('Q')" title="Make the picked curve a quadratic">Quad</button>
      <button type="button" (click)="editor.convertPicked('C')" title="Make the picked curve a cubic">Cubic</button>
      <button type="button" (click)="editor.splitPicked()" title="Add a point halfway along the picked curve (or double-click a curve)">Split</button>
    }
    <button type="button" [disabled]="!hasPick()" (click)="editor.toggleClosed()" title="Close the picked stroke back to its start, or open it">{{ closed() ? 'Open' : 'Close' }}</button>
    <button type="button" [disabled]="!hasPick()" (click)="editor.reversePicked()" title="Draw the picked stroke the other way (pressure and pooling follow the pen)">Reverse</button>
    <button type="button" [disabled]="!hasPick()" (click)="editor.deletePicked()" title="Delete what's picked (Delete)">Delete</button>
    <span class="sep"></span>
    <button type="button" [disabled]="!editor.canUndo()" (click)="editor.undo()" title="Undo (Ctrl/⌘ Z)">Undo</button>
    <button type="button" [disabled]="!editor.canRedo()" (click)="editor.redo()" title="Redo (Ctrl/⌘ Shift Z)">Redo</button>
    <span class="sep"></span>
    <label title="Snap points to a 0.05 grid (hold Alt not to)"><input type="checkbox" [checked]="editor.snap()" (change)="editor.snap.set($any($event.target).checked)" /> Snap</label>
    <label title="Show the glyph's other variants faintly"><input type="checkbox" [checked]="editor.onion()" (change)="editor.onion.set($any($event.target).checked)" /> Variants</label>
    <span class="spacer"></span>
    <button type="button" (click)="canvas()?.zoom(1 / 1.25)" title="Zoom out" aria-label="Zoom out">−</button>
    <button type="button" (click)="canvas()?.fit()" title="Fit the glyph (F)">Fit</button>
    <button type="button" (click)="canvas()?.zoom(1.25)" title="Zoom in" aria-label="Zoom in">+</button>
  `,
  styles: `
    .spacer { flex: 1; }
    .muted { color: var(--muted); }
    label { display: inline-flex; gap: 4px; align-items: center; }
  `,
})
export class GlyphToolbarComponent {
  protected readonly editor = inject(FontEditorService);
  readonly canvas = input<GlyphCanvasComponent | undefined>();

  protected readonly tools: { tool: Tool; label: string; title: string }[] = [
    { tool: 'select', label: 'Select', title: 'Pick points and curves; drag them; drag across empty space to pick several (V)' },
    { tool: 'stroke', label: 'Stroke', title: 'Begin a new pen stroke where you click (M)' },
    { tool: 'L', label: 'Line', title: 'Add a straight line to the stroke, to where you click (L)' },
    { tool: 'Q', label: 'Quad', title: 'Add a quadratic curve to the stroke (Q)' },
    { tool: 'C', label: 'Cubic', title: 'Add a cubic curve to the stroke (C)' },
    { tool: 'dot', label: 'Dot', title: 'Add a dot: a stroke too short to see as a line (D)' },
    { tool: 'ellipse', label: 'Ellipse', title: 'Drag out an ellipse from its centre; Shift for a circle (E)' },
  ];

  protected readonly hasPick = computed(() => this.editor.picked().length > 0 || this.editor.pickedSegment() !== null);
  protected readonly closed = computed(() => {
    this.editor.picked();
    const s = this.editor.pickedStroke();
    return s !== null && !!this.editor.current()?.[s]?.closed;
  });
}
