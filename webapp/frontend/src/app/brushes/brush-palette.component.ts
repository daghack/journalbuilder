import { Component, computed, inject, signal } from '@angular/core';
import type { BrushNodeType } from '../scribe';
import { BrushEditorService } from './brush-editor.service';
import { NODE_DRAG } from './brush-graph.component';

/** The left: every node type brushes are made of, by category, to drag into the brush (or click to add). */
@Component({
  selector: 'app-brush-palette',
  template: `
    <div class="head">
      <input type="text" placeholder="Find a node type" aria-label="Find a node type" [value]="filter()" (input)="filter.set($any($event.target).value)" />
    </div>
    @if (editor.loadError(); as error) { <p class="error">{{ error }}</p> }
    <p class="note">Drag one into the brush, or click it. Each stroke runs from the inputs (the pen) through the nodes to Brush Settings and Composite, which paint it.</p>
    @for (group of groups(); track group.category) {
      <h3>{{ group.category }}</h3>
      <ul>
        @for (type of group.types; track type.type_id) {
          <li class="type" [class.terminal]="type.is_terminal" draggable="true" (dragstart)="drag($event, type)" (click)="editor.add(type.type_id)" [title]="type.description">
            <span class="name">{{ type.display_name }}</span>
            <span class="about">{{ type.description }}</span>
          </li>
        }
      </ul>
    }
  `,
  styles: `
    :host { display: flex; flex-direction: column; overflow-y: auto; background: var(--rail-bg); color: var(--rail-fg); min-height: 0; font-size: 12px; padding-bottom: 12px; }
    .head { padding: 8px; display: flex; }
    .head input { flex: 1; }
    .error { margin: 0 8px; color: #f2a59c; }
    .note { margin: 0 10px 4px; color: #a39b90; font-size: 11px; }
    h3 { margin: 10px 10px 4px; font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--rail-selected); }
    ul { list-style: none; margin: 0; padding: 0 6px; display: flex; flex-direction: column; gap: 2px; }
    .type { display: flex; flex-direction: column; gap: 2px; padding: 6px 8px; border-radius: 6px; cursor: grab; border: 1px solid transparent; }
    .type:hover { background: var(--rail-hover); border-color: var(--rail-selected-bg); }
    .type.terminal { border-left: 2px solid #c9a227; }
    .name { font-weight: 600; }
    .about { color: #a39b90; font-size: 11px; line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  `,
})
export class BrushPaletteComponent {
  protected readonly editor = inject(BrushEditorService);
  protected readonly filter = signal('');

  protected readonly groups = computed(() => {
    const types = this.editor.types();
    if (!types) return [];
    const find = this.filter().trim().toLowerCase();
    const shows = (t: BrushNodeType) => !find || `${t.display_name} ${t.type_id} ${t.description}`.toLowerCase().includes(find);
    return types.categories().map((g) => ({ ...g, types: g.types.filter(shows) })).filter((g) => g.types.length);
  });

  protected drag(event: DragEvent, type: BrushNodeType) {
    event.dataTransfer?.setData(NODE_DRAG, type.type_id);
    event.dataTransfer!.effectAllowed = 'copy';
  }
}
