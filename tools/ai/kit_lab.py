"""激突Kit 1.9.1 的外部实验入口；Python 必须在 conda opencode 中执行。"""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[2]
ALPHABET = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"

def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()

def read_forts(path: Path, bounds: bool) -> list[dict]:
    text = path.read_text(encoding="utf-8-sig")
    result = []
    for part in text.split("/"):
        if not part.strip():
            continue
        if part.count("&") > 1:
            raise ValueError("同一条出现多个 &；多个阵必须用 / 分隔，不能仅换行")
        name, sep, code = part.strip().partition("&")
        if not sep:
            code, name = name, f"fort-{len(result):04d}"
        name, code = name.strip(), "".join(code.split())
        if not name or any(c in name for c in "\r\n#"):
            raise ValueError("阵名不能为空，也不能含换行或 #")
        if len(code) < 6 or len(code) % 6 or code[0] not in "012":
            raise ValueError(f"{name}: 编码须为 6 的倍数，首字符为 0..2")
        if any(c not in ALPHABET[:61] for c in code):
            raise ValueError(f"{name}: 只接受规范编码；拒绝 #HP、Z、注释及非编码字符")
        for offset in range(0, len(code), 6):
            if offset and not 1 <= ALPHABET.index(code[offset]) <= 60:
                raise ValueError(f"{name}: 非核心槽位类型必须为 1..60")
            value = 0
            for c in code[offset + 1:offset + 6]:
                value = value * 61 + ALPHABET.index(c)
            r, raw_x, raw_y = value // 1000000, value // 1000 % 1000, value % 1000
            x, y = (raw_x - 52, raw_y - 58) if offset == 0 else (raw_x - 16, raw_y - 20)
            max_x, max_y = (276, 276) if offset == 0 else (348, 349)
            if bounds and not (0 <= r < 360 and 0 <= x <= max_x and 0 <= y <= max_y):
                raise ValueError(f"{name}: slot {offset // 6} 超出工作台范围；先 audit 检查，勿静默裁剪")
        result.append({"name": name, "code": code, "sha256": sha(code.encode("ascii"))})
    if not result:
        raise ValueError(f"阵集为空: {path}")
    return result

