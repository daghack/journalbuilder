import { Component, ElementRef, computed, inject, signal, viewChild } from '@angular/core';
import { zipSync } from 'fflate';
import { FileService } from '../services/file.service';
import { JournalService } from '../services/journal.service';
import { RenderService, type RenderProgress } from '../services/render.service';

type Format = 'pdf' | 'png';

/** The buttons under the editor, and the dialog that renders an export with Darkly and shows how far it's got. */
@Component({
  selector: 'app-export-bar',
  template: `
    <button type="button" class="export" (click)="ask('pdf')" [disabled]="!canExport()">Export as PDF</button>
    <button type="button" class="export" (click)="ask('png')" [disabled]="!canExport()">Export as PNG</button>

    <dialog #dialog (close)="onClose()" (cancel)="$event.preventDefault(); stop()">
      <h2>Export as {{ format() === 'pdf' ? 'PDF' : 'PNGs' }}</h2>
      @if (!running() && !done()) {
        <p class="note">
          {{ journal.pages().length }} page{{ journal.pages().length === 1 ? '' : 's' }}, rendered with Darkly
          {{ format() === 'pdf' ? 'into one PDF' : 'as a zip of PNGs, one per page' }}.
        </p>
        <label>Scale
          <select (change)="scale.set(+$any($event.target).value)">
            @for (s of scales; track s) { <option [value]="s" [selected]="s === scale()">{{ s }}{{ s === 1 ? ' (full size)' : '' }}</option> }
          </select>
        </label>
        <label><input type="checkbox" [checked]="parchment()" (change)="parchment.set($any($event.target).checked)" /> On parchment</label>
        <div class="actions">
          <button type="button" (click)="dialog.close()">Close</button>
          <button type="button" class="primary" (click)="start()">Export</button>
        </div>
      } @else {
        <p class="note">{{ status() }}</p>
        <progress [value]="overall()" max="1"></progress>
        @if (error(); as error) { <p class="error">{{ error }}</p> }
        <div class="actions">
          @if (running()) {
            <button type="button" (click)="stop()">Cancel</button>
          } @else {
            <button type="button" class="primary" (click)="dialog.close()">Done</button>
          }
        </div>
      }
    </dialog>
  `,
  styles: `
    :host { display: flex; gap: 10px; padding: 10px; background: var(--rail-bg); }
    .export { flex: 1; padding: 10px; font-size: 13px; }
    dialog { border: 1px solid var(--border); border-radius: 10px; background: var(--surface); color: var(--fg); padding: 20px; width: min(420px, calc(100vw - 32px)); }
    dialog::backdrop { background: rgb(0 0 0 / 0.45); }
    h2 { margin: 0 0 12px; font-size: 16px; }
    .note { font-size: 13px; color: var(--muted); margin: 0 0 12px; }
    label { display: flex; gap: 8px; align-items: center; margin-bottom: 10px; font-size: 13px; }
    progress { width: 100%; height: 10px; }
    .error { color: var(--danger); font-size: 13px; }
    .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
  `,
})
export class ExportBarComponent {
  protected readonly journal = inject(JournalService);
  private readonly render = inject(RenderService);
  private readonly files = inject(FileService);
  protected readonly dialog = viewChild.required<ElementRef<HTMLDialogElement>>('dialog');

  protected readonly scales = [0.25, 0.5, 1, 2];
  protected readonly format = signal<Format>('pdf');
  protected readonly scale = signal(1);
  protected readonly parchment = signal(false);
  protected readonly running = signal(false);
  protected readonly done = signal(false);
  protected readonly progress = signal<RenderProgress | null>(null);
  protected readonly error = signal<string | null>(null);

  protected readonly canExport = computed(() => this.journal.pages().some((p) => p.view) && !this.journal.error());
  protected readonly overall = computed(() => {
    const p = this.progress();
    if (this.done()) return 1;
    return p && p.pages ? (p.page + p.fraction) / p.pages : 0;
  });
  protected readonly status = computed(() => {
    if (this.error()) return 'The export stopped.';
    if (this.done()) return 'Exported.';
    const p = this.progress();
    return p ? `Rendering page ${p.page + 1} of ${p.pages}… ${Math.round(this.overall() * 100)}%` : 'Starting the renderer…';
  });

  protected ask(format: Format) {
    this.format.set(format);
    this.done.set(false);
    this.error.set(null);
    this.progress.set(null);
    this.dialog().nativeElement.showModal();
  }

  protected async start() {
    this.running.set(true);
    const source = this.journal.shownSource();
    const name = this.journal.baseName();
    const options = { scale: this.scale(), parchment: this.parchment() };
    const progress = (p: RenderProgress) => this.progress.set(p);
    try {
      if (this.format() === 'pdf') {
        const pdf = await this.render.exportPdf(source, name, options, progress);
        this.files.download(pdf, `${name}.pdf`);
      } else {
        const pages = this.journal.pages().filter((p) => p.view);
        const files: Record<string, Uint8Array> = {};
        const pngs: Promise<void>[] = [];
        await this.render.renderPages(source, pages.map((p) => p.index), options, (index, png) => {
          const page = pages.find((p) => p.index === index)!;
          if (png) pngs.push(png.arrayBuffer().then((b) => void (files[`darkly_${page.label}.png`] = new Uint8Array(b))));
        }, progress);
        await Promise.all(pngs);
        // PNGs are compressed already.
        this.files.download(new Blob([zipSync(files, { level: 0 }) as Uint8Array<ArrayBuffer>], { type: 'application/zip' }), `${name}.zip`);
      }
      this.done.set(true);
    } catch (err) {
      this.error.set(err instanceof Error ? err.message : String(err));
    } finally {
      this.running.set(false);
    }
  }

  protected stop() {
    if (this.running()) this.render.cancelRenders('Export cancelled.');
    else this.dialog().nativeElement.close();
  }

  protected onClose() {
    if (this.running()) this.render.cancelRenders('Export cancelled.');
  }
}
