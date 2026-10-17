"""参数计划生成器；评测与证据核验复用现存批量工具。"""
from __future__ import annotations
import argparse
from collections import Counter
import itertools
import math
import os
from pathlib import Path
import random
import subprocess
import sys

import ai
import formation_codec as codec
import formation_rules

require = formation_rules.require
keys = formation_rules.keys


def save(path, value):
    with Path(path).open('x', encoding='utf-8') as stream:
        stream.write(ai.dumps(value) + '\n')


def check(units, rules):
    return formation_rules.check(units, rules)


def load_rows(path):
    import gravity
    return gravity.load_candidates(path)


def compile_axes(group, template, rules):
    axes, assigned = [], set()
    require(isinstance(group.get('axes', []), list), 'axes须为列表')
    for axis in group.get('axes', []):
        keys(axis, ('slots', 'field', 'values', 'offsets', 'options'), 'axis')
        if 'options' in axis:
            require(set(axis) == {'options'}, 'options轴不能混用slots/field/values/offsets')
            options = axis['options']
        else:
            slots, field = axis['slots'], axis['field']
            require(field in codec.FIELDS, 'axis.field须为id/x/y/r')
            require(isinstance(slots, list) and slots and all(type(s) is int and 0 <= s < len(template) for s in slots)
                    and len(set(slots)) == len(slots), 'axis.slots越界或重复')
            require(('values' in axis) != ('offsets' in axis), 'axis必须且只能指定values或offsets')
            mode = 'offsets' if 'offsets' in axis else 'values'
            require(not (field == 'id' and mode == 'offsets'), 'id轴仅支持显式values')
            values = axis[mode]
            require(isinstance(values, list) and values and all(type(v) is int for v in values), 'axis须为非空整数取值列表')
            k = codec.FIELDS.index(field)
            options = [[{'slot': s, 'field': field, 'value': template[s][k] + v if mode == 'offsets' else v}
                        for s in slots] for v in values]
        require(isinstance(options, list) and options, 'options须为非空列表')
        touched = set()
        for option in options:
            require(isinstance(option, list), '每个option须为edit列表')
            this_option = set()
            for edit in option:
                keys(edit, ('slot', 'field', 'value'), 'edit')
                slot, field, value = edit['slot'], edit['field'], edit['value']
                require(type(slot) is int and 0 <= slot < len(template) and field in codec.FIELDS and type(value) is int,
                        'edit槽位、字段或整数值非法')
                if field == 'id':
                    require(rules['profile'] == 'structural-fixed-speed-v1' and slot > 0,
                            'id改动仅可用于结构规则的非核心槽位')
                require((slot, field) not in this_option, 'option重复写入同一slot字段')
                this_option.add((slot, field))
            touched.update(this_option)
        require(not assigned.intersection(touched), '不同axis重复写入同一slot字段')
        assigned.update(touched)
        axes.append(options)
    return axes


def sampled_combinations(axes, sample, seed):
    total = math.prod(len(axis) for axis in axes)
    if sample is None:
        require(total <= 1000000, '未抽样组合超过1000000，请拆分计划或声明sample与seed')
        return total, enumerate(itertools.product(*axes))
    require(type(sample) is int and 1 <= sample <= min(total, 1000000), 'sample须为正整数且不大于组合数及1000000')
    require(type(seed) is int and total <= sys.maxsize, 'seed须为整数，组合数不得溢出')
    # Select mixed-radix indices without allocating the full Cartesian product.
    def selected():
        for index in random.Random(seed).sample(range(total), sample):
            remaining, choice = index, []
            for options in reversed(axes):
                remaining, digit = divmod(remaining, len(options))
                choice.append(options[digit])
            yield index, tuple(reversed(choice))
    return total, selected()


