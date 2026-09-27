"""A baseline using public releases and the existing research runtime only."""
import argparse
import json
import random
import shutil
import urllib.request
from pathlib import Path
from . import data, runner, report

TREC_REV = "a6255c10119a2984a874f46172d94045168ab1f3"
TREC = f"https://raw.githubusercontent.com/TREC-RAG/trec-rag-data/{TREC_REV}/trec-rag-2026/development-data"
DRB_REV = "b38f360603db9531b102aef8c166cedb8509b6f6"
DSQA_REV = "b2623f8653065c2672de6d941fc5434cd652376c"
REFLECT_REV = "21c43e620844ed2150addb797d001a3e705a9dd0"
SOURCES = {
    "deepsearchqa": (f"https://huggingface.co/datasets/google/deepsearchqa/resolve/{DSQA_REV}/DSQA-full.csv", DSQA_REV, None),
    "drb2": (f"https://raw.githubusercontent.com/imlrz/DeepResearch-Bench-II/{DRB_REV}/tasks_and_rubrics.jsonl", DRB_REV, None),
    "trec-rag": (TREC+"/rag25-dev-nuggets/rag25-dev-nuggets.jsonl", TREC_REV, TREC+"/topics/rag25-topics-dev.tsv"),
    "researchrubrics": (TREC+"/researchrubrics-dev-rubrics/research-rubrics-dev-rubrics.jsonl", TREC_REV, TREC+"/topics/research-rubrics-topics-dev.tsv"),
}
REFLECT = f"https://raw.githubusercontent.com/LWang-Laura/REFLECT/{REFLECT_REV}/output-level-baselines/data/holistic_200cases.jsonl"


def add_arguments(commands, positive):
    p = commands.add_parser("baseline", help="download public data, compare agents, grade and report with the existing account")
    p.add_argument("output")
    p.add_argument("--prepare-only", action="store_true", help="download and select real tasks without consuming model capacity")
    p.add_argument("--tasks-per-benchmark", type=positive, default=2)
    p.add_argument("--seed", type=int, default=7)
    p.add_argument("--repeats", type=positive, default=1)
    p.add_argument("--configuration", default="subscription-perspective")
    p.add_argument("--policy", choices=["staged", "evidence_access", "adaptive", "perspective", "question_driven", "multi_agent"], default="perspective")
    p.add_argument("--seconds", type=positive, default=1200)
    p.add_argument("--tool-calls", type=positive, default=80)
    q = commands.add_parser("score-baseline", help="grade saved baseline answers without repeating research")
    q.add_argument("baseline")
    q.add_argument("output")
    for sub in (p, q):
        sub.add_argument("--binary", default="target/debug/mcp-deepresearch")
        sub.add_argument("--judge-seconds", type=positive, default=600)
        sub.add_argument("--judge-tool-calls", type=positive, default=24)


def download(url, target):
    # Only the pinned public release URLs above are used; no provider credential.
    with urllib.request.urlopen(url, timeout=60) as response, target.open("wb") as output:
        shutil.copyfileobj(response, output)


def select_reflect(samples):
    # Published defect rows can reuse the same whole-report answer pair.
    chosen, traces, pairs = {}, set(), set()
    for row in samples:
        pair = (row["whole_original_answer"], row["whole_perturbed_answer"])
        defect = row["perturbation_type"]
        if defect not in chosen and row["trace_id"] not in traces and pair not in pairs:
            chosen[defect] = row
            traces.add(row["trace_id"])
            pairs.add(pair)
    return list(chosen.values())


