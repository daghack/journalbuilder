//! The incompressible flow: advection, forcing, and the pressure projection
//! that keeps it divergence-free.

use crate::geom::{Rng, V2, v2};
use crate::grid::{Grid, Layout, sample_linear, sample_linear_bounds};
use crate::objects::{Kind, Mask, Object, Obstacles};
use std::rc::Rc;
use crate::scene::{DyeInflow, Edge, Edges, FlowSettings};

/// How fast stirrers and stirring pull the fluid toward their target
/// velocity, a share a second at their centre.
const STIRRER_RESPONSE: f32 = 6.0;
const STIR_RESPONSE: f32 = 25.0;

/// A push on the fluid: a jet, a vortex or the mouse.
#[derive(Clone, Copy, Debug)]
pub struct Push {
    pub pos: V2,
    pub radius: f32,
    pub kind: PushKind,
    /// A share a second.
    pub response: f32,
    /// Whether it leaves dye.
    pub dye: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum PushKind {
    /// Toward this velocity.
    Toward(V2),
    /// Round its centre, this fast at its radius (clockwise when positive).
    Spin(f32),
}

impl Push {
    pub fn stir(pos: V2, velocity: V2, radius: f32) -> Self {
        Push { pos, radius: radius.max(1.0), kind: PushKind::Toward(velocity), response: STIR_RESPONSE, dye: true }
    }

    /// The stirrers among the objects (sources aside: they set divergence).
    pub fn from_objects(objects: &[Object]) -> Vec<Push> {
        objects
            .iter()
            .filter_map(|o| match o.kind {
                Kind::Jet { pos, angle, speed, radius } => Some(Push {
                    pos,
                    radius: radius.max(1.0),
                    kind: PushKind::Toward(v2(1.0, 0.0).rotated(angle.to_radians()) * speed),
                    response: STIRRER_RESPONSE,
                    dye: false,
                }),
                Kind::Vortex { pos, spin, radius } => Some(Push {
                    pos,
                    radius: radius.max(1.0),
                    kind: PushKind::Spin(spin),
                    response: STIRRER_RESPONSE,
                    dye: false,
                }),
                _ => None,
            })
            .collect()
    }

    fn target(&self, d: V2) -> V2 {
        match self.kind {
            PushKind::Toward(t) => t,
            PushKind::Spin(s) => d.perp() * (s / self.radius),
        }
    }

