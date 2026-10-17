//! 批量并行入口对拍：`run_batch`/`run_batch_with` 与逐局运行完全一致，且与线程数无关。

use gkt_core::decode::compile_fort;
use gkt_core::engine::{
    BatchJob, BattleRecorder, GameTask, TelemetryMode, run_batch, run_batch_with,
};

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_cases(limit: usize) -> Vec<(String, String, Option<i32>, i32)> {
    std::fs::read_to_string(root().join("fixtures/golden/cases_all.txt"))
        .expect("missing cases_all.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .take(limit)
        .map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            (
                t[0].to_string(),
                t[1].to_string(),
                if t[2] == "-" {
                    None
                } else {
                    Some(t[2].parse().unwrap())
                },
                t[3].parse().unwrap(),
            )
        })
        .collect()
}

#[test]
fn batch_matches_sequential_and_thread_counts() {
    let cases = load_cases(48);
    let forts: Vec<_> = {
        let mut all: Vec<(&str, gkt_core::decode::CompiledFort)> = Vec::new();
        for (c1, c2, _, _) in &cases {
            for code in [c1.as_str(), c2.as_str()] {
                if !all.iter().any(|(c, _)| *c == code) {
                    all.push((code, compile_fort("", code)));
                }
            }
        }
        all
    };
    let fort = |code: &str| &forts.iter().find(|(c, _)| *c == code).unwrap().1;
    let jobs: Vec<BatchJob> = cases
        .iter()
        .map(|(c1, c2, seed, max_frames)| BatchJob {
            p1: fort(c1),
            p2: fort(c2),
            max_frames: *max_frames,
            seed: *seed,
        })
        .collect();

    let sequential: Vec<_> = jobs
        .iter()
        .map(|job| {
            let (task, result) = GameTask::run_single(job.p1, job.p2, job.max_frames, job.seed);
            (result, task.rng_seed, task.hp[0], task.hp[1])
        })
        .collect();

    for threads in [1usize, 2, 3, 8] {
        let outcomes = run_batch(&jobs, threads);
        for (i, outcome) in outcomes.iter().enumerate() {
            let (result, rng, hp1, hp2) = sequential[i];
            assert_eq!(outcome.result, result, "job {i} result, threads {threads}");
            assert_eq!(outcome.final_rng, rng, "job {i} rng, threads {threads}");
            assert_eq!(outcome.p1_hp, hp1, "job {i} p1_hp, threads {threads}");
            assert_eq!(outcome.p2_hp, hp2, "job {i} p2_hp, threads {threads}");
        }
    }

    // 遥测批量：与逐局 recorder 结果逐位一致
    let mut recorders: Vec<Option<BattleRecorder>> = cases
        .iter()
        .map(|_| Some(BattleRecorder::new(TelemetryMode::EventsAndSnapshots, 7)))
        .collect();
    let outcomes = run_batch_with(&jobs, &mut recorders, 4);
    assert_eq!(outcomes.len(), cases.len());
    for (i, outcome) in outcomes.iter().enumerate() {
        let (result, rng, hp1, hp2) = sequential[i];
        assert_eq!(outcome.result, result, "telemetry job {i} result");
        assert_eq!(outcome.final_rng, rng, "telemetry job {i} rng");
        assert_eq!(outcome.p1_hp, hp1);
        assert_eq!(outcome.p2_hp, hp2);

        let mut inline = BattleRecorder::new(TelemetryMode::EventsAndSnapshots, 7);
        let job = &jobs[i];
        let (_task, _result) = GameTask::run_single_observed(
            job.p1,
            job.p2,
            job.max_frames,
            job.seed,
            Some(&mut inline),
        );
        let expected = inline.into_telemetry();
        let actual = recorders[i].take().unwrap().into_telemetry();
        assert_eq!(
            actual.summary, expected.summary,
            "telemetry job {i} summary"
        );
        assert_eq!(actual.events, expected.events, "telemetry job {i} events");
        assert_eq!(
            actual.snapshots, expected.snapshots,
            "telemetry job {i} snapshots"
        );
    }
}
