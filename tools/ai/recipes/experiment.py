#!/usr/bin/env python3
"""通用实验建档、带真实超时的单步执行，以及文件/SQLite依赖快照核验。"""
from __future__ import annotations
import argparse
from contextlib import closing
from datetime import datetime, timezone
import math
import os
from pathlib import Path
import re
import shutil
import signal
import sqlite3
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import ai
import formation_codec as codec
import formation_rules
import gravity


def save(path, value):
    with Path(path).open('x', encoding='utf-8') as stream:
        stream.write(ai.dumps(value)+'\n')


def stamp():
    return datetime.now(timezone.utc).isoformat()


def init(args):
    source, rule_path = Path(args.source).resolve(strict=True), Path(args.rules).resolve(strict=True)
    rows, rules = gravity.load_candidates(source), ai.read_json(rule_path)
    formation_rules.validate(rules)
    parent = next((row for row in rows if row['code']==rules['parentCode']), None)
    if parent is None:
        raise ValueError('源输入中没有rules.parentCode')
    reason = formation_rules.check(codec.decode(parent['code']), rules)
    if reason is not None:
        raise ValueError(f'父阵不满足声明规则：{reason}')
    pools = {}
    for spec in args.pool:
        label, separator, filename = spec.partition('=')
        if not separator or not re.fullmatch(r'[a-zA-Z0-9_-]+', label) or label in pools:
            raise ValueError('--pool须为不重复的label=文件路径')
        path = Path(filename).resolve(strict=True)
        pools[label] = (path, len(ai.formations(path)))
    jar = Path(args.jar).resolve(strict=True)
    out = Path(args.out).resolve()
    if out.exists():
        raise FileExistsError(f'禁止覆盖：{out}')
    probe = ai.probe_java_engine(jar, args.java, args.probe_timeout)
    engine = probe['fingerprint']
    out.mkdir(parents=True, exist_ok=False)
    inputs = out/'inputs'; inputs.mkdir()
    records = []
    def freeze(origin, target):
        before = ai.file_hash(origin)
        shutil.copyfile(origin, target)
        if before != ai.file_hash(target) or before != ai.file_hash(origin):
            raise ValueError(f'冻结时文件改变：{origin}')
        records.append({'source': str(origin), 'path': target.relative_to(out).as_posix(), 'sha256': before})
    freeze(source, inputs/('source'+source.suffix))
    freeze(rule_path, inputs/'rules.json')
    save(inputs/'base.json', [{'name': parent.get('name', ''), 'code': parent['code']}])
    save(inputs/'units.json', [{'slot': i, 'unit': u, 'name': codec.CATALOG[u[0]]['name']}
                              for i, u in enumerate(codec.decode(parent['code']))])
    for label, (path, count) in pools.items():
        freeze(path, inputs/f'pool-{label}.txt')
    freeze(jar, inputs/'engine.jar')
    if ai.file_hash(inputs/'engine.jar')!=engine['binarySha256']:
        raise ValueError('探测与冻结的JAR不同，不发布建档身份')
    save(inputs/'engine-probe.json', probe)
    shutil.copyfile(ai.ROOT/'docs/ai/templates/STATE.md', out/'STATE.md')
    metadata = ai.read_json(ai.ROOT/'docs/ai/templates/experiment.json')
    metadata['runId'] = out.name
    metadata['rules']['budget'] = rules['budget']
    metadata['rules']['allowedCoreTypes'] = [codec.decode(parent['code'])[0][0]]
    metadata['evaluation'].update(jarFile='inputs/engine.jar', jarSha256=engine['binarySha256'],
                                  dllSha256=engine['dllSha256'], engineProbeFile='inputs/engine-probe.json')
    metadata['inputs'].update(baselineFile='inputs/base.json', benchmarkFile=None,
                             benchmarkManifest=None, benchmarkSha256=None,
                             pools={label: {'file': f'inputs/pool-{label}.txt', 'count': count,
                                           'sha256': ai.file_hash(inputs/f'pool-{label}.txt')}
                                    for label, (_, count) in pools.items()})
    save(out/'experiment.json', metadata)
    tool_paths = [Path(__file__), HERE/'stable_dedup.py', HERE/'pool_report.py', HERE/'script_inventory.py']
    tool_paths += [HERE.parent/name for name in ('ai.py', 'gravity.py', 'formation_codec.py',
                   'formation_rules.py', 'formation_workflow.py', 'evaluate_pool.py', 'verify_pool.py')]
    save(inputs/'identity.json', {'schema': 'gkt-experiment-inputs-v1', 'createdAtUtc': stamp(),
         'parentCodeSha256': ai.digest(parent['code'].encode()), 'files': records,
         'pools': {label: {'count': count, 'sha256': ai.file_hash(inputs/f'pool-{label}.txt')}
                   for label, (_, count) in pools.items()},
         'engine': {'file': 'inputs/engine.jar', 'fingerprint': engine,
                    'probeFile': 'inputs/engine-probe.json', 'probeSha256': ai.file_hash(inputs/'engine-probe.json')},
         'toolHashes': {p.relative_to(ai.ROOT).as_posix(): ai.file_hash(p) for p in tool_paths},
         'status': 'prepared', 'note': '只建档，不执行基线或搜索；STATE、元数据和搜索预期须按任务填写。'})
    print(f'实验已建档：{out}；仅冻结输入，未启动评测。')


