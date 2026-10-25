import { Component, computed, inject, signal } from '@angular/core';
import { KINDS, VALID_NAME, entryPath, nameOf, pathOf, splitPath, type KindInfo } from '../journal/journal-files';
import { template } from '../journal/templates';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { ShellService } from './shell.service';

const OPEN_KEY = 'journal-editor.sidebar-open';

/** A file as the sidebar lists it. */
interface Item {
  path: string;
  label: string;
  /** The journal's file stands in for the library's. */
  overrides: boolean;
  /** An entry file the manifest doesn't list. */
  unlisted?: boolean;
}

/** A kind's files: the journal's own, and the library's it hasn't its own of. */
interface Group {
  info: KindInfo;
  own: Item[];
  library: Item[];
}

function readOpen(): Record<string, boolean> {
  try {
    return JSON.parse(localStorage.getItem(OPEN_KEY) ?? '{}') ?? {};
  } catch {
    return {};
  }
}

/** Which kinds' lists are open: one for every editor's Files tab. */
const open = signal<Record<string, boolean>>(readOpen());

/** The Files tab: the journal's title and its files, kind by kind, with the library's beside them. */
@Component({
  selector: 'app-file-sidebar',
  template: `
    <header>
      <input class="title" aria-label="The journal's title" [value]="store.manifest().title" (change)="setTitle($any($event.target).value)" />
      <textarea class="description" rows="2" aria-label="What the journal is" placeholder="What the journal is…"
                [value]="store.manifest().description ?? ''" (change)="setDescription($any($event.target).value)"></textarea>
    </header>
    @for (group of groups(); track group.info.kind) {
      <section>
        <h3>
          <button type="button" class="toggle" (click)="toggle(group.info.kind)" [attr.aria-expanded]="isOpen(group.info.kind)">
            <span class="chevron">{{ isOpen(group.info.kind) ? '▾' : '▸' }}</span>{{ group.info.label }}
            <span class="count">{{ group.own.length }}</span>
          </button>
          <button type="button" class="add" (click)="add(group.info)" [title]="'New ' + group.info.label.toLowerCase().replace(/s$/, '') + ' file'">+</button>
        </h3>
        @if (isOpen(group.info.kind)) {
          <ul>
            @for (item of group.own; track item.path) {
              <li [class.selected]="shell.selected() === item.path" [class.unlisted]="item.unlisted">
                <button type="button" class="file" (click)="shell.open(item.path)" [title]="item.path">
                  {{ item.label }}
                  @if (item.overrides) { <span class="override" title="Stands in for the library's file of the same name">⚠</span> }
                  @if (item.unlisted) { <span class="note" title="Not in journal.json's entries, so not built">unlisted</span> }
                </button>
                <span class="actions">
                  @if (group.info.kind === 'entries') {
                    @if (item.unlisted) {
                      <button type="button" (click)="store.listEntry(nameOf(item.path))" title="List it in journal.json's entries">list</button>
                    } @else {
                      <button type="button" (click)="store.moveEntry(nameOf(item.path), -1)" title="Move up">↑</button>
                      <button type="button" (click)="store.moveEntry(nameOf(item.path), 1)" title="Move down">↓</button>
                    }
                  }
                  <button type="button" (click)="rename(item.path)" title="Rename">✎</button>
                  <button type="button" (click)="remove(item.path)" [title]="item.overrides ? 'Remove it: the library\\'s is used again' : 'Remove it'">✕</button>
                </span>
              </li>
            } @empty {
              @if (!group.library.length) { <li class="empty">none</li> }
            }
          </ul>
          @if (group.library.length) {
            <button type="button" class="library-toggle" (click)="toggle('library:' + group.info.kind)">
              {{ isOpen('library:' + group.info.kind) ? '▾' : '▸' }} library <span class="count">{{ group.library.length }}</span>
            </button>
            @if (isOpen('library:' + group.info.kind)) {
              <ul class="library">
                @for (item of group.library; track item.path) {
                  <li [class.selected]="shell.selected() === item.path">
                    <button type="button" class="file" (click)="shell.open(item.path)" [title]="'The library\\'s ' + item.path">{{ item.label }}</button>
                  </li>
                }
              </ul>
            }
          }
        }
      </section>
    }
    @if (message(); as m) { <p class="message">{{ m }}</p> }
  `,
  styles: `
    :host { display: flex; flex-direction: column; overflow: auto; background: var(--surface); font-size: 12px; padding-bottom: 12px; }
    header { padding: 8px; display: flex; flex-direction: column; gap: 4px; border-bottom: 1px solid var(--border); }
    .title { font-weight: 600; font-size: 13px; }
    .description { resize: vertical; font: inherit; color: var(--muted); }
    section { padding: 2px 0; }
    h3 { display: flex; align-items: center; margin: 4px 0 0; font-size: 12px; }
    .toggle { flex: 1; display: flex; align-items: center; gap: 4px; background: transparent; border: 0; padding: 3px 8px; text-align: left; font-weight: 600; cursor: pointer; color: var(--fg); }
    .chevron { width: 10px; color: var(--muted); }
    .count { color: var(--muted); font-weight: 400; margin-left: 4px; }
    .add { background: transparent; border: 0; padding: 0 8px; font-size: 15px; line-height: 1; color: var(--muted); cursor: pointer; }
    .add:hover { color: var(--accent); }
    ul { list-style: none; margin: 0; padding: 0; }
    li { display: flex; align-items: center; }
    li.selected { background: var(--selected-bg, var(--surface-2)); }
    li.selected .file { font-weight: 600; }
    .file { flex: 1; min-width: 0; text-align: left; background: transparent; border: 0; padding: 3px 8px 3px 24px; cursor: pointer; color: var(--fg);
            font-family: var(--mono); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .file:hover { background: var(--surface-2); }
    li.unlisted .file { color: var(--muted); }
    .override { color: var(--warning); margin-left: 4px; font-family: var(--font); }
    .note { color: var(--muted); margin-left: 4px; font-family: var(--font); font-size: 11px; }
    .actions { display: none; gap: 1px; padding-right: 4px; }
    li:hover .actions, li:focus-within .actions { display: inline-flex; }
    .actions button { background: transparent; border: 0; padding: 0 3px; color: var(--muted); cursor: pointer; font-size: 11px; }
    .actions button:hover { color: var(--accent); }
    .empty { color: var(--muted); padding: 2px 24px; }
    .library-toggle { background: transparent; border: 0; padding: 2px 8px 2px 24px; color: var(--muted); cursor: pointer; font-size: 11px; }
    .library .file { color: var(--muted); padding-left: 36px; }
    .message { margin: 8px; color: var(--danger); }
  `,
})
export class FileSidebarComponent {
  protected readonly store = inject(JournalStore);
  protected readonly shell = inject(ShellService);
  private readonly library = inject(LibraryService);
  protected readonly message = signal<string | null>(null);
  private readonly open = open;
  protected readonly nameOf = nameOf;

