#!/usr/bin/env python3
"""固定侧位的完整阵池批评测；按已核验的逐局证据复用，支持断点续跑。"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import time
import zipfile

import ai


def fingerprint(args):
    jar = Path(args.jar or ai.default_jar())
    with zipfile.ZipFile(jar) as archive:
        dll = ai.digest(archive.read("native/gkt_jni.dll"))
    return {"backend": "java", "binarySha256": ai.file_hash(jar),
            "adapterSha256": ai.file_hash(ai.__file__), "policy": ai.POLICY,
            "dllSha256": dll}


def physical_matrices(data):
    # reverse的索引仍指物理1P/2P；它们分别索引原输入p2/p1。
    yield "forward", data["p1"], data["p2"], data.get("forward", [])
    yield "reverse", data["p2"], data["p1"], data.get("reverse", [])


def score(outcomes, side):
    counts = {"W": 0, "L": 0, "D": 0, "U": 0, "E": 0}
    for status, hp, frames in outcomes:
        if status not in (-1, 0, 1, 2) or frames <= 0:
            raise ValueError("非法对局结果，不能作为平局")
        key = "U" if status == -1 else "D" if status == 0 else "W" if status == side else "L"
        counts[key] += 1
    n = len(outcomes)
    if not n:
        raise ValueError("空评测池")
    return {**counts, "N": n, "scorePct": 100 * (counts["W"] + .5 * (counts["D"] + counts["U"])) / n}


def ingest(data, artifact, raw_sha, side, candidates, opponents, cache):
    for matrix, p1, p2, rows in physical_matrices(data):
        seen = set()
        for index, row in enumerate(rows):
            a, b = row["p1Index"], row["p2Index"]
            if not (0 <= a < len(p1) and 0 <= b < len(p2)) or (a, b) in seen:
                raise ValueError("矩阵索引越界或重复")
            seen.add((a, b))
            own, opp = (p1[a], p2[b]) if side == 1 else (p2[b], p1[a])
            if own["code"] not in candidates or opp["code"] not in opponents:
                continue
            outcome = [row[k] for k in ("status", "winnerHp", "frames")]
            score([outcome], side)
            key = own["code"], opp["code"]
            if key in cache and cache[key][0] != outcome:
                raise ValueError("同一冻结输入的缓存对局不一致")
            cache.setdefault(key, (outcome, {"artifact": artifact, "rawSha256": raw_sha,
                                           "matrix": matrix, "rowIndex": index}))
        if rows and len(seen) != len(p1) * len(p2):
            raise ValueError("已发布的批矩阵缺局")


def batches(candidates, opponents, cache, batch_size, max_jobs=12800):
    # 同缺口候选成组；保留池顺序、重复条目及等权分母。
    groups = {}
    for candidate in candidates:
        missing = tuple(i for i, opp in enumerate(opponents)
                        if (candidate["code"], opp["code"]) not in cache)
        if missing:
            groups.setdefault(missing, []).append(candidate)
    for indices, group in groups.items():
        width = min(batch_size, max(1, max_jobs // min(len(indices), max_jobs)))
        for start in range(0, len(group), width):
            chunk = group[start:start + width]
            step = max_jobs // len(chunk)
            for offset in range(0, len(indices), step):
                yield chunk, [opponents[i] for i in indices[offset:offset + step]]


def save(path, data):
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(ai.dumps(data), encoding="utf-8")
    temp.replace(path)


def run(args):
    if os.environ.get("CONDA_DEFAULT_ENV") != "opencode":
        raise ValueError("请先conda activate opencode")
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    source = Path(args.candidates)
    candidates = ai.read_json(source) if source.suffix == ".json" else ai.formations(source)
    if isinstance(candidates, dict):
        candidates = candidates["candidates"]
    if not candidates or len({c["code"] for c in candidates}) != len(candidates):
        raise ValueError("候选必须非空且按纯码唯一")
    for candidate in candidates:
        ai.validate_code(candidate["code"])
    opponents = ai.formations(args.opponents)
    fp = fingerprint(args)
    manifest = {"schema": "gkt-pool-side-v1", "side": args.side, "maxFrames": args.max_frames,
                "seedPolicy": "native-default", "scoringPolicy": ai.POLICY,
                "candidateInputSha256": ai.file_hash(source), "opponentInputSha256": ai.file_hash(args.opponents),
                "runnerSha256": ai.file_hash(__file__), "fingerprint": fp,
                "candidateCount": len(candidates), "opponentCount": len(opponents)}
    frozen = out / "manifest.json"
    if frozen.exists() and ai.read_json(frozen) != manifest:
        raise ValueError("输入或运行器指纹改变，必须使用新输出目录")
    if not frozen.exists():
        frozen.write_text(ai.dumps(manifest), encoding="utf-8")
    cache = {}
    own_codes, opp_codes = {c["code"] for c in candidates}, {o["code"] for o in opponents}
    store = Path(args.store).resolve()
    start = time.monotonic()
    with ai.connect(store) as db:
        for row in db.execute("SELECT id,request,fingerprint FROM artifacts WHERE action='battle'").fetchall():
            req = json.loads(row["request"])
            if json.loads(row["fingerprint"]) != fp or req.get("maxFrames") != args.max_frames:
                continue
            # 只接受本批评测器支持的固定默认seed；不混入其他求解条件。
            if req.get("seed") is not None:
                continue
            verified = ai.verified(db, row["id"])
            data = ai.read_json(verified["raw_path"])
            ingest(data, row["id"], verified["raw_sha256"], args.side, own_codes, opp_codes, cache)
        initial = sum((c["code"], o["code"]) in cache for c in candidates for o in opponents)
        previous = ai.read_json(out / "progress.json") if (out / "progress.json").exists() else {}
        new_games = previous.get("newGames", 0)
        def checkpoint(status):
            done = sum(all((c["code"], o["code"]) in cache for o in opponents) for c in candidates)
            save(out / "progress.json", {"status": status, "completedCandidates": done,
                 "totalCandidates": len(candidates), "cachedGamesAtStart": initial,
                 "newGames": new_games, "elapsedThisSessionSeconds": round(time.monotonic()-start, 2)})
        checkpoint("running")
        for chunk, pool in batches(candidates, opponents, cache, args.batch_size):
            if (out / "STOP").exists() or time.monotonic()-start >= args.time_limit:
                checkpoint("stopped")
                return
            p1, p2 = (chunk, pool) if args.side == 1 else (pool, chunk)
            req = {"action": "battle", "p1": [{"name": c.get("name", ""), "code": c["code"]} for c in p1],
                   "p2": [{"name": c.get("name", ""), "code": c["code"]} for c in p2],
                   "bothSides": False, "maxFrames": args.max_frames, "threads": args.threads, "maxJobs": 12800}
            try:
                result = ai.compute(db, store, req, args, "java")
                row = ai.verified(db, result["artifact"])
                ingest(ai.read_json(row["raw_path"]), result["artifact"], row["raw_sha256"],
                       args.side, own_codes, opp_codes, cache)
                if not result["cached"]:
                    new_games += len(chunk) * len(pool)
                checkpoint("running")
                print(f"已完成本批{len(chunk)}阵×{len(pool)}局，累计新增{new_games}局", flush=True)
            except Exception:
                checkpoint("failed")
                raise
        records = []
        for candidate in candidates:
            entries = [cache[(candidate["code"], opp["code"])] for opp in opponents]
            outcomes = [entry[0] for entry in entries]
            sources, source_by_opponent = [], []
            source_indices = {}
            for _, proof in entries:
                key = proof["artifact"]
                if key not in source_indices:
                    source_indices[key] = len(sources)
                    sources.append({"artifact": key, "rawSha256": proof["rawSha256"]})
                source_by_opponent.append([source_indices[key], proof["matrix"], proof["rowIndex"]])
            records.append({"name": candidate.get("name", ""), "code": candidate["code"],
                            "sha256": ai.digest(candidate["code"].encode()), "side": args.side,
                            **score(outcomes, args.side), "outcomes": outcomes,
                            "sources": sources, "sourceByOpponent": source_by_opponent})
        results = {"manifest": manifest, "store": str(store), "candidates": records}
        save(out / "results.json", results)
        save(out / "gate.json", {"passed": True, "N": len(opponents), "candidates": len(records),
                                 "E": 0, "resultsSha256": ai.file_hash(out / "results.json")})
        checkpoint("complete")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for key in ("candidates", "opponents", "store", "out"):
        p.add_argument("--" + key, required=True)
    p.add_argument("--side", type=int, choices=(1, 2), required=True)
    p.add_argument("--threads", type=int, default=32)
    p.add_argument("--batch-size", type=int, default=64, choices=range(1, 65))
    p.add_argument("--max-frames", type=int, default=65536)
    p.add_argument("--timeout", type=float, default=300)
    p.add_argument("--time-limit", type=float, default=3600)
    p.add_argument("--jar")
    p.add_argument("--java")
    args = p.parse_args()
    if args.max_frames <= 0 or args.timeout <= 0 or args.time_limit <= 0 or args.threads <= 0:
        p.error("帧数、线程和超时必须为正数")
    run(args)


if __name__ == "__main__":
    main()
