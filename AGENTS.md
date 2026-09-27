# Contributor guidance

Build the useful research capability; avoid speculative frameworks and accounting
ledgers. Read README.md and docs/architecture.md before changing integration seams.
Keep public configuration portable and free of operator-specific data and secrets.

Use an isolated task branch and worktree under the repository's ignored `.worktrees/`.
Preserve other contributors' changes. Stage intended files only. Never force-push or
bypass hooks. Run the README checks and read explicit successful exit statuses before
committing or pushing. Review against user-visible outcomes, not implementation trivia.

The application owns research decisions and mutable notes. Restate owns execution.
All model work, including benchmark grading, uses the runtime adapter.
The baseline uses public data and existing subscription/source access only.
MCP gateway policy remains authoritative.
Do not add hidden paid-model calls or replace native Tasks with undocumented polling.
