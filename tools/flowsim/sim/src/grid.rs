//! The fluid's grid: a MAC (staggered) grid of `nx` × `ny` square cells of
//! side `h`, in domain units, y down.
//!
//! - `u`, the x velocity, sits on the vertical faces: (nx+1) × ny of them,
//!   face (i, j) at (i·h, (j+½)·h).
//! - `v`, the y velocity, sits on the horizontal faces: nx × (ny+1), face
//!   (i, j) at ((i+½)·h, j·h).
//! - `q` (pressure, scaled), `dye`, `solid` and `div_target` are per cell,
//!   cell (i, j) centred at ((i+½)·h, (j+½)·h).

use crate::geom::{V2, v2};

#[derive(Clone, Debug)]
pub struct Grid {
    pub nx: usize,
    pub ny: usize,
    pub h: f32,
    pub u: Vec<f32>,
    pub v: Vec<f32>,
    pub q: Vec<f32>,
    pub dye: Vec<f32>,
    pub solid: Vec<bool>,
    /// What each cell's divergence should be: non-zero only in sources and
    /// sinks.
    pub div_target: Vec<f32>,
}

/// Where one of the grid's fields sits: `w` × `rows` samples, sample (i, j)
/// at ((i + ox)·h, (j + oy)·h).
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub w: usize,
    pub rows: usize,
    pub ox: f32,
    pub oy: f32,
}

impl Grid {
    pub fn new(nx: usize, ny: usize, h: f32) -> Self {
        let nx = nx.max(4);
        let ny = ny.max(4);
        Grid {
            nx,
            ny,
            h,
            u: vec![0.0; (nx + 1) * ny],
            v: vec![0.0; nx * (ny + 1)],
            q: vec![0.0; nx * ny],
            dye: vec![0.0; nx * ny],
            solid: vec![false; nx * ny],
            div_target: vec![0.0; nx * ny],
        }
    }

    pub fn width(&self) -> f32 {
        self.nx as f32 * self.h
    }
    pub fn height(&self) -> f32 {
        self.ny as f32 * self.h
    }

    #[inline]
    pub fn ui(&self, i: usize, j: usize) -> usize {
        i + j * (self.nx + 1)
    }
    #[inline]
    pub fn vi(&self, i: usize, j: usize) -> usize {
        i + j * self.nx
    }
    #[inline]
    pub fn ci(&self, i: usize, j: usize) -> usize {
        i + j * self.nx
    }

    pub fn u_layout(&self) -> Layout {
        Layout { w: self.nx + 1, rows: self.ny, ox: 0.0, oy: 0.5 }
    }
    pub fn v_layout(&self) -> Layout {
        Layout { w: self.nx, rows: self.ny + 1, ox: 0.5, oy: 0.0 }
    }
    pub fn c_layout(&self) -> Layout {
        Layout { w: self.nx, rows: self.ny, ox: 0.5, oy: 0.5 }
    }

    /// Whether the u face (i, j) touches a solid cell.
    #[inline]
    pub fn u_solid(&self, i: usize, j: usize) -> bool {
        (i > 0 && self.solid[self.ci(i - 1, j)]) || (i < self.nx && self.solid[self.ci(i, j)])
    }
    /// Whether the v face (i, j) touches a solid cell.
    #[inline]
    pub fn v_solid(&self, i: usize, j: usize) -> bool {
        (j > 0 && self.solid[self.ci(i, j - 1)]) || (j < self.ny && self.solid[self.ci(i, j)])
    }

    /// The velocity at `p`, interpolated linearly.
    pub fn velocity(&self, p: V2) -> V2 {
        let inv = 1.0 / self.h;
        v2(
            sample_linear(&self.u, self.u_layout(), inv, p),
            sample_linear(&self.v, self.v_layout(), inv, p),
        )
    }

    /// The velocity at `p`, interpolated with Catmull-Rom splines: smooth
    /// across cell edges, for tracing.
    pub fn velocity_smooth(&self, p: V2) -> V2 {
        let inv = 1.0 / self.h;
        v2(
            sample_cubic(&self.u, self.u_layout(), inv, p),
            sample_cubic(&self.v, self.v_layout(), inv, p),
        )
    }