def step(args):
    command = args.command[1:] if args.command[:1]==['--'] else args.command
    if not command or not math.isfinite(args.timeout) or args.timeout<=0:
        raise ValueError('须提供命令及正数真实超时')
    cwd = Path(args.cwd).resolve(strict=True)
    out = Path(args.out).resolve(); out.mkdir(parents=True, exist_ok=False)
    save(out/'command.json', {'command': command, 'cwd': str(cwd), 'timeoutSeconds': args.timeout,
                             'acceptedExitCodes': args.accept_exit, 'startedAtUtc': stamp()})
    started = time.monotonic()
    process = None
    try:
        with (out/'output.log').open('x', encoding='utf-8') as log:
            process = subprocess.Popen(command, cwd=cwd, stdout=log, stderr=subprocess.STDOUT,
                                       start_new_session=os.name!='nt', stdin=subprocess.DEVNULL)
            save(out/'process.json', {'pid': process.pid, 'startedAtUtc': stamp()})
            print(f'已启动PID {process.pid}；日志 {out / "output.log"}', flush=True)
            try:
                status = process.wait(timeout=args.timeout)
            except subprocess.TimeoutExpired:
                if os.name=='nt':
                    subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'], capture_output=True, timeout=15)
                else:
                    os.killpg(process.pid, signal.SIGKILL)
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=15)
                save(out/'result.json', {'status': 'timeout', 'pid': process.pid,
                     'elapsedSeconds': time.monotonic()-started, 'finishedAtUtc': stamp()})
                print('步骤已超时并终止进程树；该步骤未通过。', flush=True)
                return 124
        passed = status in args.accept_exit
        save(out/'result.json', {'status': 'complete' if passed else 'failed', 'exitCode': status,
             'pid': process.pid, 'elapsedSeconds': time.monotonic()-started, 'finishedAtUtc': stamp()})
        print(f'步骤结束：退出码{status}；日志 {out / "output.log"}')
        return 0 if passed else 1
    except OSError as error:
        if not (out/'result.json').exists():
            save(out/'result.json', {'status': 'failed', 'error': str(error), 'finishedAtUtc': stamp()})
        raise


def files_under(root):
    files = []
    for directory, dirs, names in os.walk(root, followlinks=False):
        for name in dirs+names:
            if (Path(directory)/name).is_symlink():
                raise ValueError(f'快照不追踪符号链接：{Path(directory)/name}')
        files.extend(Path(directory)/name for name in names)
    return sorted(files)


def file_record(path, root=None):
    before = path.stat()
    sha = ai.file_hash(path)
    after = path.stat()
    if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
        raise ValueError(f'快照过程中仍有文件写入：{path}')
    return {'path': path.relative_to(root).as_posix() if root else str(path),
            'size': before.st_size, 'sha256': sha}


