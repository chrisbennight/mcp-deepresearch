# Scored research evaluation

The scored suite in `evals/scored/cases.json` contains public questions. Its separate
`reference.json` contains the expected findings, source links, and development or
held-out designation. The researcher receives only the request, never the reference.
These are project evaluation cases, not official benchmark tasks or leaderboard results.

## Cases

| Case | Purpose | Split |
| --- | --- | --- |
| SQLite WAL snapshot and write upgrade | Correct recovery advice from precise source semantics | Development |
| Python asyncio child failure | Distinguish superficially similar APIs | Development |
| HTTP request retry | Correct conditional advice without overgeneralizing | Development |
| PostgreSQL serialization failure | Whole-transaction reasoning and limits | Held out |
| STORM transfer | Interpret mechanisms and controlled ablations | Development |
| WebWeaver memory | Distinguish evidence access from compressed summaries | Development |
| TTD-DR transfer | Avoid turning a research result into an unsupported architecture claim | Development |
| IterResearch transfer | Separate training, inference, and budget effects | Held out |
| Benchmark selection | Compare task coverage and measurement limitations | Development |
| Asteroid sample returns | Discover qualifying entities, complete fields, respect cutoff and exclusions | Development |
| Voyager boundaries | Combine dates, attributes, and scientific interpretation | Held out |
| PostgreSQL isolation matrix | Complete a structured comparison with implementation-specific exceptions | Held out |

This is a small suite with substantial software and research-method content. It is
useful for detecting regressions and selecting experiments, not establishing broad
domain generalization. Expand into additional user task families if results approach
a ceiling. Do not tune prompts against held-out outputs.

## Run and assess

Use the live account and source configuration described in [Codex runtime](codex-runtime.md).
Both research and optional automated assessment consume that account's allowance.
No separate model API is used.

```sh
cargo run --locked -- evaluate live evals/scored/cases.json ./baseline
cargo run --locked -- judge evals/scored/reference.json ./baseline/evaluation.json ./baseline-assessment
```

The evaluator runs fresh sessions with the arm labels and timing withheld. It sees
the user question, reference requirements, primary-source URLs, and delivered answer.
An operator can supply an `evidence` string in a separate reference file with retrieved
primary passages. Keep copyrighted full documents and private evaluation artifacts
outside the public repository. Source tools remain available to the evaluator for
checking citations and unresolved claims. Reference criteria are not infallible: review
the evaluator's evidence and any disagreement with the key.

Each requirement receives 0 for absent or incorrect, 1 for partial or inadequately
supported, and 2 for fully correct and supported. The report shows the percentage
of available requirement points, plus consequential errors. `decision_ready` requires
full credit, no consequential errors, and completed execution. An incomplete answer
can receive credit for supported content, but cannot pass as a completed deliverable.

The model assessment is inspectable judgment, not proof of truth. Calibrate it using
reviewed examples and inspect consequential or disputed judgments against sources.
Make any corrections in the judgments file with evidence, then recompute:

```sh
cargo run --locked -- score evals/scored/reference.json ./baseline/evaluation.json ./baseline-assessment/judgments.json ./baseline-assessment/scores-reviewed.json
```

The scorer requires every run and every criterion to be accounted for. A missing
judgment does not silently disappear from the denominator. Report failure rates,
partial results, and actual resource use alongside answer quality. Distinguish source
or runtime outages from wrong answers, while retaining them in reliability results.

## Experimental policies

The request's optional `policy` chooses an experiment independently of its subject
strategy and output format. The default remains `staged` pending evidence.

- `staged`: existing investigation, synthesis, and review sequence.
- `evidence_access`: same sequence, with source tools and retained full material
  available in later assignments. This also preserves freshness of same-run evidence.
- `adaptive`: evidence access plus an editable question/evidence map and direct
  completion when an answer is ready. The worker can investigate gaps, compose,
  or finish without mandatory rewriting. Collection guidance distinguishes discovery
  of candidates from completion of their attributes.

Set `request.policy` on a copy of the case file. The paired single-session arm always
uses the staged policy's ordinary single-session prompt, never the experimental
policy guidance. Results identify `evidence_access` or `adaptive` explicitly. A
single-session control built from a different code revision may have a different
output schema; record the binary revision and use contemporaneous paired controls
when attributing a change to the policy.

These are deliberately limited experiments. The collection behavior uses research
questions and findings; it is not yet a dedicated entity/attribute implementation.
The question map guides model decisions; it is not an externally verified measure
of completeness. Source access makes checking possible but does not guarantee the
model uses it well. Evaluation determines whether these changes improve answers.

Relevant research: [STORM](https://arxiv.org/abs/2402.14207),
[WebWeaver](https://arxiv.org/abs/2509.13312),
[TTD-DR](https://arxiv.org/abs/2507.16075),
[TREC RAGTIME](https://arxiv.org/abs/2602.10024),
[WideSearch](https://arxiv.org/abs/2508.07999),
[DeepResearch Bench II](https://arxiv.org/abs/2601.08536), and
[ResearchRubrics](https://arxiv.org/abs/2511.07685).
