import { Injectable, inject, signal, untracked } from '@angular/core';
import type { DarklyReply, DarklyRequest, EvalReply, EvalRequest, Evaluation, JournalPayload, RenderOptions } from '../scribe';
import { JournalStore } from './journal-store.service';

/** How long a script may run before it's taken to be stuck (an endless loop). */
const SCRIPT_TIMEOUT_MS = 20_000;
/** How long a Darkly render may go without news before it's taken to be stuck. */
const RENDER_TIMEOUT_MS = 120_000;

/** Where the wasm module is served (see angular.json's assets). */
const wasmUrl = () => new URL('wasm/scribe_wasm_bg.wasm', document.baseURI).href;

/** Progress through a Darkly render: page `page` of `pages`, `fraction` of the way through it. */
export interface RenderProgress {
  page: number;
  pages: number;
  fraction: number;
}

export interface EvalCallbacks {
  evaluated(generation: number, evaluation: Evaluation): void;
  svg(generation: number, index: number, svg: string | null): void;
}

/**
 * The two workers that run the renderer: one runs scripts and lays out
 * their pages as SVG while the user types; the other renders with Darkly
 * on the GPU, for the Darkly preview and exports.
 */
@Injectable({ providedIn: 'root' })
export class RenderService {
  private readonly store = inject(JournalStore);
  /** Names scripts can call, for the editor; empty until the renderer loads. */
  readonly apiNames = signal<string[]>([]);
  /** Why the renderer couldn't load, if it couldn't. */
  readonly loadError = signal<string | null>(null);

  private evalWorker: Worker | null = null;
  private callbacks: EvalCallbacks | null = null;
  private generation = 0;
  /** The script being run, and the newest one waiting for it to finish. */
  private running: { generation: number; timer: ReturnType<typeof setTimeout> } | null = null;
  private waiting: { generation: number; source: string; first: number } | null = null;

  private darklyWorker: Worker | null = null;
  private nextId = 0;
  private jobs = new Map<number, (reply: DarklyReply) => void>();
  /** When the Darkly worker was last heard from, while it has jobs. */
  private lastHeard = 0;
  private watchdog: ReturnType<typeof setInterval> | null = null;

  /** The journal's files each worker was last sent; null for a new worker. */
  private evalJournal: JournalPayload | null = null;
  private darklyJournal: JournalPayload | null = null;

  /**
   * Send `worker` the journal's files if they've changed since `sent`, just
   * before asking for a run, so the run is sure to use them. Returns what
   * the worker has now.
   */
  private sendJournal(worker: Worker, sent: JournalPayload | null): JournalPayload {
    const files = untracked(() => this.store.resources());
    if (files !== sent) worker.postMessage({ type: 'journal', files } satisfies EvalRequest);
    return files;
  }

  /** Start the script worker; `callbacks` hear what it makes. */
  connect(callbacks: EvalCallbacks) {
    this.callbacks = callbacks;
    this.startEvalWorker();
  }

  /**
   * Run `source`, sending its pages to `evaluated` and then each page's SVG
   * to `svg`, `first` first. Returns the run's generation. While a script
   * runs, only the newest one asked for waits to run next.
   */
  evaluate(source: string, first: number): number {
    const generation = ++this.generation;
    this.waiting = { generation, source, first };
    if (!this.running) this.runWaiting();
    return generation;
  }

  /** Send page `index`'s SVG next, if it's still to come. */
  focus(generation: number, index: number) {
    this.evalWorker?.postMessage({ type: 'focus', generation, index } satisfies EvalRequest);
  }

  private startEvalWorker() {
    this.evalWorker?.terminate();
    const worker = new Worker(new URL('../workers/eval.worker', import.meta.url), { type: 'module' });
    worker.onmessage = ({ data }: MessageEvent<EvalReply>) => this.onEvalReply(data);
    worker.onerror = (event) => this.loadError.set(event.message || 'the script worker failed to start');
    worker.postMessage({ type: 'init', wasmUrl: wasmUrl() } satisfies EvalRequest);
    this.evalWorker = worker;
    this.evalJournal = null;
  }

  private runWaiting() {
    const next = this.waiting;
    if (!next) return;
    this.waiting = null;
    const timer = setTimeout(() => this.stuck(next.generation), SCRIPT_TIMEOUT_MS);
    this.running = { generation: next.generation, timer };
    this.evalJournal = this.sendJournal(this.evalWorker!, this.evalJournal);
    this.evalWorker!.postMessage({ type: 'evaluate', ...next } satisfies EvalRequest);
  }

  /** The script ran too long: end the worker and start another. */
  private stuck(generation: number) {
    this.running = null;
    this.startEvalWorker();
    const message = `the script ran for over ${SCRIPT_TIMEOUT_MS / 1000} s and was stopped; is there an endless loop?`;
    this.callbacks?.evaluated(generation, { pages: [], warnings: [], prints: [], missingGlyphs: null, error: { message, line: null, column: null }, millis: SCRIPT_TIMEOUT_MS });
    this.runWaiting();
  }

