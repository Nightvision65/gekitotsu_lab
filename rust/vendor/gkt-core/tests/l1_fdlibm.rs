//! L1：fdlibm 移植对拍 Java StrictMath 校准语料（`fixtures/math/math_corpus.bin`）。
//!
//! 语料格式（小端）：magic('GKTM') u32, version u32, count u32；每条 25 字节：
//! func u8, a u64, b u64, out u64。func: 0=sin 1=cos 2=toDegrees 3=toRadians 4=atan2。

use std::path::PathBuf;

const FUNC_SIN: u8 = 0;
const FUNC_COS: u8 = 1;
const FUNC_TO_DEGREES: u8 = 2;
const FUNC_TO_RADIANS: u8 = 3;
const FUNC_ATAN2: u8 = 4;

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/math/math_corpus.bin")
}

fn func_name(f: u8) -> &'static str {
    match f {
        FUNC_SIN => "sin",
        FUNC_COS => "cos",
        FUNC_TO_DEGREES => "toDegrees",
        FUNC_TO_RADIANS => "toRadians",
        FUNC_ATAN2 => "atan2",
        _ => "?",
    }
}

fn call(func: u8, a: f64, b: f64) -> f64 {
    match func {
        FUNC_SIN => gkt_core::fdlibm::sin(a),
        FUNC_COS => gkt_core::fdlibm::cos(a),
        FUNC_TO_DEGREES => gkt_core::fdlibm::to_degrees(a),
        FUNC_TO_RADIANS => gkt_core::fdlibm::to_radians(a),
        FUNC_ATAN2 => gkt_core::fdlibm::atan2(a, b),
        _ => panic!("unknown func {func}"),
    }
}

#[test]
fn fdlibm_matches_java_strictmath_corpus() {
    let data =
        std::fs::read(corpus_path()).expect("missing math corpus; run tools/gensrc/regen_math.ps1");
    assert!(data.len() >= 12, "corpus too short");
    let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    assert_eq!(magic, 0x4D54_4B47, "bad corpus magic");
    assert_eq!(version, 1, "bad corpus version");
    assert_eq!(data.len(), 12 + count * 25, "corpus size mismatch");

    let mut checked = 0usize;
    for i in 0..count {
        let base = 12 + i * 25;
        let func = data[base];
        let a = f64::from_bits(u64::from_le_bytes(
            data[base + 1..base + 9].try_into().unwrap(),
        ));
        let b = f64::from_bits(u64::from_le_bytes(
            data[base + 9..base + 17].try_into().unwrap(),
        ));
        let expect = f64::from_bits(u64::from_le_bytes(
            data[base + 17..base + 25].try_into().unwrap(),
        ));
        let got = call(func, a, b);
        if got.to_bits() != expect.to_bits() {
            panic!(
                "record #{i} {} a=0x{:016x} b=0x{:016x}: got 0x{:016x}, want 0x{:016x}",
                func_name(func),
                a.to_bits(),
                b.to_bits(),
                got.to_bits(),
                expect.to_bits()
            );
        }
        checked += 1;
    }
    assert!(checked > 400_000, "corpus unexpectedly small: {checked}");
}
