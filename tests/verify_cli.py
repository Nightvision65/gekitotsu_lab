"""真实CLI闭环：GUI分析、稀疏诊断、SQLite查询、详细重放及缓存核验。"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

from verify_integration import ROOT, ai


def main():
    if os.environ.get("CONDA_DEFAULT_ENV") != "opencode":
        raise SystemExit("请先conda activate opencode")
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    out = Path(args.out)
    if out.exists():
        raise SystemExit("拒绝覆盖验证记录")
    folder = out.parent/(out.stem+"-data")
    folder.mkdir(parents=True, exist_ok=False)
    store = folder/"store.sqlite"
    calls = []
    def call(*words):
        proc = subprocess.run([sys.executable, str(ROOT/"tools/ai/ai.py"), "--store", str(store), *words],
                              cwd=ROOT, capture_output=True, timeout=180)
        assert proc.returncode == 0, proc.stderr.decode("utf-8", errors="replace")
        assert len(proc.stdout) <= 12000
        value = json.loads(proc.stdout)
        assert "error" not in value, value
        calls.append({"args": list(words), "bytes": len(proc.stdout)})
        return value
    candidate = str(ROOT/"docs/ai/examples/candidate.txt")
    opponents = str(ROOT/"docs/ai/examples/opponents.txt")
    artifacts = []
    for command in ("probe", "catalog", "capabilities"):
        artifacts.append(call(command)["artifact"])
    formation = call("formation", "--file", candidate)["artifact"]
    artifacts.append(formation)
    units = call("query", "--artifact", formation, "--table", "units", "--fields", "slot,id,name,x,y,r,cost")
    assert units["rows"] and units["rows"][0]["slot"] == 0
    for command in ("evaluate", "contribution", "matchup", "battlefield"):
        words = ["evaluate"] if command == "evaluate" else ["analyze", "--kind", command]
        result = call(*words, "--p1", candidate, "--p2", opponents)
        artifacts.append(result["artifact"])
    diagnosis = call("diagnose", "--p1", candidate, "--p2", opponents)
    assert diagnosis["count"] == 6
    basics = [b["artifact"] for b in diagnosis["battles"]]
    artifacts.extend(basics)
    cohort = call("cohort", "--manifest", diagnosis["manifest"], "--limit", "10")
    assert cohort["battles"] == 6 and cohort["complete"] and cohort["rows"]
    assert any("ground" in row["landingPositionStats"] for row in cohort["rows"])
    baseline = basics[0]
    for kind in ("half_hp", "recovered_full_hp", "death", "launch", "land"):
        call("query", "--artifact", baseline, "--table", "events", "--kind", kind)
    call("query", "--artifact", baseline, "--table", "flights", "--side", "0", "--surface", "ground")
    assert call("compare", "--baseline", baseline, "--candidate", baseline)["totalChangedUnits"] == 0
    replay = call("replay", "--artifact", baseline)
    assert replay["resultMatchesBaseline"] and not replay["cached"]
    detail = replay["artifact"]
    artifacts.append(detail)
    assert call("replay", "--artifact", baseline)["cached"]
    stats = call("query", "--artifact", detail, "--table", "unitStats", "--side", "0", "--slot", "0")
    assert len(stats["rows"]) == 1 and stats["rows"][0]["slot"] == 0
    sources = call("query", "--artifact", detail, "--table", "damageSources", "--side", "0", "--slot", "0")
    if sources["rows"]:
        assert abs(sum(row["percentage"] for row in sources["rows"])-100) < 1e-8
    events = call("query", "--artifact", detail, "--table", "combatEvents", "--kind", "damage", "--limit", "2")
    assert events["rows"] and events["nextOffset"] == 2
    following = call("query", "--artifact", detail, "--table", "combatEvents", "--kind", "damage", "--offset", "2", "--limit", "2")
    assert not set(e["sequence"] for e in events["rows"]) & set(e["sequence"] for e in following["rows"])
    with ai.connect(store) as db:
        raw = ai.read_json(ai.verified(db, detail)["raw_path"])
    obj_map = {o["entityId"]: o for o in raw["objects"]}
    hit = next(e for e in raw["combatEvents"] if e["kind"] == "damage" and e["cause"] == "attack"
               and e["sourceObjectId"] is not None)
    filtered = call("query", "--artifact", detail, "--table", "combatEvents", "--kind", "damage",
                    "--attacker-entity-id", str(hit["attackerEntityId"]), "--limit", "2")
    assert filtered["rows"] and all(e["attackerEntityId"] == hit["attackerEntityId"] for e in filtered["rows"])
    current = hit["sourceObjectId"]
    while current is not None:
        rows = call("query", "--artifact", detail, "--table", "objects", "--entity-id", str(current))["rows"]
        assert len(rows) == 1 and rows[0]["entityId"] == current
        current = obj_map[current]["parentEntityId"]
    call("query", "--artifact", detail, "--table", "healingSources", "--limit", "5")
    call("query", "--artifact", detail, "--table", "flightSamples", "--side", "0", "--from-frame", "0", "--to-frame", "1200")
    verifications = [call("verify", "--artifact", artifact) for artifact in artifacts]
    assert all(v["verified"] for v in verifications)
    report = {"status": "passed", "store": str(store.resolve()),
              "manifest": diagnosis["manifest"], "baseline": baseline, "detail": detail,
              "calls": calls, "verifications": verifications,
              "checks": ["所有stdout在12000字节以内", "GUI三种分析入库", "六场基础诊断与落地统计",
                         "核心统计与伤害来源查询", "指定基础对局详细重放结果一致", "重复重放命中缓存",
                         "事件分页不重复", "具体攻击者筛选与对象父链查询", "全部索引与原始证据一致"]}
    out.write_text(ai.dumps(report), encoding="utf-8")
    print(f"CLI闭环通过：{len(calls)}次调用，{len(verifications)}份存证核验，输出均在预算内。")


if __name__ == "__main__":
    main()