    /// The speed it pushes toward, to scale the views by.
    pub fn speed(&self) -> f32 {
        match self.kind {
            PushKind::Toward(t) => t.len(),
            PushKind::Spin(s) => s.abs(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Fluid {
    pub g: Grid,
    u0: Vec<f32>,
    v0: Vec<f32>,
    d0: Vec<f32>,
    scratch: Vec<f32>,
    scratch2: Vec<f32>,
    /// Where sources pour dye in: cell and amount.
    source_dye: Vec<(usize, f32)>,
    rng: Rng,
    sway: f32,
    pcg: Pcg,
    /// How many iterations the last pressure solve took.
    pub last_iterations: usize,
    /// Whether the solids changed since the pressure equations were built.
    matrix_dirty: bool,
}

impl Fluid {
    pub fn new(nx: usize, ny: usize, h: f32) -> Self {
        let g = Grid::new(nx, ny, h);
        Fluid {
            u0: g.u.clone(),
            v0: g.v.clone(),
            d0: g.dye.clone(),
            scratch: Vec::new(),
            scratch2: Vec::new(),
            source_dye: Vec::new(),
            rng: Rng::new(0x5eed_f10e),
            sway: 0.0,
            pcg: Pcg::default(),
            last_iterations: 0,
            matrix_dirty: true,
            g,
        }
    }

    /// Marks the cells the obstacles (and the mask, if there is one)
    /// cover. Each obstacle covers at least one cell, so thin walls don't
    /// leak.
    pub fn set_solids(&mut self, objects: &[Object], mask: Option<&Rc<Mask>>) {
        let g = &mut self.g;
        let h = g.h;
        let thick = Obstacles::new(&thickened(objects, 0.5 * h)).with_mask(mask, 0.5 * h);
        self.matrix_dirty = true;
        for j in 0..g.ny {
            for i in 0..g.nx {
                let c = v2((i as f32 + 0.5) * h, (j as f32 + 0.5) * h);
                let idx = g.ci(i, j);
                g.solid[idx] = thick.sdf(c) < 0.0;
                if g.solid[idx] {
                    g.dye[idx] = 0.0;
                    g.q[idx] = 0.0;
                }
            }
        }
        self.zero_solid_faces();
    }

    /// Sets the divergence sources and sinks ask for.
    pub fn set_sources(&mut self, objects: &[Object]) {
        let g = &mut self.g;
        g.div_target.iter_mut().for_each(|d| *d = 0.0);
        self.source_dye.clear();
        let h = g.h;
        for o in objects {
            let Kind::Source { pos, speed, radius, .. } = o.kind else { continue };
            let r = radius.max(h * 0.5);
            // The flux out of the circle at `speed`, spread over a Gaussian.
            let flux = speed * std::f32::consts::TAU * r;
            let reach = (r * 2.0 / h).ceil() as isize + 1;
            let ci = (pos.x / h) as isize;
            let cj = (pos.y / h) as isize;
            let mut cells = Vec::new();
            let mut total = 0.0;
            for j in (cj - reach)..=(cj + reach) {
                for i in (ci - reach)..=(ci + reach) {
                    if i < 0 || j < 0 || i >= g.nx as isize || j >= g.ny as isize {
                        continue;
                    }
                    let (i, j) = (i as usize, j as usize);
                    let idx = g.ci(i, j);
                    if g.solid[idx] {
                        continue;
                    }
                    let c = v2((i as f32 + 0.5) * h, (j as f32 + 0.5) * h);
                    let w = (-(c - pos).len2() / (r * r)).exp();
                    if w > 1e-3 {
                        cells.push((idx, w));
                        total += w;
                    }
                }
            }
            if total <= 0.0 {
                continue;
            }
            for (idx, w) in cells {
                g.div_target[idx] += flux * w / (total * h * h);
                if speed > 0.0 {
                    self.source_dye.push((idx, w));
                }
            }
        }
    }

    /// Fills faces that were solid under `was_solid` (a cell mask) but are
    /// fluid now from the faces round them, front by front, so a moved
    /// obstacle doesn't leave a still patch behind.
    pub fn fill_uncovered(&mut self, was_solid: &[bool]) {
        let g = &mut self.g;
        let (nx, ny) = (g.nx, g.ny);
        if was_solid.len() != nx * ny {
            return;
        }
        let was = |i: usize, j: usize| was_solid[i + j * nx];
        let mut u_known: Vec<bool> = Vec::with_capacity(g.u.len());
        for j in 0..ny {
            for i in 0..=nx {
                let before = (i > 0 && was(i - 1, j)) || (i < nx && was(i, j));
                u_known.push(!before || g.u_solid(i, j));
            }
        }
        let mut v_known: Vec<bool> = Vec::with_capacity(g.v.len());
        for j in 0..=ny {
            for i in 0..nx {
                let before = (j > 0 && was(i, j - 1)) || (j < ny && was(i, j));
                v_known.push(!before || g.v_solid(i, j));
            }
        }
        extrapolate(&mut g.u, &mut u_known, nx + 1, ny);
        extrapolate(&mut g.v, &mut v_known, nx, ny + 1);
    }

    pub fn zero_solid_faces(&mut self) {
        let g = &mut self.g;
        for j in 0..g.ny {
            for i in 0..=g.nx {
                if g.u_solid(i, j) {
                    let k = g.ui(i, j);
                    g.u[k] = 0.0;
                }
            }
        }
        for j in 0..=g.ny {
            for i in 0..g.nx {
                if g.v_solid(i, j) {
                    let k = g.vi(i, j);
                    g.v[k] = 0.0;
                }
            }
        }
    }

    pub fn reset(&mut self) {
        let g = &mut self.g;
        g.u.iter_mut().for_each(|x| *x = 0.0);
        g.v.iter_mut().for_each(|x| *x = 0.0);
        g.q.iter_mut().for_each(|x| *x = 0.0);
        g.dye.iter_mut().for_each(|x| *x = 0.0);
        self.sway = 0.0;
    }

    /// One step of `dt` seconds, split in a few if the flow is fast.
    pub fn step(&mut self, dt: f32, s: &FlowSettings, pushes: &[Push]) {
        if dt <= 0.0 {
            return;
        }
        let cfl = 2.0;
        let fastest = self.g.max_speed().max(s.inflow_speed.abs());
        let n = ((fastest * dt / (self.g.h * cfl)).ceil() as usize).clamp(1, 4);
        let sub = dt / n as f32;
        for _ in 0..n {
            self.substep(sub, s, pushes);
        }
    }

    fn substep(&mut self, dt: f32, s: &FlowSettings, pushes: &[Push]) {
        self.apply_boundaries(s);
        self.advect(dt);
        for p in pushes {
            self.push(p, dt);
        }
        if s.vorticity > 0.0 {
            self.confine_vorticity(dt, s.vorticity);
        }
        self.sway_inflow(s);
        if s.viscosity > 0.0 {
            self.diffuse(dt, s.viscosity);
        }
        self.apply_boundaries(s);
        self.project(s, false);
        self.dye(dt, s);
        if !self.g.u.iter().chain(self.g.v.iter()).all(|x| x.is_finite()) {
            self.reset();
            self.apply_boundaries(s);
        }
    }

    /// Fixes the faces on the domain's edges, and solid faces.
    pub fn apply_boundaries(&mut self, s: &FlowSettings) {
        let speed = s.inflow_speed;
        let e = s.edges;
        let g = &mut self.g;
        let (nx, ny) = (g.nx, g.ny);
        for j in 0..ny {
            let (l, l1) = (g.ui(0, j), g.ui(1, j));
            g.u[l] = match e.left {
                Edge::Wall => 0.0,
                Edge::Inflow => speed,
                Edge::Open => g.u[l1],
            };
            let (r, r1) = (g.ui(nx, j), g.ui(nx - 1, j));
            g.u[r] = match e.right {
                Edge::Wall => 0.0,
                Edge::Inflow => -speed,
                Edge::Open => g.u[r1],
            };
        }
        for i in 0..nx {
            let (t, t1) = (g.vi(i, 0), g.vi(i, 1));
            g.v[t] = match e.top {
                Edge::Wall => 0.0,
                Edge::Inflow => speed,
                Edge::Open => g.v[t1],
            };
            let (b, b1) = (g.vi(i, ny), g.vi(i, ny - 1));
            g.v[b] = match e.bottom {
                Edge::Wall => 0.0,
                Edge::Inflow => -speed,
                Edge::Open => g.v[b1],
            };
        }
        self.zero_solid_faces();
    }

    /// A slow random sway across each inflow: the inflow comes in a little
    /// askew, which breaks the symmetry wakes need to start shedding.
    fn sway_inflow(&mut self, s: &FlowSettings) {
        if s.perturbation <= 0.0 {
            return;
        }
        self.sway = (self.sway * 0.98 + 0.1 * self.rng.signed()).clamp(-1.0, 1.0);
        let amount = self.sway * s.perturbation * s.inflow_speed;
        let e = s.edges;
        let g = &mut self.g;
        let (nx, ny) = (g.nx, g.ny);
        for j in 1..ny {
            if e.left == Edge::Inflow {
                let idx = g.vi(0, j);
                g.v[idx] = amount;
            }
            if e.right == Edge::Inflow {
                let idx = g.vi(nx - 1, j);
                g.v[idx] = amount;
            }
        }
        for i in 1..nx {
            if e.top == Edge::Inflow {
                let idx = g.ui(i, 0);
                g.u[idx] = amount;
            }
            if e.bottom == Edge::Inflow {
                let idx = g.ui(i, ny - 1);
                g.u[idx] = amount;
            }
        }
    }

    /// Relaxes the fluid near a push toward its target velocity.
    pub fn push(&mut self, p: &Push, dt: f32) {
        let g = &mut self.g;
        let h = g.h;
        let reach = p.radius * 2.5;
        let r2 = p.radius * p.radius;
        let i0 = (((p.pos.x - reach) / h).floor().max(0.0)) as usize;
        let i1 = (((p.pos.x + reach) / h).ceil() as usize).min(g.nx);
        let j0 = (((p.pos.y - reach) / h).floor().max(0.0)) as usize;
        let j1 = (((p.pos.y + reach) / h).ceil() as usize).min(g.ny);
        for j in j0..j1.min(g.ny) {
            for i in i0..=i1.min(g.nx) {
                if g.u_solid(i, j) {
                    continue;
                }
                let d = v2(i as f32 * h, (j as f32 + 0.5) * h) - p.pos;
                let w = (-d.len2() / r2).exp();
                if w < 1e-3 {
                    continue;
                }
                let k = g.ui(i, j);
                let blend = 1.0 - (-p.response * dt * w).exp();
                g.u[k] += (p.target(d).x - g.u[k]) * blend;
            }
        }
        for j in j0..=j1.min(g.ny) {
            for i in i0..i1.min(g.nx) {
                if g.v_solid(i, j) {
                    continue;
                }
                let d = v2((i as f32 + 0.5) * h, j as f32 * h) - p.pos;
                let w = (-d.len2() / r2).exp();
                if w < 1e-3 {
                    continue;
                }
                let k = g.vi(i, j);
                let blend = 1.0 - (-p.response * dt * w).exp();
                g.v[k] += (p.target(d).y - g.v[k]) * blend;
            }
        }
        if p.dye {
            for j in j0..j1.min(g.ny) {
                for i in i0..i1.min(g.nx) {
                    let idx = g.ci(i, j);
                    if g.solid[idx] {
                        continue;
                    }
                    let d = v2((i as f32 + 0.5) * h, (j as f32 + 0.5) * h) - p.pos;
                    let w = (-d.len2() / r2).exp();
                    g.dye[idx] = g.dye[idx].max(w.min(1.0));
                }
            }
        }
    }

    /// MacCormack advection of velocity and dye through the velocity at the
    /// start of the step.
    fn advect(&mut self, dt: f32) {
        self.u0.clone_from(&self.g.u);
        self.v0.clone_from(&self.g.v);
        self.d0.clone_from(&self.g.dye);
        let (ul, vl, cl) = (self.g.u_layout(), self.g.v_layout(), self.g.c_layout());
        let h = self.g.h;
        let flow = Frozen { u: &self.u0, v: &self.v0, ul, vl, inv_h: 1.0 / h };

        maccormack(&flow, &self.u0, &mut self.g.u, ul, h, dt, &mut self.scratch, &mut self.scratch2);
        maccormack(&flow, &self.v0, &mut self.g.v, vl, h, dt, &mut self.scratch, &mut self.scratch2);
        maccormack(&flow, &self.d0, &mut self.g.dye, cl, h, dt, &mut self.scratch, &mut self.scratch2);
    }

    /// Vorticity confinement: a push round each swirl's core, which numerical
    /// diffusion would otherwise smear out.
    fn confine_vorticity(&mut self, dt: f32, eps: f32) {
        let g = &mut self.g;
        let (nx, ny, h) = (g.nx, g.ny, g.h);
        g.vorticity(&mut self.scratch);
        let w = &self.scratch;
        self.scratch2.clear();
        self.scratch2.resize(nx * ny * 2, 0.0);
        let f = &mut self.scratch2;
        for j in 2..ny - 2 {
            for i in 2..nx - 2 {
                let c = g.ci(i, j);
                let gx = (w[c + 1].abs() - w[c - 1].abs()) * 0.5 / h;
                let gy = (w[c + nx].abs() - w[c - nx].abs()) * 0.5 / h;
                let len = (gx * gx + gy * gy).sqrt() + 1e-6;
                let (nxv, nyv) = (gx / len, gy / len);
                f[2 * c] = eps * h * nyv * w[c];
                f[2 * c + 1] = -eps * h * nxv * w[c];
            }
        }
        for j in 0..ny {
            for i in 1..nx {
                if g.u_solid(i, j) {
                    continue;
                }
                let (a, b) = (g.ci(i - 1, j), g.ci(i, j));
                let k = g.ui(i, j);
                g.u[k] += dt * 0.5 * (f[2 * a] + f[2 * b]);
            }
        }
        for j in 1..ny {
            for i in 0..nx {
                if g.v_solid(i, j) {
                    continue;
                }
                let (a, b) = (g.ci(i, j - 1), g.ci(i, j));
                let k = g.vi(i, j);
                g.v[k] += dt * 0.5 * (f[2 * a + 1] + f[2 * b + 1]);
            }
        }
    }

    /// Implicit viscosity, by a few Jacobi sweeps.
    fn diffuse(&mut self, dt: f32, nu: f32) {
        let alpha = nu * dt / (self.g.h * self.g.h);
        if alpha <= 0.0 {
            return;
        }
        let (ul, vl) = (self.g.u_layout(), self.g.v_layout());
        jacobi_diffuse(&mut self.g.u, ul, alpha, &mut self.scratch);
        jacobi_diffuse(&mut self.g.v, vl, alpha, &mut self.scratch);
        self.zero_solid_faces();
    }

    fn dye(&mut self, dt: f32, s: &FlowSettings) {
        let g = &mut self.g;
        let fade = (1.0 - s.dye_fade.max(0.0) * dt).clamp(0.0, 1.0);
        g.dye.iter_mut().for_each(|d| *d *= fade);
        for &(idx, w) in &self.source_dye {
            g.dye[idx] = g.dye[idx].max(w);
        }
        if s.dye == DyeInflow::Stripes {
            let (nx, ny) = (g.nx, g.ny);
            let band_y = (ny / 24).max(2);
            let band_x = (nx / 24).max(2);
            let e = s.edges;
            for j in 0..ny {
                let on = if (j / band_y) % 2 == 0 { 1.0 } else { 0.0 };
                if e.left == Edge::Inflow {
                    let k = g.ci(0, j);
                    g.dye[k] = on;
                }
                if e.right == Edge::Inflow {
                    let k = g.ci(nx - 1, j);
                    g.dye[k] = on;
                }
            }
            for i in 0..nx {
                let on = if (i / band_x) % 2 == 0 { 1.0 } else { 0.0 };
                if e.top == Edge::Inflow {
                    let k = g.ci(i, 0);
                    g.dye[k] = on;
                }
                if e.bottom == Edge::Inflow {
                    let k = g.ci(i, ny - 1);
                    g.dye[k] = on;
                }
            }
        }
        for (d, &solid) in g.dye.iter_mut().zip(&g.solid) {
            if solid {
                *d = 0.0;
            }
        }
    }

    /// The divergence of each fluid cell, less what it should be.
    pub fn divergence_error(&self) -> f32 {
        let g = &self.g;
        let mut worst = 0.0f32;
        for j in 0..g.ny {
            for i in 0..g.nx {
                let c = g.ci(i, j);
                if g.solid[c] {
                    continue;
                }
                let div = (g.u[g.ui(i + 1, j)] - g.u[g.ui(i, j)] + g.v[g.vi(i, j + 1)] - g.v[g.vi(i, j)]) / g.h;
                worst = worst.max((div - g.div_target[c]).abs());
            }
        }
        worst
    }

    /// Makes the flow divergence-free (apart from sources and sinks), by
    /// MIC(0)-preconditioned conjugate gradients from the last pressure: to
    /// a loose tolerance in at most `pressure_iterations` while running, or,
    /// when `accurate`, to a tight one.
    ///
    /// With q the pressure scaled by dt/ρ, each face loses the difference in
    /// q across it over h, and for each fluid cell Σ (q − q_neighbour) =
    /// (target − divergence)·h², over its neighbours that aren't solid or a
    /// closed edge; beyond an open edge q is 0.
    pub fn project(&mut self, s: &FlowSettings, accurate: bool) {
        self.project_to(s, if accurate { Accuracy::Full } else { Accuracy::Step });
    }

    pub fn project_to(&mut self, s: &FlowSettings, accuracy: Accuracy) {
        let open = s.edges;
        let (nx, ny, h) = (self.g.nx, self.g.ny, self.g.h);
        if self.matrix_dirty || self.pcg.edges != Some(open) || self.pcg.nx != nx || self.pcg.ny != ny {
            self.pcg.build(&self.g, open);
            self.matrix_dirty = false;
        }
        let g = &self.g;
        let pcg = &mut self.pcg;
        let pad = pcg.pad;
        let mut sum = 0.0f64;
        let mut count = 0usize;
        for j in 0..ny {
            for i in 0..nx {
                let c = g.ci(i, j);
                let k = c + pad;
                if pcg.diag[k] <= 0.0 {
                    pcg.b[k] = 0.0;
                    pcg.x[k] = 0.0;
                    continue;
                }
                let flux = g.u[g.ui(i + 1, j)] - g.u[g.ui(i, j)] + g.v[g.vi(i, j + 1)] - g.v[g.vi(i, j)];
                let b = (g.div_target[c] * h - flux) as f64 * h as f64;
                pcg.b[k] = b;
                pcg.x[k] = g.q[c] as f64;
                sum += b;
                count += 1;
            }
        }
        if !open.any_open() && count > 0 {
            // A closed box: only a balanced right-hand side has a solution.
            let mean = sum / count as f64;
            for k in pad..pad + nx * ny {
                if pcg.diag[k] > 0.0 {
                    pcg.b[k] -= mean;
                }
            }
        }
        // A residual r leaves a divergence of about r/h², so a speed error of
        // about r/h: solve to a share of the fastest speed.
        let speed = g.max_speed().max(s.inflow_speed.abs()).max(1.0) as f64;
        let steps = s.pressure_iterations.max(1) as usize;
        self.last_iterations = match accuracy {
            Accuracy::Full => pcg.solve(1e-6 * speed * h as f64, 2000),
            Accuracy::Step => pcg.solve(2e-3 * speed * h as f64, steps),
            Accuracy::Rough => pcg.solve(2e-3 * speed * h as f64, steps.max(60)),
        };
        let g = &mut self.g;
        for (c, q) in g.q.iter_mut().enumerate() {
            *q = pcg.x[c + pad] as f32;
        }
        // Take the pressure's gradient off each face it can change.
        let q = &g.q;
        let inv_h = 1.0 / h;
        for j in 0..ny {
            for i in 0..=nx {
                if g.u_solid(i, j) {
                    continue;
                }
                let k = g.ui(i, j);
                let row = j * nx;
                let dq = if i == 0 {
                    if open.left == Edge::Open { q[row] } else { 0.0 }
                } else if i == nx {
                    if open.right == Edge::Open { -q[row + nx - 1] } else { 0.0 }
                } else {
                    q[row + i] - q[row + i - 1]
                };
                g.u[k] -= dq * inv_h;
            }
        }
        for j in 0..=ny {
            for i in 0..nx {
                if g.v_solid(i, j) {
                    continue;
                }
                let k = g.vi(i, j);
                let dq = if j == 0 {
                    if open.top == Edge::Open { q[i] } else { 0.0 }
                } else if j == ny {
                    if open.bottom == Edge::Open { -q[i + (ny - 1) * nx] } else { 0.0 }
                } else {
                    q[i + j * nx] - q[i + (j - 1) * nx]
                };
                g.v[k] -= dq * inv_h;
            }
        }
    }
}

/// How far a projection solves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accuracy {
    /// To a tight tolerance: divergence-free for tracing.
    Full,
    /// A running step's: a share of the fastest speed, within the settings'
    /// iterations.
    Step,
    /// As a step, with a few more iterations: a frozen flow re-solved every
    /// tick round a moving source.
    Rough,
}

/// The pressure equations and the conjugate gradient solver's work arrays.
/// Every array is a cell's worth per cell, with `pad` zeros either side so
/// neighbours never need bounds: a coefficient to a solid cell, a closed
/// edge or across a row's end is 0.
#[derive(Clone, Debug, Default)]
struct Pcg {
    nx: usize,
    ny: usize,
    pad: usize,
    edges: Option<Edges>,
    /// How many neighbours each fluid cell exchanges with (0 for solid).
    diag: Vec<f64>,
    /// −1 where the cell and the one after it (+x, +y) are both fluid.
    plus_i: Vec<f64>,
    plus_j: Vec<f64>,
    precon: Vec<f64>,
    x: Vec<f64>,
    b: Vec<f64>,
    r: Vec<f64>,
    z: Vec<f64>,
    s: Vec<f64>,
}

impl Pcg {
    fn build(&mut self, g: &Grid, open: Edges) {
        let (nx, ny) = (g.nx, g.ny);
        let pad = nx + 1;
        let len = nx * ny + 2 * pad;
        self.nx = nx;
        self.ny = ny;
        self.pad = pad;
        self.edges = Some(open);
        for v in [&mut self.diag, &mut self.plus_i, &mut self.plus_j, &mut self.precon] {
            v.clear();
            v.resize(len, 0.0);
        }
        for v in [&mut self.x, &mut self.b, &mut self.r, &mut self.z, &mut self.s] {
            v.resize(len, 0.0);
        }
        let fluid = |i: usize, j: usize| !g.solid[g.ci(i, j)];
        for j in 0..ny {
            for i in 0..nx {
                if !fluid(i, j) {
                    continue;
                }
                let k = g.ci(i, j) + pad;
                let mut d = 0.0;
                if i > 0 {
                    d += fluid(i - 1, j) as u8 as f64;
                } else if open.left == Edge::Open {
                    d += 1.0;
                }
                if i + 1 < nx {
                    if fluid(i + 1, j) {
                        d += 1.0;
                        self.plus_i[k] = -1.0;
                    }
                } else if open.right == Edge::Open {
                    d += 1.0;
                }
                if j > 0 {
                    d += fluid(i, j - 1) as u8 as f64;
                } else if open.top == Edge::Open {
                    d += 1.0;
                }
                if j + 1 < ny {
                    if fluid(i, j + 1) {
                        d += 1.0;
                        self.plus_j[k] = -1.0;
                    }
                } else if open.bottom == Edge::Open {
                    d += 1.0;
                }
                self.diag[k] = d;
            }
        }
        // The modified incomplete Cholesky factor.
        let (tau, sigma) = (0.97, 0.25);
        let (pi, pj, p) = (&self.plus_i, &self.plus_j, &mut self.precon);
        for k in pad..pad + nx * ny {
            let d = self.diag[k];
            if d <= 0.0 {
                continue;
            }
            let a = pi[k - 1] * p[k - 1];
            let b = pj[k - nx] * p[k - nx];
            let mut e = d
                - a * a
                - b * b
                - tau * (pi[k - 1] * pj[k - 1] * p[k - 1] * p[k - 1] + pj[k - nx] * pi[k - nx] * p[k - nx] * p[k - nx]);
            if e < sigma * d {
                e = d;
            }
            p[k] = 1.0 / e.sqrt();
        }
    }

    /// Conjugate gradients from `x` until no cell's residual is over
    /// `tolerance`, or `max_iterations`. Gives the iterations it took.
    fn solve(&mut self, tolerance: f64, max_iterations: usize) -> usize {
        let Pcg { nx, ny, pad, diag, plus_i, plus_j, precon, x, b, r, z, s, .. } = self;
        let (nx, pad) = (*nx, *pad);
        let cells = pad..pad + nx * *ny;
        let matvec = |from: &[f64], to: &mut [f64]| {
            for k in cells.clone() {
                to[k] = diag[k] * from[k]
                    + plus_i[k] * from[k + 1]
                    + plus_i[k - 1] * from[k - 1]
                    + plus_j[k] * from[k + nx]
                    + plus_j[k - nx] * from[k - nx];
            }
        };
        let apply_precon = |r: &[f64], z: &mut [f64]| {
            for k in cells.clone() {
                let t = r[k] - plus_i[k - 1] * precon[k - 1] * z[k - 1] - plus_j[k - nx] * precon[k - nx] * z[k - nx];
                z[k] = t * precon[k];
            }
            for k in cells.clone().rev() {
                let t = z[k] - plus_i[k] * precon[k] * z[k + 1] - plus_j[k] * precon[k] * z[k + nx];
                z[k] = t * precon[k];
            }
        };
        matvec(x, r);
        for k in cells.clone() {
            r[k] = if diag[k] > 0.0 { b[k] - r[k] } else { 0.0 };
        }
        if max_abs(r) <= tolerance {
            return 0;
        }
        apply_precon(r, z);
        s.copy_from_slice(z);
        let mut sigma = dot(z, r);
        for it in 1..=max_iterations {
            matvec(s, z);
            let zs = dot(z, s);
            if zs.abs() < 1e-300 {
                return it;
            }
            let alpha = sigma / zs;
            for k in cells.clone() {
                x[k] += alpha * s[k];
                r[k] -= alpha * z[k];
            }
            if max_abs(r) <= tolerance {
                return it;
            }
            apply_precon(r, z);
            let sigma_new = dot(z, r);
            let beta = sigma_new / sigma;
            for k in cells.clone() {
                s[k] = z[k] + beta * s[k];
            }
            sigma = sigma_new;
        }
        max_iterations
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn max_abs(a: &[f64]) -> f64 {
    a.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

/// Velocity read from copies of the grid's faces.
struct Frozen<'a> {
    u: &'a [f32],
    v: &'a [f32],
    ul: Layout,
    vl: Layout,
    inv_h: f32,
}

impl Frozen<'_> {
    #[inline]
    fn at(&self, p: V2) -> V2 {
        v2(sample_linear(self.u, self.ul, self.inv_h, p), sample_linear(self.v, self.vl, self.inv_h, p))
    }
    /// Where the fluid at `p` was `dt` ago (or will be, for negative dt), by
    /// the midpoint rule.
    #[inline]
    fn trace(&self, p: V2, dt: f32) -> V2 {
        let mid = p - self.at(p) * (0.5 * dt);
        p - self.at(mid) * dt
    }
}

/// `field` carried along by `flow` for `dt`, into `out`: a semi-Lagrangian
/// step forward, one back, and half their difference added, clamped to the
/// samples it came from so it can't ring.
#[allow(clippy::too_many_arguments)]
fn maccormack(
    flow: &Frozen,
    field: &[f32],
    out: &mut [f32],
    l: Layout,
    h: f32,
    dt: f32,
    hat: &mut Vec<f32>,
    bounds: &mut Vec<f32>,
) {
    let inv_h = 1.0 / h;
    hat.clear();
    hat.resize(field.len(), 0.0);
    bounds.clear();
    bounds.resize(field.len() * 2, 0.0);
    for j in 0..l.rows {
        for i in 0..l.w {
            let p = v2((i as f32 + l.ox) * h, (j as f32 + l.oy) * h);
            let back = flow.trace(p, dt);
            let (val, lo, hi) = sample_linear_bounds(field, l, inv_h, back);
            let k = i + j * l.w;
            hat[k] = val;
            bounds[2 * k] = lo;
            bounds[2 * k + 1] = hi;
        }
    }
    for j in 0..l.rows {
        for i in 0..l.w {
            let p = v2((i as f32 + l.ox) * h, (j as f32 + l.oy) * h);
            let ahead = flow.trace(p, -dt);
            let returned = sample_linear(hat, l, inv_h, ahead);
            let k = i + j * l.w;
            let corrected = hat[k] + 0.5 * (field[k] - returned);
            out[k] = corrected.clamp(bounds[2 * k], bounds[2 * k + 1]);
        }
    }
}

/// Grows `known` values into the unknown ones, a ring at a time: each takes
/// the mean of its known neighbours.
fn extrapolate(field: &mut [f32], known: &mut [bool], w: usize, rows: usize) {
    let mut front = Vec::new();
    for _ in 0..(w + rows) {
        front.clear();
        for j in 0..rows {
            for i in 0..w {
                let k = i + j * w;
                if known[k] {
                    continue;
                }
                let (mut sum, mut n) = (0.0, 0);
                let mut take = |kk: usize| {
                    if known[kk] {
                        sum += field[kk];
                        n += 1;
                    }
                };
                if i > 0 {
                    take(k - 1);
                }
                if i + 1 < w {
                    take(k + 1);
                }
                if j > 0 {
                    take(k - w);
                }
                if j + 1 < rows {
                    take(k + w);
                }
                if n > 0 {
                    front.push((k, sum / n as f32));
                }
            }
        }
        if front.is_empty() {
            break;
        }
        for &(k, value) in &front {
            field[k] = value;
            known[k] = true;
        }
    }
}

fn jacobi_diffuse(field: &mut [f32], l: Layout, alpha: f32, tmp: &mut Vec<f32>) {
    let (w, rows) = (l.w, l.rows);
    let b = field.to_vec();
    for _ in 0..12 {
        tmp.clear();
        tmp.extend_from_slice(field);
        for j in 0..rows {
            for i in 0..w {
                let k = i + j * w;
                let left = if i > 0 { tmp[k - 1] } else { tmp[k] };
                let right = if i + 1 < w { tmp[k + 1] } else { tmp[k] };
                let up = if j > 0 { tmp[k - w] } else { tmp[k] };
                let down = if j + 1 < rows { tmp[k + w] } else { tmp[k] };
                field[k] = (b[k] + alpha * (left + right + up + down)) / (1.0 + 4.0 * alpha);
            }
        }
    }
}

/// The obstacles with every wall at least `half` thick each side, every
/// pebble at least `half` round and every rect at least `half` each way.
fn thickened(objects: &[Object], half: f32) -> Vec<Object> {
    objects
        .iter()
        .filter_map(|o| {
            let kind = match &o.kind {
                Kind::Pebble { pos, radius } => Kind::Pebble { pos: *pos, radius: radius.max(half) },
                Kind::Rect { pos, size, angle } => Kind::Rect {
                    pos: *pos,
                    size: v2(size.x.abs().max(2.0 * half), size.y.abs().max(2.0 * half)),
                    angle: *angle,
                },
                Kind::Wall { points, thickness } => {
                    Kind::Wall { points: points.clone(), thickness: thickness.max(2.0 * half) }
                }
                _ => return None,
            };
            Some(Object { id: o.id, kind })
        })
        .collect()
}
