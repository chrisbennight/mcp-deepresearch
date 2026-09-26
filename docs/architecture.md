# Architecture direction

One Research workflow serves focused investigation, field exploration, comparison,
and broad collection. These strategies share execution and data handling. Prompts
and ordinary code guide research, without a workflow language or custom scheduler.

The application owns the question, resource limits, mutable notes, selected source
excerpts, and cited answer. Restate owns execution and recovery. The parent merges
worker findings into the workspace; child assignments do not write shared notes.
All model work, including planning, writing, and review, uses the replaceable runtime.
Subscription access does not imply unlimited capacity or reliable token accounting.

The MCP boundary targets **2026-07-28 only**, stateless Streamable HTTP at `/mcp`.
Current Tasks project the Restate lifecycle; they are not another scheduler.
Waygate's upstream Tasks implementation is tracked separately in
[Waygate #51](https://github.com/chrisbennight/waygate/issues/51).

Files use an isolated adapter for the
[SEP-2631 draft profile](https://github.com/chrisbennight/waygate/blob/main/docs/file-transfer.md):
URI-string inputs and compact output references, with bytes outside JSON-RPC.
A report download failure must not rerun successful research.

The gateway owns policy and approvals. The service authenticates that hop, verifies
research ownership, and uses separate scoped credentials for outbound source access.
Retrieved pages are untrusted evidence, not permission to execute instructions.
Private operator deployment, accounts, and network configuration stay outside this
public project. Live execution will require authenticated ingress and worker isolation.

## Dependency selection

The official [Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk) provides
current protocol support. Version 3.0.0 exists on crates.io and declares Rust 1.88;
protocol conformance must still be checked at the wire boundary during integration.
The official [Restate Rust SDK](https://github.com/restatedev/sdk-rust) version 0.12.1
exists on crates.io and declares Rust 1.90. Its generated ingress clients require
Restate 1.7 or newer. Neither SDK is added to the bootstrap executable before use.

Current protocol references:

- [MCP changelog](https://modelcontextprotocol.io/specification/2026-07-28/changelog)
- [Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
- [Tasks](https://modelcontextprotocol.io/extensions/tasks/overview)
- [Restate services](https://docs.restate.dev/foundations/services)
