"""真实引擎回归：普通/观察结果一致、JNI对照、单位身份及事件完整性。"""
import argparse
from collections import Counter
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("gkt_ai", ROOT / "tools/ai/ai.py")
ai = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ai)


def call_java(req, tmp, name):
    inp, out = tmp/f"{name}-in.json", tmp/f"{name}-out.json"
    inp.write_text(ai.dumps(req), encoding="utf-8")
    subprocess.run(["java", "-Djava.awt.headless=true", "-Dfile.encoding=UTF-8", "-jar",
                    str(ROOT/"target/gekitotsu_lab-0.6.7.7.jar"), "--ai", str(inp), str(out)],
                   cwd=tmp, check=True, timeout=180, capture_output=True)
    return ai.read_json(out)


def observe(req, tmp, name):
    inp, out = tmp/f"{name}-in.json", tmp/f"{name}-out.json"
    inp.write_text(ai.dumps(req), encoding="utf-8")
    binary="gkt-detail.exe" if req.get("mode")=="detailed" else "gkt-observe.exe"
    process=subprocess.run([str(ROOT/"rust/target/release"/binary), str(inp), str(out)],
                   cwd=tmp, timeout=30, capture_output=True)
    if process.returncode:
        folder=ROOT/"ai-data/verification/failures";folder.mkdir(parents=True,exist_ok=True)
        failure=folder/f"{name}-{time.time_ns()}.json"
        failure.write_text(ai.dumps({"request":req,"binary":binary,"exitCode":process.returncode,"stderr":process.stderr.decode("utf-8",errors="replace")}),encoding="utf-8")
        raise RuntimeError(f"{binary}退出码0x{process.returncode & 0xffffffff:08x}；现场已保留：{failure}")
    return ai.read_json(out)


