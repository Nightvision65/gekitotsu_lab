"""只读核验随仓库分发的固定对手集；不依赖本地导入现场。"""
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/ai'))
from kit_lab import read_forts, ALPHABET


def verify():
    folder = Path(__file__).resolve().parent
    manifest = json.loads((folder / 'manifest.json').read_text(encoding='utf-8'))
    sha = hashlib.sha256((folder / 'opponents.txt').read_bytes()).hexdigest()
    assert sha == manifest['normalizedFileSha256'], '对手集SHA256不匹配'
    forts = read_forts(folder / 'opponents.txt', True)
    hashes = [f['sha256'] for f in forts]
    assert len(forts) == manifest['actualEntryCount'] == 10433
    assert len(set(hashes)) == manifest['pureCodeUniqueCount'] == len(forts)
    standard_path = ROOT / 'benchmarks/standard200/opponents.txt'
    assert hashlib.sha256(standard_path.read_bytes()).hexdigest() == manifest['standard200Sha256']
    standard = read_forts(standard_path, True)
    standard_hashes = {f['sha256'] for f in standard}
    overlap = [i for i, f in enumerate(forts) if f['sha256'] in standard_hashes]
    partitions = json.loads((folder / 'evaluation-partitions.json').read_text(encoding='utf-8'))
    assert overlap == partitions['overlapExternalIndices']
    overlap_set = set(overlap)
    assert [i for i in range(len(forts)) if i not in overlap_set] == partitions['nonOverlapExternalIndices']
    pairs = json.loads((folder / 'standard200-overlap.json').read_text(encoding='utf-8'))
    assert sorted(p['externalIndex'] for p in pairs) == overlap
    assert json.loads((folder / 'duplicates.json').read_text(encoding='utf-8')) == []
    catalog = {u['id']: u for u in json.loads((ROOT / 'docs/ai/data/unit-catalog.json').read_text(encoding='utf-8'))['units']}
    costs, cores, counts = [], Counter(), []
    for f in forts:
        total = 0
        for slot in range(len(f['code']) // 6):
            uid = {'0': 0, '1': 61, '2': 62}[f['code'][0]] if slot == 0 else ALPHABET.index(f['code'][slot * 6])
            total += catalog[uid]['cost']
            if slot == 0:
                cores[str(uid)] += 1
        assert total <= 1500
        costs.append(total)
        counts.append(len(f['code']) // 6)
    statistics = json.loads((folder / 'statistics.json').read_text(encoding='utf-8'))
    assert dict(cores) == statistics['coreCounts']
    assert sum(c == 1500 for c in costs) == statistics['exact1500Count']
    assert [min(counts), max(counts)] == statistics['unitCountIncludingCoreRange']
    assert hashlib.sha256((ROOT / 'docs/ai/data/unit-catalog.json').read_bytes()).hexdigest() == statistics['unitCatalogSha256']
    return {'verified': True, 'entries': len(forts), 'overlap': len(overlap), 'sha256': sha}


if __name__ == '__main__':
    if os.environ.get('CONDA_DEFAULT_ENV') != 'opencode':
        sys.exit('请先conda activate opencode')
    print(json.dumps(verify(), ensure_ascii=False))
