#!/bin/sh
set -eu
output=''
mode='normal'
while [ "$#" -gt 0 ]; do
  if [ "$1" = '--output-last-message' ]; then
    shift
    output=$1
  elif [ "$1" = '--model' ]; then
    shift
    mode=$1
  fi
  shift
done
cat >/dev/null
printf 'started\n' >> launches.txt
printf '%s\n' '{"type":"thread.started","thread_id":"fixture-session"}'
printf '%s\n' '{"type":"item.started","item":{"type":"mcp_tool_call","id":"source-1"}}'
sleep 0.05
cat > "$output" <<'JSON'
{"sources":[],"findings":[],"uncertainties":[],"outline":[],"draft":"Fixture worker answer","next":{"action":"finish"},"usage":{"tool_calls":999,"input_tokens":999,"output_tokens":999}}
JSON
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":12,"output_tokens":8}}'

if [ "$mode" = 'fixture-malformed' ]; then
  printf 'not JSON' > "$output"
fi
if [ "$mode" = 'fixture-crash' ]; then
  exit 17
fi
