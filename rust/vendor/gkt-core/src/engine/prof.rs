//! 性能剖析（仅 `phase-timing` feature，默认不编译）：只累计时间/计数，不参与语义。
#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};

pub static BASE_MOVE_NS: AtomicU64 = AtomicU64::new(0);
pub static JUDGE_NS: AtomicU64 = AtomicU64::new(0);
pub static RESIGN_NS: AtomicU64 = AtomicU64::new(0);
pub static STEP_NS: AtomicU64 = AtomicU64::new(0);
pub static STEP_COUNT: AtomicU64 = AtomicU64::new(0);
pub static HIT_TEST_CALLS: AtomicU64 = AtomicU64::new(0);
/// 按 `Element` 变体分桶的 hit_test_point 调用/命中次数
pub static HIT_TARGET_CALLS: [AtomicU64; 13] = [const { AtomicU64::new(0) }; 13];
pub static HIT_TARGET_HITS: [AtomicU64; 13] = [const { AtomicU64::new(0) }; 13];
pub static SPAWNS: AtomicU64 = AtomicU64::new(0);
pub static STEP_BY_KIND: [AtomicU64; 13] = [const { AtomicU64::new(0) }; 13];
pub static LASER_MARCH_ITERS: AtomicU64 = AtomicU64::new(0);
pub static LMC: AtomicU64 = AtomicU64::new(0);
pub static LMC_DISABLED: AtomicU64 = AtomicU64::new(0);
pub static LMC_A: AtomicU64 = AtomicU64::new(0);
pub static LMC_B: AtomicU64 = AtomicU64::new(0);
pub static LMC_HIT: AtomicU64 = AtomicU64::new(0);
pub static LMC_SCAN: AtomicU64 = AtomicU64::new(0);
/// 命中调用归因：当前调用方标签（仅分阶段剖析时使用）
pub static CURRENT_CALLER: AtomicU64 = AtomicU64::new(10);
/// 传感器族：0=通用 Sensor（wall/shield/heal/...）1=UnitSensor 2=AtkSensor
pub static SENSOR_CALLS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
pub static SENSOR_ENTRIES: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
/// 各调用方发起的精确命中调用数（标签见 `CALLER_NAMES`）
pub static HIT_BY_CALLER: [AtomicU64; 11] = [const { AtomicU64::new(0) }; 11];
pub const CALLER_NAMES: [&str; 11] = [
    "Ball.land",
    "Ball.jump",
    "Ball.ground",
    "Ball.step.atk",
    "Ball.step.heal",
    "Ball.step.sensor",
    "Laser.march",
    "Core.step",
    "Bullet.step",
    "Wall.step",
    "other",
];

#[inline]
pub fn set_caller(label: u64) -> u64 {
    CURRENT_CALLER.swap(label, Ordering::Relaxed)
}
pub static LASER_MARCH_NS: AtomicU64 = AtomicU64::new(0);
pub static STEP_NS_BY_KIND: [AtomicU64; 13] = [const { AtomicU64::new(0) }; 13];
pub static ALIVE_AT_END: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default)]
pub struct PhaseProfile {
    pub base_move_ns: u64,
    pub judge_ns: u64,
    pub resign_ns: u64,
    pub step_ns: u64,
    pub step_count: u64,
    pub hit_test_calls: u64,
    pub hit_by_target_calls: [u64; 13],
    pub hit_by_target_hits: [u64; 13],
    pub spawns: u64,
    pub step_by_kind: [u64; 13],
    pub laser_march_iters: u64,
    pub lmc: [u64; 6],
    pub hit_by_caller: [u64; 11],
    pub sensor_calls: [u64; 3],
    pub sensor_entries: [u64; 3],
    pub laser_march_ns: u64,
    pub step_ns_by_kind: [u64; 13],
    pub alive_at_end: u64,
}

#[inline]
pub fn add(counter: &AtomicU64, value: u64) {
    counter.fetch_add(value, Ordering::Relaxed);
}

#[inline]
pub fn inc(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}

impl PhaseProfile {
    pub fn capture() -> Self {
        Self {
            base_move_ns: BASE_MOVE_NS.load(Ordering::Relaxed),
            judge_ns: JUDGE_NS.load(Ordering::Relaxed),
            resign_ns: RESIGN_NS.load(Ordering::Relaxed),
            step_ns: STEP_NS.load(Ordering::Relaxed),
            step_count: STEP_COUNT.load(Ordering::Relaxed),
            hit_test_calls: HIT_TEST_CALLS.load(Ordering::Relaxed),
            hit_by_target_calls: std::array::from_fn(|i| {
                HIT_TARGET_CALLS[i].load(Ordering::Relaxed)
            }),
            hit_by_target_hits: std::array::from_fn(|i| HIT_TARGET_HITS[i].load(Ordering::Relaxed)),
            spawns: SPAWNS.load(Ordering::Relaxed),
            step_by_kind: std::array::from_fn(|i| STEP_BY_KIND[i].load(Ordering::Relaxed)),
            laser_march_iters: LASER_MARCH_ITERS.load(Ordering::Relaxed),
            lmc: [
                LMC.load(Ordering::Relaxed),
                LMC_DISABLED.load(Ordering::Relaxed),
                LMC_A.load(Ordering::Relaxed),
                LMC_B.load(Ordering::Relaxed),
                LMC_HIT.load(Ordering::Relaxed),
                LMC_SCAN.load(Ordering::Relaxed),
            ],
            hit_by_caller: std::array::from_fn(|i| HIT_BY_CALLER[i].load(Ordering::Relaxed)),
            sensor_calls: std::array::from_fn(|i| SENSOR_CALLS[i].load(Ordering::Relaxed)),
            sensor_entries: std::array::from_fn(|i| SENSOR_ENTRIES[i].load(Ordering::Relaxed)),
            laser_march_ns: LASER_MARCH_NS.load(Ordering::Relaxed),
            step_ns_by_kind: std::array::from_fn(|i| STEP_NS_BY_KIND[i].load(Ordering::Relaxed)),
            alive_at_end: ALIVE_AT_END.load(Ordering::Relaxed),
        }
    }
}

pub fn reset() {
    for counter in [
        &BASE_MOVE_NS,
        &JUDGE_NS,
        &RESIGN_NS,
        &STEP_NS,
        &STEP_COUNT,
        &HIT_TEST_CALLS,
        &SPAWNS,
        &ALIVE_AT_END,
    ] {
        counter.store(0, Ordering::Relaxed);
    }
    for counter in HIT_TARGET_CALLS
        .iter()
        .chain(HIT_TARGET_HITS.iter())
        .chain(STEP_BY_KIND.iter())
        .chain(STEP_NS_BY_KIND.iter())
        .chain([&LASER_MARCH_ITERS, &LASER_MARCH_NS])
        .chain(HIT_BY_CALLER.iter())
        .chain(SENSOR_CALLS.iter())
        .chain(SENSOR_ENTRIES.iter())
    {
        counter.store(0, Ordering::Relaxed);
    }
}
