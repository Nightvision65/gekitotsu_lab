"""Native stable geometry deduplication, provenance and representation boundaries."""
import argparse
import contextlib
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'tools/ai/recipes'))
sys.path.insert(0, str(ROOT/'tools/ai'))
import ai
import formation_codec as codec
import stable_dedup


def code(*units):
    return codec.encode([list(u) for u in units])


class StableDedupTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name); self.count = 0

    def run_rows(self, rows, **options):
        self.count += 1
        source = self.root/f'source-{self.count}.json'
        source.write_text(ai.dumps(rows), encoding='utf-8')
        values = dict(input=[str(source)], out=str(self.root/f'run-{self.count}'), exclude_run=[],
                      max_frames=240, stable_frames=2, batch_size=2000, timeout=60)
        values.update(options)
        args = argparse.Namespace(**values)
        with contextlib.redirect_stdout(io.StringIO()):
            status = stable_dedup.run(args)
        return status, Path(args.out)

    def test_pass_and_fail_share_one_negative_y_layout_and_keep_legal_source(self):
        high = code((0,0,0,0),(25,200,0,0),(7,200,0,280))
        pushed = code((0,0,0,0),(25,200,0,0),(7,200,1,280))
        status, out = self.run_rows([{'name':'先未通过','code':pushed,'scorePct':99},
                                     {'name':'后通过','code':high,'W':999}])
        self.assertEqual(status, 0)
        summary = ai.read_json(out/'summary.json')
        self.assertEqual((summary['sourceAuditPassed'],summary['sourceAuditFailed'],summary['uniqueStableLayouts']), (1,1,1))
        classes = ai.read_json(out/'classes.json')
        self.assertEqual(classes[0]['units'][2]['y'], -33)
        self.assertEqual(classes[0]['sourceIndices'], [0,1])
        row = ai.read_json(out/'candidates.json')[0]
        self.assertEqual(row['code'], high)
        self.assertEqual(row['sourceIndex'], 1)
        self.assertEqual(row['representation'], 'source')
        self.assertTrue(row['requiresSearchAcceptance'])
        self.assertNotIn('W', row); self.assertNotIn('scorePct', row)
        self.assertEqual([a['outputIndex'] for a in ai.read_json(out/'aliases.json')], [0,0])
        stable_dedup.verify(out)

    def test_two_falls_merge_and_new_code_is_replayed_idempotently(self):
        a = code((0,0,0,0),(7,250,0,280))
        b = code((0,0,0,0),(7,250,100,280))
        status, out = self.run_rows([{'name':'first','code':a,'parentCode':a,'sha256':'old'}, {'code':b}])
        self.assertEqual(status, 0)
        row = ai.read_json(out/'candidates.json')[0]
        self.assertNotEqual(row['code'], a)
        self.assertEqual(row['representation'], 'stable')
        self.assertEqual(row['parentCode'], a)
        self.assertTrue(row['changes'])
        result = ai.read_json(out/'result-representative-00000.json')['formations'][0]
        self.assertTrue(result['converged'])
        self.assertEqual(result['changedUnits'], 0)
        stable_dedup.verify(out)

    def test_identical_codes_across_files_simulate_once_and_preserve_all_aliases(self):
        a = code((0,0,0,0),(7,250,0,280))
        second = self.root/'extra.json'
        second.write_text(ai.dumps([{'name':'second-file','code':a}]), encoding='utf-8')
        source = self.root/'first.json'
        source.write_text(ai.dumps([{'name':'one','code':a},{'name':'alias','code':a}]), encoding='utf-8')
        status, out = self.run_rows([{'code':a}], input=[str(source),str(second)], batch_size=1)
        self.assertEqual(status, 0)
        summary = ai.read_json(out/'summary.json')
        self.assertEqual((summary['inputCount'],summary['uniqueSourceCodes'],summary['uniqueStableLayouts']), (3,1,1))
        self.assertEqual([a['sourceFileIndex'] for a in ai.read_json(out/'aliases.json')], [0,0,1])

    def test_unit_order_and_angle_remain_part_of_geometry(self):
        a = code((0,0,0,0),(7,250,0,280),(9,250,0,338))
        b = code((0,0,0,0),(9,250,0,338),(7,250,0,280))
        c = code((0,0,0,0),(7,250,0,281),(9,250,0,338))
        status, out = self.run_rows([{'code':x} for x in (a,b,c)])
        self.assertEqual(status, 0)
        self.assertEqual(ai.read_json(out/'summary.json')['uniqueStableLayouts'], 3)

    def test_distinct_one_pixel_stable_layouts_are_not_merged(self):
        a = code((0,0,0,0),(25,200,100,0),(7,200,100,280))
        b = code((0,0,0,0),(25,200,101,0),(7,200,101,280))
        status, out = self.run_rows([{'code':a},{'code':b}])
        self.assertEqual(status, 0)
        groups = ai.read_json(out/'classes.json')
        self.assertEqual(len(groups), 2)
        self.assertEqual(groups[1]['units'][2]['y']-groups[0]['units'][2]['y'], 1)

    def test_nonconverged_rows_remain_pending_without_fake_stable_classes(self):
        a = code((0,0,0,0),(7,250,0,280))
        status, out = self.run_rows([{'code':a}], max_frames=2)
        self.assertEqual(status, 3)
        self.assertEqual(ai.read_json(out/'classes.json'), [])
        self.assertEqual(ai.read_json(out/'pending.json')[0]['reason'], 'non-converged')
        stable_dedup.verify(out)

    def test_unencodable_failed_layout_keeps_legal_source_for_task_acceptance(self):
        a = code((0,0,0,0),(25,200,0,0),(7,200,1,280))
        status, out = self.run_rows([{'code':a}])
        self.assertEqual(status, 0)
        candidate = ai.read_json(out/'candidates.json')[0]
        self.assertEqual(candidate['code'], a)
        self.assertEqual(candidate['representation'], 'source')
        self.assertFalse(candidate['sourceAuditPassed'])
        self.assertTrue(candidate['requiresSearchAcceptance'])
        group = ai.read_json(out/'classes.json')[0]
        self.assertLess(group['units'][2]['y'], 0)
        self.assertEqual(group['representative']['code'], a)
        self.assertEqual(ai.read_json(out/'pending.json'), [])
        stable_dedup.verify(out)

    def test_explicit_previous_runs_exclude_duplicate_geometry(self):
        a = code((0,0,0,0),(7,250,0,280))
        b = code((0,0,0,0),(7,250,100,280))
        _, previous = self.run_rows([{'code':a}])
        status, out = self.run_rows([{'code':b}], exclude_run=[str(previous)])
        self.assertEqual(status, 3)
        self.assertEqual(ai.read_json(out/'summary.json')['excludedLayouts'], 1)
        self.assertEqual(ai.read_json(out/'classes.json')[0]['excludedBy'][0]['run'], str(previous))
        stable_dedup.verify(out)

    def test_previous_convergence_window_mismatch_is_rejected(self):
        a = code((0,0,0,0),(7,250,0,280))
        _, previous = self.run_rows([{'code':a}])
        with self.assertRaisesRegex(ValueError, '收敛窗口不同'):
            self.run_rows([{'code':a}], exclude_run=[str(previous)], stable_frames=3)

    def test_tampered_evidence_and_overwrite_are_rejected(self):
        a = code((0,0,0,0),(7,250,0,280))
        _, out = self.run_rows([{'code':a}])
        (out/'classes.json').write_text('[]', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, '哈希不同'):
            stable_dedup.verify(out)
        with self.assertRaises(FileExistsError):
            self.run_rows([{'code':a}], out=str(out))

    def test_tick_uses_engine_grid_and_refuses_coarse_approximation(self):
        self.assertEqual(stable_dedup.tick(.05-.00001), 1)
        self.assertEqual(stable_dedup.tick(.05+.00001), 1)
        self.assertEqual(stable_dedup.tick(1.05)-stable_dedup.tick(.05), 20)
        with self.assertRaisesRegex(ValueError, '量子格点'):
            stable_dedup.tick(.02)

    def test_dedup_only_starts_static_tool_never_battle_evaluation(self):
        import gravity
        a = code((0,0,0,0),(7,250,0,280))
        with mock.patch.object(gravity, 'checked', wraps=gravity.checked) as checked:
            self.run_rows([{'code':a}])
        self.assertTrue(checked.call_args_list)
        self.assertTrue(all(call.args[0][0]==str(gravity.BINARY) for call in checked.call_args_list))


if __name__=='__main__':
    if os.environ.get('CONDA_DEFAULT_ENV')!='opencode':
        raise SystemExit('请先conda activate opencode')
    unittest.main()
