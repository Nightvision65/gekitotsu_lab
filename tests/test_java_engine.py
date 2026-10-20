"""研究JAR入口、冻结身份及JNI预检；不运行对战或GUI。"""
import argparse
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'tools/ai'))
sys.path.insert(0, str(ROOT/'tools/ai/recipes'))
import ai
import evaluate_pool
import experiment


class JavaEngineTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(); self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)

    def jar(self, *, api=True, dispatch=True):
        path = self.root/'engine.jar'
        with zipfile.ZipFile(path, 'w') as archive:
            archive.writestr('META-INF/MANIFEST.MF', 'Manifest-Version: 1.0\r\nMain-Class: org.example.Main\r\n')
            archive.writestr('org/example/Main.class', b'\x01\x00\x04--ai org/example/ai/AiApi' if dispatch else b'GUI only')
            if api: archive.writestr('org/example/ai/AiApi.class', b'fixture')
            archive.writestr('native/gkt_jni.dll', b'fixture-dll')
        return path

    def test_missing_api_or_main_dispatch_is_rejected_without_starting_java(self):
        with mock.patch.object(ai.subprocess, 'run', side_effect=AssertionError('不能启动GUI')):
            for flags in ({'api': False}, {'dispatch': False}):
                with self.subTest(flags=flags), self.assertRaisesRegex(ValueError, '不支持研究--ai'):
                    ai.java_engine(self.jar(**flags))

    def test_wrong_jar_creates_neither_experiment_nor_pool_state(self):
        jar = self.jar(api=False)
        source, opponents, rules = [self.root/name for name in ('source.json', 'opponents.txt', 'rules.json')]
        source.write_text(ai.dumps([{'code': '000iyg'}]), encoding='utf-8')
        opponents.write_text('000iyh', encoding='utf-8')
        rules.write_text(ai.dumps({'profile': 'coordinate-micro-v1', 'parentCode': '000iyg', 'budget': 1500,
                                   'fixed': [], 'pairs': []}), encoding='utf-8')
        with mock.patch.object(ai.subprocess, 'run', side_effect=AssertionError('不能启动GUI')):
            with self.assertRaisesRegex(ValueError, '不支持研究--ai'):
                experiment.init(argparse.Namespace(source=source, rules=rules, jar=jar, java=None, probe_timeout=1,
                                pool=['custom='+str(opponents)], out=self.root/'experiment'))
            with self.assertRaisesRegex(ValueError, '不支持研究--ai'):
                evaluate_pool.run(argparse.Namespace(candidates=source, opponents=opponents, jar=jar,
                                  store=self.root/'store.sqlite', out=self.root/'evaluation', side=1, max_frames=65536))
        self.assertFalse((self.root/'experiment').exists())
        self.assertFalse((self.root/'evaluation').exists())
        self.assertFalse((self.root/'store.sqlite').exists())

    def test_runtime_probe_checks_real_request_shape_and_native_availability(self):
        jar = self.jar()
        available = True
        def java(command, **kwargs):
            self.assertEqual(command[-4:-2], [str(jar), '--ai'])
            self.assertEqual(ai.read_json(command[-2]), {'action': 'probe'})
            self.assertEqual(kwargs['timeout'], 2)
            self.assertTrue(any(v.startswith('-Dgekitotsu.native.path=') for v in command))
            Path(command[-1]).write_text(ai.dumps({'schemaVersion': 'gkt-ai-analysis-v1', 'action': 'probe',
                                        'rustAvailable': available, 'rustReason': 'fixture'}), encoding='utf-8')
            return subprocess.CompletedProcess(command, 0, b'', b'')
        with mock.patch.object(ai.subprocess, 'run', side_effect=java):
            proof = ai.probe_java_engine(jar, java='fixture-java', timeout=2)
            self.assertEqual(proof['fingerprint'], evaluate_pool.fingerprint(argparse.Namespace(jar=jar)))
            available = False
            with self.assertRaisesRegex(ValueError, 'JNI不可用'):
                ai.probe_java_engine(jar, java='fixture-java', timeout=2)

    def test_engine_change_is_rejected_before_cache_lookup_or_execution(self):
        jar = self.jar()
        expected = ai.java_engine(jar)[2]
        with zipfile.ZipFile(jar, 'a') as archive:
            archive.writestr('changed.txt', 'new build')
        with self.assertRaisesRegex(ValueError, '与冻结指纹不同'):
            ai.compute(None, self.root/'unused.sqlite', {'action': 'battle'}, argparse.Namespace(jar=jar),
                       'java', expected_fingerprint=expected)
        self.assertFalse((self.root/'unused.sqlite').exists())

    def test_overwritten_frozen_jar_is_rejected_against_initialization_identity(self):
        jar = self.jar()
        fingerprint = ai.java_engine(jar)[2]
        (self.root/'identity.json').write_text(ai.dumps({'schema': 'gkt-experiment-inputs-v1',
                                    'engine': {'fingerprint': fingerprint}}), encoding='utf-8')
        self.assertEqual(ai.java_engine(jar)[2], fingerprint)
        with zipfile.ZipFile(jar, 'a') as archive:
            archive.writestr('changed.txt', 'another valid research build')
        with self.assertRaisesRegex(ValueError, '与建档冻结身份不同'):
            evaluate_pool.fingerprint(argparse.Namespace(jar=jar))


if __name__=='__main__':
    unittest.main()
