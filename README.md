# mcp-deepresearch

A research service for people and agents using MCP clients. The target is useful,
cited answers developed through search, reading, comparison, and focused follow-up.

This is an early implementation. A fixture-backed controller can investigate, write,
and review a cited answer without credentials. The MCP service uses Restate for durable
execution, clarification, cancellation, and revision. A configured Codex adapter is
available for live assignments; integrated live verification is still pending.
See the [service guide](docs/service.md). Progress is tracked in the
[implementation epic](https://github.com/chrisbennight/mcp-deepresearch/issues/1).

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
[security](SECURITY.md). License selection is pending the repository owner's choice;
public visibility alone does not grant an open-source license.

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
