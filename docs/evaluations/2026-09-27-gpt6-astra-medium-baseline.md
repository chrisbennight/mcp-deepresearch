# Research workflow evaluation: GPT-6 Astra, medium reasoning

27 September 2026 · Project-specific evaluation, not a benchmark leaderboard result

## Decision

Keep the single-session research agent as the quality control. Develop the adaptive,
source-accessible policy as the next candidate, and retain longer staged research as
an option to test on tasks that actually require it. This study does **not** justify
making mandatory investigation–writing–review passes the universal default, nor does
it establish that slower research is worse. The production default is unchanged.

The completed study contains **50 scored research outputs**: 12 baseline pairs, six
experimental pairs, and seven longer-budget pairs, plus three seeded assessor checks.
The initial screen allowed five minutes per answer. The extension allowed **twenty
minutes for both members of each pair**, with the same questions and scoring criteria.
Quality scores contain no latency penalty. A partial result is evidence about delivery
at its time limit, not a ceiling on what that workflow could achieve.

| Finding | Evidence | Implication |
| --- | --- | --- |
| A single session is a strong research agent | All 12 baseline controls completed; mean requirement coverage 98.3% | Additional orchestration must demonstrate added value over iterative search already happening inside a session |
| The original staged design sometimes loses useful information | WebWeaver explanation disappeared during review; a focused asteroid follow-up overwrote a complete table | Preserve evidence and the whole working answer across stages |
| More time must be evaluated, not scored as failure | All seven longer-budget pairs completed; evidence-enabled answers took 335 and 340 seconds | Report completed quality and runtime separately; retain capped observations without treating them as maximum capability |
| Source access is useful infrastructure, not a guaranteed quality gain | Writers and reviewers actually reopened retained sources; the three-case screen tied its controls at 100% | Keep access available, but do not require extra stages merely because they can use it |
| Adaptive research is a promising candidate | All three screen cases scored 100%; each finished in one assignment | Continue testing it; this does not yet validate its cross-session memory or superiority on difficult long research |
| Reference scores need interpretation | Some 90% answers were factually sound but omitted a rubric nuance; one 80% answer misstated a paper's search-step count | Separate factual correctness, useful coverage, and execution outcome rather than treating one number as truth |

## What was compared

**Single session** gives the model the full task, Kagi search and extraction tools,
and its configured allowances. It can search repeatedly, read, reconsider, verify,
and write within the session. It is not a one-shot answer from model memory.

**Original staged research** starts separate investigation, synthesis, and review
assignments. Later stages receive compact findings but cannot reopen source material.
**Evidence-enabled staged research** preserves that sequence while allowing later
stages to retrieve retained material and consult sources. **Adaptive research** adds
a mutable question/evidence map and allows direct completion or further investigation
without compulsory rewriting. Collection guidance distinguishes finding candidates
from filling their attributes; this is not yet a dedicated large-scale entity table.

The model was `gpt-6-astra` at medium reasoning effort, through the subscription-backed
Codex runtime. Search and reading used `kagi.kagi_search_fetch` and `kagi.kagi_extract`
through MCP. No separate paid model API was introduced. The harness exercises the real
runtime and research controller; these are not measurements of complete Restate/MCP
service request latency.

The five-minute screen used at most six assignments and a configured tool allowance
of 24. **Those were restrictive experimental settings, not service defaults**: the
service defaults are fifteen minutes, eight assignments, and an allowance of 80.
The twenty-minute extension initially changed only elapsed time. All extension runs
completed; no source-allowance rejection was observed in the inspected trajectories.
The evidence-enabled asteroid run finished with 24 observed MCP events, matching its
configured allowance; completion and the absence of a rejection do not prove budgets
never influenced research depth. It had already resolved the identified source conflicts
and requested completion, so there was no observed unfinished investigation requiring
another extension.

Reported tool counts are observed MCP events. One Code Mode event may invoke several
source operations, and the proxy enforces its own per-assignment allowance. Therefore
these runs share a configured allowance, but do not establish equal, strictly enforced
cumulative Kagi-query budgets across workflows. Runtime and token counts do not establish
actual subscription-quota consumption or dollar cost.

## Cases and scoring

The [case file](../../evals/scored/cases.json) contains complete prompts; the separate
[reference file](../../evals/scored/reference.json) contains requirements and primary
source links. The researcher never receives that reference. Eight cases are development
cases and four are held out. The experimental screen was selected before held-out
answers were used for tuning; no held-out answer was used to tune these policies.

