"""隔离核验指定JAR的Java及JNI HP规则，记录实际包和DLL指纹。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    if os.environ.get("CONDA_DEFAULT_ENV") != "opencode":
        raise SystemExit("请先conda activate opencode")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, default=ROOT / "app/gekitotsu_java-1.9.1.jar")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    jar = args.jar.resolve()
    if args.out.exists():
        raise ValueError("拒绝覆盖核验记录")
    with zipfile.ZipFile(jar) as archive:
        dll = archive.read("native/gkt_jni.dll")
    expected = json.loads((ROOT / "app/kit-manifest.json").read_text(encoding="utf-8"))["nativeLibrary"]["sha256"]
    if sha(dll) != expected:
        raise ValueError("JAR未包含已知catalog-hp-v1 JNI DLL")
    env = dict(os.environ)
    env.pop("GEKITOTSU_NATIVE_PATH", None)
    logs = {}
    with tempfile.TemporaryDirectory(prefix="gkt-release-hp-") as tmp:
        classes = Path(tmp) / "classes"
        classes.mkdir()
        command = ["javac", "-encoding", "UTF-8", "--release", "21", "-cp", str(jar), "-d", str(classes)]
        command += [str(ROOT / "tests/UnitHpRulesCheck.java"), str(Path(__file__).with_name("NativeHpRulesCheck.java"))]
        subprocess.run(command, cwd=tmp, env=env, check=True, capture_output=True, timeout=30)
        for check in ("UnitHpRulesCheck", "NativeHpRulesCheck"):
            command = ["java", "--enable-native-access=ALL-UNNAMED", "-Dfile.encoding=UTF-8",
                       "-Dsun.stdout.encoding=UTF-8", "-Dsun.stderr.encoding=UTF-8",
                       "-cp", str(classes) + os.pathsep + str(jar), check]
            process = subprocess.run(command, cwd=tmp, env=env, check=True, capture_output=True, timeout=30)
            logs[check] = process.stdout.decode("utf-8")
            print(logs[check], end="", flush=True)
    report = {"status": "passed", "ruleset": "catalog-hp-v1", "jar": str(jar),
              "jarSha256": sha(jar.read_bytes()), "bundledDllSha256": sha(dll), "checks": logs,
              "scope": "Java构造HP/max_hp及加速贡献；真实JNI无治疗耗尽HP；分发DLL指纹核验"}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("x", encoding="utf-8") as output:
        json.dump(report, output, ensure_ascii=False, indent=2)
    print(f"发行包HP核验完成：{args.out}")


if __name__ == "__main__":
    main()
