// The tools on the rail: what each makes, how dragging shapes it, and the
// handles and hit areas of what's placed.

import {
  type Obj,
  type Vec,
  angleOf,
  dist,
  polylineDistance,
  rotate,
  segmentDistance,
} from './model';

export type ToolName =
  | 'select'
  | 'stir'
  | 'drop'
  | 'seed_point'
  | 'seed_line'
  | 'seed_ring'
  | 'pebble'
  | 'rect'
  | 'wall'
  | 'jet'
  | 'vortex'
  | 'source'
  | 'sink'
  | 'attractor'
  | 'repeller'
  | 'm_attractor'
  | 'm_repeller'
  | 'm_source'
  | 'm_sink'
  | 'delete';

export interface ToolDef {
  name: ToolName;
  label: string;
  group: string;
  key?: string;
  hint: string;
  /** SVG content for a 24 × 24 icon, stroked in currentColor. */
  icon: string;
}

const ICON = {
  source: '<circle cx="12" cy="12" r="7"/><path d="M12 8.5 V15.5 M8.5 12 H15.5"/>',
  sink: '<circle cx="12" cy="12" r="7"/><path d="M8.5 12 H15.5"/>',
  attractor:
    '<circle cx="12" cy="12" r="2" fill="currentColor"/><circle cx="12" cy="12" r="7.5" stroke-dasharray="2.5 2"/><path d="M12 2.5 V6 M12 18 V21.5 M2.5 12 H6 M18 12 H21.5"/>',
  repeller:
    '<circle cx="12" cy="12" r="2" fill="currentColor"/><circle cx="12" cy="12" r="7.5" stroke-dasharray="2.5 2"/><path d="M12 6 V2.5 M12 18 V21.5 M6 12 H2.5 M18 12 H21.5"/>',
};

/** An icon shrunk to the right, with speed lines behind it. */
function moving(icon: string): string {
  return `<g transform="translate(5 2.5) scale(0.8)">${icon}</g><path d="M1 8.5 H4.5 M0.5 12 H4 M1 15.5 H4.5" stroke-linecap="round"/>`;
}

