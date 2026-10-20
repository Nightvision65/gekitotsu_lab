"""一键验收；全量记录留文件，终端只输出阶段摘要。"""
import os
from pathlib import Path
import subprocess
import sys
import time
from build_paths import rust_environment

if os.environ.get("CONDA_DEFAULT_ENV") != "opencode":
    sys.exit("请先conda activate opencode")
timeout=int(os.environ.get("GKT_TEST_TIMEOUT","300"))
folder=Path("ai-data/verification")/f"run-{time.time_ns()}"
folder.mkdir(parents=True)
env=rust_environment()
env["JAVA_TOOL_OPTIONS"]="-Dfile.encoding=UTF-8 -Dsun.stdout.encoding=UTF-8 -Dsun.stderr.encoding=UTF-8"

def run(command,name):
    path=folder/f"{name}.log"
    print(f"验收阶段：{name}",flush=True)
    with path.open("wb") as log:
        proc=subprocess.Popen(command,env=env,stdout=log,stderr=subprocess.STDOUT)
        try:
            proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            if os.name=="nt":subprocess.run(["taskkill","/PID",str(proc.pid),"/T","/F"],capture_output=True,timeout=10)
            else:proc.kill()
            proc.wait(timeout=10)
            raise
    if proc.returncode:
        print(path.read_text(encoding="utf-8",errors="replace")[-3000:],flush=True)
        raise subprocess.CalledProcessError(proc.returncode,command)

try:
    run([sys.executable,"tools/ai/prepare_core.py"],"source-and-hooks")
    run([sys.executable,"tools/ai/gravity.py","build","--timeout",str(timeout)],"gravity-build")
    run([sys.executable,"-m","unittest","discover","-s","tests","-p","test_*.py","-v"],"store-and-contract")
    run([sys.executable,"tests/verify_native_hp.py","--out",str(folder/"release-hp.json")],"release-hp")
    run([sys.executable,"tests/verify_native_hp.py","--jar","target/gekitotsu_lab-0.6.7.7.jar","--out",str(folder/"research-hp.json")],"research-hp")
    run(["javac","-encoding","UTF-8","-cp","target/gekitotsu_lab-0.6.7.7.jar","-d","target/test-tools",
         "tests/UnitHpRulesCheck.java","tests/UpdateReleaseCheck.java"],"java-release-checks-compile")
    for entry in ("UnitHpRulesCheck", "org.example.UpdateReleaseCheck"):
        run(["java","-cp","target/test-tools;target/gekitotsu_lab-0.6.7.7.jar",entry],entry)
    run([os.environ.get("GKT_CARGO","cargo"),"test","--release","--locked","--offline","--manifest-path","rust/Cargo.toml","--","--nocapture"],"rust-public-tests")
    run([os.environ.get("GKT_CARGO","cargo"),"test","--locked","--offline","--manifest-path","rust/gravity/Cargo.toml"],"rust-gravity-tests")
    run([sys.executable,"tests/verify_integration.py","--out",str(folder/"engine-200.json")],"engine-200")
    run([sys.executable,"tests/verify_runtime.py","--out",str(folder/"runtime.json")],"windows-runtime")
    run([sys.executable,"tests/verify_cli.py","--out",str(folder/"cli.json")],"cli-workflow")
    if os.environ.get("GKT_BENCHMARK")=="1":
        run([sys.executable,"tests/benchmark_observer.py","--out",str(folder/"performance.json")],"basic-performance")
    print(f"验收完成：{folder.resolve()}。公开telemetry.bin/frames金样缺失项见rust日志，不能计作对拍通过。",flush=True)
except (OSError,subprocess.SubprocessError) as e:
    sys.exit(f"验收失败：{e}")
