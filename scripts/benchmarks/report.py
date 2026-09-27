"""Native scorecards, task-balanced paired differences, and explicit missingness."""
import itertools
import random
from collections import defaultdict
from statistics import mean
from pathlib import Path
from .data import read, write


def interval(values):
    if len(values) < 2:
        return None
    rng = random.Random(7)
    samples = sorted(mean(rng.choices(values, k=len(values))) for _ in range(2000))
    return [samples[49], samples[1949]]


def summarize(reports):
    groups = defaultdict(list)
    seen = set()
    for report in reports:
        identity = tuple(str(report.get(k, "")) for k in ("benchmark", "release", "protocol", "judge_model", "judge_environment", "judge_source_tools", "judge_effort", "judge_seconds", "judge_tool_calls", "batch_size", "environment", "mode"))
        for record in report["records"]:
            key = (identity, record["research_id"])
            if key in seen:
                raise ValueError("duplicate scored answer; select one judgment recipe per report")
            seen.add(key)
            groups[identity].append(record)
    results = []
    for identity, records in groups.items():
        metrics = sorted({name for r in records for name in r["metrics"]})
        arms = sorted({r["arm"] for r in records})
        summary, paired = [], []
        for arm in arms:
            rs = [r for r in records if r["arm"] == arm]
            for metric in metrics:
                task_values = defaultdict(list)
                for r in rs:
                    value = r["metrics"].get(metric)
                    if value is not None:
                        task_values[r["case"]].append(value)
                summary.append({"arm": arm, "metric": metric, "task_macro_mean": mean(mean(v) for v in task_values.values()) if task_values else None,
                    "scored_tasks": len(task_values), "observed_tasks": len({r["case"] for r in rs}), "attempts": len(rs),
                    "scored_attempts": sum(r["metrics"].get(metric) is not None for r in rs),
                    "missing_metric_attempts": sum(r["metrics"].get(metric) is None for r in rs),
                    "completed": sum(r["outcome"] == "completed" for r in rs), "mean_seconds": mean(r["elapsed_ms"]/1000 for r in rs),
                    "errors": sum(len(r["judgment"]["consequential_errors"]) for r in rs),
                    "unresolved": sum(len(r["judgment"]["unresolved"]) for r in rs)})
        for a, b in itertools.combinations(arms, 2):
            for metric in metrics:
                av, bv = defaultdict(list), defaultdict(list)
                for r in records:
                    value = r["metrics"].get(metric)
                    if value is not None and r["arm"] in (a, b):
                        (av if r["arm"] == a else bv)[r["case"]].append(value)
                common = sorted(av.keys() & bv.keys())
                differences = [mean(av[k])-mean(bv[k]) for k in common]
                paired.append({"metric": metric, "difference": f"{a} minus {b}", "paired_tasks": len(common),
                    "missing_metric_attempts": {arm: sum(r["metrics"].get(metric) is None for r in records if r["arm"] == arm) for arm in (a, b)},
                    "mean_difference": mean(differences) if differences else None, "task_bootstrap_95_interval": interval(differences),
                    "note": "Conditional on tasks scored in both arms; inspect missingness before interpreting. Repeats averaged within task. Cross-benchmark overlap is not independent evidence."})
        results.append({"identity": dict(zip(("benchmark", "release", "protocol", "judge_model", "judge_environment", "judge_source_tools", "judge_effort", "judge_seconds", "judge_tool_calls", "batch_size", "environment", "mode"), identity)), "summary": summary, "paired": paired, "records": records})
    return results


