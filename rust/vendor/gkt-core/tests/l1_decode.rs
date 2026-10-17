//! L1：fort code 解码对拍 Java 金样（`fixtures/golden/forts.txt`，由 `org.example.tools.GeoDump forts` 生成）。
//!
//! 金样来自 `fixtures/games/1P.txt` + `2P.txt` 的全部 201 个阵型，覆盖解析、编译、双侧生成参数。

use gkt_core::decode;

fn bits(tok: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(tok.trim_start_matches("0x"), 16).unwrap())
}

struct GoldenFort {
    code: String,
    core_type: i32,
    core_x: i32,
    core_y: i32,
    unit_count: usize,
    unit_type: Vec<i32>,
    x: Vec<i32>,
    y: Vec<i32>,
    r: Vec<i32>,
    spawn0: Vec<(f32, f32, i32)>,
    spawn1: Vec<(f32, f32, i32)>,
}

fn parse_golden(text: &str) -> Vec<GoldenFort> {
    let mut out: Vec<GoldenFort> = Vec::new();
    for line in text.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        match t[0] {
            "fort" => out.push(GoldenFort {
                code: t[2].to_string(),
                core_type: 0,
                core_x: 0,
                core_y: 0,
                unit_count: 0,
                unit_type: Vec::new(),
                x: Vec::new(),
                y: Vec::new(),
                r: Vec::new(),
                spawn0: Vec::new(),
                spawn1: Vec::new(),
            }),
            "core" => {
                let f = out.last_mut().unwrap();
                f.core_type = t[1].parse().unwrap();
                f.core_x = t[2].parse().unwrap();
                f.core_y = t[3].parse().unwrap();
            }
            "unitcount" => {
                out.last_mut().unwrap().unit_count = t[1].parse().unwrap();
            }
            "unit" => {
                let f = out.last_mut().unwrap();
                assert_eq!(f.unit_type.len(), t[1].parse::<usize>().unwrap());
                f.unit_type.push(t[2].parse().unwrap());
                f.x.push(t[3].parse().unwrap());
                f.y.push(t[4].parse().unwrap());
                f.r.push(t[5].parse().unwrap());
            }
            "spawn0" | "spawn1" => {
                let f = out.last_mut().unwrap();
                let entry = (bits(t[2]), bits(t[3]), t[4].parse().unwrap());
                if t[0] == "spawn0" {
                    f.spawn0.push(entry);
                } else {
                    f.spawn1.push(entry);
                }
            }
            other => panic!("unknown token {other}"),
        }
    }
    out
}

#[test]
fn decode_matches_java_golden() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden = std::fs::read_to_string(root.join("fixtures/golden/forts.txt"))
        .expect("missing forts golden; run tools/gensrc/regen_geo.ps1");
    let fixtured = [
        std::fs::read_to_string(root.join("fixtures/games/1P.txt")).unwrap(),
        std::fs::read_to_string(root.join("fixtures/games/2P.txt")).unwrap(),
    ];
    let parsed: Vec<decode::CompiledFort> = fixtured
        .iter()
        .flat_map(|c| decode::parse_forts(c))
        .map(|mut f| {
            // parse_forts 保留名字，金样不含名字
            f.name = String::new();
            f
        })
        .collect();

    let golden = parse_golden(&golden);
    assert_eq!(golden.len(), 201, "golden fort count");
    assert_eq!(parsed.len(), golden.len(), "parsed fort count");

    for (idx, (g, p)) in golden.iter().zip(parsed.iter()).enumerate() {
        assert_eq!(p.core_type, g.core_type, "fort {idx} coreType");
        assert_eq!(p.core_x, g.core_x, "fort {idx} coreX");
        assert_eq!(p.core_y, g.core_y, "fort {idx} coreY");
        assert_eq!(p.unit_count, g.unit_count, "fort {idx} unitCount");
        assert_eq!(p.unit_type, g.unit_type, "fort {idx} types");
        assert_eq!(p.x, g.x, "fort {idx} x");
        assert_eq!(p.y, g.y, "fort {idx} y");
        assert_eq!(p.r, g.r, "fort {idx} r");

        // 同一 code 直接 compile_fort 必须与文件解析结果一致
        let direct = decode::compile_fort("", &g.code);
        assert_eq!(&direct.unit_type, &g.unit_type, "fort {idx} direct compile");
        assert_eq!(&direct.x, &g.x, "fort {idx} direct compile x");

        for k in 0..g.unit_count {
            let (ux, uy, rr) = decode::unit_spawn(p, 0, k);
            let (gx, gy, gr) = g.spawn0[k];
            assert_eq!(
                (ux.to_bits(), uy.to_bits(), rr),
                (gx.to_bits(), gy.to_bits(), gr),
                "fort {idx} unit {k} spawn0"
            );
            let (ux, uy, rr) = decode::unit_spawn(p, 1, k);
            let (gx, gy, gr) = g.spawn1[k];
            assert_eq!(
                (ux.to_bits(), uy.to_bits(), rr),
                (gx.to_bits(), gy.to_bits(), gr),
                "fort {idx} unit {k} spawn1"
            );
        }
        let (cx, cy) = decode::core_spawn(p, 0);
        assert_eq!((cx, cy), (g.core_x, g.core_y));
        let (cx, cy) = decode::core_spawn(p, 1);
        assert_eq!((cx, cy), (-g.core_x, g.core_y));
    }
}

#[test]
fn to_xyr_matches_oracle_definition() {
    assert_eq!(decode::to_xyr(b"00000"), [0, 0, 0]);
    // 'z' 在 pskey 中 index=35（数字10 + 小写26）
    assert_eq!(decode::to_xyr(b"0000z"), [0, 35, 0]);
    // 'Z' index=61；注意权重是 61 的幂但字符表有 62 个字符（oracle 原样）
    assert_eq!(decode::to_xyr(b"0000Z"), [0, 61, 0]);
    assert_eq!(decode::to_xyr(b"ZZZZZ"), [672, 905, 858]);
    // 非法字符：Java indexOf 返回 -1 参与同一 int 运算；除法向零截断
    assert_eq!(decode::to_xyr(b"00!00"), [-3, -721, 0]);
}

#[test]
fn seed_factor_matches_main_setup() {
    let f = decode::compile_fort("", "0001dwsincah7jnQeeDaaOU8JT");
    let expect = ((f.core_x + 190) % 168 + 48) * ((f.core_y + 400) % 168 + 48);
    assert_eq!(decode::seed_factor(&f), expect);
}
