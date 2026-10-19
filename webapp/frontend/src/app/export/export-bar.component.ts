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
    <button type="button" class="export" (click)="ask()" [disabled]="!canExport()">Export</button>

    <dialog #dialog (close)="onClose()" (cancel)="$event.preventDefault(); stop()">
      <h2>Export</h2>
      @if (!running() && !done()) {
        <p class="note">
          {{ journal.pages().length }} page{{ journal.pages().length === 1 ? '' : 's' }}, rendered with Darkly
          {{ format() === 'pdf' ? 'into one PDF' : 'as a zip of PNGs, one per page' }}.
        </p>
        <label>Export as
          <select (change)="format.set($any($event.target).value)">
            <option value="pdf" [selected]="format() === 'pdf'">PDF</option>
            <option value="png" [selected]="format() === 'png'">PNG zip</option>
          </select>
        </label>
        <label>Scale
          <select (change)="scale.set(+$any($event.target).value)">
            @for (s of scales; track s) { <option [value]="s" [selected]="s === scale()">{{ s }}{{ s === 1 ? ' (full size)' : '' }}</option> }
          </select>
        </label>
        <label>Supersample
          <input type="number" min="1" step="0.1" [value]="supersample()" (change)="setSupersample($any($event.target))" />
        </label>
        @if (format() === 'png') {
          <label>Stitch every
            <input type="number" min="1" step="1" placeholder="off" [value]="stitch() ?? ''" (change)="setStitch($any($event.target))" />
            pages
          </label>
        }
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
  protected readonly supersample = signal(1.2);
  /** How many pages to a stitched image; null for none. */
  protected readonly stitch = signal<number | null>(null);
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

  protected ask() {
    this.done.set(false);
    this.error.set(null);
    this.progress.set(null);
    this.dialog().nativeElement.showModal();
  }

  protected setSupersample(input: HTMLInputElement) {
    const value = parseFloat(input.value);
    this.supersample.set(Number.isFinite(value) ? Math.max(1, value) : 1.2);
    input.value = String(this.supersample());
  }

  protected setStitch(input: HTMLInputElement) {
    const value = Math.floor(parseFloat(input.value));
    this.stitch.set(Number.isFinite(value) && value >= 1 ? value : null);
    input.value = this.stitch() === null ? '' : String(this.stitch());
  }

  protected async start() {
    this.running.set(true);
    const source = this.journal.shownSource();
    const name = this.journal.baseName();
    const options = { scale: this.scale(), supersample: this.supersample(), parchment: this.parchment() };
    const progress = (p: RenderProgress) => this.progress.set(p);
    try {
      if (this.format() === 'pdf') {
        const pdf = await this.render.exportPdf(source, name, options, progress);
        this.files.download(pdf, `${name}.pdf`);
      } else {
        const pages = this.journal.pages().filter((p) => p.view);
        const files: Record<string, Uint8Array> = {};
        const pngs: Promise<void>[] = [];
        const rendered = new Map<number, Blob>();
        await this.render.renderPages(source, pages.map((p) => p.index), options, (index, png) => {
          const page = pages.find((p) => p.index === index)!;
          if (png) {
            rendered.set(index, png);
            pngs.push(png.arrayBuffer().then((b) => void (files[`darkly_${page.label}.png`] = new Uint8Array(b))));
          }
        }, progress);
        await Promise.all(pngs);
        const n = this.stitch();
        if (n !== null) {
          const drawn = pages.filter((p) => rendered.has(p.index));
          for (let i = 0; i < drawn.length; i += n) {
            const group = drawn.slice(i, i + n);
            // A page left alone isn't stitched.
            if (group.length < 2) continue;
            const png = await stitched(group.map((p) => rendered.get(p.index)!));
            files[`darkly_${stitchLabel(group[0].label, group[group.length - 1].label)}.png`] = new Uint8Array(await png.arrayBuffer());
          }
        }
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

/** What a stitch of pages `first` to `last` is called: page_0 to page_5 is page_0-5 (as journalbuild names them). */
function stitchLabel(first: string, last: string): string {
  const stem = (label: string) => label.replace(/\d+$/, '').length;
  const [a, b] = [stem(first), stem(last)];
  return a < first.length && b < last.length && first.slice(0, a) === last.slice(0, b) ? `${first}-${last.slice(b)}` : `${first}-${last}`;
}

/** PNGs side by side, left to right, their tops level. */
async function stitched(pngs: Blob[]): Promise<Blob> {
  const images = await Promise.all(pngs.map((png) => createImageBitmap(png)));
  const canvas = new OffscreenCanvas(images.reduce((w, i) => w + i.width, 0), Math.max(...images.map((i) => i.height)));
  const context = canvas.getContext('2d')!;
  let x = 0;
  for (const image of images) {
    context.drawImage(image, x, 0);
    x += image.width;
    image.close();
  }
  return canvas.convertToBlob({ type: 'image/png' });
}
