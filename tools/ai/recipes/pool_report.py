#!/usr/bin/env python3
"""从已核验完整结果生成排名、画像和真实父阵增失分；不启动模拟。"""
from __future__ import annotations
import argparse
from collections import Counter
import csv
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ai
import formation_codec as codec
from evaluate_pool import score
import gravity

CONTRACT = ('side', 'maxFrames', 'seedPolicy', 'scoringPolicy', 'fingerprint',
            'opponentInputSha256', 'opponentCount')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_verified(folder, opponents):
    folder = Path(folder).resolve()
    data = ai.read_json(folder/'results.json')
    manifest = ai.read_json(folder/'manifest.json')
    gate = ai.read_json(folder/'gate.json')
    proof = ai.read_json(folder/'verification.json')
    result_sha = ai.file_hash(folder/'results.json')
    require(data['manifest']==manifest and manifest['scoringPolicy']==ai.POLICY, '清单或计分政策不符')
    require(gate.get('passed') and proof.get('passed') and gate['E']==proof['E']==0, '需要完整独立verify且E=0的结果')
    require(proof['scoringPolicy']==manifest['scoringPolicy'], '独立核验的计分政策不同')
    require(gate['resultsSha256']==proof['resultsSha256']==result_sha, '结果已改变或独立核验摘要过期')
    require(manifest['opponentInputSha256']==ai.file_hash(opponents), '对手文件字节或顺序不同')
    n = len(ai.formations(opponents))
    rows = data['candidates']; side = manifest['side']
    require(side in (1, 2) and proof['side']==side, '非法或不同物理侧位')
    require(n==manifest['opponentCount']==proof['N']==gate['N'], '对手数量不符')
    require(len(rows)==manifest['candidateCount']==proof['candidates']==gate['candidates']
            and len({r['code'] for r in rows})==len(rows) and rows, '候选数量或唯一性不符')
    require(proof['scoredGames']==n*len(rows), '独立核验的完整局数不符')
    for row in rows:
        ai.validate_code(row['code'])
        require(row['side']==side and row['sha256']==ai.digest(row['code'].encode()), '候选身份或侧位不符')
        require(len(row['outcomes'])==n and all(row[k]==v for k, v in score(row['outcomes'], side).items()),
                '逐局长度或计分不符')
    return data, {'run': str(folder), 'resultsSha256': result_sha,
                  'verificationSha256': ai.file_hash(folder/'verification.json')}


def validate_groups(value):
    require(isinstance(value, list), 'groups须为列表')
    names = {'all'}
    for group in value:
        require(isinstance(group, dict) and not group.keys()-{'name', 'anyIds', 'allIds', 'noIds', 'counts', 'accelMin', 'accelMax', 'core'}, '分群含未知字段')
        name = group.get('name')
        require(isinstance(name, str) and name.strip() and name not in names, '分群名称为空、重复或占用all')
        names.add(name)
        for key in ('anyIds', 'allIds', 'noIds'):
            if key in group:
                require(isinstance(group[key], list) and group[key] and all(type(v) is int and 1<=v<=60 for v in group[key]), f'{key}须为非空单位ID列表')
        counts = group.get('counts', {})
        require(isinstance(counts, dict), 'counts须为单位ID到数量区间的对象')
        for uid, bounds in counts.items():
            require(isinstance(uid, str) and uid.isdecimal() and str(int(uid))==uid and 1<=int(uid)<=60, 'counts键须为1..60的十进制ID')
            require(isinstance(bounds, dict) and bounds and not bounds.keys()-{'min','max'}, '数量区间仅支持min/max')
            require(all(type(v) is int and 0<=v<=511 for v in bounds.values())
                    and bounds.get('min',0)<=bounds.get('max',511), '单位数量区间非法')
        for key in ('accelMin', 'accelMax'):
            if key in group:
                require(type(group[key]) is int and group[key]>=0, f'{key}须为非负整数')
        require(group.get('accelMin', 0)<=group.get('accelMax', 1022), '加速区间倒置')
        core = group.get('core', {})
        require(isinstance(core, dict) and not core.keys()-{'types', 'xMin', 'xMax', 'yMin', 'yMax'}, 'core分群含未知字段')
        if 'types' in core:
            require(isinstance(core['types'], list) and core['types'] and all(type(t) is int and t in (0,61,62) for t in core['types']), 'core.types使用解码ID0/61/62')
        for key in ('xMin', 'xMax', 'yMin', 'yMax'):
            if key in core:
                require(type(core[key]) is int and 0<=core[key]<=276, '核心范围须在0..276')
        for axis in ('x', 'y'):
            require(core.get(axis+'Min', 0)<=core.get(axis+'Max', 276), '核心区间倒置')
    return value


