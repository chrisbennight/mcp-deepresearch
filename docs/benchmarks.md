# Published research benchmarks

Use the benchmark commands to compare research configurations on published tasks,
without asking people to author a new reference set. The Python command uses only the
standard library for dataset conversion and reporting; all model work goes through the
Rust runtime. Ordinary tests require no model account, datasets or API credentials.

This is a collection of distinct evaluations, not a universal research score. Preserve
native results from upstream evaluators. The built-in grading path substitutes our
runtime and prompts and is always labeled **adapted**, even when metric arithmetic
follows the publication. A local run is not an official leaderboard submission.

## Available paths

| Component | Input and supported behavior | What it tells us |
| --- | --- | --- |
| DeepResearch Bench II | Released `tasks_and_rubrics.jsonl`; prepare, run, adapted rubric grading, upstream Markdown export | Information recall versus analysis versus presentation; blocked-reference rate |
| ResearchRubrics | Authorized `processed_data.jsonl`; prepare, run, adapted weighted grading, upstream Markdown export | Task-specific explicit/implicit requirements and synthesis; negative-weight penalties retained |
| DeepSearchQA | Official `DSQA-full.csv`; prepare, run, adapted semantic answer matching | Answer-set precision, recall, F1 and completeness |
| TREC RAG | TREC 2024 nugget assignments, RAGDoll nugget JSONL, or RAG 2026 development nuggets with topic TSV | Essential and all-nugget coverage, strict and partial credit; presence is not citation support |
| DeepResearch Bench I | Released `query.jsonl`; prepare, run, upstream report export and score import | Keep upstream RACE report quality and FACT source assessment separate |
| DEER | Upstream evaluation and flat per-task score import using a locally prepared suite/evaluation | Optional report and source verification; no automated dataset importer or data redistribution |
| REFLECT | Authorized holistic JSONL; order-swapped diagnostic through the runtime | Whether a judge notices published defects; not an agent-quality score |

The command's `catalog` includes source and paper links. Dataset access and code rights
are separate. ResearchRubrics is gated. DRB II excludes noncommercial tasks unless
`--allow-noncommercial` confirms the operator's use is eligible. DEER's data is
noncommercial and cannot be redistributed. Do not commit downloaded tasks, reference
answers or private run outputs. No upstream evaluation packages are installed or paid
APIs called implicitly.

## Prepare and compare

Build the Rust binary as described in the README. Obtain datasets from the links below
through their authorized distribution. Pin a release or source revision in `--release`.
Select explicit IDs for a bounded experiment; omit `--ids` only when the full eligible
population is intended. DeepSearchQA IDs are zero-based CSV row numbers for that release.

```sh
uv run --no-project python scripts/benchmark.py catalog
uv run --no-project python scripts/benchmark.py prepare deepsearchqa \
  /private/data/DSQA-full.csv /private/eval/dsqa \
  --release YOUR_DATASET_REVISION --environment open-web --ids 0 1 2
```

The result separates `cases.json` (questions and public source restrictions) from
`suite.json` (assessment references). Only cases reach the researcher. Keep the suite
and assessment directories outside the worker's accessible filesystem. The existing
Codex adapter disables shell and built-in web search and exposes configured source tools;
this is not a promise to prevent every form of public-web benchmark contamination.

Configure the existing [Codex runtime](codex-runtime.md), including Kagi source access.
A run compares the selected research policy against the ordinary single-session agent,
using matched limits and alternating order. All agent work consumes the configured
account. Use `--configuration` to distinguish model, policy and budget conditions.

```sh
uv run --no-project python scripts/benchmark.py run /private/eval/dsqa/cases.json \
  /private/eval/experiment --mode live --repeats 2 \
  --configuration model-effort-policy-1200s --environment open-web
```

Each task/repeat has its own `run-*/evaluation.json` and saved Markdown answers.
`experiment.json` records the entire assigned population, including work not reached
if execution stops. A new directory is required for retries or longer allowances.
Do not delete the original attempt or grade only successful survivors. Fixture mode
checks execution mechanics and remains labeled fixture; it is not a quality study.

## Grade without a new model API

```sh
uv run --no-project python scripts/benchmark.py grade /private/eval/dsqa/suite.json \
  /private/eval/experiment/run-0-0/evaluation.json /private/eval/assessment \
  --seconds 600 --tool-calls 24 --judge-environment kagi-open-web
```

Use a nonsecret logical `--judge-environment` label for the actual grading source
backend and collection. Source tool names are also recorded for adapted grading;
never put credentials or credential-bearing endpoint URLs in a label.
This runs fresh assessments without agent labels or timing. Graders can read sources,
check extra consequential claims and report unresolved evidence. Rubric inputs are
batched to bound individual assignments, with every published criterion accounted for.
Scoring uses the delivered answer, not a replacement answer written by the grader.
A null score remains unresolved rather than becoming zero or disappearing. A failed
assessment stops that command; completed judgments and sessions remain available.
Recovery uses a new directory and explicitly selected remaining measurements. There
is no automatic retry that silently consumes account capacity.

Local grading is a useful development instrument, not a correctness guarantee. It
uses our prompts, and model interpretation may differ from the upstream recipe.
Inspect reasons and source passages when findings change a decision. Checking source
support is not equivalent to counting citation markers. No local support percentage is
inferred from an empty list of reported errors. Upstream FACT/DEER/support outputs can
supply their own separate measures.

## Use upstream graders

