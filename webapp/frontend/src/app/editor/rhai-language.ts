// Rhai for CodeMirror: a stream tokenizer for highlighting, and completion
// of keywords and the functions journals add to rhai.

import { type CompletionContext, type CompletionResult, type Completion } from '@codemirror/autocomplete';
import { StreamLanguage, type StreamParser, type StringStream } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';

export const KEYWORDS = [
  'let', 'const', 'fn', 'if', 'else', 'switch', 'do', 'while', 'loop', 'until', 'for', 'in',
  'continue', 'break', 'return', 'throw', 'try', 'catch', 'import', 'export', 'as', 'private',
  'global', 'is', 'Fn', 'call', 'curry', 'type_of', 'print', 'debug', 'eval',
];
const KEYWORD_SET = new Set(KEYWORDS);
const ATOMS = new Set(['true', 'false', '()']);

/** Where the tokenizer is: in code (with how deep in `${ }` of which strings), a block comment, or a string. */
export interface RhaiState {
  /** Nesting depth of the block comment it's in; 0 if none. */
  comment: number;
  /** The string it's in: '`' or '"'; null in code. */
  string: '`' | '"' | null;
  /** For each `${` it's in, the brace depth inside it. */
  interpolations: number[];
  /** The last thing read, to tell `fn name` and `.method` apart. */
  last: 'fn' | 'dot' | 'other';
}

/** A stream parser for rhai; `builtins` are highlighted as the journal API. */
export function rhaiParser(builtins: () => ReadonlySet<string>): StreamParser<RhaiState> {
  return {
    name: 'rhai',
    startState: () => ({ comment: 0, string: null, interpolations: [], last: 'other' }),
    copyState: (s) => ({ ...s, interpolations: [...s.interpolations] }),
    languageData: {
      commentTokens: { line: '//', block: { open: '/*', close: '*/' } },
      closeBrackets: { brackets: ['(', '[', '{', '"', '`'] },
      indentOnInput: /^\s*[}\]]$/,
    },
    tokenTable: {
      api: t.function(t.standard(t.variableName)),
      call: t.function(t.variableName),
      def: t.function(t.definition(t.variableName)),
      method: t.function(t.propertyName),
      property: t.propertyName,
      namespace: t.namespace,
      interpolation: t.special(t.brace),
      mapOpen: t.special(t.brace),
      self: t.self,
    },
    token(stream, state) {
      if (state.comment > 0) return blockComment(stream, state);
      if (state.string) return stringBody(stream, state);
      if (stream.eatSpace()) return null;
      const token = code(stream, state, builtins());
      if (token !== null || !stream.current().match(/^\s*$/)) {
        state.last = token === 'keyword' && stream.current() === 'fn' ? 'fn' : stream.current() === '.' || stream.current() === '?.' ? 'dot' : 'other';
      }
      return token;
    },
  };
}

function blockComment(stream: StringStream, state: RhaiState): string {
  while (!stream.eol()) {
    if (stream.match('/*')) state.comment++;
    else if (stream.match('*/')) {
      state.comment--;
      if (state.comment === 0) break;
    } else stream.next();
  }
  return 'comment';
}

/** Inside a string: to its end, or (in a backtick string) to a `${`. */
function stringBody(stream: StringStream, state: RhaiState): string {
  const quote = state.string!;
  if (quote === '`' && stream.match('${')) {
    state.string = null;
    state.interpolations.push(0);
    return 'interpolation';
  }
  while (!stream.eol()) {
    const ch = stream.next();
    if (ch === '\\') {
      stream.next();
    } else if (ch === quote) {
      state.string = null;
      break;
    } else if (quote === '`' && ch === '$' && stream.peek() === '{') {
      stream.backUp(1);
      break;
    }
  }
  // A "..." string ends at the line's end unless the line ends with a backslash.
  if (quote === '"' && stream.eol() && state.string && !stream.current().endsWith('\\')) state.string = null;
  return 'string';
}

