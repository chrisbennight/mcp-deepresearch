#!/usr/bin/env bash
set -euo pipefail
scratch="$(mktemp -d)"
trap 'rm -rf -- "$scratch"' EXIT
curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 \
  --retry 3 --connect-timeout 10 --max-time 120 \
  --output "$scratch/actionlint.tar.gz" \
  https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_amd64.tar.gz
printf '%s  %s\n' 8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8 "$scratch/actionlint.tar.gz" | sha256sum --check --strict
tar --no-same-owner -xzf "$scratch/actionlint.tar.gz" -C "$scratch" actionlint
"$scratch/actionlint" .github/workflows/*.yml