```sh
uv run --no-project python scripts/benchmark.py export /private/eval/drb2/suite.json \
  /private/eval/experiment/run-0-0/evaluation.json /private/eval/export/model-name \
  --arm perspective
```

DRB II receives `idx-<id>.md`; ResearchRubrics receives `<sample_id>.md`; DRB I receives
`reports.jsonl` with `id`, `prompt`, `article`. The export also records outcomes. Follow
the selected upstream evaluator's documented invocation in its own environment, with
its required credentials and permitted spending. This command does not run it.

Import a selected arm's flat per-task CSV or JSONL results. Specify columns explicitly
and retain native units; split multi-model upstream tables to the selected model first.
The complete supplied upstream result remains in the imported score file.

```sh
uv run --no-project python scripts/benchmark.py native-import \
  /private/eval/drb2/suite.json /private/eval/experiment/run-0-0/evaluation.json \
  /private/eval/upstream-scores.csv /private/eval/native-scores.json \
  --arm perspective --id-column idx --metrics analysis=model-name \
  --protocol upstream-release-and-recipe --judge upstream-model \
  --judge-environment upstream-source-environment
```

Missing task scores remain null. Giving a recipe name does not certify protocol
conformance: source restrictions, population, grader and output requirements still
matter. Do not call an adapted run native merely because its arithmetic matches.

## Controlled corpus access

The evaluation-only `corpus` command offers search/read over locally obtained JSONL
with one `{"id":"document-id","text":"document text"}` per line. It binds only to
loopback and requires `DEEPRESEARCH_SOURCE_TOKEN` from the environment. Use an existing
secret provider or a process-local credential; never put credential values in arguments.

```sh
cargo run --locked -- corpus /private/data/permitted-pool.jsonl 8099
```

For research against that pool, configure the runtime source endpoint as
`http://127.0.0.1:8099/mcp` and the source tool allowlist as
`corpus_search,corpus_read`. Only supplied documents are exposed. Gold nuggets must
never be included in the pool. Search is a simple lexical baseline over an in-memory
pool; it is not a full-corpus indexing service or the official TREC retriever.
Use a registered, permitted corpus service for full-scale track comparisons.

TREC 2026 development nugget files omit question text; `prepare trec-rag` accepts
`--topics /private/data/rag25-topics-dev.tsv`. Label the selected collection, edition
and retrieval conditions through `--environment`. A restricted pool, live-web run,
changed report format or substituted judge is an adapted comparison. Published
RAGtime's individual-citation support convention must not silently be replaced by a
joint-support metric while retaining its native score name.

## Qualify the grader and interpret results

```sh
uv run --no-project python scripts/benchmark.py qualify \
  /private/data/holistic_200cases.jsonl /private/eval/qualification \
  --release REFLECT_RELEASE --limit 3
uv run --no-project python scripts/benchmark.py report /private/eval/summary.json \
  --scores /private/eval/assessment/scores.json \
  --experiments /private/eval/experiment/experiment.json
```

REFLECT diagnostics compare original and perturbed reports in both presentation
orders and preserve ties/unresolved judgments. This tests sensitivity without creating
human-authored benchmark cases. It does not automatically validate every rubric or
answer-set grader. Use `grade --qualification` to identify the relevant diagnostic
and its limitations; no automatic pass threshold is asserted.

The JSON and Markdown scorecards show per-benchmark native/adapted metrics, scored task
counts, completion, elapsed time, concrete error findings and unresolved findings.
Repeated observations are averaged within tasks. Paired differences use the common
scored task subset and bootstrap tasks, not rubric rows. Small samples and missing
scores limit interpretation. Related tasks across benchmarks are not independent
corroboration; no cross-benchmark confidence interval or universal composite is emitted.
Different judge configurations and fixture/live modes stay in separate scorecards.
Longer budgets need distinct configuration labels. Time is never a penalty inside
quality scores. Unknown costs and tokens remain unknown in the underlying results.

## Research sources

- [ResearchRubrics](https://arxiv.org/abs/2511.07685), [implementation](https://github.com/scaleapi/researchrubrics).
- [DeepResearch Bench I](https://deepresearch-bench.github.io/), [current evaluator](https://github.com/Ayanami0730/deep_research_bench).
- [DeepResearch Bench II](https://arxiv.org/abs/2601.08536), [data terms and evaluator](https://github.com/imlrz/DeepResearch-Bench-II).
- [DeepSearchQA](https://arxiv.org/abs/2601.20975), [data and native autorater protocol](https://huggingface.co/datasets/google/deepsearchqa/blob/main/README.md).
- [TREC RAG](https://trec-rag.github.io/), [official data](https://github.com/TREC-RAG/trec-rag-data), [AutoNuggetizer](https://arxiv.org/abs/2411.09607), [RAGDoll](https://github.com/castorini/RAGDoll).
- [ARGUE](https://arxiv.org/abs/2405.00982), [RAGtime 2025 revised overview](https://arxiv.org/html/2602.10024v2), [2026 request decomposition](https://trec-ragtime.github.io/).
- [DEER](https://arxiv.org/abs/2512.17776), [data restrictions](https://github.com/hanjanghoon/DEER/blob/main/DATA_LICENSE).
- [REFLECT](https://arxiv.org/abs/2605.19196), [published release](https://github.com/LWang-Laura/REFLECT).

The [benchmark epic](https://github.com/chrisbennight/mcp-deepresearch/issues/25)
records the broader intent and access caveats. This capability does not require a new
production workflow, claim ledger or human-authored benchmark collection.
