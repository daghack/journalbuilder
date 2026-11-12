// A journal as the editor holds it: its manifest and its files by path,
// as a journal's directory has them (see the repository's
// src/journal/rhaientries/manifest.rs and resources.rs), and zips of it.
// Pictures (images/NAME.png or .jpg) are held as data URLs, as the
// renderer reads them, and are bytes again in a zip or on disk.

import { strFromU8, strToU8, unzipSync, zipSync, type Zippable } from 'fflate';

/** The kinds of file a journal has, each in a directory of its own. */
export type Kind = 'entries' | 'chains' | 'locks' | 'helpers' | 'illustrations' | 'layouts' | 'styles' | 'content' | 'glyphsets' | 'lib' | 'images' | 'edits';

/** The kinds the resource editor edits. */
export const RESOURCE_KINDS: readonly Kind[] = ['locks', 'helpers', 'illustrations', 'layouts', 'lib'];

export interface KindInfo {
  kind: Kind;
  /** What the sidebar calls them. */
  label: string;
  /** The extensions its files may have; the first is a new file's. */
  exts: string[];
  /** Whether the library has files of this kind for a journal's to stand in for. */
  library: boolean;
  /** Kept by an editor and not listed: the visual editor's edits, an entry's each. */
  hidden?: boolean;
}

/** Every kind, in the order the sidebar lists them. */
export const KINDS: KindInfo[] = [
  { kind: 'entries', label: 'Entries', exts: ['rhai'], library: false },
  { kind: 'chains', label: 'Chains', exts: ['rhai'], library: true },
  { kind: 'locks', label: 'Locks', exts: ['rhai'], library: true },
  { kind: 'helpers', label: 'Helpers', exts: ['rhai'], library: true },
  { kind: 'illustrations', label: 'Illustrations', exts: ['rhai'], library: true },
  { kind: 'layouts', label: 'Layouts', exts: ['rhai'], library: true },
  { kind: 'styles', label: 'Styles', exts: ['json'], library: true },
  { kind: 'content', label: 'Content', exts: ['txt', 'md', 'json', 'graph'], library: true },
  { kind: 'glyphsets', label: 'Glyphsets', exts: ['json'], library: true },
  { kind: 'lib', label: 'Lib', exts: ['rhai'], library: true },
  { kind: 'images', label: 'Images', exts: ['png', 'jpg'], library: true },
  { kind: 'edits', label: 'Edits', exts: ['json'], library: false, hidden: true },
];

export const KIND_INFO = new Map(KINDS.map((k) => [k.kind, k]));

/** A style as a script, as they were: kept until it's converted (see StyleConversionService). */
export const OLD_STYLE = /^styles\/([A-Za-z0-9_]+)\.rhai$/;

/** The types of content, each kept in files of its own extension, which nobody types. */
export const CONTENT_TYPES = [
  { ext: 'txt', label: 'String', about: 'plain text, read as a string' },
  { ext: 'md', label: 'Markdown', about: 'marked-up text: markdown(content(name)) styles it' },
  { ext: 'json', label: 'Symbols', about: 'text read as symbols in a base, for a lock' },
  { ext: 'graph', label: 'Graph', about: 'places and the ways between them, for graph_map' },
] as const;

/** What a content file's extension makes it: String, Markdown, Symbols or Graph. */
export function contentType(ext: string): string {
  return CONTENT_TYPES.find((t) => t.ext === ext)?.label ?? ext;
}

/** The media types of pictures, by extension. */
export const IMAGE_TYPES: Record<string, string> = { png: 'image/png', jpg: 'image/jpeg' };

/** Whether `path` is a picture's, held as a data URL. */
export function isImage(path: string): boolean {
  return splitPath(path)?.kind === 'images';
}

/** A picture's bytes as a data URL, of the type its extension `ext` says. */
export function bytesToDataUrl(bytes: Uint8Array, ext: string): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return `data:${IMAGE_TYPES[ext] ?? 'application/octet-stream'};base64,${btoa(binary)}`;
}

/** The bytes a data URL (or plain base64) holds. */
export function dataUrlToBytes(url: string): Uint8Array {
  const binary = atob(url.slice(url.indexOf(',') + 1));
  return Uint8Array.from(binary, (c) => c.charCodeAt(0));
}

/** A file's text as a journal holds it: a picture's bytes as a data URL, other files' as text. */
export function fileText(path: string, bytes: Uint8Array): string {
  return isImage(path) ? bytesToDataUrl(bytes, splitPath(path)!.ext) : strFromU8(bytes);
}

