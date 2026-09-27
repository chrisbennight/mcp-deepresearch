# Source access

Each investigation assignment gets a temporary, authenticated loopback MCP endpoint
inside the research process. The worker sees only the configured source tools and
`read_source_material`. The host calls the upstream gateway using a separate source
credential. Neither that credential nor file-transfer descriptors enter the worker's
prompt, environment, or saved research output. The worker receives a short-lived
credential for its own assignment endpoint instead.

The host and worker endpoints use MCP 2026-07-28 discovery and stateless Streamable
HTTP. They do not initialize legacy sessions or fall back to an older transport.
The selected Rust MCP SDK is recorded in Cargo.lock. Codex compatibility must also
be established by the opt-in live walkthrough; SDK support alone is not evidence
that the executable installed by an operator supports this wire profile.

## Operator configuration

Set `DEEPRESEARCH_GATEWAY_URL`, `DEEPRESEARCH_SOURCE_TOOLS`, and, when required,
`DEEPRESEARCH_SOURCE_TOKEN`. The tool list contains exact names returned by the
configured gateway's discovery. Select Kagi search and extraction tools initially;
other read tools can use the same interface. Tool arguments and descriptions come
from the gateway rather than an application-specific copy of a provider's schema.

Only explicitly selected source tools are admitted. Operators select read-only
operations and restrict the credential to them through gateway policy. The optional
MCP read-only hint may be absent (as it is for Kagi); an explicit writable hint is
rejected. The assignment proxy publishes a read-only hint for its selected source
tools so workers can classify them without interactive approval. This describes the
operator-authorized source contract; annotation hints do not grant authority.
Do not select administration, general code execution, or recursive research tools.
The local allowlist is an additional boundary, not a replacement for gateway policy.

By default file downloads must use the same origin as the configured gateway.
`DEEPRESEARCH_FILE_ORIGINS` can add comma-separated operator-approved HTTPS origins
when the gateway uses separate file delivery. Redirects are not followed. Plain
HTTP is accepted only for the exact origin of an already cleartext gateway endpoint.
The host sends only descriptor-provided headers to file endpoints; it does not
forward the gateway credential to them.

## Reading and failure handling

Search results are leads. A snippet does not establish that the full document was
read. The worker instructions require actual extraction before claiming to have read
source text, and distinguish retrieved evidence from interpretation.

Inline material is saved in the assignment directory. File-backed results use the
native draft SEP-2631 `files/authorizeDownload` flow. Bytes stream into a temporary
file, with a 16 MiB ingestion limit, then become available after any supplied size
and SHA-256 integrity requirements match. This digest is a file-transfer check,
not a research history or transaction ledger. The current reader accepts UTF-8 text;
use a configured extraction tool for PDFs and other binary formats.

The inline JSON limit is applied after the MCP SDK decodes the response; it limits
retained material, not peak transport memory. Configure the trusted gateway to use
file-backed delivery for large responses and give the service a container memory
limit. A bounded JSON decoder is not implemented in this adapter.

Mixed results retain both structured metadata and distinct text blocks. Nested
`mcp-file:` references outside the supported retained-result envelopes are reported
as an explicit delivery limitation; their presence does not mean their bytes were read.

The worker receives up to 32,000 characters with a material identifier and the next
character offset when more is available. `read_source_material` reads subsequent
sections from the saved copy. Transfer URLs and headers stay host-side. Files stay
with the private workspace until its operator-managed retention removes them;
temporary gateway transfer authorization does not determine local evidence lifetime.

A successful source operation with failed delivery returns those two outcomes
separately. The worker can retry `read_source_material` using the returned identifier;
this obtains file delivery again without repeating the source operation. A failure
or missing document remains an explicit limitation in the research answer.

All allowed calls, including follow-up material reads, reserve from the assignment
allowance before dispatch. Calls have timeouts and inherit assignment cancellation.
No generic request method or arbitrary file-reference lookup is exposed to the worker.

## Validation

The local integration test runs actual HTTP and MCP clients against a fixture gateway.
It exercises current discovery, source-tool filtering, request-local native file
capabilities, host-only transfer headers, failed delivery recovery without duplicate
source calls, and exhaustion of the source-call allowance. It makes no live provider
calls and is not a research-quality evaluation.

## References

- [MCP 2026-07-28 Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
- [Draft file-transfer proposal SEP-2631](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2631)
- [Waygate file-transfer conventions](https://github.com/chrisbennight/waygate/blob/main/docs/file-transfer.md)
- [Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