def metadata(opponents, groups):
    rows = []
    for index, fort in enumerate(opponents):
        units = codec.decode(fort['code']); counts = Counter(u[0] for u in units[1:])
        ids = set(counts); accel = counts[29]+2*counts[30]; core = units[0]
        tags = ['all']
        for group in groups:
            if 'anyIds' in group and not ids.intersection(group['anyIds']):
                continue
            if not set(group.get('allIds', []))<=ids or ids.intersection(group.get('noIds', [])):
                continue
            if any(not bounds.get('min',0)<=counts[int(uid)]<=bounds.get('max',511)
                   for uid, bounds in group.get('counts',{}).items()):
                continue
            if not group.get('accelMin', 0)<=accel<=group.get('accelMax', 1022):
                continue
            region = group.get('core', {})
            if 'types' in region and core[0] not in region['types']:
                continue
            if any(not region.get(axis+'Min', 0)<=core[k]<=region.get(axis+'Max', 276)
                   for axis, k in (('x', 1), ('y', 2))):
                continue
            tags.append(group['name'])
        rows.append({'index': index, 'ordinal': index+1, 'name': fort.get('name', ''),
                     'codeSha256': ai.digest(fort['code'].encode()), 'tags': tags,
                     'counts': dict(counts), 'accel': accel, 'core': core})
    return rows


def subset_score(outcomes, indices, side):
    if not indices:
        return {'N': 0, 'W': 0, 'L': 0, 'D': 0, 'U': 0, 'E': 0, 'scorePct': None}
    return score([outcomes[i] for i in indices], side)


def points(outcome, side):
    return 1 if outcome[0]==side else .5 if outcome[0] in (0,-1) else 0


def compare(before, after, meta, side):
    require(len(before)==len(after)==len(meta), '比较长度不同，禁止zip静默截断')
    score(before, side); score(after, side)
    delta = [points(b, side)-points(a, side) for a, b in zip(before, after)]
    gains = [i for i, d in enumerate(delta) if d>0]
    losses = [i for i, d in enumerate(delta) if d<0]
    return {'improvedOpponents': len(gains), 'regressedOpponents': len(losses),
            'addedPoints': sum(delta[i] for i in gains), 'lostPoints': -sum(delta[i] for i in losses),
            'netPoints': sum(delta), 'deltaPercentagePoints': 100*sum(delta)/len(delta),
            'improvedIndices': gains, 'regressedIndices': losses,
            'outcomeChangedIndices': [i for i, (a,b) in enumerate(zip(before, after)) if a!=b],
            'scoreChanges': [{'index': i, 'ordinal': i+1, 'name': meta[i]['name'], 'tags': meta[i]['tags'],
                              'before': before[i], 'after': after[i], 'deltaPoints': d}
                             for i, d in enumerate(delta) if d]}


