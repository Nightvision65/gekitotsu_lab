#!/usr/bin/env python3
"""通用静态稳定态去重；位移通过/未通过统一分组，搜索预期留给本轮检查。"""
from __future__ import annotations
import argparse
import math
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import ai
import formation_codec as codec
import gravity

SCHEMA = 'gkt-stable-dedup-v1'
KEY_POLICY = 'ordered-static-geometry-grid005-v1'
TICKS_PER_PIXEL = 20
PROVENANCE = ('group', 'hypothesis', 'source', 'parentCode', 'parentSha256', 'choices')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def save(path, value):
    gravity.save(path, value)


def identity():
    return {'policy': KEY_POLICY, 'gravity': gravity.fingerprint(),
            'toolSha256': ai.file_hash(__file__), 'codecSha256': ai.file_hash(codec.__file__),
            'side': 1, 'coordinateSpace': 'kit-workbench', 'frozenSpecialEffects': True}


def tick(value):
    require(type(value) in (int, float) and math.isfinite(value), '稳定坐标必须为有限数值')
    result = round(value*TICKS_PER_PIXEL)
    require(abs(value-result/TICKS_PER_PIXEL)<=gravity.EPSILON,
            '稳定坐标不在引擎0.05px量子格点上，不能近似合并')
    return result


def geometry(result):
    require(result['converged'] is True, '未收敛末帧不能作为稳定态')
    units = result['units']
    require(units and [u['slot'] for u in units]==list(range(len(units))), '稳定态槽位缺失或调序')
    return [[u['typeId'], tick(u['final']['x']), tick(u['final']['y']), u['r']] for u in units]


def key_for(layout, model):
    return ai.digest(ai.dumps({'model': model, 'units': layout}).encode())


def simulate(rows, folder, args, prefix='batch'):
    results, evidence = [], []
    for offset in range(0, len(rows), args.batch_size):
        chunk = rows[offset:offset+args.batch_size]
        suffix = f'-{prefix}-{len(evidence):05d}'
        data = gravity.compute(chunk, folder, args, suffix)
        require(data.get('coordinateSpace')=='kit-workbench' and data.get('frozenSpecialEffects') is True,
                '静态模型或坐标系不符')
        results.extend(data['formations'])
        evidence += [f'request{suffix}.json', f'result{suffix}.json']
    return results, evidence


def verify(folder):
    """Check a frozen deduplication run without simulations or search acceptance."""
    folder = Path(folder).resolve()
    manifest = ai.read_json(folder/'manifest.json'); summary = ai.read_json(folder/'summary.json')
    require(manifest['schema']==summary['schema']==SCHEMA, '稳定态去重schema不同')
    require(manifest['identity']==identity(), '去重工具或重力模型身份改变，须在新目录重做')
    require(summary['manifestSha256']==ai.file_hash(folder/'manifest.json'), '去重清单哈希不同')
    require({'input.json','results.json','classes.json','aliases.json','candidates.json','pending.json'}<=set(summary['files']),
            '去重证据文件清单不完整')
    for name, sha in summary['files'].items():
        path = (folder/name).resolve()
        require(path.is_relative_to(folder) and ai.file_hash(path)==sha, f'去重证据哈希不同：{name}')
    classes = ai.read_json(folder/'classes.json')
    candidates = ai.read_json(folder/'candidates.json')
    require(len(classes)==summary['uniqueStableLayouts'] and len(candidates)==summary['draftCandidates'], '去重计数不同')
    keys = set()
    for index, row in enumerate(classes):
        require(row['classIndex']==index and row['stableKey']==key_for(row['geometryTicks'], manifest['identity']),
                '稳定态指纹或类索引不符')
        require(row['stableKey'] not in keys, '稳定态重复类未合并')
        keys.add(row['stableKey'])
    codes = set()
    for row in candidates:
        ai.validate_code(row['code'])
        require(row['code'] not in codes and row['sha256']==ai.digest(row['code'].encode()), '代表纯码重复或哈希不同')
        codes.add(row['code'])
        group = classes[row['classIndex']]
        require(row['stableKey']==group['stableKey'] and row['code']==group['representative']['code']
                and not group['excludedBy'] and row['requiresSearchAcceptance'] is True,
                '代表映射或待本轮验收状态不符')
    return manifest, classes, summary


