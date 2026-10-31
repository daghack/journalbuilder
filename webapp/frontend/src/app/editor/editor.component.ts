import { Component, DestroyRef, ElementRef, effect, inject, untracked, viewChild, type OnDestroy } from '@angular/core';
import { autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap } from '@codemirror/autocomplete';
import { defaultKeymap, history, historyField, historyKeymap, indentWithTab } from '@codemirror/commands';
import { HighlightStyle, bracketMatching, indentOnInput, syntaxHighlighting } from '@codemirror/language';
import { lintGutter, setDiagnostics, type Diagnostic } from '@codemirror/lint';
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search';
import { Compartment, EditorState, type StateEffect, type Text } from '@codemirror/state';
import {
  EditorView, crosshairCursor, drawSelection, dropCursor, highlightActiveLine, highlightActiveLineGutter,
  highlightSpecialChars, keymap, lineNumbers, rectangularSelection,
} from '@codemirror/view';
import { tags as t } from '@lezer/highlight';
import { EDITOR_DOCUMENT } from './editor-document';
import { JournalStore } from '../services/journal-store.service';
import { RenderService } from '../services/render.service';
import { rhai, rhaiCompletions } from './rhai-language';

/** What's kept of an editor's state: its text, cursor and undo history. */
const KEPT_FIELDS = { history: historyField };

/** Each file's editor as it was left, while another shows, for any editor of it (a file's kind can change editors). */
const left = new Map<string, { state: { doc: string }; scroll: StateEffect<unknown> }>();

/** Colours come from the page's CSS tokens, so the editor follows light and dark. */
const highlight = HighlightStyle.define([
  { tag: t.comment, color: 'var(--syntax-comment)', fontStyle: 'italic' },
  { tag: t.keyword, color: 'var(--syntax-keyword)', fontWeight: '600' },
  { tag: [t.string, t.special(t.brace)], color: 'var(--syntax-string)' },
  { tag: [t.number, t.atom, t.self], color: 'var(--syntax-number)' },
  { tag: t.function(t.standard(t.variableName)), color: 'var(--syntax-api)', fontWeight: '600' },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], color: 'var(--syntax-call)' },
  { tag: t.function(t.definition(t.variableName)), color: 'var(--syntax-def)', fontWeight: '600' },
  { tag: t.propertyName, color: 'var(--syntax-property)' },
  { tag: t.namespace, color: 'var(--syntax-namespace)' },
  { tag: t.constant(t.variableName), color: 'var(--syntax-constant)' },
  { tag: [t.operator, t.punctuation, t.bracket], color: 'var(--syntax-punctuation)' },
]);

const theme = EditorView.theme({
  '&': { height: '100%', backgroundColor: 'var(--editor-bg)', color: 'var(--editor-fg)', fontSize: '13px' },
  '.cm-scroller': { fontFamily: 'var(--mono)', lineHeight: '1.55' },
  '.cm-content': { caretColor: 'var(--editor-fg)' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--editor-fg)' },
  '.cm-gutters': { backgroundColor: 'var(--editor-gutter-bg)', color: 'var(--muted)', border: 'none' },
  '.cm-activeLine': { backgroundColor: 'var(--editor-active-line)' },
  '.cm-activeLineGutter': { backgroundColor: 'var(--editor-active-line)' },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection': { backgroundColor: 'var(--editor-selection) !important' },
  '.cm-tooltip': { backgroundColor: 'var(--surface)', color: 'var(--fg)', border: '1px solid var(--border)' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: 'var(--accent)', color: 'var(--accent-fg)' },
  '.cm-panels': { backgroundColor: 'var(--surface)', color: 'var(--fg)' },
});

