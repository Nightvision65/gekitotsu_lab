"""Native integration: upward/downward gravity, normalization, and no unreviewed battles."""
import argparse
import contextlib
import io
import os
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools/ai'))
import gravity
import ai


def code(units):
    alphabet=ai.ALPHABET
    out=''
    for slot,(kind,x,y,r) in enumerate(units):
        dx,dy=(52,58) if slot==0 else (16,20)
        number=r*1000000+(x+dx)*1000+y+dy
        digits=''
        for _ in range(5):digits=alphabet[number%61]+digits;number//=61
        out+=alphabet[kind]+digits
    return out


class GravityGateTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)
        self.source=self.root/'input.json'

    def run_audit(self,rows,apply=False,max_frames=240):
        self.source.write_text(ai.dumps(rows),encoding='utf-8')
        args=argparse.Namespace(input=str(self.source),out=str(self.root/'audit'),max_frames=max_frames,stable_frames=2,timeout=60)
        with contextlib.redirect_stdout(io.StringIO()):status=gravity.audit(args,apply=apply)
        return status,self.root/'audit'

    def test_apply_deduplicates_preserving_aliases_not_old_scores(self):
        a=code([(0,0,0,0),(7,250,0,280)])
        b=code([(0,0,0,0),(7,250,100,280)])
        status,out=self.run_audit([{'name':'先','code':a,'scorePct':99,'sha256':'old'}, {'name':'后','code':b}],apply=True)
        self.assertEqual(status,0)
        rows=ai.read_json(out/'candidates.json');self.assertEqual(len(rows),1)
        self.assertEqual(rows[0]['name'],'先');self.assertNotIn('scorePct',rows[0]);self.assertNotIn('sha256',rows[0])
        self.assertEqual([r['outputIndex'] for r in ai.read_json(out/'aliases.json')],[0,0])
        gravity.verify_audit(out,self.source)

    def test_upward_and_downward_report_and_immutable_evidence(self):
        status,out=self.run_audit([{'code':code([(0,0,0,0),(25,200,100,0),(10,200,100,330),(7,300,0,280)])}])
        self.assertEqual(status,3)
        row=ai.read_json(out/'result.json')['formations'][0]
        self.assertLess(row['units'][2]['delta']['y'],0)
        self.assertGreater(row['units'][3]['delta']['y'],300)
        with self.assertRaises(ValueError):gravity.verify_audit(out,self.source)
        with self.assertRaises(FileExistsError):gravity.audit(argparse.Namespace(input=str(self.source),out=str(out),max_frames=240,stable_frames=2,timeout=60))

    def test_no_battle_before_gravity_gate(self):
        self.source.write_text(ai.dumps([{'code':code([(0,0,0,0),(7,250,0,280)])}]),encoding='utf-8')
        args=argparse.Namespace(candidates=str(self.source),audit_out=str(self.root/'preflight'),max_frames=240,stable_frames=12,timeout=60,
            evaluation_timeout=60,runner_args=['--','--opponents',str(self.root/'MUST-NOT-OPEN'),'--out',str(self.root/'MUST-NOT-CREATE')])
        with contextlib.redirect_stdout(io.StringIO()):self.assertEqual(gravity.evaluate(args),3)
        self.assertFalse((self.root/'MUST-NOT-CREATE').exists())

    def test_unencodable_and_short_run_never_publish(self):
        status,out=self.run_audit([{'code':code([(0,0,0,0),(25,200,0,0),(7,200,0,280)])}],apply=True)
        self.assertEqual(status,3);self.assertFalse((out/'candidates.json').exists())

    def test_normal_support_push_passes_even_above_editor_boundary(self):
        status,out=self.run_audit([{'code':code([(0,0,0,0),(25,200,0,0),(7,200,0,280)])}])
        self.assertEqual(status,0)
        row=ai.read_json(out/'result.json')['formations'][0]
        self.assertFalse(row['encodable'])
        self.assertEqual(row['units'][2]['final']['y'],-33)
        self.assertTrue(row['units'][2]['acceptedSupportPush'])
        gravity.verify_audit(out,self.source)

    def test_stacked_support_push_over_one_wall_rejected(self):
        status,out=self.run_audit([{'code':code([(0,0,130,0),(25,200,100,0),(25,200,66,0),(7,200,100,280)])}])
        self.assertEqual(status,3)
        with self.assertRaises(ValueError):gravity.verify_audit(out,self.source)

    def test_short_run_can_pass_normal_push_without_convergence(self):
        status,out=self.run_audit([{'code':code([(0,0,0,0),(25,200,0,0),(7,200,0,280)])}],max_frames=2)
        self.assertEqual(status,0)
        self.assertFalse(ai.read_json(out/'result.json')['formations'][0]['converged'])
        gravity.verify_audit(out,self.source)

    def test_target_ball_intentional_fall_passes(self):
        status,out=self.run_audit([{'code':code([(0,0,0,0),(42,250,0,0)])}])
        self.assertEqual(status,0)
        row=ai.read_json(out/'result.json')['formations'][0]
        self.assertTrue(row['units'][1]['acceptedTargetFall'])
        self.assertGreater(row['units'][1]['maxDown'],300)
        gravity.verify_audit(out,self.source)

    def test_tampered_or_replaced_input_rejected(self):
        status,out=self.run_audit([{'code':code([(0,0,0,0),(7,250,0,280)])}],apply=True)
        self.assertEqual(status,0)
        (out/'result-applied.json').write_text('{}',encoding='utf-8')
        with self.assertRaises(ValueError):gravity.verify_audit(out,self.source)

    def test_process_timeout_is_real(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            gravity.bounded([sys.executable,'-c','import time; time.sleep(30)'],0.1)


if __name__=='__main__':
    if os.environ.get('CONDA_DEFAULT_ENV')!='opencode':raise SystemExit('请先conda activate opencode')
    unittest.main()
