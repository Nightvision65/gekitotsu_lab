"""生成与稳定态验收共享的显式规则，不携带特定阵的默认槽位。"""
from collections import Counter

import formation_codec as codec


def require(value, message):
    if not value:
        raise ValueError(message)


def keys(value, allowed, label):
    require(isinstance(value, dict), f'{label}须为JSON对象')
    require(not value.keys() - set(allowed), f'{label}含不支持的字段：{sorted(value.keys() - set(allowed))}')


def validate(rules):
    keys(rules, ('profile', 'parentCode', 'budget', 'coreType', 'coordinates', 'preserveIds', 'fixed', 'pairs',
                 'bannedIds', 'allowedIds', 'accelLevel', 'maxAccelLevel', 'maxCounts', 'dropControl'), 'rules')
    require(rules.get('profile') in ('coordinate-micro-v1', 'structural-fixed-speed-v1'), '须显式指定受支持的rules.profile')
    parent = codec.decode(rules['parentCode'])
    require(parent[0][0] == 0 and type(rules.get('coreType', 0)) is int and rules.get('coreType', 0) == 0, '当前搜索规则仅支持普通核心')
    require(rules.get('coordinates', 'kit-workbench') == 'kit-workbench', '坐标须为kit-workbench')
    require(type(rules.get('budget')) is int and rules['budget'] > 0, '须显式指定正整数budget')
    if 'preserveIds' in rules:
        require(type(rules['preserveIds']) is bool, 'preserveIds须为布尔值')
    if rules['profile'] == 'structural-fixed-speed-v1':
        require('accelLevel' in rules, '结构规则须显式指定accelLevel')
    for key in ('accelLevel', 'maxAccelLevel'):
        if key in rules:
            require(type(rules[key]) is int and rules[key] >= 0, f'{key}须为非负整数')
    for key in ('bannedIds', 'allowedIds'):
        if key in rules:
            require(isinstance(rules[key], list) and all(type(i) is int and 1 <= i <= 60 for i in rules[key]), f'{key}须为非核心ID列表')
    counts = rules.get('maxCounts', {})
    require(isinstance(counts, dict) and all(k.isdigit() and 1 <= int(k) <= 60 and type(v) is int and v >= 0 for k, v in counts.items()), 'maxCounts须为ID到非负整数的对象')
    require(isinstance(rules.get('fixed', []), list) and isinstance(rules.get('pairs', []), list), 'fixed/pairs须为列表')
    for fixed in rules.get('fixed', []):
        keys(fixed, ('slot', 'fields', *codec.FIELDS), 'fixed')
        slot = fixed.get('slot')
        require(type(slot) is int and 0 <= slot < len(parent), 'fixed.slot越界')
        fields = fixed.get('fields', [f for f in codec.FIELDS if f in fixed])
        require(isinstance(fields, list) and fields and all(f in codec.FIELDS for f in fields), 'fixed须声明fields或显式字段值')
        require(not ('fields' in fixed and any(f in fixed for f in codec.FIELDS)), 'fixed不能混用fields与显式字段值')
        require(all(type(fixed[f]) is int for f in fields if f in fixed), '固定字段值须为整数')
    for pair in rules.get('pairs', []):
        keys(pair, ('slots', 'fields', 'dx', 'dy', 'signedXGap', 'sameY', 'unitId'), 'pair')
        slots = pair.get('slots')
        require(isinstance(slots, list) and len(slots) == 2 and all(type(s) is int and 0 <= s < len(parent) for s in slots)
                and slots[0] != slots[1], 'pair须指定两个不同的合法slot')
        fields = pair.get('fields', [])
        require(isinstance(fields, list) and all(f in ('x', 'y', 'r') for f in fields), 'pair.fields仅支持x/y/r')
        require(fields or any(f in pair for f in ('dx', 'dy', 'signedXGap')) or pair.get('sameY') is True, 'pair未声明约束')
        require(not ('dx' in pair and 'signedXGap' in pair), 'pair不能混用dx与signedXGap')
        for key in ('dx', 'dy', 'signedXGap', 'unitId'):
            if key in pair:
                require(type(pair[key]) is int, f'pair.{key}须为整数')
        if 'sameY' in pair:
            require(type(pair['sameY']) is bool, 'sameY须为布尔值')
    if 'dropControl' in rules:
        control = rules['dropControl']
        keys(control, ('removeSlot', 'unitSlot'), 'dropControl')
        require(all(type(control.get(k)) is int and 1 <= control[k] < len(parent)
                    for k in ('removeSlot', 'unitSlot')) and control['removeSlot'] != control['unitSlot'],
                'dropControl须指定两个不同的非核心槽位')
    return parent


def check(units, rules):
    try:
        parent = validate(rules)
        codec.encode(units)
        require(units[0][0] == 0, '核心类型改变')
        if rules['profile'] == 'coordinate-micro-v1' or rules.get('preserveIds', False):
            require([u[0] for u in units] == [u[0] for u in parent], '单位类型、数量或顺序改变')
        cost = sum(codec.CATALOG[u[0]]['cost'] for u in units)
        require(cost <= rules['budget'], '超出军资')
        counts = Counter(u[0] for u in units[1:])
        accel = counts[29] + 2 * counts[30]
        if 'accelLevel' in rules:
            require(accel == rules['accelLevel'], '加速等级改变')
        if 'maxAccelLevel' in rules:
            require(accel <= rules['maxAccelLevel'], '超过加速等级上限')
        require(not set(counts).intersection(rules.get('bannedIds', [])), '包含禁用单位')
        if 'allowedIds' in rules:
            require(set(counts) <= set(rules['allowedIds']), '单位不在allowedIds内')
        for uid, limit in rules.get('maxCounts', {}).items():
            require(counts[int(uid)] <= limit, f'ID{uid}超过数量限制')
        for fixed in rules.get('fixed', []):
            slot = fixed['slot']
            require(slot < len(units), f'固定slot{slot}被删除')
            for field in fixed.get('fields', [f for f in codec.FIELDS if f in fixed]):
                expected = fixed.get(field, parent[slot][codec.FIELDS.index(field)])
                require(units[slot][codec.FIELDS.index(field)] == expected, f'固定slot{slot}.{field}改变')
        for pair in rules.get('pairs', []):
            a, b = pair['slots']
            require(max(a, b) < len(units), 'pair槽位被删除')
            if 'unitId' in pair:
                require(units[a][0] == units[b][0] == pair['unitId'], 'pair单位类型改变')
            for field in pair.get('fields', []):
                k = codec.FIELDS.index(field)
                require(units[b][k] - units[a][k] == parent[b][k] - parent[a][k], f'pair.{field}相对关系改变')
            for field, k in (('dx', 1), ('signedXGap', 1), ('dy', 2)):
                if field in pair:
                    require(units[b][k] - units[a][k] == pair[field], f'pair.{field}改变')
            if pair.get('sameY'):
                require(units[a][2] == units[b][2], 'pair.sameY不满足')
        if 'dropControl' in rules:
            control = rules['dropControl']
            for key in ('removeSlot', 'unitSlot'):
                slot = control[key]
                require(slot < len(units) and units[slot][0] == parent[slot][0], 'dropControl槽位被删除或类型改变')
        return None
    except (ValueError, KeyError, TypeError) as error:
        return str(error)