def prepare(root, args):
    raw = root / "data"
    raw.mkdir()
    manifest = {"environment": "open-web", "selection_seed": args.seed, "benchmarks": {},
                "note": "Public development tasks, adapted subscription grading. TREC uses open-web research, not a corpus-restricted official score."}
    for kind, (url, revision, topics_url) in SOURCES.items():
        source = raw / (kind + (".csv" if kind == "deepsearchqa" else ".jsonl"))
        download(url, source)
        topics_path, topics = None, None
        if topics_url:
            topics_path = raw / (kind + "-topics.tsv")
            download(topics_url, topics_path)
            topics = dict(line.rstrip("\n").split("\t", 1) for line in topics_path.read_text().splitlines() if line.strip())
        tasks = data.import_tasks(kind, source, topics=topics)
        available = len(tasks)
        random.Random(args.seed).shuffle(tasks)
        if kind == "deepsearchqa" and args.tasks_per_benchmark >= 2:
            # Exercise both answer-set completion and unique-answer questions.
            first = [next(t for t in tasks if t["reference"]["answer_type"] == kind_)
                     for kind_ in ("Set Answer", "Single Answer")]
            tasks = first + [t for t in tasks if t not in first]
        ids = [t["id"] for t in tasks[:args.tasks_per_benchmark]]
        data.prepare(argparse.Namespace(benchmark=kind, input=source, output=root/kind,
            release=revision, ids=ids, allow_noncommercial=False, topics=topics_path,
            environment="open-web", policy=args.policy, seconds=args.seconds, tool_calls=args.tool_calls))
        manifest["benchmarks"][kind] = {"ids": ids, "available_tasks": available, "source": url, "topics": topics_url, "release": revision}
    download(REFLECT, raw/"reflect.jsonl")
    samples = data.rows(raw/"reflect.jsonl")
    chosen = select_reflect(samples)
    (raw/"reflect-selected.jsonl").write_text("".join(json.dumps(row)+"\n" for row in chosen))
    manifest["qualification"] = {"source": REFLECT, "release": REFLECT_REV, "pairs": len(chosen)}
    data.write(root/"baseline.json", manifest)
    return manifest


def score_cells(root, destination, kind, args, scores):
    for evaluation in sorted((root/"research"/kind).glob("run-*/evaluation.json")):
        output = destination/kind/evaluation.parent.name
        output.parent.mkdir(parents=True, exist_ok=True)
        try:
            runner.grade(argparse.Namespace(suite=root/kind/"suite.json", evaluation=evaluation, output=output,
                binary=args.binary, seconds=args.judge_seconds, tool_calls=args.judge_tool_calls,
                batch_size=20, judge_environment="open-web", qualification="See baseline qualification.json; unfinished diagnostics are not a pass."))
        finally:
            if (output/"scores.json").exists():
                scores.append(output/"scores.json")


def summarize(root, output, scores):
    experiments = sorted((root/"research").glob("*/experiment.json"))
    if scores:
        report.report(argparse.Namespace(output=output/"summary.json", scores=scores, experiments=experiments))
    manifest = data.read(root/"baseline.json")
    data.write(output/"coverage.json", {"selected_tasks": {k:len(v["ids"]) for k,v in manifest["benchmarks"].items()},
        "scored_files": len(scores), "note": "An absent summary or score is unfinished work, never zero quality. Check baseline.json and experiment.json for assigned work."})


def run(args):
    root = Path(args.output).resolve()
    root.mkdir(parents=True, exist_ok=False)
    manifest = prepare(root, args)
    if args.prepare_only:
        print(f"Public baseline prepared at {root}; no model calls made.")
        return
    scores = []
    try:
        for kind in manifest["benchmarks"]:
            runner.run(argparse.Namespace(cases=root/kind/"cases.json", output=root/"research"/kind,
                repeats=args.repeats, mode="live", configuration=args.configuration, environment="open-web", binary=args.binary))
            score_cells(root, root/"grades", kind, args, scores)
        runner.qualify(argparse.Namespace(input=root/"data/reflect-selected.jsonl", output=root/"qualification",
            release=REFLECT_REV, ids=None, family="holistic", limit=manifest["qualification"]["pairs"],
            binary=args.binary, seconds=args.judge_seconds, tool_calls=args.judge_tool_calls))
    finally:
        summarize(root, root, scores)


def score(args):
    root, output = Path(args.baseline).resolve(), Path(args.output).resolve()
    manifest = data.read(root/"baseline.json")
    output.mkdir(parents=True, exist_ok=False)
    scores = []
    try:
        for kind in manifest["benchmarks"]:
            score_cells(root, output/"grades", kind, args, scores)
        if not scores:
            raise ValueError("no saved baseline answers are available to score")
    finally:
        summarize(root, output, scores)
