"""Explicit adapters to published evaluators in operator-managed environments.

The normal grade command uses the subscription runtime. This separate command
runs upstream programs only when --execute acknowledges their provider usage.
It never installs packages, acquires credentials or changes an upstream checkout.
"""
import json
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
from .data import read, write, rows, file_id
from .runner import read_evaluation, agent_label


def add_arguments(commands):
    p = commands.add_parser("upstream-run", description=__doc__)
    for name in ("suite", "evaluation", "output"):
        p.add_argument(name)
    p.add_argument("--checkout", help="trusted checkout of the published evaluator")
    p.add_argument("--autorater-prompt", help="published DeepSearchQA format template, obtained from the starter notebook")
    p.add_argument("--arm", required=True)
    p.add_argument("--python", default=sys.executable, help="Python from the prepared upstream environment")
    p.add_argument("--execute", action="store_true", help="run the displayed upstream recipe; may consume provider credit")
    p.add_argument("--judge", required=True, help="actual model/recipe label; configuration follows upstream")
    p.add_argument("--judge-environment", required=True)
    p.add_argument("--provider", default="openai", choices=["openai", "anthropic", "together", "huggingface", "huggingface_local"])
    p.add_argument("--collection-dir", help="matching Auto-ARGUE document lookup")
    p.add_argument("--collection", nargs="+", help="actual Auto-ARGUE collection identifiers")
    p.add_argument("--structured-reports", help="sentence/citation JSONL in the published TREC/ARGUE format")
    p.add_argument("--resolved-answers", help="RAGDoll answer JSONL with cited passage text")


def jsonl(path, values):
    Path(path).write_text("".join(json.dumps(v, ensure_ascii=False, allow_nan=False)+"\n" for v in values))