  protected readonly groups = computed<Group[]>(() => {
    const files = this.store.files();
    const library = this.library.byKind();
    const libraryFiles = this.library.files();
    const label = (path: string) => {
      const split = splitPath(path)!;
      return split.kind === 'content' ? `${split.name}.${split.ext}` : split.name;
    };
    return KINDS.map((info) => {
      const paths = Object.keys(files).filter((path) => splitPath(path)?.kind === info.kind).sort();
      let own: Item[];
      if (info.kind === 'entries') {
        const listed = this.store.manifest().entries.map(entryPath).filter((path) => path in files);
        own = [
          ...listed.map((path) => ({ path, label: label(path), overrides: false })),
          ...paths.filter((path) => !listed.includes(path)).map((path) => ({ path, label: label(path), overrides: false, unlisted: true })),
        ];
      } else {
        own = paths.map((path) => ({ path, label: label(path), overrides: path in libraryFiles }));
      }
      const theirs = (library.get(info.kind) ?? []).filter((path) => !(path in files)).map((path) => ({ path, label: label(path), overrides: false }));
      return { info, own, library: theirs };
    });
  });

  protected isOpen(key: string): boolean {
    return this.open()[key] ?? !key.startsWith('library:');
  }

  protected toggle(key: string) {
    this.open.update((open) => ({ ...open, [key]: !this.isOpen(key) }));
    try {
      localStorage.setItem(OPEN_KEY, JSON.stringify(this.open()));
    } catch {
      // Not kept, then.
    }
  }

  private say(message: string | null) {
    this.message.set(message);
    if (message) setTimeout(() => this.message() === message && this.message.set(null), 8_000);
  }

  protected setTitle(title: string) {
    this.store.setManifest({ title: title.trim() || 'Untitled' });
  }

  protected setDescription(description: string) {
    this.store.setManifest({ description: description.trim() });
  }

  /** A new file of `info`'s kind, named as the user says. */
  protected add(info: KindInfo) {
    const hint = info.kind === 'content' ? ' (with .txt, .md, or .json for symbols)' : '';
    const typed = prompt(`New ${info.label.toLowerCase().replace(/s$/, '')} file's name${hint}:`)?.trim();
    if (!typed) return;
    const match = /^(.*?)(?:\.([a-z]+))?$/.exec(typed)!;
    const name = match[1];
    const ext = match[2] ?? info.exts[0];
    if (!VALID_NAME.test(name)) return this.say('A name is letters, digits and _ only.');
    if (!info.exts.includes(ext)) return this.say(`${info.label} files end .${info.exts.join(', .')}.`);
    const path = pathOf(info.kind, name, ext);
    if (path in this.store.files()) return this.say(`There's a ${path} already.`);
    if (info.kind === 'entries') {
      const problem = this.store.addEntry(name);
      if (problem) return this.say(problem);
    } else {
      this.store.setFile(path, template(info.kind, name, ext));
    }
    this.open.update((open) => ({ ...open, [info.kind]: true }));
    this.shell.open(path);
    if (this.library.has(path)) this.say(`${path} stands in for the library's file of that name.`);
  }

  protected rename(path: string) {
    const split = splitPath(path)!;
    const typed = prompt(`Rename ${path} to:`, split.name)?.trim();
    if (!typed || typed === split.name) return;
    const to = pathOf(split.kind, typed, split.ext);
    const problem = this.store.renameFile(path, to);
    if (problem) return this.say(problem);
    if (this.shell.selected() === path) this.shell.open(to);
  }

  protected remove(path: string) {
    const builtin = this.library.has(path);
    const doing = builtin ? `Remove ${path}? The library's file of that name is used again.` : `Remove ${path}? It's gone once the journal is exported without it.`;
    if (!confirm(doing)) return;
    this.store.removeFile(path);
    if (builtin && this.shell.selected() === path) this.shell.open(path);
  }
}

