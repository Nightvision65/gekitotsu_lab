//! L2：对局结果对拍 Java 金样（`fixtures/golden/battles.txt`，由 `GameDump battles` 生成）。
//!
//! 比较 status/winnerHp/framePassed/finalRandomState/p1Hp/p2Hp，timeUsed 不在比较范围。

use gkt_core::decode::compile_fort;
use gkt_core::engine::GameTask;

struct Case {
    code1: String,
    code2: String,
    seed: Option<i32>,
    max_frames: i32,
}

struct Golden {
    status: i32,
    winner_hp: i32,
    frame_passed: i32,
    final_rng: i32,
    hp1: i32,
    hp2: i32,
}

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn l2_battles_match_java_golden() {
    let cases_text = std::fs::read_to_string(root().join("fixtures/golden/cases_all.txt")).unwrap();
    let golden_text = std::fs::read_to_string(root().join("fixtures/golden/battles.txt"))
        .expect("missing battles golden; run tools/gensrc/regen_engine.ps1");

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
    let golden: Vec<Golden> = golden_text
        .lines()
        .filter(|l| l.starts_with("b "))
        .map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            Golden {
                status: t[1].parse().unwrap(),
                winner_hp: t[2].parse().unwrap(),
                frame_passed: t[3].parse().unwrap(),
                final_rng: t[4].parse().unwrap(),
                hp1: t[5].parse().unwrap(),
                hp2: t[6].parse().unwrap(),
            }
        })
        .collect();
    assert_eq!(cases.len(), golden.len(), "case/golden count mismatch");
    assert!(!cases.is_empty());

    for (i, (case, g)) in cases.iter().zip(golden.iter()).enumerate() {
        let f1 = compile_fort("", &case.code1);
        let f2 = compile_fort("", &case.code2);
        let (task, r) = GameTask::run_single(&f1, &f2, case.max_frames, case.seed);
        assert_eq!(
            r.status, g.status,
            "case {i} status ({} vs {})",
            case.code1, case.code2
        );
        assert_eq!(r.winner_hp, g.winner_hp, "case {i} winnerHp");
        assert_eq!(r.frame_passed, g.frame_passed, "case {i} framePassed");
        assert_eq!(task.rng_seed, g.final_rng, "case {i} finalRandomState");
        assert_eq!(task.hp[0], g.hp1, "case {i} p1Hp");
        assert_eq!(task.hp[1], g.hp2, "case {i} p2Hp");
    }
}
