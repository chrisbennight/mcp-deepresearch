"""Read published data without copying it into the public project.

Only the question and public source restrictions cross into research cases.
Grading references stay in the operator's assessment directory.
"""
import csv
import json
import re
from pathlib import Path


def read(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n")


def rows(path):
    path = Path(path)
    if path.suffix in (".csv", ".tsv"):
        with path.open(newline="") as f:
            return list(csv.DictReader(f, delimiter="\t" if path.suffix == ".tsv" else ","))
    if path.suffix == ".json":
        return read(path)
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def file_id(value):
    value = str(value)
    if not re.fullmatch(r"[\w.-]+", value) or value in (".", ".."):
        raise ValueError("task identifiers used as filenames must be simple names")
    return value


CATALOG = {
    "drb2": {"source": "https://github.com/imlrz/DeepResearch-Bench-II", "paper": "https://arxiv.org/abs/2601.08536", "metrics": ["info_recall", "analysis", "presentation", "total", "blocked_rate"], "access": "Public; per-task CC BY/CC BY-NC/CC0. Subscription grading is adapted."},
    "researchrubrics": {"source": "https://github.com/TREC-RAG/trec-rag-data/tree/main/trec-rag-2026/development-data/researchrubrics-dev-rubrics", "paper": "https://arxiv.org/abs/2511.07685", "metrics": ["compliance"], "access": "Public TREC ResearchRubrics development rubrics plus topic TSV; subscription grading."},
    "deepsearchqa": {"source": "https://huggingface.co/datasets/google/deepsearchqa", "paper": "https://arxiv.org/abs/2601.20975", "metrics": ["precision", "recall", "f1", "complete"], "access": "Apache-2.0 CSV. Local semantic matching uses the configured runtime, not the prescribed native autorater."},
    "trec-rag": {"source": "https://github.com/TREC-RAG/trec-rag-data/tree/main/trec-rag-2026/development-data/rag25-dev-nuggets", "paper": "https://arxiv.org/abs/2411.09607", "metrics": ["strict_vital", "strict_all", "vital", "all"], "access": "Public RAG25 development nuggets and topic TSV. Open-web baseline uses subscription grading; not an official corpus-restricted score."},
    "reflect": {"source": "https://github.com/LWang-Laura/REFLECT", "paper": "https://arxiv.org/abs/2605.19196", "metrics": ["defect_detection", "order_consistency"], "access": "Locally obtained holistic_200cases.jsonl. Grader diagnostic, excluded from agent scorecards; code is not vendored."},
}


def import_tasks(kind, path, ids=None, allow_noncommercial=False, topics=None):
    tasks = {}
    source_rows = rows(path)
    for n, row in enumerate(source_rows):
        if kind == "drb2":
            key, prompt = str(row["idx"]), row["prompt"]
            license_ = row["license"]
            if "NC" in license_.upper() and not allow_noncommercial:
                continue
            content = row["content"]
            if isinstance(content, str):
                content = json.loads(content)
            criteria = [{"id": f"{dim}-{i}", "text": text, "dimension": dim, "weight": 1}
                        for dim, items in content["rubric"].items() for i, text in enumerate(items)]
            blocked = content.get("blocked", {}).get("urls", [])
            reference = {"criteria": criteria, "blocked": blocked}
        elif kind == "researchrubrics":
            key = str(row["qid"])
            prompt, license_ = topics[key], "Public TREC development release; retain upstream terms"
            reference = {"criteria": [{"id": str(i), "text": r["criterion"], "dimension": r["axis"], "weight": r["weight"]} for i, r in enumerate(row["rubrics"])]}
        elif kind == "deepsearchqa":
            key, prompt, license_ = str(n), row["problem"], "Apache-2.0"
            reference = {"answer": row["answer"], "answer_type": row["answer_type"]}
        elif kind == "trec-rag":
            key, prompt, license_ = str(row["qid"]), row.get("query") or topics[str(row["qid"])], "Obtain rights for selected TREC release"
            reference = {"criteria": [{"id": str(i), "text": r["text"], "dimension": r["importance"], "weight": 1} for i, r in enumerate(row["nuggets"])]}
        else:
            raise ValueError("this component uses external scores or qualification, not task import")
        file_id(key)
        if ids and key not in ids:
            continue
        task = {"id": key, "prompt": prompt, "license": license_, "reference": reference}
        if key in tasks and tasks[key] != task:
            raise ValueError(f"conflicting reference for task {key}")
        tasks[key] = task
    if not tasks or (ids and set(tasks) != set(ids)):
        raise ValueError("selection is empty or requested IDs were absent/ineligible")
    return list(tasks.values())


def prepare(args):
    topics = None
    if args.topics:
        with Path(args.topics).open() as f:
            topics = dict(line.rstrip("\n").split("\t", 1) for line in f if line.strip())
    tasks = import_tasks(args.benchmark, args.input, args.ids, args.allow_noncommercial, topics)
    output = Path(args.output)
    output.mkdir(parents=True, exist_ok=False)
    suite = {"benchmark": args.benchmark, "release": args.release, "source": CATALOG[args.benchmark]["source"],
             "population": [t["id"] for t in tasks], "environment": args.environment, "tasks": tasks}
    write(output / "suite.json", suite)
    cases = []
    for task in tasks:
        constraints = ["Do not retrieve benchmark answers, grading rubrics or answer keys."]
        blocked = task["reference"].get("blocked", [])
        if blocked:
            constraints.append("Do not use these designated reference reports: " + json.dumps(blocked))
        if args.environment != "open-web":
            constraints.append("Use only the configured benchmark corpus search/read tools. No open-web search.")
        cases.append({"id": task["id"], "assess": [], "request": {"objective": task["prompt"],
            "policy": args.policy, "format": "report", "source_constraints": constraints,
            "limits": {"wall_seconds": args.seconds, "max_assignments": 16, "context_chars": 128000, "max_tool_calls": args.tool_calls}}})
    write(output / "cases.json", cases)
    print(f"Prepared {len(tasks)} tasks. Keep suite.json outside the research worker's accessible filesystem.")