def generate(args):
    plan = ai.read_json(args.plan)
    keys(plan, ('base', 'rules', 'groups', 'maxCandidates'), 'plan')
    rules = ai.read_json(ai.ROOT / plan['rules']) if isinstance(plan['rules'], str) else plan['rules']
    formation_rules.validate(rules)
    base_path = ai.ROOT / plan['base']
    base_rows = load_rows(base_path)
    require(any(r['code'] == rules['parentCode'] for r in base_rows), 'base中找不到rules.parentCode')
    require(type(plan.get('maxCandidates')) is int and 1 <= plan['maxCandidates'] <= 20000, 'maxCandidates须为1至20000')
    require(isinstance(plan.get('groups'), list) and plan['groups'], 'groups须为非空列表')
    rows, rejected, aliases, seen, attempts, total_combinations = [], [], [], {}, 0, 0
    for group in plan['groups']:
        keys(group, ('name', 'hypothesis', 'source', 'axes', 'template', 'parents', 'sample', 'seed'), 'group')
        require(isinstance(group.get('name'), str) and group['name'] and not any(c in group['name'] for c in '&/#\r\n'), 'group.name为空或含分隔符')
        if 'template' in group:
            require(rules['profile'] == 'structural-fixed-speed-v1', 'template仅可用于结构规则')
        require('seed' not in group or 'sample' in group, 'seed须与sample一起声明')
        bases = group.get('parents', base_rows)
        require(isinstance(bases, list) and bases, 'group.parents须为非空候选列表')
        for parent in bases:
            before = codec.decode(parent['code'])
            reason = check(before, rules)
            require(reason is None, f'父阵不符合rules：{reason}')
            template = group.get('template', before)
            codec.encode(template)
            axes = compile_axes(group, template, rules)
            total, combinations = sampled_combinations(axes, group.get('sample'), group.get('seed', 0))
            total_combinations += total
            for combination_index, combination in combinations:
                attempts += 1
                require(attempts <= 1000000, '全计划实际组合超过1000000，请拆分计划')
                units = [u[:] for u in template]
                for option in combination:
                    for edit in option:
                        key = codec.FIELDS.index(edit['field'])
                        units[edit['slot']][key] = edit['value'] % 360 if key == 3 else edit['value']
                reason = check(units, rules)
                origin = {'group': group['name'], 'parentSha256': ai.digest(parent['code'].encode()),
                          'combination': combination_index, 'choices': combination}
                if reason:
                    rejected.append({**origin, 'reason': reason})
                    continue
                code = codec.encode(units)
                if code in seen or code == parent['code']:
                    aliases.append({**origin, 'code': code, 'outputIndex': seen.get(code),
                                    'reason': 'unchanged-parent' if code == parent['code'] else 'duplicate-code'})
                    continue
                require(len(rows) < plan['maxCandidates'], f'唯一合法候选超过maxCandidates={plan["maxCandidates"]}；未发布截断结果')
                seen[code] = len(rows)
                rows.append({'name': f"{group['name']}-{len(rows)+1:05d}", 'code': code,
                             **origin, 'hypothesis': group.get('hypothesis', ''), 'source': group.get('source'),
                             'parentCode': parent['code'], 'sha256': ai.digest(code.encode()), 'changes': codec.changes(before, units),
                             'cost': sum(codec.CATALOG[u[0]]['cost'] for u in units),
                             'accelLevel': sum({29: 1, 30: 2}.get(u[0], 0) for u in units),
                             'coordinates': 'kit-workbench', 'requiresGravityAudit': True,
                             'timing': codec.timing(before, units), 'corePositionChangesDefaultSeed': before[0][1:3] != units[0][1:3]})
    folder = Path(args.out)
    folder.mkdir(parents=True, exist_ok=False)
    save(folder/'plan.json', plan)
    save(folder/'base.json', base_rows)
    save(folder/'rules.json', rules)
    save(folder/'aliases.json', aliases)
    save(folder/'rejected.json', rejected)
    if rows:
        save(folder/'candidates.json', rows)
        (folder/'formations.txt').write_text('/\n'.join(r['name']+'&'+r['code'] for r in rows)+'\n', encoding='utf-8')
    save(folder/'summary.json', {'schema': 'gkt-generation-v2', 'count': len(rows), 'attempted': attempts,
         'combinations': total_combinations, 'rejected': dict(Counter(r['reason'] for r in rejected)),
         'aliases': len(aliases), 'samplingPolicy': 'deterministic-index-v1', 'requiresGravityAudit': True,
         'inputSha256': ai.file_hash(args.plan), 'baseSha256': ai.file_hash(base_path),
         'baseSnapshotSha256': ai.file_hash(folder/'base.json'),
         'candidateSha256': ai.file_hash(folder/'candidates.json') if rows else None,
         'toolHashes': {p.name: ai.file_hash(p) for p in (Path(__file__), Path(codec.__file__), Path(formation_rules.__file__), Path(ai.__file__), codec.CATALOG_PATH)}})
    print(f'生成完成：{len(rows)}个唯一草稿；拒绝{len(rejected)}个方案。草稿须经重力审核后才可评测。')
    return 0 if rows else 3


