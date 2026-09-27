"""Exercise public-data preparation through scorecards at the runtime boundary."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from . import baseline, data


def download(url, target):
    if url.endswith('.csv'):
        target.write_text('problem,answer,answer_type\nSet question,A,Set Answer\nSingle question,A,Single Answer\n')
    elif url.endswith('holistic_200cases.jsonl'):
        target.write_text(json.dumps(dict(trace_id='1',perturbation_type='fabrication',query='Q',whole_original_answer='original',whole_perturbed_answer='perturbed'))+'\n')
    elif url.endswith('.tsv'):
        target.write_text('1\tQuestion\n')
    else:
        if 'tasks_and_rubrics' in url:
            row=dict(idx=1,prompt='Question',license='CC BY 4.0',content=dict(rubric={'analysis':['PRIVATE criterion']},blocked={'urls':[]}))
        elif 'dev-nuggets' in url:
            row=dict(qid='1',nuggets=[dict(text='PRIVATE nugget',importance='vital')])
        else:
            row=dict(qid='1',rubrics=[dict(criterion='PRIVATE criterion',axis='analysis',weight=4)])
        target.write_text(json.dumps(row)+'\n')


def runtime(binary, arguments):
    operation, *rest = arguments
    if operation == 'evaluate':
        mode, source, output = rest
        case=data.read(source)[0]
        assert 'PRIVATE' not in json.dumps(case)
        output.mkdir(parents=True)
        records=[]
        for arm in ('perspective','single_session'):
            rid=output.parent.name+'-'+output.name+'-'+arm
            (output/(rid+'.md')).write_text('Answer A with a cited source.')
            data.write(output/(rid+'.json'),dict(draft='Answer A',notes=[]))
            records.append(dict(research_id=rid,case=case['id'],arm=arm,outcome='completed',elapsed_ms=1000,usage={}))
        data.write(output/'evaluation.json',dict(mode='fixture',measurements=records))
    elif operation == 'assess':
        source, output = rest
        request=data.read(source);context=json.loads(request['context'])
        output.mkdir(parents=True)
        if 'A' in context:
            judgment=dict(better='A' if context['A']=='original' else 'B',reason='Published factual defect')
        elif 'criteria' in context['reference']:
            judgment=dict(items=[dict(id=c['id'],score=1,reason='present',evidence='answer and source') for c in context['reference']['criteria']],consequential_errors=[],unresolved=[])
        else:
            judgment=dict(gold_items=['A'],predicted_items=['A'],matches=[[0,0]],reason='match',consequential_errors=[],unresolved=[])
        data.write(output/'assessment.json',judgment)
    else:
        raise AssertionError('Unexpected runtime operation')


class PublicBaseline(unittest.TestCase):
    def options(self, root):
        return argparse.Namespace(output=root,prepare_only=False,tasks_per_benchmark=2,seed=7,
            repeats=1,configuration='contract-test',policy='perspective',seconds=1200,tool_calls=80,
            binary='external-runtime',judge_seconds=600,judge_tool_calls=24)

    def test_public_sources_to_paired_scores_and_diagnostic(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)/'baseline'
            with patch('benchmarks.baseline.download',side_effect=download),patch('benchmarks.runner.invoke',side_effect=runtime):
                baseline.run(self.options(root))
            self.assertEqual(set(data.read(root/'baseline.json')['benchmarks']),{'deepsearchqa','drb2','trec-rag','researchrubrics'})
            cards=data.read(root/'summary.json')['scorecards']
            self.assertEqual(len(cards),4)
            self.assertTrue(all(c['paired'][0]['paired_tasks']>0 for c in cards))
            self.assertTrue(data.read(root/'qualification/qualification.json')['complete'])
            self.assertTrue(all('PRIVATE' not in (root/k/'cases.json').read_text() for k in baseline.SOURCES))
            rescore=argparse.Namespace(baseline=root,output=Path(directory)/'rescored',binary='external-runtime',judge_seconds=600,judge_tool_calls=24)
            with patch('benchmarks.runner.invoke',side_effect=runtime) as calls:
                baseline.score(rescore)
                self.assertTrue(all(c.args[1][0]=='assess' for c in calls.call_args_list))
            self.assertEqual(len(data.read(rescore.output/'summary.json')['scorecards']),4)

    def test_capacity_failure_remains_unfinished_and_preserves_population(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)/'baseline'
            with patch('benchmarks.baseline.download',side_effect=download),patch('benchmarks.runner.invoke',side_effect=subprocess.CalledProcessError(1,['runtime'])):
                with self.assertRaises(subprocess.CalledProcessError):baseline.run(self.options(root))
            self.assertEqual(data.read(root/'coverage.json')['scored_files'],0)
            self.assertEqual(len(data.read(root/'baseline.json')['benchmarks']),4)
            self.assertFalse((root/'summary.json').exists())


if __name__=='__main__':
    unittest.main()
