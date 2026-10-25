import { strToU8, zipSync } from 'fflate';
import { blankJournal, checkManifest, journalHash, manifestText, packZip, snakeCase, splitPath, unpackZip, type JournalData } from './journal-files';

const journal: JournalData = {
  name: 'letters',
  manifest: { version: 1, title: 'Letters', description: 'Two letters.', entries: ['letter_1', 'reply'] },
  files: {
    'entries/letter_1.rhai': 'journal_entry.add(new_page());\n',
    'entries/reply.rhai': 'journal_entry.add(new_page());\n',
    'locks/swap.rhai': '// swap\n',
    'content/note.md': 'a *note*',
  },
};

describe('a journal', () => {
  it('knows its files by path', () => {
    expect(splitPath('content/note.md')).toEqual({ kind: 'content', name: 'note', ext: 'md' });
    expect(splitPath('locks/swap.json')).toBeNull();
    expect(splitPath('journal.json')).toBeNull();
  });

  it('checks its manifest', () => {
    expect(checkManifest({ version: 1, title: 'T', entries: ['a'] })).toEqual({ version: 1, title: 'T', entries: ['a'] });
    expect(() => checkManifest({ version: 1, title: 'T', entries: [] })).toThrow('should list its entries');
    expect(() => checkManifest({ version: 1, title: 'T', entries: ['a/b'] })).toThrow("isn't a name");
    expect(() => checkManifest({ version: 2, title: 'T', entries: ['a'] })).toThrow('version 2');
    expect(() => checkManifest({ version: 1, title: 'T', entries: ['a'], colour: 1 })).toThrow("has 'colour'");
  });

  it('writes its manifest as journalbuild reads it', () => {
    expect(manifestText(journal.manifest)).toBe('{\n  "version": 1,\n  "title": "Letters",\n  "description": "Two letters.",\n  "entries": [\n    "letter_1",\n    "reply"\n  ]\n}\n');
  });

  it('zips up and unzips as it was, dropping the library files it bundled', () => {
    const zip = packZip(journal, { 'locks/huffman.rhai': '// the library\'s', 'locks/swap.rhai': '// not bundled: the journal has its own' });
    const back = unpackZip(zip, 'fallback');
    expect(back).toEqual(journal);
    expect(journalHash(back)).toBe(journalHash(journal));
  });

  it('unzips a journal from a zip of its directory\'s contents, or failing that says why', () => {
    const flat = zipSync({ 'journal.json': strToU8(manifestText({ version: 1, title: 'Flat', entries: ['one'] })), 'entries/one.rhai': strToU8('') });
    expect(unpackZip(flat, 'flat').name).toBe('flat');
    const missing = zipSync({ 'journal.json': strToU8(manifestText({ version: 1, title: 'Flat', entries: ['one'] })) });
    expect(() => unpackZip(missing, 'x')).toThrow("entry 'one' has no file");
    expect(() => unpackZip(zipSync({ 'a.txt': strToU8('') }), 'x')).toThrow('no journal.json');
  });

  it('starts new as one blank entry', () => {
    const blank = blankJournal();
    expect(blank.manifest.entries).toEqual(['entry_1']);
    expect(Object.keys(blank.files)).toEqual(['entries/entry_1.rhai']);
  });

  it("names a journal for its title in snake_case", () => {
    expect(snakeCase('My Journal!', 'x')).toBe('my_journal');
    expect(snakeCase('  The  API reference — v2 ', 'x')).toBe('the_api_reference_v2');
    expect(snakeCase('lockNodeHTMLString', 'x')).toBe('lock_node_html_string');
    expect(snakeCase('already_snake', 'x')).toBe('already_snake');
    expect(snakeCase('!!!', 'untitled')).toBe('untitled');
  });
});
