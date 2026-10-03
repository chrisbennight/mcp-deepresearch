#!/usr/bin/env bash
set -euo pipefail
rust=false; python=false; integration=false; workflows=false
case "${GITHUB_EVENT_NAME:?event is required}" in
  workflow_dispatch|schedule) full=true ;;
  push|pull_request) full=false; if [[ "$GITHUB_EVENT_NAME" == push && "${GITHUB_REF:-}" == refs/tags/* ]]; then full=true; fi ;;
  *) echo 'Unsupported CI event' >&2; exit 1 ;;
esac
if [[ "$full" == true ]]; then
  rust=true; python=true; integration=true; workflows=true
else
  [[ "${BASE_SHA:-}" =~ ^[0-9a-f]{40}$ ]] || { echo 'A full base commit is required' >&2; exit 1; }
  changed_files="$(mktemp)"
  trap 'rm -f "$changed_files"' EXIT
  if [[ "$GITHUB_EVENT_NAME" == pull_request ]]; then
    git diff --name-only --no-renames -z "$BASE_SHA...HEAD" >"$changed_files"
  else
    git diff --name-only --no-renames -z "$BASE_SHA" HEAD >"$changed_files"
  fi
  while IFS= read -r -d '' path; do
    case "$path" in
      .ci/select-checks.sh|.github/workflows/ci.yml) rust=true; python=true; integration=true; workflows=true ;;
      .github/workflows/*|.ci/check-workflows.sh) workflows=true ;;
      Cargo.toml|Cargo.lock|rust-toolchain.toml|.cargo/*) rust=true; integration=true ;;
      src/main.rs|src/lib.rs|src/mcp.rs|src/lifecycle.rs|src/controller.rs|src/research.rs|src/runtime.rs|src/workspace.rs|src/files.rs) rust=true; integration=true ;;
      tests/restate_mcp.rs) integration=true ;;
      src/*|prompts/*|build.rs|tests/*.rs|tests/fixtures/*|rustfmt.toml|.rustfmt.toml|clippy.toml|.clippy.toml) rust=true ;;
      scripts/test-integration.sh) integration=true ;;
      scripts/benchmarks/*|scripts/benchmark.py) python=true ;;
    esac
  done <"$changed_files"
fi
printf 'rust=%s\npython=%s\nintegration=%s\nworkflows=%s\n' "$rust" "$python" "$integration" "$workflows" >>"${GITHUB_OUTPUT:?output file is required}"
