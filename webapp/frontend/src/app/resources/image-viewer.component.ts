import { Component, computed, inject, signal } from '@angular/core';
import { splitPath } from '../journal/journal-files';
import { ThreePaneComponent } from '../layout/three-pane.component';
import { JournalStore } from '../services/journal-store.service';
import { LibraryService } from '../services/library.service';
import { ShellService } from '../shell/shell.service';
import { LibraryBannerComponent } from './library-banner.component';

/**
 * An image: the picture, its size, and how an entry paints with it. Its
 * file is held as a data URL; it's added from the Files tab, not edited.
 */
@Component({
  selector: 'app-image-viewer',
  imports: [ThreePaneComponent, LibraryBannerComponent],
  template: `
    <app-three-pane key="images" [right]="false">
      <section center class="centre">
        <app-library-banner [path]="path()" [own]="own()" [builtin]="builtin()" (copy)="copyIn()" (revert)="revert()" />
        <div class="stage">
          @if (url(); as url) { <img [src]="url" [alt]="'The image ' + name()" (load)="measure($event)" /> }
          <p class="hint">
            @if (size(); as s) { {{ s.width }} × {{ s.height }} pixels. }
            An entry reads it with image("{{ name() }}"), and draws strokes coloured from it with
            new_image_paint(image("{{ name() }}"), #{{ '{' }} at, width {{ '}' }}): each stroke the colour of the picture halfway along it.
          </p>
        </div>
      </section>
    </app-three-pane>
  `,
  styles: `
    :host { display: flex; flex-direction: column; min-height: 0; height: 100%; }
    .centre { display: flex; flex-direction: column; min-height: 0; min-width: 0; }
    .stage { flex: 1; overflow: auto; padding: 12px; background: var(--stage-bg); display: flex; flex-direction: column; gap: 8px; }
    img { max-width: 100%; max-height: 70vh; object-fit: contain; align-self: flex-start; border-radius: 4px; box-shadow: 0 1px 6px rgb(0 0 0 / 0.25); }
  `,
})
export class ImageViewerComponent {
  private readonly store = inject(JournalStore);
  private readonly library = inject(LibraryService);
  private readonly shell = inject(ShellService);

  /** The image shown, which stays while another kind's file shows. */
  protected readonly path = this.shell.lastOpened(['images']);
  protected readonly name = computed(() => splitPath(this.path() ?? '')?.name ?? '');
  protected readonly own = computed(() => (this.path() ?? '') in this.store.files());
  protected readonly builtin = computed(() => this.library.has(this.path() ?? ''));
  protected readonly url = computed(() => {
    const path = this.path();
    return path ? (this.store.files()[path] ?? this.library.files()[path] ?? null) : null;
  });
  protected readonly size = signal<{ width: number; height: number } | null>(null);

  protected measure(event: Event) {
    const img = event.target as HTMLImageElement;
    this.size.set({ width: img.naturalWidth, height: img.naturalHeight });
  }

  protected copyIn() {
    const path = this.path();
    const url = this.url();
    if (path && url) this.store.setFile(path, url);
  }

  protected revert() {
    const path = this.path();
    if (path) this.store.removeFile(path);
  }
}