def recipe(kind, checkout, output, tasks, measurements, answers, args):
    """Materialize selected inputs and return argv/cwd pairs, never shell code."""
    if kind != "deepsearchqa" and not checkout:
        raise ValueError("this evaluator requires --checkout")
    checkout, output = Path(checkout or ".").resolve(), Path(output).resolve()
    py = ["uv", "run", "--no-project", "--no-sync", str(Path(args.python).absolute())]
    commands = []
    def script(name, *arguments):
        commands.append({"argv": py + [str(checkout/name), *map(str, arguments)], "cwd": str(checkout)})
    def module(name, *arguments):
        commands.append({"argv": py + ["-m", name, *map(str, arguments)], "cwd": str(checkout)})
    source = output/"tasks.jsonl"
    jsonl(source, [t["source_record"] for t in tasks])
    if kind == "deepsearchqa":
        if not args.autorater_prompt:
            raise ValueError("DeepSearchQA needs --autorater-prompt from its published starter notebook")
        write(output/"tasks.json",tasks)
        write(output/"answers.json",{m["case"]:answers[m["research_id"]] for m in measurements})
        commands.append({"argv":py+[str(Path(__file__).with_name("dsqa_api.py")),str(output/"tasks.json"),
            str(output/"answers.json"),str(Path(args.autorater_prompt).resolve()),str(output/"autorater.json"),args.judge],
            "cwd":str(output)})
    elif kind == "drb1":
        raw = output/"reports"
        raw.mkdir()
        jsonl(raw/"candidate.jsonl", [dict(id=int(m["case"]), prompt=t["prompt"], article=answers[m["research_id"]]) for m,t in zip(measurements,tasks)])
        script("deepresearch_bench_race.py", "candidate", "--raw_data_dir", raw, "--cleaned_data_dir", output/"cleaned",
               "--query_file", source, "--output_dir", output/"race", "--max_workers", 1)
        previous = raw/"candidate.jsonl"
        for stage, name in (("extract","extracted"),("deduplicate","deduplicated"),("scrape","scraped"),("validate","validated")):
            destination = output/f"{name}.jsonl"
            arguments = ["--raw_data_path",previous,"--output_path",destination,"--n_total_process",1]
            if stage != "scrape":
                arguments += ["--query_data_path",source]
            module(f"utils.{stage}", *arguments)
            previous = destination
    elif kind == "drb2":
        reports = output/"reports"/"candidate"
        reports.mkdir(parents=True)
        for m in measurements:
            (reports/f"idx-{file_id(m['case'])}.md").write_text(answers[m["research_id"]])
        script("run_evaluation.py","--pdf_dir",reports.parent,"--tasks_jsonl",source,
               "--out_jsonl",output/"judgments.jsonl","--log_file",output/"upstream.log","--max_workers",1,"--model",args.judge)
        script("aggregate_scores.py","--input",output/"judgments.jsonl","--tasks-file",source,"--output-prefix",output/"aggregate")
    elif kind == "researchrubrics":
        if args.judge != "litellm_proxy/gemini/gemini-2.5-pro-preview-06-05":
            raise ValueError("the published ResearchRubrics entry point fixes its judge to litellm_proxy/gemini/gemini-2.5-pro-preview-06-05; use grade for a substituted judge")
        reports = output/"reports"
        reports.mkdir()
        for m in measurements:
            (reports/f"{file_id(m['case'])}.md").write_text(answers[m["research_id"]])
        commands.append({"argv":py+[str(Path(__file__).with_name("upstream_bridge.py")),str(checkout),str(source),str(reports),str(output/"rubrics.json")],"cwd":str(output)})
    elif kind == "deer":
        by_domain = {}
        for m,t in zip(measurements,tasks):
            ref = t["reference"]
            domain, sample = file_id(ref["domain"]), file_id(ref["sample"])
            if not sample.isdecimal():
                raise ValueError("DEER upstream samples must be numeric directory names")
            target=output/"data"/domain/sample
            target.mkdir(parents=True)
            (target/"query.md").write_text(t["prompt"])
            (target/"core_criteria.md").write_text(ref["core_criteria"])
            (target/f"candidate_{int(sample)}.md").write_text(answers[m["research_id"]])
            by_domain.setdefault(domain,[]).append(sample)
        for domain, samples in by_domain.items():
            root, out = output/"data"/domain, output/"deer"/domain
            common=["--prefix","candidate","--samples",",".join(samples)]
            script("run_information_verification.py",*common,"--root",root,"--output_root",out)
            script("run_report_evaluation.py",*common,"--root",root,"--output_dir",out,"--eval_model",args.judge,"--max_concurrency",1)
            script("run_score_integration.py",*common,"--output_dir",out)
    elif kind == "ragtime":
        if not args.structured_reports or not args.collection_dir or not args.collection:
            raise ValueError("Auto-ARGUE needs --structured-reports, --collection-dir and --collection; use structure-reports for saved Markdown")
        reports = {str(r["metadata"]["topic_id"]):r for r in rows(args.structured_reports)}
        for t in tasks:
            key=file_id(t["id"])
            report=reports[key]
            # Do not relabel another run as the selected answer.
            if report["metadata"]["run_id"] != next(m["research_id"] for m in measurements if m["case"]==key):
                raise ValueError("structured report run_id does not match selected saved answer")
            input_file=output/f"report-{key}.jsonl"
            jsonl(input_file,[report])
            nuggets=output/f"nuggets_{key}.v3.json"
            write(nuggets,t["reference"]["source_record"])
            module("auto_argue.eval",input_file,nuggets,output/f"argue-{key}","-p",args.provider,"-m",args.judge,
                   "-C",Path(args.collection_dir).resolve(),"-c",*args.collection,"-a","annotate","score","--validate-judgments")
    elif kind == "trec-rag":
        if not args.resolved_answers:
            raise ValueError("RAGDoll needs --resolved-answers with matching corpus passages; use structure-reports then its resolve-references command")
        answer_rows=rows(args.resolved_answers)
        expected={m["research_id"] for m in measurements}
        selected=[r for r in answer_rows if str(r.get("run_id",r.get("runtag",""))) in expected]
        if {str(r.get("run_id",r.get("runtag",""))) for r in selected} != expected:
            raise ValueError("resolved answers must include every selected research_id as run_id")
        jsonl(output/"answers.jsonl",selected)
        jsonl(output/"nuggets.jsonl",[{**t["source_record"],"query":t["prompt"]} for t in tasks])
        module("ragdoll.cli","nuggetizer","eval","--nuggets-file",output/"nuggets.jsonl","--answers-file",output/"answers.jsonl",
               "--output-dir",output/"nuggets","--model",args.judge)
        module("ragdoll.cli","support","judge","--input-file",output/"answers.jsonl","--output-file",output/"support-judgments.jsonl","--model",args.judge)
        module("ragdoll.cli","support","assemble","--answers-file",output/"answers.jsonl","--judgments",output/"support-judgments.jsonl","--output-file",output/"support.jsonl")
        module("ragdoll.cli","support","metrics","--input-file",output/"support.jsonl","--output-file",output/"support-metrics.jsonl")
    else:
        raise ValueError(f"{kind}: use grade for the adapted subscription-runtime recipe; no upstream execution adapter for this release")
    return commands


