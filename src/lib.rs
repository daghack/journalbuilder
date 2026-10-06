// Script-argument checks use `!(x > 0.0)` on purpose: unlike `x <= 0.0` it
// also rejects NaN.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod journal;
pub mod lockbuilder;
