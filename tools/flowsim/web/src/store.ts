// This browser's storage, which may be missing or full: every call is
// guarded, and the page works without it.

export function load<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? null : (JSON.parse(raw) as T);
  } catch {
    return null;
  }
}

export function loadText(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** Gives false if it couldn't be kept (no storage, or too big). */
export function save(key: string, value: unknown): boolean {
  return saveText(key, JSON.stringify(value));
}

export function saveText(key: string, text: string): boolean {
  try {
    localStorage.setItem(key, text);
    return true;
  } catch {
    return false;
  }
}

export function remove(key: string): void {
  try {
    localStorage.removeItem(key);
  } catch {
    // Nothing to do.
  }
}
