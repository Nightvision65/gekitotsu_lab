import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("gkt_ai", ROOT / "tools/ai/ai.py")
ai = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ai)


def code(unit_id, x, y, r=0, core=False):
    number = r*1000000+(x+(52 if core else 16))*1000+y+(58 if core else 20)
    digits = ""
    for _ in range(5):
        number, d = divmod(number, 61)
        digits = ai.ALPHABET[d]+digits
    return ("0" if core else ai.ALPHABET[unit_id])+digits


class InputTests(unittest.TestCase):
    def test_frozen_rust_sources(self):
        manifest=ai.read_json(ROOT/"rust/vendor/source-hashes.json")
        for path,expected in manifest["files"].items():
            self.assertEqual(ai.file_hash(ROOT/path),expected,path)

    def test_refuse_silent_clean_or_clamp(self):
        valid = code(0, 0, 0, core=True)+code(1, 348, 349, 359)
        self.assertEqual(ai.validate_code(valid), valid)
        for invalid in (valid+"#HP", valid.replace(valid[2], "Z", 1), "000000", "中国阵", valid+code(61, 0, 0), code(0, 277, 0, core=True)):
            with self.assertRaises(ValueError):
                ai.validate_code(invalid)


class StoreTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.store = Path(self.tmp.name)/"store.sqlite"
        self.db = ai.connect(self.store)

    def tearDown(self):
        self.db.close()
        self.tmp.cleanup()

    def insert(self, data):
        req, fp = {"p1":"test"}, {"backend":"fixture"}
        artifact = ai.digest(ai.dumps({"request":req,"fingerprint":fp}).encode())
        raw = Path(self.tmp.name)/"raw.json"
        raw.write_text(ai.dumps(data), encoding="utf-8")
        tables = ai.normalized(data)
        with self.db:
            self.db.execute("INSERT INTO artifacts VALUES(?,?,?,?,?,?,?,?)", (artifact, "fixture", ai.dumps(req), ai.dumps(fp), ai.dumps(ai.compact_summary(data,tables)), str(raw), ai.file_hash(raw), "now"))
            for table, rows in tables.items():
                for index, record in enumerate(rows):
                    self.db.execute("INSERT INTO records VALUES(?,?,?,?,?,?,?,?)", (artifact, table, index, record.get("side"), record.get("slot"), record.get("kind"), record.get("frame",record.get("launchFrame")), ai.dumps(record)))
        return artifact, raw

    def test_bounded_indexed_queries_with_repeat_units(self):
        events = [{"frame":i, "kind":"death", "side":i%2, "slot":i%3, "entityId":i, "detail":{"text":"汉"*500}} for i in range(60)]
        artifact, _ = self.insert({"events":events})
        args = ai.parse_args(["--max-bytes", "1024", "query", "--artifact", artifact, "--table", "events", "--side", "0", "--slot", "2", "--fields", "frame,entityId", "--limit", "3"])
        out = ai.query(self.db, args)
        self.assertEqual(out["total"],10)
        self.assertEqual([r["frame"] for r in out["rows"]], [2,8,14])
        self.assertEqual(out["nextOffset"],3)
        self.assertLess(len(ai.dumps(out).encode()),1024)
        args.fields = "detail"
        out = ai.query(self.db,args)
        self.assertEqual(out["rows"],[])
        self.assertEqual(out["nextOffset"],0)
        self.assertTrue(out["limitedByBytes"])

    def test_tamper_is_not_cache_hit(self):
        artifact, raw = self.insert({"events":[]})
        raw.write_text("{}", encoding="utf-8")
        with self.assertRaisesRegex(ValueError,"SHA256"):
            ai.verified(self.db,artifact)

    def test_verify_detects_index_tamper(self):
        artifact, _ = self.insert({"events":[{"kind":"death","frame":3}]})
        self.db.execute("UPDATE records SET data='{}'")
        self.db.commit()
        args = ai.parse_args(["verify","--artifact",artifact])
        with self.assertRaisesRegex(ValueError,"索引"):
            ai.dispatch(self.db,args)

    def test_ground_landings_are_separate_position_statistics(self):
        unit={"entityId":3,"side":0,"slot":1,"typeId":8,"deathFrame":None,"firstHalfFrame":None,"launches":2,"firstBoardFrame":800}
        flights=[{"entityId":3,"side":0,"slot":1,"typeId":8,"launchFrame":100,"landFrame":200,"destinationSurface":"ground", "endReason":"landed","endX":100,"endY":566,"enemyCoreRelativeX":-500,"enemyCoreRelativeY":180},
                 {"entityId":3,"side":0,"slot":1,"typeId":8,"launchFrame":500,"landFrame":800,"destinationSurface":"enemy_platform", "endReason":"landed","endX":500,"endY":360,"enemyCoreRelativeX":-50,"enemyCoreRelativeY":-20}]
        artifact,_=self.insert({"units":[unit],"flights":flights,"summary":{"complete":True}})
        manifest=Path(self.tmp.name)/"manifest.json"
        manifest.write_text(ai.dumps({"battles":[{"artifact":artifact}]}),encoding="utf-8")
        out=ai.cohort(self.db,ai.parse_args(["cohort","--manifest",str(manifest)]))
        row=out["rows"][0]
        self.assertEqual(row["landingSurfaces"],{"ground":1,"enemy_platform":1})
        self.assertEqual(row["landingPositionStats"]["ground"]["worldX"]["mean"],100)
        self.assertNotIn("failed",row)
        query=ai.query(self.db,ai.parse_args(["query","--artifact",artifact,"--table","flights","--surface","ground","--time-field","landFrame","--from-frame","150","--to-frame","300"]))
        self.assertEqual(query["total"],1)
        self.assertEqual(query["rows"][0]["destinationSurface"],"ground")

    def test_detailed_tables_and_attacker_queries(self):
        data={"mode":"detailed","combatEvents":[{"frame":3,"kind":"damage","entityId":5,"side":0,"slot":0,"attackerEntityId":12,"sourceObjectId":99}],
              "damageSources":[{"entityId":5,"side":0,"slot":0,"attackerEntityId":12,"percentage":100}],
              "objects":[{"entityId":99,"parentEntityId":50,"ownerUnitId":12}],
              "flightSamples":[{"frame":3,"entityId":6,"surface":"airborne"}]}
        artifact,_=self.insert(data)
        q=ai.query(self.db,ai.parse_args(["query","--artifact",artifact,"--table","combatEvents","--attacker-entity-id","12","--source-object-id","99"]))
        self.assertEqual(q["total"],1)
        q=ai.query(self.db,ai.parse_args(["query","--artifact",artifact,"--table","flightSamples","--surface","airborne"]))
        self.assertEqual(q["total"],1)
        self.assertTrue(ai.dispatch(self.db,ai.parse_args(["verify","--artifact",artifact]))["verified"])


if __name__ == "__main__":
    unittest.main()
