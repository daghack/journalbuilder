//! A snapshot of the fluid: its velocity (on the grid's faces, exactly as
//! the solver has it), pressure and dye, kept to distort what a page
//! draws. Journals keep fields in their `flows/` files, encoded (see
//! [`EncodedField`]), and sample them with [`Field::sample`] and
//! [`Field::velocity`].
//!
//! The domain runs from (0, 0) to [`Field::size`], in the simulator's
//! units, y down.

use crate::geom::{V2, v2};
use crate::grid::{Grid, sample_cubic, sample_linear};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// What can be read off a field at a point, besides its velocity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// How fast the fluid moves.
    Speed,
    /// How it turns: positive turning clockwise on screen.
    Vorticity,
    Pressure,
    /// How much dye is there: what sources pour in, carried by the flow.
    Dye,
}

impl Channel {
    pub const ALL: [Channel; 4] = [Channel::Speed, Channel::Vorticity, Channel::Pressure, Channel::Dye];

    pub fn name(self) -> &'static str {
        match self {
            Channel::Speed => "speed",
            Channel::Vorticity => "vorticity",
            Channel::Pressure => "pressure",
            Channel::Dye => "dye",
        }
    }

    pub fn from_name(name: &str) -> Option<Channel> {
        Channel::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// The fluid at one moment.
#[derive(Clone, Debug)]
pub struct Field {
    /// The velocity (faces), pressure (`q`) and dye; nothing is solid.
    grid: Grid,
    /// Derived when made, per cell.
    vorticity: Vec<f32>,
    speed: Vec<f32>,
    /// Each channel's least and greatest value, as `Channel::ALL` orders them.
    ranges: [(f32, f32); 4],
}

impl Field {
    /// The fluid in `grid` as it is now.
    pub fn from_grid(grid: &Grid) -> Field {
        let mut g = Grid::new(grid.nx, grid.ny, grid.h);
        g.u.copy_from_slice(&grid.u);
        g.v.copy_from_slice(&grid.v);
        g.q.copy_from_slice(&grid.q);
        g.dye.copy_from_slice(&grid.dye);
        Field::from_parts(g)
    }

    /// A field from its raw arrays: `u` on the (nx+1) × ny vertical faces,
    /// `v` on the nx × (ny+1) horizontal ones, `pressure` and `dye` per
    /// cell, row by row, cells `h` across.
    pub fn from_raw(nx: usize, ny: usize, h: f32, u: &[f32], v: &[f32], pressure: &[f32], dye: &[f32]) -> Result<Field, String> {
        if nx < 4 || ny < 4 || !(h > 0.0 && h.is_finite()) {
            return Err(format!("a field needs at least 4 x 4 cells of a positive size, got {nx} x {ny} of {h}"));
        }
        let mut g = Grid::new(nx, ny, h);
        for (name, to, from) in [("u", &mut g.u, u), ("v", &mut g.v, v), ("pressure", &mut g.q, pressure), ("dye", &mut g.dye, dye)] {
            if to.len() != from.len() {
                return Err(format!("the field's {name} has {} values, not {}", from.len(), to.len()));
            }
            to.copy_from_slice(from);
        }
        Ok(Field::from_parts(g))
    }

    fn from_parts(grid: Grid) -> Field {
        let mut vorticity = Vec::new();
        grid.vorticity(&mut vorticity);
        let speed: Vec<f32> = (0..grid.ny)
            .flat_map(|j| (0..grid.nx).map(move |i| (i, j)))
            .map(|(i, j)| grid.cell_velocity(i, j).len())
            .collect();
        let extent = |data: &[f32]| {
            let (lo, hi) = data.iter().filter(|x| x.is_finite()).fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &x| (lo.min(x), hi.max(x)));
            if lo <= hi { (lo, hi) } else { (0.0, 0.0) }
        };
        let (_, top_speed) = extent(&speed);
        let (wlo, whi) = extent(&vorticity);
        let spin = wlo.abs().max(whi.abs());
        let ranges = [(0.0, top_speed), (-spin, spin), extent(&grid.q), extent(&grid.dye)];
        Field { grid, vorticity, speed, ranges }
    }

    pub fn nx(&self) -> usize {
        self.grid.nx
    }
    pub fn ny(&self) -> usize {
        self.grid.ny
    }
    /// A cell's side.
    pub fn h(&self) -> f32 {
        self.grid.h
    }
    /// How far it reaches across and down.
    pub fn size(&self) -> V2 {
        v2(self.grid.width(), self.grid.height())
    }
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The fluid's velocity at `p`, smoothly interpolated (Catmull-Rom),
    /// held at its value at the nearest edge beyond the domain.
    pub fn velocity(&self, p: V2) -> V2 {
        self.grid.velocity_smooth(p)
    }

    /// `channel` at `p`, interpolated linearly, held at the edges.
    pub fn sample(&self, channel: Channel, p: V2) -> f32 {
        let inv = 1.0 / self.grid.h;
        let cells = self.grid.c_layout();
        match channel {
            Channel::Speed => sample_linear(&self.speed, cells, inv, p),
            Channel::Vorticity => sample_linear(&self.vorticity, cells, inv, p),
            Channel::Pressure => sample_linear(&self.grid.q, cells, inv, p),
            Channel::Dye => sample_linear(&self.grid.dye, cells, inv, p),
        }
    }

    /// `channel` at `p`, smoothly interpolated (Catmull-Rom).
    pub fn sample_smooth(&self, channel: Channel, p: V2) -> f32 {
        let inv = 1.0 / self.grid.h;
        let cells = self.grid.c_layout();
        let data = match channel {
            Channel::Speed => &self.speed,
            Channel::Vorticity => &self.vorticity,
            Channel::Pressure => &self.grid.q,
            Channel::Dye => &self.grid.dye,
        };
        sample_cubic(data, cells, inv, p)
    }

    /// How `channel` rises at `p`, per unit, by central differences a cell
    /// apart.
    pub fn gradient(&self, channel: Channel, p: V2) -> V2 {
        let e = self.grid.h;
        let dx = self.sample_smooth(channel, p + v2(e, 0.0)) - self.sample_smooth(channel, p - v2(e, 0.0));
        let dy = self.sample_smooth(channel, p + v2(0.0, e)) - self.sample_smooth(channel, p - v2(0.0, e));
        v2(dx, dy) / (2.0 * e)
    }

    /// `channel`'s least and greatest values: speed from 0, vorticity
    /// either way of 0 alike.
    pub fn range(&self, channel: Channel) -> (f32, f32) {
        self.ranges[Channel::ALL.iter().position(|c| *c == channel).expect("every channel is in ALL")]
    }

    /// The field encoded, to keep in a file.
    pub fn encode(&self) -> EncodedField {
        let g = &self.grid;
        let channels = [("u", &g.u, g.nx + 1), ("v", &g.v, g.nx), ("pressure", &g.q, g.nx), ("dye", &g.dye, g.nx)];
        let mut bytes = Vec::with_capacity(2 * (g.u.len() + g.v.len() + 2 * g.q.len()));
        let mut ranges = Vec::new();
        for (name, data, width) in channels {
            let (lo, hi) = data.iter().filter(|x| x.is_finite()).fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &x| (lo.min(x), hi.max(x)));
            let (lo, hi) = if lo <= hi { (lo, hi) } else { (0.0, 0.0) };
            let scale = if hi > lo { 65535.0 / (hi - lo) } else { 0.0 };
            let quantised: Vec<u16> = data.iter().map(|&x| {
                let x = if x.is_finite() { x } else { lo };
                ((x - lo) * scale).round().clamp(0.0, 65535.0) as u16
            }).collect();
            // Each row as differences from the value before it: smooth
            // fields deflate far better so.
            for row in quantised.chunks(width) {
                let mut last = 0u16;
                for &q in row {
                    bytes.extend_from_slice(&q.wrapping_sub(last).to_le_bytes());
                    last = q;
                }
            }
            ranges.push(ChannelRange { name: name.to_string(), lo, hi });
        }
        let mut deflate = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        deflate.write_all(&bytes).expect("writing to memory doesn't fail");
        let packed = deflate.finish().expect("writing to memory doesn't fail");
        EncodedField {
            nx: g.nx,
            ny: g.ny,
            h: g.h,
            channels: ranges,
            data: base64::engine::general_purpose::STANDARD.encode(packed),
        }
    }

    /// A field from its encoded form.
    pub fn decode(encoded: &EncodedField) -> Result<Field, String> {
        let EncodedField { nx, ny, h, .. } = *encoded;
        if !(4..=4096).contains(&nx) || !(4..=4096).contains(&ny) || !(h > 0.0 && h.is_finite()) {
            return Err(format!("the field's grid ({nx} x {ny} cells of {h}) isn't one a field can have"));
        }
        let packed = base64::engine::general_purpose::STANDARD
            .decode(encoded.data.trim())
            .map_err(|e| format!("the field's data isn't base64: {e}"))?;
        let mut bytes = Vec::new();
        flate2::read::DeflateDecoder::new(packed.as_slice())
            .read_to_end(&mut bytes)
            .map_err(|e| format!("the field's data doesn't inflate: {e}"))?;
        let mut grid = Grid::new(nx, ny, h);
        let mut at = 0;
        for (name, width) in [("u", nx + 1), ("v", nx), ("pressure", nx), ("dye", nx)] {
            let range = encoded.channels.iter().find(|c| c.name == name)
                .ok_or_else(|| format!("the field has no range for its {name}"))?;
            let data = match name {
                "u" => &mut grid.u,
                "v" => &mut grid.v,
                "pressure" => &mut grid.q,
                _ => &mut grid.dye,
            };
            let need = 2 * data.len();
            let chunk = bytes.get(at..at + need).ok_or_else(|| format!("the field's data ends before its {name}"))?;
            at += need;
            let step = if range.hi > range.lo { (range.hi - range.lo) / 65535.0 } else { 0.0 };
            for (row, values) in data.chunks_mut(width).zip(chunk.chunks(2 * width)) {
                let mut last = 0u16;
                for (value, pair) in row.iter_mut().zip(values.chunks_exact(2)) {
                    last = last.wrapping_add(u16::from_le_bytes([pair[0], pair[1]]));
                    *value = range.lo + last as f32 * step;
                }
            }
        }
        if at != bytes.len() {
            return Err(format!("the field's data is {} bytes, not the {} its grid takes", bytes.len(), at));
        }
        Ok(Field::from_parts(grid))
    }
}

