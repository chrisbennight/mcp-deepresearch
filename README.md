# mcp-deepresearch

A research service for people and agents using MCP clients. The target is useful,
cited answers developed through search, reading, comparison, and focused follow-up.

This is an early implementation. The current executable is a bootstrap placeholder;
it does not yet perform research. Progress is tracked in the
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
