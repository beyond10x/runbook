#!/usr/bin/env bash
# LLM adapter: `claude -p` directly, for when metaharness cannot be used. Same contract as
# llm-adapter-metaharness.sh. The neutral operations map to Claude Code tool names: the only
# built-in tools offered (--tools) and the --allowedTools list; everything else is disallowed.
#
# Hermetic, as the metaharness adapter is (../metaharness/crates/metaharness-claude/src/launch.rs):
# `--setting-sources ""` (no user, project or local settings, so no CLAUDE.md or AGENTS.md of the
# tree, hooks, enabled plugins or output style), a fresh CLAUDE_CONFIG_DIR per call (no user
# CLAUDE.md, plugins, skills, agents or memory), `--strict-mcp-config` (no MCP servers),
# `--disable-slash-commands`, auto-memory and the git status block (user name, branch) off. The login is the one file `.credentials.json`
# copied into that config home for the call (mode 600, removed on exit; SDC_CLAUDE_CREDENTIALS,
# else $CLAUDE_CONFIG_DIR or ~/.claude), or ANTHROPIC_API_KEY from the environment.
# With SDC_GATEWAY_FILE (sdc-node: the binding's `--gateway-file`, a file of `runbook run`'s private directory
# with the address and the token) no login is used at all: the file is read by `sdc_gateway_read` of gateway.sh,
# `claude` gets ANTHROPIC_BASE_URL and ANTHROPIC_AUTH_TOKEN in the environment of that one start and a config
# home with no login; no other ANTHROPIC_* or CLAUDE_CODE_* variable of the caller and not the two SDC_ variables
# that name the file and a login are in its environment (the adapter's own CLAUDE_CODE_ settings below stay).
# What `claude` prints to its two streams goes through a filter that replaces every occurrence of the token by
# `<gateway token>` before `events.jsonl` and `stderr.txt` are written. A gateway file that names no token (a
# gateway without ingress auth) gives ANTHROPIC_AUTH_TOKEN the fixed placeholder `runbook-gateway-no-token`, no
# credential, and its output is not filtered. A file that cannot be read ends the
# call with exit 127 before `claude` starts, naming the variable and no value. With RUNBOOK_ROUTE_FILE (a daemon run on a
# route: `runbook.route/1`, the daemon's relay and the run's own token; docs/model-routes.md, section 11) the route file
# takes the place of a gateway file the same way, before it, and `claude` asks for the model the file gives the run's role (`runbook-<role>`; the role is the request's model
# when the file names a route for it, else `default`), not a model alias; RUNBOOK_ROUTE_FILE is not in `claude`'s environment.
# `claude` and what it starts get no descriptor of the two filters beyond their stdout and stderr (descriptors 4 and 5
# are closed for the start), and no descriptor above 2 that the caller left open.
# A call
# is the same on any host: its effort is the request's (set by sdc-node or the decider of `runbook run`), and
# CLAUDE_CODE_EFFORT_LEVEL, which would override it, is unset for the child.
set -uo pipefail
for p in jq claude; do command -v "$p" >/dev/null 2>&1 || { echo "llm-adapter-claude-cli.sh: $p is not on PATH" >&2; exit 127; }; done # program error, not a result
req=$(cat "$1")
r() { jq -r "$1" <<< "$req"; }
rec=$(r .record_dir)
mkdir -p "$rec"
# The config home holds the copy of the login. When the login is a file in a directory of the user's
# with mode 700 (the private directory of `runbook run`), the whole config home is made there,
# never in the record directory, which lies in the run directory: a SIGKILL of this script leaves the
# home, and the runner removes its private directory with all it holds. Otherwise (the driver's runs)
# it is made in the record directory, as it always was.
scratch=$rec
model=$(r .model)
route_file=${RUNBOOK_ROUTE_FILE:-}
if [[ -n $route_file ]]; then
  # A route (docs/model-routes.md, section 11): the run's route file names the daemon's relay and the run's own
  # token, never a gateway's credential. It takes the place of a gateway file, before any gateway file, key or
  # login of the environment: the address and the token go to the one start of `claude` as a gateway's do. The
  # model `claude` asks for is the one the file gives a role of the run (`runbook-<role>`, a name `claude` does not
  # take for an alias of its own): the role is the request's model when the file names a route for it, else
  # `default`; the relay sets the route's alias.
  SDC_GATEWAY_BASE_URL=$(jq -er 'select(.format == "runbook.route/1") | .base_url | strings' "$route_file" 2>/dev/null) &&
    SDC_GATEWAY_TOKEN=$(jq -er 'select(.format == "runbook.route/1") | .token | strings' "$route_file" 2>/dev/null) &&
    model=$(jq -er --arg m "$model" '(.routes // {}) as $r | (if ($r | has($m)) then $m else "default" end) as $role | ($r[$role].model // ("runbook-" + $role))' "$route_file" 2>/dev/null) ||
    { echo "llm-adapter-claude-cli.sh: RUNBOOK_ROUTE_FILE does not name a route file that can be read" >&2; exit 127; }
  SDC_GATEWAY_FILE=$route_file
  export -n SDC_GATEWAY_FILE
