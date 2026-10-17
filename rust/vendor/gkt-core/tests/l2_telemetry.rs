//! L2+ 遥测对拍：事件流 + 快照 + summary（EVENTS / EVENTS_AND_SNAPSHOTS interval 1、7）。
//!
//! 金样由 Java `org.example.training.TelemetryGoldenDump` 生成（`tools/gensrc/regen_engine.ps1`）；
//! 每个 case 依次三组配置，二进制大端，浮点按原始位比较。

use gkt_core::decode::compile_fort;
use gkt_core::engine::{BattleRecorder, GameTask, TelemetryMode};

const CONFIGS: [(TelemetryMode, i32); 3] = [
    (TelemetryMode::Events, 0),
    (TelemetryMode::EventsAndSnapshots, 1),
    (TelemetryMode::EventsAndSnapshots, 7),
];

struct Reader {
    buf: Vec<u8>,
    pos: usize,
}

impl Reader {
    fn take(&mut self, n: usize) -> &[u8] {
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        s
    }

    fn i32(&mut self) -> i32 {
        i32::from_be_bytes(self.take(4).try_into().unwrap())
    }

    fn u32(&mut self) -> u32 {
        u32::from_be_bytes(self.take(4).try_into().unwrap())
    }

    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }
}

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn l2_telemetry_matches_java_golden() {
    let cases_text = std::fs::read_to_string(root().join("fixtures/golden/cases_all.txt")).unwrap();
    // telemetry.bin 约 112MB，未随开源仓库分发；缺失时跳过（可用 oracle 重新生成，见 README）
    let Ok(data) = std::fs::read(root().join("fixtures/golden/telemetry.bin")) else {
        eprintln!("skip telemetry: golden not distributed; regenerate via tools/gensrc/regen_engine.ps1");
        return;
    };

    struct Case {
        code1: String,
        code2: String,
        seed: Option<i32>,
        max_frames: i32,
    }
    let cases: Vec<Case> = cases_text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            Case {
                code1: t[0].to_string(),
                code2: t[1].to_string(),
                seed: if t[2] == "-" {
                    None
                } else {
                    Some(t[2].parse().unwrap())
                },
                max_frames: t[3].parse().unwrap(),
            }
        })
        .collect();

    let mut r = Reader { buf: data, pos: 0 };
    assert_eq!(r.u32(), 0x474B5454, "magic");
    assert_eq!(r.u32(), 1, "version");
    assert_eq!(r.u32() as usize, cases.len() * CONFIGS.len(), "run count");

    let mut run_index = 0usize;
    for (i, case) in cases.iter().enumerate() {
        let f1 = compile_fort("", &case.code1);
        let f2 = compile_fort("", &case.code2);
        for &(mode, interval) in &CONFIGS {
            let run = format!("case {i} run {run_index} ({mode:?} {interval})");

            assert_eq!(r.i32(), case.max_frames, "{run} header maxFrames");
            assert_eq!(r.i32(), case.seed.unwrap_or(0), "{run} header seed");
            assert_eq!(
                r.u8(),
                u8::from(case.seed.is_some()),
                "{run} header hasSeed"
            );
            assert_eq!(
                r.u8(),
                if mode == TelemetryMode::Events { 1 } else { 2 },
                "{run} header mode"
            );
            assert_eq!(r.i32(), interval, "{run} header interval");
            let status = r.i32();
            let frame_passed = r.i32();

            let mut recorder = BattleRecorder::new(mode, interval);
            let (_task, result) = GameTask::run_single_observed(
                &f1,
                &f2,
                case.max_frames,
                case.seed,
                Some(&mut recorder),
            );
            let telemetry = recorder.into_telemetry();

            assert_eq!(status, result.status, "{run} status");
            assert_eq!(frame_passed, result.frame_passed, "{run} framePassed");

            let golden_summary: [i32; 18] = std::array::from_fn(|_| r.i32());
            assert_eq!(
                golden_summary,
                telemetry.summary.as_array(),
                "{run} summary ({:?})",
                telemetry.summary
            );

            let event_count = r.u32() as usize;
            assert_eq!(event_count, telemetry.events.len(), "{run} event count");
            for (j, event) in telemetry.events.iter().enumerate() {
                assert_eq!(r.i32(), event.frame, "{run} event {j} frame");
                assert_eq!(r.i32(), event.event_type as i32, "{run} event {j} type");
                assert_eq!(r.i32(), event.side, "{run} event {j} side");
                assert_eq!(r.i32(), event.related_side, "{run} event {j} relatedSide");
                assert_eq!(r.i32(), event.entity_id, "{run} event {j} entityId");
                assert_eq!(r.i32(), event.unit_type, "{run} event {j} unitType");
                assert_eq!(r.u32(), event.x.to_bits(), "{run} event {j} x bits");
                assert_eq!(r.u32(), event.y.to_bits(), "{run} event {j} y bits");
                assert_eq!(r.i32(), event.amount, "{run} event {j} amount");
            }

            let snapshot_count = r.u32() as usize;
            assert_eq!(
                snapshot_count,
                telemetry.snapshots.len(),
                "{run} snapshot count"
            );
            for (j, snap) in telemetry.snapshots.iter().enumerate() {
                assert_eq!(r.i32(), snap.frame, "{run} snapshot {j} frame");
                assert_eq!(r.i32(), snap.p1_hp, "{run} snapshot {j} p1Hp");
                assert_eq!(r.i32(), snap.p2_hp, "{run} snapshot {j} p2Hp");
                assert_eq!(
                    r.i32(),
                    snap.p1_alive_units,
                    "{run} snapshot {j} p1AliveUnits"
                );
                assert_eq!(
                    r.i32(),
                    snap.p2_alive_units,
                    "{run} snapshot {j} p2AliveUnits"
                );
                assert_eq!(
                    r.i32(),
                    snap.p1_projectiles,
                    "{run} snapshot {j} p1Projectiles"
                );
                assert_eq!(
                    r.i32(),
                    snap.p2_projectiles,
                    "{run} snapshot {j} p2Projectiles"
                );
                assert_eq!(
                    r.u8(),
                    u8::from(snap.p1_base_alive),
                    "{run} snapshot {j} p1BaseAlive"
                );
                assert_eq!(
                    r.u8(),
                    u8::from(snap.p2_base_alive),
                    "{run} snapshot {j} p2BaseAlive"
                );
                assert_eq!(
                    r.u32(),
                    snap.p1_base_x.to_bits(),
                    "{run} snapshot {j} p1BaseX bits"
                );
                assert_eq!(
                    r.u32(),
                    snap.p2_base_x.to_bits(),
                    "{run} snapshot {j} p2BaseX bits"
                );
                assert_eq!(
                    r.u32(),
                    snap.p1_base_speed.to_bits(),
                    "{run} snapshot {j} p1BaseSpeed bits"
                );
                assert_eq!(
                    r.u32(),
                    snap.p2_base_speed.to_bits(),
                    "{run} snapshot {j} p2BaseSpeed bits"
                );
                assert_eq!(
                    r.u32(),
                    snap.p1_core_x.to_bits(),
                    "{run} snapshot {j} p1CoreX bits"
                );
                assert_eq!(
                    r.u32(),
                    snap.p1_core_y.to_bits(),
                    "{run} snapshot {j} p1CoreY bits"
                );
                assert_eq!(
                    r.u32(),
                    snap.p2_core_x.to_bits(),
                    "{run} snapshot {j} p2CoreX bits"
                );
                assert_eq!(
                    r.u32(),
                    snap.p2_core_y.to_bits(),
                    "{run} snapshot {j} p2CoreY bits"
                );
            }

            run_index += 1;
        }
    }
    assert_eq!(r.pos, r.buf.len(), "trailing bytes");
}
