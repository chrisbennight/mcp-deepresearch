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
current protocol support. The application uses version 3.4.1;
protocol conformance must still be checked at the wire boundary during integration.
The official [Restate Rust SDK](https://github.com/restatedev/sdk-rust) version 0.12.1
exists on crates.io and declares Rust 1.90. Its generated ingress clients require
Restate 1.7 or newer. Both SDKs are used by the service.

Current protocol references:

- [MCP changelog](https://modelcontextprotocol.io/specification/2026-07-28/changelog)
- [Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
- [Tasks](https://modelcontextprotocol.io/extensions/tasks/overview)
- [Restate services](https://docs.restate.dev/foundations/services)

## Shared research data

The `research` module defines requests, strategies, assignment inputs and returns,
resource limits, and lifecycle states independently of MCP and the model runtime.
A research identifier also identifies its writable workspace. A revision of a
terminal investigation receives a fresh identifier and copies useful evidence;
it cannot concurrently overwrite the original investigation. Runtime session IDs
will remain separate from research and assignment identifiers.

The `workspace` module keeps retrieved excerpts separate from interpreted findings.
Only the workflow parent applies worker returns. It rejects results after termination
and references to missing evidence before changing notes. Citation existence checks
cannot establish that an excerpt actually supports a claim; substantive review remains
agent and human work. Context assembly selects relevant excerpts within a character
budget. Unknown tool usage remains unknown rather than being reported as zero usage.

Local snapshots are for standalone use; Restate owns durable service workflow state.
The snapshot store assumes one parent writer per workspace. Operators must protect
the workspace directory as confidential data. No immutable history or claim database
is maintained. Native Tasks project workflow progress, clarification, cancellation, and results.
Native text uploads and complete Markdown report downloads use an isolated file adapter.
The Codex adapter executes live assignments through the host-managed source adapter;
source file downloads and bounded material reads have local MCP integration coverage.
Integrated live validation is still pending.


## Controller and runtime

`Controller` chooses the next meaningful assignment from worker proposals. It carries
an evolving focus and strategy, reserves the final available assignment for writing,
and bounds review follow-ups. Strategy is independent of output format. Investigation,
synthesis, and review all pass through `AgentRuntime`; no separate model API is hidden
in the controller. Clarification waits preserve state and accept an explicit user reply.
The original elapsed-time allowance continues during a wait.

The standalone driver uses a deterministic fixture runtime and handles cancellation,
failure, and partial output. The Codex runtime enforces its supplied deadline and
reports cancellation only after its external worker has stopped. Restate
persists the same controller between assignments; the standalone snapshot is not
an execution queue and does not resume a running external process.

Fixture tests establish orchestration behavior, including strategy transitions and
bounded review. They do not measure answer quality or prove that a model investigates
the right questions. Those require matched live evaluation and human judgment.


Assignments carry the complete user objective, context, accepted clarifications and
source constraints separately from selected research evidence. Initial context and
each clarification answer have separate input limits; the finite assignment budget
bounds clarification turns. Accepted clarifications extend the brief instead of
evicting earlier user context. This can increase prompt size beyond the original
admission allowance. Evidence selection must not erase the user's brief.

See the [service guide](service.md) for the supported single-host deployment,
retention, recovery limits, protocol contract, and isolated integration walkthrough.

## Research policy experiments

The default `staged` policy retains the existing sequence for comparison. Opt-in
`evidence_access` and `adaptive` policies are described in the
[scored evaluation guide](scored-evaluation.md). They share the same execution and
source boundaries. Adaptive investigations keep editable research questions and can
finish directly; their proposed completion is still subject to external evaluation.