export const TOOLS: ToolDef[] = [
  {
    name: 'select',
    label: 'Select',
    group: 'Edit',
    key: 'v',
    hint: 'Pick an object or a recorded path; drag to move, handles to shape. Delete removes it.',
    icon: '<path d="M6 3 L6 19 L10 15 L13 21 L15.5 20 L12.5 14 L18 14 Z"/>',
  },
  {
    name: 'stir',
    label: 'Stir',
    group: 'Edit',
    key: 's',
    hint: 'Drag to push the fluid. Frozen, this reshapes the frozen flow.',
    icon: '<path d="M4 15 C7 9 10 9 12 12 C14 15 17 15 20 9"/><path d="M17 9 L20 9 L20 12"/>',
  },
  {
    name: 'delete',
    label: 'Delete',
    group: 'Edit',
    hint: 'Click an object or a recorded path to remove it.',
    icon: '<path d="M5 7 H19 M9 7 V4 H15 V7 M7 7 L8 20 H16 L17 7 M10 11 V17 M14 11 V17"/>',
  },
  {
    name: 'drop',
    label: 'Particle',
    group: 'Particles',
    key: 'p',
    hint: 'Click to drop a particle, or drag to drop a trail of them. Each leaves a recorded path when it stops.',
    icon: '<circle cx="8" cy="16" r="2.5"/><path d="M10 14 C13 10 15 12 19 5" stroke-dasharray="2 2"/>',
  },
  {
    name: 'seed_point',
    label: 'Seed',
    group: 'Particles',
    hint: 'Click to place a seed. Frozen, its path is traced live; running, it can emit particles.',
    icon: '<circle cx="12" cy="12" r="2.5"/><circle cx="12" cy="12" r="6.5"/>',
  },
  {
    name: 'seed_line',
    label: 'Rake',
    group: 'Particles',
    hint: 'Drag a line of seeds.',
    icon: '<path d="M6 4 L6 20"/><circle cx="6" cy="6" r="1.6"/><circle cx="6" cy="12" r="1.6"/><circle cx="6" cy="18" r="1.6"/><path d="M9 6 H18 M9 12 H18 M9 18 H18" stroke-dasharray="2 2"/>',
  },
  {
    name: 'seed_ring',
    label: 'Ring',
    group: 'Particles',
    hint: 'Drag out a ring of seeds.',
    icon: '<circle cx="12" cy="12" r="7"/><circle cx="12" cy="5" r="1.5"/><circle cx="19" cy="12" r="1.5"/><circle cx="12" cy="19" r="1.5"/><circle cx="5" cy="12" r="1.5"/>',
  },
  {
    name: 'pebble',
    label: 'Pebble',
    group: 'Obstacles',
    hint: 'Drag out a round obstacle.',
    icon: '<circle cx="12" cy="12" r="7" fill="currentColor" fill-opacity="0.25"/>',
  },
  {
    name: 'rect',
    label: 'Block',
    group: 'Obstacles',
    hint: 'Drag a block from corner to corner; turn it by its top handle.',
    icon: '<rect x="4" y="7" width="16" height="10" fill="currentColor" fill-opacity="0.25"/>',
  },
  {
    name: 'wall',
    label: 'Wall',
    group: 'Obstacles',
    hint: 'Draw a wall freehand.',
    icon: '<path d="M4 18 C8 6 13 18 20 6" stroke-width="3" stroke-linecap="round"/>',
  },
  {
    name: 'jet',
    label: 'Jet',
    group: 'Stirrers',
    hint: 'Drag in the direction a jet pushes the fluid.',
    icon: '<circle cx="7" cy="12" r="3"/><path d="M10 12 H20 M16 8 L20 12 L16 16"/>',
  },
  {
    name: 'vortex',
    label: 'Vortex',
    group: 'Stirrers',
    hint: 'Drag out a vortex that spins the fluid.',
    icon: '<path d="M12 12 C12 10 15 10 15 12 C15 15 10 16 9 12 C8 7 15 6 17.5 10 C20 15 15 20 10 19"/>',
  },
  {
    name: 'source',
    label: 'Source',
    group: 'Stirrers',
    hint: 'Drag out a source that pours fluid out.',
    icon: ICON.source,
  },
  {
    name: 'sink',
    label: 'Sink',
    group: 'Stirrers',
    hint: 'Drag out a sink that draws fluid in.',
    icon: ICON.sink,
  },
  {
    name: 'attractor',
    label: 'Attract',
    group: 'Forces',
    hint: 'Drag out an attractor that pulls particles (not the fluid).',
    icon: ICON.attractor,
  },
  {
    name: 'repeller',
    label: 'Repel',
    group: 'Forces',
    hint: 'Drag out a repeller that pushes particles away.',
    icon: ICON.repeller,
  },
  {
    name: 'm_attractor',
    label: 'Attract',
    group: 'Moving',
    hint: 'Drag out an attractor that moves like a particle, pulling the others along as it goes.',
    icon: moving(ICON.attractor),
  },
  {
    name: 'm_repeller',
    label: 'Repel',
    group: 'Moving',
    hint: 'Drag out a repeller that moves like a particle, pushing the others away as it goes.',
    icon: moving(ICON.repeller),
  },
  {
    name: 'm_source',
    label: 'Source',
    group: 'Moving',
    hint: 'Drag out a source that moves like a particle, pouring fluid out as it goes.',
    icon: moving(ICON.source),
  },
  {
    name: 'm_sink',
    label: 'Sink',
    group: 'Moving',
    hint: 'Drag out a sink that moves like a particle, drawing fluid in as it goes.',
    icon: moving(ICON.sink),
  },
];

/** Whether a tool places objects. */
export function places(tool: ToolName): boolean {
  return !['select', 'stir', 'drop', 'delete'].includes(tool);
}

