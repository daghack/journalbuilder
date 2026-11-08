// Drawing the stage: the field, paths, particles and objects, in domain
// units fitted to the canvas.

import { type Obj, type Vec, DEG, rotate } from './model';
import { handles, jetLength } from './tools';

export interface PathBundle {
  path: Path2D;
  points: Float32Array;
  offsets: Uint32Array;
  closed: Uint8Array;
}

export function bundle(set: { points: Float32Array; offsets: Uint32Array; closed: Uint8Array }): PathBundle {
  const { points, offsets, closed } = set;
  const path = new Path2D();
  for (let i = 0; i + 1 < offsets.length; i++) {
    const start = offsets[i];
    const end = offsets[i + 1];
    if (end - start < 2) continue;
    path.moveTo(points[2 * start], points[2 * start + 1]);
    for (let k = start + 1; k < end; k++) path.lineTo(points[2 * k], points[2 * k + 1]);
    if (closed[i]) path.closePath();
  }
  return { path, points, offsets, closed };
}

export function onePath(b: PathBundle, i: number): Path2D {
  const path = new Path2D();
  const start = b.offsets[i];
  const end = b.offsets[i + 1];
  if (end === undefined) return path;
  path.moveTo(b.points[2 * start], b.points[2 * start + 1]);
  for (let k = start + 1; k < end; k++) path.lineTo(b.points[2 * k], b.points[2 * k + 1]);
  if (b.closed[i]) path.closePath();
  return path;
}

export interface Arrows {
  cols: number;
  rows: number;
  data: Float32Array;
  reference: number;
}

export interface DrawState {
  domain: [number, number];
  field: HTMLCanvasElement | null;
  /** Drawn faintly over the fluid, in its box (domain units). */
  backdrop?: { image: HTMLCanvasElement; rect: [number, number, number, number] } | null;
  objects: Obj[];
  selection: number | null;
  hover: number | null;
  selectedPath: number | null;
  recorded: PathBundle | null;
  live: PathBundle | null;
  trails: PathBundle | null;
  heads: Float32Array | null;
  arrows: Arrows | null;
  show: { recorded: boolean; live: boolean; particles: boolean; objects: boolean };
  brush: { pos: Vec; radius: number } | null;
}

const C = {
  recorded: 'rgba(246, 236, 220, 0.92)',
  live: '#ff9b57',
  trail: 'rgba(255, 226, 110, 0.85)',
  head: '#fff6d6',
  obstacleFill: 'rgba(92, 100, 114, 0.9)',
  obstacleLine: '#c9d1dc',
  stirrer: '#6fe0ff',
  source: '#7dffb2',
  sink: '#ffaa7d',
  pull: '#ff4d6d',
  push: '#5aa9ff',
  seed: '#a5ff5e',
  select: '#ffffff',
  arrow: 'rgba(220, 235, 245, 0.55)',
  frame: 'rgba(255, 255, 255, 0.25)',
};

export class Renderer {
  scale = 1;
  ox = 0;
  oy = 0;
  /** How many times closer than fitting the domain in. */
  zoom = 1;
  /** The domain point at the canvas's centre; null for the domain's centre. */
  center: Vec | null = null;
  private dpr = 1;
  private cssW = 0;
  private cssH = 0;
  private ctx: CanvasRenderingContext2D;

  constructor(private canvas: HTMLCanvasElement) {
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('No 2D canvas');
    this.ctx = ctx;
  }

