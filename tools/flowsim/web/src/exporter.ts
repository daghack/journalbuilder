// The Export SVG dialog: options, a preview, and the download.

import type { FlowSim } from '../wasm-pkg/flowsim.js';
import { type Edit, check, el, numberBox, select, slider, text } from './form';
import type { SvgOptions } from './model';
import { load, save } from './store';

const KEY = 'flowsim.svg-options';

export function download(name: string, contents: string, type: string): void {
  const url = URL.createObjectURL(new Blob([contents], { type }));
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export class Exporter {
  private options: SvgOptions;
  private dialog = document.getElementById('export-dialog') as HTMLDialogElement;
  private optionsRoot = document.getElementById('export-options')!;
  private preview = document.getElementById('export-preview') as HTMLImageElement;
  private info = document.getElementById('export-info')!;
  private url: string | null = null;
  private timer = 0;

  constructor(
    private sim: () => FlowSim,
    defaultsJson: string,
  ) {
    const defaults = JSON.parse(defaultsJson) as SvgOptions;
    this.options = { ...defaults, ...(load<Partial<SvgOptions>>(KEY) ?? {}) };
    document.getElementById('export-download')!.addEventListener('click', (e) => {
      e.preventDefault();
      download('flow-paths.svg', this.svg(), 'image/svg+xml');
    });
    this.dialog.addEventListener('close', () => this.release());
    this.build();
  }

  open(): void {
    this.refresh();
    this.dialog.showModal();
  }

  private svg(): string {
    return this.sim().svg(JSON.stringify(this.options));
  }

  private build(): void {
    const o = this.options;
    const edit: Edit = {
      begin: () => {},
      change: () => {
        save(KEY, o);
        clearTimeout(this.timer);
        this.timer = window.setTimeout(() => this.refresh(), 120);
      },
    };
    this.optionsRoot.replaceChildren(
      numberBox('Width', () => o.width, (v) => (o.width = v), { min: 1, max: 100000, step: 1 }, edit),
      select('Unit', () => o.unit, (v) => (o.unit = v), [
        ['mm', 'millimetres'],
        ['px', 'pixels'],
      ], edit),
      text('Stroke', () => o.stroke, (v) => (o.stroke = v), edit, 'Any SVG colour'),
      slider('Stroke width', () => o.stroke_width, (v) => (o.stroke_width = v), { min: 0.05, max: 5, step: 0.05, title: 'In the unit above' }, edit),
      slider('Simplify', () => o.simplify, (v) => (o.simplify = v), { min: 0, max: 5, step: 0.05, title: 'How far (domain units) a simplified path may stray from the traced one' }, edit),
      check('Smooth curves', () => o.smooth, (v) => (o.smooth = v), edit, 'Cubic curves through the points rather than straight lines'),
      slider('Min length', () => o.min_length, (v) => (o.min_length = v), { min: 0, max: 500, step: 1, title: 'Shorter paths are left out (domain units)' }, edit),
      el('div', 'sub', 'Paths'),
      check('Recorded paths', () => o.recorded, (v) => (o.recorded = v), edit),
      check('Live paths of frozen seeds', () => o.live, (v) => (o.live = v), edit),
      el('div', 'sub', 'Outlines, each in its own group'),
      check('Obstacles', () => o.obstacles, (v) => (o.obstacles = v), edit),
      check('Stirrers', () => o.stirrers, (v) => (o.stirrers = v), edit),
      check('Attractors', () => o.attractors, (v) => (o.attractors = v), edit),
      check('Seeds', () => o.seeds, (v) => (o.seeds = v), edit),
      text('Background', () => o.background, (v) => (o.background = v), edit, 'A fill behind everything; leave empty for none'),
    );
  }

  private refresh(): void {
    const svg = this.svg();
    this.release();
    this.url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
    this.preview.src = this.url;
    const paths = (svg.match(/<path /g) ?? []).length;
    const kb = new Blob([svg]).size / 1024;
    this.info.textContent = `${paths} path${paths === 1 ? '' : 's'}, ${kb < 1024 ? `${kb.toFixed(0)} KB` : `${(kb / 1024).toFixed(1)} MB`}`;
  }

  private release(): void {
    if (this.url) URL.revokeObjectURL(this.url);
    this.url = null;
  }
}
