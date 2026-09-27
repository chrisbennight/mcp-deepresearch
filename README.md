# mcp-deepresearch

A research service for people and agents using MCP clients. The target is useful,
cited answers developed through search, reading, comparison, and focused follow-up.

This is an early implementation. A fixture-backed controller can investigate, write,
and review a cited answer without credentials. The MCP service uses Restate for durable
execution, clarification, cancellation, and revision. A configured Codex adapter is
available for live assignments. Actual Codex/current-MCP compatibility is verified
against a local fixture source. The [first scored study](docs/evaluations/2026-09-27-gpt6-astra-medium-baseline.md)
compares live Kagi research policies with a single-session control, including longer-budget
runs. It is an initial evaluation, not evidence of general superiority; see the
[scored evaluation guide](docs/scored-evaluation.md).
The [deployment guide](docs/deployment.md#verified-integration-and-remaining-gap)
records verified Gateway behavior and its remaining report-file integration gap.
See the [service guide](docs/service.md). Progress is tracked in the
[implementation epic](https://github.com/chrisbennight/mcp-deepresearch/issues/1).

## Multi-agent research

The opt-in `multi_agent` policy uses separate research sessions, materialized evidence
handoffs and review that can request further research. Its evaluation includes the same
method in one session and the ordinary single-agent baseline. See the
[research method and verification](docs/multi-agent-research.md). Superiority remains
an empirical question; the production default is unchanged.

## Published benchmark evaluation

Run the public-data baseline with the existing Codex subscription and configured
source gateway. It downloads public DeepSearchQA, DeepResearch Bench II, TREC RAG25
nuggets, TREC ResearchRubrics development tasks, and REFLECT diagnostics. Research
and grading use the same runtime; no separate model-provider keys are needed.

```sh
uv run --no-project python scripts/benchmark.py baseline /private/eval/baseline
```

Use `--prepare-only` to download and inspect the selected tasks without model calls.
The [benchmark guide](docs/benchmarks.md) explains the comparisons, source checks,
small default sample, and recovery after subscription limits. Scores are adapted
measurements, not official leaderboard results.

## Build

Install Rust with rustup, then run:

```sh
cargo build --locked
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

The pinned toolchain is the supported build environment. The minimum Rust version
is raised deliberately when required; CI tests the pinned toolchain rather than
claiming compatibility with untested compiler versions. Public Cargo sources and
the committed application lockfile make builds independent of private services.
Building and ordinary tests require no model account or credentials.

## Direction

Restate owns durable execution and execution visibility. A replaceable worker
runs meaningful agent assignments, initially using Codex. A configurable MCP gateway
provides source tools, initially Kagi search and reading. Research material lives in
a mutable workspace; this project is not an immutable artifact ledger.

See [architecture](docs/architecture.md), [contributing](CONTRIBUTING.md), and
[security](SECURITY.md). Licensed under [Apache-2.0](LICENSE).

## Try the fixture workflow

```sh
cargo run --locked -- fixture examples/comparison.json ./workspaces
```

The command prints a cited Markdown comparison and saves a mutable workspace snapshot.
The documents and alternatives are explicitly invented fixtures. This demonstrates
execution and output handling, not research quality. Change `strategy` to `focused`,
`exploration`, `comparison`, or `collection`; `format` independently accepts `answer`,
`report`, or `table`. Request limits bound assignments, elapsed time, context size,
and observed tool calls. Ctrl-C requests cancellation and preserves available material.

The controller gives the worker the objective, relevant evidence and remaining limits.
It follows proposed questions, writes from selected evidence, and permits a bounded
review follow-up. Exhaustion preserves a partial report and explains why work stopped.
Live runtimes must enforce their assignment deadline and stop before acknowledging
cancellation; the fixture runtime performs no external work.

## Live runtime

The initial supported worker platform is Linux. See [Codex runtime](docs/codex-runtime.md)
for the opt-in live command, scoped gateway access, account setup, cancellation and
recovery behavior. Local process tests verify execution mechanics without model calls;
live research quality and gateway wire compatibility are separate integration checks.
The [source adapter](docs/source-access.md) handles source restrictions, retained
material, and native file delivery without exposing transfer credentials to the worker.

[Native files](docs/files.md) support text attachments and full Markdown reports
without carrying file bytes in JSON-RPC or model context.

## Run the complete service

Follow the [Docker deployment and native MCP walkthrough](docs/deployment.md) for
upload, durable execution, task inspection, and report download. Use the
[paired evaluation](docs/evaluation.md) to compare structured research with a
single-session baseline; fixture success is not a research-quality result.
