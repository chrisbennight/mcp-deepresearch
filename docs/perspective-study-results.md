# Domain-sensitive research: initial experiment

27 September 2026 · GPT-6 Astra · medium reasoning effort

The new perspective policy produced useful, source-supported answers across all six development cases, but **did not demonstrate better answer quality than the existing single-session agent**. The independent reviewer judged every pair a tie and found no consequential factual errors. Both arms answered precise questions directly and explored meaningful constraints and counterpoints on broader questions.

Keep the policy available for further comparison; do not promote it on a claim of superiority from this experiment. I recommend preserving source access and flexible investigation; these results do not motivate extra mandatory stages. It also shows that more discriminating cases are needed before attributing useful research behavior to the added prompt.

## Results

| Case | Perspective coverage | Control coverage | Perspective seconds | Control seconds | Outside reviewer preference |
| --- | ---: | ---: | ---: | ---: | --- |
| marine-fastener | 100% (completed) | 100% (completed) | 81.7 | 89.9 | tie |
| sqlite-direct | 100% (completed) | 100% (completed) | 36.8 | 32.3 | tie |
| office-policy | 100% (completed) | 100% (completed) | 274.9 | 276.5 | tie |
| rent-policy | 100% (completed) | 100% (completed) | 274.9 | 300.6 | tie |
| long-context | 100% (completed) | 100% (completed) | 300.0 | 201.0 | tie |
| citation-gate | 100% (completed) | 100% (completed) | 177.4 | 163.4 | tie |

Coverage means the reviewer's supported coverage of five prewritten requirements per case, scored zero to two each. It is not a percentage of all factual claims proven true. The [criterion-level judgments](../evals/perspectives/review.json) include supporting source links and reasons, including where the reviewer found a criterion overprescriptive. The [machine-readable results](../evals/perspectives/results.json) include configuration, completion, time, usage and pairwise judgments; the [protocol](perspective-study.md) and [cases](../evals/perspectives/cases.json) describe the comparison.

All twelve answers completed in one assignment. None reached the twenty-minute limit, and no run needed a budget extension. No answer was omitted for being slow. The largest observed time difference was long context: 300 seconds with perspective guidance versus 201 seconds for the control, with no judged quality advantage. Conversely, the perspective rent-policy answer finished about 26 seconds sooner, also with tied quality. One run per case cannot establish a dependable speed advantage.

## Substantive findings

The important result is whether the agent reached beyond surface wording while keeping its answer useful. Both arms did that across different domains:

- **Marine fastener:** both distinguished matching threads from suitability in salt spray, identified galvanic corrosion, and gave practical protection and mechanical checks. Neither invented a debate. The reviewer did not penalize the shorter explanation for omitting relative wetted-area detail when the recommendation remained correct. The underlying constraint is supported by [ASSDA's galvanic-corrosion guidance](https://www.assda.asn.au/publications/technical-faqs/galvanicdissimilar-metal-corrosion).
- **SQLite:** both answered directly that the active read transaction retains its snapshot, explained how to see the later commit, and used the [official isolation documentation](https://sqlite.org/isolation.html). Neither turned a settled behavior question into a survey of viewpoints.
- **Office policy:** both found the [Trip.com randomized trial](https://www.nature.com/articles/s41586-024-07500-2), then considered mentoring, collaboration and innovation evidence that limits an uncomplicated pro-hybrid conclusion. Both distinguished the six-month randomized policy contrast from longer follow-up, optional eligibility from actual attendance, and task-level benefits from proof that five office days are necessary. The reviewer found different useful details but no material decision advantage.
- **Rent policy:** both distinguished incumbent protection from access for future renters and separated empirical effects from the city's priorities. Both identified genuinely conflicting Catalonia studies rather than merely saying “some disagree.” They also considered assistance, eviction defense and housing supply. The treatment more explicitly distinguished advertised listings from registered rental stock; the control included a broader literature review. Neither difference changed the reviewer's recommendation. The [San Francisco study](https://www.aeaweb.org/articles?id=10.1257/aer.20181289) supports both tenant protection and a rental-supply response, not a universally optimal policy.
- **Long context:** both investigated evidence favorable to removing retrieval and evidence for retaining or improving it. Both discussed caching, task dependence and hybrid approaches. The treatment used an OP-RAG counterexample and an illustrative cost calculation; the control added LOFT and global-summary evidence. Both correctly distinguished the [CAG preprint's](https://arxiv.org/html/2412.15605v1) cached-versus-uncached timing comparison from a measured speedup over RAG. More or different papers did not produce a reviewer preference.
- **Citation gate:** both distinguished working URLs, citation presence, actual claim support, source reliability and adequate coverage. Both used [ALCE](https://aclanthology.org/2023.emnlp-main.398/) and related research to recommend calibrated checks rather than automatic trust. Their different examples and evaluator studies did not reveal a material omission in the other answer.

These are useful examples of the desired behavior. They also show why the number of perspectives, questions or sources is not a success metric: different evidence selections can produce equally well-supported decisions.

## Independent assessment of observable research

After saving its quality judgments, the reviewer examined the actual search and read invocations with policy identities still hidden. It found substantive exploration in both arms, often from their first searches: galvanic corrosion beyond thread fit; mentoring and innovation alongside hybrid performance; rental assistance and welfare alongside rent caps; caching and larger-document retrieval alongside full context; and evaluator failure alongside citation checking. This was not merely a collection of additional questions returned after the answer.

Both arms also performed useful verification: seeking pre-cutoff paper versions, reading later methods sections, and checking what a reported timing comparison actually measured. They followed different papers toward similar qualified recommendations. Final working questions alone do not prove when an angle was discovered, but the visible queries establish that relevant alternatives were actually investigated.

The reviewer identified a separate efficiency hypothesis: source access and response handling may offer more concrete opportunities than extra workflow stages. Runs repeatedly discovered tool descriptions, decoded result wrappers, inspected object keys, paged through material and tried alternative source forms. Some of that was likely necessary recovery from inaccessible or incomplete material; the invocation-only packet cannot establish which calls were avoidable. A matched test of simpler source discovery and passage reading, with unchanged research guidance, would measure whether this reduces effort without losing evidence.

Raw tool-call count is not a reliable efficiency ranking. In the long-context pair, the control recorded 64 usage events and finished in 201 seconds; the treatment recorded 27 and finished in 300 seconds. Wrapper calls can contain multiple source operations. These values do not reveal how much time was spent researching, waiting or writing. Neither invocation timestamps nor final notes justify a diagram claiming exact internal phase durations.

The reviewer also noted repeated qualifications in otherwise useful answers and searches that already contained expected numerical findings. The latter may be efficient verification of remembered results, but a future comparison should test whether neutral initial discovery finds contrary evidence that targeted confirmation misses. Neither observation changed the saved quality scores.

## What was tested

The treatment is an opt-in `perspective` policy: understand the user's purpose, read initial sources, identify consequential questions, investigate them, revise the working answer, and check both support and omissions. These are activities within one agent assignment. They are not separate mandatory planner, writer, and critic calls. The existing mutable questions and findings are the working state; no claim ledger or new provenance service was added.

Perspectives depend on the domain. Technical work should surface relevant constraints without inventing disagreement. Scientific work should compare mechanisms, measurements and transfer limits. Policy work should distinguish evidence about consequences from choices about whose interests and which outcomes matter. A weakly supported viewpoint may deserve explanation without deserving equal evidential weight.

The control is the existing single-session research agent. It already receives source-grounding instructions, tools, and strategy guidance. Focused strategy seeks disconfirming evidence; comparison strategy asks for alternatives and evidence that could change the recommendation. This is therefore a comparison against a capable existing baseline, not an unconfigured chat prompt. The treatment adds both adaptive working-question guidance and explicit domain-sensitive perspective guidance. It does not isolate the latter's effect over adaptive guidance alone.

Both arms used GPT-6 Astra at medium reasoning effort, the same Kagi search/read tools, a twenty-minute allowance per answer, and a configured source-call allowance of 80. Each received one complete-research assignment. Case order alternated which arm ran first, and batches ran serially. The researcher received the request and shared constraints, not the separate reference requirements. The model ran with local shell, built-in search, inherited rules and sub-agents disabled.

The six cases and five requirements per case were written before execution. They are development cases, not held-out evidence. A fresh-context reviewer received anonymized answers without our conversation, implementation, policy labels, timings or preferred conclusion. It recorded an open-ended assessment before seeing the reference, checked material source claims through Kagi, and then scored each requirement from zero to two. It could identify an overprescriptive requirement rather than reward irrelevant detail. Only after those judgments were saved did it receive observable tool calls and timings for a separate process assessment. The reviewer was outside the research workflow: no feedback was used to repair these answers.

The reviewer had independent context, not an independent model family. Its source checks reduce reliance on superficial citation counting, but neither its approval nor full rubric coverage certifies that every claim is true. Runtime and quality were assessed separately.

## What this experiment can establish

These cases test sensible behavior across different domains and question sizes. They are less demanding tests of discovering an unfamiliar, initially invisible issue. The bolt case closely follows the motivating example; the policy questions explicitly request tradeoffs; the technical research questions concern well-known literature. The shared baseline also already asks the model to challenge an apparent answer. Both arms reached the ceiling on this rubric. We need more discriminating cases; research quality has not been solved.

There is one observation per arm and case. Differences in wording, sources or elapsed time cannot establish a reliable treatment effect. Search results, caching, model variation and shared infrastructure can affect a run. Recorded input tokens include repeated context and do not measure subscription quota or billed cost. Observable MCP events do not identify exact cumulative upstream search operations.

The frozen trial executable preceded a correction that retains returned perspective questions in the workspace after completion. Its prompts and one-session model inputs match the reviewed implementation; the correction affects post-answer persistence, not the delivered trial answers. Returned questions remain available in the original worker results. Container packaging was also corrected to include the compiled prompt file. Neither change triggered a selectively favorable rerun.

## Recommendation

Keep this as an opt-in research policy and retain the one-session baseline. The desired behavior is useful, and this implementation expresses it without adding mandatory handoffs or restricting evidence access. This experiment does not justify replacing the default on a claim of superior answer quality, adding more agents, or requiring a fixed number of perspectives.

The next discriminating comparison should use unfamiliar, representative questions selected independently of this prompt: consequential constraints absent from the wording; misleading initial terminology; genuine minority positions whose relevance differs from their factual strength; and evidence that forces the researcher to abandon its first explanation. Include exact factual questions to detect unnecessary expansion. Predefine the important answer content, but allow an assessor to identify useful discoveries outside that reference.

Compare the existing single-session agent, adaptive guidance alone, and the perspective policy with repeated matched runs. That separates the effect of revisable questions from the additional perspective instructions. Judge improvements by supported, decision-relevant content and important errors or omissions. Report time separately and extend allowances if unfinished work could plausibly change the result. A slower answer is worth keeping when its additional evidence materially improves the decision.

Keep the independent reviewer outside the workflow for that comparison. An additional reviewer inside production is a separate hypothesis; it should not be introduced merely because an outside evaluator was useful. Likewise, final working notes are useful state, but their existence is not evidence that every perspective was investigated or that a particular sequence of reasoning occurred.