def assert_trace(data, codes):
    assert data["summary"]["complete"] and data["summary"]["droppedEvents"] == 0
    end = data["result"]["frames"]
    units = {u["entityId"]:u for u in data["units"]}
    initial = {(u["side"],u["slot"]):u for u in units.values() if u["slot"] is not None}
    for side, code in enumerate(codes):
        assert sum(1 for s,_ in initial if s==side) == len(code)//6
        for slot in range(len(code)//6):
            type_id = {"0":0,"1":61,"2":62}[code[0]] if slot==0 else ai.ALPHABET.index(code[slot*6])
            assert initial[(side,slot)]["typeId"] == type_id
    deaths = Counter();halves=Counter();fulls=Counter();recoveries=Counter()
    assert [e["frame"] for e in data["events"]]==sorted(e["frame"] for e in data["events"])
    for e in data["events"]:
        assert 0 <= e["frame"] <= end
        assert e["kind"] not in ("unit_damage", "core_damage", "unit_heal")
        if e["kind"] == "death":
            deaths[e["entityId"]] += 1
            assert units[e["entityId"]]["deathFrame"] == e["frame"]
        if e["kind"] == "half_hp":
            assert 2*e["hp"] < e["maxHp"]
            halves[e["entityId"]]+=1
            assert e["detail"]["episode"]==halves[e["entityId"]]
            assert e["frame"]<=e["detail"]["confirmedFrame"]<=end
            if e["detail"]["confirmation"] in ("persisted_beyond_window","recovered_after_window"):
                assert e["detail"]["durationObservedFrames"]>data["request"]["healthRecoveryWindowFrames"] or data["request"]["healthRecoveryWindowFrames"]==0
        if e["kind"] == "recovered_full_hp":
            fulls[e["entityId"]]+=1
            assert e["hp"]>=e["maxHp"] and e["detail"]["fromHp"]<e["maxHp"]
            assert e["detail"]["recovery"]==fulls[e["entityId"]]
        if e["kind"] == "recovered_half_hp":
            recoveries[e["entityId"]]+=1
            assert 2*e["hp"]>=e["maxHp"] and e["detail"]["episode"]<=halves[e["entityId"]]
    for u in units.values():
        assert deaths[u["entityId"]] == (0 if u["deathFrame"] is None else 1)
        assert u["alive"] == (u["deathFrame"] is None)
        assert halves[u["entityId"]]==u["halfHpEpisodes"]
        assert fulls[u["entityId"]]==u["fullHpRecoveries"]
        assert recoveries[u["entityId"]]<=u["halfHpEpisodes"]
        half_events=[e for e in data["events"] if e["kind"]=="half_hp" and e["entityId"]==u["entityId"]]
        assert u["firstHalfFrame"]==(half_events[0]["frame"] if half_events else None)
    assert data["timeline"][0]["frame"] == 0 and data["timeline"][-1]["frame"] == end
    for flight in data["flights"]:
        assert 0<=flight["launchFrame"]<=flight["endFrame"]<=end
        assert flight["endX"] is not None and flight["enemyCoreRelativeX"] is not None
        if flight["landFrame"] is not None:
            assert flight["destinationSurface"] in ("ground","own_platform","enemy_platform")
        else:
            assert flight["endReason"] in ("removed_airborne","battle_ended_airborne")


def assert_detail(data):
    summary=data["detailSummary"]
    assert summary["complete"] and summary["statisticsComplete"] and summary["attributionComplete"],summary
    objects={x["entityId"]:x for x in data["objects"]}
    units={x["entityId"]:x for x in data["units"]}
    stats={x["entityId"]:x for x in data["unitStats"]}
    assert stats.keys()==units.keys()
    assert summary["eventPolicy"]=="shield-as-ability-v2"
    seq=[e["sequence"] for e in data["combatEvents"]]
    assert seq==list(range(len(seq)))
    for s in stats.values():
        assert s["initialHp"]-s["rawDamageTaken"]+s["healingReceived"]+s["hpAdjustment"]==s["finalHp"]==s["calculatedFinalHp"]
    for target,s in stats.items():
        full_recoveries=sum(e["kind"]=="heal" and e["restoredFullHp"] and e["entityId"]==target for e in data["combatEvents"])
        assert full_recoveries==units[target]["fullHpRecoveries"]
        sources=[r for r in data["damageSources"] if r["entityId"]==target]
        assert sum(r["effectiveAmount"] for r in sources)==s["effectiveDamageTaken"]
        if s["effectiveDamageTaken"]:
            assert abs(sum(r["percentage"] for r in sources)-100)<1e-8
        assert sum(r["rawAmount"] for r in sources)==s["rawDamageTaken"]
        roots=[r for r in data["damageRootSources"] if r["entityId"]==target]
        assert sum(r["effectiveAmount"] for r in roots)==s["effectiveDamageTaken"]
        assert s["groundLandings"]+s["ownPlatformLandings"]+s["enemyPlatformLandings"]==s["landingsInStep"]
    for e in data["combatEvents"]:
        if e["sourceObjectId"] is not None and objects[e["sourceObjectId"]]["kindCode"]==34:
            # Kabe shield creation is a defensive ability, not an attack action.
            if e["cause"]=="object_created":
                assert e["kind"]=="ability" and e["typeId"]==19
        if e["kind"]=="damage" and e["cause"]=="attack":
            assert e["sourceObjectId"] in e["overlappingObjectIds"]
            assert e["attackerEntityId"] in units
        if e["kind"]=="heal":
            assert e["hpAfter"]>e["hpBefore"] and e["attackerEntityId"] in units
        source=e["sourceObjectId"]
        if source is not None:
            assert source in objects
            current=source;seen=set()
            while objects[current]["parentEntityId"] is not None:
                assert current not in seen
                seen.add(current);current=objects[current]["parentEntityId"]
                assert current in objects
    for row in data["flightSamples"]:
        assert row["surface"]=="airborne" and row["entityId"] in units
    for s in stats.values():
        if s["typeId"]==19:
            assert s["attackActions"]==s["attackObjects"]==0


def main():
    if os.environ.get("CONDA_DEFAULT_ENV") != "opencode":
        raise SystemExit("请先conda activate opencode")
    p=argparse.ArgumentParser()
    p.add_argument("--out", required=True)
    p.add_argument("--limit", type=int, default=200)
    args=p.parse_args()
    output=Path(args.out)
    if output.exists():
        raise SystemExit("拒绝覆盖验证记录")
    start=time.monotonic()
    pool=ai.formations(ROOT/"tests/fixtures/standard200.txt")[:args.limit]
    candidate=ai.formations(ROOT/"docs/ai/examples/candidate.txt")
    reports=[]
    with tempfile.TemporaryDirectory(prefix="gkt-regression-") as tmp:
        tmp=Path(tmp)
        java=call_java({"action":"battle", "p1":candidate, "p2":pool, "maxFrames":65536},tmp,"jni200")
        kinds=Counter();surfaces=Counter();health_categories=Counter();detail_kinds=Counter();byte_count=event_count=snapshot_count=spawned=suppressed=repeat_half=detailed_bytes=cannon_explosion_hits=0
        sample_traces=[]
        for index, fort in enumerate(pool):
            req={"p1":candidate[0]["code"],"p2":fort["code"],"maxFrames":65536,"snapshotInterval":120}
            observed=observe(req,tmp,f"observed-{index}")
            plain=observe({**req,"plain":True},tmp,f"plain-{index}")
            assert observed["result"]==plain["result"], f"观察改变了结果：{index}"
            j=java["forward"][index]
            for key in ("status", "winnerHp", "frames"):
                assert observed["result"][key]==j[key], f"JNI与公开核心不同：{index} {key}"
            assert_trace(observed, [req["p1"],req["p2"]])
            detailed=observe({**req,"mode":"detailed"},tmp,f"detailed-{index}")
            assert detailed["result"]==observed["result"]
            for field in ("units","events","timeline","flights"):
                assert detailed[field]==observed[field],f"详细模式改变了基础观察：{index} {field}"
            try:
                assert_detail(detailed)
            except AssertionError:
                output.parent.mkdir(parents=True,exist_ok=True)
                failure=output.parent/f"{output.stem}-failed-{index}-{time.time_ns()}.json"
                failure.write_text(ai.dumps(detailed),encoding="utf-8")
                print(f"详细验收失败：对手索引{index}，现场保存在{failure}",flush=True)
                raise
            detail_kinds.update(e["kind"] for e in detailed["combatEvents"])
            detailed_bytes+=len(ai.dumps(detailed).encode())
            objects={x["entityId"]:x for x in detailed["objects"]}
            for e in detailed["combatEvents"]:
                if e["kind"]=="damage" and e["sourceObjectId"] is not None:
                    obj=objects[e["sourceObjectId"]];owner=objects.get(e["attackerEntityId"])
                    if obj["kindCode"] in (45,46) and owner and owner["typeId"]==9:cannon_explosion_hits+=1
            kinds.update(e["kind"] for e in observed["events"])
            surfaces.update(f["destinationSurface"] or f["endReason"] for f in observed["flights"])
            health_categories.update(f'{u["category"]}:{e["kind"]}' for u in observed["units"] for e in observed["events"]
                                     if u["entityId"]==e.get("entityId") and e["kind"] in ("half_hp","recovered_full_hp","death"))
            suppressed+=sum(u["suppressedHalfHpEpisodes"] for u in observed["units"])
            repeat_half+=sum(u["halfHpEpisodes"]>1 for u in observed["units"])
            event_count += len(observed["events"])
            snapshot_count += len(observed["timeline"])
            spawned += sum(u["spawned"] for u in observed["units"])
            byte_count += len(ai.dumps(observed).encode())
            reports.append({"opponentIndex":index,"opponentCodeSha256":ai.digest(fort["code"].encode()),"result":observed["result"],
                            "observerEvidenceSha256":ai.digest(ai.dumps(observed).encode())})
            if index<2 or fort["code"][0]!="0":
                sample_traces.append(detailed)
            if (index+1)%25==0:
                print(f"已核验{index+1}/{len(pool)}局：原Rust、基础模式、详细模式、JNI结果一致",flush=True)
        # 显式seed、零帧、超时，以及全部非核心类型的构造/槽位映射。
        from test_ai import code
        all_types=code(0,138,138,core=True)+"".join(code(i,(i%10)*33,(i//10)*50) for i in range(1,61))
        ai.validate_code(all_types)
        extra=[]
        for frames,seed in ((0,None),(3,12345),(2048,12345)):
            req={"p1":all_types,"p2":all_types,"maxFrames":frames,"seed":seed,"snapshotInterval":120}
            observed=observe(req,tmp,f"extra-{frames}")
            plain=observe({**req,"plain":True},tmp,f"extra-plain-{frames}")
            assert observed["result"]==plain["result"]
            assert_trace(observed,[all_types,all_types])
            detailed=observe({**req,"mode":"detailed"},tmp,f"extra-detailed-{frames}")
            assert detailed["result"]==observed["result"]
            assert_detail(detailed)
            if frames==0:
                for u in observed["units"]:
                    if u["typeId"] in (30,60):
                        expected={30:100,60:75}[u["typeId"]]
                        assert u["initialHp"]==u["maxHp"]==expected
            if frames==3:
                assert any(s["coreX"]!=s["coreTargetX"] or s["coreY"]!=s["coreTargetY"] for s in observed["timeline"][-1]["sides"])
            extra.append({"maxFrames":frames,"seed":seed,"result":observed["result"]})
        for core_type in (1,2):
            boss_code=str(core_type)+all_types[1:6]
            opponent=code(0,138,138,core=True)
            req={"p1":boss_code,"p2":opponent,"maxFrames":1024,"seed":12345}
            basic=observe(req,tmp,f"boss-{core_type}")
            plain=observe({**req,"plain":True},tmp,f"boss-plain-{core_type}")
            detail=observe({**req,"mode":"detailed"},tmp,f"boss-detail-{core_type}")
            assert basic["result"]==plain["result"]==detail["result"]
            assert_detail(detail)
            assert any(e["kind"]=="attack" and e["slot"]==0 and e["side"]==0 for e in detail["combatEvents"])
            extra.append({"coreType":core_type,"maxFrames":1024,"seed":12345,"result":detail["result"]})
        disabled=observe({"p1":all_types,"p2":all_types,"maxFrames":2048,"recordHalfHp":False},tmp,"half-disabled")
        assert all(e["kind"] not in ("half_hp","quarter_hp","recovered_half_hp") for e in disabled["events"])
        limited=observe({"p1":candidate[0]["code"],"p2":pool[0]["code"],"mode":"detailed","maxDetailedEvents":100},tmp,"limited")
        assert limited["detailSummary"]["droppedEvents"]>0 and not limited["summary"]["complete"]
        assert limited["detailSummary"]["statisticsComplete"] and limited["detailSummary"]["attributionComplete"]
        assert len(limited["combatEvents"])==100
        boss=next(x for x in pool if x["code"][0]!="0")
        contrib=call_java({"action":"contribution","p1":[boss],"p2":pool[:1],"maxFrames":8,"maxJobs":1000},tmp,"boss-contribution")
        for combo in contrib["report"]["combos"]:
            ai.validate_code(combo["code"])
            assert combo["code"][:6]==boss["code"][:6]
        bf=call_java({"action":"battlefield","p1":candidate,"p2":pool[:6],"maxFrames":2048},tmp,"battlefield")
        assert sorted(x["slot"] for x in bf["report"]["slots"])==list(range(len(candidate[0]["code"])//6))
        mu=call_java({"action":"matchup","p1":candidate,"p2":pool[:6],"maxFrames":8},tmp,"matchup")
        assert mu["report"]["score"]["scorePercent"]==50 and mu["report"]["score"]["U"]==6
        result={"date":time.strftime("%Y-%m-%d"),"ruleset":"catalog-hp-v1","status":"passed","jniComparison":"passed","fixtureSha256":ai.file_hash(ROOT/"tests/fixtures/standard200.txt"),
            "javaJarSha256":ai.file_hash(ROOT/"target/gekitotsu_lab-0.6.7.7.jar"),
            "bundledDllSha256":java["bundledDllSha256"],"observerSha256":ai.file_hash(ROOT/"rust/target/release/gkt-observe.exe"),
            "detailedObserverSha256":ai.file_hash(ROOT/"rust/target/release/gkt-detail.exe"),
            "candidateCodeSha256":ai.digest(candidate[0]["code"].encode()),"checkedMatches":len(pool),
            "checks":["observer/plain完整结果一致（含双方HP和RNG）","JNI对照status/winnerHp/frames一致","初始槽位和动态实体身份",
                "恢复后重复半血、恢复满血、实际maxHp、短暂恢复过滤及每实体唯一死亡","观察不保存逐次伤害事件","全60非核心类型初始化、显式seed/零帧","蓝加速器HP/maxHp=100、电梯壁HP/maxHp=75",
                "BOSS单删保留纯码","战场槽位映射","帧上限未决计平局"],
            "detailedChecks":["200局详细模式与基础/原Rust结果及基础记录一致","所有单位/墙/核心HP账本平衡","伤害来源有效HP损失占比合计100%",
                "攻击/治疗源对应具体单位、对象来源链无环且完整","炮玉子弹爆炸追溯","仅采样空中位置"],
            "detailedEventsByKind":dict(detail_kinds),"detailedRawBytes":detailed_bytes,"cannonExplosionDamageEvents":cannon_explosion_hits,
            "eventsByKind":dict(kinds),"totalEvents":event_count,"totalSnapshots":snapshot_count,"spawnedEntities":spawned,
            "flightDestinations":dict(surfaces),
            "healthEventPolicy":"recovery-episodes-v1","healthRecoveryWindowFrames":30,
            "healthEventsByCategory":dict(health_categories),"suppressedHalfEpisodes":suppressed,"entitiesWithRepeatedHalfEpisodes":repeat_half,
            "rawTelemetryBytes":byte_count,"seconds":round(time.monotonic()-start,3),"matches":reports,"extraCases":extra,
            "limits":["这不是全引擎/原Flash等价证明；只覆盖冻结输入和列出的字段","未公开的Rust金样未参与本验收"]}
        output.parent.mkdir(parents=True,exist_ok=True)
        output.write_text(ai.dumps(result),encoding="utf-8")
        evidence=output.parent/(output.stem+"-samples")
        evidence.mkdir(exist_ok=False)
        for i,trace in enumerate(sample_traces):
            (evidence/f"trace-{i}.json").write_text(ai.dumps(trace),encoding="utf-8")
        print(ai.dumps({k:v for k,v in result.items() if k not in ("matches",)}),flush=True)


if __name__=="__main__":
    main()
