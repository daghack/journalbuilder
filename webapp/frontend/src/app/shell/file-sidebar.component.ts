import { Component, ElementRef, computed, inject, signal, viewChild } from '@angular/core';
import { showEditorTab } from '../layout/pane-state';
import { CONTENT_TYPES, KINDS, VALID_NAME, bytesToDataUrl, contentType, entryPath, journalName, nameOf, pathOf, splitPath, type KindInfo } from '../journal/journal-files';
import { template } from '../journal/templates';
import { StyleBuilderComponent } from '../resources/style-builder.component';
import { CONTENT_ACCEPT, contentExt, importedName, importedText, type Imported } from '../journal/linked-content';
import { symbolHelp } from '../journal/symbols';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { LinkedContentService, type LinkState, type PickedLink } from '../services/linked-content.service';
import { ShellService } from './shell.service';

const OPEN_KEY = 'journal-editor.sidebar-open';

/** A file being imported: its text, the type it's kept as (its extension's to start with), and the name it'll have; symbols' base and whether they're strict. */
interface ImportRow {
  file: File;
  text: string;
  /** Its file's type, by extension. */
  fromExt: string;
  ext: string;
  name: string;
  base: number;
  strict: boolean;
}

/** A file as the sidebar lists it. */
interface Item {
  path: string;
  label: string;
  /** A content file's type: String, Markdown or Symbols. */
  tag?: string;
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
  imports: [StyleBuilderComponent],
  template: `
    <header>
      <input class="title" aria-label="The journal's title" [value]="store.manifest().title" (change)="setTitle($any($event.target).value)" />
      <textarea class="description" rows="2" aria-label="What the journal is" placeholder="What the journal is…"
                [value]="store.manifest().description ?? ''" (change)="setDescription($any($event.target).value)"></textarea>
    </header>
    @for (group of groups(); track group.info.kind) {
      <section [class.dropping]="group.info.kind === 'content' && dropping()"
               (dragover)="group.info.kind === 'content' && dragOver($event)" (dragleave)="dropping.set(false)"
               (drop)="group.info.kind === 'content' && drop($event)">
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
                <button type="button" class="file" (click)="openFile(item.path)" [title]="item.path">
                  {{ item.label }}
                  @if (item.tag) { <span class="note">{{ item.tag }}</span> }
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
                  @if (group.info.kind === 'styles') {
                    <button type="button" (click)="copyStyle(item.path)" title="Copy it to a new style">⧉</button>
                  }
                  <button type="button" (click)="rename(item.path)" title="Rename">✎</button>
                  <button type="button" (click)="remove(item.path)" [title]="item.overrides ? 'Remove it: the library\\'s is used again' : 'Remove it'">✕</button>
                </span>
              </li>
            } @empty {
              @if (!group.library.length && !(group.info.kind === 'content' && links.links().length)) { <li class="empty">none</li> }
            }
          </ul>
          @if (group.info.kind === 'content') {
            <div class="content-tools">
              <button type="button" (click)="importContent()" title="Copy a .md, .txt, .json or .graph file into the journal (or drop files here)">import…</button>
              @if (links.supported) {
                <button type="button" (click)="link('file')" title="Read a content file from your disk each run, as it is then">link file…</button>
                <button type="button" (click)="link('folder')" title="Read a folder's content files from your disk each run, as they are then">link folder…</button>
              }
            </div>
            @for (link of links.links(); track link.path) {
              <div class="link" [title]="stateNote(link.state)">
                <span class="link-path">⇄ {{ link.path }}</span>
                <span class="link-state" [class]="link.state">{{ stateLabel(link.state) }}</span>
                <span class="link-actions">
                  @if (link.state === 'unconnected' || link.state === 'missing') {
                    <button type="button" (click)="connect(link.path)" title="Pick the file or folder it links">connect…</button>
                  }
                  @if (link.state === 'permission') {
                    <button type="button" (click)="reconnect(link.path)" title="Let the page read it again">reconnect</button>
                  }
                  <button type="button" (click)="links.unlink(link.path)" title="Take it out of journal.json">✕</button>
                </span>
              </div>
              <ul class="linked">
                @for (path of linkedPaths(link.files); track path) {
                  <li [class.selected]="shell.selected() === path">
                    <button type="button" class="file" (click)="openFile(path)" [title]="path + ', linked from ' + (link.picked ?? link.path) + ': read from your disk, edited there'">
                      {{ nameOf(path) }} <span class="note">{{ tag(path) }}</span>
                      @if (path in store.files()) { <span class="override" title="Stands in for the journal's own file of the same name">⇄</span> }
                    </button>
                  </li>
                }
              </ul>
            }
          }
          @if (group.library.length) {
            <button type="button" class="library-toggle" (click)="toggle('library:' + group.info.kind)">
              {{ isOpen('library:' + group.info.kind) ? '▾' : '▸' }} library <span class="count">{{ group.library.length }}</span>
            </button>
            @if (isOpen('library:' + group.info.kind)) {
              <ul class="library">
                @for (item of group.library; track item.path) {
                  <li [class.selected]="shell.selected() === item.path">
                    <button type="button" class="file" (click)="openFile(item.path)" [title]="'The library\\'s ' + item.path">
                      {{ item.label }}
                      @if (item.tag) { <span class="note">{{ item.tag }}</span> }
                    </button>
                    @if (group.info.kind === 'styles') {
                      <span class="actions">
                        <button type="button" (click)="copyStyle(item.path)" title="Copy it to a new style">⧉</button>
                      </span>
                    }
                  </li>
                }
              </ul>
            }
          }
        }
      </section>
    }
    @if (message(); as m) { <p class="message">{{ m }}</p> }

    <dialog #importDialog (close)="importRows.set([])">
      <form method="dialog" (submit)="$event.preventDefault(); finishImport()">
        <h2>Import content</h2>
        <p class="hint">Copied into the journal's content/, as content("name") reads it.</p>
        <div class="rows">
          @for (row of importRows(); track row.file; let i = $index) {
            <div class="import-row">
              <span class="from">{{ row.file.name }}</span>
              <input type="text" spellcheck="false" aria-label="Its name" [value]="row.name" (input)="changeImport(i, { name: $any($event.target).value.trim() })" />
              <select aria-label="The content type it's kept as" (change)="changeImport(i, { ext: $any($event.target).value })">
                @for (t of contentTypes; track t.ext) { <option [value]="t.ext" [selected]="t.ext === row.ext">{{ t.label }}</option> }
              </select>
              @if (row.ext === 'json') {
                <span class="symbol-options">
                  <label>Base
                    <select (change)="changeImport(i, { base: +$any($event.target).value })">
                      @for (b of bases; track b) { <option [value]="b" [selected]="b === row.base">{{ b }}</option> }
                    </select>
                  </label>
                  <label [title]="'Strict: the text must be written in base ' + row.base + ' already (' + symbolHelp(row.base) + '). Otherwise it is made to fit as it is read: capitals, what the base cannot write dropped.'">
                    <input type="checkbox" [checked]="row.strict" (change)="changeImport(i, { strict: $any($event.target).checked })" /> Strict
                  </label>
                </span>
                @if (converted(row); as c) {
                  @if (!('error' in c) && c.reads !== undefined) {
                    <span class="row-note">{{ c.note ? c.note + ' ' : '' }}{{ c.count }} symbol{{ c.count === 1 ? '' : 's' }}, reading <code>{{ clip(c.reads) }}</code></span>
                  }
                }
              }
              @if (importProblem(row); as p) { <span class="row-note" [class.bad]="p.bad">{{ p.text }}</span> }
            </div>
          }
        </div>
        <div class="dialog-actions">
          <button type="button" (click)="importDialog.close()">Cancel</button>
          <button type="submit" [disabled]="!canImport()">Import</button>
        </div>
      </form>
    </dialog>

    <dialog #linkDialog (close)="picked.set(null); linkProblem.set(null)">
      <form method="dialog" (submit)="$event.preventDefault(); finishLink(linkPath.value)">
        <h2>Link {{ picked()?.as }}</h2>
        <p class="hint">{{ picked()?.name }} is read from your disk each run, and again whenever it changes: edit it there. The journal keeps a link to it, not a copy.</p>
        <label>Where journalbuild finds it, from the journal's folder (or absolute)
          <input #linkPath type="text" spellcheck="false" (input)="linkProblem.set(null)" />
        </label>
        @if (linkProblem(); as p) { <p class="message">{{ p }}</p> }
        <div class="dialog-actions">
          <button type="button" (click)="linkDialog.close()">Cancel</button>
          <button type="submit">Link</button>
        </div>
      </form>
    </dialog>

    <dialog #newContent (close)="contentProblem.set(null)">
      <form method="dialog" (submit)="$event.preventDefault(); addContent(contentName.value, contentKind.value)">
        <h2>New content</h2>
        <label>Name <input #contentName type="text" spellcheck="false" placeholder="letters, digits and _" (input)="contentProblem.set(null)" /></label>
        <label>Type
          <select #contentKind>
            @for (t of contentTypes; track t.ext) { <option [value]="t.ext">{{ t.label }}: {{ t.about }}</option> }
          </select>
        </label>
        @if (contentProblem(); as p) { <p class="message">{{ p }}</p> }
        <div class="dialog-actions">
          <button type="button" (click)="newContent.close()">Cancel</button>
          <button type="submit">Create</button>
        </div>
      </form>
    </dialog>

    <app-style-builder (created)="create(styleKind, $event.name, 'json', $event.text)" />
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
    section.dropping { outline: 2px dashed var(--accent); outline-offset: -2px; }
    .content-tools { display: flex; gap: 6px; padding: 2px 8px 2px 24px; }
    .content-tools button, .link-actions button { background: transparent; border: 0; padding: 0; color: var(--muted); cursor: pointer; font-size: 11px; }
    .content-tools button:hover, .link-actions button:hover { color: var(--accent); }
    .link { display: flex; align-items: center; gap: 6px; padding: 3px 8px 0 24px; }
    .link-path { flex: 1; min-width: 0; font-family: var(--mono); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .link-state { font-size: 11px; color: var(--muted); }
    .link-state.permission, .link-state.missing { color: var(--warning); }
    .link-actions { display: inline-flex; gap: 4px; }
    .linked .file { padding-left: 36px; }
    .rows { display: flex; flex-direction: column; gap: 8px; max-height: 50vh; overflow: auto; }
    .import-row { display: grid; grid-template-columns: 1fr auto; gap: 2px 8px; align-items: center; font-size: 13px; }
    .import-row .from { grid-column: 1 / -1; color: var(--muted); font-size: 11px; font-family: var(--mono); }
    .import-row .row-note { grid-column: 1 / -1; font-size: 11px; color: var(--muted); }
    .import-row .row-note.bad { color: var(--danger); }
    .import-row .row-note code { font-family: var(--mono); word-break: break-all; }
    .symbol-options { grid-column: 1 / -1; display: flex; gap: 12px; align-items: center; }
    dialog .symbol-options label { flex-direction: row; align-items: center; gap: 4px; margin: 0; font-size: 12px; }
    dialog p.hint { font-size: 12px; color: var(--muted); margin: 0 0 10px; }
    dialog { border: 1px solid var(--border); border-radius: 10px; background: var(--surface); color: var(--fg); padding: 20px; width: min(420px, calc(100vw - 32px)); }
    dialog::backdrop { background: rgb(0 0 0 / 0.45); }
    dialog h2 { margin: 0 0 12px; font-size: 16px; }
    dialog label { display: flex; flex-direction: column; gap: 4px; margin-bottom: 10px; font-size: 13px; }
    dialog .message { margin: 0 0 8px; }
    .dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
  `,
})
export class FileSidebarComponent {
  protected readonly store = inject(JournalStore);
  protected readonly shell = inject(ShellService);
  private readonly library = inject(LibraryService);
  protected readonly links = inject(LinkedContentService);
  protected readonly dropping = signal(false);
  /** Files being imported, each with the name it'll have. */
  protected readonly importRows = signal<ImportRow[]>([]);
  private readonly importDialog = viewChild.required<ElementRef<HTMLDialogElement>>('importDialog');
  /** What's picked to link, waiting for its path. */
  protected readonly picked = signal<PickedLink | null>(null);
  protected readonly linkProblem = signal<string | null>(null);
  private readonly linkDialog = viewChild.required<ElementRef<HTMLDialogElement>>('linkDialog');
  private readonly linkPath = viewChild.required<ElementRef<HTMLInputElement>>('linkPath');
  protected readonly message = signal<string | null>(null);
  private readonly open = open;
  protected readonly nameOf = nameOf;
  protected readonly contentTypes = CONTENT_TYPES;
  protected readonly contentProblem = signal<string | null>(null);
  private readonly newContent = viewChild.required<ElementRef<HTMLDialogElement>>('newContent');
  private readonly contentName = viewChild.required<ElementRef<HTMLInputElement>>('contentName');
  private readonly styleBuilder = viewChild.required(StyleBuilderComponent);
  protected readonly styleKind = KINDS.find((k) => k.kind === 'styles')!;

