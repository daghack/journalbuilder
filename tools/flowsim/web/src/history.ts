// Undo and redo. An entry is either the scene's objects and settings (as
// JSON) or a snapshot of the engine's recorded paths; undoing one restores
// only what it holds, so trails recorded since aren't lost by undoing an
// object's move.

import type { FlowSim } from '../wasm-pkg/flowsim.js';

type Entry = { kind: 'scene'; json: string } | { kind: 'paths'; snap: number };

const LIMIT = 100;

export class History {
  private undos: Entry[] = [];
  private redos: Entry[] = [];

  constructor(
    private sim: () => FlowSim,
    private sceneJson: () => string,
    private restoreScene: (json: string) => void,
    private onChange: () => void,
  ) {}

  get canUndo(): boolean {
    return this.undos.length > 0;
  }
  get canRedo(): boolean {
    return this.redos.length > 0;
  }

  /** Call before changing objects or settings. */
  recordScene(): void {
    this.push({ kind: 'scene', json: this.sceneJson() });
  }

  /** Call before changing recorded paths. */
  recordPaths(): void {
    this.push({ kind: 'paths', snap: this.sim().snapshot_paths() });
  }

  undo(): void {
    const e = this.undos.pop();
    if (!e) return;
    this.redos.push(this.capture(e.kind));
    this.restore(e);
  }

  redo(): void {
    const e = this.redos.pop();
    if (!e) return;
    this.undos.push(this.capture(e.kind));
    this.restore(e);
  }

  clear(): void {
    [...this.undos, ...this.redos].forEach((e) => this.forget(e));
    this.undos = [];
    this.redos = [];
    this.onChange();
  }

  private push(e: Entry): void {
    this.undos.push(e);
    while (this.undos.length > LIMIT) this.forget(this.undos.shift()!);
    this.redos.forEach((r) => this.forget(r));
    this.redos = [];
    this.onChange();
  }

  private capture(kind: Entry['kind']): Entry {
    return kind === 'scene'
      ? { kind: 'scene', json: this.sceneJson() }
      : { kind: 'paths', snap: this.sim().snapshot_paths() };
  }

  private restore(e: Entry): void {
    if (e.kind === 'scene') this.restoreScene(e.json);
    else {
      this.sim().restore_paths(e.snap);
      this.forget(e);
    }
    this.onChange();
  }

  private forget(e: Entry): void {
    if (e.kind === 'paths') this.sim().forget_paths(e.snap);
  }
}
