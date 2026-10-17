"""汇总标准200阵集胜率；保留超时异常并核验完整矩阵。"""
import argparse
import json
import os
from pathlib import Path
from kit_lab import ROOT, read_forts, sha

def summary(rows, candidate_side):
    counts = {"win": 0, "lose": 0, "draw": 0, "timeout": 0, "error": 0}
    for row in rows:
        s = row["status"]
        key = "win" if s == candidate_side else "lose" if s == 3-candidate_side else "draw" if s == 0 else "timeout" if s == -1 else "error"
        counts[key] += 1
    n = len(rows)
    points = counts["win"] + 0.5 * (counts["draw"] + counts["timeout"])
    return {**counts, "n": n, "effectiveDraw": counts["draw"]+counts["timeout"], "scoringPolicy": "timeout-as-draw-v1", "kitWinRatePct": 100 * points / n if n else None,
            "strictWinRatePct": 100 * counts["win"] / n,
            "scoreAllPct": 100 * points / n,
            "rankEligible": counts["error"] == 0}

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("input", type=Path)
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args()
    if os.environ.get("CONDA_DEFAULT_ENV") != "opencode":
        p.error("请先激活 conda opencode")
    if a.out.exists():
        p.error("输出已存在，请使用新路径")
    reference = ROOT / "benchmarks/standard200/opponents.txt"
    manifest = json.loads(reference.with_name("manifest.json").read_text(encoding="utf-8"))
    if sha(reference.read_bytes()) != manifest["normalizedFileSha256"]:
        p.error("标准200文件哈希与manifest不一致，先查明基准是否被修改")
    pool = read_forts(reference, bounds=True)
    data = json.loads(a.input.read_text(encoding="utf-8"))
    if data.get("action") != "battle" or len(pool) != 200:
        p.error("需要 battle 输出和恰好200阵的基准")
    if [f["code"] for f in data["p2"]] != [f["code"] for f in pool]:
        p.error("p2阵集与冻结的标准200阵集不一致，不能混入同一排行榜")
    n = len(data["p1"])
    expected = {(i, j) for i in range(n) for j in range(200)}
    rows = data["forward"]
    if len(rows) != n * 200 or {(r["p1Index"], r["p2Index"]) for r in rows} != expected:
        p.error("正向矩阵不完整或存在重复对局")
    reverse = data.get("reverse")
    if reverse is not None and (len(reverse) != n * 200 or {(r["p2Index"], r["p1Index"]) for r in reverse} != expected):
        p.error("反向矩阵不完整或存在重复对局")
    result = []
    for i, candidate in enumerate(data["p1"]):
        forward = summary([r for r in rows if r["p1Index"] == i], 1)
        backward = summary([r for r in reverse if r["p2Index"] == i], 2) if reverse is not None else None
        result.append({"candidateIndex": i, "name": candidate["name"], "codeSha256": sha(candidate["code"].encode("ascii")),
                       "primary1P": forward, "auxiliary2P": backward})
    result.sort(key=lambda r: (r["primary1P"]["rankEligible"], r["primary1P"]["kitWinRatePct"] or 0), reverse=True)
    previous_rate, current_rank = None, None
    for index, row in enumerate(result, 1):
        s = row["primary1P"]
        if s["rankEligible"] and s["kitWinRatePct"] != previous_rate:
            current_rank, previous_rate = index, s["kitWinRatePct"]
        row["rank"] = current_rank if s["rankEligible"] else None
    out = {"metric": "(W + 0.5*(D + U))/200 against fixed standard200, candidate as 1P",
           "scoringPolicy": "timeout-as-draw-v1",
           "scorerSha256": sha(Path(__file__).read_bytes()),
           "note": "未决按平局且固定200局分母；原始D/U保留，计分平局为D+U。异常不晋级；预算与赛制另行审计。",
           "benchmarkId": manifest["id"], "benchmarkSha256": sha(reference.read_bytes()), "sourceResult": str(a.input.resolve()),
           "sourceResultSha256": sha(a.input.read_bytes()), "engine": data["engine"],
           "provenance": data["provenance"], "candidates": result}
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    for row in result:
        s = row["primary1P"]
        print(f"{row['name']}：200阵胜率 {s['kitWinRatePct']}%，胜{s['win']} 负{s['lose']} 平{s['draw']} 超时{s['timeout']} 异常{s['error']}")
    print(f"完成：{a.out.resolve()}")

if __name__ == "__main__":
    main()
