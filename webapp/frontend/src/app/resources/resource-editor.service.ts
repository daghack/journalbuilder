import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';
import { problemAt, type DocumentProblem, type EditorDocument } from '../editor/editor-document';
import { splitPath, type Kind } from '../journal/journal-files';
import type { LockPorts, LockType, RenderPreview } from '../scribe';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { ToolsService, failed, type Answer } from '../services/tools.service';
import { ShellService } from '../shell/shell.service';
import { fieldsOf, type OptionField } from './options-form.component';

/** How long typing pauses before the preview is drawn again. */
const PREVIEW_DELAY_MS = 300;
/** How wide a text style's preview writes. */
const STYLE_WIDTH = 1000;

/** What a lock file does with the options given: its examples, run, and its ports. */
export interface LockCheck {
  examples: Answer<{ examples: number }>;
  ports: Answer<LockPorts>;
}

/** The kinds this editor edits. */
export type ResourceKind = Extract<Kind, 'locks' | 'illustrations' | 'layouts' | 'styles' | 'lib'>;

/**
 * A lock, illustration, layout, style or helper file being edited: its
 * text (the journal's, or the library's until it's changed, when the
 * journal gets a copy that stands in for it), the options it declares,
 * values for them to preview with, and the preview.
 */
@Injectable()
export class ResourceEditorService {
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);
  private readonly tools = inject(ToolsService);
  private readonly shell = inject(ShellService);

  /** The file edited. */
  readonly path = computed(() => {
    const path = this.shell.selected();
    const kind = path ? splitPath(path)?.kind : null;
    return kind && ['locks', 'illustrations', 'layouts', 'styles', 'lib'].includes(kind) ? path : null;
  });
  readonly kind = computed(() => (splitPath(this.path() ?? '')?.kind ?? null) as ResourceKind | null);
  readonly name = computed(() => splitPath(this.path() ?? '')?.name ?? '');
  /** Whether the journal has the file (else it's the library's). */
  readonly own = computed(() => (this.path() ?? '') in this.store.files());
  /** Whether the library has a file of its path: the journal's stands in for it. */
  readonly builtin = computed(() => this.library.has(this.path() ?? ''));
  readonly text = computed(() => {
    const path = this.path();
    if (!path) return '';
    return this.store.files()[path] ?? this.library.files()[path] ?? '';
  });

  /** What the file declares: its options and description, read by the engine. */
  readonly info = signal<{ description?: string; fields: OptionField[]; error?: string } | null>(null);
  /** Option values given to preview with, by file. */
  private readonly given = signal<Record<string, Record<string, unknown>>>({});
  readonly values = computed(() => this.given()[this.path() ?? ''] ?? {});
  readonly preview = signal<RenderPreview | { error: string } | null>(null);
  readonly lockCheck = signal<LockCheck | null>(null);
  readonly previewing = signal(false);

  /** The problem to mark in the code: the preview's, or the file's. */
  readonly problem = computed<DocumentProblem | null>(() => {
    const preview = this.preview();
    if (preview && 'error' in preview) return problemAt(preview.error);
    const check = this.lockCheck();
    if (check && failed(check.examples)) return problemAt(check.examples.error);
    const info = this.info();
    return info?.error ? problemAt(info.error) : null;
  });

  /** The code, as the script editor edits it. */
  readonly document: EditorDocument = {
    source: this.text,
    setSource: (source: string) => this.setSource(source),
    error: this.problem,
    warnings: computed(() => []),
  };

  private timer: ReturnType<typeof setTimeout> | null = null;
  private asked = 0;

  constructor() {
    effect(() => {
      const path = this.path();
      const kind = this.kind();
      const text = this.text();
      const values = this.values();
      const resources = this.store.resources();
      if (this.timer) clearTimeout(this.timer);
      if (!path || !kind || kind === 'lib') {
        untracked(() => {
          this.info.set(null);
          this.preview.set(null);
          this.lockCheck.set(null);
        });
        return;
      }
      this.timer = setTimeout(() => void this.refresh(path, kind, text, values, resources), PREVIEW_DELAY_MS);
    });
  }

  /** The file changed: the journal's copy is changed, made if it hasn't one. */
  setSource(source: string) {
    const path = this.path();
    if (!path || source === this.text()) return;
    this.store.setFile(path, source);
  }

  /** Give option `name` a value to preview with, or its default again (undefined). */
  setValue(name: string, value: unknown) {
    const path = this.path();
    if (!path) return;
    this.given.update((given) => {
      const values = { ...(given[path] ?? {}) };
      if (value === undefined) delete values[name];
      else values[name] = value;
      return { ...given, [path]: values };
    });
  }

  /** Copy the library's file into the journal, to change. */
  copyIn() {
    const path = this.path();
    if (path && !this.own()) this.store.setFile(path, this.text());
  }

  /** Remove the journal's copy: the library's is used again. */
  revertToLibrary() {
    const path = this.path();
    if (path && this.own() && this.builtin()) this.store.removeFile(path);
  }

  private async refresh(path: string, kind: ResourceKind, text: string, values: Record<string, unknown>, _resources: unknown) {
    const asked = ++this.asked;
    const name = splitPath(path)!.name;
    this.previewing.set(true);
    try {
      if (kind === 'locks') {
        const types = await this.tools.lockTypes();
        const type: LockType | undefined = failed(types) ? undefined : types.find((t) => t.name === name);
        const fields: OptionField[] = (type?.options ?? []).map((o) => ({ name: o.name, kind: o.kind, choices: o.choices, default: o.default }));
        const options = { ...Object.fromEntries(fields.filter((f) => f.default !== undefined).map((f) => [f.name, f.default])), ...values };
        const [examples, ports] = await Promise.all([
          this.tools.lockExamples(name, text).catch((err: Error) => ({ error: err.message })),
          this.tools.lockPorts(name, options).catch((err: Error) => ({ error: err.message })),
        ]);
        if (asked !== this.asked) return;
        this.info.set({ description: type?.description, fields, error: failed(types) ? types.error : type?.error });
        this.lockCheck.set({ examples, ports });
        this.preview.set(null);
        return;
      }
      const list = kind === 'illustrations' ? await this.tools.illustrationTypes()
        : kind === 'layouts' ? await this.tools.layoutTypes() : await this.tools.styleTypes();
      const type = list.find((t) => t.name === name);
      const preview = kind === 'illustrations' ? await this.tools.illustrationPreview(name, text, values, null)
        : kind === 'layouts' ? await this.tools.layoutPreview(name, text, values)
        : await this.tools.stylePreview(name, text, values, STYLE_WIDTH);
      if (asked !== this.asked) return;
      this.info.set({ description: type?.description, fields: fieldsOf(type?.options), error: type?.error });
      this.preview.set(preview);
      this.lockCheck.set(null);
    } catch (err) {
      if (asked === this.asked) this.preview.set({ error: (err as Error).message });
    } finally {
      if (asked === this.asked) this.previewing.set(false);
    }
  }
}
