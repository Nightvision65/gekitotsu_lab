#!/usr/bin/env python3
"""独立复核完整阵池的输入、侧位、逐局来源和汇总，不启动模拟。"""
import argparse
from contextlib import closing
import json
from pathlib import Path

import ai
from evaluate_pool import score


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(args):
    folder = Path(args.run)
    manifest = ai.read_json(folder/'manifest.json')
    gate = ai.read_json(folder/'gate.json')
    data = ai.read_json(folder/'results.json')
    require(gate.get('passed') and gate['resultsSha256'] == ai.file_hash(folder/'results.json'), '汇总哈希或门禁不符')
    require(manifest == data['manifest'] and manifest['scoringPolicy'] == ai.POLICY, '冻结清单或计分政策不符')
    require(ai.file_hash(args.candidates) == manifest['candidateInputSha256'], '候选输入哈希改变')
    require(ai.file_hash(args.opponents) == manifest['opponentInputSha256'], '对手输入哈希改变')
    source = Path(args.candidates)
    own = ai.read_json(source) if source.suffix == '.json' else ai.formations(source)
    if isinstance(own, dict):
        own = own['candidates']
    opponents = ai.formations(args.opponents)
    candidates = data['candidates']
    require(len(own) == len(candidates) == manifest['candidateCount'] == gate['candidates'], '候选数量不符')
    require(len(opponents) == manifest['opponentCount'] == gate['N'] and gate['E'] == 0, '阵池数量或异常数不符')
    require([c['code'] for c in candidates] == [c['code'] for c in own], '候选顺序或纯码改变')
    side = manifest['side']
    require(side in (1, 2), '非法侧位')
    artifacts = {}
    with closing(ai.connect(args.store or data['store'])) as db:
        for candidate in candidates:
            require(candidate['side'] == side and candidate['sha256'] == ai.digest(candidate['code'].encode()), '候选身份或侧位不符')
            outcomes = candidate['outcomes']
            require(len(outcomes) == len(candidate['sourceByOpponent']) == len(opponents), '逐局结果或来源缺失')
            require(all(candidate[k] == v for k, v in score(outcomes, side).items()), '计分与逐局结果不同')
            for index, (outcome, proof) in enumerate(zip(outcomes, candidate['sourceByOpponent'])):
                source_index, matrix, row_index = proof
                require(type(source_index) is int and 0 <= source_index < len(candidate['sources']), '来源索引非法')
                source = candidate['sources'][source_index]
                artifact = source['artifact']
                if artifact not in artifacts:
                    row = ai.verified(db, artifact)
                    request = json.loads(row['request'])
                    require(json.loads(row['fingerprint']) == manifest['fingerprint'], '来源引擎指纹不同')
                    require(request.get('action') == 'battle' and request.get('maxFrames') == manifest['maxFrames'] and request.get('seed') is None, '来源帧数、动作或seed不同')
                    artifacts[artifact] = (ai.read_json(row['raw_path']), row['raw_sha256'])
                raw, raw_sha = artifacts[artifact]
                require(raw_sha == source['rawSha256'], '来源原始文件哈希不同')
                require(matrix in ('forward', 'reverse') and matrix in raw, '物理矩阵来源不存在')
                require(type(row_index) is int and 0 <= row_index < len(raw[matrix]), '矩阵行索引非法')
                row = raw[matrix][row_index]
                p1, p2 = (raw['p1'], raw['p2']) if matrix == 'forward' else (raw['p2'], raw['p1'])
                a, b = row['p1Index'], row['p2Index']
                require(0 <= a < len(p1) and 0 <= b < len(p2), '物理侧位索引越界')
                own_fort, opp_fort = (p1[a], p2[b]) if side == 1 else (p2[b], p1[a])
                require(own_fort['code'] == candidate['code'] and opp_fort['code'] == opponents[index]['code'], '逐局候选、对手顺序或物理侧位不同')
                require([row[k] for k in ('status', 'winnerHp', 'frames')] == outcome, '逐局结果与原始矩阵不同')
    result = {'passed': True, 'side': side, 'candidates': len(candidates), 'N': len(opponents),
              'scoredGames': len(candidates)*len(opponents), 'rawArtifactsVerified': len(artifacts),
              'E': 0, 'resultsSha256': gate['resultsSha256'], 'scoringPolicy': ai.POLICY}
    (folder/'verification.json').write_text(ai.dumps(result), encoding='utf-8')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('run', 'candidates', 'opponents'):
        parser.add_argument('--'+name, required=True)
    parser.add_argument('--store')
    print(ai.dumps(verify(parser.parse_args())))
