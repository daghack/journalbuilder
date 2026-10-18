import { Component, computed, inject, signal } from '@angular/core';
import { fontChanges, parseFonts, type FontChange, type FontsFile } from '../fonts/font-file';
import { ToolsService } from '../services/tools.service';
import { WorkspaceService } from '../services/workspace.service';
import { ShellService, type Mode } from './shell.service';

/** Across the top: which editor shows, and what the workspace changes from the repository's files. */
@Component({
  selector: 'app-mode-tabs',
  template: `
    <nav class="tabs" aria-label="Editors">
      @for (tab of tabs; track tab.mode) {
        <button type="button" [class.on]="shell.mode() === tab.mode" [attr.aria-current]="shell.mode() === tab.mode ? 'page' : null"
                (click)="shell.mode.set(tab.mode)" [title]="tab.title">{{ tab.label }}</button>
      }
    </nav>
    <span class="spacer"></span>
    @if (workspace.fonts() !== null) {
      <div class="menu">
        <button type="button" class="chip" aria-haspopup="true" (click)="shell.mode.set('fonts')"
                title="Journals use the fonts as edited in the font editor, not the built-in fonts">Edited Fonts</button>
        <ul class="list" aria-label="Edited fonts">
          @for (font of changedFonts(); track font.name) {
            <li>
              @if (font.change === 'removed') {
                <span class="removed">{{ font.name }} <em>removed</em></span>
              } @else {
                <button type="button" (click)="shell.openFont(font.name)" [title]="'Edit ' + font.name">
                  {{ font.name }}@if (font.change === 'added') { <em>new</em>}
                </button>
              }
            </li>
          } @empty {
            <li><span class="removed">{{ builtinFonts() ? 'No font differs' : 'Loading…' }}</span></li>
          }
        </ul>
      </div>
    }
    @if (lockNames().length) {
      <span class="chip" [title]="'Lock files only in this browser: ' + lockNames().join(', ')">
        {{ lockNames().length }} workspace lock{{ lockNames().length === 1 ? '' : 's' }}
      </span>
    }
  `,
  styles: `
    :host { display: flex; align-items: center; gap: 8px; padding: 4px 10px; background: var(--rail-bg); color: var(--rail-fg); font-size: 12px; }
    .tabs { display: inline-flex; gap: 2px; }
    .tabs button { background: transparent; border: 1px solid transparent; color: var(--rail-fg); padding: 3px 12px; }
    .tabs button:hover { background: var(--rail-hover); border-color: transparent; }
    .tabs button.on { background: var(--rail-selected-bg); border-color: var(--rail-selected); font-weight: 600; }
    .spacer { flex: 1; }
    .chip { padding: 2px 8px; border-radius: 10px; background: var(--rail-selected-bg); border: 1px solid var(--rail-selected); color: var(--rail-fg); font: inherit; }
    .menu { position: relative; }
    .menu > .chip { cursor: pointer; }
    .list { display: none; position: absolute; right: 0; top: 100%; z-index: 20; margin: 0; padding: 4px; list-style: none; min-width: 100%;
            background: var(--surface); color: var(--fg); border: 1px solid var(--border); border-radius: 6px; box-shadow: 0 4px 14px rgb(0 0 0 / 0.25); }
    /* The gap above the list still counts as hovering it. */
    .list::before { content: ''; position: absolute; left: 0; right: 0; top: -6px; height: 6px; }
    .menu:hover .list, .menu:focus-within .list { display: block; }
    .list button, .list .removed { display: block; width: 100%; text-align: left; white-space: nowrap; padding: 3px 10px; font-family: var(--mono); }
    .list button { background: transparent; border: 0; color: inherit; cursor: pointer; border-radius: 4px; }
    .list button:hover, .list button:focus-visible { background: var(--surface-2); }
    .list .removed { color: var(--muted); }
    em { font-style: normal; color: var(--muted); font-family: var(--font); margin-left: 4px; }
  `,
})
export class ModeTabsComponent {
  protected readonly shell = inject(ShellService);
  protected readonly workspace = inject(WorkspaceService);
  protected readonly lockNames = computed(() => Object.keys(this.workspace.locks()));
  /** the built-in fonts as built in, read; null until the renderer gives it. */
  protected readonly builtinFonts = signal<FontsFile | null>(null);
  /** The fonts the edited fonts change from the built-in ones. */
  protected readonly changedFonts = computed<FontChange[]>(() => {
    const edited = this.workspace.fonts();
    const builtin = this.builtinFonts();
    if (edited === null || builtin === null) return [];
    try {
      return fontChanges(parseFonts(edited), builtin);
    } catch {
      return [];
    }
  });
  protected readonly tabs: { mode: Mode; label: string; title: string }[] = [
    { mode: 'journal', label: 'Journal', title: 'Write journal scripts and see their pages' },
    { mode: 'fonts', label: 'Fonts', title: 'Draw glyphs and set font options' },
    { mode: 'locks', label: 'Locks', title: 'Wire lock types together into new ones' },
  ];

  constructor() {
    inject(ToolsService).builtinFonts()
      .then((text) => this.builtinFonts.set(parseFonts(text)))
      .catch(() => {});
  }
}
