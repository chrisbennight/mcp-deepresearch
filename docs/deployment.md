# Local deployment and end-to-end walkthrough

This deployment runs one research host and one Restate server. It supports one
trusted principal per instance. Start with fixtures to verify the complete path
without a model account: native text upload, research submission, Tasks, a cited
report, and native report download. Fixtures demonstrate execution, not quality.

## Start the fixture service

Install Docker Compose and the repository's Rust toolchain. Supply a fresh local
`DEEPRESEARCH_INBOUND_TOKEN` through your shell's secret manager or another private
runtime mechanism; do not put it in a command argument, Git, or a shared `.env` file.
Then run:

```sh
docker compose up --build -d
curl --fail --retry 20 --retry-connrefused --retry-delay 1 http://127.0.0.1:8088/health
curl --fail --retry 20 --retry-connrefused --retry-delay 1 \
  -X POST http://127.0.0.1:9070/deployments \
  -H 'Content-Type: application/json' \
  -d '{"uri":"http://research:9080"}'
cargo run --locked -- walkthrough examples/comparison.json ./workspaces/walkthrough README.md
```

Restate may need a few seconds to start. If registration returns an error, inspect
its response and retry after readiness; do not enable forced registration on a
shared server. `/health` is public process liveness only. It does not promise that
Restate, source providers, or model authentication are healthy. The walkthrough is
the functional check. It prints the research/task IDs and writes the full report
only after a successful file transfer. A failed transfer may leave a `.part` file;
that file is not a completed report. The unattended client declines clarification,
so use an interactive MCP client when clarification matters.

Open [the local Restate UI](http://127.0.0.1:9070) to inspect the research invocation,
its current state, and investigation, writing, and review calls. Service records
are execution visibility, not an additional research ledger. The authoritative
research notes and report remain the mutable workspace in the workflow state.

The example publishes MCP and Restate management ports only on loopback. The
workflow endpoint is available only inside the Compose network. Do not expose the
Restate management API or workflow listener to an untrusted network. `docker
compose down` stops the example and preserves named volumes. Deleting volumes
removes research and execution data; do that only when you intend to discard it.

## Use a real account and sources

Build the `live` target through `compose.live.yaml`. It adds Codex CLI 0.157.0 to
the same application image. Authenticate a dedicated Codex home through the normal
[Codex login flow](https://developers.openai.com/codex/cli), and make that directory
readable and writable by container UID 10001. Keep it outside this repository.
Do not mount a personal home directory or the Docker socket into the service.
Account refresh writes to this directory; use one worker host for that account.

Supply `DEEPRESEARCH_CODEX_HOME`, `DEEPRESEARCH_MODEL`,
`DEEPRESEARCH_GATEWAY_URL`, `DEEPRESEARCH_SOURCE_TOOLS`, and
`DEEPRESEARCH_SOURCE_TOKEN` as described in [runtime setup](codex-runtime.md).
The source token must be separately scoped to approved read-only search and reading
tools. Use their actual discovered names, not invented aliases. The host uses that
credential; the worker receives only its bounded assignment endpoint. The inbound
token authorizes the gateway hop and must not double as the source credential.

```sh
docker compose -f compose.yaml -f compose.live.yaml up --build -d
# Register the service with Restate as above on a fresh installation.
cargo run --locked -- walkthrough evals/request.json ./workspaces/live
```

For an existing installation, drain or retain old deployments before registering
changed workflow code. Restarting the application is not a guarantee that an
in-flight external Codex process can be recovered. See [recovery behavior](service.md#recovery-and-operations).
The live command consumes the account's allowance; subscription access and provider
capacity are not guaranteed. No additional model API is called by the controller.
The baseline comparison also consumes that allowance and is deliberately opt-in.

The walkthrough defaults to the direct local service. For a gateway, set
`DEEPRESEARCH_MCP_URL` to its endpoint, supply the gateway credential through
`DEEPRESEARCH_INBOUND_TOKEN` for this client process, and set
`DEEPRESEARCH_TOOL_PREFIX` to its discovered tool-name prefix (for example,
`research.`). Native gateway files require an HTTPS public origin. Keep this
client credential separate from the service's own inbound token.

The committed Compose file is a local example. Behind a gateway, configure
`DEEPRESEARCH_FILE_ORIGIN` to the externally reachable service origin, set the
Host allowlist accordingly, and keep byte routes reachable by the gateway's native
file-transfer host. These routes use short-lived transfer authority. Keep TLS and
network policy at the gateway boundary; do not publish the workflow listener.

## Diagnose and operate

| Symptom | Check |
| --- | --- |
| Discovery returns unauthorized | Inbound token and configured principal; overlap current/previous tokens during rotation |
| Submit cannot reach a workflow | Restate readiness, deployment registration, and the private workflow address |
| Task waits for input | Answer or decline the current native Tasks elicitation; the deadline continues while waiting |
| Live worker fails immediately | Dedicated Codex login, account-supported model, source tool discovery and scope |
| Report exists but download fails | Request fresh download authority; do not submit the research again |
| Work fails after service interruption | Read the partial report, reconcile the account session, and create an explicit revision |

Do not paste authentication payloads, transfer descriptors, or worker output logs
into issues. Normal service errors omit those values. Back up Restate and the
research volume together. Monitor disk usage: uploaded material, retained research
files, worker outcomes, and published reports have operator-managed disk retention.
Restate completion retention does not delete those files. Apply volume quotas and
remove old terminal research material only according to your retention policy.
The example has a container memory and process limit; tune these for the workload.

## Evaluation

Run the [paired evaluation](evaluation.md) to compare the controller against a
single-session agent under the same source, wall-clock, and tool limits. Report
failures and incomplete runs along with answer quality, latency, and observed
usage. A green walkthrough does not establish that the system produces better
research. Gateway-native task routing depends on the gateway's Tasks support;
this repository does not implement Waygate's separate task-routing work.
