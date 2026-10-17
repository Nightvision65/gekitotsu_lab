"""生成器契约：边界、约束、结构预算、计划规模与稳定态来源。"""
import argparse
import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools/ai'))
import ai
import formation_workflow as workflow


class GeneratorContractTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.units = [[0, 0, 0, 0], [7, 42, 100, 290], [7, 52, 100, 290]]
        self.parent = workflow.codec.encode(self.units)
        self.rules = {'profile': 'coordinate-micro-v1', 'parentCode': self.parent,
                      'budget': 1500, 'preserveIds': True}
        self.source = self.root / 'parent.json'
        self.source.write_text(ai.dumps([{'name': '父阵', 'code': self.parent}]), encoding='utf-8')

    def generate(self, groups, rules=None, **extra):
        plan = {'base': str(self.source), 'rules': rules or self.rules,
                'maxCandidates': 20, 'groups': groups, **extra}
        path = self.root / 'plan.json'
        path.write_text(ai.dumps(plan), encoding='utf-8')
        out = self.root / 'generated'
        with contextlib.redirect_stdout(io.StringIO()):
            workflow.generate(argparse.Namespace(plan=str(path), out=str(out)))
        return out, ai.read_json(out / 'candidates.json')

    def test_unconstrained_acceleration_is_not_silently_zero(self):
        units = self.units + [[29, 10, 100, 0]]
        rules = {**self.rules, 'parentCode': workflow.codec.encode(units)}
        self.assertIsNone(workflow.check(units, rules))

    def test_fixed_and_pairs_are_checked(self):
        rules = {**self.rules, 'fixed': [{'slot': 0, 'fields': ['x', 'y']}],
                 'pairs': [{'slots': [1, 2], 'fields': ['x', 'y']}]}
        units = [u[:] for u in self.units]
        units[1][1] += 1
        self.assertIsNotNone(workflow.check(units, rules))
        units[2][1] += 1
        self.assertIsNone(workflow.check(units, rules))
        units[0][1] += 1
        self.assertIsNotNone(workflow.check(units, rules))

    def test_cartesian_groups_normalization_and_ordered_dedup(self):
        out, rows = self.generate([
            {'name': 'first', 'axes': [{'slots': [1], 'field': 'r', 'values': [-69, 291, 651]},
                                       {'slots': [1, 2], 'field': 'x', 'offsets': [-1, 1]}]},
            {'name': 'alias', 'axes': [{'slots': [1], 'field': 'r', 'values': [291]},
                                       {'slots': [1, 2], 'field': 'x', 'offsets': [-1]}]}])
        self.assertEqual(len(rows), 2)
        self.assertTrue(all(r['group'] == 'first' for r in rows))
        self.assertTrue((out / 'aliases.json').exists())
        self.assertTrue(all(r['parentSha256'] == ai.digest(self.parent.encode()) for r in rows))

    def test_coordinate_profile_cannot_disable_structure_protection(self):
        units = [u[:] for u in self.units]
        units[1][0] = 25
        self.assertIsNotNone(workflow.check(units, {**self.rules, 'preserveIds': False}))

    def test_structure_recomputes_budget_and_acceleration(self):
        rules = {**self.rules, 'profile': 'structural-fixed-speed-v1', 'accelLevel': 2,
                 'preserveIds': False, 'budget': 250}
        self.assertIsNone(workflow.check([[0, 0, 0, 0], [30, 10, 100, 0]], rules))
        self.assertIsNone(workflow.check([[0, 0, 0, 0], [29, 10, 100, 0], [29, 40, 100, 0]], rules))
        self.assertIsNotNone(workflow.check([[0, 0, 0, 0], [29, 10, 100, 0]], rules))
        self.assertIsNotNone(workflow.check([[0, 0, 0, 0], [30, 10, 100, 0], [7, 40, 100, 290]], rules))

    def test_template_offsets_use_template_coordinates(self):
        rules = {**self.rules, 'profile': 'structural-fixed-speed-v1', 'accelLevel': 0, 'preserveIds': False}
        out, rows = self.generate([{'name': 'template', 'template': [[0, 0, 0, 0], [7, 100, 100, 290]],
                                   'axes': [{'slots': [1], 'field': 'x', 'offsets': [1]}]}], rules)
        self.assertEqual(workflow.codec.decode(rows[0]['code'])[1][1], 101)

    def test_forbidden_ids_and_maximum_counts(self):
        self.assertIsNotNone(workflow.check(self.units, {**self.rules, 'bannedIds': [7]}))
        self.assertIsNotNone(workflow.check(self.units, {**self.rules, 'maxCounts': {'7': 1}}))
        self.assertIsNone(workflow.check(self.units, {**self.rules, 'maxCounts': {'7': 2}}))

    def test_wrong_parent_and_unknown_rule_do_not_fail_open(self):
        with self.assertRaises(ValueError):
            self.generate([{'name': 'x', 'axes': []}], {**self.rules, 'parentCode': '000iyh'})
        self.assertIsNotNone(workflow.check(self.units, {**self.rules, 'budgte': 1}))

    def test_candidate_cap_never_publishes_a_truncated_list(self):
        with self.assertRaises(ValueError):
            self.generate([{'name': 'x', 'axes': [{'slots': [1], 'field': 'r', 'values': [291, 292]}]}],
                          maxCandidates=1)
        self.assertFalse((self.root / 'generated/candidates.json').exists())

    def test_duplicate_writes_and_invalid_axis_are_rejected(self):
        for axes in ([{'slots': [1], 'field': 'x', 'values': [50], 'offsets': [1]}],
                     [{'slots': [1], 'field': 'x', 'values': [50]}, {'slots': [1], 'field': 'x', 'values': [60]}]):
            with self.subTest(axes=axes), self.assertRaises(ValueError):
                self.generate([{'name': 'ambiguous', 'axes': axes}])


if __name__ == '__main__':
    unittest.main()