def report(args):
    inputs = [read(path) for path in args.scores]
    result = {"scorecards": summarize(inputs), "coverage": [{k: r.get(k) for k in ("benchmark", "release", "population", "qualification", "source_evaluation")} for r in inputs],
              "note": "No cross-benchmark composite: scales differ. Quality is not time-discounted. Unknown scores stay missing; native and adapted recipes never share an average.", "experiments": []}
    scored = {r["research_id"] for s in inputs for r in s["records"] if r.get("graded", True)}
    for experiment in args.experiments:
        p = Path(experiment)
        manifest = read(p)
        observed = []
        for evaluation in p.parent.glob("run-*/evaluation.json"):
            observed.extend(read(evaluation)["measurements"])
        result["experiments"].append({"configuration": manifest["configuration"], "environment": manifest["environment"],
            "assigned_attempts": len(manifest["cases"])*manifest["repeats"]*2,
            "observed_attempts": len(observed), "graded_attempts": sum(m["research_id"] in scored for m in observed),
            "completed_attempts": sum(m["outcome"] == "completed" for m in observed), "manifest": str(p)})
    capabilities = {"information_coverage": [], "analysis_and_synthesis": [], "source_support": [], "request_fit": []}
    for card in result["scorecards"]:
        for item in card["summary"]:
            metric = item["metric"].lower()
            dimension = None
            if metric in {"info_recall", "inforecall", "recall", "strict_vital", "strict_all", "vital", "all", "strict_vital_score", "strict_all_score", "vital_score", "all_score", "nugget_coverage", "nugget_coverage_weighted", "comprehensiveness"}:
                dimension = "information_coverage"
            elif "analysis" in metric or "synthesis" in metric or metric == "insight":
                dimension = "analysis_and_synthesis"
            elif metric in {"citation_support", "sentence_support", "source_support", "factual_support", "verification", "citation_accuracy", "hard_precision", "hard_recall", "weighted_precision_first", "weighted_precision_all", "weighted_recall_first", "weighted_recall_all"}:
                dimension = "source_support"
            elif metric in {"presentation", "compliance"} or "instruction" in metric:
                dimension = "request_fit"
            if dimension and item["task_macro_mean"] is not None:
                capabilities[dimension].append({"benchmark": card["identity"]["benchmark"], "scorecard": card["identity"], **item})
    result["capabilities"] = capabilities
    write(args.output, result)
    lines = ["# Research capability scorecard", "", result["note"], "",
        "Coverage: TREC/DRB II recall and answer-set recall measure information found; rubric analysis measures synthesis; citation/support results measure evidence separately. Nugget presence alone is not source support. REFLECT is a grader diagnostic, not agent quality.", "",
        "Scores below are conditional on observed judgments. See assigned/observed/graded counts and the declared population before drawing a winner. A small sample is a demonstration, not a general ranking.", ""]
    lines.extend(["## Capability coverage", "", "| Capability | Available measurements |", "| --- | --- |"])
    for dimension, measurements in capabilities.items():
        names = sorted({m["benchmark"] + " / " + m["scorecard"]["protocol"] + " / " + m["scorecard"]["mode"] + ": " + m["metric"] for m in measurements})
        lines.append(f"| {dimension.replace('_', ' ')} | {', '.join(names) if names else 'Not measured in this report'} |")
    lines.append("")
    for coverage in result["coverage"]:
        lines.append(f"Declared population for {coverage['benchmark']}: {len(coverage['population'])} tasks. Grader qualification: {coverage['qualification']}.")
    for e in result["experiments"]:
        lines.append(f"Experiment {e['configuration']}: assigned {e['assigned_attempts']}, observed {e['observed_attempts']}, graded {e['graded_attempts']}, completed {e['completed_attempts']} attempts.")
    for card in result["scorecards"]:
        lines.extend(["", "## " + " / ".join(card["identity"].values()), "", "| Agent | Metric | Task mean | Scored/observed tasks | Scored metric/attempts | Completed/attempts | Mean seconds | Error findings | Unresolved findings |", "| --- | --- | --- | --- | --- | --- | --- | --- | --- |"])
        for s in card["summary"]:
            value = "unresolved" if s["task_macro_mean"] is None else f"{s['task_macro_mean']:.4f}"
            lines.append(f"| {s['arm']} | {s['metric']} | {value} | {s['scored_tasks']}/{s['observed_tasks']} | {s['scored_attempts']}/{s['attempts']} | {s['completed']}/{s['attempts']} | {s['mean_seconds']:.1f} | {s['errors']} | {s['unresolved']} |")
        lines.extend(["", "Paired differences average repeated attempts within each task. Intervals resample tasks, not criteria; no interval is claimed for a single task.", ""])
        for p in card["paired"]:
            lines.append(f"- {p['metric']}: {p['difference']}; difference {p['mean_difference']}; tasks {p['paired_tasks']}; interval {p['task_bootstrap_95_interval']}; missing metric attempts by arm {p['missing_metric_attempts']}.")
    Path(args.output).with_suffix(".md").write_text("\n".join(lines)+"\n")
