import { contentExt, importedName, importedText, linkedFiles, linkedPath, linksAFile, withLinked } from './linked-content';

describe('content from disk', () => {
  it('imports a file as the content type its extension says', () => {
    expect(contentExt('Notes.md')).toBe('md');
    expect(contentExt('notes.MARKDOWN')).toBe('md');
    expect(contentExt('routes.graph')).toBe('graph');
    expect(contentExt('picture.png')).toBeNull();
    expect(importedName('My notes (draft).md')).toBe('My_notes__draft_');
    expect(importedName('.md')).toBe('content');
  });

  it('reads the content files a link gives, as journalbuild does', () => {
    expect(linkedPath('raven.md')).toBe('content/raven.md');
    expect(linkedPath('secret.json')).toBe('content/secret.json');
    expect(linkedPath('raven.markdown')).toBeNull();
    expect(linkedPath('two words.md')).toBeNull();
    expect(linkedPath('.hidden.md')).toBeNull();
    expect(linksAFile('../handouts/letter.md')).toBe(true);
    expect(linksAFile('../notes')).toBe(false);
    expect(linksAFile('../notes.d')).toBe(false);
  });

  it('puts earlier links in front of later ones, and links in front of the journal', () => {
    const linked = linkedFiles([{ 'content/a.md': 'first' }, { 'content/a.md': 'second', 'content/b.txt': 'b' }]);
    expect(linked).toEqual({ 'content/a.md': 'first', 'content/b.txt': 'b' });
    expect(withLinked({ 'content/a.md': 'own', 'locks/x.rhai': '' }, linked)).toEqual({
      'content/a.md': 'first', 'content/b.txt': 'b', 'locks/x.rhai': '',
    });
  });

  it('imports a file as any type of content, symbols strict or made to fit', () => {
    const as = (ext: string, strict = true, base = 27) => ({ ext, base, strict });
    expect(importedText('A *note*\n', 'md', as('md'))).toEqual({ text: 'A *note*\n' });
    expect(importedText('A -> B\n', 'md', as('graph'))).toEqual({ text: 'A -> B\n' });
    expect(importedText('HELLO WORLD\n', 'md', as('json'))).toEqual({
      text: '{ "base": 27, "text": "HELLO WORLD", "strict": true }\n', reads: 'HELLO WORLD', count: 11,
    });
    const strict = importedText('Hello, world!', 'md', as('json'));
    expect('error' in strict && strict.error).toContain("Strict: ',' isn't a base-27 symbol");
    expect(importedText('Hello, world!', 'md', as('json', false))).toEqual({
      text: '{ "base": 27, "text": "Hello, world!" }\n', reads: 'HELLO WORLD', count: 11,
    });
    expect(importedText('!!!', 'txt', as('json', false))).toEqual({ error: "Nothing in it is a base-27 symbol (letters and spaces)." });
    expect(importedText('0110', 'txt', as('json', true, 10))).toEqual({
      text: '{ "base": 10, "text": "0110", "strict": true }\n', reads: '0110', count: 4,
    });
    const file = '{ "base": 27, "symbols": [8, 9] }\n';
    expect(importedText(file, 'json', as('json'))).toEqual({ text: file, reads: 'HI', count: 2, note: 'A symbols file already, in base 27: kept as it is.' });
  });
});
