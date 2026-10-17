//! 31 位 LCG，逐位复刻 oracle `Utils.random()`（randFunc == 1 路径，唯一被启用的分支）。

pub const DEFAULT_SEED: i32 = 1;
pub const LCG_MUL: i64 = 1_103_515_245;
pub const LCG_ADD: i64 = 12_345;

/// Java `Utils.normalizeSeed(long)`：`(int)(seed & 0x7fffffffL)`
#[inline(always)]
pub fn normalize_seed(seed: i64) -> i32 {
    (seed & 0x7fff_ffff) as i32
}

/// Java `new Utils()`
#[inline(always)]
pub fn new_seed() -> i32 {
    DEFAULT_SEED
}

/// Java `Utils.random()`：先提升为 long 相乘（避免低 31 位精度丢失），推进状态后返回 `[0,1)`。
#[inline(always)]
pub fn next(seed: &mut i32) -> f32 {
    let next = (*seed as i64) * LCG_MUL + LCG_ADD;
    *seed = (next & 0x7fff_ffff) as i32;
    (((*seed as u32) >> 16) & 0x7fff) as f32 / 32_768.0f32
}