def collect(kind, output, checkout):
    """Normalize per-task outputs; keep the complete upstream files alongside them."""
    output=Path(output)
    found={}
    if kind=="deepsearchqa":
        found={r["id"]:r["metrics"] for r in read(output/"autorater.json")}
    elif kind=="drb1":
        for row in rows(output/"race/raw_results.jsonl"):
            found[str(row["id"])]={k:row.get(k) for k in ("overall_score","comprehensiveness","insight","instruction_following","readability")}
        for row in rows(output/"validated.jsonl"):
            valid=[]
            unknown=0
            for citation in row.get("citations_deduped",{}).values():
                if citation.get("validate_error") is not None:
                    unknown+=1
                    continue
                for result in citation.get("validate_res",[]):
                    if result["result"]=="unknown":
                        unknown+=1
                    else:
                        valid.append(result["result"]=="supported")
            found.setdefault(str(row["id"]),{}).update(citation_accuracy=sum(valid)/len(valid) if valid else None,
                effective_citations=sum(valid),unresolved_citations=unknown)
    elif kind=="drb2":
        for path in sorted(output.glob("aggregate_*.csv")):
            metric=path.stem.removeprefix("aggregate_")
            for row in rows(path):
                key=str(next(iter(row.values())))
                if key in ("Average","AVG","average","Mean"): continue
                value=row.get("candidate")
                found.setdefault(key,{})[metric]=float(value) if value not in (None,"") else None
    elif kind=="researchrubrics":
        for row in read(output/"rubrics.json"):
            found[row["id"]]={"compliance":row["compliance"]}
    elif kind=="deer":
        for path in output.glob("deer/*/candidate/final/*.json"):
            found[f"{path.parents[2].name}.{int(path.stem)}"]=read(path)["score_avgs"]
    elif kind=="ragtime":
        for path in output.glob("argue-*.scores.tsv"):
            for row in rows(path):
                if row["request_id"]!="all":
                    found.setdefault(row["request_id"],{})[row["metric"]]=float(row["value"])
    elif kind=="trec-rag":
        for row in rows(output/"support-metrics.jsonl"):
            found[str(row["topic_id"])]={k:v for k,v in row.items() if isinstance(v,(int,float))}
        for row in rows(output/"nuggets/metrics/cell_metrics.csv"):
            found.setdefault(str(row["qid"]),{}).update({k:float(row[k]) if row[k] else None for k in
                ("strict_vital_score","strict_all_score","vital_score","all_score","failed_count")})
    return found


def execute(command, log):
    """Run trusted upstream code, keeping credential values out of retained output."""
    secret_values=[v for k,v in os.environ.items() if v and re.search(r"(KEY|TOKEN|SECRET|PASSWORD)$",k)]
    with subprocess.Popen(command["argv"],cwd=command["cwd"],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True) as process:
        try:
            with Path(log).open("w") as stream:
                for line in process.stdout:
                    for value in secret_values:
                        line=line.replace(value,"[REDACTED]")
                    stream.write(line)
            code=process.wait()
        except BaseException:
            process.terminate()
            process.wait()
            raise
    if code:
        raise RuntimeError(f"upstream evaluator failed (exit {code}); inspect redacted log {log}")


