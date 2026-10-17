"""检查Windows独立程序的运行库依赖，并重复运行公开测试输入。"""
import argparse
import json
import os
from pathlib import Path
import struct
import tempfile

from verify_integration import ROOT, ai, assert_detail, assert_trace, observe


def pe_imports(path):
    data = path.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    assert data[pe:pe+4] == b"PE\0\0"
    sections, opt_size = struct.unpack_from("<H", data, pe+6)[0], struct.unpack_from("<H", data, pe+20)[0]
    opt = pe+24
    assert struct.unpack_from("<H", data, opt)[0] == 0x20B
    import_rva = struct.unpack_from("<I", data, opt+112+8)[0]
    mapping = []
    for index in range(sections):
        pos = opt+opt_size+40*index
        virtual_size, virtual_address, raw_size, raw_offset = struct.unpack_from("<IIII", data, pos+8)
        mapping.append((virtual_address, max(virtual_size, raw_size), raw_offset))
    def offset(rva):
        for address, size, raw in mapping:
            if address <= rva < address+size:
                return raw+rva-address
        raise ValueError(f"未找到PE地址：{rva}")
    names = []
    pos = offset(import_rva)
    while any(data[pos:pos+20]):
        name = offset(struct.unpack_from("<I", data, pos+12)[0])
        names.append(data[name:data.index(b"\0", name)].decode("ascii"))
        pos += 20
    return sorted(names)


def main():
    if os.environ.get("CONDA_DEFAULT_ENV") != "opencode":
        raise SystemExit("请先conda activate opencode")
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    out = Path(args.out)
    if out.exists():
        raise SystemExit("拒绝覆盖验证记录")
    imports = {}
    for name in ("gkt-observe.exe", "gkt-detail.exe"):
        binary = ROOT/"rust/target/release"/name
        names = pe_imports(binary)
        assert not any("vcruntime" in x.lower() or "msvcp" in x.lower() for x in names), names
        imports[name] = {"sha256": ai.file_hash(binary), "imports": names}
    req = {"p1": ai.formations(ROOT/"docs/ai/examples/candidate.txt")[0]["code"],
           "p2": ai.formations(ROOT/"tests/fixtures/standard200.txt")[41]["code"],
           "maxFrames": 65536, "snapshotInterval": 120}
    repeats = 20
    with tempfile.TemporaryDirectory(prefix="gkt-runtime-") as folder:
        folder = Path(folder)
        baseline = observe({**req, "plain": True}, folder, "runtime-plain")["result"]
        for index in range(repeats):
            basic = observe(req, folder, f"runtime-basic-{index}")
            detail = observe({**req, "mode": "detailed"}, folder, f"runtime-detail-{index}")
            assert basic["result"] == detail["result"] == baseline
            assert_trace(basic, [req["p1"], req["p2"]])
            assert_detail(detail)
    report = {"status": "passed", "imports": imports,
              "opponentIndex": 41, "request": req, "repeatsPerMode": repeats, "result": baseline,
              "scope": "公开测试输入基础/详细各20次复跑；仅覆盖当前二进制与输入"}
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(ai.dumps(report), encoding="utf-8")
    print("运行库检查通过；公开测试输入基础/详细各复跑20次，结果与HP账本一致。")


if __name__ == "__main__":
    main()