| Case | What a useful answer must do | Split |
| --- | --- | --- |
| SQLite snapshot | Explain a stale WAL snapshot, failed write upgrade, recovery, and writer contention | Development |
| asyncio failure | Distinguish gather and TaskGroup cancellation and error delivery | Development |
| HTTP retry | Give conditional retry advice without equating idempotency with identical responses | Development |
| PostgreSQL retry | Explain serialization failure and the whole-transaction retry boundary | Held out |
| STORM | Separate useful research mechanisms from unsupported universal persona mandates | Development |
| WebWeaver | Explain retrievable evidence, iterative planning, and the limits of the ablation evidence | Development |
| TTD-DR | Distinguish research iteration from mandatory critic passes and interpret cumulative ablations | Development |
| IterResearch | Separate workspace reconstruction, training, and interaction-budget effects | Held out |
| Benchmark selection | Compare four benchmarks' tasks, measures, and coverage gaps | Development |
| Asteroid returns | Find all qualifying missions, fill fields, apply a cutoff, and exclude nonqualifying missions | Development |
| Voyager boundaries | Join dates and attributes while distinguishing physical crossing from later confirmation | Held out |
| Isolation matrix | Complete a database-specific comparison without importing incorrect generic guarantees | Held out |

Each case has five prewritten requirements, scored 0, 1, or 2. **Coverage** is the
percentage of those ten points earned for correct, supported content, not a probability
of truth. An additional check looks for consequential false claims anywhere in an answer,
including claims outside the requirements. The raw `decision_ready` flag requires full
coverage, no consequential findings, and completed execution. It is a strict delivery
criterion; it should not be read as a general declaration that every other answer is wrong.

The assessor runs in fresh sessions with workflow labels and timing withheld. It sees
the question, answer, reference, primary passages and links, and can use source tools.
Answer style can still reveal its origin. It uses the same model family as the researcher;
correlated errors remain possible. Inspection of disputed judgments and observable tool
trajectories supplements it, but also uses GPT-6 Astra and is not human validation.

Three seeded checks passed: a correct answer scored 100%; false claims with real citations
were detected; and a correct core answer with an added false claim scored 100% coverage
but failed the separate correctness check. This verifies a useful basic distinction,
not general assessor reliability.

## Five-minute baseline

| Case | Staged coverage | Single-session coverage | Staged seconds | Single-session seconds |
| --- | ---: | ---: | ---: | ---: |
| sqlite-snapshot | 100% | 100% | 191.7 | 81.3 |
| asyncio-failure | 100% | 100% | 119.3 | 45.0 |
| http-retry | 100% | 100% | 189.8 | 128.8 |
| postgres-retry | 90% | 90% | 142.2 | 62.0 |
| storm-transfer | 100% | 100% | 187.3 | 127.5 |
| webweaver-memory | 90% | 100% | 212.9 | 85.8 |
| ttd-transfer | 80% (incomplete) | 90% | 299.7 | 124.8 |
| iterresearch-transfer | 90% | 100% | 253.9 | 161.2 |
| benchmark-selection | 100% | 100% | 248.6 | 139.3 |
| asteroid-returns | 80% (incomplete) | 100% | 299.1 | 142.2 |
| voyager-boundaries | 0% (failed) | 100% | 207.2 | 116.8 |
| isolation-matrix | 100% | 100% | 185.0 | 64.2 |

The staged arm completed 9/12 answers, returned two partial answers at the time limit,
and had one provider failure. Its completed-answer mean coverage was **96.7%**; the
single-session arm completed 12/12 with **98.3%** mean coverage. These conditional means
have different denominators and are not a clean head-to-head superiority estimate.
Including the failure as zero and the partials yields 85.8% staged coverage, but that
number mixes reliability, the chosen time limit, and answer content; it is not the
staged method's intrinsic quality. Median observed runtime was 199.4 versus 120.8 seconds.

The staged Voyager failure was a provider stream termination marked `content_filter`,
not an incorrect delivered answer. An earlier startup attempt also suffered Gateway
source-discovery failures: one answer completed and seven failed. That attempt is
retained separately from the complete retry summarized here, not silently erased.
One assessment batch also needed a fresh retry after discovery failed before producing
any judgments.

## Five-minute experimental screen

Both policies were tested on SQLite, WebWeaver, and benchmark selection, each against
fresh controls built from the same experimental binary. This matters because the newer
response schema itself differs from the original baseline binary.

| Policy | Completed | Mean coverage: policy / control | Median seconds: policy / control | Recorded input tokens: policy / control |
| --- | ---: | ---: | ---: | ---: |
| Evidence-enabled staged | 3/3 | 100% / 100% | 264.5 / 115.4 | 1,795,847 / 1,073,712 |
| Adaptive | 3/3 | 100% / 100% | 98.3 / 116.8 | 947,799 / 1,264,569 |

