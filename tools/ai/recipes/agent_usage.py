#!/usr/bin/env python3
"""从明确指定的本地Codex JSONL提取用量快照；不输出会话正文，不推算费用。"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path

FIELDS = ('input_tokens', 'cached_input_tokens', 'output_tokens', 'reasoning_output_tokens', 'total_tokens')


def validate_usage(value):
    if any(type(value.get(k)) is not int or value[k] < 0 for k in FIELDS):
        raise ValueError('token计数字段缺失或非法')
    if value['cached_input_tokens'] > value['input_tokens']:
        raise ValueError('缓存输入超过总输入')
    if value['reasoning_output_tokens'] > value['output_tokens']:
        raise ValueError('推理输出超过总输出')
    if value['total_tokens'] != value['input_tokens']+value['output_tokens']:
        raise ValueError('总token与输入/输出不守恒')


def prefix_hash(path, size):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        remaining = size
        while remaining:
            data = stream.read(min(remaining, 1024*1024))
            if not data:
                raise ValueError('日志已被截短')
            digest.update(data)
            remaining -= len(data)
    return digest.hexdigest()


def read_usage(path):
    path = Path(path).resolve(strict=True)
    size = path.stat().st_size
    session_id = None
    agent_path = None
    usage = None
    event_time = None
    started = None
    completed = None
    ancestors = set()
    inherited_metadata = []
    events = 0
    digest = hashlib.sha256()
    # 限定本次读取的字节边界，容许正在追加的末行尚未写完。
    with path.open('rb') as stream:
        while stream.tell() < size:
            line = stream.readline(size-stream.tell())
            digest.update(line)
            try:
                row = json.loads(line)
            except (json.JSONDecodeError, UnicodeDecodeError):
                if not line.endswith(b'\n'):
                    break
                raise
            payload = row.get('payload', {})
            if row.get('type') == 'session_meta':
                current_id = payload.get('id')
                if session_id is not None and session_id != current_id:
                    if current_id not in ancestors:
                        raise ValueError('日志出现未声明的session ID')
                    inherited_metadata.append(current_id)
                    if payload.get('forked_from_id'):
                        ancestors.add(payload['forked_from_id'])
                    continue
                session_id = current_id
                started = payload.get('timestamp', row.get('timestamp'))
                if payload.get('forked_from_id'):
                    ancestors.add(payload['forked_from_id'])
                source = payload.get('source')
                if isinstance(source, dict):
                    agent_path = source.get('subagent', {}).get('thread_spawn', {}).get('agent_path')
            # fork日志可携带祖先元数据和历史事件，只核算本会话开始之后的事件。
            if started and row.get('timestamp') and datetime.fromisoformat(row['timestamp']) < datetime.fromisoformat(started):
                continue
            if row.get('type') == 'event_msg' and payload.get('type') == 'task_complete':
                completed = row.get('timestamp')
            if row.get('type') != 'event_msg' or payload.get('type') != 'token_count':
                continue
            info = payload.get('info')
            if not info or not info.get('total_token_usage'):
                continue
            current = info['total_token_usage']
            validate_usage(current)
            current = {k: current[k] for k in FIELDS}
            if usage and any(current[k] < usage[k] for k in FIELDS):
                raise ValueError('累计计数回退；不可跨重置合计')
            usage = current
            event_time = row.get('timestamp')
            events += 1
        if stream.tell() != size:
            raise ValueError('读取期间日志已被截短')
    if not session_id or usage is None:
        raise ValueError(f'缺少session ID或有效token_count：{path}')
    return {'sessionId': session_id, 'agentPath': agent_path, 'path': str(path),
            'sessionStartedAtUtc': started, 'lastCompletedAtUtc': completed,
            'inheritedMetadataIds': inherited_metadata,
            'observedBytes': size, 'observedPrefixSha256': digest.hexdigest(), 'tokenCountEvents': events,
            'lastUsageAtUtc': event_time, 'usage': usage}


def totals(rows):
    result = {k: sum(r['usage'][k] for r in rows) for k in FIELDS}
    result['uncached_input_tokens'] = result['input_tokens']-result['cached_input_tokens']
    return result


def snapshot(paths, baseline=None):
    sessions = [read_usage(path) for path in paths]
    ids = [r['sessionId'] for r in sessions]
    if len(ids) != len(set(ids)):
        raise ValueError('同一session不得重复计入')
    result = {'schema': 'gkt-agent-usage-v1',
              'createdAtUtc': datetime.now(timezone.utc).isoformat(),
              'sessions': sessions, 'totals': totals(sessions),
              'scope': '本地会话累计计数；缓存输入属于总输入，推理输出属于输出；不是费用、上下文长度或单代理对照。末次用量事件之后的工作尚未计入。'}
    if baseline is not None:
        if baseline.get('schema') != result['schema']:
            raise ValueError('baseline schema不符')
        old = {r['sessionId']: r for r in baseline['sessions']}
        if set(old) != set(ids):
            raise ValueError('前后快照的session集合必须相同；新增代理须另列')
        deltas = []
        for row in sessions:
            before = old[row['sessionId']]
            validate_usage(before['usage'])
            if row['observedBytes'] < before['observedBytes']:
                raise ValueError('日志被截短')
            if not before.get('observedPrefixSha256'):
                raise ValueError('baseline缺少日志前缀哈希')
            if prefix_hash(row['path'], before['observedBytes']) != before['observedPrefixSha256']:
                raise ValueError('日志旧边界之前的内容已被改写')
            delta = {k: row['usage'][k]-before['usage'][k] for k in FIELDS}
            validate_usage(delta)
            deltas.append({'sessionId': row['sessionId'], 'agentPath': row['agentPath'], 'usage': delta})
        result['delta'] = {'sinceUtc': baseline['createdAtUtc'], 'sessions': deltas, 'totals': totals(deltas)}
    return result


def main(argv=None):
    if os.environ.get('CONDA_DEFAULT_ENV') != 'opencode':
        raise ValueError('请先conda activate opencode')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--session', action='append', required=True, help='只读明确指定的JSONL文件，可重复')
    parser.add_argument('--baseline', help='同一组session的前次快照')
    parser.add_argument('--out', required=True)
    args = parser.parse_args(argv)
    out = Path(args.out)
    if out.exists():
        raise FileExistsError(f'禁止覆盖：{out}')
    baseline = json.loads(Path(args.baseline).read_text(encoding='utf-8')) if args.baseline else None
    result = snapshot(args.session, baseline)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open('x', encoding='utf-8') as stream:
        json.dump(result, stream, ensure_ascii=False, indent=2)
        stream.write('\n')
    print(json.dumps(result.get('delta', result)['totals'], ensure_ascii=False))


if __name__ == '__main__':
    main()
