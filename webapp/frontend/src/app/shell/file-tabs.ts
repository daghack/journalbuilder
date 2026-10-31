/** A file with a tab above the editors. */
export interface FileTab {
  path: string;
  /** When it last showed, counting up: the highest is the file used last. */
  used: number;
}

/** Tabs kept before the one used longest ago drops off. */
export const MAX_TABS = 8;

/** The tabs with `path` showing: its tab where it is, or a new one on the right. A file has one tab at most. */
export function showTab(tabs: FileTab[], path: string, limit = MAX_TABS): FileTab[] {
  const used = Math.max(0, ...tabs.map((tab) => tab.used));
  const tab = tabs.find((t) => t.path === path);
  if (tab) return tab.used === used ? tabs : tabs.map((t) => (t === tab ? { path, used: used + 1 } : t));
  const next = [...tabs, { path, used: used + 1 }];
  if (next.length <= limit) return next;
  const oldest = next.reduce((a, b) => (b.used < a.used ? b : a));
  return next.filter((t) => t !== oldest);
}

/** The tabs with the file at `from` now at `to`: where it was, unless `to` has a tab already. */
export function renameTab(tabs: FileTab[], from: string, to: string): FileTab[] {
  if (tabs.some((t) => t.path === to)) return tabs.filter((t) => t.path !== from);
  return tabs.map((t) => (t.path === from ? { ...t, path: to } : t));
}

/** The file used last, of those with tabs. */
export function lastUsed(tabs: FileTab[]): string | null {
  return tabs.length ? tabs.reduce((a, b) => (b.used > a.used ? b : a)).path : null;
}

/** The tabs as kept from the last visit, dropping anything that isn't one. */
export function keptTabs(kept: unknown): FileTab[] {
  if (!Array.isArray(kept)) return [];
  const tabs = new Map<string, FileTab>();
  for (const tab of kept) {
    if (typeof tab?.path === 'string' && typeof tab?.used === 'number') tabs.set(tab.path, { path: tab.path, used: tab.used });
  }
  return [...tabs.values()].slice(-MAX_TABS);
}
