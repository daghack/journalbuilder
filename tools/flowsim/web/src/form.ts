// Small form rows for the side panel and the export dialog. Each row calls
// `begin` once at the start of an edit (for undo) and `change` as it goes.

export interface Edit {
  begin(): void;
  change(): void;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

export { el };

function row(label: string, title?: string): HTMLLabelElement {
  const r = el('label', 'row');
  const name = el('span', 'name', label);
  if (title) r.title = title;
  r.append(name);
  return r;
}

function decimals(step: number): number {
  const s = String(step);
  const dot = s.indexOf('.');
  return dot < 0 ? 0 : s.length - dot - 1;
}

/** A slider with a number box beside it. */
export function slider(
  label: string,
  get: () => number,
  set: (v: number) => void,
  opts: { min: number; max: number; step: number; title?: string; unit?: string },
  edit: Edit,
): HTMLElement {
  const r = row(label, opts.title);
  const range = el('input');
  range.type = 'range';
  range.min = String(opts.min);
  range.max = String(opts.max);
  range.step = String(opts.step);
  const box = el('input', 'num');
  box.type = 'number';
  box.step = String(opts.step);
  const places = decimals(opts.step);
  const show = () => {
    const v = get();
    range.value = String(v);
    box.value = String(Number(v.toFixed(places)));
  };
  show();
  let editing = false;
  const start = () => {
    if (!editing) {
      edit.begin();
      editing = true;
    }
  };
  range.addEventListener('input', () => {
    start();
    set(Number(range.value));
    box.value = String(Number(Number(range.value).toFixed(places)));
    edit.change();
  });
  range.addEventListener('change', () => (editing = false));
  box.addEventListener('change', () => {
    const v = Number(box.value);
    if (!Number.isFinite(v)) return show();
    edit.begin();
    set(v);
    show();
    edit.change();
  });
  r.append(range, box);
  if (opts.unit) r.append(el('span', 'unit', opts.unit));
  return r;
}

/** A plain number box. */
export function numberBox(
  label: string,
  get: () => number,
  set: (v: number) => void,
  opts: { min?: number; max?: number; step?: number; title?: string; unit?: string },
  edit: Edit,
): HTMLElement {
  const r = row(label, opts.title);
  const box = el('input', 'num wide');
  box.type = 'number';
  if (opts.min !== undefined) box.min = String(opts.min);
  if (opts.max !== undefined) box.max = String(opts.max);
  box.step = String(opts.step ?? 1);
  box.value = String(get());
  box.addEventListener('change', () => {
    let v = Number(box.value);
    if (!Number.isFinite(v)) {
      box.value = String(get());
      return;
    }
    if (opts.min !== undefined) v = Math.max(opts.min, v);
    if (opts.max !== undefined) v = Math.min(opts.max, v);
    edit.begin();
    set(v);
    box.value = String(get());
    edit.change();
  });
  r.append(box);
  if (opts.unit) r.append(el('span', 'unit', opts.unit));
  return r;
}

export function select<T extends string | number>(
  label: string,
  get: () => T,
  set: (v: T) => void,
  choices: [T, string][],
  edit: Edit,
  title?: string,
): HTMLElement {
  const r = row(label, title);
  const s = el('select');
  for (const [value, text] of choices) {
    const o = el('option', undefined, text);
    o.value = String(value);
    s.append(o);
  }
  s.value = String(get());
  s.addEventListener('change', () => {
    const choice = choices.find(([v]) => String(v) === s.value);
    if (!choice) return;
    edit.begin();
    set(choice[0]);
    edit.change();
  });
  r.append(s);
  return r;
}

export function check(label: string, get: () => boolean, set: (v: boolean) => void, edit: Edit, title?: string): HTMLElement {
  const r = row(label, title);
  r.classList.add('check');
  const c = el('input');
  c.type = 'checkbox';
  c.checked = get();
  c.addEventListener('change', () => {
    edit.begin();
    set(c.checked);
    edit.change();
  });
  r.prepend(c);
  return r;
}

export function text(label: string, get: () => string, set: (v: string) => void, edit: Edit, title?: string): HTMLElement {
  const r = row(label, title);
  const t = el('input', 'wide');
  t.type = 'text';
  t.value = get();
  t.addEventListener('change', () => {
    edit.begin();
    set(t.value);
    edit.change();
  });
  r.append(t);
  return r;
}
