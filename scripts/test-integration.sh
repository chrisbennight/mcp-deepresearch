#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# This container belongs only to this test invocation, including its disposable data.
container_id=$(docker run -d --add-host host.docker.internal:host-gateway \
  -p 127.0.0.1::8080 -p 127.0.0.1::9070 \
  docker.restate.dev/restatedev/restate:1.7.0@sha256:1ffcff010a4a857553ab9fe3e6efa2a1086e7954768e1f3fbfb243ce0ffe28c2)
trap 'docker rm -fv "$container_id" >/dev/null' EXIT
export DEEPRESEARCH_TEST_RESTATE_ADMIN="http://$(docker port "$container_id" 9070/tcp)"
export DEEPRESEARCH_TEST_RESTATE_INGRESS="http://$(docker port "$container_id" 8080/tcp)"
cargo test --locked --test restate_mcp -- --ignored
