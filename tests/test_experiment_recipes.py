"""Portable experiment helpers: frozen evidence, physical sides, timeouts and snapshots."""
import argparse
import contextlib
import io
import json
import os
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest
from unittest import mock
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'tools/ai/recipes'))
sys.path.insert(0, str(ROOT/'tools/ai'))
import ai
import formation_codec as codec
from evaluate_pool import score
import experiment
import pool_report
import script_inventory


class RecipeTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(); self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.parent = codec.encode([[0,0,0,0], [7,200,300,280]])
        self.child = codec.encode([[0,0,0,0], [7,200,300,281]])
        self.child2 = codec.encode([[0,0,0,0], [7,200,300,282]])
        a = codec.encode([[0,10,0,0], [55,150,300,0]])
        b = codec.encode([[0,20,100,0], [30,150,300,0]])
        self.opponents = self.root/'opponents.txt'
        self.opponents.write_text('/'.join(['first&'+a, 'duplicate&'+a, 'last&'+b]), encoding='utf-8')

    def save(self, path, value):
        path.write_text(ai.dumps(value), encoding='utf-8')

    def run_fixture(self, name, rows, seed='native-default'):
        folder = self.root/name; folder.mkdir()
        manifest = {'schema': 'gkt-pool-side-v1', 'side': 2, 'maxFrames': 65536,
                    'seedPolicy': seed, 'scoringPolicy': ai.POLICY, 'fingerprint': {'binarySha256': 'fixture'},
                    'opponentInputSha256': ai.file_hash(self.opponents), 'opponentCount': 3,
                    'candidateCount': len(rows)}
        records = []
        for code, statuses in rows:
            outcomes = [[s, 10, 100] for s in statuses]
            records.append({'name': str(len(records)), 'code': code, 'sha256': ai.digest(code.encode()),
                            'side': 2, **score(outcomes, 2), 'outcomes': outcomes})
        self.save(folder/'manifest.json', manifest)
        self.save(folder/'results.json', {'manifest': manifest, 'candidates': records})
        proof = {'passed': True, 'E': 0, 'N': 3, 'side': 2, 'candidates': len(rows),
                 'scoredGames': 3*len(rows), 'scoringPolicy': ai.POLICY,
                 'resultsSha256': ai.file_hash(folder/'results.json')}
        self.save(folder/'gate.json', proof); self.save(folder/'verification.json', proof)
        return folder

    def report_args(self, **changes):
        baseline = self.run_fixture('baseline', [(self.parent, [1,0,-1])])
        current = self.run_fixture('current', [(self.child, [2,1,2])])
        parents = self.root/'parents.json'; self.save(parents, [{'code': self.child, 'parentCode': self.parent}])
        groups = self.root/'groups.json'; self.save(groups, [{'name': 'assault', 'anyIds': [55,56]},
                                                           {'name': 'empty', 'counts': {'9': {'min': 4}}}])
        values = dict(run=current, opponents=self.opponents, out=self.root/'report', reference_run=[baseline],
                      parents=parents, parent_code=None, groups=groups)
        values.update(changes)
        return argparse.Namespace(**values)

    def test_side_two_deltas_keep_duplicate_opponent_weights(self):
        args = self.report_args()
        result = pool_report.report(args)
        row = result['candidates'][0]
        self.assertEqual(row['comparison']['addedPoints'], 1.5)
        self.assertEqual(row['comparison']['lostPoints'], .5)
        self.assertEqual(row['comparison']['netPoints'], 1)
        self.assertEqual(row['comparison']['improvedIndices'], [0,2])
        self.assertEqual(row['groups']['assault']['N'], 2)
        self.assertEqual(row['groups']['assault']['netPoints'], .5)
        self.assertIsNone(row['groups']['empty']['scorePct'])
        self.assertEqual(row['editsFromParent'], [{'slot': 1, 'field': 'r', 'before': 280, 'after': 281}])
        with self.assertRaises(FileExistsError):
            pool_report.report(args)

    def test_report_rejects_stale_verification(self):
        args = self.report_args()
        data = ai.read_json(args.run/'results.json'); data['extra'] = True
        self.save(args.run/'results.json', data)
        with self.assertRaisesRegex(ValueError, '摘要过期'):
            pool_report.report(args)
        self.assertFalse(args.out.exists())

    def test_report_rejects_changed_contract_or_missing_real_parent(self):
        args = self.report_args()
        other = self.run_fixture('other-seed', [(self.parent, [1,0,-1])], seed='different')
        args.reference_run = [other]
        with self.assertRaisesRegex(ValueError, '契约不同'):
            pool_report.report(args)
        args.reference_run = []
        with self.assertRaisesRegex(ValueError, '真实父阵'):
            pool_report.report(args)

    def test_ranking_preserves_exact_ties(self):
        folder = self.run_fixture('ties', [(self.child, [2,1,2]), (self.child2, [2,2,1])])
        args = argparse.Namespace(run=folder, opponents=self.opponents, out=self.root/'ties-report',
                                  reference_run=[], parents=None, parent_code=None, groups=None)
        result = pool_report.report(args)
        self.assertEqual([r['rank'] for r in result['candidates']], [1,1])
        self.assertEqual(len(ai.formations(args.out/'leaders.txt')), 2)

    def test_groups_check_counts_and_reject_unknown_fields(self):
        groups = pool_report.validate_groups([{'name': 'has-wall', 'counts': {'55': {'min': 1}}},
                                               {'name': 'no-wall', 'counts': {'55': {'max': 0}}}])
        meta = pool_report.metadata(ai.formations(self.opponents), groups)
        self.assertEqual(meta[0]['tags'], ['all','has-wall'])
        self.assertEqual(meta[1]['tags'], ['all','has-wall'])
        self.assertEqual(meta[2]['tags'], ['all','no-wall'])
        with self.assertRaises(ValueError):
            pool_report.validate_groups([{'name': 'typo', 'count': {'55': 1}}])

    def test_report_rejects_unknown_status_even_with_fresh_claimed_proof(self):
        folder = self.run_fixture('invalid', [(self.child, [2,1,2])])
        data = ai.read_json(folder/'results.json'); data['candidates'][0]['outcomes'][0][0] = 77
        self.save(folder/'results.json', data)
        for name in ('gate.json', 'verification.json'):
            proof = ai.read_json(folder/name); proof['resultsSha256'] = ai.file_hash(folder/'results.json')
            self.save(folder/name, proof)
        with self.assertRaisesRegex(ValueError, '非法对局'):
            pool_report.read_verified(folder, self.opponents)

    def test_init_freezes_input_bytes_and_does_not_start_search(self):
        source = self.root/'source.json'; self.save(source, [{'name': 'parent', 'code': self.parent}])
        rules = self.root/'rules.json'
        self.save(rules, {'profile': 'coordinate-micro-v1', 'parentCode': self.parent, 'budget': 1500,
                          'fixed': [], 'pairs': []})
        out = self.root/'new-run'
        jar = self.root/'research.jar'
        with zipfile.ZipFile(jar, 'w') as archive:
            archive.writestr('META-INF/MANIFEST.MF', 'Main-Class: org.example.Main\n')
            archive.writestr('org/example/Main.class', b'\x01\x00\x04--ai org/example/ai/AiApi')
            archive.writestr('org/example/ai/AiApi.class', b'fixture')
            archive.writestr('native/gkt_jni.dll', b'fixture-dll')
        probe = {'fingerprint': ai.java_engine(jar)[2], 'result': {'action': 'probe', 'rustAvailable': True}}
        args = argparse.Namespace(source=source, rules=rules, out=out, jar=jar, java=None, probe_timeout=1,
                                  pool=['custom='+str(self.opponents)])
        with mock.patch.object(ai, 'probe_java_engine', return_value=probe) as check:
            experiment.init(args)
        check.assert_called_once_with(jar, None, 1)
        self.assertEqual((out/'inputs/pool-custom.txt').read_bytes(), self.opponents.read_bytes())
        self.assertEqual((out/'inputs/engine.jar').read_bytes(), jar.read_bytes())
        identity = ai.read_json(out/'inputs/identity.json')['engine']
        self.assertEqual(identity['fingerprint'], probe['fingerprint'])
        self.assertEqual(identity['probeSha256'], ai.file_hash(out/'inputs/engine-probe.json'))
        self.assertEqual(ai.read_json(out/'experiment.json')['evaluation']['jarFile'], 'inputs/engine.jar')
        self.assertTrue((out/'STATE.md').exists())
        self.assertFalse((out/'evaluation').exists())
        with self.assertRaises(FileExistsError):
            experiment.init(args)

    def test_step_has_real_timeout_and_records_failure(self):
        out = self.root/'step'
        args = argparse.Namespace(command=[sys.executable, '-c', 'import time; time.sleep(30)'],
                                  timeout=.1, cwd=ROOT, out=out, accept_exit=[0])
        self.assertEqual(experiment.step(args), 124)
        self.assertEqual(ai.read_json(out/'result.json')['status'], 'timeout')
        self.assertGreater(ai.read_json(out/'process.json')['pid'], 0)

    def test_snapshot_checks_external_raw_and_detects_changes(self):
        root = self.root/'archive'; root.mkdir()
        raw = self.root/'external.json'; self.save(raw, {'example': True})
        db = sqlite3.connect(root/'store.sqlite')
        db.execute('CREATE TABLE artifacts(raw_path TEXT,raw_sha256 TEXT)')
        db.execute('INSERT INTO artifacts VALUES(?,?)', (str(raw), ai.file_hash(raw)))
        db.commit(); db.close()
        manifest = self.root/'snapshot.json'
        experiment.snapshot(argparse.Namespace(root=root, out=manifest))
        self.assertEqual(len(ai.read_json(manifest)['externalFiles']), 1)
        args = argparse.Namespace(root=None, manifest=manifest)
        experiment.verify_snapshot(args)
        raw.write_text('changed', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, '外部raw'):
            experiment.verify_snapshot(args)

    def test_snapshot_refuses_live_files_and_detects_added_files(self):
        root = self.root/'static'; root.mkdir()
        (root/'x.tmp').touch()
        with self.assertRaisesRegex(ValueError, 'WAL/SHM'):
            experiment.snapshot(argparse.Namespace(root=root, out=self.root/'bad.json'))
        (root/'x.tmp').unlink()
        (root/'file.txt').write_text('one', encoding='utf-8')
        manifest = self.root/'good.json'
        experiment.snapshot(argparse.Namespace(root=root, out=manifest))
        (root/'extra.txt').touch()
        with self.assertRaisesRegex(ValueError, '新增'):
            experiment.verify_snapshot(argparse.Namespace(root=None, manifest=manifest))

    def test_inventory_is_static_and_finds_duplicated_functions(self):
        folder = self.root/'scripts'; folder.mkdir()
        function = 'def shared(x):\n    value=x+1\n    value*=2\n    return value\n'
        (folder/'one.py').write_text('raise RuntimeError("must not execute")\n'+function, encoding='utf-8')
        (folder/'two.py').write_text('# a different module\n'+function, encoding='utf-8')
        result = script_inventory.inventory([folder])
        self.assertEqual(result['fileCount'], 2)
        self.assertEqual(len(result['duplicateFunctions']), 1)
        self.assertEqual(result['parseErrors'], [])


if __name__=='__main__':
    if os.environ.get('CONDA_DEFAULT_ENV')!='opencode':
        raise SystemExit('请先conda activate opencode')
    with contextlib.redirect_stdout(io.StringIO()):
        unittest.main()