def run(args):
    require(2<=args.stable_frames<=args.max_frames<=2000 and math.isfinite(args.timeout) and args.timeout>0,
            '要求2 <= stable-frames <= max-frames <= 2000，超时须为正数')
    require(1<=args.batch_size<=20000, 'batch-size须为1..20000')
    model = identity()
    previous, previous_manifests = {}, []
    for path in args.exclude_run:
        manifest, classes, summary = verify(path)
        require(manifest['stableFrames']==args.stable_frames, '历史去重的收敛窗口不同')
        previous_manifests.append({'run': str(Path(path).resolve()),
                                   'manifestSha256': ai.file_hash(Path(path)/'manifest.json'),
                                   'summarySha256': ai.file_hash(Path(path)/'summary.json')})
        for row in classes:
            previous.setdefault(row['stableKey'], []).append({'run': str(Path(path).resolve()), 'classIndex': row['classIndex']})
    sources, unique, by_code, inputs = [], [], {}, []
    for path in args.input:
        path = Path(path).resolve()
        before = ai.file_hash(path); rows = gravity.load_candidates(path)
        require(before==ai.file_hash(path), '读取期间输入发生改变')
        source_file = len(inputs)
        inputs.append({'path': str(path), 'sha256': before, 'count': len(rows)})
        for source_row, row in enumerate(rows):
            code = row['code']
            if code not in by_code:
                by_code[code] = len(unique)
                unique.append({'name': row.get('name',''), 'code': code})
            sources.append({'inputIndex': len(sources), 'sourceFileIndex': source_file,
                            'sourceRowIndex': source_row, 'simulationIndex': by_code[code], 'row': row})
    out = Path(args.out).resolve(); out.mkdir(parents=True, exist_ok=False)
    save(out/'input.json', sources)
    save(out/'manifest.json', {'schema': SCHEMA, 'identity': model, 'inputs': inputs,
         'maxFrames': args.max_frames, 'stableFrames': args.stable_frames, 'batchSize': args.batch_size,
         'timeoutSecondsPerBatch': args.timeout, 'excludedRuns': previous_manifests,
         'representativePolicy': 'passing-source-then-encoded-stable-then-source-v1',
         'purpose': '几何去重草稿；本轮预期和最终代表约束须另行自动验收'})
    results, evidence = simulate(unique, out, args)
    save(out/'results.json', results)
    classes, by_key, aliases, pending = [], {}, [], []
    for source in sources:
        row = source['row']; result = results[source['simulationIndex']]
        alias = {k: source[k] for k in ('inputIndex','sourceFileIndex','sourceRowIndex','simulationIndex')}
        alias.update(name=row.get('name',''), code=row['code'], sourceCodeSha256=ai.digest(row['code'].encode()),
                     auditPassed=result['auditPassed'], converged=result['converged'], classIndex=None, outputIndex=None)
        if not result['converged']:
            pending.append({**alias, 'reason': 'non-converged'})
            aliases.append(alias)
            continue
        layout = geometry(result); key = key_for(layout, model)
        if key not in by_key:
            by_key[key] = len(classes)
            classes.append({'classIndex': len(classes), 'stableKey': key, 'geometryTicks': layout,
                            'units': [{'slot': u['slot'], 'typeId': u['typeId'], 'category': u['category'],
                                       'x': g[1]/TICKS_PER_PIXEL, 'y': g[2]/TICKS_PER_PIXEL, 'r': u['r']}
                                      for u, g in zip(result['units'], layout)],
                            'sourceIndices': [], 'representative': None, 'excludedBy': previous.get(key,[])})
        group = classes[by_key[key]]; group['sourceIndices'].append(source['inputIndex'])
        alias['classIndex'] = group['classIndex']
        choice = {'code': row['code'], 'sourceIndex': source['inputIndex'],
                  'representation': 'source', 'sourceAuditPassed': result['auditPassed']}
        if result['auditPassed']:
            priority = 0
        elif result['encodable'] and result['appliedCode']:
            choice.update(code=result['appliedCode'], representation='stable')
            priority = 1
        else:
            priority = 2
        current = group['representative']
        current_priority = (0 if current['sourceAuditPassed'] else
                            1 if current['representation']=='stable' else 2) if current else 3
        if priority < current_priority:
            group['representative'] = choice
        aliases.append(alias)
    # Verify newly encoded representatives in batches, never inherit source scores.
    stable_groups = [g for g in classes if g['representative'] and g['representative']['representation']=='stable'
                     and not g['excludedBy']]
    if stable_groups:
        replay, proof_files = simulate([{'code': g['representative']['code']} for g in stable_groups], out, args, 'representative')
        evidence.extend(proof_files)
        for group, result in zip(stable_groups, replay):
            require(result['converged'] and result['encodable'] and result['changedUnits']==0
                    and result['appliedCode']==result['code'] and result['auditPassed']
                    and geometry(result)==group['geometryTicks'], '稳定态代表复核未通过，不发布去重输出')
    candidates = []
    for group in classes:
        if group['excludedBy']:
            continue
        representative = group['representative']
        require(representative is not None, '稳定类缺少来源代表')
        original = sources[representative['sourceIndex']]['row']
        candidate = {k: original[k] for k in PROVENANCE if k in original}
        candidate.update(name=original.get('name',''), code=representative['code'],
                         sha256=ai.digest(representative['code'].encode()), stableKey=group['stableKey'],
                         classIndex=group['classIndex'], sourceIndex=representative['sourceIndex'],
                         sourceCode=original['code'], representation=representative['representation'],
                         sourceAuditPassed=representative['sourceAuditPassed'],
                         requiresSearchAcceptance=True)
        units = codec.decode(candidate['code'])
        candidate.update(coordinates='kit-workbench', cost=sum(codec.CATALOG[u[0]]['cost'] for u in units),
                         accelLevel=sum({29: 1, 30: 2}.get(u[0], 0) for u in units))
        if original.get('parentCode'):
            candidate['parentSha256'] = ai.digest(ai.validate_code(original['parentCode']).encode())
            candidate['changes'] = codec.changes(codec.decode(original['parentCode']), codec.decode(candidate['code']))
        for source_index in group['sourceIndices']:
            aliases[source_index]['outputIndex'] = len(candidates)
        candidates.append(candidate)
    save(out/'classes.json', classes); save(out/'aliases.json', aliases)
    save(out/'candidates.json', candidates); save(out/'pending.json', pending)
    files = ['input.json','results.json','classes.json','aliases.json','candidates.json','pending.json',*evidence]
    summary = {'schema': SCHEMA, 'status': 'needs-review' if pending else 'complete',
         'manifestSha256': ai.file_hash(out/'manifest.json'), 'files': {p: ai.file_hash(out/p) for p in files},
         'inputCount': len(sources), 'uniqueSourceCodes': len(unique), 'uniqueStableLayouts': len(classes),
         'sourceAuditPassed': sum(a['auditPassed'] for a in aliases),
         'sourceAuditFailed': sum(not a['auditPassed'] for a in aliases),
         'nonConverged': sum(not a['converged'] for a in aliases),
         'excludedLayouts': sum(bool(g['excludedBy']) for g in classes), 'draftCandidates': len(candidates),
         'unencodableSourceRepresentatives': sum(c['representation']=='source' and not c['sourceAuditPassed'] for c in candidates),
         'requiresSearchAcceptance': True}
    save(out/'summary.json', summary)
    lines = ['# 稳定态统一去重', '',
             f'输入{len(sources)}阵/{len(unique)}个唯一原码，收敛到{len(classes)}个唯一稳定布局；草稿{len(candidates)}阵。',
             f'原码位移通过{summary["sourceAuditPassed"]}，未通过{summary["sourceAuditFailed"]}；未收敛{summary["nonConverged"]}。',
             f'历史重复布局{summary["excludedLayouts"]}；负坐标等不可编码几何保留来源原码{summary["unencodableSourceRepresentatives"]}。', '',
             '位移通过与未通过统一按完整稳定几何去重，保留槽位顺序、全部来源和别名。',
             'candidates.json为去重草稿；按本轮预期批量检查后再冻结评测输入。',
             'classes.json提供稳定位置，aliases.json→simulationIndex→results.json提供各来源支撑与位移证据。',
             '未收敛者见pending.json，不当成最终淘汰。不可编码几何可用合法来源原码表示，仍须本轮验收。几何相同不证明源原码战斗等价。']
    (out/'REPORT.md').write_text('\n'.join(lines)+'\n', encoding='utf-8')
    print(f'稳定态去重：{len(sources)}个来源 → {len(classes)}个稳定布局 → {len(candidates)}个待本轮验收草稿；结果 {out}')
    return 3 if pending or not candidates else 0


def main(argv=None):
    require(os.environ.get('CONDA_DEFAULT_ENV')=='opencode', '请先conda activate opencode')
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('run')
    p.add_argument('--input', action='append', required=True)
    p.add_argument('--out', required=True)
    p.add_argument('--exclude-run', action='append', default=[])
    p.add_argument('--max-frames', type=int, default=240)
    p.add_argument('--stable-frames', type=int, default=12)
    p.add_argument('--batch-size', type=int, default=2000)
    p.add_argument('--timeout', type=float, default=120)
    p = sub.add_parser('verify'); p.add_argument('--run', required=True)
    args = parser.parse_args(argv)
    if args.command=='verify':
        verify(args.run); print('稳定态去重证据核验通过；未执行搜索预期验收。'); return 0
    return run(args)


if __name__=='__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
        sys.exit(f'稳定态去重失败：{error}')