fi
if [[ -n ${SDC_GATEWAY_FILE:-} ]]; then
  # A gateway (docs/loom-run-settings.md, "[gateway]"): the file holds the address and the token, read by the
  # one reader of the bindings. It comes before any key or login of the environment, and the config home that
  # is made below holds no login. The file is in the runner's private directory, so is the config home.
  # shellcheck source=gateway.sh
  [[ -n $route_file ]] || { source "$(dirname -- "${BASH_SOURCE[0]}")/gateway.sh" && sdc_gateway_read "$SDC_GATEWAY_FILE"; } ||
    { echo "llm-adapter-claude-cli.sh: SDC_GATEWAY_FILE does not name a gateway file that can be read" >&2; exit 127; }
  # The two values are shell variables of this script and no process but the one `claude` start gets them: a
  # caller that exported either name does not make the filter and the other programs inherit the token.
  export -n SDC_GATEWAY_BASE_URL SDC_GATEWAY_TOKEN
  # A gateway without ingress auth names no token. `claude` still gets an auth token, this fixed placeholder, so
  # that it does not stop for a missing login; it is no credential and no secret, so its output is not filtered.
  gateway_token=$SDC_GATEWAY_TOKEN
  export -n gateway_token
  [[ -n $gateway_token ]] || gateway_token=runbook-gateway-no-token
  priv=$(dirname -- "$SDC_GATEWAY_FILE")
  [[ -d $priv && -O $priv && $(stat -c %a "$priv") == 700 ]] && scratch=$priv
elif [[ -z ${ANTHROPIC_API_KEY:-} && -n ${SDC_CLAUDE_CREDENTIALS:-} ]]; then
  priv=$(dirname -- "$SDC_CLAUDE_CREDENTIALS")
  [[ -d $priv && -O $priv && $(stat -c %a "$priv") == 700 ]] && scratch=$priv
fi
home=$(mktemp -d "$scratch/config-home.XXXXXX") && chmod 700 "$home" || exit 127
trap 'rm -rf "$home"' EXIT
if [[ -z ${SDC_GATEWAY_FILE:-} && -z ${ANTHROPIC_API_KEY:-} ]]; then
  # Loom gives a program PATH and LOOM_* alone, so HOME may be unset: no variable is read bare.
  creds=${SDC_CLAUDE_CREDENTIALS:-}
  if [[ -z $creds ]]; then
    config=${CLAUDE_CONFIG_DIR:-}
    [[ -n $config || -z ${HOME:-} ]] || config=$HOME/.claude
    [[ -z $config ]] || creds=$config/.credentials.json
  fi
  [[ -n $creds ]] || { echo "llm-adapter-claude-cli.sh: no login: none of SDC_CLAUDE_CREDENTIALS, CLAUDE_CONFIG_DIR, HOME and ANTHROPIC_API_KEY is set (a Loom program gets PATH alone: the binding passes --claude-credentials <file> to sdc-node)" >&2; exit 127; }
  [[ -f $creds ]] || { echo "llm-adapter-claude-cli.sh: no login ($creds) and no ANTHROPIC_API_KEY" >&2; exit 127; }
  install -m 600 "$creds" "$home/.credentials.json" || exit 127
