#!/usr/bin/env python3
"""批量开局重力审核、应用稳定态，以及强制先审核的候选评测入口。"""
from __future__ import annotations
import argparse
import os
import signal
from pathlib import Path
import subprocess
import sys

import ai
from build_paths import rust_environment

ROOT = ai.ROOT
CRATE = ROOT / 'rust/gravity'
BINARY = CRATE / 'target/release' / ('gkt-gravity.exe' if os.name == 'nt' else 'gkt-gravity')
BUILD = CRATE / 'build.json'
POLICY = 'static-support-gravity-v2'
MAX_SUPPORT_PUSH_UP = 35.0
EPSILON = 0.001


def bounded(command, timeout):
    process=subprocess.Popen(command,cwd=ROOT,env=rust_environment(),start_new_session=os.name!='nt')
    try:
        return process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        if os.name=='nt':
            subprocess.run(['taskkill','/PID',str(process.pid),'/T','/F'],capture_output=True,timeout=10)
        else:
            os.killpg(process.pid,signal.SIGKILL)
        if process.poll() is None:process.kill()
        process.wait(timeout=10)
        raise


def checked(command, timeout):
    status=bounded(command,timeout)
    if status:raise subprocess.CalledProcessError(status,command)


def source_hash():
    paths = [CRATE/'Cargo.toml', CRATE/'Cargo.lock', ROOT/'rust/Cargo.toml']
    paths += sorted((CRATE/'src').rglob('*.rs'))
    paths += sorted((ROOT/'rust/vendor/gkt-core').rglob('*.rs'))
    paths += [ROOT/'rust/vendor/gkt-core/Cargo.toml']
    return ai.digest(ai.dumps([(p.relative_to(ROOT).as_posix(), ai.file_hash(p)) for p in paths]).encode())


def save(path, value):
    with Path(path).open('x', encoding='utf-8') as f:
        f.write(ai.dumps(value)+'\n')


def build(args):
    # Offline by default: packages used by the existing Rust observers are already cached.
    cmd = ['cargo', 'build', '--release', '--offline', '--manifest-path', str(CRATE/'Cargo.toml')]
    checked(cmd,args.timeout)
    value = {'policy': POLICY, 'sourceSha256': source_hash(), 'binarySha256': ai.file_hash(BINARY)}
    BUILD.write_text(ai.dumps(value)+'\n', encoding='utf-8')
    print('重力审核器构建完成；原有JAR、DLL和对战观察器保持原文件。')


def fingerprint():
    if not BINARY.is_file() or not BUILD.is_file():
        raise ValueError('请先运行 python tools/ai/gravity.py build')
    frozen = ai.read_json(BUILD)
    if frozen != {'policy': POLICY, 'sourceSha256': source_hash(), 'binarySha256': ai.file_hash(BINARY)}:
        raise ValueError('重力工具源码或二进制已改变，请重新build')
    return {**frozen, 'adapterSha256': ai.file_hash(__file__), 'inputAdapterSha256': ai.file_hash(ai.__file__)}


def load_candidates(path):
    path = Path(path)
    value = ai.read_json(path) if path.suffix.lower()=='.json' else ai.formations(path)
    rows = value['candidates'] if isinstance(value, dict) else value
    if not isinstance(rows, list) or not rows or len(rows)>20000:
        raise ValueError('输入必须为1至20000个阵，可用 candidates JSON 或 / 分隔阵码文件')
    for row in rows:
        ai.validate_code(row['code'])
        if any(c in row.get('name', '') for c in '&/#\r\n'):
            raise ValueError('阵名含分隔符')
    return rows


def compute(rows, out, args, suffix=''):
    req = {'formations': [{k: row.get(k,'') for k in ('name','code')} for row in rows],
           'maxFrames': args.max_frames, 'stableFrames': args.stable_frames}
    request = out / ('request'+suffix+'.json'); result = out / ('result'+suffix+'.json')
    save(request, req)
    checked([str(BINARY), str(request), str(result)],args.timeout)
    data = ai.read_json(result)
    if data.get('policy')!=POLICY or len(data['formations'])!=len(rows):
        raise ValueError('重力输出政策或数量不符')
    if [r['code'] for r in data['formations']] != [r['code'] for r in rows]:
        raise ValueError('重力输出顺序或阵码不符')
    return data


