# Attachments and complete report delivery

The service implements the native draft SEP-2631 methods `files/authorizeUpload`
and `files/authorizeDownload`. Only references cross MCP JSON-RPC; the host streams
bytes on the authorized HTTP endpoint. The adapter is isolated in `files.rs` because
the draft can change. It does not add a second gateway, DPoP exchange, or client helper.
Clients without native support use their gateway's existing file-transfer helper.

Declare request-local file capabilities under
`_meta["io.modelcontextprotocol/clientCapabilities"].files`:

```json
{"upload":true,"download":true,"transports":["https"]}
```

An upload authorization accepts optional `name`, `mimeType`, `size`, and `digest`.
It returns a `file` reference and an `upload` descriptor. Follow its PUT URL and
headers from the host, then submit the returned URI string in
`request.attachments`. Tool discovery annotates these strings with `x-mcp-file`.
The initial reader accepts UTF-8 text, Markdown, CSV, and JSON, up to 16 MiB per
file and sixteen attachments per investigation including inherited attachments.
Extract PDFs or other binary documents before upload. File checks verify transfer
bytes; they do not claim malware scanning or source accuracy.

Set `DEEPRESEARCH_FILE_ORIGIN` to the externally reachable HTTPS origin that routes
`/files/*` to this service. Its default is `http://127.0.0.1:8088` for local use;
cleartext is accepted only for loopback configuration. Discovery advertises that
origin's actual transport; local HTTP clients must include `http` in their file
capabilities. Descriptors always match the URL scheme. Waygate advertises HTTP
only when its upstream MCP connection permits the pinned cleartext exception.
Transfer headers carry
short-lived authorization. Never put them in a prompt, log, issue, or shared URL.
They are distinct from both inbound MCP authentication and outbound source access.

The service publishes only completed uploads whose supplied size and SHA-256 match.
A digest uses base64url without padding. Failed/incomplete uploads cannot be admitted
as evidence. Upload tickets authorize one attempt; if delivery is uncertain, try
`files/authorizeDownload` on the returned file URI to confirm publication before
uploading again. A failed attempt requires a new upload authorization.

Admitted attachments are retained under the research directory independently of
upload tickets and the original upload copy. Revisions retain their own reference
to the same unmodified bytes, while their notes and drafts remain independently
writable. Workers receive selected excerpts and an attachment index; the existing
`read_source_material` tool can read further sections without sending the full
attachment to the model. Attachment names and contents are untrusted evidence.

A terminal report includes a bounded inline preview and, for a file-capable caller,
a stable Markdown `FileValue` with name, type, size, and SHA-256. The output schema
describes that file. Authorize a download using its URI, then follow the GET descriptor
from the host. Reports retain their original bytes after publication. An active
research report is a bounded snapshot; full file publication waits for a terminal
state so one stable file reference cannot later describe different bytes.

Research success and file delivery are separate. If report publication fails,
the response preserves the research status and preview and reports delivery as
unavailable with `retry_operation: false`. Retry `research_report` to publish or
retrieve the completed result; do not submit the investigation again. If a transfer
ticket expires or is invalidated by an application restart, authorize another
download for the same file. Completed research is not rerun.

## Ownership and retention

The authenticated gateway connection maps to the configured trusted principal.
Stored upload/report metadata records that owner; a different principal cannot
authorize the file. Transfer authorization is checked again at the byte endpoint.
The byte endpoint accepts only its file-scoped ticket, never an MCP bearer as a
substitute. Tickets expire after five minutes and are held only in memory; a restart
revokes them. Metadata contains no transfer credentials.

File data lives in the service workspace under `files/uploads`, `files/reports`, and
`files/research/<research-id>`. Protect the entire workspace as confidential data.
Data retention is operator-managed and independent of ticket expiry and Restate's
completed-workflow retention. There is no automatic file deletion in this release.
Set an appropriate filesystem/container storage quota. Remove abandoned upload copies
and expired report copies according to the deployment's retention policy; remove a
research directory only after that run and any intended revisions no longer need it.
Deleting a file does not change a completed research outcome, but can make delivery
unavailable. In-flight work requires its retained attachments to remain present.

## Evidence

The isolated Restate walkthrough covers upload, discovery annotations, submission,
application restart, report download, revoked-ticket renewal, and revision after
removal of the original upload copy. HTTP file tests cover a failed size check,
owner separation, missing transfer authority, and retrieval without research work.
Source tests cover embedded MCP text resources with attribution. These tests verify
file and lifecycle contracts; they do not evaluate the truth of a research answer.

- [Draft SEP-2631](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2631)
- [Waygate file-transfer profile](https://github.com/chrisbennight/waygate/blob/main/docs/file-transfer.md)
- [Service configuration and lifecycle](service.md)