def snapshot(args):
    root = Path(args.root).resolve(strict=True)
    out = Path(args.out).resolve()
    if not root.is_dir() or out.is_relative_to(root):
        raise ValueError('快照清单须放在被检查目录之外，避免自引用')
    if out.exists():
        raise FileExistsError(f'禁止覆盖：{out}')
    paths = files_under(root)
    if any(p.name.endswith(('-wal', '-shm', '.tmp')) for p in paths):
        raise ValueError('存在WAL/SHM或临时文件，须先确认写入已经结束；不自动修改数据库')
    records = [file_record(p, root) for p in paths]
    databases, dependencies = [], {}
    for path in paths:
        if path.suffix.lower() not in {'.sqlite', '.sqlite3', '.db'}:
            continue
        with closing(sqlite3.connect(path.as_uri()+'?mode=ro&immutable=1', uri=True, timeout=10)) as db:
            if [r[0] for r in db.execute('PRAGMA integrity_check')] != ['ok']:
                raise ValueError(f'SQLite完整性检查失败：{path}')
            databases.append(path.relative_to(root).as_posix())
            columns = {r[1] for r in db.execute('PRAGMA table_info(artifacts)')}
            if not {'raw_path', 'raw_sha256'} <= columns:
                continue
            for raw_path, raw_sha in db.execute('SELECT raw_path,raw_sha256 FROM artifacts'):
                raw = Path(raw_path)
                raw = raw.resolve(strict=True) if raw.is_absolute() else (ai.ROOT/raw).resolve(strict=True)
                if str(raw) in dependencies:
                    if dependencies[str(raw)]['sha256'] != raw_sha:
                        raise ValueError('同一raw路径声明了不同哈希')
                    continue
                record = file_record(raw)
                if record['sha256'] != raw_sha:
                    raise ValueError(f'数据库引用raw哈希不同：{raw}')
                dependencies[str(raw)] = record
    if paths != files_under(root) or any(p.stat().st_size!=r['size'] or ai.file_hash(p)!=r['sha256'] for p, r in zip(paths, records)):
        raise ValueError('快照期间目录改变，未发布清单')
    external = [r for p, r in sorted(dependencies.items()) if not Path(p).is_relative_to(root)]
    value = {'schema': 'gkt-experiment-snapshot-v1', 'root': str(root), 'createdAtUtc': stamp(),
             'files': records, 'externalFiles': external, 'sqliteIntegrityChecked': databases,
             'count': len(records), 'bytes': sum(r['size'] for r in records),
             'scope': '目录全部文件与SQLite引用raw的字节核验；外部依赖只绑定哈希，不复制，不构成可搬迁备份。'}
    out.parent.mkdir(parents=True, exist_ok=True)
    save(out, value)
    print(f'快照核验通过：{len(records)}文件，外部raw依赖{len(external)}份；清单 {out}')


def verify_snapshot(args):
    data = ai.read_json(args.manifest)
    if data.get('schema') != 'gkt-experiment-snapshot-v1':
        raise ValueError('不支持的快照schema')
    root = Path(args.root or data['root']).resolve(strict=True)
    if not root.is_dir():
        raise ValueError('快照复核根路径须为目录')
    actual = {p.relative_to(root).as_posix(): p for p in files_under(root)}
    expected = {r['path']: r for r in data['files']}
    if len(expected)!=len(data['files']) or set(actual)!=set(expected):
        raise ValueError('目录文件缺失、新增或清单条目重复')
    for name, path in actual.items():
        if file_record(path, root) != expected[name]:
            raise ValueError(f'文件字节不同：{name}')
    for record in data['externalFiles']:
        if file_record(Path(record['path']).resolve(strict=True)) != record:
            raise ValueError(f'外部raw依赖改变：{record["path"]}')
    print(f'快照复核通过：{len(actual)}份文件及{len(data["externalFiles"])}份外部依赖。')


def main(argv=None):
    if os.environ.get('CONDA_DEFAULT_ENV')!='opencode':
        raise ValueError('请先conda activate opencode')
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    p = sub.add_parser('init')
    for name in ('source', 'rules', 'out'):
        p.add_argument('--'+name, required=True)
    p.add_argument('--pool', action='append', required=True)
    p.add_argument('--jar', required=True, help='支持--ai及JNI的研究JAR，冻结后用于后续全部评测')
    p.add_argument('--java')
    p.add_argument('--probe-timeout', type=float, default=30)
    p = sub.add_parser('step')
    p.add_argument('--out', required=True); p.add_argument('--timeout', type=float, required=True)
    p.add_argument('--cwd', default=str(ai.ROOT))
    p.add_argument('--accept-exit', type=int, action='append', default=[0])
    p.add_argument('command', nargs=argparse.REMAINDER)
    p = sub.add_parser('snapshot')
    p.add_argument('--root', required=True); p.add_argument('--out', required=True)
    p = sub.add_parser('verify-snapshot')
    p.add_argument('--manifest', required=True); p.add_argument('--root')
    args = parser.parse_args(argv)
    return {'init': init, 'step': step, 'snapshot': snapshot, 'verify-snapshot': verify_snapshot}[args.action](args)


if __name__=='__main__':
    sys.exit(main() or 0)
