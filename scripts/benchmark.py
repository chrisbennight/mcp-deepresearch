#!/usr/bin/env python3
"""Offline-first published benchmark workflow. Run through uv; no extra packages."""
import argparse
import json
import subprocess
import sys
from benchmarks import data, runner, report, baseline


def positive(value):
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return number


def main():
    p = argparse.ArgumentParser(description=__doc__)
    commands = p.add_subparsers(dest="command", required=True)
    commands.add_parser("catalog")
    prepare = commands.add_parser("prepare")
    prepare.add_argument("benchmark", choices=["drb2", "researchrubrics", "deepsearchqa", "trec-rag"])
    prepare.add_argument("input")
    prepare.add_argument("output")
    prepare.add_argument("--release", required=True)
    prepare.add_argument("--ids", nargs="+")
    prepare.add_argument("--topics", help="TREC topic TSV for nugget releases without query text")
    prepare.add_argument("--allow-noncommercial", action="store_true", help="operator confirms this use satisfies applicable noncommercial terms")
    prepare.add_argument("--environment", required=True, help="open-web or the specific permitted corpus/retrieval environment")
    prepare.add_argument("--policy", choices=["staged", "evidence_access", "adaptive", "perspective", "question_driven", "multi_agent"], default="perspective")
    prepare.add_argument("--seconds", type=positive, default=1200)
    prepare.add_argument("--tool-calls", type=positive, default=80)
    run = commands.add_parser("run")
    run.add_argument("cases")
    run.add_argument("output")
    run.add_argument("--mode", choices=["fixture", "live"], required=True)
    run.add_argument("--repeats", type=positive, default=1)
    run.add_argument("--configuration", required=True, help="agent/model/budget configuration being compared; no secrets")
    run.add_argument("--environment", required=True)
    grade = commands.add_parser("grade")
    grade.add_argument("suite")
    grade.add_argument("evaluation")
    grade.add_argument("output")
    grade.add_argument("--batch-size", type=positive, default=20)
    grade.add_argument("--qualification", default="unqualified: no grader diagnostic supplied")
    grade.add_argument("--judge-environment", required=True, help="nonsecret logical source backend/corpus label for this grader")
    qualify = commands.add_parser("qualify")
    qualify.add_argument("input", help="REFLECT holistic JSONL obtained through authorized distribution")
    qualify.add_argument("output")
    qualify.add_argument("--release", required=True)
    qualify.add_argument("--ids", nargs="+")
    qualify.add_argument("--family", choices=["holistic", "chunk", "reasoning", "tool-use"], default="holistic")
    qualify.add_argument("--limit", type=positive, default=3)
    for sub in (run, grade, qualify):
        sub.add_argument("--binary", default="target/debug/mcp-deepresearch")
    for sub in (grade, qualify):
        sub.add_argument("--seconds", type=positive, default=600)
        sub.add_argument("--tool-calls", type=positive, default=24)
    baseline.add_arguments(commands, positive)
    result = commands.add_parser("report")
    result.add_argument("output")
    result.add_argument("--scores", nargs="+", required=True)
    result.add_argument("--experiments", nargs="*", default=[])
    args = p.parse_args()
    if args.command == "catalog":
        print(json.dumps(data.CATALOG, indent=2))
    else:
        {"prepare": data.prepare, "run": runner.run, "grade": runner.grade,
         "qualify": runner.qualify, "report": report.report, "baseline": baseline.run, "score-baseline": baseline.score}[args.command](args)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Benchmark command failed: {error}", file=sys.stderr)
        sys.exit(1)