/** A new object for a click at `p`; `px` is one screen pixel in domain units. */
export function create(tool: ToolName, id: number, p: Vec): Obj | null {
  const pos: Vec = [p[0], p[1]];
  switch (tool) {
    case 'pebble':
      return { id, kind: 'pebble', pos, radius: 40 };
    case 'rect':
      return { id, kind: 'rect', pos, size: [140, 50], angle: 0 };
    case 'wall':
      return { id, kind: 'wall', points: [pos], thickness: 10 };
    case 'jet':
      return { id, kind: 'jet', pos, angle: 0, speed: 160, radius: 30 };
    case 'vortex':
      return { id, kind: 'vortex', pos, spin: 120, radius: 70 };
    case 'source':
      return { id, kind: 'source', pos, speed: 60, radius: 30 };
    case 'sink':
      return { id, kind: 'source', pos, speed: -60, radius: 30 };
    case 'attractor':
      return { id, kind: 'attractor', pos, strength: 80, radius: 120, falloff: 'inverse_square', spin: 0, capture: 6 };
    case 'repeller':
      return { id, kind: 'attractor', pos, strength: -80, radius: 120, falloff: 'inverse_square', spin: 0, capture: 0 };
    case 'm_source':
      return { id, kind: 'source', pos, speed: 60, radius: 30, moving: true };
    case 'm_sink':
      return { id, kind: 'source', pos, speed: -60, radius: 30, moving: true };
    case 'm_attractor':
      return { id, kind: 'attractor', pos, strength: 60, radius: 80, falloff: 'inverse_square', spin: 0, capture: 0, moving: true };
    case 'm_repeller':
      return { id, kind: 'attractor', pos, strength: -60, radius: 80, falloff: 'inverse_square', spin: 0, capture: 0, moving: true };
    case 'seed_point':
      return { id, kind: 'seed_point', pos, emit_rate: 0 };
    case 'seed_line':
      return { id, kind: 'seed_line', a: [p[0], p[1] - 150], b: [p[0], p[1] + 150], count: 16, emit_rate: 0 };
    case 'seed_ring':
      return { id, kind: 'seed_ring', pos, radius: 40, count: 24, emit_rate: 0 };
    default:
      return null;
  }
}

/** Reshapes an object being made, dragged from `p0` to `p`. */
export function shape(o: Obj, p0: Vec, p: Vec, px: number): void {
  const d = dist(p0, p);
  switch (o.kind) {
    case 'pebble':
    case 'vortex':
    case 'source':
    case 'attractor':
    case 'seed_ring':
      o.radius = Math.max(d, 4);
      break;
    case 'rect':
      o.pos = [(p0[0] + p[0]) / 2, (p0[1] + p[1]) / 2];
      o.size = [Math.max(Math.abs(p[0] - p0[0]), 4), Math.max(Math.abs(p[1] - p0[1]), 4)];
      o.angle = 0;
      break;
    case 'wall': {
      const last = o.points[o.points.length - 1];
      if (dist(last, p) >= 6 * px) o.points.push([p[0], p[1]]);
      break;
    }
    case 'jet':
      o.angle = angleOf(p0, p);
      break;
    case 'seed_line':
      o.a = [p0[0], p0[1]];
      o.b = [p[0], p[1]];
      break;
    default:
      break;
  }
}

export interface Handle {
  id: string;
  pos: Vec;
}

/** The handles of a selected object; `px` is a screen pixel in domain units. */
export function handles(o: Obj, px: number): Handle[] {
  const at = (pos: Vec, v: Vec): Vec => [pos[0] + v[0], pos[1] + v[1]];
  switch (o.kind) {
    case 'pebble':
    case 'vortex':
    case 'source':
    case 'seed_ring':
      return [{ id: 'radius', pos: at(o.pos, [o.radius, 0]) }];
    case 'attractor': {
      const list = [{ id: 'radius', pos: at(o.pos, [o.radius, 0]) }];
      if (o.capture > 0) list.push({ id: 'capture', pos: at(o.pos, [-o.capture, 0]) });
      return list;
    }
    case 'rect':
      return [
        { id: 'size', pos: at(o.pos, rotate([o.size[0] / 2, o.size[1] / 2], o.angle)) },
        { id: 'rotate', pos: at(o.pos, rotate([0, -o.size[1] / 2 - 24 * px], o.angle)) },
      ];
    case 'jet':
      return [
        { id: 'radius', pos: at(o.pos, rotate([0, o.radius], o.angle)) },
        { id: 'aim', pos: at(o.pos, rotate([jetLength(o.radius, px), 0], o.angle)) },
      ];
    case 'wall':
      return o.points.map((pos, i) => ({ id: `p${i}`, pos }));
    case 'seed_line':
      return [
        { id: 'a', pos: o.a },
        { id: 'b', pos: o.b },
      ];
    case 'seed_point':
      return [];
  }
}

