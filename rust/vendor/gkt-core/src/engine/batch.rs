//! 批量并行入口（AGENTS §1）：round-robin 分片 + `std::thread::scope`，零第三方依赖。
//!
//! - `run_batch`：无观测器，返回显式种子/终局 HP。
//! - `run_batch_with`：每个 job 一个观测器（如 `BattleRecorder`）；观测器原样还回，
//!   调用方可再 `into_telemetry()`。
//!
//! 结果顺序与 `jobs` 一致；同一批内不同 job 完全独立（各自新建 `GameTask`）。

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{BattleObserver, GameTask, Result};
use crate::decode::CompiledFort;

#[derive(Clone, Copy, Debug)]
pub struct BatchJob<'a> {
    pub p1: &'a CompiledFort,
    pub p2: &'a CompiledFort,
    pub max_frames: i32,
    pub seed: Option<i32>,
}

#[derive(Clone, Copy, Debug)]
pub struct BatchOutcome {
    pub result: Result,
    pub final_rng: i32,
    pub p1_hp: i32,
    pub p2_hp: i32,
}

fn finish(task: &GameTask, result: Result) -> BatchOutcome {
    BatchOutcome {
        result,
        final_rng: task.rng_seed,
        p1_hp: task.hp[0],
        p2_hp: task.hp[1],
    }
}

/// 动态调度：各线程从共享计数器逐个领取 job。
/// 对局帧数差异可达百倍（最长 33,788 帧），静态 round-robin 在 20 线程时
/// 最忙线程超载 +52%（帧数模拟）；逐个领取使负载均衡接近理想，
/// 且每 job 一次 fetch_add 的开销相对毫秒级单局可忽略。结果与线程数无关。
pub fn run_batch(jobs: &[BatchJob], threads: usize) -> Vec<BatchOutcome> {
    if jobs.is_empty() {
        return Vec::new();
    }
    let threads = threads.max(1).min(jobs.len());
    let outcomes: Mutex<Vec<Option<BatchOutcome>>> = Mutex::new(vec![None; jobs.len()]);
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let outcomes = &outcomes;
            let next = &next;
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= jobs.len() {
                    break;
                }
                let job = &jobs[i];
                let (task, result) = GameTask::run_single(job.p1, job.p2, job.max_frames, job.seed);
                outcomes.lock().unwrap()[i] = Some(finish(&task, result));
            });
        }
    });
    outcomes
        .into_inner()
        .unwrap()
        .into_iter()
        .map(Option::unwrap)
        .collect()
}

/// 与 `run_batch` 相同，但每个 job 配一个观测器；观测器跑完原样还回 `observers`。
pub fn run_batch_with<T: BattleObserver + Send>(
    jobs: &[BatchJob],
    observers: &mut [Option<T>],
    threads: usize,
) -> Vec<BatchOutcome> {
    assert_eq!(
        observers.len(),
        jobs.len(),
        "observers 数量必须与 jobs 一致"
    );
    if jobs.is_empty() {
        return Vec::new();
    }
    let threads = threads.max(1).min(jobs.len());
    let cells: Vec<Mutex<Option<T>>> = observers.iter_mut().map(|o| Mutex::new(o.take())).collect();
    let outcomes: Mutex<Vec<Option<BatchOutcome>>> = Mutex::new(vec![None; jobs.len()]);
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let outcomes = &outcomes;
            let cells = &cells;
            let next = &next;
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= jobs.len() {
                    break;
                }
                let job = &jobs[i];
                let mut guard = cells[i].lock().unwrap();
                let observer = guard.as_mut().map(|o| o as &mut dyn BattleObserver);
                let (task, result) = GameTask::run_single_observed(
                    job.p1,
                    job.p2,
                    job.max_frames,
                    job.seed,
                    observer,
                );
                outcomes.lock().unwrap()[i] = Some(finish(&task, result));
            });
        }
    });
    for (slot, cell) in observers.iter_mut().zip(cells) {
        *slot = cell.into_inner().unwrap();
    }
    outcomes
        .into_inner()
        .unwrap()
        .into_iter()
        .map(Option::unwrap)
        .collect()
}
