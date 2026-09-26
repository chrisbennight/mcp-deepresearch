#!/bin/sh
set -eu
cat >/dev/null
printf '%s\n' "$$" > parent.pid
sleep 30 &
printf '%s\n' "$!" > descendant.pid
printf '%s\n' '{"type":"thread.started","thread_id":"slow-fixture"}'
wait