fi
tools=$(jq -r '[.operations[] | {"file.read": "Read", "file.write": "Write", "file.edit": "Edit",
  "dir.list": "Glob", "search": "Grep", "shell": "Bash"}[.] // empty] | join(",")' <<< "$req")
# A money budget is optional: `--max-budget-usd` goes to the CLI only when the request names `max_budget_usd`.
budget=()
[[ -n $(r '.max_budget_usd // ""') ]] && budget=(--max-budget-usd "$(r .max_budget_usd)")
argv=("${CLAUDE:-claude}" -p "$(cat "$(r .prompt_file)")" --model "$model"
  --max-turns "$(r .max_turns)" "${budget[@]}"
  --disallowedTools "Bash,WebFetch,WebSearch,Task,Skill"
  --tools "$tools" --setting-sources "" --strict-mcp-config --disable-slash-commands
  --permission-mode acceptEdits --output-format stream-json --verbose --no-session-persistence)
# No operations (change.review: one request, no tools): `--tools ""` offers none and no allow list is passed.
[[ -n $tools ]] && argv+=(--allowedTools "$tools")
[[ -n $(r '.effort // ""') ]] && argv+=(--effort "$(r .effort)")
# change.implement in request mode (request `implement_mode` request, docs/driver.md "Implement as
# one request"): no tool at all (`--tools ""`), the prompt on stdin (its inlined files can pass the
# 128 KiB limit of one argv string), and the output limit from `max_output_tokens`.
stdin=/dev/null
extra_env=()
if [[ $(r '.implement_mode // ""') == request ]]; then
  argv=("${CLAUDE:-claude}" -p --model "$model" --max-turns "$(r .max_turns)" "${budget[@]}"
    --tools "" --disallowedTools "Bash,WebFetch,WebSearch,Task,Skill" --setting-sources "" --strict-mcp-config
    --disable-slash-commands --output-format stream-json --verbose --no-session-persistence)
  [[ -n $(r '.effort // ""') ]] && argv+=(--effort "$(r .effort)")
  stdin=$(r .prompt_file)
  [[ -n $(r '.max_output_tokens // ""') ]] && extra_env=(CLAUDE_CODE_MAX_OUTPUT_TOKENS="$(r .max_output_tokens)")
fi
# A request with `json_schema` (a schema file): `--json-schema`, the CLI validates the reply against
# it and re-prompts on a mismatch; the reply is the final result's `structured_output`, written to
# <record>/structured-output.json.
schema=$(r '.json_schema // ""')
[[ -n $schema && -s $schema ]] && argv+=(--json-schema "$(jq -c . "$schema")")
if [[ $stdin == /dev/null ]]; then
  printf '%s\0' "${argv[@]}" | jq -Rs 'split("\u0000") | .[0:2] + ["<prompt_file>"] + .[3:-1]
    | map(if startswith("{") then "<json_schema>" else . end)' > "$rec/argv.json"
else
  printf '%s\0' "${argv[@]}" | jq -Rs 'split("\u0000") | .[:-1] | map(if startswith("{") then "<json_schema>" else . end)
    + ["< <prompt_file>"]' > "$rec/argv.json"
fi
start=$(date +%s%3N)
# With a gateway the two values go into the environment of this one start (the subshell below, by the shell's own
# `export`: no argv of any process holds the token), every ANTHROPIC_* and CLAUDE_CODE_* name of the caller that
# the environment holds now, but the two, is removed from it (`env -u`; the adapter's own settings follow on the
# same line), and the variables that name the gateway file and a login path stay out of `claude`'s environment.
unset_names=()
filtered=''
if [[ -n ${SDC_GATEWAY_FILE:-} ]]; then
  while IFS= read -r -d '' kv; do
    case ${kv%%=*} in
      ANTHROPIC_BASE_URL | ANTHROPIC_AUTH_TOKEN) ;;
      ANTHROPIC_* | CLAUDE_CODE_*) unset_names+=(-u "${kv%%=*}") ;;
    esac
  done < <(env -0)