def run(args):
    suite,evaluation=read(args.suite),read_evaluation(args.evaluation)
    measurements=[m for m in evaluation["measurements"] if m["arm"]==args.arm]
    if not measurements:
        raise ValueError("selected arm has no measurements")
    if len({m["case"] for m in measurements}) != len(measurements):
        raise ValueError("run upstream evaluation separately for each repeat; duplicate task IDs would overwrite reports")
    by_id={t["id"]:t for t in suite["tasks"]}
    tasks=[by_id[m["case"]] for m in measurements]
    if any("source_record" not in t for t in tasks):
        raise ValueError("re-prepare this suite from its original release to retain the upstream input fields")
    answers={m["research_id"]:(Path(args.evaluation).parent/f"{file_id(m['research_id'])}.md").read_text() for m in measurements}
    output=Path(args.output).resolve()
    output.mkdir(parents=True,exist_ok=False)
    commands=recipe(suite["benchmark"],args.checkout,output,tasks,measurements,answers,args)
    revision="published-prompt-api-adapter" if suite["benchmark"]=="deepsearchqa" else subprocess.run(["git","-C",str(Path(args.checkout).resolve()),"rev-parse","HEAD"],check=True,capture_output=True,text=True).stdout.strip()
    for command in commands:
        entry=command["argv"][5]
        if entry != "-m" and not Path(entry).is_file():
            raise ValueError(f"upstream entry point is absent: {entry}")
    if suite["benchmark"] == "deepsearchqa":
        template=Path(args.autorater_prompt).read_text()
        revision="published-prompt-"+hashlib.sha256(template.encode()).hexdigest()
        template.format(prompt="question",prompt_type="Set Answer",answer="reference",response="candidate")
        (output/"autorater-prompt.txt").write_text(template)
        commands[0]["argv"][8]=str(output/"autorater-prompt.txt")
    if suite["benchmark"] == "drb1" and not (Path(args.checkout)/"data/test_data/cleaned_data/reference.jsonl").is_file():
        raise ValueError("RACE requires its published reference.jsonl in the upstream checkout")
    write(output/"recipe.json",dict(benchmark=suite["benchmark"],upstream_revision=revision,commands=commands,
        note="Upstream execution may use paid providers and upstream retry policies. Prepared inputs alone are not evaluated results."))
    if not args.execute:
        print(f"Prepared upstream inputs and recipe: {output/'recipe.json'}. No evaluator or paid API was invoked.")
        return
    for i,command in enumerate(commands):
        execute(command,output/f"stage-{i}.log")
    metrics=collect(suite["benchmark"],output,args.checkout)
    names=sorted({k for m in metrics.values() for k in m})
    if not names:
        raise ValueError("upstream produced no recognized per-task scores; raw results retained")
    records=[]
    for m in measurements:
        values=metrics.get(m["case"],{})
        records.append({**m,"arm":agent_label(evaluation,m),"metrics":{k:values.get(k) for k in names},
            "judgment":{"consequential_errors":[],"unresolved":["Inspect upstream judgments; missing scores remain null."]}})
    write(output/"scores.json",dict(benchmark=suite["benchmark"],release=suite["release"],population=suite["population"],
        environment=evaluation.get("environment","unspecified"),mode=evaluation["mode"],protocol=f"upstream-adapter-v1-{revision}"+(f"-{args.provider}" if suite["benchmark"]=="ragtime" else ""),
        judge_model=args.judge,judge_environment=args.judge_environment,qualification="upstream evaluator adapter; inspect recipe and input conversion; not an official submission",
        source_evaluation=str(Path(args.evaluation).resolve()),records=records))
    print(f"Upstream scores: {output/'scores.json'}")
