# Is the structured workflow worth its complexity?

The evaluation compares the research product with a straightforward single-session
agent. Both arms use the same configured runtime/model, source tools, user question,
wall-clock allowance, and source-call allowance. The structured arm can plan further
assignments, write, and review within its assignment cap. The baseline receives the
whole question and produces a final cited answer in one session with source access.
Order alternates across cases to reduce a systematic first-run/cache advantage.
This is a useful comparison, not a claim that all context or provider caching is equal.

## Run

```sh
cargo run --locked -- evaluate fixture evals/cases.json ./evaluation-fixture
# Requires the explicit live account/source configuration in codex-runtime.md:
cargo run --locked -- evaluate live evals/cases.json ./evaluation-live
```

The live command consumes the configured account's allowance. Set an explicit
`DEEPRESEARCH_MODEL` for a comparison you intend to publish. Keep tool allowlists,
model settings, and budgets unchanged between arms. Use a new output directory for
each comparison. Run several repetitions before drawing a performance conclusion.
The command does not call a separate paid grading model.

Each run saves a readable Markdown answer, a mutable workspace snapshot, and an
`evaluation.json` summary. The summary records completed, failed, incomplete,
needs-input, and skipped outcomes separately, elapsed time, runtime calls, and
usage when observable. It does not fabricate token counts, monetary cost, or quality
scores. Cancellation skips later work. A clarification wait is not a completed
answer; the unattended comparison makes no human interventions. Report those cases
and optionally conduct a separately labeled comparison with equal human assistance.

## Public case suite

| Case | Capability under examination | Important limitation |
| --- | --- | --- |
| Focused protocol question | Exact current facts and supporting primary citations | Does not measure long-form coverage |
| Field overview | Perspectives, synthesis, boundaries, and uncertainty | Coverage requires human judgment |
| Product decision | Consistent criteria, tradeoffs, and decision-changing evidence | Current product behavior can change |
| Benchmark collection | Candidate diversity, attribute completeness, missing values | A small collection is not evidence of exhaustive recall |

The cases borrow evaluation dimensions from [BrowseComp](https://arxiv.org/abs/2504.12516),
[STORM](https://arxiv.org/abs/2402.14207), and
[WideSearch](https://arxiv.org/abs/2508.07999). They are not official benchmark tasks,
scoring implementations, or leaderboard-equivalent results. BrowseComp motivates
hard fact finding; STORM motivates breadth and article structure; WideSearch motivates
collection completeness. These dimensions complement rather than replace each other.

## Human scoring

Hide the arm labels, read both answers, and use the case's `assess` questions. Score
each dimension from 0 (unusable), 1 (major gaps), 2 (useful with corrections), to
3 (decision-ready within the stated scope):

- Correctness: consequential factual claims agree with their primary evidence.
- Citation support: inspect the actual passages, not just whether a URL exists.
  Include a source that mentions a subject without supporting the claimed conclusion.
- Coverage: answers the question, includes serious alternatives, and exposes omissions.
- Usefulness: helps the requested decision or understanding with appropriate detail.
- Uncertainty: distinguishes observations, inference, disagreement, and missing evidence.
- Stopping: follows the resource allowance and explains unresolved work honestly.

Record brief reasons and examples for each judgment. Count unsupported consequential
claims separately; a polished answer with unsupported advice is not a success.
For collection, check duplicate candidates and unsupported claims of completeness.
For current facts, record evaluation date and source freshness. Use a second reviewer
when a decision depends on small subjective differences; report disagreement.

Report paired results and failure rates, not only averages among successes. Separate
answer-quality changes from elapsed time and resource overhead. Subscription usage is
not a reliable monetary-cost measure, and unknown usage must remain unknown. Do not
claim improvement from a single successful run or unmatched tools/models.

## Execution evidence versus answer quality

The initial fixture comparison completed both arms for all four public case shapes.
The fixture structured arm used separate investigation, writing, and review calls;
the baseline used one complete session. Those are invented documents and deterministic
answers. Quality, cost, and live improvement remain **unmeasured**. Fixture elapsed
time is not representative of model or network latency.

Ordinary tests cover unavailable-source outcomes and skipped comparisons. The separate
Restate/MCP walkthrough covers restart during clarification, cancellation, expired or
revoked transfer authority, report recovery, and revision. These are reliability tests,
not evidence that the agent asks good questions or supports its conclusions. The live
comparison and human review are the evidence needed to choose future research changes.

## Scored evaluation and workflow experiments

The [scored evaluation guide](scored-evaluation.md) supplies reference-backed cases,
separate-session assessment, reviewed score import, and opt-in policy comparisons.
The paired runner itself still leaves `quality_score` empty; assessments live in
separate files so execution success cannot be mistaken for answer quality.
