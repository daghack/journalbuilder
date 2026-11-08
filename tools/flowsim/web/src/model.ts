// The scene as the engine reads and writes it (tools/flowsim/sim/src/scene.rs
// and objects.rs): domain units, y down, angles in degrees clockwise.

export type Vec = [number, number];

export type Obj =
  | { id: number; kind: 'pebble'; pos: Vec; radius: number }
  | { id: number; kind: 'rect'; pos: Vec; size: Vec; angle: number }
  | { id: number; kind: 'wall'; points: Vec[]; thickness: number }
  | { id: number; kind: 'jet'; pos: Vec; angle: number; speed: number; radius: number }
  | { id: number; kind: 'vortex'; pos: Vec; spin: number; radius: number }
  | { id: number; kind: 'source'; pos: Vec; speed: number; radius: number; moving?: boolean }
  | {
      id: number;
      kind: 'attractor';
      pos: Vec;
      strength: number;
      radius: number;
      falloff: Falloff;
      spin: number;
      capture: number;
      moving?: boolean;
    }
  | { id: number; kind: 'seed_point'; pos: Vec; emit_rate: number }
  | { id: number; kind: 'seed_line'; a: Vec; b: Vec; count: number; emit_rate: number }
  | { id: number; kind: 'seed_ring'; pos: Vec; radius: number; count: number; emit_rate: number };

export type Kind = Obj['kind'];
export type Falloff = 'inverse_square' | 'gaussian';
export type Family = 'obstacle' | 'stirrer' | 'attractor' | 'seed';

export type Edge = 'wall' | 'inflow' | 'open';

export interface FlowSettings {
  width: number;
  height: number;
  resolution: number;
  viscosity: number;
  vorticity: number;
  inflow_speed: number;
  edges: { left: Edge; right: Edge; top: Edge; bottom: Edge };
  pressure_iterations: number;
  perturbation: number;
  dye: 'stripes' | 'none';
  dye_fade: number;
  speed: number;
}

export interface ParticleSettings {
  inertia: number;
  step: number;
  max_length: number;
  backward: boolean;
  at_obstacle: 'stop' | 'bounce';
  close_loops: boolean;
  max_particles: number;
}

export interface Settings {
  flow: FlowSettings;
  particles: ParticleSettings;
}

/** The scene less its recorded paths, which stay in the engine. */
export interface SceneState extends Settings {
  objects: Obj[];
}

export interface SvgOptions {
  width: number;
  unit: 'mm' | 'px';
  stroke: string;
  stroke_width: number;
  simplify: number;
  smooth: boolean;
  min_length: number;
  recorded: boolean;
  live: boolean;
  obstacles: boolean;
  stirrers: boolean;
  attractors: boolean;
  seeds: boolean;
  background: string;
}

export interface Stats {
  frozen: boolean;
  grid: [number, number];
  particles: number;
  movers: number;
  live_paths: number;
  recorded_paths: number;
  recorded_points: number;
  divergence: number;
  ends: Record<string, number>;
}

export function family(kind: Kind): Family {
  switch (kind) {
    case 'pebble':
    case 'rect':
    case 'wall':
      return 'obstacle';
    case 'jet':
    case 'vortex':
    case 'source':
      return 'stirrer';
    case 'attractor':
      return 'attractor';
    default:
      return 'seed';
  }
}

/** Whether it moves like a particle: a moving source or attractor. */
export function moving(o: Obj): boolean {
  return (o.kind === 'source' || o.kind === 'attractor') && !!o.moving;
}

export function label(o: Obj): string {
  const name = baseLabel(o);
  return moving(o) ? `Moving ${name.toLowerCase()}` : name;
}

function baseLabel(o: Obj): string {
  switch (o.kind) {
    case 'pebble':
      return 'Pebble';
    case 'rect':
      return 'Block';
    case 'wall':
      return 'Wall';
    case 'jet':
      return 'Jet';
    case 'vortex':
      return 'Vortex';
    case 'source':
      return o.speed < 0 ? 'Sink' : 'Source';
    case 'attractor':
      return o.strength < 0 ? 'Repeller' : 'Attractor';
    case 'seed_point':
      return 'Seed';
    case 'seed_line':
      return 'Rake';
    case 'seed_ring':
      return 'Ring of seeds';
  }
}

/** Every point an object is placed by, to move it. */
export function anchors(o: Obj): Vec[] {
  switch (o.kind) {
    case 'wall':
      return o.points;
    case 'seed_line':
      return [o.a, o.b];
    default:
      return [o.pos];
  }
}

export function moveBy(o: Obj, dx: number, dy: number): void {
  for (const p of anchors(o)) {
    p[0] += dx;
    p[1] += dy;
  }
}

export function clone<T>(x: T): T {
  return JSON.parse(JSON.stringify(x)) as T;
}

// --- geometry ---

export function dist(a: Vec, b: Vec): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1]);
}

export function segmentDistance(p: Vec, a: Vec, b: Vec): number {
  const abx = b[0] - a[0];
  const aby = b[1] - a[1];
  const l2 = abx * abx + aby * aby;
  let t = l2 > 1e-12 ? ((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / l2 : 0;
  t = Math.max(0, Math.min(1, t));
  return Math.hypot(p[0] - (a[0] + abx * t), p[1] - (a[1] + aby * t));
}

export function polylineDistance(p: Vec, points: Vec[]): number {
  if (points.length === 1) return dist(p, points[0]);
  let best = Infinity;
  for (let i = 1; i < points.length; i++) best = Math.min(best, segmentDistance(p, points[i - 1], points[i]));
  return best;
}

export const DEG = Math.PI / 180;

export function rotate(v: Vec, degrees: number): Vec {
  const a = degrees * DEG;
  const c = Math.cos(a);
  const s = Math.sin(a);
  return [v[0] * c - v[1] * s, v[0] * s + v[1] * c];
}

export function angleOf(from: Vec, to: Vec): number {
  return Math.atan2(to[1] - from[1], to[0] - from[0]) / DEG;
}

export function round(x: number, places = 2): number {
  const k = 10 ** places;
  return Math.round(x * k) / k;
}
