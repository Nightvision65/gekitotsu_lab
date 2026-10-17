//! fort code 解码与对局输入解析，逐位复刻 `Main.to_xyr` / `Main.compileFort` / `Setting.CompileForts`。
//!
//! 契约（AGENTS §3.1）：61 进制字符表 `0123456789a-zA-Z`；核心取 code[1..6]，
//! `coreX = x-190`、`coreY = y-400`；单位每 6 字符，`type = pskey.indexOf(char)`。
//! `CompiledFort` 的 `baseSeed`/`seed[]` 是死字段（AGENTS §D-003），不移植。

use crate::gen_consts::MAIN_PSKEY;

/// 字符表 → 索引；非法字符 -1（与 Java `String.indexOf` 一致）。
/// 表由 oracle 导出的 pskey 直接构建，避免手抄字符表。
static PSKEY_INDEX: [i8; 256] = {
    let mut table = [-1i8; 256];
    let bytes = MAIN_PSKEY.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        table[bytes[i] as usize] = i as i8;
        i += 1;
    }
    table
};

#[inline]
fn pskey_index(b: u8) -> i32 {
    PSKEY_INDEX[b as usize] as i32
}

/// Java `Main.to_xyr`：5 位 61 进制 → `[x, y, r]`（负数除法向零截断）
pub fn to_xyr(s: &[u8]) -> [i32; 3] {
    debug_assert_eq!(s.len(), 5);
    let rxy = pskey_index(s[0]) * 13_845_841
        + pskey_index(s[1]) * 226_981
        + pskey_index(s[2]) * 3_721
        + pskey_index(s[3]) * 61
        + pskey_index(s[4]);
    let r = rxy / 1_000_000;
    let x = (rxy / 1_000) % 1_000;
    let y = rxy % 1_000;
    [x, y, r]
}

/// Java `CompiledFort`（去掉死字段 seed/baseSeed）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledFort {
    pub name: String,
    pub core_type: i32,
    pub core_x: i32,
    pub core_y: i32,
    pub unit_count: usize,
    pub unit_type: Vec<i32>,
    pub x: Vec<i32>,
    pub y: Vec<i32>,
    pub r: Vec<i32>,
}

/// Java `Main.compileFort`
pub fn compile_fort(name: &str, code: &str) -> CompiledFort {
    let bytes = code.as_bytes();
    if bytes.len() < 6 {
        panic!("阵容代码长度不足: {name} [{code}]");
    }
    let core = to_xyr(&bytes[1..6]);
    let core_type = bytes[0] as i32 - b'0' as i32;
    let unit_count = bytes.len() / 6 - 1;
    let mut unit_type = Vec::with_capacity(unit_count);
    let mut x = Vec::with_capacity(unit_count);
    let mut y = Vec::with_capacity(unit_count);
    let mut r = Vec::with_capacity(unit_count);
    for i in 0..unit_count {
        let j = 6 + 6 * i;
        let u = to_xyr(&bytes[j + 1..j + 6]);
        unit_type.push(pskey_index(bytes[j]));
        x.push(u[0]);
        y.push(u[1]);
        r.push(u[2]);
    }
    CompiledFort {
        name: name.to_string(),
        core_type,
        core_x: core[0] - 190,
        core_y: core[1] - 400,
        unit_count,
        unit_type,
        x,
        y,
        r,
    }
}

/// Java `GameTask.main_setup`：核心位置（side 1 取反）
#[inline]
pub fn core_spawn(f: &CompiledFort, side: usize) -> (i32, i32) {
    let cx = if side == 1 { -f.core_x } else { f.core_x };
    (cx, f.core_y)
}

/// Java `GameTask.main_setup`：单位生成参数 `(ux, uy, rr)`（严格保留 float 运算顺序）
#[inline]
pub fn unit_spawn(f: &CompiledFort, side: usize, k: usize) -> (f32, f32, i32) {
    let mut ux = f.x[k] as f32;
    let mut uy = f.y[k] as f32;
    let mut rr = f.r[k];
    if side == 1 {
        ux = (380.0f32 - ux) + 1490.0f32;
        rr = 180 - rr;
    } else {
        ux += 50.0f32;
    }
    uy += 132.0f32;
    rr = (rr % 360 + 360) % 360;
    (ux, uy, rr)
}

/// Java `GameTask.main_setup` 的种子乘子（int 回绕）
#[inline]
pub fn seed_factor(f: &CompiledFort) -> i32 {
    ((f.core_x + 190) % 168 + 48) * ((f.core_y + 400) % 168 + 48)
}

/// Java `Setting.CompileForts`：以 `/` 分组，`name&code` 或纯 code；code 仅保留 ASCII 字母数字
pub fn parse_forts(content: &str) -> Vec<CompiledFort> {
    let mut out = Vec::new();
    for part in content.trim().split('/') {
        let part = part.trim_matches(|c: char| c <= ' ');
        if part.is_empty() {
            continue;
        }
        match part.rfind('&') {
            None => {
                let code: String = part.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
                if code.len() < 6 {
                    continue;
                }
                out.push(compile_fort("", &code));
            }
            Some(idx) => {
                let name = &part[..idx];
                let code: String = part[idx + 1..]
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect();
                if !code.len().is_multiple_of(6) {
                    continue;
                }
                out.push(compile_fort(name, &code));
            }
        }
    }
    out
}
