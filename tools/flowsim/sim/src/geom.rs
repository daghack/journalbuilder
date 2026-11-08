//! Points in the domain: its own units, x to the right, y down (as on the
//! canvas and in SVG).

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// A point or vector. Written to JSON as `[x, y]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "[f32; 2]", into = "[f32; 2]")]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

pub const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl From<[f32; 2]> for V2 {
    fn from([x, y]: [f32; 2]) -> Self {
        V2 { x, y }
    }
}

impl From<V2> for [f32; 2] {
    fn from(p: V2) -> Self {
        [p.x, p.y]
    }
}

impl V2 {
    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn cross(self, o: V2) -> f32 {
        self.x * o.y - self.y * o.x
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn len2(self) -> f32 {
        self.dot(self)
    }
    /// Turned a quarter: clockwise on screen, since y points down.
    pub fn perp(self) -> V2 {
        v2(-self.y, self.x)
    }
    pub fn normalized(self) -> V2 {
        let l = self.len();
        if l > 1e-12 { self / l } else { V2::default() }
    }
    pub fn rotated(self, radians: f32) -> V2 {
        let (s, c) = radians.sin_cos();
        v2(self.x * c - self.y * s, self.x * s + self.y * c)
    }
    pub fn lerp(self, o: V2, t: f32) -> V2 {
        self + (o - self) * t
    }
    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
    /// Rounded to hundredths, which is all the JSON and SVG keep.
    pub fn rounded(self) -> V2 {
        v2((self.x * 100.0).round() / 100.0, (self.y * 100.0).round() / 100.0)
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}
impl SubAssign for V2 {
    fn sub_assign(&mut self, o: V2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}
impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        v2(self.x * s, self.y * s)
    }
}
impl Div<f32> for V2 {
    type Output = V2;
    fn div(self, s: f32) -> V2 {
        v2(self.x / s, self.y / s)
    }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        v2(-self.x, -self.y)
    }
}

/// The distance from `p` to the segment `a`–`b`.
pub fn segment_distance(p: V2, a: V2, b: V2) -> f32 {
    let ab = b - a;
    let l2 = ab.len2();
    let t = if l2 > 1e-12 { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    p.dist(a + ab * t)
}

/// A small, fast, seedable random source (xorshift64*).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    /// Uniform in [-1, 1).
    pub fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}