/** The script editor: rhai with highlighting, completion and the last run's errors marked. */
@Component({
  selector: 'app-editor',
  template: `<div class="host" #host></div>`,
  styles: `
    :host { display: block; min-height: 0; overflow: hidden; }
    .host { height: 100%; }
  `,
})
export class EditorComponent implements OnDestroy {
  private readonly doc = inject(EDITOR_DOCUMENT);
  private readonly render = inject(RenderService);
  private readonly store = inject(JournalStore);
  private readonly host = viewChild.required<ElementRef<HTMLElement>>('host');
  private view: EditorView | null = null;
  /** The file the editor has. */
  private shown: string | null = null;
  private readonly language = new Compartment();
  private builtins: ReadonlySet<string> = new Set();
  private readonly extensions = [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightSpecialChars(),
    history(),
    drawSelection(),
    dropCursor(),
    EditorState.allowMultipleSelections.of(true),
    indentOnInput(),
    bracketMatching(),
    closeBrackets(),
    autocompletion({ override: [rhaiCompletions(() => this.render.apiNames())] }),
    rectangularSelection(),
    crosshairCursor(),
    highlightActiveLine(),
    highlightSelectionMatches(),
    lintGutter(),
    keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...searchKeymap, ...historyKeymap, ...completionKeymap, indentWithTab]),
    this.language.of(rhai(() => this.builtins)),
    syntaxHighlighting(highlight),
    theme,
    EditorView.updateListener.of((update) => {
      if (update.docChanged) this.doc.setSource(update.state.doc.toString());
    }),
  ];

  constructor() {
    // Create the editor once its element is there.
    effect(() => {
      const host = this.host().nativeElement;
      untracked(() => this.create(host));
    });
    // Another journal: what's kept of the last one's files isn't undone into it.
    const loads = this.store.loads();
    effect(() => {
      if (this.store.loads() === loads) return;
      untracked(() => {
        left.clear();
        this.shown = null;
        if (this.view) this.show(this.doc.path(), this.doc.source());
      });
    });
    // Another file: the editor as it was left on it. A new text for this
    // file (changed elsewhere) replaces the editor's.
    effect(() => {
      const path = this.doc.path();
      const source = this.doc.source();
      const view = this.view;
      if (!view) return;
      if (path !== this.shown) {
        untracked(() => this.show(path, source));
      } else if (source !== view.state.doc.toString()) {
        view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: source }, selection: { anchor: 0 } });
        view.scrollDOM.scrollTo({ top: 0, left: 0 });
      }
    });
    // A file renamed is the same file.
    inject(DestroyRef).onDestroy(this.store.onRename((from, to) => {
      const kept = left.get(from);
      if (kept) left.set(to, kept);
      left.delete(from);
      if (this.shown === from) this.shown = to;
    }));
    // The API's names, once the renderer loads, are highlighted.
    effect(() => {
      this.builtins = new Set(this.render.apiNames());
      this.view?.dispatch({ effects: this.language.reconfigure(rhai(() => this.builtins)) });
    });
    // Mark the last run's error and warnings.
    effect(() => this.mark());
  }

  private mark() {
    const error = this.doc.error();
    const warnings = this.doc.warnings();
    const view = this.view;
    if (!view) return;
    const doc = view.state.doc;
    const diagnostics: Diagnostic[] = [];
    if (error?.line) diagnostics.push(at(doc, error.line, error.column ?? null, 'error', error.message));
    for (const warning of warnings) {
      const line = warning.match(/\(line (\d+)\)$/);
      if (line) diagnostics.push(at(doc, Number(line[1]), null, 'warning', warning));
    }
    view.dispatch(setDiagnostics(view.state, diagnostics));
  }

  private create(parent: HTMLElement) {
    if (this.view) return;
    this.view = new EditorView({ state: EditorState.create({ extensions: this.extensions }), parent });
    this.show(this.doc.path(), this.doc.source());
  }

  /** Have file `path`, its text `source`: as the editor was left on it, unless its text has changed since. */
  private show(path: string | null, source: string) {
    const view = this.view!;
    this.leave();
    this.shown = path;
    const kept = path === null ? undefined : left.get(path);
    if (kept?.state.doc === source) {
      view.setState(EditorState.fromJSON(kept.state, { extensions: this.extensions }, KEPT_FIELDS));
      view.dispatch({ effects: kept.scroll });
    } else {
      view.setState(EditorState.create({ doc: source, extensions: this.extensions }));
    }
    this.mark();
  }

  /** Keep the editor as it is on the file it has. */
  private leave() {
    if (this.view && this.shown !== null) left.set(this.shown, { state: this.view.state.toJSON(KEPT_FIELDS), scroll: this.view.scrollSnapshot() });
  }

  /** Put the cursor on `line` and show it. */
  goTo(line: number) {
    const view = this.view;
    if (!view) return;
    const pos = view.state.doc.line(Math.min(Math.max(line, 1), view.state.doc.lines)).from;
    view.dispatch({ selection: { anchor: pos }, scrollIntoView: true });
    view.focus();
  }

  ngOnDestroy() {
    this.leave();
    this.view?.destroy();
  }
}

/** A diagnostic on `line`: from `column` (1-based) to the end of its word, or the whole line. */
function at(doc: Text, lineNumber: number, column: number | null, severity: Diagnostic['severity'], message: string): Diagnostic {
  const line = doc.line(Math.min(Math.max(lineNumber, 1), doc.lines));
  if (!column) return { from: line.from, to: line.to, severity, message };
  const from = Math.min(line.from + column - 1, line.to);
  const word = line.text.slice(from - line.from).match(/^[\w$]+|^./);
  return { from, to: Math.min(from + (word?.[0].length ?? 1), line.to), severity, message };
}