fi
if [[ -n ${SDC_GATEWAY_FILE:-} && -n ${SDC_GATEWAY_TOKEN:-} ]]; then
  filtered=1
  # Each of the two streams of `claude` goes through a filter before it is a file: every occurrence of the token, and
  # of its form inside JSON text, is replaced by `<gateway token>`. The token reaches the filter on a descriptor
  # (`jq --rawfile`), never in an argv or an environment variable.
  redact() {
    jq -Rr --rawfile t <(printf '%s' "$SDC_GATEWAY_TOKEN") '($t | tojson | .[1:-1]) as $e
      | (if $e == $t then [$t] else [$e, $t] end) as $keys | reduce $keys[] as $k (.; split($k) | join("<gateway token>"))'
  }
  exec 4> >(redact > "$rec/events.jsonl")
  filter_events=$!
  exec 5> >(redact 4>&- > "$rec/stderr.txt")
  filter_stderr=$!
else
  exec 4> "$rec/events.jsonl" 5> "$rec/stderr.txt"
fi
# A descriptor above 2 that the caller of this script left open without close-on-exec (a CI runner's pipe) is closed in
# the subshell that starts `claude`, so neither `claude` nor what it starts holds it; 4 and 5 are closed on the start line.
close_inherited() {
  local f n
  for f in /dev/fd/*; do
    n=${f##*/}
    [[ $n =~ ^[0-9]+$ ]] && ((n > 2 && n != 4 && n != 5)) && eval "exec $n>&-"
  done
  return 0
}
(cd "$(r .cwd)" && close_inherited && { [[ -z ${SDC_GATEWAY_FILE:-} ]] || {
    export ANTHROPIC_BASE_URL=$SDC_GATEWAY_BASE_URL ANTHROPIC_AUTH_TOKEN=$gateway_token
    unset SDC_GATEWAY_FILE SDC_GATEWAY_BASE_URL SDC_GATEWAY_TOKEN SDC_CLAUDE_CREDENTIALS RUNBOOK_ROUTE_FILE gateway_token
  }; } && env -u CLAUDE_CODE_EFFORT_LEVEL "${unset_names[@]}" "${extra_env[@]}" CLAUDE_CONFIG_DIR="$home" CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS=1 "${argv[@]}" < "$stdin" >&4 2>&5 4>&- 5>&-)
code=$?
exec 4>&- 5>&-
if [[ -n $filtered ]] && ! { wait "$filter_events" && wait "$filter_stderr"; }; then
  rm -f "$rec/events.jsonl" "$rec/stderr.txt"
  echo "llm-adapter-claude-cli.sh: the filter that takes the gateway token out of the output of claude failed; the output of the call is not kept" >&2
  exit 127
fi
end=$(date +%s%3N)
constrained=false
if [[ -n $schema && -s $schema ]] &&
   jq -se '(map(select(.type == "result")) | last | .structured_output) | type == "object"' "$rec/events.jsonl" > /dev/null 2>&1; then
  jq -s '(map(select(.type == "result")) | last | .structured_output)' "$rec/events.jsonl" > "$rec/structured-output.json"
  constrained=true
fi
jq -s --arg model "$(r .model)" --arg effort "$(r '.effort // "default"')" --argjson code "$code" --argjson ms "$((end - start))" --argjson sc "$constrained" \
  --arg schema "$schema" '
  (map(select(.type == "result")) | last) as $e
  | {adapter: "claude-cli", model: $model, effort: $effort, exit: $code, duration_ms: ($e.duration_ms // $ms), wall_ms: $ms,
     cost_usd: $e.total_cost_usd, num_turns: $e.num_turns, usage: $e.usage, is_error: $e.is_error,
     subtype: $e.subtype, schema_constrained: $sc,
     schema_note: (if $sc then "claude -p --json-schema: the CLI validated the reply against the schema (re-prompting on a mismatch)"
       elif $schema != "" then "a schema was passed, but the result has no structured_output" else null end),
     tool_calls: [.[] | select(.type == "assistant") | .message.content[]? | select(.type == "tool_use")
       | {name, input: (.input | tostring | .[0:300])}]}' "$rec/events.jsonl" 2>/dev/null \
  || jq -n --argjson code "$code" '{adapter: "claude-cli", exit: $code, error: "no readable output"}'
exit $code
