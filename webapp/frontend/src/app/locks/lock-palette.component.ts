import { Component, computed, inject, signal } from '@angular/core';
import type { IllustrationType, LockType } from '../scribe';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import type { SourceKind } from './chain-model';
import { LockEditorService } from './lock-editor.service';

/** The left: the chains, to edit; every lock type and illustration, to drag into the chain (or click to add). */
@Component({
  selector: 'app-lock-palette',
  template: `
    <div class="head">
      <input type="text" placeholder="Find a lock type or illustration" aria-label="Find a lock type" [value]="filter()" (input)="filter.set($any($event.target).value)" />
    </div>
    @if (editor.typesError(); as error) { <p class="error">{{ error }}</p> }
    <h3 class="renderers-head">Chains <button type="button" class="open" (click)="editor.newFile('chains')" title="Start a new chain">New</button></h3>
    <p class="note">Click one to edit it: entries using chain("name") follow along.</p>
    <ul>
      @for (c of chains(); track c.name) {
        <li class="type chain" [class.current]="c.name === editor.model().name" [class.broken]="!!c.error" (click)="editor.openFile('chains/' + c.name + '.rhai')" [title]="c.error ?? c.description ?? ''">
          <span class="name">{{ c.name }}</span>
          @if (c.own && c.builtin) { <span class="tag warn" title="The journal's stands in for the library's chain of that name">⚠</span> }
          @else if (c.own) { <span class="tag" title="The journal's own">journal</span> }
          <span class="about">{{ c.error ?? c.description }}</span>
        </li>
      }
    </ul>
    @for (group of groups(); track group.title) {
      @if (group.types.length) {
        <h3>{{ group.title }}</h3>
        <ul>
          @for (type of group.types; track type.name) {
            <li class="type" draggable="true" [class.broken]="!!type.error" (dragstart)="drag($event, type)"
                (click)="!type.error && add(type)" [title]="type.error ?? type.description ?? ''">
              <span class="name">{{ type.name }}</span>
              @if (type.composite) { <span class="tag" title="A composite lock file: other lock types wired together">composite</span> }
              @if (type.own && type.builtin) { <span class="tag warn" title="The journal's stands in for the library's lock type of that name">⚠</span> }
              <button type="button" class="open" (click)="$event.stopPropagation(); editor.openFile('locks/' + type.name + '.rhai')" title="Open this lock type's file">Open</button>
              <span class="about">{{ type.error ?? type.description }}</span>
            </li>
          }
        </ul>
      }
    }
    <h3 class="renderers-head">Sources</h3>
    <p class="note">The journal's files a chain draws on: content feeds locks and illustrations; styles, glyphsets and layouts feed illustrations.</p>
    @for (group of sources(); track group.kind) {
      @if (group.names.length) {
        <h3>{{ group.title }}</h3>
        <ul>
          @for (name of group.names; track name) {
            <li class="type source" draggable="true" (dragstart)="dragSource($event, group.kind, name)" (click)="addSource(group.kind, name)"
                [title]="'Add a ' + group.kind + ' source drawing on ' + name">
              <span class="name">{{ name }}</span>
            </li>
          }
        </ul>
      }
    }
    <h3 class="renderers-head">Illustrations <button type="button" class="open" (click)="editor.newFile('illustrations')" title="Write a new illustration file">New</button></h3>
    <p class="note">Draw a lock's values as canvases: wire an output to one. Entries attach them with lock.attach.</p>
    <ul>
      @for (type of renderers(); track type.name) {
        <li class="type renderer" draggable="true" [class.broken]="!!type.error" (dragstart)="dragRenderer($event, type)"
            (click)="!type.error && addRenderer(type)" [title]="type.error ?? type.description ?? ''">
          <span class="name">{{ type.name }}</span>
          @if (type.own && type.builtin) { <span class="tag warn" title="The journal's stands in for the library's illustration of that name">⚠</span> }
          <button type="button" class="open" (click)="$event.stopPropagation(); editor.openFile('illustrations/' + type.name + '.rhai')" title="Open this illustration's file">Open</button>
          <span class="about">{{ type.error ?? type.description }}</span>
        </li>
      }
    </ul>
  `,
  styles: `
    :host { display: flex; flex-direction: column; overflow-y: auto; background: var(--rail-bg); color: var(--rail-fg); min-height: 0; font-size: 12px; }
    .head { padding: 8px; display: flex; }
    .head input { flex: 1; }
    .error { margin: 0 8px; color: #f2a59c; }
    h3 { margin: 10px 10px 4px; font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--rail-selected); }
    ul { list-style: none; margin: 0; padding: 0 6px; display: flex; flex-direction: column; gap: 2px; }
    .type { display: grid; grid-template-columns: 1fr auto auto auto; gap: 2px 6px; align-items: center; padding: 6px 8px; border-radius: 6px; cursor: grab; border: 1px solid transparent; }
    .type:hover { background: var(--rail-hover); border-color: var(--rail-selected-bg); }
    .type.broken { cursor: default; opacity: 0.7; }
    .name { font: 600 12px var(--mono); overflow: hidden; text-overflow: ellipsis; }
    .tag { font-size: 10px; padding: 0 5px; border-radius: 8px; background: var(--rail-selected-bg); border: 1px solid var(--rail-selected); }
    .about { grid-column: 1 / -1; color: #a39b90; font-size: 11px; line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
    .type.broken .about { color: #f2a59c; }
    .open, .remove { padding: 0 6px; font-size: 11px; background: transparent; color: var(--rail-fg); border-color: var(--rail-selected); }
    .renderers-head { display: flex; align-items: center; gap: 6px; margin-top: 14px; }
    .note { margin: 0 10px 4px; color: #a39b90; font-size: 11px; }
    .type.renderer { border-left: 2px dashed #8fc1e3; }
    .type.source { border-left: 2px solid #c9a227; cursor: grab; }
    .type.chain { cursor: pointer; }
    .type.chain.current { border-color: var(--rail-selected); }
    .tag.warn { color: var(--warning); border-color: var(--warning); background: transparent; }
  `,
})
export class LockPaletteComponent {
  protected readonly editor = inject(LockEditorService);
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);
  protected readonly filter = signal('');

  protected readonly groups = computed(() => {
    const filter = this.filter().trim().toLowerCase();
    const types = this.editor.types().filter((t) => !filter || t.name.includes(filter) || (t.description ?? '').toLowerCase().includes(filter));
    return [
      { title: "The journal's lock types", types: types.filter((t) => t.own) },
      { title: 'Lock types', types: types.filter((t) => !t.own) },
    ];
  });

  protected readonly chains = computed(() => {
    const filter = this.filter().trim().toLowerCase();
    return this.editor.chainTypes().filter((c) => !filter || c.name.includes(filter) || (c.description ?? '').toLowerCase().includes(filter));
  });

  protected readonly renderers = computed(() => {
    const filter = this.filter().trim().toLowerCase();
    return this.editor.rendererTypes().filter((t) => !filter || t.name.includes(filter) || (t.description ?? '').toLowerCase().includes(filter));
  });

  /** The files a source can draw on, by kind: the journal's and the library's. */
  protected readonly sources = computed(() => {
    const filter = this.filter().trim().toLowerCase();
    const paths = [...Object.keys(this.store.files()), ...Object.keys(this.library.files())];
    const names = (dir: string) => [...new Set(paths.filter((p) => p.startsWith(`${dir}/`)).map((p) => p.slice(dir.length + 1).replace(/\.[a-z]+$/, '')))]
      .filter((name) => !filter || name.includes(filter)).sort();
    return [
      { kind: 'content' as SourceKind, title: 'Content', names: names('content') },
      { kind: 'style' as SourceKind, title: 'Styles', names: names('styles') },
      { kind: 'glyphset' as SourceKind, title: 'Glyphsets', names: names('glyphsets') },
      { kind: 'layout' as SourceKind, title: 'Layouts', names: names('layouts') },
    ];
  });

  protected dragSource(event: DragEvent, kind: SourceKind, name: string) {
    event.dataTransfer?.setData('application/x-source', JSON.stringify({ kind, name }));
    event.dataTransfer!.effectAllowed = 'copy';
  }

  /** Clicked: add it left of the locks. */
  protected addSource(kind: SourceKind, name: string) {
    const model = this.editor.model();
    const placed = [...model.nodes, ...model.sources];
    const x = placed.length ? Math.min(...placed.map((n) => n.x)) - (model.sources.length ? 0 : 300) : 120;
    const y = model.sources.length ? Math.max(...model.sources.map((n) => n.y)) + 120 : 60;
    this.editor.addSource(kind, name, x, y);
  }

  protected dragRenderer(event: DragEvent, type: IllustrationType) {
    if (type.error) return event.preventDefault();
    event.dataTransfer?.setData('application/x-renderer-type', type.name);
    event.dataTransfer!.effectAllowed = 'copy';
  }

  /** Clicked: attach it below the locks. */
  protected addRenderer(type: IllustrationType) {
    const model = this.editor.model();
    const placed = [...model.nodes, ...model.renderers];
    const x = placed.length ? Math.max(...placed.map((n) => n.x)) + 360 : 120;
    const y = placed.length ? Math.max(...placed.map((n) => n.y)) + 160 : 60;
    this.editor.addRenderer(type.name, x, y);
  }

  protected drag(event: DragEvent, type: LockType) {
    if (type.error) return event.preventDefault();
    event.dataTransfer?.setData('application/x-lock-type', type.name);
    event.dataTransfer!.effectAllowed = 'copy';
  }

  /** Clicked: add it beside the locks already there. */
  protected add(type: LockType) {
    const nodes = this.editor.model().nodes;
    const x = nodes.length ? Math.max(...nodes.map((n) => n.x)) + 360 : 120;
    const y = nodes.length ? nodes[nodes.length - 1].y : 60;
    this.editor.addNode(type.name, x, y);
  }
}
