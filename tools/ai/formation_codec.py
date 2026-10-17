"""通用工作台阵码编解码；军资和战术限制由本轮rules审计。"""
import ai

CATALOG_PATH = ai.ROOT / 'docs/ai/data/unit-catalog.json'
CATALOG = {u['id']: u for u in ai.read_json(CATALOG_PATH)['units']}
FIELDS = ('id', 'x', 'y', 'r')


def decode(code):
    ai.validate_code(code)
    units = []
    for slot in range(len(code) // 6):
        chunk = code[slot * 6:slot * 6 + 6]
        number = 0
        for digit in chunk[1:]:
            number = number * 61 + ai.ALPHABET.index(digit)
        uid = {'0': 0, '1': 61, '2': 62}[chunk[0]] if slot == 0 else ai.ALPHABET.index(chunk[0])
        units.append([uid, number // 1000 % 1000 - (52 if slot == 0 else 16),
                      number % 1000 - (58 if slot == 0 else 20), number // 1000000])
    return units


def encode(units):
    if not isinstance(units, list) or not 1 <= len(units) <= 512:
        raise ValueError('须为1至512槽位，核心在slot0')
    blocks = []
    for slot, unit in enumerate(units):
        if not isinstance(unit, list) or len(unit) != 4 or any(type(v) is not int for v in unit):
            raise ValueError(f'slot{slot}须为整数[id,x,y,r]')
        uid, x, y, r = unit
        if not (uid in (0, 61, 62) if slot == 0 else 1 <= uid <= 60):
            raise ValueError(f'slot{slot}类型非法')
        if not (0 <= x <= (276 if slot == 0 else 348) and 0 <= y <= (276 if slot == 0 else 349) and 0 <= r < 360):
            raise ValueError(f'slot{slot}坐标或角度越界')
        number = r * 1000000 + (x + (52 if slot == 0 else 16)) * 1000 + y + (58 if slot == 0 else 20)
        digits = ''
        for _ in range(5):
            number, digit = divmod(number, 61)
            digits = ai.ALPHABET[digit] + digits
        if number:
            raise ValueError('base61编码溢出')
        blocks.append(({0: '0', 61: '1', 62: '2'}[uid] if slot == 0 else ai.ALPHABET[uid]) + digits)
    result = ''.join(blocks)
    if decode(result) != units:
        raise ValueError('阵码回编码不一致')
    return result


def changes(before, after):
    rows = []
    for slot in range(max(len(before), len(after))):
        a = before[slot] if slot < len(before) else [None] * 4
        b = after[slot] if slot < len(after) else [None] * 4
        rows.extend({'slot': slot, 'field': field, 'before': a[k], 'after': b[k]}
                    for k, field in enumerate(FIELDS) if a[k] != b[k])
    return rows


def timing(before, after):
    rows = []
    for slot, unit in enumerate(after[1:], 1):
        if slot >= len(before) or unit[0] != before[slot][0] or unit[1] == before[slot][1]:
            continue
        row = {'slot': slot, 'unitId': unit[0], 'note': '有效递增次数，不能当成实际命中帧；x也改变几何和支撑'}
        spec = {1: (30, 36), 7: (30, 36), 9: (120, 120), 10: (20, 20)}.get(unit[0])
        if spec:
            modulus, trigger = spec
            row.update(initializationModulus=modulus, attackTriggerCnt=trigger)
            for key, x in (('before', before[slot][1]), ('after', unit[1])):
                cnt = -((x + 16) % modulus)
                row[key] = {'x': x, 'initialCnt': cnt, 'effectiveIncrementsToFirstTrigger': trigger - cnt}
        rows.append(row)
    return rows