All twelve outputs had full requirement credit and no consequential errors identified.
The evidence-enabled policy used three assignments per case; adaptive used one.
These results favor testing adaptive completion rather than forcing extra stages on
already-answerable questions. They do **not** demonstrate improved cross-session memory:
there was no adaptive handoff in these cases. The small sample and score ceiling also
prevent a general quality claim.

## Twenty-minute extension: quality without a speed penalty

The original staged workflow was repeated on TTD-DR and asteroid returns, which had
hit the screen limit, plus benchmark selection as a completed demanding comparison.
Both experimental policies were repeated on the two time-limited cases. This selection
was made after inspecting results; it is a diagnostic extension, not a held-out test.
These are fresh paired runs, not continuations, and do not overwrite the original results.

| Workflow | Case | Workflow coverage | Control coverage | Workflow seconds | Control seconds |
| --- | --- | ---: | ---: | ---: | ---: |
| Original staged | ttd-transfer | 80% | 90% | 257.5 | 117.5 |
| Original staged | asteroid-returns | 100% | 100% | 267.1 | 201.1 |
| Original staged | benchmark-selection | 100% | 100% | 243.0 | 152.2 |
| Adaptive | ttd-transfer | 100% | 90% | 148.2 | 137.0 |
| Adaptive | asteroid-returns | 100% | 100% | 130.5 | 189.8 |
| Evidence-enabled staged | ttd-transfer | 90% | 90% | 335.3 | 137.7 |
| Evidence-enabled staged | asteroid-returns | 100% | 100% | 339.8 | 178.0 |

All fourteen answers completed. The original staged asteroid answer now covered the
whole requested table correctly. All three original staged extension runs nevertheless
finished within five minutes: variation in research paths is part of the result, so
we cannot attribute the improvement solely to time actually spent beyond the old limit.
The evidence-enabled TTD and asteroid answers took 335 and 340 seconds respectively,
so the longer allowance captured completed work that the short screen would cut off.

On TTD, the original staged answer made a substantive error: it described a comparison
as **20 self-evolution steps**, while the paper specifies **20 search steps**. The final
review introduced that more specific, incorrect wording. This is independent of runtime.
The evidence-enabled and adaptive answers avoided that mistake. Several 90% TTD answers
were otherwise factually sound: their missing credit concerned early-draft anchoring,
an inferred design risk, not a measured finding of the paper. Adaptive's 100% versus
its control's 90% should not be advertised as proof of a decisive correctness advantage.

The longer TTD answers included useful detail about actual component settings and judge
agreement, beyond the core question. Their paired single-session controls also supplied
these details. Inspection found no clear additional decision-relevant benefit from the
slower staged answer in that comparison. Its final text also spoke to an internal draft
review rather than consistently delivering a standalone answer; coverage alone misses
that product-quality weakness. Length and the number of passes are not benefits by themselves.

