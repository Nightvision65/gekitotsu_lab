"""用量核算须隔离正文、避免重复、识别计数重置及活跃日志末行。"""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'tools/ai/recipes'))
import agent_usage


class UsageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name)/'session.jsonl'
        self.write({'type': 'session_meta', 'payload': {'id': 'test', 'instructions': 'PRIVATE'}},
                   {'type': 'response_item', 'payload': {'text': 'PRIVATE'}})

    def write(self, *rows):
        with self.path.open('a', encoding='utf-8') as stream:
            for row in rows:
                stream.write(json.dumps(row)+'\n')

    def count(self, n=100, cached=60, output=10, reasoning=2):
        self.write({'type': 'event_msg', 'timestamp': '2026-10-20T00:00:00Z',
                    'payload': {'type': 'token_count', 'info': {'total_token_usage': {
                        'input_tokens': n, 'cached_input_tokens': cached, 'output_tokens': output,
                        'reasoning_output_tokens': reasoning, 'total_tokens': n+output}}}})

    def test_delta_does_not_double_count_cache_or_reasoning(self):
        self.count()
        before = agent_usage.snapshot([self.path])
        self.count(140, 90, 15)
        after = agent_usage.snapshot([self.path], before)
        delta = after['delta']['totals']
        self.assertEqual(delta['input_tokens'], 40)
        self.assertEqual(delta['uncached_input_tokens'], 10)
        self.assertEqual(delta['output_tokens'], 5)
        self.assertEqual(delta['total_tokens'], 45)
        self.assertNotIn('PRIVATE', json.dumps(after))

    def test_duplicate_sessions_fail(self):
        self.count()
        with self.assertRaisesRegex(ValueError, '重复'):
            agent_usage.snapshot([self.path, self.path])

    def test_reset_and_missing_usage_fail(self):
        with self.assertRaisesRegex(ValueError, '缺少'):
            agent_usage.read_usage(self.path)
        self.count()
        self.count(99)
        with self.assertRaisesRegex(ValueError, '回退'):
            agent_usage.read_usage(self.path)

    def test_live_tail_is_not_a_published_event(self):
        self.count()
        with self.path.open('ab') as stream:
            stream.write(b'{"type": "event_msg",')
        self.assertEqual(agent_usage.read_usage(self.path)['usage']['input_tokens'], 100)

    def test_valid_final_line_without_newline_is_counted(self):
        self.count()
        self.path.write_bytes(self.path.read_bytes().rstrip(b'\n'))
        self.assertEqual(agent_usage.read_usage(self.path)['usage']['input_tokens'], 100)

    def test_broken_published_line_is_not_silently_skipped(self):
        self.count()
        with self.path.open('ab') as stream:
            stream.write(b'{"type":\n')
        with self.assertRaises(json.JSONDecodeError):
            agent_usage.read_usage(self.path)

    def test_changed_session_set_fails(self):
        self.count()
        before = agent_usage.snapshot([self.path])
        before['sessions'][0]['sessionId'] = 'another'
        with self.assertRaisesRegex(ValueError, '集合'):
            agent_usage.snapshot([self.path], before)

    def test_fork_keeps_child_identity_and_excludes_historical_usage(self):
        self.path.unlink()
        self.write({'type': 'session_meta', 'payload': {'id': 'child', 'forked_from_id': 'parent',
                    'timestamp': '2026-10-20T00:00:00Z'}},
                   {'type': 'session_meta', 'payload': {'id': 'parent', 'timestamp': '2026-10-19T00:00:00Z'}})
        self.write({'type': 'event_msg', 'timestamp': '2026-10-19T00:00:00Z',
                    'payload': {'type': 'token_count', 'info': {'total_token_usage': {
                        'input_tokens': 9999, 'cached_input_tokens': 0, 'output_tokens': 10,
                        'reasoning_output_tokens': 0, 'total_tokens': 10009}}}})
        self.count()
        result = agent_usage.read_usage(self.path)
        self.assertEqual(result['sessionId'], 'child')
        self.assertEqual(result['usage']['input_tokens'], 100)
        self.assertEqual(result['inheritedMetadataIds'], ['parent'])

    def test_unrelated_metadata_rejected(self):
        self.write({'type': 'session_meta', 'payload': {'id': 'unrelated'}})
        self.count()
        with self.assertRaisesRegex(ValueError, '未声明'):
            agent_usage.read_usage(self.path)

    def test_same_length_rewrite_rejected(self):
        self.count()
        before = agent_usage.snapshot([self.path])
        self.path.write_bytes(self.path.read_bytes().replace(b'PRIVATE', b'CHANGED'))
        with self.assertRaisesRegex(ValueError, '改写'):
            agent_usage.snapshot([self.path], before)

    def test_counter_conservation_and_delta_reasoning(self):
        invalid = dict(input_tokens=100, cached_input_tokens=0, output_tokens=10,
                       reasoning_output_tokens=0, total_tokens=111)
        with self.assertRaisesRegex(ValueError, '不守恒'):
            agent_usage.validate_usage(invalid)
        invalid.update(total_tokens=110, reasoning_output_tokens=11)
        with self.assertRaisesRegex(ValueError, '推理输出'):
            agent_usage.validate_usage(invalid)

    def test_delta_reasoning_cannot_exceed_incremental_output(self):
        self.count()
        before = agent_usage.snapshot([self.path])
        self.count(140, 90, 11, reasoning=5)
        with self.assertRaisesRegex(ValueError, '推理输出'):
            agent_usage.snapshot([self.path], before)


if __name__ == '__main__':
    unittest.main()
