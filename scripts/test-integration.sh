#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
scratch="$(mktemp -d)"
container="deepresearch-restate-${scratch##*/}"
cleanup() {
  local status=$? ids
  trap - EXIT
  if ids=$(docker container ls -aq --filter "name=^/${container}$"); then
    if [[ -n "$ids" ]]; then docker rm -fv "$container" >/dev/null || status=1; fi
  else
    status=1
  fi
  rm -rf -- "$scratch" || status=1
  exit "$status"
}
trap cleanup EXIT
# This container belongs only to this test invocation, including its disposable data.
container_id=$(docker run -d --name "$container" --add-host host.docker.internal:host-gateway \
  -p 127.0.0.1::8080 -p 127.0.0.1::9070 \
  docker.restate.dev/restatedev/restate:1.7.0@sha256:1ffcff010a4a857553ab9fe3e6efa2a1086e7954768e1f3fbfb243ce0ffe28c2)
export DEEPRESEARCH_TEST_RESTATE_ADMIN="http://$(docker port "$container_id" 9070/tcp)"
export DEEPRESEARCH_TEST_RESTATE_INGRESS="http://$(docker port "$container_id" 8080/tcp)"
cargo test --locked --test restate_mcp -- --ignored
