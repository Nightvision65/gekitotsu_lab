//! Java `Utils.sin(int)` / `Utils.cos(int)` 的离散角度查表。
//! 表值为 `sin(度数)`，索引 0..450（多出的 90 个高位是给 `cos` 的 `+90` 溢位准备的）。

use crate::gen_consts::UTILS_TRIGONOMETRIC_LOOKUP as LUT;

/// Java `if (x >= 0 && x < 450) lookup[x]; else lookup[((x % 360) + 360) % 360]`
// 保留 Java 原表达式形状（与 `rem_euclid` 等价但更便于逐行对照 oracle）。
#[allow(clippy::manual_rem_euclid)]
#[inline(always)]
fn lookup(x: i32) -> f32 {
    if (0..450).contains(&x) {
        LUT[x as usize]
    } else {
        LUT[(((x % 360) + 360) % 360) as usize]
    }
}

/// Java `Utils.sin(int x)`
#[inline(always)]
pub fn sin(x: i32) -> f32 {
    lookup(x)
}

/// Java `Utils.cos(int x)`：`x += 90`（int 回绕）后走同一查表逻辑
#[inline(always)]
pub fn cos(x: i32) -> f32 {
    lookup(x.wrapping_add(90))
}
