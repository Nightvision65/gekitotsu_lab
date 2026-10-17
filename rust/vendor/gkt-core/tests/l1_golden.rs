//! L1 黄金向量：RNG 状态、Java 数值转换语义。

use gkt_core::gen_consts::UTILS_TRIGONOMETRIC_LOOKUP;
use gkt_core::jmath;
use gkt_core::rng;
use gkt_core::trig;

#[test]
fn lcg_first_five_states_from_seed_zero() {
    // AGENTS.md §6-L1 黄金向量（Java `new Utils(0)` 连调 5 次后的 universalSeed）
    let mut s = rng::normalize_seed(0);
    let mut states = [0i32; 5];
    for slot in states.iter_mut() {
        rng::next(&mut s);
        *slot = s;
    }
    assert_eq!(
        states,
        [12345, 1406932606, 654583775, 1449466924, 229283573]
    );
}

#[test]
fn lcg_float_output_bit_exact() {
    // seed=0 首次调用：state=12345 → ((12345>>16)&0x7fff)/32768 = 0/32768
    let mut s = rng::normalize_seed(0);
    assert_eq!(rng::next(&mut s).to_bits(), 0f32.to_bits());
    assert_eq!(s, 12345);
    // 状态 1406932606：高位片段取自 oracle 状态链，返回值必须落在 [0,1)
    let v = rng::next(&mut s);
    let expect = (((1406932606u32) >> 16) & 0x7fff) as f32 / 32_768.0f32;
    assert_eq!(v.to_bits(), expect.to_bits());
}

#[test]
fn normalize_seed_masks_to_31_bits() {
    assert_eq!(rng::normalize_seed(-1), 0x7fff_ffff);
    assert_eq!(rng::normalize_seed(0x1_0000_0001), 1);
    assert_eq!(rng::new_seed(), 1);
}

#[test]
fn java_cast_semantics() {
    assert_eq!(jmath::to_i32(f32::NAN), 0);
    assert_eq!(jmath::to_i32(f32::INFINITY), i32::MAX);
    assert_eq!(jmath::to_i32(f32::NEG_INFINITY), i32::MIN);
    assert_eq!(jmath::to_i32(3.9), 3);
    assert_eq!(jmath::to_i32(-3.9), -3);
    assert_eq!(jmath::to_i32(-0.0), 0);
    assert_eq!(jmath::to_i64(f64::NAN), 0);
    assert_eq!(jmath::to_i64(f64::INFINITY), i64::MAX);
}

#[test]
fn xy_sync_quantizes_like_flash() {
    assert_eq!(jmath::xy_sync(0.07).to_bits(), 0.05f32.to_bits());
    assert_eq!(jmath::xy_sync(0.05).to_bits(), 0.05f32.to_bits());
    // -0.6 向零截断为 0，结果必须是 +0.0
    assert_eq!(jmath::xy_sync(-0.03).to_bits(), 0.0f32.to_bits());
    assert_eq!(jmath::xy_sync(-0.1).to_bits(), (-0.1f32).to_bits());
    assert_eq!(jmath::xy_sync(f32::NAN).to_bits(), 0.0f32.to_bits());
    assert_eq!(
        jmath::xy_sync(f32::INFINITY).to_bits(),
        ((i32::MAX as f32) * 0.05f32).to_bits()
    );
}

#[test]
fn trig_table_anchors_match_oracle_export() {
    // 451 项 = oracle `Utils.trigonometricLookup` 的 0..450 度
    assert_eq!(UTILS_TRIGONOMETRIC_LOOKUP.len(), 451);
    assert_eq!(UTILS_TRIGONOMETRIC_LOOKUP[90].to_bits(), 1.0f32.to_bits());
    assert_eq!(UTILS_TRIGONOMETRIC_LOOKUP[180].to_bits(), 0.0f32.to_bits());
    assert_eq!(
        UTILS_TRIGONOMETRIC_LOOKUP[270].to_bits(),
        (-1.0f32).to_bits()
    );
    assert_eq!(UTILS_TRIGONOMETRIC_LOOKUP[450].to_bits(), 1.0f32.to_bits());
}

#[test]
fn trig_accessors_follow_java_semantics() {
    assert_eq!(trig::sin(0).to_bits(), 0.0f32.to_bits());
    assert_eq!(trig::sin(90).to_bits(), 1.0f32.to_bits());
    assert_eq!(trig::cos(0).to_bits(), 1.0f32.to_bits());
    assert_eq!(trig::cos(90).to_bits(), 0.0f32.to_bits());
    assert_eq!(trig::cos(180).to_bits(), (-1.0f32).to_bits());
    // 越界负角走 `((x % 360) + 360) % 360` 归一化
    assert_eq!(trig::sin(-90).to_bits(), trig::sin(270).to_bits());
    assert_eq!(trig::cos(-90).to_bits(), trig::cos(270).to_bits());
    // 450.. 高位区只可能被 cos 的 +90 触及
    assert_eq!(
        trig::cos(360).to_bits(),
        UTILS_TRIGONOMETRIC_LOOKUP[450].to_bits()
    );
}

#[test]
fn java_round_semantics() {
    assert_eq!(jmath::round_f32(2.5), 3);
    assert_eq!(jmath::round_f32(-2.5), -2);
    assert_eq!(jmath::round_f32(2.4), 2);
    assert_eq!(jmath::round_f32(f32::NAN), 0);
    assert_eq!(jmath::round_f32(f32::INFINITY), i32::MAX);
    assert_eq!(jmath::round_f32(f32::NEG_INFINITY), i32::MIN);
    assert_eq!(jmath::round_f64(-2.5), -2);
}