def audit(args, apply=False):
    fp = fingerprint(); rows = load_candidates(args.input)
    out = Path(args.out).resolve()
    # No overwrite or ambiguous resume. Each audit has immutable input and evidence.
    out.mkdir(parents=True, exist_ok=False)
    save(out/'input.json', rows)
    manifest = {'schema': 'gkt-gravity-audit-v1', 'policy': POLICY, 'fingerprint': fp,
                'inputPath': str(Path(args.input).resolve()), 'inputSha256': ai.file_hash(args.input),
                'maxFrames': args.max_frames, 'stableFrames': args.stable_frames,
                'inputCount': len(rows), 'mode': 'apply' if apply else 'audit'}
    save(out/'manifest.json', manifest)
    data = compute(rows, out, args)
    results = data['formations']
    passed = all(r['auditPassed'] for r in results)
    applied = []
    aliases = []
    if apply and all(r['converged'] and r['encodable'] and r['appliedCode'] for r in results):
        # Different input codes can converge to the same code; keep first order and aliases.
        by_code = {}
        for index,(row,result) in enumerate(zip(rows,results)):
            code = result['appliedCode']
            if code not in by_code:
                by_code[code] = len(applied)
                # Old scores/hashes describe the old code. Never copy them onto a new code.
                applied.append({'name': row.get('name',''), 'code': code,
                                'gravitySourceCode': row['code'], 'gravitySourceIndex': index})
            aliases.append({'inputIndex': index, 'name': row.get('name',''),
                            'sourceSha256': ai.digest(row['code'].encode()), 'outputIndex': by_code[code]})
        checked = compute(applied, out, args, '-applied')
        passed = all(r['auditPassed'] and r['appliedCode']==r['code'] for r in checked['formations'])
        if passed:
            save(out/'candidates.json', applied)
            (out/'formations.txt').write_text('/'.join((r.get('name','')+'&' if r.get('name') else '')+r['code'] for r in applied), encoding='utf-8')
    elif apply:
        passed = False
    gate = {'schema': 'gkt-gravity-gate-v1', 'policy': POLICY, 'passed': passed,
            'manifestSha256': ai.file_hash(out/'manifest.json'), 'resultSha256': ai.file_hash(out/'result.json'),
            'inputCount': len(rows), 'changedFormations': sum(r['changedUnits']>0 for r in results),
            'changedUnits': sum(r['changedUnits'] for r in results),
            'nonConverged': sum(not r['converged'] for r in results),
            'unencodable': sum(not r['encodable'] for r in results)}
    if apply:
        gate['outputCount']=len(applied) if passed else 0
        if passed:
            gate['appliedResultSha256']=ai.file_hash(out/'result-applied.json')
            gate['candidateOutputSha256']=ai.file_hash(out/'candidates.json')
        save(out/'aliases.json', aliases)
    save(out/'gate.json', gate)
    lines = ['# 开局重力审核', '', '政策：所有特殊效果关闭；车板、核心、全部墙体固定初始形状。仅调用实际Ball.land。全程上移容限35px，的玉（ID42）豁免下落限制，其他兵玉拒绝下落；连续静止与收敛仅作观察，不作审核门禁。', '',
             f"通过：{passed}；输入{len(rows)}阵，变化{gate['changedFormations']}阵/{gate['changedUnits']}单位，未收敛{gate['nonConverged']}，不可编码{gate['unencodable']}。", '',
             '| 索引 | 阵名 | 收敛 | 移动单位 | 最大上移 | 最大下落 |', '|---|---|---|---:|---:|---:|']
    for i,r in enumerate(results):
        lines.append(f"| {i} | {r['name'].replace('|','／')} | {r['converged']} | {r['changedUnits']} | {max(u['maxUp'] for u in r['units']):.2f} | {max(u['maxDown'] for u in r['units']):.2f} |")
    lines += ['', '各槽位初始/最终坐标、位移、支撑与最后移动帧见result.json。应用模式只在全部输出经再次审核、幂等通过后发布candidates.json。新码需重新测战斗成绩。']
    (out/'REPORT.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(f"重力{'应用' if apply else '审核'}：{'通过' if passed else '未通过'}；变化{gate['changedFormations']}/{len(rows)}阵，{gate['changedUnits']}单位；结果 {out}")
    return 0 if passed else 3


def verify_audit(folder, source):
    """Verify immutable audit evidence before resuming evaluations. No simulations."""
    folder=Path(folder);manifest=ai.read_json(folder/'manifest.json');gate=ai.read_json(folder/'gate.json')
    if manifest['inputSha256']!=ai.file_hash(source) or manifest['fingerprint']!=fingerprint():
        raise ValueError('重力审核输入或工具版本已改变，须在新目录重新审核')
    if gate['manifestSha256']!=ai.file_hash(folder/'manifest.json') or gate['resultSha256']!=ai.file_hash(folder/'result.json'):
        raise ValueError('重力证据哈希不同')
    rows=load_candidates(source);data=ai.read_json(folder/'result.json')
    if data['policy']!=POLICY or manifest['policy']!=POLICY:
        raise ValueError('重力政策不同')
    results=data['formations']
    if [r['code'] for r in rows]!=[r['code'] for r in results]:
        raise ValueError('重力证据候选顺序不同')
    if manifest['mode']=='apply':
        if not all(r['converged'] and r['encodable'] for r in results):
            raise ValueError('源阵无完整可编码稳定态')
        if gate['appliedResultSha256']!=ai.file_hash(folder/'result-applied.json') or gate['candidateOutputSha256']!=ai.file_hash(folder/'candidates.json'):
            raise ValueError('应用结果哈希不同')
        applied=ai.read_json(folder/'candidates.json');results=ai.read_json(folder/'result-applied.json')['formations']
        if [r['code'] for r in applied]!=[r['code'] for r in results]:raise ValueError('应用结果顺序不同')
    def permitted(unit):
        return (abs(unit['delta']['x'])<=EPSILON
                and 0<=unit['maxUp']<=MAX_SUPPORT_PUSH_UP+EPSILON
                and 0<=unit['maxDown']
                and (unit['maxDown']<=EPSILON or unit['typeId']==42)
                and -unit['maxUp']-EPSILON<=unit['delta']['y']<=unit['maxDown']+EPSILON)
    passed=(data.get('maxSupportPushUp')==MAX_SUPPORT_PUSH_UP and
            all(r['auditPassed'] and all(permitted(u) for u in r['units']) for r in results))
    if manifest['mode']=='apply':
        passed=passed and all(r['changedUnits']==0 and r['appliedCode']==r['code'] for r in results)
    if not passed or not gate['passed']:raise ValueError('重力门禁未通过')
    return gate


def evaluate(args):
    if '--candidates' in args.runner_args:
        raise ValueError('--candidates只能在重力入口指定一次')
    # The evaluator receives exactly the original, audited bytes. It may never apply silently.
    audit_args = argparse.Namespace(**vars(args), input=args.candidates, out=args.audit_out)
    if Path(args.audit_out).exists():
        verify_audit(args.audit_out,args.candidates);status=0
        print('已核验既有重力门禁，复用审核证据。',flush=True)
    else:status = audit(audit_args)
    if status:
        print('重力门禁未通过，未启动任何对局；修正布局或先apply再重测。')
        return status
    gate = ai.read_json(Path(args.audit_out)/'gate.json')
    manifest = ai.read_json(Path(args.audit_out)/'manifest.json')
    if manifest['inputSha256']!=ai.file_hash(args.candidates) or not gate['passed']:
        raise ValueError('审核后候选改变')
    extra = args.runner_args[1:] if args.runner_args[:1]==['--'] else args.runner_args
    command = [sys.executable, str(ROOT/'tools/ai/evaluate_pool.py'), '--candidates',args.candidates,*extra]
    # Child runner has its own per-batch timeout and time-limit; outer bound is explicit too.
    return bounded(command,args.evaluation_timeout)


def main(argv=None):
    if os.environ.get('CONDA_DEFAULT_ENV')!='opencode':
        raise ValueError('请先conda activate opencode')
    p=argparse.ArgumentParser(description=__doc__)
    sub=p.add_subparsers(dest='command',required=True)
    b=sub.add_parser('build');b.add_argument('--timeout',type=float,default=240)
    v=sub.add_parser('verify');v.add_argument('--run',required=True);v.add_argument('--input',required=True)
    for name in ('audit','apply','evaluate'):
        q=sub.add_parser(name)
        if name=='evaluate':
            q.add_argument('--candidates',required=True);q.add_argument('--audit-out',required=True)
            q.add_argument('--evaluation-timeout',type=float,default=3600)
        else:
            q.add_argument('--input',required=True);q.add_argument('--out',required=True)
        q.add_argument('--max-frames',type=int,default=240)
        q.add_argument('--stable-frames',type=int,default=12)
        q.add_argument('--timeout',type=float,default=120)
        if name=='evaluate':q.add_argument('runner_args',nargs=argparse.REMAINDER)
    args=p.parse_args(argv)
    if args.command=='verify':verify_audit(args.run,args.input);print('重力证据核验通过。');return 0
    if args.timeout<=0: p.error('进程超时须为正数')
    if args.command=='build':build(args);return 0
    if not 2<=args.stable_frames<=args.max_frames<=2000: p.error('要求2 <= stable-frames <= max-frames <= 2000')
    if args.command=='evaluate':return evaluate(args)
    return audit(args,apply=args.command=='apply')


if __name__=='__main__':
    try: sys.exit(main())
    except (ValueError,OSError,subprocess.SubprocessError) as e:
        sys.exit(f'重力操作失败：{e}')