def accept(args):
    """稳定化后重新核验规则、全局去重，并补回源阵实验元数据。"""
    rows = load_rows(args.source)
    applied = ai.read_json(Path(args.applied)/'candidates.json')
    rules = ai.read_json(args.rules)
    formation_rules.validate(rules)
    out = Path(args.out)
    for path in (out, Path(str(out)+'.rejected.json'), Path(str(out)+'.aliases.json'), Path(str(out)+'.audit.json')):
        require(not path.exists(), f'输出已存在，禁止覆盖：{path}')
    import gravity
    gravity.verify_audit(args.applied, args.source)
    require(ai.read_json(Path(args.applied)/'manifest.json')['mode'] == 'apply', 'accept须使用gravity apply输出')
    excluded = set()
    for path in args.exclude:
        old = load_rows(path)
        excluded.update(r['code'] for r in old)
    accepted, rejected, seen = [], [], set(excluded)
    for row in applied:
        source_index = row['gravitySourceIndex']
        require(type(source_index) is int and 0 <= source_index < len(rows), 'gravitySourceIndex越界')
        original = rows[source_index]
        require(original['code'] == row['gravitySourceCode'], '稳定态源阵身份不同')
        units = codec.decode(row['code'])
        reason = check(units, rules)
        if reason or row['code'] in seen:
            rejected.append({'name': row['name'], 'code': row['code'], 'reason': reason or '全局重复'})
            continue
        seen.add(row['code'])
        # Only provenance is transferable. Scores and hashes on the old code never are.
        provenance = {k: original[k] for k in ('group', 'hypothesis', 'source', 'parentCode', 'parentSha256', 'choices') if k in original}
        parent_code = original.get('parentCode', rules['parentCode'])
        before = codec.decode(parent_code)
        accepted.append({**provenance, **row, 'sha256': ai.digest(row['code'].encode()),
                         'parentCode': parent_code, 'parentSha256': ai.digest(parent_code.encode()),
                         'coordinates': 'kit-workbench', 'cost': sum(codec.CATALOG[u[0]]['cost'] for u in units),
                         'accelLevel': sum({29: 1, 30: 2}.get(u[0], 0) for u in units),
                         'changes': codec.changes(before, units), 'timing': codec.timing(before, units)})
    drop_result_hash = None
    if accepted and rules.get('dropControl'):
        control = rules['dropControl']
        controls = []
        for row in accepted:
            units = codec.decode(row['code'])
            units.pop(control['removeSlot'])
            controls.append({'name': row['name'], 'code': codec.encode(units)})
        output = Path(str(args.out)+'.drop-control')
        output.mkdir(exist_ok=False)
        save(output/'manifest.json', {'policy': 'remove-support-require-base-drop-v1', 'rulesSha256': ai.file_hash(args.rules),
             'sourceSha256': ai.file_hash(args.source), 'gravityFingerprint': gravity.fingerprint(),
             'control': control, 'formations': [{'name': r['name'], 'code': r['code']} for r in accepted],
             'maxFrames': 240, 'stableFrames': 12, 'timeoutSeconds': 120})
        result = gravity.compute(controls, output, argparse.Namespace(max_frames=240, stable_frames=12, timeout=120))
        keep = []
        slot = control['unitSlot'] - int(control['removeSlot'] < control['unitSlot'])
        for row, test in zip(accepted, result['formations']):
            target = test['units'][slot]
            if test['converged'] and target.get('support', {}).get('kind') == 'base' and target['final']['y'] > codec.decode(row['code'])[control['unitSlot']][2]:
                keep.append(row)
            else:
                rejected.append({'name': row['name'], 'code': row['code'], 'reason': '去临时支撑后未落到车板'})
        accepted = keep
        drop_result_hash = ai.file_hash(output/'result.json')
        save(output/'summary.json', {'passed': len(accepted), 'tested': len(controls),
                                     'resultSha256': drop_result_hash})
    save(args.out, accepted)
    save(str(args.out)+'.rejected.json', rejected)
    by_code = {row['code']: i for i, row in enumerate(accepted)}
    aliases = []
    for alias in ai.read_json(Path(args.applied)/'aliases.json'):
        stable_code = applied[alias['outputIndex']]['code']
        aliases.append({**alias, 'appliedOutputIndex': alias['outputIndex'], 'outputIndex': by_code.get(stable_code)})
    save(str(args.out)+'.aliases.json', aliases)
    save(str(args.out)+'.audit.json', {'sourceSha256': ai.file_hash(args.source), 'rulesSha256': ai.file_hash(args.rules),
         'appliedCandidateSha256': ai.file_hash(Path(args.applied)/'candidates.json'),
         'outputSha256': ai.file_hash(args.out), 'count': len(accepted), 'rejected': len(rejected),
         'gravityFingerprint': gravity.fingerprint(),
         'dropControlResultSha256': drop_result_hash})
    print(f'稳定态规则复核：接受{len(accepted)}阵，拒绝{len(rejected)}阵。')


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest='command', required=True)
    g = sub.add_parser('generate')
    g.add_argument('--plan', required=True); g.add_argument('--out', required=True)
    a = sub.add_parser('accept')
    for key in ('source', 'applied', 'rules', 'out'): a.add_argument('--'+key, required=True)
    a.add_argument('--exclude', action='append', default=[])
    args = p.parse_args(argv)
    require(os.environ.get('CONDA_DEFAULT_ENV') == 'opencode', '请先conda activate opencode，或使用formation_workflow.ps1')
    if args.command == 'generate': return generate(args)
    elif args.command == 'accept': return accept(args)


if __name__ == '__main__':
    try: sys.exit(main())
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
        sys.exit(f'生成器操作失败：{error}')
