#!/usr/bin/env python3
"""只读战斗观察、不可覆盖证据与有界查询。仅依赖Python标准库。"""
from __future__ import annotations
import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import zipfile
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[2]

def default_jar():
    """Prefer a source build; portable releases keep the same JAR under app/."""
    name = "gekitotsu_lab-0.6.7.7.jar"
    built = ROOT / "target" / name
    return built if built.is_file() else ROOT / "app" / name

DEFAULT_JAR = default_jar()
POLICY = "timeout-as-draw-v1"
ALPHABET = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
SCHEMA = "gkt-store-v1"


def dumps(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_hash(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as f:
        for chunk in iter(lambda: f.read(1048576), b""):
            h.update(chunk)
    return h.hexdigest()


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8-sig"),
                      parse_constant=lambda x: (_ for _ in ()).throw(ValueError(f"非法JSON数值：{x}")))


def validate_code(code):
    if len(code) < 6 or len(code) % 6 or len(code) > 3072 or code[0] not in "012":
        raise ValueError("阵码必须为6的倍数，核心在slot0，最多512槽位")
    for slot in range(len(code)//6):
        chunk = code[slot*6:slot*6+6]
        if slot and chunk[0] not in ALPHABET[1:61]:
            raise ValueError(f"slot={slot} 非核心ID不合法")
        if any(c not in ALPHABET[:61] for c in chunk[1:]):
            raise ValueError("坐标必须base61，不能使用Z或其他字符")
        number = 0
        for c in chunk[1:]:
            number = number*61 + ALPHABET.index(c)
        x = number//1000 % 1000 - (52 if slot == 0 else 16)
        y = number % 1000 - (58 if slot == 0 else 20)
        if not (0 <= x <= (276 if slot == 0 else 348) and
                0 <= y <= (276 if slot == 0 else 349) and number//1000000 < 360):
            raise ValueError(f"slot={slot} 坐标或角度越界")
    return code


def formations(path):
    text = Path(path).read_text(encoding="utf-8-sig")
    forts = []
    for part in text.strip().split("/"):
        part = part.strip()
        if not part:
            raise ValueError("阵集不能包含空条目或尾部分隔符")
        name, code = part.split("&", 1) if "&" in part else ("", part)
        if any(c in name for c in "&/#\r\n") or any(c.isspace() for c in code):
            raise ValueError("名称含分隔符或代码含空白；多阵必须用/分隔")
        forts.append({"name": name, "code": validate_code(code)})
    return forts


def connect(path):
    path = Path(path).resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    db = sqlite3.connect(path, timeout=10)
    db.row_factory = sqlite3.Row
    db.execute("PRAGMA foreign_keys=ON")
    db.executescript("""
      CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS artifacts(
        id TEXT PRIMARY KEY, action TEXT NOT NULL, request TEXT NOT NULL,
        fingerprint TEXT NOT NULL, summary TEXT NOT NULL, raw_path TEXT NOT NULL,
        raw_sha256 TEXT NOT NULL, created TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS records(
        artifact TEXT NOT NULL REFERENCES artifacts(id), table_name TEXT NOT NULL,
        row_index INTEGER NOT NULL, side INTEGER, slot INTEGER, kind TEXT, frame INTEGER,
        data TEXT NOT NULL, PRIMARY KEY(artifact,table_name,row_index));
      CREATE INDEX IF NOT EXISTS records_lookup ON records(artifact,table_name,side,slot,kind,frame);
      CREATE INDEX IF NOT EXISTS records_entity ON records(artifact,table_name,json_extract(data,'$.entityId'),frame);
      CREATE INDEX IF NOT EXISTS records_attacker ON records(artifact,table_name,json_extract(data,'$.attackerEntityId'),frame);
      CREATE INDEX IF NOT EXISTS records_source ON records(artifact,table_name,json_extract(data,'$.sourceObjectId'),frame);
    """)
    row = db.execute("SELECT value FROM meta WHERE key='schema'").fetchone()
    if row and row[0] != SCHEMA:
        raise ValueError("不支持的数据库版本")
    db.execute("INSERT OR IGNORE INTO meta VALUES('schema',?)", (SCHEMA,))
    db.commit()
    return db


def normalized(data):
    tables = {}
    def add(name, rows):
        if isinstance(rows, list):
            tables.setdefault(name, []).extend(x if isinstance(x, dict) else {"value": x} for x in rows)
    for name in ("units", "events", "timeline", "flights", "flightSamples", "combatEvents", "objects", "unitStats", "damageSources", "damageRootSources", "healingSources", "forward", "reverse", "scores"):
        add(name, data.get(name))
    for index, formation in enumerate(data.get("formations", [])):
        add("formations", [{k:v for k,v in formation.items() if k not in ("units", "assaultGroups", "issues", "code")}])
        for name in ("units", "assaultGroups", "issues"):
            add(name, [{**x, "formationIndex": index} for x in formation.get(name, [])])
    report = data.get("report", {})
    for name, values in report.items():
        if isinstance(values, list) and name not in ("rawGuiReport",):
            add(name, values)
    if isinstance(report.get("guiReport"), dict):
        gui = report["guiReport"]
        if isinstance(gui.get("strength"), dict):
            add("strengthEntries", gui["strength"].get("entries"))
            add("strengthSummary", [{k:v for k,v in gui["strength"].items() if k != "entries"}])
        add("opponents", gui.get("evalForts"))
    prediction = data.get("prediction", {})
    add("trajectory", prediction.get("points"))
    return tables


def compact_summary(data, tables):
    summary = {k: data[k] for k in ("schemaVersion", "action", "engine", "engineVersion",
                "coordinateSpace", "sideConvention", "observation", "result", "summary",
                "rustAvailable", "rustReason", "capabilities", "maxFrames", "seedPolicy",
                "scoringPolicy", "limits", "bundledDllSha256", "mode", "detailSummary", "healthEventPolicy") if k in data}
    summary["tables"] = {k:len(v) for k,v in tables.items()}
    if "p1" in data and isinstance(data["p1"], list):
        summary["inputs"] = {k:{"count":len(data.get(k, [])), "sha256":digest(dumps(data.get(k, [])).encode())} for k in ("p1", "p2")}
    report = data.get("report", {})
    summary["report"] = {k:v for k,v in report.items() if not isinstance(v, list) and not isinstance(v, dict)}
    for k in ("score", "baseline"):
        if k in report:
            summary["report"][k] = report[k]
    if "prediction" in data:
        summary["prediction"] = {k:v for k,v in data["prediction"].items() if k != "points"}
    if not summary["report"]:
        summary.pop("report")
    return summary


def verified(db, artifact):
    row = db.execute("SELECT * FROM artifacts WHERE id=?", (artifact,)).fetchone()
    if row is None:
        raise ValueError("未找到artifact；使用list查看已有存证")
    path = Path(row["raw_path"])
    if not path.is_file() or file_hash(path) != row["raw_sha256"]:
        raise ValueError("原始证据不存在或SHA256不符，拒绝复用/查询")
    identity = dumps({"request":json.loads(row["request"]), "fingerprint":json.loads(row["fingerprint"])})
    if digest(identity.encode()) != artifact:
        raise ValueError("冻结请求/引擎指纹与artifact不符")
    return row


def java_engine(jar):
    """核验研究入口并读取同一JAR内的JNI，不执行GUI或对战。"""
    binary = Path(jar).resolve()
    if not binary.is_file():
        raise ValueError(f"缺少研究JAR：{binary}；先运行tools/ai/build.ps1")
    before = file_hash(binary)
    try:
        with zipfile.ZipFile(binary) as archive:
            manifest = archive.read("META-INF/MANIFEST.MF").decode("utf-8").replace("\r\n", "\n").replace("\n ", "")
            attributes = dict(line.split(": ", 1) for line in manifest.splitlines() if ": " in line)
            main_class = archive.read(attributes["Main-Class"].replace(".", "/")+".class")
            archive.getinfo("org/example/ai/AiApi.class")
            if b"\x01\x00\x04--ai" not in main_class or b"org/example/ai/AiApi" not in main_class:
                raise ValueError("主类没有研究--ai入口")
            dll = archive.read("native/gkt_jni.dll")
    except (KeyError, ValueError, zipfile.BadZipFile, UnicodeError) as error:
        raise ValueError(f"JAR不支持研究--ai接口或缺少JNI：{binary}；请使用构建后的{DEFAULT_JAR.name}") from error
    if file_hash(binary)!=before:
        raise ValueError("核验期间JAR发生改变，拒绝使用不稳定构建")
    fingerprint = {"backend": "java", "binarySha256": before,
                   "adapterSha256": file_hash(__file__), "policy": POLICY,
                   "dllSha256": digest(dll)}
    identity_file = binary.parent/"identity.json"
    if identity_file.is_file():
        frozen = read_json(identity_file)
        if frozen.get("schema")=="gkt-experiment-inputs-v1":
            expected = frozen.get("engine", {}).get("fingerprint", {})
            if any(expected.get(key)!=fingerprint[key] for key in ("binarySha256", "dllSha256")):
                raise ValueError("JAR/DLL与建档冻结身份不同，拒绝运行；请另建实验目录冻结所需构建")
    return binary, dll, fingerprint


def compute(db, store, request, args, backend, expected_result=None, expected_fingerprint=None):
    dll = None
    if backend in ("observer","detailed"):
        name="gkt-detail" if backend=="detailed" else "gkt-observe"
        configured=args.detailed_observer if backend=="detailed" else args.observer
        binary = Path(configured or ROOT / "rust/target/release" / (name+".exe" if os.name == "nt" else name)).resolve()
    else:
        binary, dll, fingerprint = java_engine(args.jar or default_jar())
    if not binary.is_file():
        raise ValueError(f"缺少构建产物：{binary}；先运行tools/ai/build.ps1")
    if backend in ("observer", "detailed"):
        fingerprint = {"backend": backend, "binarySha256": file_hash(binary),
                       "adapterSha256": file_hash(__file__), "policy": POLICY}
    if backend=="detailed":
        base=Path(args.observer or ROOT/"rust/target/release"/("gkt-observe.exe" if os.name=="nt" else "gkt-observe"))
        fingerprint["baseObserverSha256"]=file_hash(base)
        fingerprint["coreHookGeneratorSha256"]=file_hash(ROOT/"tools/ai/prepare_core.py")
    if expected_fingerprint is not None and fingerprint!=expected_fingerprint:
        raise ValueError("运行引擎与冻结指纹不同，拒绝启动或复用对局")
    identity = dumps({"request": request, "fingerprint": fingerprint})
    artifact = digest(identity.encode())
    old = db.execute("SELECT id FROM artifacts WHERE id=?", (artifact,)).fetchone()
    if old:
        row = verified(db, artifact)
        if expected_result is not None and json.loads(row["summary"]).get("result")!=expected_result:
            raise ValueError("详细重放结果与基础存证不同，拒绝作为同场对局研究")
        return {"artifact": artifact, "cached": True, "summary": json.loads(row["summary"])}
    raw_dir = Path(store).resolve().parent / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="gkt-ai-") as tmp:
        tmp = Path(tmp)
        inp, out = tmp / "request.json", tmp / "output.json"
        inp.write_text(dumps(request), encoding="utf-8")
        if backend in ("observer","detailed"):
            command = [str(binary), str(inp), str(out)]
        else:
            java = args.java or shutil.which("java")
            if not java:
                raise ValueError("未找到Java 21+")
            # 显式固定加载JAR携带的DLL，避免本机工作目录native库影响证据。
            dll_path = tmp / "gkt_jni.dll"
            dll_path.write_bytes(dll)
            command = [java, "-Djava.awt.headless=true", f"-Dgekitotsu.native.path={dll_path}",
                       "-jar", str(binary), "--ai", str(inp), str(out)]
        proc = subprocess.run(command, cwd=tmp, capture_output=True, timeout=args.timeout)
        if proc.returncode:
            raise ValueError(f"执行失败：{binary.name}，退出码={proc.returncode}（0x{proc.returncode & 0xffffffff:08x}）：" + proc.stderr.decode("utf-8", errors="replace")[-3000:])
        if not out.is_file():
            raise ValueError("执行结束却没有输出；不会将失败当平局")
        data = read_json(out)
        if expected_result is not None and data.get("result")!=expected_result:
            raise ValueError("详细重放改变了胜负/HP/帧数/RNG，拒绝发布存证")
        if backend=="detailed" and not data.get("detailSummary",{}).get("statisticsComplete"):
            raise ValueError("详细模式HP账本不平衡，拒绝发布统计")
        tables = normalized(data)
        summary = compact_summary(data, tables)
        summary["provenance"] = fingerprint
        summary["requestSha256"] = digest(dumps(request).encode())
        raw = raw_dir / f"{artifact}.json"
        # 外部程序失败的半成品始终留在临时目录，成功后以不可覆盖方式发布。
        with raw.open("xb") as f:
            f.write(out.read_bytes())
    try:
        with db:
            db.execute("INSERT INTO artifacts VALUES(?,?,?,?,?,?,?,?)", (artifact, request.get("action", "battle"),
                       dumps(request), dumps(fingerprint), dumps(summary), str(raw), file_hash(raw), datetime.now(timezone.utc).isoformat()))
            for table, rows in tables.items():
                for index, record in enumerate(rows):
                    db.execute("INSERT INTO records VALUES(?,?,?,?,?,?,?,?)", (artifact, table, index,
                               record.get("side"), record.get("slot"), record.get("kind"), record.get("frame",record.get("launchFrame")), dumps(record)))
    except Exception:
        raw.unlink()
        raise
    return {"artifact": artifact, "cached": False, "summary": summary}


def probe_java_engine(jar, java=None, timeout=30):
    """用真实--ai请求核验JNI，临时存证不留下评测现场，不运行对战。"""
    if not 0 < timeout < float("inf"):
        raise ValueError("引擎探测超时须为有限正数")
    binary, _, expected = java_engine(jar)
    args = argparse.Namespace(jar=str(binary), java=java, timeout=timeout)
    with tempfile.TemporaryDirectory(prefix="gkt-engine-probe-") as folder:
        store = Path(folder)/"store.sqlite"
        with closing(connect(store)) as db:
            result = compute(db, store, {"action": "probe"}, args, "java", expected_fingerprint=expected)
            row = verified(db, result["artifact"])
            data = read_json(row["raw_path"])
    if data.get("schemaVersion")!="gkt-ai-analysis-v1" or data.get("action")!="probe":
        raise ValueError("JAR未返回研究--ai探测响应")
    if data.get("rustAvailable") is not True:
        raise ValueError("JAR的JNI不可用："+str(data.get("rustReason", "未知原因")))
    return {"fingerprint": expected, "result": data}


DEFAULT_FIELDS = {
    "combatEvents":"sequence,frame,kind,side,slot,entityId,sourceObjectId,attackerEntityId,attackerRootEntityId,cause,amount,effectiveHpLoss,hpBefore,hpAfter,restoredFullHp,overlappingObjectIds,detail",
    "objects":"entityId,kindCode,side,slot,typeId,parentEntityId,ownerUnitId,originalOwnerUnitId,rootUnitId,birthFrame,deathFrame",
    "damageSources":"entityId,side,slot,typeId,attackerEntityId,attacker,cause,effectiveAmount,rawAmount,percentage,events,ambiguousHitEvents",
    "damageRootSources":"entityId,side,slot,typeId,attackerRootEntityId,attacker,cause,effectiveAmount,rawAmount,percentage,events,ambiguousHitEvents",
    "healingSources":"entityId,side,slot,typeId,attackerEntityId,attacker,cause,effectiveAmount,percentage,events",
    "unitStats":"entityId,side,slot,typeId,rawDamageTaken,effectiveDamageTaken,healingReceived,attackActions,attackObjects,rawDamageDealt,effectiveDamageDealt,healingGiven,firstDamageFrame,lastDamageFrame,finalHp,launchesInStep,landingsInStep,groundLandings,enemyPlatformLandings,ownPlatformLandings",
    "events": "frame,kind,side,slot,entityId,typeId,hp,x,y,detail",
    "units": "entityId,side,slot,typeId,id,name,category,initialHp,catalogHp,maxHp,firstHalfFrame,halfHpEpisodes,fullHpRecoveries,suppressedHalfHpEpisodes,deathFrame,firstLaunchFrame,firstBoardFrame,alive,finalHp,x,y,r,cost,cd,at,shoot",
    "combos": "slots,removedCost,score,deltaPoints",
    "formations": "name,coordinateSpace,cost,accelLevel,overDefaultBudget",
    "flights": "entityId,side,slot,typeId,launchFrame,landFrame,endFrame,endReason,originSurface,destinationSurface,endX,endY,enemyCoreRelativeX,enemyCoreRelativeY",
}


def query(db, args):
    row = verified(db, args.artifact)
    if args.table == "summary":
        return {"artifact": args.artifact, "summary": json.loads(row["summary"])}
    fields = args.fields or DEFAULT_FIELDS.get(args.table)
    fields = fields.split(",") if fields else None
    clauses, values = ["artifact=?", "table_name=?"], [args.artifact, args.table]
    for attr in ("side", "slot", "kind"):
        value = getattr(args, attr)
        if value is not None:
            clauses.append(f"{attr}=?")
            values.append(value)
    for attr, key in (("entity_id", "entityId"), ("formation_index", "formationIndex"), ("attacker_entity_id","attackerEntityId"), ("source_object_id","sourceObjectId"), ("attacker_root_entity_id","attackerRootEntityId")):
        value = getattr(args, attr)
        if value is not None:
            clauses.append(f"json_extract(data,'$.{key}')=?")
            values.append(value)
    if args.surface:
        clauses.append("(json_extract(data,'$.destinationSurface')=? OR json_extract(data,'$.detail.surface')=? OR json_extract(data,'$.surface')=?)")
        values.extend([args.surface,args.surface,args.surface])
    time_expr = "frame" if args.time_field=="frame" else f"json_extract(data,'$.{args.time_field}')"
    if args.from_frame is not None:
        clauses.append(time_expr+">=?")
        values.append(args.from_frame)
    if args.to_frame is not None:
        clauses.append(time_expr+"<=?")
        values.append(args.to_frame)
    sql = " AND ".join(clauses)
    total = db.execute("SELECT COUNT(*) FROM records WHERE " + sql, values).fetchone()[0]
    rows = db.execute("SELECT data FROM records WHERE " + sql + " ORDER BY row_index LIMIT ? OFFSET ?",
                      values + [args.limit, args.offset]).fetchall()
    out = {"artifact": args.artifact, "table": args.table, "total": total,
           "offset": args.offset, "rows": [], "nextOffset": None, "limitedByBytes": False}
    for r in rows:
        data = json.loads(r[0])
        if fields:
            data = {k:data[k] for k in fields if k in data}
        out["rows"].append(data)
        if len(dumps(out).encode()) > args.max_bytes - 200:
            out["rows"].pop()
            out["limitedByBytes"] = True
            break
    following = args.offset + len(out["rows"])
    if following < total:
        out["nextOffset"] = following
        if not out["rows"]:
            out["hint"] = "单行超过字节预算，请使用--fields减少字段或增大--max-bytes"
    return out


def compare(db, args):
    left, right = verified(db, args.baseline), verified(db, args.candidate)
    fl, fr = json.loads(left["fingerprint"]), json.loads(right["fingerprint"])
    if fl != fr:
        raise ValueError("引擎/适配器/计分口径指纹不同，不能直接比较")
    req_l, req_r = json.loads(left["request"]), json.loads(right["request"])
    for key in ("maxFrames", "seed", "snapshotInterval", "healthRecoveryWindowFrames", "recordHalfHp", "mode", "flightSampleInterval", "maxDetailedEvents", "p2" if args.side==0 else "p1", "plain"):
        if req_l.get(key) != req_r.get(key):
            raise ValueError(f"冻结输入不同：{key}")
    def units(artifact):
        rows = db.execute("SELECT data FROM records WHERE artifact=? AND table_name='units' AND side=? AND slot IS NOT NULL ORDER BY slot",
                          (artifact, args.side)).fetchall()
        return {r["slot"]: r for r in (json.loads(x[0]) for x in rows)}
    a, b = units(args.baseline), units(args.candidate)
    rows = []
    for slot in sorted(a.keys() & b.keys()):
        if a[slot].get("typeId") != b[slot].get("typeId"):
            continue
        changes = {key:{"baseline":a[slot].get(key), "candidate":b[slot].get(key)}
                   for key in ("firstHalfFrame", "halfHpEpisodes", "fullHpRecoveries", "suppressedHalfHpEpisodes", "deathFrame", "firstLaunchFrame", "firstBoardFrame", "finalHp")
                   if a[slot].get(key) != b[slot].get(key)}
        if changes:
            rows.append({"slot":slot, "typeId":a[slot].get("typeId"), "changes":changes})
    return {"baseline":args.baseline, "candidate":args.candidate,
            "baselineResult":json.loads(left["summary"]).get("result"), "candidateResult":json.loads(right["summary"]).get("result"),
            "changedUnits":rows[:args.limit], "totalChangedUnits":len(rows),
            "limit":"按原槽位比较；调序/增删后必须人工确认对应关系；这不是因果归因"}


def cohort(db, args):
    manifest = read_json(args.manifest)
    artifacts = [x["artifact"] for x in manifest["battles"]]
    if len(set(artifacts)) != len(artifacts) or not 1 <= len(artifacts) <= 64:
        raise ValueError("群组必须1..64场不同存证")
    reference = None
    groups = {}
    complete = True
    for artifact in artifacts:
        row = verified(db, artifact)
        req, fp = json.loads(row["request"]), json.loads(row["fingerprint"])
        contract = {"fingerprint":fp, "candidate":req["p1" if args.side==0 else "p2"],
                    "maxFrames":req.get("maxFrames"), "seed":req.get("seed"), "plain":req.get("plain"),
                    "healthRecoveryWindowFrames":req.get("healthRecoveryWindowFrames",30),"recordHalfHp":req.get("recordHalfHp",True)}
        if contract["plain"]:
            raise ValueError("无观察对照不能参与遥测汇总")
        if reference is None:
            reference = contract
        elif contract != reference:
            raise ValueError("群组的候选/引擎/帧数/seed政策不同")
        complete = complete and json.loads(row["summary"])["summary"]["complete"]
        for r in db.execute("SELECT data FROM records WHERE artifact=? AND table_name='units' AND side=? AND slot IS NOT NULL", (artifact,args.side)):
            u = json.loads(r[0])
            group = groups.setdefault(u["slot"], {"slot":u["slot"], "typeId":u["typeId"], "samples":0, "deaths":0,
                       "halfHpReached":0, "halfHpEpisodes":0,"fullHpRecoveries":0,"suppressedHalfHpEpisodes":0,
                       "launches":0, "boardEnemy":0, "deathFrames":[], "halfFrames":[], "landingSurfaces":{},"landingPositions":{}})
            group["samples"] += 1
            group["launches"] += u["launches"]
            for field in ("halfHpEpisodes","fullHpRecoveries","suppressedHalfHpEpisodes"):
                group[field] += u.get(field,0)
            group["boardEnemy"] += u["firstBoardFrame"] is not None
            if u["deathFrame"] is not None:
                group["deaths"] += 1
                group["deathFrames"].append(u["deathFrame"])
            if u["firstHalfFrame"] is not None:
                group["halfHpReached"] += 1
                group["halfFrames"].append(u["firstHalfFrame"])
        for r in db.execute("SELECT data FROM records WHERE artifact=? AND table_name='flights' AND side=? AND slot IS NOT NULL", (artifact,args.side)):
            flight=json.loads(r[0]);group=groups[flight["slot"]]
            surface=flight["destinationSurface"] or flight["endReason"]
            group["landingSurfaces"][surface]=group["landingSurfaces"].get(surface,0)+1
            if flight["landFrame"] is not None:
                group["landingPositions"].setdefault(surface,[]).append((flight["endX"],flight["endY"],flight["enemyCoreRelativeX"],flight["enemyCoreRelativeY"]))
    rows = []
    for slot in sorted(groups):
        g = groups[slot]
        deaths, halves = g.pop("deathFrames"), g.pop("halfFrames")
        g["deathRate"] = g["deaths"]/g["samples"]
        g["meanDeathFrameAmongDeaths"] = sum(deaths)/len(deaths) if deaths else None
        g["meanHalfFrameAmongReached"] = sum(halves)/len(halves) if halves else None
        positions=g.pop("landingPositions")
        g["landingPositionStats"]={}
        for surface, samples in positions.items():
            coordinates=list(zip(*samples))
            g["landingPositionStats"][surface]={"count":len(samples),**{key:{"min":min(xs),"mean":sum(xs)/len(xs),"max":max(xs)} for key,xs in zip(("worldX","worldY","enemyCoreRelativeX","enemyCoreRelativeY"),coordinates)}}
        rows.append(g)
    out={"battles":len(artifacts), "complete":complete, "totalSlots":len(rows), "offset":args.offset,"rows":[],"nextOffset":None,
         "limitedByBytes":False,"interpretation":"落地地面/己方平台/敌方平台分别统计，不预设登阵目标。均值仅针对达到事件者；未结束空中轨迹/未死亡者为删失，子集不等于完整200胜率"}
    for row in rows[args.offset:args.offset+args.limit]:
        out["rows"].append(row)
        if len(dumps(out).encode())>args.max_bytes-200:
            out["rows"].pop();out["limitedByBytes"]=True;break
    following=args.offset+len(out["rows"])
    if following<len(rows):out["nextOffset"]=following
    return out


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="激突AI观察与有界查询")
    parser.add_argument("--store", default=str(ROOT / "ai-data/store.sqlite"))
    parser.add_argument("--jar")
    parser.add_argument("--observer")
    parser.add_argument("--detailed-observer")
    parser.add_argument("--java")
    parser.add_argument("--timeout", type=int, default=180)
    parser.add_argument("--max-bytes", type=int, default=12000)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("capabilities")
    sub.add_parser("probe")
    sub.add_parser("catalog")
    run = sub.add_parser("run", help="执行Java或Rust请求JSON并存证")
    run.add_argument("--request", required=True)
    f = sub.add_parser("formation")
    inp = f.add_mutually_exclusive_group(required=True)
    inp.add_argument("--code")
    inp.add_argument("--file")
    for cmd in ("battle", "diagnose", "evaluate", "analyze"):
        p = sub.add_parser(cmd)
        p.add_argument("--p1", required=True)
        p.add_argument("--p2", required=True)
        p.add_argument("--max-frames", type=int, default=65536)
        if cmd in ("battle", "diagnose"):
            p.add_argument("--seed", type=int)
            p.add_argument("--snapshot-interval", type=int, default=120)
            p.add_argument("--health-recovery-window-frames", type=int, default=30, help="半血后快速恢复的过滤窗口；30帧对应原SWF声明的1秒，0关闭过滤")
            p.add_argument("--no-half-hp", action="store_true", help="关闭半血/四分之一血/恢复半血记录；保留满血恢复和死亡")
            p.add_argument("--detailed", action="store_true", help="按需开启完整HP写点、攻击与来源链；日常批量筛阵不使用")
            p.add_argument("--flight-sample-interval", type=int, default=6)
            p.add_argument("--max-detailed-events", type=int, default=500000)
            p.add_argument("--plain", action="store_true", help="无观察对照")
            if cmd == "diagnose":
                p.add_argument("--max-battles", type=int, default=8)
        else:
            p.add_argument("--threads", type=int, default=4)
            p.add_argument("--max-jobs", type=int, default=12800)
            if cmd == "evaluate":
                p.add_argument("--both-sides", action="store_true")
            else:
                p.add_argument("--kind", choices=("contribution", "matchup", "battlefield"), required=True)
                p.add_argument("--delete-count", type=int, default=1)
                p.add_argument("--strength", action="store_true")
                p.add_argument("--filter", choices=("ALL", "WIN", "LOSE", "DRAW", "TIMEOUT"), default="ALL")
                p.add_argument("--bin-frames", type=int, default=120)
    q = sub.add_parser("query")
    q.add_argument("--artifact", required=True)
    q.add_argument("--table", default="summary")
    q.add_argument("--fields")
    q.add_argument("--side", type=int, choices=(0, 1))
    q.add_argument("--slot", type=int)
    q.add_argument("--entity-id", type=int)
    q.add_argument("--attacker-entity-id", type=int)
    q.add_argument("--attacker-root-entity-id", type=int)
    q.add_argument("--source-object-id", type=int)
    q.add_argument("--formation-index", type=int)
    q.add_argument("--surface", choices=("ground","own_platform","enemy_platform","airborne"))
    q.add_argument("--time-field", choices=("frame","launchFrame","landFrame","endFrame"), default="frame")
    q.add_argument("--kind")
    q.add_argument("--from-frame", type=int)
    q.add_argument("--to-frame", type=int)
    q.add_argument("--offset", type=int, default=0)
    q.add_argument("--limit", type=int, default=20)
    l = sub.add_parser("list")
    l.add_argument("--limit", type=int, default=20)
    c = sub.add_parser("compare")
    c.add_argument("--baseline", required=True)
    c.add_argument("--candidate", required=True)
    c.add_argument("--side", type=int, choices=(0, 1), default=0)
    c.add_argument("--limit", type=int, default=20)
    v = sub.add_parser("verify")
    v.add_argument("--artifact", required=True)
    replay=sub.add_parser("replay",help="从基础artifact冻结输入开启详细重放，并核对完整结果")
    replay.add_argument("--artifact",required=True)
    replay.add_argument("--flight-sample-interval",type=int,default=6)
    replay.add_argument("--max-detailed-events",type=int,default=500000)
    co = sub.add_parser("cohort")
    co.add_argument("--manifest", required=True)
    co.add_argument("--side", type=int, choices=(0,1), default=0)
    co.add_argument("--offset", type=int, default=0)
    co.add_argument("--limit", type=int, default=20)
    args = parser.parse_args(argv)
    if not 512 <= args.max_bytes <= 32768 or not 1 <= args.timeout <= 3600:
        parser.error("max-bytes必须512..32768，timeout必须1..3600")
    if hasattr(args, "limit") and not 1 <= args.limit <= 100:
        parser.error("limit必须1..100")
    if hasattr(args, "offset") and args.offset < 0:
        parser.error("offset不能为负数")
    return args


def dispatch(db, args):
    cmd = args.command
    if cmd=="replay":
        row=verified(db,args.artifact);fp=json.loads(row["fingerprint"])
        if fp.get("backend")!="observer":raise ValueError("详细重放入口要求基础观察artifact")
        base=Path(args.observer or ROOT/"rust/target/release"/("gkt-observe.exe" if os.name=="nt" else "gkt-observe"))
        if file_hash(base)!=fp["binarySha256"]:raise ValueError("基础观察器版本已变化，请先使用冻结版本重现基础证据")
        data=read_json(row["raw_path"]);req=data["request"]
        if req.get("plain"):raise ValueError("请从有基础信息的对局选择详细重放")
        req.update(mode="detailed",flightSampleInterval=args.flight_sample_interval,maxDetailedEvents=args.max_detailed_events)
        out=compute(db,args.store,req,args,"detailed",expected_result=data["result"])
        return {**out,"replayOf":args.artifact,"resultMatchesBaseline":True}
    if cmd == "query":
        return query(db, args)
    if cmd == "compare":
        return compare(db, args)
    if cmd == "cohort":
        return cohort(db,args)
    if cmd == "diagnose":
        a, b = formations(args.p1), formations(args.p2)
        if len(a)!=1 or not 1 <= args.max_battles <= 64 or len(b)>args.max_battles:
            raise ValueError("诊断必须单一候选，1..max-battles个对手；max-battles上限64，不会悄悄截断输入")
        battles = []
        for index, fort in enumerate(b):
            req = {"p1":a[0]["code"], "p2":fort["code"], "maxFrames":args.max_frames, "seed":args.seed,
                   "snapshotInterval":args.snapshot_interval,"healthRecoveryWindowFrames":args.health_recovery_window_frames,"recordHalfHp":not args.no_half_hp, "plain":args.plain,
                   "mode":"detailed" if args.detailed else "basic","flightSampleInterval":args.flight_sample_interval,"maxDetailedEvents":args.max_detailed_events}
            if args.detailed and args.plain:raise ValueError("详细模式不能同时关闭观察")
            result = compute(db,args.store,req,args,"detailed" if args.detailed else "observer")
            battles.append({"opponentIndex":index,"artifact":result["artifact"], "cached":result["cached"],
                            "result":result["summary"]["result"], "complete":result["summary"]["summary"]["complete"]})
        # manifest保存完整名单，后续cohort无需一次读取全量遥测。
        manifest = {"schemaVersion":"gkt-diagnostics-v1", "battles":battles}
        folder = Path(args.store).resolve().parent/"jobs"
        folder.mkdir(exist_ok=True)
        path = folder/(digest(dumps(manifest).encode())+".json")
        if not path.exists():
            with path.open("x",encoding="utf-8") as file:
                file.write(dumps(manifest))
        return {"manifest":str(path), "battles":battles, "count":len(battles)}
    if cmd == "verify":
        row = verified(db, args.artifact)
        data = read_json(row["raw_path"])
        expected = normalized(data)
        actual_tables = {r[0]:r[1] for r in db.execute("SELECT table_name,COUNT(*) FROM records WHERE artifact=? GROUP BY table_name", (args.artifact,))}
        if actual_tables != {k:len(v) for k,v in expected.items() if v}:
            raise ValueError("数据库表/行数与原始证据不一致")
        for table, rows in expected.items():
            stored = db.execute("SELECT data FROM records WHERE artifact=? AND table_name=? ORDER BY row_index", (args.artifact, table)).fetchall()
            if [dumps(x) for x in rows] != [x[0] for x in stored]:
                raise ValueError(f"数据库索引与原始证据不一致：{table}")
        summary = json.loads(row["summary"])
        summary.pop("provenance",None)
        summary.pop("requestSha256",None)
        if summary != compact_summary(data,expected):
            raise ValueError("摘要与原始证据不一致")
        return {"artifact": args.artifact, "verified": True, "rawSha256":row["raw_sha256"], "tables":{k:len(v) for k,v in expected.items()}}
    if cmd == "list":
        return {"artifacts":[dict(r) for r in db.execute("SELECT id,action,created FROM artifacts ORDER BY created DESC,id LIMIT ?", (args.limit,))]}
    backend = "java"
    if cmd == "run":
        req = read_json(args.request)
        if "action" not in req:backend="detailed" if req.get("mode")=="detailed" else "observer"
    elif cmd == "formation":
        req = {"action":cmd, "p1":formations(args.file) if args.file else [{"name":"", "code":validate_code(args.code)}]}
    elif cmd in ("battle", "evaluate", "analyze"):
        a, b = formations(args.p1), formations(args.p2)
        if cmd == "battle":
            if len(a) != 1 or len(b) != 1:
                raise ValueError("稀疏事件只用于单场诊断；批量评测使用evaluate")
            backend = "detailed" if args.detailed else "observer"
            if args.detailed and args.plain:raise ValueError("详细模式不能同时关闭观察")
            req = {"p1":a[0]["code"], "p2":b[0]["code"], "maxFrames":args.max_frames, "seed":args.seed,
                   "snapshotInterval":args.snapshot_interval,"healthRecoveryWindowFrames":args.health_recovery_window_frames,"recordHalfHp":not args.no_half_hp, "plain":args.plain,
                   "mode":"detailed" if args.detailed else "basic","flightSampleInterval":args.flight_sample_interval,"maxDetailedEvents":args.max_detailed_events}
        else:
            req = {"action":"battle" if cmd == "evaluate" else args.kind, "p1":a, "p2":b,
                   "maxFrames":args.max_frames, "threads":args.threads, "maxJobs":args.max_jobs}
            if cmd == "evaluate":
                req["bothSides"] = args.both_sides
            else:
                req.update(deleteCount=args.delete_count, strength=args.strength, filter=args.filter, binFrames=args.bin_frames)
    else:
        req = {"action":cmd}
    return compute(db, args.store, req, args, backend)


def main(argv=None):
    if os.environ.get("CONDA_DEFAULT_ENV") != "opencode":
        print("请先conda activate opencode", file=sys.stderr)
        return 2
    args = parse_args(argv)
    try:
        with connect(args.store) as db:
            out = dispatch(db, args)
        text = dumps(out)
        if len(text.encode()) > args.max_bytes:
            out = {"error":"输出超过字节预算", "hint":"使用query的--fields、--limit或增大--max-bytes", "maxBytes":args.max_bytes,
                   **({"artifact":out["artifact"], "saved":True} if "artifact" in out else {}),
                   **({"manifest":out["manifest"], "saved":True} if "manifest" in out else {})}
            text = dumps(out)
        print(text)
        return 0
    except (ValueError, OSError, sqlite3.Error, subprocess.TimeoutExpired) as e:
        print(dumps({"error":str(e), "failed":True}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
