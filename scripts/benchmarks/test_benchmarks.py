import argparse
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from .runner import grade, native_import
from .data import import_tasks, prepare
from .scoring import rubric_metrics, set_metrics
from .report import summarize


class PublishedContracts(unittest.TestCase):
    def test_rubric_penalties_and_unresolved_do_not_receive_credit(self):
        criteria = [dict(id="a", dimension="analysis", weight=5), dict(id="b", dimension="analysis", weight=-4)]
        judged = {"items": [dict(id=i, score=1, reason="observed", evidence="passage") for i in ("a", "b")]}
        self.assertEqual(rubric_metrics("researchrubrics", criteria, judged)["compliance"], .2)
        judged["items"][0]["score"] = None
        self.assertIsNone(rubric_metrics("researchrubrics", criteria, judged)["compliance"])

    def test_published_nuggets_distinguish_partial_from_strict_coverage(self):
        criteria = [dict(id="a", dimension="vital", weight=1), dict(id="b", dimension="okay", weight=1)]
        judged = {"items": [dict(id="a", score=.5, reason="partial", evidence="passage"), dict(id="b", score=1, reason="full", evidence="passage")]}
        metrics = rubric_metrics("trec-rag", criteria, judged)
        self.assertEqual(metrics, dict(strict_vital=0, strict_all=.5, vital=.5, all=.75))

    def test_drb2_blocked_reference_is_not_credit(self):
        criteria = [dict(id="a", dimension="analysis", weight=1)]
        judged = {"items": [dict(id="a", score=-1, reason="blocked report", evidence="URL")]}
        self.assertEqual(rubric_metrics("drb2", criteria, judged), dict(analysis=0, total=0, blocked_rate=1))

    def test_answer_sets_count_missing_and_invented_items(self):
        j = dict(gold_items=["a", "b"], predicted_items=["a", "c", "d"], matches=[[0, 0]], unresolved=[])
        m = set_metrics(j)
        self.assertEqual(m["precision"], 1/3)
        self.assertEqual(m["recall"], .5)
        self.assertEqual(m["complete"], 0)
        j["matches"] = [[0, 0], [1, 0]]
        with self.assertRaises(ValueError): set_metrics(j)

    def test_reference_does_not_leak_into_research_cases(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            source = root / "tasks.jsonl"
            source.write_text(json.dumps(dict(idx=1, prompt="Question", license="CC BY 4.0", content={"rubric": {"analysis": ["SECRET REFERENCE"]}, "blocked": {"urls": ["https://example.org/blocked"]}}))+"\n")
            args = argparse.Namespace(benchmark="drb2", input=str(source), output=str(root/"prepared"), release="test", ids=None, allow_noncommercial=False, topics=None, environment="open-web", policy="perspective", seconds=1200, tool_calls=80)
            prepare(args)
            public = (root/"prepared/cases.json").read_text()
            self.assertNotIn("SECRET REFERENCE", public)
            self.assertIn("https://example.org/blocked", public)
            self.assertIn("SECRET REFERENCE", (root/"prepared/suite.json").read_text())
            source.write_text(source.read_text().replace("CC BY 4.0", "CC BY-NC 4.0"))
            with self.assertRaises(ValueError): import_tasks("drb2", source)

    def test_score_identity_uses_recorded_run_environment(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            suite = dict(benchmark="deepsearchqa", release="test", environment="intended-corpus", population=["a"], tasks=[dict(id="a", prompt="Question", reference=dict(answer="answer", answer_type="Single Answer"))])
            evaluation = dict(mode="live", model="researcher", environment="actual-corpus", measurements=[dict(research_id="answer", case="a", arm="single_session", outcome="completed", elapsed_ms=1000, usage={})])
            for name, value in (("suite",suite),("evaluation",evaluation)):
                (root/(name+".json")).write_text(json.dumps(value))
            (root/"answer.md").write_text("answer")
            (root/"upstream.csv").write_text("id,f1\na,1\n")
            args = argparse.Namespace(suite=root/"suite.json", evaluation=root/"evaluation.json", output=root/"grade", binary="unused", seconds=600, tool_calls=24, batch_size=20, qualification="test")
            with patch("benchmarks.runner.assess", return_value=dict(gold_items=["answer"], predicted_items=["answer"], matches=[[0,0]], reason="match", consequential_errors=[], unresolved=[])):
                grade(args)
            self.assertEqual(json.loads((root/"grade/scores.json").read_text())["environment"],"actual-corpus")
            args.output=root/"native.json"
            args.input=root/"upstream.csv"
            args.arm="single_session"
            args.id_column="id"
            args.metrics=["f1=f1"]
            args.protocol="upstream-test"
            args.judge="judge"
            native_import(args)
            self.assertEqual(json.loads(args.output.read_text())["environment"],"actual-corpus")
            del evaluation["environment"]
            (root/"evaluation.json").write_text(json.dumps(evaluation))
            native_import(args)
            self.assertEqual(json.loads(args.output.read_text())["environment"],"unspecified (legacy evaluation)")

    def test_repeats_do_not_outweigh_tasks_and_judges_stay_separate(self):
        def record(rid, task, arm, value, outcome="completed"):
            return dict(research_id=rid, case=task, arm=arm, metrics={"recall":value}, outcome=outcome, elapsed_ms=5000, judgment=dict(consequential_errors=[], unresolved=[]))
        r = dict(benchmark="x", release="a", protocol="adapted", judge_model="judge", environment="web", mode="live", records=[record("1", "a", "agent", 1), record("2", "a", "agent", 1), record("3", "b", "agent", 0, "incomplete"), record("4", "a", "control", .5), record("5", "b", "control", None)])
        cards = summarize([r])
        agent = next(x for x in cards[0]["summary"] if x["arm"] == "agent")
        self.assertEqual(agent["task_macro_mean"], .5)
        self.assertEqual(agent["completed"], 2)
        self.assertEqual(cards[0]["paired"][0]["paired_tasks"], 1)
        self.assertIsNone(cards[0]["paired"][0]["task_bootstrap_95_interval"])
        self.assertEqual(len(summarize([r, {**r,"judge_model":"different"}])), 2)
        with self.assertRaises(ValueError): summarize([r, r])


if __name__ == "__main__":
    unittest.main()
