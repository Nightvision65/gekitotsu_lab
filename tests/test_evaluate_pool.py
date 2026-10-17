import sys
from pathlib import Path
import unittest
import argparse
import gc
import tempfile
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools/ai"))
import evaluate_pool as pool
import verify_pool


class SidePoolTests(unittest.TestCase):
    def test_verified_cache_only_resume_and_frozen_input(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            own, a, b = '000iyg', '000iyh', '000iyi'
            source, opponents, store = root/'candidates.json', root/'opponents.txt', root/'store.sqlite'
            source.write_text(pool.ai.dumps([{'name':'candidate','code':own}]), encoding='utf-8')
            opponents.write_text(a+'/'+b, encoding='utf-8')
            fp = {'backend':'fixture'}
            req = {'action':'battle','maxFrames':65536}
            data = {'p1':[{'code':a},{'code':b}], 'p2':[{'code':own}],
                    'forward':[{'p1Index':i,'p2Index':0,'status':2 if i==0 else -1,
                                'winnerHp':5 if i==0 else -1,'frames':20 if i==0 else 65536} for i in range(2)]}
            artifact = pool.ai.digest(pool.ai.dumps({'request':req,'fingerprint':fp}).encode())
            raw = root/'fixture.json'
            raw.write_text(pool.ai.dumps(data), encoding='utf-8')
            with pool.ai.connect(store) as db:
                db.execute('INSERT INTO artifacts VALUES(?,?,?,?,?,?,?,?)',
                           (artifact,'battle',pool.ai.dumps(req),pool.ai.dumps(fp),'{}',str(raw),pool.ai.file_hash(raw),'now'))
            db.close()
            args = argparse.Namespace(candidates=str(source),opponents=str(opponents),store=str(store),
                                      out=str(root/'out'),side=2,max_frames=65536,batch_size=64,time_limit=30)
            with patch.dict(pool.os.environ, {'CONDA_DEFAULT_ENV':'opencode'}), patch.object(pool,'fingerprint',return_value=fp), patch.object(pool.ai,'compute',side_effect=AssertionError('不应重复模拟')):
                pool.run(args)
                pool.run(args)
                result = pool.ai.read_json(root/'out/results.json')['candidates'][0]
                self.assertEqual((result['W'],result['U'],result['N'],result['scorePct']), (1,1,2,75))
                self.assertTrue(pool.ai.read_json(root/'out/gate.json')['passed'])
                proof_args = argparse.Namespace(run=str(root/'out'), candidates=str(source), opponents=str(opponents), store=None)
                self.assertEqual(verify_pool.verify(proof_args)['scoredGames'], 2)
                # 即使重新计算汇总哈希，也不能把另一物理行冒充本场对局。
                changed = pool.ai.read_json(root/'out/results.json')
                changed['candidates'][0]['sourceByOpponent'][0][2] = 1
                (root/'out/results.json').write_text(pool.ai.dumps(changed),encoding='utf-8')
                gate = pool.ai.read_json(root/'out/gate.json')
                gate['resultsSha256'] = pool.ai.file_hash(root/'out/results.json')
                (root/'out/gate.json').write_text(pool.ai.dumps(gate),encoding='utf-8')
                with self.assertRaisesRegex(ValueError,'对手顺序'):
                    verify_pool.verify(proof_args)
                source.write_text(pool.ai.dumps([{'name':'changed','code':own}]),encoding='utf-8')
                with self.assertRaisesRegex(ValueError,'指纹改变'):
                    pool.run(args)
            # 连续调用CLI函数的测试在同一进程内，回收SQLite连接的内部循环后清理临时目录。
            gc.collect()

    def test_reverse_indices_are_physical_rectangular(self):
        data = {"p1": [{"code": "candidate"}], "p2": [{"code": "a"}, {"code": "b"}],
                "forward": [{"p1Index": 0, "p2Index": i, "status": 1, "winnerHp": 10, "frames": 20} for i in range(2)],
                "reverse": [{"p1Index": i, "p2Index": 0, "status": 2 if i == 0 else 1, "winnerHp": 4, "frames": 30} for i in range(2)]}
        cache = {}
        pool.ingest(data, "artifact", "raw", 2, {"candidate"}, {"a", "b"}, cache)
        self.assertEqual(cache[("candidate", "a")][0][0], 2)
        self.assertEqual(cache[("candidate", "b")][0][0], 1)
        self.assertEqual(pool.score([v[0] for v in cache.values()], 2)["scorePct"], 50)
        data["reverse"].pop()
        with self.assertRaisesRegex(ValueError, "缺局"):
            pool.ingest(data, "artifact", "raw", 2, {"candidate"}, {"a", "b"}, {})

    def test_unresolved_draw_and_error(self):
        s = pool.score([[2, 10, 20], [1, 5, 30], [0, -1, 10], [-1, -1, 65536]], 2)
        self.assertEqual((s["W"], s["L"], s["D"], s["U"], s["scorePct"]), (1, 1, 1, 1, 50))
        with self.assertRaises(ValueError):
            pool.score([[-2, 0, 0]], 2)

    def test_partial_cache_batch_cap_and_duplicate_pool_weights(self):
        candidates = [{"code": str(i)} for i in range(70)]
        opponents = [{"code": "a"}, {"code": "a"}, {"code": "b"}]
        cache = {("0", "a"): ([2, 4, 30], {})}
        chunks = list(pool.batches(candidates, opponents, cache, 64))
        self.assertEqual(len(chunks[0][1]), 1)
        self.assertEqual(sum(len(c)*len(o) for c, o in chunks), 1 + 69*3)
        for chunk, ops in pool.batches(candidates, [{"code": str(i)} for i in range(10433)], {}, 64):
            self.assertLessEqual(len(chunk)*len(ops), 12800)
            self.assertLessEqual(len(chunk), 64)


if __name__ == "__main__":
    unittest.main()