export function jetLength(radius: number, px: number): number {
  return Math.max(radius * 1.8, 40 * px);
}

export function dragHandle(o: Obj, id: string, p: Vec): void {
  switch (o.kind) {
    case 'pebble':
    case 'vortex':
    case 'source':
    case 'seed_ring':
      o.radius = Math.max(dist(o.pos, p), 2);
      break;
    case 'attractor':
      if (id === 'capture') o.capture = dist(o.pos, p);
      else o.radius = Math.max(dist(o.pos, p), 2);
      break;
    case 'rect':
      if (id === 'rotate') {
        o.angle = normalizeAngle(angleOf(o.pos, p) + 90);
      } else {
        const local = rotate([p[0] - o.pos[0], p[1] - o.pos[1]], -o.angle);
        o.size = [Math.max(Math.abs(local[0]) * 2, 2), Math.max(Math.abs(local[1]) * 2, 2)];
      }
      break;
    case 'jet':
      if (id === 'aim') o.angle = angleOf(o.pos, p);
      else o.radius = Math.max(dist(o.pos, p), 2);
      break;
    case 'wall': {
      const i = Number(id.slice(1));
      if (o.points[i]) o.points[i] = [p[0], p[1]];
      break;
    }
    case 'seed_line':
      if (id === 'a') o.a = [p[0], p[1]];
      else o.b = [p[0], p[1]];
      break;
    case 'seed_point':
      break;
  }
}

export function normalizeAngle(a: number): number {
  let x = ((a + 180) % 360 + 360) % 360 - 180;
  if (Object.is(x, -0)) x = 0;
  return x;
}

/** Whether `p` picks the object; `px` is a screen pixel in domain units. */
export function hits(o: Obj, p: Vec, px: number): boolean {
  const tol = 6 * px;
  const icon = 14 * px;
  switch (o.kind) {
    case 'pebble':
      return dist(o.pos, p) <= o.radius + tol;
    case 'rect': {
      const local = rotate([p[0] - o.pos[0], p[1] - o.pos[1]], -o.angle);
      return Math.abs(local[0]) <= o.size[0] / 2 + tol && Math.abs(local[1]) <= o.size[1] / 2 + tol;
    }
    case 'wall':
      return polylineDistance(p, o.points) <= o.thickness / 2 + tol;
    case 'jet':
      return (
        dist(o.pos, p) <= icon ||
        segmentDistance(p, o.pos, [
          o.pos[0] + Math.cos((o.angle * Math.PI) / 180) * jetLength(o.radius, px),
          o.pos[1] + Math.sin((o.angle * Math.PI) / 180) * jetLength(o.radius, px),
        ]) <= tol
      );
    case 'vortex':
    case 'source':
    case 'attractor':
    case 'seed_ring':
      return dist(o.pos, p) <= icon || Math.abs(dist(o.pos, p) - o.radius) <= tol;
    case 'seed_point':
      return dist(o.pos, p) <= icon;
    case 'seed_line':
      return segmentDistance(p, o.a, o.b) <= tol + 2 * px;
  }
}

/** The recorded path nearest `p`, within `tol`, from flat arrays. */
export function nearestPath(points: Float32Array, offsets: Uint32Array, p: Vec, tol: number): number | null {
  let best = tol;
  let found: number | null = null;
  for (let i = 0; i + 1 < offsets.length; i++) {
    const start = offsets[i];
    const end = offsets[i + 1];
    for (let k = start + 1; k < end; k++) {
      const a: Vec = [points[2 * k - 2], points[2 * k - 1]];
      const b: Vec = [points[2 * k], points[2 * k + 1]];
      // Skip segments whose box is too far off.
      if (
        Math.min(a[0], b[0]) - best > p[0] ||
        Math.max(a[0], b[0]) + best < p[0] ||
        Math.min(a[1], b[1]) - best > p[1] ||
        Math.max(a[1], b[1]) + best < p[1]
      )
        continue;
      const d = segmentDistance(p, a, b);
      if (d < best) {
        best = d;
        found = i;
      }
    }
  }
  return found;
}
