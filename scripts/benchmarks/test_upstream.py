"""Contract tests for published evaluator formats, not provider responses."""
import argparse
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from .data import import_tasks, read
from .dsqa_api import metrics as autorater_metrics
from .structure import sentences
from .upstream import recipe, collect, jsonl, run
from .report import summarize


def options(**changes):
    base=dict(python="/usr/bin/python3",judge="upstream-model",provider="openai",structured_reports=None,
              collection_dir=None,collection=None,resolved_answers=None)
    return argparse.Namespace(**(base|changes))


class UpstreamContracts(unittest.TestCase):
    def test_published_autorater_counts_excess_answers(self):
        result=autorater_metrics({"Answer Correctness":{"Correctness Details":{"A":True,"B":False},"Excessive Answers":["C"]}})
        self.assertEqual(result,dict(precision=.5,recall=.5,f1=.5,complete=0))
        self.assertTrue(all(v is None for v in autorater_metrics(None).values()))
        ambiguous={"Answer Correctness":{"Correctness Details":{"A":True},"Excessive Answers":["B"]}}
        self.assertEqual(autorater_metrics(ambiguous,"Single Answer")["f1"],0)

    def test_different_grading_prompts_keep_same_answer_in_separate_scorecards(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            task=dict(id="0",prompt="Question",reference=dict(answer="A",answer_type="Single Answer"),source_record={})
            suite=root/"suite.json"
            suite.write_text(json.dumps(dict(benchmark="deepsearchqa",release="test",population=["0"],tasks=[task])))
            evaluation=root/"evaluation.json"
            evaluation.write_text(json.dumps(dict(mode="fixture",measurements=[dict(case="0",research_id="run",arm="perspective",outcome="completed",elapsed_ms=1)])))
            (root/"run.md").write_text("Answer A")
            reports=[]
            for i in range(2):
                prompt=root/f"prompt-{i}.txt"
                prompt.write_text(f"Recipe {i}: {{prompt}} {{prompt_type}} {{answer}} {{response}}")
                output=root/f"output-{i}"
                args=options(suite=suite,evaluation=evaluation,output=output,checkout=None,arm="perspective",autorater_prompt=prompt,judge_environment="same",execute=True)
                def provider(command, log):
                    (output/"autorater.json").write_text(json.dumps([dict(id="0",metrics=dict(f1=1))]))
                with patch("benchmarks.upstream.execute",side_effect=provider):
                    run(args)
                reports.append(read(output/"scores.json"))
            cards=summarize(reports)
            self.assertEqual(len(cards),2)
            self.assertTrue(all(c["summary"][0]["scored_tasks"]==1 for c in cards))

    def test_deer_reference_is_private_and_numeric_samples_match_runner(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            for sample in ("1","temp"):
                p=root/"physics"/sample
                p.mkdir(parents=True)
                (p/"query.md").write_text("Question")
                (p/"core_criteria.md").write_text("Private guidance")
            with self.assertRaises(ValueError):import_tasks("deer",root)
            tasks=import_tasks("deer",root,allow_noncommercial=True)
            self.assertEqual([t["id"] for t in tasks],["physics.1"])
            out=root/"out";out.mkdir()
            commands=recipe("deer",root,out,tasks,[dict(case="physics.1",research_id="run")],{"run":"answer"},options())
            self.assertEqual((out/"data/physics/1/candidate_1.md").read_text(),"answer")
            self.assertEqual(len(commands),3)
            self.assertIn("--output_root",commands[0]["argv"])
            result=out/"deer/physics/candidate/final/0001.json";result.parent.mkdir(parents=True)
            result.write_text(json.dumps({"score_avgs":{"request_fulfillment":8.5}}))
            self.assertEqual(collect("deer",out,root),{"physics.1":{"request_fulfillment":8.5}})

    def test_argue_preserves_and_or_answers_and_uses_actual_run_identity(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            bank=dict(query_id="388",full_query="Question",nugget_bank={"q":{"question":"q","aggregator_type":"AND","answers":{"a":{},"b":{}}}})
            p=root/"nuggets_388.v3.json";p.write_text(json.dumps(bank))
            tasks=import_tasks("ragtime",p)
            reports=root/"reports.jsonl"
            jsonl(reports,[dict(metadata=dict(topic_id="388",run_id="run"),responses=[],references=[])])
            out=root/"out";out.mkdir()
            commands=recipe("ragtime",root,out,tasks,[dict(case="388",research_id="run")],{"run":"answer"},
                options(structured_reports=reports,collection_dir=root,collection=["neuclir/1/ru"]))
            self.assertEqual(read(out/"nuggets_388.v3.json"),bank)
            self.assertIn("auto_argue.eval",commands[0]["argv"])
            (out/"argue-388.scores.tsv").write_text("run_id\trequest_id\tmetric\tvalue\nrun\t388\tnugget_coverage\t0.5\nrun\tall\tnugget_coverage_macro\t0.5\n")
            self.assertEqual(collect("ragtime",out,root),{"388":{"nugget_coverage":.5}})

    def test_report_conversion_cannot_improve_or_drop_original_content(self):
        answer="A claim [doc-a].\n\nAnother claim."
        result={"responses":[{"text":"A claim [doc-a].","citations":["doc-a"]},{"text":"Another claim.","citations":[]}]}
        self.assertEqual(len(sentences(answer,result)),2)
        result["responses"].pop()
        with self.assertRaises(ValueError):sentences(answer,result)
        result={"responses":[{"text":answer,"citations":["invented"]}]}
        with self.assertRaises(ValueError):sentences(answer,result)

    def test_drb1_fact_keeps_unresolved_citations_out_of_known_accuracy(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);(root/"race").mkdir()
            jsonl(root/"race/raw_results.jsonl",[dict(id=1,overall_score=.6)])
            jsonl(root/"validated.jsonl",[dict(id=1,citations_deduped={"x":dict(validate_error=None,validate_res=[dict(result="supported"),dict(result="unknown"),dict(result="unsupported")])})])
            result=collect("drb1",root,root)["1"]
            self.assertEqual(result["citation_accuracy"],.5)
            self.assertEqual(result["effective_citations"],1)
            self.assertEqual(result["unresolved_citations"],1)

    def test_ragdoll_preserves_both_published_coverage_and_support(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);(root/"nuggets/metrics").mkdir(parents=True)
            jsonl(root/"support-metrics.jsonl",[dict(topic_id="1",run_id="r",hard_precision=.75)])
            (root/"nuggets/metrics/cell_metrics.csv").write_text("qid,run_id,strict_vital_score,strict_all_score,vital_score,all_score,failed_count\n1,r,0.5,0.4,0.6,0.7,0\n")
            result=collect("trec-rag",root,root)["1"]
            self.assertEqual(result["strict_vital_score"],.5)
            self.assertEqual(result["hard_precision"],.75)


if __name__=="__main__":
    unittest.main()