  protected readonly groups = computed<Group[]>(() => {
    const files = this.store.files();
    const library = this.library.byKind();
    const libraryFiles = this.library.files();
    const label = (path: string) => splitPath(path)!.name;
    const tag = (path: string) => {
      const split = splitPath(path)!;
      return split.kind === 'content' ? contentType(split.ext) : undefined;
    };
    return KINDS.filter((info) => !info.hidden).map((info) => {
      const paths = Object.keys(files).filter((path) => splitPath(path)?.kind === info.kind).sort();
      let own: Item[];
      if (info.kind === 'entries') {
        const listed = this.store.manifest().entries.map(entryPath).filter((path) => path in files);
        own = [
          ...listed.map((path) => ({ path, label: label(path), overrides: false })),
          ...paths.filter((path) => !listed.includes(path)).map((path) => ({ path, label: label(path), overrides: false, unlisted: true })),
        ];
      } else {
        own = paths.map((path) => ({ path, label: label(path), tag: tag(path), overrides: path in libraryFiles }));
      }
      const theirs = (library.get(info.kind) ?? []).filter((path) => !(path in files)).map((path) => ({ path, label: label(path), tag: tag(path), overrides: false }));
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

  /** Edit `path`, its editor's own left tab showing in place of the files. */
  protected openFile(path: string) {
    this.shell.open(path);
    showEditorTab();
  }

  protected setTitle(title: string) {
    this.store.setManifest({ title: title.trim() || 'Untitled' });
  }

  protected setDescription(description: string) {
    this.store.setManifest({ description: description.trim() });
  }

  /** A new file of `info`'s kind, named as the user says; content asks its type too, and a style is built. */
  protected add(info: KindInfo) {
    if (info.kind === 'styles') return this.styleBuilder().open();
    if (info.kind === 'images') return this.addImage(info);
    if (info.kind === 'content') {
      const name = this.contentName().nativeElement;
      name.value = '';
      this.newContent().nativeElement.showModal();
      name.focus();
      return;
    }
    const typed = prompt(`New ${info.label.toLowerCase().replace(/s$/, '')} file's name:`)?.trim();
    if (!typed) return;
    if (!VALID_NAME.test(typed)) return this.say('A name is letters, digits and _ only.');
    this.create(info, typed, info.exts[0]);
  }

  /** A new image: a PNG or JPEG picked from the user's files, named as they say (its file's name, to start with). */
  protected addImage(info: KindInfo) {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'image/png,image/jpeg';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const ext = file.type === 'image/png' ? 'png' : file.type === 'image/jpeg' ? 'jpg' : null;
      if (!ext) return this.say('An image is a PNG or a JPEG.');
      const typed = prompt("New image's name:", journalName(file.name.replace(/\.[^.]*$/, ''), 'image'))?.trim();
      if (!typed) return;
      if (!VALID_NAME.test(typed)) return this.say('A name is letters, digits and _ only.');
      const other = pathOf('images', typed, ext === 'png' ? 'jpg' : 'png');
      if (other in { ...this.library.files(), ...this.store.files() }) return this.say(`There's ${other} already: image("${typed}") would be ambiguous.`);
      this.create(info, typed, ext, bytesToDataUrl(new Uint8Array(await file.arrayBuffer()), ext));
    };
    input.click();
  }

  /** A content file's type, as the list tags it. */
  protected tag(path: string): string {
    return contentType(splitPath(path)?.ext ?? '');
  }

  protected linkedPaths(files: Record<string, string>): string[] {
    return Object.keys(files).sort();
  }

  protected stateLabel(state: LinkState): string {
    return { connected: 'read', permission: 'needs permission', unconnected: 'not connected', missing: "can't be read", unsupported: 'not in this browser' }[state];
  }

  protected stateNote(state: LinkState): string {
    return {
      connected: 'Read from your disk, and again whenever it changes.',
      permission: 'The browser needs your leave to read it again this visit.',
      unconnected: "Not picked in this browser yet: connect it to the file or folder journal.json's path names.",
      missing: "It's gone, or couldn't be read: connect it again.",
      unsupported: "This browser can't read files from your disk: the journal's own content is used. Chrome and Edge can.",
    }[state];
  }

  /** Content from files the user picks on their disk, copied into the journal. */
  protected importContent() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = CONTENT_ACCEPT;
    input.multiple = true;
    input.onchange = () => void this.startImport([...(input.files ?? [])]);
    input.click();
  }

  protected dragOver(event: DragEvent) {
    if (!event.dataTransfer?.types.includes('Files')) return;
    event.preventDefault();
    this.dropping.set(true);
  }

  protected drop(event: DragEvent) {
    this.dropping.set(false);
    const files = [...(event.dataTransfer?.files ?? [])];
    if (!files.length) return;
    event.preventDefault();
    void this.startImport(files);
  }

  /** Read `files` and ask what to call each: content files only. */
  private async startImport(files: File[]) {
    const rows: ImportRow[] = [];
    const skipped: string[] = [];
    for (const file of files) {
      const ext = contentExt(file.name);
      if (ext) rows.push({ file, fromExt: ext, ext, text: await file.text(), name: importedName(file.name), base: 27, strict: true });
      else skipped.push(file.name);
    }
    if (skipped.length) this.say(`${skipped.join(', ')} ${skipped.length === 1 ? "isn't" : "aren't"} content: content is a .md, .txt, .json or .graph file.`);
    if (!rows.length) return;
    this.importRows.set(rows);
    this.importDialog().nativeElement.showModal();
  }

  protected readonly bases = Array.from({ length: 26 }, (_, i) => i + 2);
  protected readonly symbolHelp = symbolHelp;

  protected changeImport(i: number, change: Partial<ImportRow>) {
    this.importRows.update((rows) => rows.map((row, j) => (j === i ? { ...row, ...change } : row)));
  }

  /** `row`'s file as the content it's kept as. */
  protected converted(row: ImportRow): Imported {
    return importedText(row.text, row.fromExt, row);
  }

  protected clip(text: string): string {
    return text.length > 80 ? `${text.slice(0, 80)}…` : text;
  }

  /** What's wrong with importing `row` as it's named (bad), or what it'll do. */
  protected importProblem(row: ImportRow): { text: string; bad: boolean } | null {
    if (!VALID_NAME.test(row.name)) return { text: 'A name is letters, digits and _ only.', bad: true };
    const converted = this.converted(row);
    if ('error' in converted) return { text: converted.error, bad: true };
    const rows = this.importRows();
    if (rows.filter((r) => r.name === row.name).length > 1) return { text: 'Two files are given this name.', bad: true };
    const all = { ...this.library.files(), ...this.store.files() };
    const other = CONTENT_TYPES.find((t) => t.ext !== row.ext && pathOf('content', row.name, t.ext) in all);
    if (other) return { text: `There's ${other.label} content called ${row.name}: content("${row.name}") would be ambiguous.`, bad: true };
    const path = pathOf('content', row.name, row.ext);
    if (path in this.store.files()) return { text: `Replaces the journal's ${row.name}.`, bad: false };
    if (this.library.has(path)) return { text: `Stands in for the library's ${row.name}.`, bad: false };
    return null;
  }

  protected readonly canImport = computed(() => this.importRows().length > 0 && this.importRows().every((row) => !this.importProblem(row)?.bad));

  protected finishImport() {
    if (!this.canImport()) return;
    const rows = this.importRows();
    for (const row of rows) {
      const converted = this.converted(row);
      if (!('error' in converted)) this.store.setFile(pathOf('content', row.name, row.ext), converted.text);
    }
    this.importDialog().nativeElement.close();
    this.open.update((open) => ({ ...open, content: true }));
    this.openFile(pathOf('content', rows[0].name, rows[0].ext));
    this.say(null);
  }

  /** Link a file or folder the user picks on their disk, once they say where journalbuild finds it. */
  protected async link(as: 'file' | 'folder') {
    let picked: PickedLink | null;
    try {
      picked = await this.links.pickNew(as);
    } catch (err) {
      return this.say(`It couldn't be picked: ${(err as Error).message}`);
    }
    if (!picked) return;
    this.picked.set(picked);
    this.linkProblem.set(null);
    this.linkPath().nativeElement.value = `../${picked.name}`;
    this.linkDialog().nativeElement.showModal();
    this.linkPath().nativeElement.select();
  }

  protected async finishLink(path: string) {
    const picked = this.picked();
    if (!picked) return;
    const problem = await this.links.add(picked, path);
    if (problem) return this.linkProblem.set(problem);
    this.linkDialog().nativeElement.close();
    this.open.update((open) => ({ ...open, content: true }));
    const files = Object.keys(this.links.links().find((link) => link.path === path.trim())?.files ?? {}).sort();
    this.say(files.length ? null : `${picked.name} has no content files in it: .txt, .md, .json or .graph, named letters, digits and _.`);
    if (files.length) this.openFile(files[0]);
  }

  protected async connect(path: string) {
    const problem = await this.links.connect(path);
    if (problem) this.say(problem);
  }

  protected async reconnect(path: string) {
    const problem = await this.links.reconnect(path);
    if (problem) this.say(problem);
  }

  /** New content called `name`, of the type kept in files ending `ext`: none of another type may share its name. */
  protected addContent(typed: string, ext: string) {
    const name = typed.trim();
    if (!VALID_NAME.test(name)) return this.contentProblem.set('A name is letters, digits and _ only.');
    const files = { ...this.library.files(), ...this.store.files() };
    const other = CONTENT_TYPES.find((t) => t.ext !== ext && pathOf('content', name, t.ext) in files);
    if (other) return this.contentProblem.set(`There's ${other.label} content called ${name} already: content("${name}") would be ambiguous.`);
    if (pathOf('content', name, ext) in this.store.files()) return this.contentProblem.set(`There's content called ${name} already.`);
    this.newContent().nativeElement.close();
    this.create(KINDS.find((k) => k.kind === 'content')!, name, ext);
  }

  /** Make file `name` of `info`'s kind: `text`, or the kind's template. */
  protected create(info: KindInfo, name: string, ext: string, text?: string) {
    const path = pathOf(info.kind, name, ext);
    if (path in this.store.files()) return this.say(`There's a ${path} already.`);
    if (info.kind === 'entries') {
      const problem = this.store.addEntry(name);
      if (problem) return this.say(problem);
    } else {
      this.store.setFile(path, text ?? template(info.kind, name, ext));
    }
    this.open.update((open) => ({ ...open, [info.kind]: true }));
    this.shell.open(path);
    if (this.library.has(path)) this.say(`${info.kind === 'content' ? `content ${name}` : path} stands in for the library's file of that name.`);
  }

  /** A new style, built starting as a copy of the one at `path`. */
  protected copyStyle(path: string) {
    const problem = this.styleBuilder().copy(nameOf(path));
    if (problem) this.say(problem);
  }

  protected rename(path: string) {
    const split = splitPath(path)!;
    const typed = prompt(`Rename ${split.kind === 'content' ? split.name : path} to:`, split.name)?.trim();
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

