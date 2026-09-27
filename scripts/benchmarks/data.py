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
    "drb2": {"source": "https://github.com/imlrz/DeepResearch-Bench-II", "paper": "https://arxiv.org/abs/2601.08536", "metrics": ["info_recall", "analysis", "presentation", "total", "blocked_rate"], "access": "Public; per-task CC BY/CC BY-NC/CC0. Local grading is adapted; upstream grader is separate."},
    "researchrubrics": {"source": "https://github.com/scaleapi/researchrubrics", "paper": "https://arxiv.org/abs/2511.07685", "metrics": ["compliance"], "access": "Obtain gated processed_data.jsonl through its authorized distribution; MIT."},
    "deepsearchqa": {"source": "https://huggingface.co/datasets/google/deepsearchqa", "paper": "https://arxiv.org/abs/2601.20975", "metrics": ["precision", "recall", "f1", "complete"], "access": "Apache-2.0 CSV. Local semantic matching uses the configured runtime, not the prescribed native autorater."},
    "trec-rag": {"source": "https://trec.nist.gov/data/rag2024.html", "paper": "https://arxiv.org/abs/2411.09607", "metrics": ["strict_vital", "strict_all", "vital", "all"], "access": "TREC 2024 nugget_assignment.20241218.jsonl or RAGDoll nuggets JSONL. Corpus access and judgment rights are separate. Nugget presence is not source support."},
    "drb1": {"source": "https://github.com/Ayanami0730/deep_research_bench", "paper": "https://deepresearch-bench.github.io/", "metrics": ["RACE", "FACT"], "access": "Import query.jsonl; export reports for the upstream RACE/FACT evaluator. No local substitute for its reference-relative score."},
    "deer": {"source": "https://github.com/hanjanghoon/DEER", "paper": "https://arxiv.org/abs/2512.17776", "metrics": ["report_quality", "verification"], "access": "Conditional: noncommercial, no dataset redistribution. Import an authorized extracted data directory; upstream-run executes report and verification evaluation."},
    "ragtime": {"source": "https://github.com/hltcoe/auto-argue", "paper": "https://arxiv.org/abs/2509.26184", "metrics": ["nugget_coverage", "sentence_support", "citation_support", "f1"], "access": "Released ARGUE v3 nugget banks, matching corpus and report requests. Register for restricted track material. Upstream-run uses Auto-ARGUE."},
    "reflect": {"source": "https://github.com/LWang-Laura/REFLECT", "paper": "https://arxiv.org/abs/2605.19196", "metrics": ["defect_detection", "order_consistency"], "access": "Locally obtained holistic_200cases.jsonl. Grader diagnostic, excluded from agent scorecards; code is not vendored."},
}


def import_tasks(kind, path, ids=None, allow_noncommercial=False, topics=None):
    tasks = {}
    if kind == "deer":
        if not allow_noncommercial:
            raise ValueError("DEER requires --allow-noncommercial and an authorized extracted dataset")
        source_rows = []
        for query in sorted(Path(path).glob("*/*/query.md")):
            if not query.parent.name.isdecimal():
                continue
            relative = query.parent.relative_to(path)
            sample = str(int(relative.parts[1]))
            source_rows.append(dict(id=relative.parts[0]+"."+sample, prompt=query.read_text(),
                core_criteria=(query.parent/"core_criteria.md").read_text(),
                domain=relative.parts[0], sample=sample))
    elif kind == "ragtime":
        paths = sorted(Path(path).glob("*.v3.json")) if Path(path).is_dir() else [Path(path)]
        source_rows = [read(p) for p in paths]
    else:
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
            key, prompt, license_ = str(row["sample_id"]), row["prompt"], "MIT"
            reference = {"criteria": [{"id": str(i), "text": r["criterion"], "dimension": r["axis"], "weight": r["weight"]} for i, r in enumerate(row["rubrics"])]}
        elif kind == "deepsearchqa":
            key, prompt, license_ = str(n), row["problem"], "Apache-2.0"
            reference = {"answer": row["answer"], "answer_type": row["answer_type"]}
        elif kind == "trec-rag":
            key, prompt, license_ = str(row["qid"]), row.get("query") or topics[str(row["qid"])], "Obtain rights for selected TREC release"
            reference = {"criteria": [{"id": str(i), "text": r["text"], "dimension": r["importance"], "weight": 1} for i, r in enumerate(row["nuggets"])]}
        elif kind == "deer":
            key, prompt, license_ = row["id"], row["prompt"], "DEER noncommercial; no redistribution"
            reference = {"core_criteria": row["core_criteria"], "domain": file_id(row["domain"]), "sample": file_id(row["sample"])}
        elif kind == "ragtime":
            key, prompt, license_ = str(row["query_id"]), row["full_query"], "Obtain rights for selected ARGUE/RAGtime release"
            if row.get("full_background"):
                prompt += "\n\n" + row["full_background"]
            reference = {"nugget_bank": row["nugget_bank"], "source_record": row}
        elif kind == "drb1":
            key, prompt, license_ = str(row["id"]), row["prompt"], "Apache-2.0 dataset"
            reference = {}
        else:
            raise ValueError("this component uses external scores or qualification, not task import")
        file_id(key)
        if ids and key not in ids:
            continue
        task = {"id": key, "prompt": prompt, "license": license_, "reference": reference, "source_record": row}
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
