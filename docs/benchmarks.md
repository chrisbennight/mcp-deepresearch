# Runnable public-data baseline

The baseline uses public releases, the configured Codex subscription, and existing
source-gateway access. Research, grading, and grader diagnostics all use the same
runtime. There are no external evaluator environments or additional model-provider
keys to configure.

## Included evaluations

| Public release | What we measure |
| --- | --- |
| [DeepSearchQA](https://huggingface.co/datasets/google/deepsearchqa) | Answer-set precision, recall, F1 and completeness; both single and set questions |
| [DeepResearch Bench II](https://github.com/imlrz/DeepResearch-Bench-II) | Information recall, analysis, presentation and reliance on prohibited reference reports |
| [TREC RAG25 development nuggets](https://github.com/TREC-RAG/trec-rag-data/tree/main/trec-rag-2026/development-data/rag25-dev-nuggets) | Essential and all-nugget coverage, with strict and partial credit |
| [TREC ResearchRubrics development release](https://github.com/TREC-RAG/trec-rag-data/tree/main/trec-rag-2026/development-data/researchrubrics-dev-rubrics) | Weighted task requirements, including penalties for undesirable behavior |
| [REFLECT holistic release](https://github.com/LWang-Laura/REFLECT) | Whether the grader notices published defects in both presentation orders |

These are **adapted evaluations**: published tasks and criteria with our subscription
runtime and grading prompts. The TREC task set uses open-web research through the
configured gateway. It does not claim an official corpus-restricted TREC score.
REFLECT measures grader sensitivity, not agent quality. Source checks report concrete
errors and unresolved evidence separately; nugget coverage is not proof of grounding.

The baseline fetches pinned public revisions automatically. It excludes DRB II tasks
with noncommercial-only terms by default. The ResearchRubrics input is the public TREC
development subset; there is no gated dataset prerequisite. Downloaded references and
results stay in the private output directory and must not be committed to this repo.

## Run it

Build the Rust binary using the README checks and configure the existing
[Codex runtime](codex-runtime.md) with source-gateway access. Then:

```sh
uv run --no-project python scripts/benchmark.py baseline /private/eval/run
```

This downloads the data, selects two tasks from each scored dataset, compares the
selected research policy against the single-session control, grades saved answers,
and creates `summary.json` and `summary.md`. It then checks one REFLECT pair per defect
in both orders. The research conditions are matched between agents and their execution
order alternates. Default limits are 1,200 seconds and 80 source calls per research
attempt; grading allows 600 seconds and 24 calls per assessment. Grading can inspect
sources and does not receive the agent identity or elapsed time.

Two tasks per dataset is a small initial sample, not enough to establish a general
winner. Set `--tasks-per-benchmark`, `--repeats`, `--seed`, and `--configuration` for a
larger comparison. `--policy` selects the workflow under test. `--seconds` and
`--tool-calls` control research; `--judge-seconds` and `--judge-tool-calls` control grading.
Longer runs get their own directory and configuration label. Time is reported separately
and is never deducted from quality scores.

To download and inspect the actual data without consuming model capacity:

```sh
uv run --no-project python scripts/benchmark.py baseline /private/eval/prepared \
  --prepare-only
```

`baseline.json` records the selected IDs, available populations and source revisions.
Each dataset directory separates `cases.json` (researcher questions) from `suite.json`
(grading references). Only cases go to the researcher. Raw public downloads are in
`data/`. Keep the output directory outside any research worker's allowed file access.

## Interrupted work and saved answers

Subscription capacity is finite. A failed operation exits unsuccessfully; completed
research and judgments remain available. `coverage.json` records the intended datasets,
while experiment files record assigned attempts and scorecards show observed/scored
counts. Unstarted or unresolved work is not silently scored zero. A partial
`qualification/qualification.json` is not a completed diagnostic.

To grade already saved baseline answers without repeating research:

```sh
uv run --no-project python scripts/benchmark.py score-baseline \
  /private/eval/run /private/eval/rescored
```

Use a new output directory for each scoring attempt. The standalone `run`, `grade`,
`qualify`, and `report` commands also remain available for a selected dataset or saved
run. For example, a prepared TREC task set can be run directly:

```sh
uv run --no-project python scripts/benchmark.py run \
  /private/eval/prepared/trec-rag/cases.json /private/eval/trec-run \
  --mode live --configuration perspective-1200s --environment open-web
```

The baseline does not silently switch providers, retry paid calls, or replace an old
attempt with a better one. A preparation-only run demonstrates data access, not research
quality. Ordinary tests simulate the external runtime and do not spend account capacity.

## Read the results

Each dataset retains its own metrics, judge configuration and source conditions.
Repeated attempts are averaged within tasks. Paired differences use tasks scored in
both arms; intervals resample tasks, not rubric rows. Inspect missingness before
interpreting an apparent winner. Related tasks across releases are not independent
corroboration. No universal cross-benchmark composite is calculated.

Rubric judgments include reasons and evidence. Graders inspect consequential claims,
citations and qualifications and can report faulty references as unresolved. Those
model judgments are useful checks, not guarantees of correctness. An empty error list
is not a measured factual-support percentage.

## Research sources

- [DeepSearchQA paper](https://arxiv.org/abs/2601.20975).
- [DeepResearch Bench II paper](https://arxiv.org/abs/2601.08536).
- [ResearchRubrics paper](https://arxiv.org/abs/2511.07685).
- [AutoNuggetizer paper](https://arxiv.org/abs/2411.09607), [TREC RAG 2025](https://trec-rag.github.io/trec25/).
- [REFLECT paper](https://arxiv.org/abs/2605.19196).

Paid-provider evaluators, gated-data integrations, native score import/export and
report-conversion machinery have been removed from this tooling. The scope is a
baseline we can actually operate with existing access. Track work in the
[benchmark epic](https://github.com/chrisbennight/mcp-deepresearch/issues/25).
