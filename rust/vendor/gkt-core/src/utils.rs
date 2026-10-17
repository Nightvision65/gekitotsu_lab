//! oracle `Utils` 的包装：离散角度查表与 `cosF`/`sinF`。

use crate::fdlibm;
use crate::fdlibm_consts::MATH_DUMP_DEG_TO_RAD_F32;

pub use crate::trig::{cos, sin};

/// oracle 字面量 `0.017453292519943295F`
pub const DEG_TO_RAD_F32: f32 = MATH_DUMP_DEG_TO_RAD_F32;

/// Java `Utils.cosF(RDeg)` = `(float)Math.cos((double)RDeg * 0.017453292519943295F)`
#[inline(always)]
pub fn cos_f(r_deg: f32) -> f32 {
    fdlibm::cos(r_deg as f64 * DEG_TO_RAD_F32 as f64) as f32
}

/// Java `Utils.sinF(RDeg)`
#[inline(always)]
pub fn sin_f(r_deg: f32) -> f32 {
    fdlibm::sin(r_deg as f64 * DEG_TO_RAD_F32 as f64) as f32
}
