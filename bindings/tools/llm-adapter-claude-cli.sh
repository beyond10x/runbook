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
# else $CLAUDE_CONFIG_DIR or ~/.claude), or ANTHROPIC_API_KEY from the environment. A call
# is the same on any host: its effort is the request's (set by sdc-node / decider.sh), and
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
if [[ -z ${ANTHROPIC_API_KEY:-} && -n ${SDC_CLAUDE_CREDENTIALS:-} ]]; then
  priv=$(dirname -- "$SDC_CLAUDE_CREDENTIALS")
  [[ -d $priv && -O $priv && $(stat -c %a "$priv") == 700 ]] && scratch=$priv
fi
home=$(mktemp -d "$scratch/config-home.XXXXXX") && chmod 700 "$home" || exit 127
trap 'rm -rf "$home"' EXIT
if [[ -z ${ANTHROPIC_API_KEY:-} ]]; then
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
argv=("${CLAUDE:-claude}" -p "$(cat "$(r .prompt_file)")" --model "$(r .model)"
  --max-turns "$(r .max_turns)" --max-budget-usd "$(r .max_budget_usd)"
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
  argv=("${CLAUDE:-claude}" -p --model "$(r .model)" --max-turns "$(r .max_turns)" --max-budget-usd "$(r .max_budget_usd)"
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
(cd "$(r .cwd)" && env -u CLAUDE_CODE_EFFORT_LEVEL "${extra_env[@]}" CLAUDE_CONFIG_DIR="$home" CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS=1 "${argv[@]}" < "$stdin" > "$rec/events.jsonl" 2> "$rec/stderr.txt")
code=$?
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