  /** Sizes the canvas to its box and fits the domain in; true if it changed. */
  fit(domain: [number, number]): boolean {
    const box = this.canvas.parentElement!.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.floor(box.width));
    const h = Math.max(1, Math.floor(box.height));
    const changed = w !== this.cssW || h !== this.cssH || dpr !== this.dpr;
    if (changed) {
      this.cssW = w;
      this.cssH = h;
      this.dpr = dpr;
      this.canvas.width = Math.round(w * dpr);
      this.canvas.height = Math.round(h * dpr);
      this.canvas.style.width = `${w}px`;
      this.canvas.style.height = `${h}px`;
    }
    const margin = 16;
    const scale = Math.max(
      0.01,
      Math.min((this.cssW - 2 * margin) / domain[0], (this.cssH - 2 * margin) / domain[1]),
    ) * this.zoom;
    const [cx, cy] = this.center ?? [domain[0] / 2, domain[1] / 2];
    const ox = this.cssW / 2 - cx * scale;
    const oy = this.cssH / 2 - cy * scale;
    const moved = scale !== this.scale || ox !== this.ox || oy !== this.oy;
    this.scale = scale;
    this.ox = ox;
    this.oy = oy;
    return changed || moved;
  }

  toDomain(clientX: number, clientY: number): Vec {
    const r = this.canvas.getBoundingClientRect();
    return [(clientX - r.left - this.ox) / this.scale, (clientY - r.top - this.oy) / this.scale];
  }

  /** `n` screen pixels in domain units. */
  px(n = 1): number {
    return n / this.scale;
  }

  draw(s: DrawState): void {
    const ctx = this.ctx;
    const [dw, dh] = s.domain;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    const k = this.scale * this.dpr;
    ctx.setTransform(k, 0, 0, k, this.ox * this.dpr, this.oy * this.dpr);
    const px = this.px();

    ctx.save();
    ctx.beginPath();
    ctx.rect(0, 0, dw, dh);
    ctx.clip();
    if (s.field) {
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = 'high';
      ctx.drawImage(s.field, 0, 0, dw, dh);
    } else {
      ctx.fillStyle = '#0e1a26';
      ctx.fillRect(0, 0, dw, dh);
    }
    if (s.backdrop) {
      const [x, y, w, h] = s.backdrop.rect;
      ctx.globalAlpha = 0.5;
      ctx.drawImage(s.backdrop.image, x, y, w, h);
      ctx.globalAlpha = 1;
    }
    if (s.arrows) this.drawArrows(s.arrows, dw, dh);
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    if (s.show.recorded && s.recorded) {
      ctx.strokeStyle = C.recorded;
      ctx.lineWidth = 1.1 * px;
      ctx.stroke(s.recorded.path);
    }
    if (s.show.live && s.live) {
      ctx.strokeStyle = C.live;
      ctx.lineWidth = 1.3 * px;
      ctx.stroke(s.live.path);
    }
    if (s.selectedPath !== null && s.recorded) {
      ctx.strokeStyle = C.select;
      ctx.lineWidth = 3 * px;
      ctx.setLineDash([6 * px, 4 * px]);
      ctx.stroke(onePath(s.recorded, s.selectedPath));
      ctx.setLineDash([]);
    }
    if (s.show.particles && s.trails) {
      ctx.strokeStyle = C.trail;
      ctx.lineWidth = 1.2 * px;
      ctx.stroke(s.trails.path);
    }
    if (s.show.particles && s.heads && s.heads.length) {
      ctx.fillStyle = C.head;
      const r = 2 * px;
      ctx.beginPath();
      for (let i = 0; i < s.heads.length; i += 2) {
        ctx.moveTo(s.heads[i] + r, s.heads[i + 1]);
        ctx.arc(s.heads[i], s.heads[i + 1], r, 0, Math.PI * 2);
      }
      ctx.fill();
    }
    if (s.show.objects) {
      for (const o of s.objects) this.drawObject(o, o.id === s.selection, o.id === s.hover);
    }
    ctx.restore();

    ctx.strokeStyle = C.frame;
    ctx.lineWidth = px;
    ctx.strokeRect(0, 0, dw, dh);

    if (s.brush) {
      ctx.strokeStyle = 'rgba(255,255,255,0.6)';
      ctx.setLineDash([4 * px, 4 * px]);
      ctx.lineWidth = px;
      ctx.beginPath();
      ctx.arc(s.brush.pos[0], s.brush.pos[1], s.brush.radius, 0, Math.PI * 2);
      ctx.stroke();
      ctx.setLineDash([]);
    }
    const selected = s.objects.find((o) => o.id === s.selection);
    if (selected && s.show.objects) this.drawHandles(selected);
  }

  private drawArrows(a: Arrows, dw: number, dh: number): void {
    const ctx = this.ctx;
    const cw = dw / a.cols;
    const ch = dh / a.rows;
    const cell = Math.min(cw, ch);
    const scale = (cell * 0.9) / Math.max(a.reference, 1e-3);
    ctx.strokeStyle = C.arrow;
    ctx.lineWidth = this.px(1);
    ctx.beginPath();
    for (let j = 0; j < a.rows; j++) {
      for (let i = 0; i < a.cols; i++) {
        const k = 2 * (i + j * a.cols);
        const vx = a.data[k];
        const vy = a.data[k + 1];
        const len = Math.hypot(vx, vy) * scale;
        if (len < this.px(1.5)) continue;
        const clamp = Math.min(len, cell * 1.4) / len;
        const x = (i + 0.5) * cw;
        const y = (j + 0.5) * ch;
        const ex = x + vx * scale * clamp;
        const ey = y + vy * scale * clamp;
        ctx.moveTo(x, y);
        ctx.lineTo(ex, ey);
        const ang = Math.atan2(vy, vx);
        const head = Math.min(len * clamp * 0.35, cell * 0.3);
        ctx.moveTo(ex, ey);
        ctx.lineTo(ex - Math.cos(ang - 0.5) * head, ey - Math.sin(ang - 0.5) * head);
        ctx.moveTo(ex, ey);
        ctx.lineTo(ex - Math.cos(ang + 0.5) * head, ey - Math.sin(ang + 0.5) * head);
      }
    }
    ctx.stroke();
  }

  private ring(pos: Vec, r: number, colour: string, dashed: boolean, width = 1.2): void {
    const ctx = this.ctx;
    ctx.strokeStyle = colour;
    ctx.lineWidth = this.px(width);
    if (dashed) ctx.setLineDash([this.px(5), this.px(4)]);
    ctx.beginPath();
    ctx.arc(pos[0], pos[1], Math.max(r, 0.1), 0, Math.PI * 2);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  private dot(pos: Vec, r: number, colour: string): void {
    const ctx = this.ctx;
    ctx.fillStyle = colour;
    ctx.beginPath();
    ctx.arc(pos[0], pos[1], r, 0, Math.PI * 2);
    ctx.fill();
  }

  private arrowHead(tip: Vec, angleDeg: number, size: number): void {
    const ctx = this.ctx;
    const a = angleDeg * DEG;
    ctx.beginPath();
    ctx.moveTo(tip[0], tip[1]);
    ctx.lineTo(tip[0] - Math.cos(a - 0.45) * size, tip[1] - Math.sin(a - 0.45) * size);
    ctx.moveTo(tip[0], tip[1]);
    ctx.lineTo(tip[0] - Math.cos(a + 0.45) * size, tip[1] - Math.sin(a + 0.45) * size);
    ctx.stroke();
  }

  private drawObject(o: Obj, selected: boolean, hover: boolean): void {
    const ctx = this.ctx;
    const px = this.px();
    const emphasis = selected || hover;
    const lw = (emphasis ? 2 : 1.2) * px;
    switch (o.kind) {
      case 'pebble':
        ctx.fillStyle = C.obstacleFill;
        ctx.strokeStyle = emphasis ? C.select : C.obstacleLine;
        ctx.lineWidth = lw;
        ctx.beginPath();
        ctx.arc(o.pos[0], o.pos[1], o.radius, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
        break;
      case 'rect': {
        const corners = [
          [-1, -1],
          [1, -1],
          [1, 1],
          [-1, 1],
        ].map(([x, y]) => rotate([(x * o.size[0]) / 2, (y * o.size[1]) / 2], o.angle));
        ctx.fillStyle = C.obstacleFill;
        ctx.strokeStyle = emphasis ? C.select : C.obstacleLine;
        ctx.lineWidth = lw;
        ctx.beginPath();
        corners.forEach((c, i) =>
          i ? ctx.lineTo(o.pos[0] + c[0], o.pos[1] + c[1]) : ctx.moveTo(o.pos[0] + c[0], o.pos[1] + c[1]),
        );
        ctx.closePath();
        ctx.fill();
        ctx.stroke();
        break;
      }
      case 'wall': {
        ctx.beginPath();
        o.points.forEach((p, i) => (i ? ctx.lineTo(p[0], p[1]) : ctx.moveTo(p[0], p[1])));
        if (o.points.length === 1) ctx.lineTo(o.points[0][0] + 0.01, o.points[0][1]);
        ctx.strokeStyle = emphasis ? C.select : C.obstacleLine;
        ctx.lineWidth = o.thickness + 2 * lw;
        ctx.stroke();
        ctx.strokeStyle = C.obstacleFill;
        ctx.lineWidth = o.thickness;
        ctx.stroke();
        break;
      }
      case 'jet': {
        this.ring(o.pos, o.radius, C.stirrer, true, emphasis ? 1.8 : 1);
        const len = jetLength(o.radius, px);
        const dir = rotate([1, 0], o.angle);
        const tip: Vec = [o.pos[0] + dir[0] * len, o.pos[1] + dir[1] * len];
        ctx.strokeStyle = C.stirrer;
        ctx.lineWidth = (emphasis ? 2.5 : 2) * px;
        ctx.beginPath();
        ctx.moveTo(o.pos[0], o.pos[1]);
        ctx.lineTo(tip[0], tip[1]);
        ctx.stroke();
        this.arrowHead(tip, o.angle, 10 * px);
        this.dot(o.pos, 3.5 * px, C.stirrer);
        break;
      }
      case 'vortex': {
        this.ring(o.pos, o.radius, C.stirrer, true, emphasis ? 1.8 : 1);
        const r = Math.min(o.radius * 0.6, 26 * px);
        const cw = o.spin >= 0;
        ctx.strokeStyle = C.stirrer;
        ctx.lineWidth = 2 * px;
        ctx.beginPath();
        const start = cw ? -0.5 * Math.PI : 0.5 * Math.PI;
        const end = start + (cw ? 1.5 : -1.5) * Math.PI;
        ctx.arc(o.pos[0], o.pos[1], r, start, end, !cw);
        ctx.stroke();
        const tip: Vec = [o.pos[0] + Math.cos(end) * r, o.pos[1] + Math.sin(end) * r];
        this.arrowHead(tip, end / DEG + (cw ? 90 : -90), 8 * px);
        this.dot(o.pos, 2.5 * px, C.stirrer);
        break;
      }
      case 'source': {
        const colour = o.speed >= 0 ? C.source : C.sink;
        this.ring(o.pos, o.radius, colour, true, emphasis ? 1.8 : 1);
        ctx.strokeStyle = colour;
        ctx.lineWidth = 2 * px;
        const s = 7 * px;
        ctx.beginPath();
        ctx.moveTo(o.pos[0] - s, o.pos[1]);
        ctx.lineTo(o.pos[0] + s, o.pos[1]);
        if (o.speed >= 0) {
          ctx.moveTo(o.pos[0], o.pos[1] - s);
          ctx.lineTo(o.pos[0], o.pos[1] + s);
        }
        ctx.stroke();
        if (o.moving) this.puck(o.pos, colour);
        break;
      }
      case 'attractor': {
        const colour = o.strength >= 0 ? C.pull : C.push;
        ctx.fillStyle = o.strength >= 0 ? 'rgba(255, 77, 109, 0.1)' : 'rgba(90, 169, 255, 0.1)';
        ctx.beginPath();
        ctx.arc(o.pos[0], o.pos[1], o.radius, 0, Math.PI * 2);
        ctx.fill();
        this.ring(o.pos, o.radius, colour, true, emphasis ? 2 : 1.5);
        if (o.capture > 0) this.ring(o.pos, o.capture, colour, false, 1);
        this.dot(o.pos, 4 * px, colour);
        if (o.spin !== 0) {
          const r = 12 * px;
          const cw = o.spin > 0;
          ctx.strokeStyle = colour;
          ctx.lineWidth = 1.5 * px;
          ctx.beginPath();
          const start = cw ? Math.PI : 0;
          const end = start + (cw ? 1 : -1) * Math.PI * 0.9;
          ctx.arc(o.pos[0], o.pos[1], r, start, end, !cw);
          ctx.stroke();
        }
        if (o.moving) this.puck(o.pos, colour);
        break;
      }
      case 'seed_point':
        this.ring(o.pos, 6 * px, C.seed, false, emphasis ? 2 : 1.2);
        this.dot(o.pos, 2.5 * px, C.seed);
        break;
      case 'seed_line': {
        ctx.strokeStyle = C.seed;
        ctx.lineWidth = lw;
        ctx.setLineDash([this.px(4), this.px(3)]);
        ctx.beginPath();
        ctx.moveTo(o.a[0], o.a[1]);
        ctx.lineTo(o.b[0], o.b[1]);
        ctx.stroke();
        ctx.setLineDash([]);
        const n = Math.max(1, Math.round(o.count));
        for (let i = 0; i < n; i++) {
          const t = n === 1 ? 0.5 : i / (n - 1);
          this.dot([o.a[0] + (o.b[0] - o.a[0]) * t, o.a[1] + (o.b[1] - o.a[1]) * t], 2.2 * px, C.seed);
        }
        break;
      }
      case 'seed_ring': {
        this.ring(o.pos, o.radius, C.seed, true, emphasis ? 1.8 : 1);
        const n = Math.max(1, Math.round(o.count));
        for (let i = 0; i < n; i++) {
          const t = (i / n) * Math.PI * 2;
          this.dot([o.pos[0] + Math.cos(t) * o.radius, o.pos[1] + Math.sin(t) * o.radius], 2.2 * px, C.seed);
        }
        break;
      }
    }
  }

  /** A mover's mark: a ring round its centre, like a particle's head. */
  private puck(pos: Vec, colour: string): void {
    const ctx = this.ctx;
    const px = this.px();
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 2.5 * px;
    ctx.beginPath();
    ctx.arc(pos[0], pos[1], 9 * px, 0, Math.PI * 2);
    ctx.stroke();
    ctx.strokeStyle = colour;
    ctx.lineWidth = 1.5 * px;
    ctx.stroke();
  }

  private drawHandles(o: Obj): void {
    const ctx = this.ctx;
    const px = this.px();
    const s = 4.5 * px;
    for (const h of handles(o, px)) {
      ctx.fillStyle = '#ffffff';
      ctx.strokeStyle = '#111';
      ctx.lineWidth = px;
      ctx.beginPath();
      if (h.id === 'rotate' || h.id === 'aim') ctx.arc(h.pos[0], h.pos[1], s, 0, Math.PI * 2);
      else ctx.rect(h.pos[0] - s, h.pos[1] - s, 2 * s, 2 * s);
      ctx.fill();
      ctx.stroke();
    }
  }
}