/** A file's bytes, as a zip or a directory has them. */
export function fileBytes(path: string, text: string): Uint8Array {
  return isImage(path) ? dataUrlToBytes(text) : strToU8(text);
}

/** A journal's `journal.json`. */
export interface Manifest {
  version: number;
  title: string;
  description?: string;
  /** The entries, in order: entries/NAME.rhai each. */
  entries: string[];
  /** Library files an export copied in, as paths: an import drops them. */
  bundled?: string[];
}

/** A journal: its name (its directory's), manifest, and files by path, journal.json aside. */
export interface JournalData {
  name: string;
  manifest: Manifest;
  files: Record<string, string>;
}

export const MANIFEST = 'journal.json';
export const MANIFEST_VERSION = 1;

/** What names an entry or another file: letters, digits and _. */
export const VALID_NAME = /^[A-Za-z0-9_]+$/;

/** A file's path, split: its kind, name and extension; null for one in no kind's directory. */
export function splitPath(path: string): { kind: Kind; name: string; ext: string } | null {
  const match = /^([a-z]+)\/(.+)\.([a-z]+)$/.exec(path);
  if (!match) return null;
  const info = KIND_INFO.get(match[1] as Kind);
  if (!info || !info.exts.includes(match[3])) return null;
  return { kind: info.kind, name: match[2], ext: match[3] };
}

export function kindOf(path: string): Kind | null {
  return splitPath(path)?.kind ?? null;
}

/** A file's name: its path without directory or extension. */
export function nameOf(path: string): string {
  return splitPath(path)?.name ?? path;
}

export function pathOf(kind: Kind, name: string, ext = KIND_INFO.get(kind)!.exts[0]): string {
  return `${kind}/${name}.${ext}`;
}

/** The entry `name`'s path. */
export function entryPath(name: string): string {
  return pathOf('entries', name);
}

/** The visual editor's edits to entry `name`'s canvases: edits/NAME.json. */
export function editsPath(name: string): string {
  return pathOf('edits', name);
}

/** `value` as a manifest, or what's wrong with it. */
export function checkManifest(value: unknown): Manifest {
  if (!value || typeof value !== 'object') throw new Error('journal.json should be a JSON object');
  const m = value as Record<string, unknown>;
  const known = ['version', 'title', 'description', 'entries', 'bundled'];
  const unknown = Object.keys(m).find((key) => !known.includes(key));
  if (unknown) throw new Error(`journal.json has '${unknown}', which a manifest doesn't: ${known.join(', ')}`);
  if (m['version'] !== MANIFEST_VERSION) throw new Error(`journal.json is version ${String(m['version'])}; this editor reads ${MANIFEST_VERSION}`);
  if (typeof m['title'] !== 'string') throw new Error('journal.json needs a title');
  if (m['description'] !== undefined && typeof m['description'] !== 'string') throw new Error("journal.json's description should be a string");
  const entries = m['entries'];
  if (!Array.isArray(entries) || !entries.length) throw new Error('journal.json should list its entries');
  for (const [i, entry] of entries.entries()) {
    if (typeof entry !== 'string' || !VALID_NAME.test(entry)) throw new Error(`entry ${JSON.stringify(entry)} isn't a name: letters, digits and _ only`);
    if (entries.indexOf(entry) !== i) throw new Error(`entry '${entry}' is listed twice`);
  }
  const bundled = m['bundled'];
  if (bundled !== undefined && (!Array.isArray(bundled) || bundled.some((p) => typeof p !== 'string'))) throw new Error("journal.json's bundled should list paths");
  return {
    version: MANIFEST_VERSION,
    title: m['title'],
    ...(typeof m['description'] === 'string' && m['description'] ? { description: m['description'] } : {}),
    entries: entries as string[],
    ...(bundled ? { bundled: bundled as string[] } : {}),
  };
}

/** The manifest as journal.json holds it, as journalbuild writes it. */
export function manifestText(manifest: Manifest): string {
  const ordered: Manifest = {
    version: manifest.version,
    title: manifest.title,
    ...(manifest.description ? { description: manifest.description } : {}),
    entries: manifest.entries,
    ...(manifest.bundled?.length ? { bundled: manifest.bundled } : {}),
  };
  return JSON.stringify(ordered, null, 2) + '\n';
}

/** Fails unless `files` has a file for every entry the manifest lists. */
export function checkEntries(manifest: Manifest, files: Record<string, string>): void {
  for (const entry of manifest.entries) {
    if (!(entryPath(entry) in files)) throw new Error(`entry '${entry}' has no file, entries/${entry}.rhai`);
  }
}

