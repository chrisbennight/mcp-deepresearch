# Codex runtime

The live adapter runs `codex exec`, initially targeting Codex CLI 0.157.0 on Linux.
`-p` selects a profile; it is not a noninteractive/print switch. The adapter uses
JSON events and structured final output, with the assignment prompt on stdin.
No paid-model API is called separately by the controller.

## Run

Authenticate a dedicated Codex home using the normal Codex login flow. Configure:

| Setting | Purpose |
|---|---|
| `DEEPRESEARCH_CODEX_HOME` | Dedicated authenticated Codex home, kept outside the repository |
| `DEEPRESEARCH_GATEWAY_URL` | Source gateway MCP endpoint |
| `DEEPRESEARCH_SOURCE_TOOLS` | Comma-separated search/read tool allowlist, using actual gateway tool names |
| `DEEPRESEARCH_SOURCE_TOKEN` | Separately scoped source credential used by the host; never an inbound caller assertion |
| `DEEPRESEARCH_FILE_ORIGINS` | Optional additional HTTPS origins approved for native file downloads |
| `DEEPRESEARCH_CODEX_EXECUTABLE` | Optional executable path, default `codex` |
| `DEEPRESEARCH_MODEL` | Optional model choice, otherwise the Codex default |
| `DEEPRESEARCH_REASONING_EFFORT` | Optional explicit reasoning setting; pin it for paired evaluation |
| `DEEPRESEARCH_MAX_WORKERS` | Concurrent worker processes, default `1` |

Then run:

```sh
cargo run --locked -- live request.json ./workspaces
```

Use an actual question and public-source constraints in `request.json`; the supplied
comparison example deliberately describes fixtures. The live command consumes the
configured account's allowance. Subscription availability and quota are not guaranteed.
Keep authenticated homes and workspaces private. Account refresh may write to the
Codex home; do not bake credentials into an image or copy them into committed examples.

The adapter explicitly enables Codex's `mcp_2026_07_28` feature for the current source protocol.
It ignores ordinary user configuration and rule files, disables shell tools,
subagents and built-in web search, and connects to the host-managed [source adapter](source-access.md) during
investigation. Under the default `staged` policy, writing and review use collected
evidence without a gateway connection. Experimental `evidence_access`, `adaptive`, and `perspective`
policies give every assignment source access and a paged index of successfully retained
material from earlier assignments in the same investigation.
Use a gateway credential restricted to source operations: never permit administrative
operations, generic code execution, or recursive research calls in that allowlist.
Read-only process sandboxing and gateway policy are independent controls.

## Execution and recovery

An assignment is identified by research ID and assignment number. A reconnect attaches
to the in-memory worker instead of starting another process. A completed result is
saved before callers receive it and can be retrieved after a new runtime instance.
The session identifier is available for diagnosis; session resume is not process
attachment and is not used as an automatic retry mechanism.

After a host restart, an unfinished worker record is reported as interrupted. It is
not silently launched again. Preserve the partial workspace and start an explicit
revision after reconciling the failed run. There is no exactly-once claim for external
model work. Run live workers in a container or service unit that kills the entire
process group on service death; an abrupt host/process crash cannot be cleaned up by
Rust destructors. Normal cancellation and deadlines kill and reap the owned process
group before acknowledging termination.

Capacity waits count against the assignment's elapsed-time allowance. The host source
adapter reserves a call from the assignment allowance before dispatch. Codex events
provide an additional observed count; gateway account quotas remain the gateway's
responsibility. The worker is stopped when its observed allowance is exceeded.
Model usage is taken from runtime events,
not model-authored numbers; absent counts remain unavailable.

The adapter does not persist raw event logs or stderr. It retains only the session
identifier, current tool count and terminal research result. Authentication and capacity
failures are reported without echoing raw payloads. Result transfer is a later layer;
retrieving an already completed result must not trigger another worker.

## Validation and limitations

Local process fixtures exercise concurrent attachment, restart retrieval, interrupted
execution, process-group cancellation and timeout. They use actual child processes
but never contact Codex, a model, or a gateway. An opt-in real-provider check is also available:

```sh
cargo test --locked --test live_codex -- --ignored
```

Set `DEEPRESEARCH_CODEX_HOME` and an account-supported `DEEPRESEARCH_MODEL` first.
Run as the owner of that dedicated authenticated home. This consumes the account's
allowance and uses only an invented local MCP source. It verified Codex CLI 0.157.0
with the current MCP feature enabled: actual source invocation, structured output,
and retained cited evidence. It does not establish live Kagi access or answer quality.
The full gateway/source walkthrough still requires operator configuration.

References:

- [Noninteractive Codex](https://developers.openai.com/codex/noninteractive)
- [Codex configuration reference](https://developers.openai.com/codex/config-reference)
