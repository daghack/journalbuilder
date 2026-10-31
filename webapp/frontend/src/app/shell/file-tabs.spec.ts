import { MAX_TABS, keptTabs, lastUsed, renameTab, showTab, type FileTab } from './file-tabs';

function opened(...paths: string[]): FileTab[] {
  return paths.reduce<FileTab[]>((tabs, path) => showTab(tabs, path), []);
}

const paths = (tabs: FileTab[]) => tabs.map((tab) => tab.path);

describe('the file tabs', () => {
  it('give a file one tab, however often it opens', () => {
    expect(paths(opened('a', 'b', 'a', 'b', 'a'))).toEqual(['a', 'b']);
  });

  it('stay where they are when one is shown again', () => {
    expect(paths(opened('a', 'b', 'c', 'a'))).toEqual(['a', 'b', 'c']);
  });

  it('leave the tabs as they are when the file showing shows again', () => {
    const tabs = opened('a', 'b');
    expect(showTab(tabs, 'b')).toBe(tabs);
  });

  it('drop the tab used longest ago, not the first', () => {
    const names = Array.from({ length: MAX_TABS }, (_, i) => `f${i}`);
    expect(paths(opened(...names, 'f0', 'new'))).toEqual([...names.filter((name) => name !== 'f1'), 'new']);
  });

  it('know which file was used last', () => {
    expect(lastUsed(opened('a', 'b', 'c', 'a'))).toBe('a');
    expect(lastUsed([])).toBeNull();
  });

  it('follow a file renamed, in place', () => {
    expect(paths(renameTab(opened('a', 'b', 'c'), 'b', 'x'))).toEqual(['a', 'x', 'c']);
  });

  it('keep one tab when a file is renamed to a path with a tab', () => {
    expect(paths(renameTab(opened('a', 'b', 'c'), 'a', 'c'))).toEqual(['b', 'c']);
  });

  it('read back what was kept, with one tab to a file', () => {
    expect(keptTabs(null)).toEqual([]);
    expect(keptTabs([{ path: 'a', used: 1 }, { path: 3 }, 'b', { path: 'a', used: 2 }])).toEqual([{ path: 'a', used: 2 }]);
  });
});