    /// The largest speed on any face.
    pub fn max_speed(&self) -> f32 {
        let mu = self.u.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let mv = self.v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        mu.max(mv)
    }

    /// The velocity at each cell's centre.
    pub fn cell_velocity(&self, i: usize, j: usize) -> V2 {
        v2(
            0.5 * (self.u[self.ui(i, j)] + self.u[self.ui(i + 1, j)]),
            0.5 * (self.v[self.vi(i, j)] + self.v[self.vi(i, j + 1)]),
        )
    }

    /// The vorticity (curl) at each cell, positive turning clockwise on
    /// screen. Zero round the edge.
    pub fn vorticity(&self, out: &mut Vec<f32>) {
        let (nx, ny) = (self.nx, self.ny);
        out.clear();
        out.resize(nx * ny, 0.0);
        let k = 0.5 / self.h;
        for j in 1..ny - 1 {
            for i in 1..nx - 1 {
                let dvdx = self.cell_velocity(i + 1, j).y - self.cell_velocity(i - 1, j).y;
                let dudy = self.cell_velocity(i, j + 1).x - self.cell_velocity(i, j - 1).x;
                out[self.ci(i, j)] = (dvdx - dudy) * k;
            }
        }
    }
}

#[inline]
fn coords(l: Layout, inv_h: f32, p: V2) -> (usize, usize, f32, f32) {
    let fx = (p.x * inv_h - l.ox).clamp(0.0, (l.w - 1) as f32);
    let fy = (p.y * inv_h - l.oy).clamp(0.0, (l.rows - 1) as f32);
    let i = (fx as usize).min(l.w - 2);
    let j = (fy as usize).min(l.rows - 2);
    (i, j, fx - i as f32, fy - j as f32)
}

/// A field sampled at `p` by bilinear interpolation, clamped to its edges.
#[inline]
pub fn sample_linear(data: &[f32], l: Layout, inv_h: f32, p: V2) -> f32 {
    let (i, j, tx, ty) = coords(l, inv_h, p);
    let a = j * l.w + i;
    let b = a + l.w;
    let top = data[a] + (data[a + 1] - data[a]) * tx;
    let bottom = data[b] + (data[b + 1] - data[b]) * tx;
    top + (bottom - top) * ty
}

/// As [`sample_linear`], with the smallest and largest of the four samples
/// it blends, to limit MacCormack's correction.
#[inline]
pub fn sample_linear_bounds(data: &[f32], l: Layout, inv_h: f32, p: V2) -> (f32, f32, f32) {
    let (i, j, tx, ty) = coords(l, inv_h, p);
    let a = j * l.w + i;
    let b = a + l.w;
    let (s00, s10, s01, s11) = (data[a], data[a + 1], data[b], data[b + 1]);
    let top = s00 + (s10 - s00) * tx;
    let bottom = s01 + (s11 - s01) * tx;
    let lo = s00.min(s10).min(s01).min(s11);
    let hi = s00.max(s10).max(s01).max(s11);
    (top + (bottom - top) * ty, lo, hi)
}

#[inline]
fn catmull_rom(t: f32) -> [f32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        0.5 * (-t3 + 2.0 * t2 - t),
        0.5 * (3.0 * t3 - 5.0 * t2 + 2.0),
        0.5 * (-3.0 * t3 + 4.0 * t2 + t),
        0.5 * (t3 - t2),
    ]
}

/// A field sampled at `p` by Catmull-Rom interpolation, clamped to its
/// edges.
pub fn sample_cubic(data: &[f32], l: Layout, inv_h: f32, p: V2) -> f32 {
    let (i, j, tx, ty) = coords(l, inv_h, p);
    let wx = catmull_rom(tx);
    let wy = catmull_rom(ty);
    let mut sum = 0.0;
    for (dj, wyj) in wy.iter().enumerate() {
        let jj = (j as isize + dj as isize - 1).clamp(0, l.rows as isize - 1) as usize;
        let row = jj * l.w;
        let mut acc = 0.0;
        for (di, wxi) in wx.iter().enumerate() {
            let ii = (i as isize + di as isize - 1).clamp(0, l.w as isize - 1) as usize;
            acc += data[row + ii] * wxi;
        }
        sum += acc * wyj;
    }
    sum
}