def run_child(command: list[str], cwd: Path, timeout: int) -> dict:
    proc = subprocess.run(command, cwd=cwd, capture_output=True, timeout=timeout)
    logs = {"stdout": proc.stdout.decode("utf-8", errors="replace"),
            "stderr": proc.stderr.decode("utf-8", errors="replace")}
    if proc.returncode:
        raise RuntimeError(f"子进程退出码 {proc.returncode}\n{logs['stdout']}\n{logs['stderr']}")
    return logs

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["probe", "catalog", "audit", "battle", "contribution", "matchup", "battlefield"])
    parser.add_argument("--jar", type=Path, default=ROOT / "app/gekitotsu_java-1.9.1.jar")
    parser.add_argument("--p1", type=Path)
    parser.add_argument("--p2", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--engine", choices=["rust", "java"], default="rust")
    parser.add_argument("--both-sides", action="store_true")
    parser.add_argument("--max-frames", type=int, default=65536)
    parser.add_argument("--threads", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=180, help="Java 执行超时秒数；编译另限 30 秒")
    parser.add_argument("--max-jobs", type=int, default=10000)
    parser.add_argument("--delete-count", type=int, default=1)
    parser.add_argument("--strength", action="store_true")
    parser.add_argument("--filter", choices=["ALL", "WIN", "LOSE", "DRAW", "TIMEOUT"], default="ALL")
    parser.add_argument("--bin-frames", type=int, default=120)
    args = parser.parse_args()
    if os.environ.get("CONDA_DEFAULT_ENV") != "opencode":
        parser.error("请先执行 conda activate opencode")
    if args.out.exists():
        parser.error("输出文件已存在；请换一个实验路径，保留旧记录")
    if min(args.max_frames, args.timeout, args.max_jobs, args.bin_frames) < 1 or not 1 <= args.threads <= 256:
        parser.error("帧数、超时、任务上限及分箱须为正数，线程须为 1..256")
    analytical = args.action in {"contribution", "matchup", "battlefield"}
    if analytical and args.engine != "rust":
        parser.error("分析模式固定调用 Rust，避免自动回退改变评测口径")
    req = {"action": args.action, "engine": args.engine, "maxFrames": args.max_frames,
           "threads": args.threads, "bothSides": args.both_sides, "deleteCount": args.delete_count,
           "strength": args.strength, "filter": args.filter, "binFrames": args.bin_frames}
    inputs = {}
    if args.action not in {"probe", "catalog"}:
        if args.p1 is None:
            parser.error("此命令需要 --p1")
        req["p1"] = read_forts(args.p1, bounds=args.action != "audit")
        inputs["p1"] = {"path": str(args.p1.resolve()), "sha256": sha(args.p1.read_bytes())}
        if args.action != "audit":
            if args.p2 is None:
                parser.error("此命令需要 --p2")
            req["p2"] = read_forts(args.p2, bounds=True)
            inputs["p2"] = {"path": str(args.p2.resolve()), "sha256": sha(args.p2.read_bytes())}
            n, m = len(req["p1"]), len(req["p2"])
            jobs = n * m * (2 if args.both_sides and args.action == "battle" else 1)
            if analytical and n != 1:
                parser.error("分析模式的 --p1 必须恰好一个原阵")
            if args.action == "contribution":
                count = len(req["p1"][0]["code"]) // 6 - 1
                if not 1 <= args.delete_count <= count:
                    parser.error("删除数量超出非核心单位数")
                jobs = (math.comb(count, args.delete_count) + 1) * m
            elif args.action == "battlefield":
                jobs = 2 * m
            elif args.action == "matchup" and args.strength and m >= 6:
                jobs += min(m * (m - 1), 160000)
            if jobs > args.max_jobs:
                parser.error(f"预计最多 {jobs} 局，超过 --max-jobs {args.max_jobs}")
            req["estimatedMaxJobs"] = jobs
    java, javac = shutil.which("java"), shutil.which("javac")
    if not java or not javac:
        parser.error("需要 JDK 21+ 的 java 和 javac；EXE 自带运行时不等于 JDK")
    jar = args.jar.resolve()
    source = Path(__file__).with_name("KitLab.java")
    output = args.out.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    # 每次单独工作目录，避免污染 GUI 阵集、配置和已有对局缓存。
    with tempfile.TemporaryDirectory(prefix="kit-lab-") as temp:
        work = Path(temp)
        dll = None
        if args.engine == "rust" and args.action not in {"audit", "catalog"}:
            with zipfile.ZipFile(jar) as archive:
                dll = archive.read("native/gkt_jni.dll")
            (work / "gkt_jni.dll").write_bytes(dll)
        run_child([javac, "-encoding", "UTF-8", "-cp", str(jar), "-d", str(work), str(source)], work, 30)
        request, response = work / "request.json", work / "response.json"
        request.write_text(json.dumps(req, ensure_ascii=False), encoding="utf-8")
        command = [java, "-Dfile.encoding=UTF-8", "-Djava.awt.headless=true", "--enable-native-access=ALL-UNNAMED"]
        if dll is not None:
            command.append("-Dgekitotsu.native.path=" + str(work / "gkt_jni.dll"))
        command += ["-cp", os.pathsep.join([str(work), str(jar)]), "KitLab", str(request), str(response)]
        logs = run_child(command, work, args.timeout)
        result = json.loads(response.read_text(encoding="utf-8"))
        result["provenance"] = {"createdUtc": datetime.now(timezone.utc).isoformat(),
            "jar": str(jar), "jarSha256": sha(jar.read_bytes()), "dllSha256": sha(dll) if dll else None,
            "bridgeSha256": sha(source.read_bytes()), "wrapperSha256": sha(Path(__file__).read_bytes()),
            "java": java, "javac": javac, "python": sys.executable, "inputs": inputs,
            "request": req, "timeoutSeconds": args.timeout, "isolatedCache": True, "logs": logs}
        output.write_text(json.dumps(result, ensure_ascii=False, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(f"完成：{output}")

if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, OSError, subprocess.TimeoutExpired) as error:
        print(f"执行失败：{error}", file=sys.stderr)
        sys.exit(1)
