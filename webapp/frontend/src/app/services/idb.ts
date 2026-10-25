// A little key-value store in IndexedDB, for what's too big for
// localStorage: the journal being edited.

const DB = 'journal-editor';
const STORE = 'kv';

let opened: Promise<IDBDatabase> | null = null;

function db(): Promise<IDBDatabase> {
  opened ??= new Promise((resolve, reject) => {
    const request = indexedDB.open(DB, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB didn't open"));
  });
  return opened;
}

function run<T>(mode: IDBTransactionMode, act: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  return db().then((db) => new Promise<T>((resolve, reject) => {
    const request = act(db.transaction(STORE, mode).objectStore(STORE));
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('IndexedDB failed'));
  }));
}

/** What's kept under `key`, or undefined. */
export function idbGet<T>(key: string): Promise<T | undefined> {
  return run<T | undefined>('readonly', (store) => store.get(key));
}

/** Keep `value` under `key`. */
export function idbSet(key: string, value: unknown): Promise<void> {
  return run('readwrite', (store) => store.put(value, key)).then(() => undefined);
}
