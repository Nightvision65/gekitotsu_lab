//! Java 数值语义的逐条封装：Rust `as` 即为 JVM f2i/d2l（NaN→0、饱和到目标类型边界）。

/// Java `(int) f`
#[inline(always)]
pub fn to_i32(v: f32) -> i32 {
    v as i32
}

/// Java `(long) d`
#[inline(always)]
pub fn to_i64(v: f64) -> i64 {
    v as i64
}

/// Java `Math.round(float)` = `(int) Math.floor(a + 0.5f)`
#[inline(always)]
pub fn round_f32(v: f32) -> i32 {
    (v + 0.5f32).floor() as i32
}

/// Java `Math.round(double)` = `(long) Math.floor(a + 0.5d)`
#[inline(always)]
pub fn round_f64(v: f64) -> i64 {
    (v + 0.5f64).floor() as i64
}

/// Java `Shape.xySync()` / `xSync()` / `ySync()`：复刻 Flash 引擎 0.05 量子化。
/// `x = (int)(20 * x) * 0.05F`（负数向零截断；NaN→0；±inf 饱和 i32 后转回 f32）
#[inline(always)]
pub fn xy_sync(v: f32) -> f32 {
    ((20.0f32 * v) as i32) as f32 * crate::gen_literals::jf32!(0.05)
}

/// Java `Shape.move()`：`x += X`
#[inline(always)]
pub fn move_xy(x: &mut f32, y: &mut f32, dx: f32, dy: f32) {
    *x += dx;
    *y += dy;
}
