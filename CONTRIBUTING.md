# Contributing

Describe the user problem and intended behavior before proposing a change. Keep
implementation choices in the code and concise design notes. Issues capture purpose,
constraints, and acceptance outcomes, not an entire prescribed architecture.

Use a task branch and an isolated worktree under `.worktrees/`. Keep changes small
and coherent. Run the README build, test, formatting, and Clippy commands before
opening a pull request. Tests should prove observable behavior, especially research
limits, recovery, citation support, and delivery. Ordinary CI must not call live
models or require account credentials. Mark opt-in integration checks separately.

Verify package identity, provenance, maintenance, and fit before adding dependencies.
Commit Cargo.lock for the application. Maintainers review dependency updates through
ordinary pull requests and CI; no private update bot is required. Use public registry
URLs in committed files. Build caches are an operator choice.

Do not commit credentials, private hostnames, operator paths, or source documents.
Document limitations honestly. Do not claim exactly-once external model execution or
research quality from deterministic fixture tests.

Releases are a maintainer action after reviewed changes and validation. Public build
artifacts must be independent of a particular operator's deployment. This bootstrap
does not automatically publish packages, container images, or releases.