/**
 * `journal` as a zip: a directory named for it holding journal.json and its
 * files, with `bundled` (library files, by path) beside its own, listed in
 * the manifest so an import can drop them.
 */
export function packZip(journal: JournalData, bundled: Record<string, string> = {}): Uint8Array {
  const extra = Object.keys(bundled).filter((path) => !(path in journal.files)).sort();
  const manifest: Manifest = { ...journal.manifest, ...(extra.length ? { bundled: extra } : {}) };
  if (!extra.length) delete manifest.bundled;
  const tree: Zippable = { [`${journal.name}/${MANIFEST}`]: strToU8(manifestText(manifest)) };
  for (const [path, text] of Object.entries(journal.files)) tree[`${journal.name}/${path}`] = fileBytes(path, text);
  for (const path of extra) tree[`${journal.name}/${path}`] = fileBytes(path, bundled[path]);
  return zipSync(tree, { level: 6 });
}

/**
 * The journal in a directory's files: `manifest`, its journal.json's text,
 * and the rest by path from the directory. Library files the manifest lists
 * as bundled are dropped, so the journal uses the library's own, as are
 * files in no kind's directory (old style scripts aside, to be converted).
 */
export function readJournal(name: string, manifest: string, files: Record<string, string>): JournalData {
  let manifestValue: unknown;
  try {
    manifestValue = JSON.parse(manifest);
  } catch (err) {
    throw new Error(`journal.json isn't JSON: ${(err as Error).message}`);
  }
  const checked = checkManifest(manifestValue);
  const bundled = new Set(checked.bundled ?? []);
  delete checked.bundled;
  const own = Object.fromEntries(Object.entries(files).filter(([path]) => !bundled.has(path) && (splitPath(path) || OLD_STYLE.test(path))));
  checkEntries(checked, own);
  return { name, manifest: checked, files: own };
}

/** `name` made a journal's name, all but letters, digits and _ made _; `fallback` if it's empty. */
export function journalName(name: string, fallback: string): string {
  return name.replace(/[^A-Za-z0-9_]/g, '_') || fallback;
}

/** `text` (a title, say) in snake_case, made a journal's name: "My Journal!" is my_journal; `fallback` if nothing's left. */
export function snakeCase(text: string, fallback: string): string {
  const words = text
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2')
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean);
  return words.join('_').toLowerCase() || fallback;
}

/** The journal in a zip: one holding a journal's directory (or the directory's contents). */
export function unpackZip(bytes: Uint8Array, fallbackName: string): JournalData {
  const entries = unzipSync(bytes, { filter: (file) => !file.name.endsWith('/') });
  const paths = Object.keys(entries).filter((path) => !/(^|\/)(__MACOSX|\.DS_Store)/.test(path));
  const manifests = paths.filter((path) => path === MANIFEST || path.endsWith(`/${MANIFEST}`));
  if (!manifests.length) throw new Error('the zip has no journal.json: is it a journal?');
  const manifestPath = manifests.sort((a, b) => a.split('/').length - b.split('/').length)[0];
  const root = manifestPath.slice(0, -MANIFEST.length);
  const files: Record<string, string> = {};
  for (const path of paths) {
    if (!path.startsWith(root) || path === manifestPath || !(splitPath(path.slice(root.length)) || OLD_STYLE.test(path.slice(root.length)))) continue;
    files[path.slice(root.length)] = fileText(path.slice(root.length), entries[path]);
  }
  const name = root.replace(/\/$/, '').split('/').pop() || fallbackName;
  return readJournal(VALID_NAME.test(name) ? name : fallbackName, strFromU8(entries[manifestPath]), files);
}

/** FNV-1a of `text`, to tell versions apart. */
export function hash(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(36) + text.length.toString(36);
}

/** A hash of the whole journal, to tell whether it's changed since it was saved. */
export function journalHash(journal: JournalData): string {
  const paths = Object.keys(journal.files).sort();
  return hash([journal.name, manifestText(journal.manifest), ...paths.map((p) => `${p}\n${journal.files[p]}`)].join('\u0000'));
}

/** A new journal's one entry. */
export const BLANK_ENTRY = 'let page = new_page();\njournal_entry.add(page);\n';

/** A new journal: one blank entry, and nothing else of its own. */
export function blankJournal(): JournalData {
  return {
    name: 'untitled',
    manifest: { version: MANIFEST_VERSION, title: 'Untitled', entries: ['entry_1'] },
    files: { [entryPath('entry_1')]: BLANK_ENTRY },
  };
}