  private onEvalReply(reply: EvalReply) {
    switch (reply.type) {
      case 'ready':
        this.apiNames.set(reply.apiNames);
        break;
      case 'failed':
        this.loadError.set(reply.message);
        break;
      case 'evaluated':
        if (this.running?.generation === reply.generation) {
          clearTimeout(this.running.timer);
          this.running = null;
        }
        this.callbacks?.evaluated(reply.generation, reply.evaluation);
        this.runWaiting();
        break;
      case 'svg':
        this.callbacks?.svg(reply.generation, reply.index, reply.svg);
        break;
      case 'crashed': {
        // Show the crash as the running script's error, and carry on in a new worker.
        const generation = this.running?.generation ?? this.generation;
        if (this.running) clearTimeout(this.running.timer);
        this.running = null;
        this.startEvalWorker();
        this.callbacks?.evaluated(generation, { pages: [], warnings: [], prints: [], missingGlyphs: null, error: { message: reply.message, line: null, column: null }, millis: 0 });
        this.runWaiting();
        break;
      }
    }
  }

  // ---- Darkly ----

  private darkly(): Worker {
    if (!this.darklyWorker) {
      const worker = new Worker(new URL('../workers/darkly.worker', import.meta.url), { type: 'module' });
      worker.onmessage = ({ data }: MessageEvent<DarklyReply>) => {
        this.lastHeard = Date.now();
        this.jobs.get(data.id)?.(data);
      };
      // The renderer crashed (a panic, or the GPU failing under it): it can't go on.
      worker.onerror = (event) => {
        event.preventDefault();
        this.cancelRenders(explain(event.message || 'the renderer crashed'));
      };
      worker.postMessage({ type: 'init', wasmUrl: wasmUrl() } satisfies DarklyRequest);
      this.darklyWorker = worker;
      this.darklyJournal = null;
    }
    return this.darklyWorker;
  }

  /** Stop every Darkly render, failing them with `reason`, and start the next with a fresh renderer. */
  cancelRenders(reason = 'cancelled') {
    this.darklyWorker?.terminate();
    this.darklyWorker = null;
    const jobs = [...this.jobs];
    this.jobs.clear();
    for (const [id, job] of jobs) job({ type: 'error', id, message: reason });
    this.stopWatchdog();
  }

  /** Fail renders that stop sending news (the GPU hung, say). */
  private startWatchdog() {
    this.lastHeard = Date.now();
    this.watchdog ??= setInterval(() => {
      if (Date.now() - this.lastHeard > RENDER_TIMEOUT_MS) {
        this.cancelRenders(`Darkly stopped responding for ${RENDER_TIMEOUT_MS / 1000} s and was stopped; the GPU may have hung. Try a smaller scale.`);
      }
    }, 5_000);
  }

  private stopWatchdog() {
    if (this.watchdog && this.jobs.size === 0) {
      clearInterval(this.watchdog);
      this.watchdog = null;
    }
  }

  /**
   * Render pages `indices` of `source` with Darkly, calling `page` with each
   * one's PNG (null if nothing's drawn on it) as it's done.
   */
  renderPages(source: string, indices: number[], options: RenderOptions,
              page: (index: number, png: Blob | null) => void, progress: (p: RenderProgress) => void): Promise<void> {
    return this.job({ type: 'renderPages', id: 0, source, indices, options }, progress, (reply) => {
      if (reply.type === 'page') page(reply.index, reply.png ? new Blob([reply.png], { type: 'image/png' }) : null);
    }).then(() => undefined);
  }

  /** Every page of `source` rendered with Darkly into one PDF titled `title`. */
  exportPdf(source: string, title: string, options: RenderOptions, progress: (p: RenderProgress) => void): Promise<Blob> {
    return this.job({ type: 'exportPdf', id: 0, source, title, options }, progress)
      .then((reply) => new Blob([reply.pdf!], { type: 'application/pdf' }));
  }

  private job(request: Extract<DarklyRequest, { id: number }>, progress: (p: RenderProgress) => void,
              other?: (reply: DarklyReply) => void): Promise<Extract<DarklyReply, { type: 'done' }>> {
    const id = ++this.nextId;
    const worker = this.darkly();
    this.darklyJournal = this.sendJournal(worker, this.darklyJournal);
    this.startWatchdog();
    return new Promise((resolve, reject) => {
      this.jobs.set(id, (reply) => {
        if (reply.type === 'progress') {
          progress(reply);
        } else if (reply.type === 'done') {
          this.jobs.delete(id);
          this.stopWatchdog();
          resolve(reply);
        } else if (reply.type === 'error') {
          if (this.jobs.delete(id) && this.darklyWorker === worker) {
            // After a failure the renderer may be broken (a lost GPU, say): start afresh.
            this.cancelRenders(explain(reply.message));
          }
          this.stopWatchdog();
          reject(new Error(explain(reply.message)));
        } else {
          other?.(reply);
        }
      });
      worker.postMessage({ ...request, id });
    });
  }
}

/** A renderer error, said so the user knows what to do about it. */
export function explain(message: string): string {
  if (/found no adapters|Request for adapter failed|no WebGPU/i.test(message)) {
    return "Darkly can't render: the browser isn't offering WebGPU. If its GPU crashed earlier, the browser turns WebGPU off for this site until it restarts, so quit and reopen it (chrome://gpu shows WebGPU's status).";
  }
  if (/GPU was lost|GPU reported an error/i.test(message)) {
    return `Darkly can't render: ${message}. The browser may have reset its GPU; try again, or at a smaller scale.`;
  }
  return message.startsWith('Darkly') || message === 'cancelled' || message.endsWith('cancelled.') ? message : `Darkly couldn't render: ${message}`;
}
