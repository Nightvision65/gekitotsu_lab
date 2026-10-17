"""按问题读取单局详细事件；输出有界分页与字段投影，不打印完整原件。"""
import argparse
import gzip
import json
import os
from pathlib import Path


def project(row, fields):
    out = {}
    for field in fields:
        value = row
        for key in field.split('.'):
            value = value.get(key) if isinstance(value, dict) else None
        out[field] = value
    return out


def main():
    if os.environ.get('CONDA_DEFAULT_ENV') != 'opencode':
        raise SystemExit('请先激活conda opencode')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('path', type=Path)
    parser.add_argument('--from-frame', type=int, default=0)
    parser.add_argument('--to-frame', type=int, default=1000000)
    parser.add_argument('--side', type=int, choices=(0, 1))
    parser.add_argument('--slots', type=int, nargs='+')
    parser.add_argument('--kinds', nargs='+')
    parser.add_argument('--after-sequence', type=int, default=-1)
    parser.add_argument('--limit', type=int, default=20, choices=range(1, 101), metavar='1..100')
    parser.add_argument('--fields', nargs='+', default=['sequence', 'frame', 'kind', 'side', 'slot', 'attackerEntityId', 'effectiveHpLoss', 'hpAfter', 'x', 'y'])
    args = parser.parse_args()
    if args.from_frame > args.to_frame:
        parser.error('帧区间倒置')
    opener = gzip.open if args.path.suffix == '.gz' else open
    with opener(args.path, 'rt', encoding='utf-8') as stream:
        data = json.load(stream)
    matches = (e for e in data['combatEvents']
               if args.from_frame <= e['frame'] <= args.to_frame
               and e['sequence'] > args.after_sequence
               and (args.side is None or e['side'] == args.side)
               and (args.slots is None or e['slot'] in args.slots)
               and (args.kinds is None or e['kind'] in args.kinds))
    rows = []
    for event in matches:
        rows.append(event)
        if len(rows) > args.limit:
            break
    more = len(rows) > args.limit
    rows = rows[:args.limit]
    print(json.dumps(dict(result=data['result'], returned=len(rows), hasMore=more,
                          nextAfterSequence=rows[-1]['sequence'] if more else None,
                          events=[project(e, args.fields) for e in rows]), ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
