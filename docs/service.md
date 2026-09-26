# Research service

The service exposes current MCP tools and native Tasks backed by Restate. A task
is a view of one research workflow, not a separate job queue. Restate's execution
view shows the research invocation and its investigation, writing, and review
assignments. The mutable workspace holds evidence, findings, and the report.

## Run and connect

Start a Restate server version 1.7 or newer. Keep its ingress, administration, and
service-to-service network private. Register this application's workflow endpoint
with Restate using its deployment API or CLI. The endpoint defaults to
`http://127.0.0.1:9080`; when using containers, configure a reachable private address.

Supply `DEEPRESEARCH_INBOUND_TOKEN` through the deployment secret provider and set
`DEEPRESEARCH_PRINCIPAL` to the identity represented by that authenticated gateway
connection. Then run:

```sh
cargo run --locked -- serve fixture ./workspaces
# After configuring the account and sources described in codex-runtime.md:
cargo run --locked -- serve live ./workspaces
```

Fixture mode uses invented documents and requires no model account. Live mode uses
the same controller and lifecycle with the configured Codex worker.

| Setting | Purpose and default |
| --- | --- |
| `DEEPRESEARCH_MCP_LISTEN` | Authenticated MCP listener; `127.0.0.1:8088` |
| `DEEPRESEARCH_WORKFLOW_LISTEN` | Private Restate service listener; `127.0.0.1:9080` |
| `DEEPRESEARCH_RESTATE_INGRESS` | Private Restate ingress; `http://127.0.0.1:8080` |
| `DEEPRESEARCH_ALLOWED_HOSTS` | Comma-separated MCP Host allowlist; `localhost,127.0.0.1` |
| `DEEPRESEARCH_PRINCIPAL` | Required trusted identity for this deployment |
| `DEEPRESEARCH_INBOUND_TOKEN` | Required gateway-to-service bearer secret |
| `DEEPRESEARCH_INBOUND_TOKEN_PREVIOUS` | Optional overlapping credential during rotation |

Use TLS at the authenticated gateway boundary. The workflow listener has no public
MCP authentication and must be reachable only by trusted Restate infrastructure.
This initial deployment supports one trusted principal and one worker host. It
does not interpret a caller-provided identity header as delegated authority. Use
separate instances for separate principals until verified delegation is implemented.
Inbound and outbound source credentials are separate.

## MCP contract

POST JSON-RPC to `/mcp` with `Authorization: Bearer …`,
`MCP-Protocol-Version: 2026-07-28`, and the current MCP routing headers. There is no
legacy initialization or session transport. Send request-local client capabilities;
`research_submit` requires the `io.modelcontextprotocol/tasks` extension.

- `research_submit` accepts a UUID `request_id`, a research `request`, and optional
  `previous` research UUID. The response is a native task handle.
- `research_status` accepts `research_id` and returns concise progress.
- `research_report` accepts `research_id` and returns the current report, including
  partial material after a failure, cancellation, or exhausted limit.
- `tasks/get`, `tasks/update`, and `tasks/cancel` operate on the returned task ID.
  Set `Mcp-Name` to that task ID as required by the current routing contract.
  `tasks/get` carries the terminal result; there is no separate `tasks/result` API.

Clarification uses the task's current form elicitation request. Send its request ID
and an elicitation response through `tasks/update`. Accept supplies `content.answer`;
decline continues with explicit assumptions; cancel requests cancellation. The
wall-clock allowance continues during clarification. Cancel acknowledgment means
the request was recorded; poll until the task becomes terminal. The live worker is
stopped before the workflow reports cancellation.

Reuse the same `request_id` after an uncertain submit outcome. The first submission
wins within Restate retention; a duplicate attaches to that run, even if the caller
changes the brief. A deliberate new investigation or revision uses a new UUID.
Revision copies useful evidence from an authorized terminal run into a new workspace
and marks inherited sources as needing refresh. It does not overwrite the original.

Exhaustion produces a completed MCP task with an explicit exhausted research status
and partial report. A failed task includes its reason and partial report. A cancelled
task's available report remains retrievable using `research_report`. None of these
states claims successful, complete research.

## Recovery and operations

Restate persists workflow state, clarification promises, and assignment outcomes.
Completed workflow results, state, and promises are retained for seven days after
completion. The advertised task TTL is seven days from creation, conservatively
within that retention. Once retention expires, do not reuse an old request UUID.
Back up Restate data using its supported operational procedures.

The worker host stores private assignment files and current process outcomes under
the supplied workspace directory. Persist this directory across application restarts.
A saved completed result can be reconciled without another model call. An interrupted
external process that cannot be safely attached produces an explicit failure; revise
the research to continue from preserved material. Restate durability does not make
an external CLI process transparently recoverable across hosts.

Run one application replica for a workspace directory. Horizontal worker placement,
shared process attachment, and automatic migration of active CLI processes are not
supported. Preserve old service deployments for in-flight Restate invocations when
changing workflow code; arbitrary edits to replayed code can be incompatible with
recorded execution. Follow Restate's deployment/versioning guidance.

SIGINT and SIGTERM stop admission and active workers. Configure the container/runtime
to terminate the complete process group on a hard service kill. Use native
`tasks/cancel` for application cancellation; direct Restate administrative kill is
an emergency operation and does not provide the application's worker-stop contract.

Trace metadata is forwarded from the MCP request through the workflow to source
calls. It is correlation context, not an authorization identity. Neither traces nor
reports should contain gateway credentials or file-transfer authorization headers.

## Validation and remaining limits

Run `scripts/test-integration.sh` with Docker available. It starts an isolated,
pinned Restate server, runs the real service with fixture evidence, and removes the
test container. The walkthrough exercises discovery, duplicate submission,
clarification across an application restart, completion, revision, cancellation,
and deadline exhaustion. Ordinary `cargo test` explicitly skips this opt-in test.
The CI workflow runs it separately.

This validates lifecycle and protocol behavior, not model research quality or live
provider compatibility. Native text attachment ingestion and complete Markdown report delivery use the
[file adapter](files.md), with streamed bytes, verified ownership, and separate
transfer-ticket and research-material lifetimes.

## References

- [Restate service configuration and retention](https://docs.restate.dev/services/configuration)
- [Restate execution visibility and control](https://docs.restate.dev/ai/patterns/observability-control)
- [MCP Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
- [MCP Tasks](https://modelcontextprotocol.io/extensions/tasks/overview)
- [Codex worker setup](codex-runtime.md)
- [Source access and native source file delivery](source-access.md)