function code(stream: StringStream, state: RhaiState, builtins: ReadonlySet<string>): string | null {
  if (stream.match('//')) {
    stream.skipToEnd();
    return 'comment';
  }
  if (stream.match('/*')) {
    state.comment = 1;
    return blockComment(stream, state);
  }
  const ch = stream.peek()!;
  if (ch === '"' || ch === '`') {
    stream.next();
    state.string = ch;
    return stringBody(stream, state);
  }
  if (ch === "'") {
    stream.next();
    if (stream.next() === '\\') stream.next();
    stream.eat("'");
    return 'string';
  }
  if (stream.match(/^0x[0-9a-f_]+/i) || stream.match(/^0o[0-7_]+/i) || stream.match(/^0b[01_]+/i)
      || stream.match(/^\d[\d_]*(\.\d[\d_]*)?(e[+-]?\d+)?/i)) {
    return 'number';
  }
  if (stream.match('#{')) {
    // Its `}` mustn't end an interpolation it's in.
    if (state.interpolations.length) state.interpolations[state.interpolations.length - 1]++;
    return 'mapOpen';
  }
  if (ch === '{' && state.interpolations.length) {
    stream.next();
    state.interpolations[state.interpolations.length - 1]++;
    return 'brace';
  }
  if (ch === '}' && state.interpolations.length) {
    stream.next();
    const depth = state.interpolations.length - 1;
    if (state.interpolations[depth] === 0) {
      state.interpolations.pop();
      state.string = '`';
      return 'interpolation';
    }
    state.interpolations[depth]--;
    return 'brace';
  }
  const word = stream.match(/^[A-Za-z_][A-Za-z0-9_]*/) as RegExpMatchArray | null;
  if (word) {
    const name = word[0];
    if (stream.match('::', false)) return 'namespace';
    if (name === 'this') return 'self';
    if (ATOMS.has(name)) return 'atom';
    if (KEYWORD_SET.has(name)) return 'keyword';
    if (state.last === 'fn') return 'def';
    const called = stream.match(/^\s*\(/, false);
    const namespaced = stream.string.slice(0, stream.start).match(/([A-Za-z_]\w*)::$/);
    const full = namespaced ? `${namespaced[1]}::${name}` : name;
    if (state.last === 'dot') return called ? 'method' : 'property';
    if (called) return builtins.has(full) ? 'api' : 'call';
    if (/^[A-Z][A-Z0-9_]*$/.test(name)) return 'variableName.constant';
    return 'variableName';
  }
  if (stream.match(/^(\?\.|::|\.\.=?|=>|->|&&|\|\||\*\*|<<|>>|[-+*\/%&|^!<>=]=?)/)) return 'operator';
  if (stream.match(/^[.,;:?]/)) return 'punctuation';
  if (stream.match(/^[()[\]{}]/)) return 'bracket';
  stream.next();
  return null;
}

/** Rhai, with `builtins()` highlighted as the journal API. */
export function rhai(builtins: () => ReadonlySet<string>) {
  return StreamLanguage.define(rhaiParser(builtins));
}

/** Completes keywords, the journal API and names used in the script. */
export function rhaiCompletions(builtins: () => readonly string[]) {
  return (context: CompletionContext): CompletionResult | null => {
    const word = context.matchBefore(/[A-Za-z_][\w]*(::[A-Za-z_\w]*)?/);
    if (!word || (word.from === word.to && !context.explicit)) return null;
    const seen = new Set<string>();
    const options: Completion[] = [];
    const add = (label: string, type: string, boost = 0) => {
      if (seen.has(label)) return;
      seen.add(label);
      options.push({ label, type, boost });
    };
    for (const name of builtins()) add(name, 'function', 1);
    for (const keyword of KEYWORDS) add(keyword, 'keyword', 2);
    // Names the script itself defines.
    const text = context.state.doc.toString();
    for (const match of text.matchAll(/\b(?:let|const|fn)\s+([A-Za-z_]\w*)/g)) {
      add(match[1], match[0].startsWith('fn') ? 'function' : 'variable', 3);
    }
    return { from: word.from, options, validFor: /^[\w:]*$/ };
  };
}