The asteroid comparison supplies a positive example that a coverage ceiling would hide.
The evidence-enabled reviewer discovered that a JAXA overview gave Hayabusa's return
month as July ([JAXA overview](https://www.isas.jaxa.jp/en/topics/004280.html)).
It checked other agency records supporting June, removed that overview
as support for the precise date, and disclosed the conflict. It also investigated an
ambiguous NASA description of Stardust's Annefrank encounter before confirming that
the mission belonged outside the asteroid-return list
([NASA mission record](https://science.nasa.gov/mission/hayabusa/),
[JPL encounter report](https://www.jpl.nasa.gov/news/nasas-stardust-comet-chaser-passes-asteroid-test/)).
The table's core facts were
already correct before review, but the final answer better explained why its sources
supported them. The paired control supplied the correct table and exclusions without
this same conflict discussion. That is useful additional verification, not proof that
three stages are always needed. The fresh-run design cannot isolate how much benefit
was caused specifically by spending the extra forty seconds beyond five minutes.

## What the trajectories changed

**Preserve the complete working answer during focused follow-up.** In the original
asteroid run, investigation and synthesis had already produced the complete three-mission
table. A later assignment checked one citation and returned a one-row draft; the controller
replaced the whole table with it. A subsequent timeout left that narrower partial output.
The implementation now preserves the existing draft during focused investigation, while
retaining new findings, unless adaptive research explicitly finishes with a replacement
answer. Replaying the recorded worker results through the corrected controller preserved
the complete table and the added finding. This was a deterministic replay, not a newly
scored model run; the result remained labelled incomplete.

**Give later stages access to evidence.** In the baseline WebWeaver case, both arms made
the same observed source-tool sequence: two extractions, a retained-material read, and
a search. The staged investigator explained alternating search and outline revision;
the writer retained it; the final reviewer omitted it. The drafts shrank from 636 to
564 to 366 words. Review also removed supported details because they were absent from
the compact handoff. In the evidence-enabled SQLite experiment, both writer and reviewer
actually listed and read retained sources. The access is working, although its quality
benefit still needs harder tests.

**Check revisions for lost or altered meaning.** The TTD step-count error and WebWeaver
omission arose during review. Verification should repair material defects and preserve
correct findings, not merely produce a shorter rewrite. Adding another mandatory critic
is not an evidence-based remedy for a critic introducing mistakes.

**Keep source presentation efficient.** One inspected Kagi envelope stored the same
24,813-byte document in both `content` and `structuredContent`, producing a 50,242-byte
file. Presenting the document once while retaining attribution is a concrete future
context-efficiency experiment. Its benefit was not measured here, and this observation
does not imply doubled runtime or usage.

## Recommendations

| Recommendation | Why | What remains to establish |
| --- | --- | --- |
| Use adaptive, evidence-accessible research as the next candidate | It can answer simple tasks directly and spend further work on unresolved questions | Repeated paired tests on genuinely difficult, longer tasks; current adaptive successes all fit one assignment |
| Retain a generous quality-focused allowance alongside constrained comparisons | A short timeout measures responsiveness, not attainable answer quality | Quality-versus-effort comparisons across several allowances, extending runs when a limit actually binds |
| Keep source-linked questions, findings, and full material retrievable | Matches observed handoff failures and relevant research mechanisms | Whether this improves completeness across long sessions, contradictions, and many sources |
| Preserve the full answer during targeted checks and require a standalone final response | Prevents a local correction from replacing the user's deliverable with a fragment or reviewer memo | The draft-preservation defect is fixed; standalone delivery still needs explicit outcome evaluation |
| Compare against the single-session agent throughout | It already performs iterative research and achieved strong results here | Improvements must exceed normal search/model variation, not merely beat one weak attempt |
| Add harder collection and conflicting-source cases before changing defaults | Current entity sets are small and many scores reach the ceiling | Missing-entity recall, attribute completeness, version/date conflicts, and support for consequential claims |
| Use fixed-source comparisons as well as live search | Separates evidence discovery from writing and verification behavior | Whether gains come from better retrieval, better use of the same evidence, or additional computation |

The research supports these hypotheses, not a mandatory implementation recipe.
[STORM](https://arxiv.org/abs/2402.14207) motivates question and perspective discovery;
[WebWeaver](https://arxiv.org/abs/2509.13312) motivates detailed evidence access during
writing; [TTD-DR](https://arxiv.org/abs/2507.16075) motivates retrieval-guided revision;
and [IterResearch](https://arxiv.org/abs/2511.07327) motivates testing workspace
reconstruction while separating its effects from training and increased interaction.
[TREC RAGTIME](https://arxiv.org/abs/2602.10024) and
[ResearchRubrics](https://arxiv.org/abs/2511.07685) motivate content-based assessment;
[WideSearch](https://arxiv.org/abs/2508.07999) highlights collection completeness.
This suite's five requirements per question are a small project-specific application
of content assessment, not a reproduction of TREC nugget evaluation or an official
score on any of those benchmarks.

## Limits and reproducibility

This is a useful scored baseline and experiment screen, not enough evidence to claim
state-of-the-art capability or promote a universal default. There is one observation
per pair at each setting, mostly software and research-method questions, no independently
human-validated scoring, and no demonstrated adaptive multi-session benefit. Some
rubric clauses exceed what the user explicitly requested: PostgreSQL's overhead nuance
and TTD's anchoring discussion should not turn otherwise correct answers into claims
of hallucination. Raw judgments are preserved rather than silently improving scores.

Research and assessment sometimes overlapped on one subscription account. The longer
pairs always ran the workflow first and its control second; the experimental long-run
workers could overlap each other. Ordering, caching, live source changes, and shared
capacity can affect timing. Token totals are recorded usage, not billed cost; usage
is incomplete for the original staged baseline. No unobserved quota savings are claimed.

[Machine-readable results](../../evals/scored/results/2026-09-27-summary.json) record
all 50 outputs' scores, outcomes, timing, usage, and run configuration. The baseline
binary was pinned to `efe90edf4d993653f382c686c1e9ca6e74a4955c`; experimental research to `11ae010b4d786f67ab7276c22f3fe53502192292`.
Later delivery fixes were not silently substituted into these runs. The separate
assessment binary gained neutral completion metadata; no single-session research
answer here was incomplete, so its earlier completion-footer issue did not affect
these comparisons.

The [evaluation guide](../scored-evaluation.md) documents running, scoring, and recovery.
Completed judgments are saved, but interrupted assessment does not automatically resume;
assess only the remaining IDs separately and combine judgments. Private retained source
extracts and raw runtime logs are not published in this public repository. Ordinary
build, tests, formatting and lint checks passed; optional live integration tests remain
separate from those checks. Real Codex/Kagi research was exercised by this study.
