"""带实际进程超时的本地构建；代理不泄漏到国内Maven请求。"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
from build_paths import rust_environment

if os.environ.get("CONDA_DEFAULT_ENV") != "opencode":
    sys.exit("请先conda activate opencode")
timeout = int(os.environ.get("GKT_BUILD_TIMEOUT", "300"))
offline = os.environ.get("GKT_OFFLINE") == "1"
env = rust_environment()
for key in list(env):
    if key.lower() in ("http_proxy", "https_proxy", "all_proxy"):
        env.pop(key)
env["JAVA_TOOL_OPTIONS"] = "-Dfile.encoding=UTF-8 -Dsun.stdout.encoding=UTF-8 -Dsun.stderr.encoding=UTF-8"
maven = os.environ.get("GKT_MAVEN", "mvn")
cargo = os.environ.get("GKT_CARGO", "cargo")
for tool in (maven, cargo):
    if not Path(tool).is_file() and not shutil.which(tool):
        sys.exit(f"未找到构建工具：{tool}")

def bounded(command):
    proc = subprocess.Popen(command, env=env)
    try:
        status = proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(proc.pid), "/T", "/F"], capture_output=True, timeout=10)
        else:
            proc.kill()
        proc.wait(timeout=10)
        raise
    if status:
        raise subprocess.CalledProcessError(status, command)

try:
    print("核验冻结源码并生成详细观察核心",flush=True)
    bounded([sys.executable,"tools/ai/prepare_core.py"])
    print("构建Java共享分析接口", flush=True)
    bounded([maven, "-q", "-s", "tools/ai/maven-settings.xml", *( ["-o"] if offline else []), "package"])
    if not offline:
        for key in ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"):
            if os.environ.get(key):
                env[key] = os.environ[key]
    print("构建Rust基础与详细观察器", flush=True)
    bounded([cargo, "build", "--release", "--locked", *( ["--offline"] if offline else []),
                    "--manifest-path", "rust/Cargo.toml", "-p", "gkt-observe"])
    print("构建完成", flush=True)
except (OSError, subprocess.SubprocessError) as e:
    sys.exit(f"构建失败：{e}")