def report(args):
    opponents = ai.formations(args.opponents)
    groups = validate_groups(ai.read_json(args.groups)) if args.groups else []
    meta = metadata(opponents, groups)
    data, own_proof = read_verified(args.run, args.opponents)
    manifest = data['manifest']; side = manifest['side']
    all_rows = {r['code']: r for r in data['candidates']}
    proofs = [own_proof]
    for folder in args.reference_run:
        other, proof = read_verified(folder, args.opponents)
        require(all(other['manifest'][k]==manifest[k] for k in CONTRACT), '参照结果的引擎、seed、侧位或阵池契约不同')
        proofs.append(proof)
        for row in other['candidates']:
            if row['code'] in all_rows:
                require(all_rows[row['code']]['outcomes']==row['outcomes'], '同码同契约结果不同')
            all_rows[row['code']] = row
    parents = {}
    if args.parents:
        source_rows = gravity.load_candidates(args.parents)
        require(len({r['code'] for r in source_rows})==len(source_rows), '父阵映射中候选纯码重复')
        parents = {r['code']: r.get('parentCode') for r in source_rows}
    if args.parent_code:
        ai.validate_code(args.parent_code)
    group_indices = {'all': list(range(len(meta)))}
    group_indices.update({g['name']: [m['index'] for m in meta if g['name'] in m['tags']] for g in groups})
    entries = []
    for row in data['candidates']:
        item = {k: row[k] for k in ('name', 'code', 'sha256', 'side', 'N', 'W', 'L', 'D', 'U', 'E', 'scorePct')}
        item['halfPoints'] = 2*row['W']+row['D']+row['U']
        item['groups'] = {name: subset_score(row['outcomes'], indices, side) for name, indices in group_indices.items()}
        parent = parents.get(row['code']) or args.parent_code
        if args.parents or args.parent_code:
            require(parent is not None and parent in all_rows, '真实父阵缺映射或没有完整同契约结果，不能默认回退基线')
            item['parentCode'] = parent
            item['editsFromParent'] = codec.changes(codec.decode(parent), codec.decode(row['code']))
            before = all_rows[parent]['outcomes']
            item['comparison'] = compare(before, row['outcomes'], meta, side)
            for name, indices in group_indices.items():
                item['groups'][name]['before'] = subset_score(before, indices, side)
                changes = [points(row['outcomes'][i], side)-points(before[i], side) for i in indices]
                item['groups'][name].update(netPoints=sum(changes), addedPoints=sum(d for d in changes if d>0),
                                           lostPoints=-sum(d for d in changes if d<0))
        entries.append(item)
    ranked = sorted(entries, key=lambda item: -item['halfPoints'])
    ranks = {}
    for index, row in enumerate(ranked, 1):
        row['rank'] = ranks.setdefault(row['halfPoints'], index)
    result = {'schema': 'gkt-pool-study-v1', 'contract': {k: manifest[k] for k in CONTRACT},
              'evidence': proofs, 'opponentsSha256': ai.file_hash(args.opponents), 'metadata': meta,
              'groupDefinitions': groups, 'groupsSha256': ai.file_hash(args.groups) if args.groups else None,
              'parentsSha256': ai.file_hash(args.parents) if args.parents else None,
              'toolHashes': {p.name: ai.file_hash(p) for p in (Path(__file__), Path(ai.__file__), Path(codec.__file__))},
              'candidates': ranked, 'scope': '核对既有独立verification及结果哈希、完整性和计分；不重读大raw，不代替verify_pool，不启动对局。'}
    out = Path(args.out).resolve(); out.mkdir(parents=True, exist_ok=False)
    with (out/'report.json').open('x', encoding='utf-8') as stream:
        stream.write(ai.dumps(result)+'\n')
    with (out/'ranking.csv').open('x', encoding='utf-8-sig', newline='') as stream:
        writer = csv.writer(stream); writer.writerow(['排名', '名称', '胜率', '胜', '负', '平', '未决', '总局数', '纯码SHA256'])
        writer.writerows([r['rank'], r['name'], r['scorePct'], r['W'], r['L'], r['D'], r['U'], r['N'], r['sha256']] for r in ranked)
    (out/'leaders.txt').write_text('/'.join((r['name']+'&' if r['name'] else '')+r['code'] for r in ranked if r['rank']==1), encoding='utf-8')
    lines = ['# 完整阵池研究摘要', '', f'物理侧位：{side}；每阵N={len(opponents)}；E=0。', '',
             '本报告复用已核验完整结果，分群可重叠，不相加、不改变对手权重。', '',
             '| 排名 | 名称 | 胜率 | 增加/损失/净计分（相对真实父阵） |', '|---:|---|---:|---|']
    for row in ranked:
        change = row.get('comparison')
        delta = '/'.join(str(change[k]) for k in ('addedPoints','lostPoints','netPoints')) if change else '未指定父阵'
        name = row['name'].replace('|','／').replace('\n',' ')
        lines.append(f'| {row["rank"]} | {name} | {row["scorePct"]:.6f}% | {delta} |')
    lines += ['', '完整改善/退步索引、精确改动和分群见report.json；这些变化帮助提出假设，不单独证明因果。']
    (out/'REPORT.md').write_text('\n'.join(lines)+'\n', encoding='utf-8')
    print(f'完整结果研究完成：{len(ranked)}阵，{len(opponents)}个对手；报告 {out}；新增模拟0局。')
    return result


def main(argv=None):
    if os.environ.get('CONDA_DEFAULT_ENV')!='opencode':
        raise ValueError('请先conda activate opencode')
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('run', 'opponents', 'out'):
        parser.add_argument('--'+name, required=True)
    parser.add_argument('--reference-run', action='append', default=[])
    parser.add_argument('--parents', help='带真实parentCode的候选JSON；不按名称猜父阵')
    parser.add_argument('--parent-code', help='仅在整份当前名单确实共享一个父阵时使用')
    parser.add_argument('--groups', help='预先声明的分群JSON列表')
    report(parser.parse_args(argv))


if __name__=='__main__':
    main()