/// A channel's range in an encoded field: its values are quantised to 16
/// bits between `lo` and `hi`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelRange {
    pub name: String,
    pub lo: f32,
    pub hi: f32,
}

/// A field as a file keeps it: `nx` × `ny` cells `h` across, and its
/// velocity's faces (`u` then `v`), pressure and dye, each quantised to 16
/// bits over its range, each row as differences, deflated, in base64.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncodedField {
    pub nx: usize,
    pub ny: usize,
    pub h: f32,
    pub channels: Vec<ChannelRange>,
    pub data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swirl() -> Grid {
        let mut g = Grid::new(24, 16, 5.0);
        let c = v2(60.0, 40.0);
        for j in 0..g.ny {
            for i in 0..=g.nx {
                let p = v2(i as f32 * g.h, (j as f32 + 0.5) * g.h);
                let k = g.ui(i, j);
                g.u[k] = -(p.y - c.y) * 0.5;
            }
        }
        for j in 0..=g.ny {
            for i in 0..g.nx {
                let p = v2((i as f32 + 0.5) * g.h, j as f32 * g.h);
                let k = g.vi(i, j);
                g.v[k] = (p.x - c.x) * 0.5;
            }
        }
        for (k, d) in g.dye.iter_mut().enumerate() {
            *d = (k % 7) as f32 / 7.0;
        }
        g
    }

    #[test]
    fn an_encoded_field_decodes_to_the_same_values() {
        let field = Field::from_grid(&swirl());
        let back = Field::decode(&field.encode()).expect("it decodes");
        assert_eq!((back.nx(), back.ny(), back.h()), (24, 16, 5.0));
        let tolerance = |(lo, hi): (f32, f32)| (hi - lo) / 65535.0 + 1e-5;
        for (a, b) in field.grid.u.iter().zip(&back.grid.u) {
            assert!((a - b).abs() <= tolerance((-40.0, 40.0)), "{a} vs {b}");
        }
        for (a, b) in field.grid.dye.iter().zip(&back.grid.dye) {
            assert!((a - b).abs() <= tolerance(field.range(Channel::Dye)), "{a} vs {b}");
        }
        let json = serde_json::to_string(&field.encode()).unwrap();
        let again: EncodedField = serde_json::from_str(&json).unwrap();
        assert!(Field::decode(&again).is_ok());
    }

    #[test]
    fn a_field_samples_its_velocity_and_channels() {
        let field = Field::from_grid(&swirl());
        let v = field.velocity(v2(80.0, 40.0));
        assert!((v.x - 0.0).abs() < 0.2 && (v.y - 10.0).abs() < 0.2, "{v:?}");
        let speed = field.sample(Channel::Speed, v2(80.0, 40.0));
        assert!((speed - 10.0).abs() < 1.5, "{speed}");
        // A solid-body swirl turns the same everywhere inside.
        let spin = field.sample(Channel::Vorticity, v2(60.0, 40.0));
        assert!((spin - 1.0).abs() < 0.05, "{spin}");
        assert_eq!(field.range(Channel::Speed).0, 0.0);
    }

    #[test]
    fn broken_fields_say_why() {
        let mut encoded = Field::from_grid(&swirl()).encode();
        encoded.data = "not base64!".into();
        assert!(Field::decode(&encoded).unwrap_err().contains("base64"));
        let mut encoded = Field::from_grid(&swirl()).encode();
        encoded.nx = 30;
        assert!(Field::decode(&encoded).is_err());
    }
}
