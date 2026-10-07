//! Points and directions for rhai scripts.

use rhai::{CustomType, Engine, EvalAltResult, TypeBuilder};
use parry2d::math::Vec2 as PVec;

#[derive(Debug, Default, Clone)]
#[derive(CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Vec2{ x, y }
    }
    /// `new_vec(x, y)`, refusing infinite and NaN coordinates (from a
    /// division by zero, say), which would otherwise surface far away.
    fn checked(x: f64, y: f64) -> Result<Self, Box<EvalAltResult>> {
        if !(x.is_finite() && y.is_finite()) {
            return Err(format!("new_vec: coordinates must be finite numbers, got ({}, {})", x, y).into());
        }
        Ok(Vec2 { x, y })
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("new_vec", Self::checked)
        ;
    }
}

impl From<Vec2> for PVec {
    fn from(val: Vec2) -> Self {
        PVec::new(val.x as f32, val.y as f32)
    }
}

pub(super) fn register(engine: &mut Engine) {
    engine.build_type::<Vec2>();
}
