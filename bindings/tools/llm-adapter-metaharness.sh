#!/usr/bin/env bash
# LLM adapter: one bounded, non-interactive agent session through metaharness (Claude Code, the
# operator's login on this host). Contract shared by every adapter (docs/driver.md, "LLM
# executor"):
#
#   llm-adapter-<name>.sh <request.json>
#
# request: {model, effort (optional), prompt_file, cwd, operations: [metaharness neutral ops], max_turns,
# max_budget_usd, record_dir, node, attempt}. The adapter runs the session with cwd = request.cwd,
# writes everything it observed under record_dir (argv.json, events.jsonl, transcripts/) and
# prints one response JSON on stdout: {adapter, model, exit, duration_ms, cost_usd, num_turns,
# usage, tool_calls, observed_models, is_error}. Its exit is the session's.
#
# Injection points: SDC_LLM_ENDPOINT (a model gateway root URL: metaharness --model-endpoint, with
# --credentials none), METAHARNESS (the binary) and SDC_CLAUDE_CREDENTIALS (the login file).
#
# The login. `--credentials operator-login`, metaharness's default, copies `$HOME/.claude/.credentials.json`
# of metaharness's own environment into the scratch config home of the session (metaharness
# `credentials_file`; it reads HOME and no CLAUDE_CONFIG_DIR), and puts `$HOME/.local/bin` first on
# the child's PATH, then /usr/local/bin:/usr/bin:/bin. A program bound under Loom has PATH and LOOM_*
# alone, so there is no HOME. When SDC_CLAUDE_CREDENTIALS names a file (sdc-node: the binding's
# `--claude-credentials`) in a directory of the user's with mode 700 (the private directory of
# `runbook run`) and `$HOME/.claude/.credentials.json` is no file, the adapter makes a private
# HOME for the metaharness process alone, mode 700, in that directory (never in the record directory,
# which a SIGKILL would leave with the login in it): `.claude/.credentials.json` (a copy, mode 600) and
# `.local/bin/claude` (a link to the `claude` of this PATH, which the reduced child PATH would not
# find in the operator's own `~/.local/bin`). TMPDIR is that HOME too, so the scratch root of
# metaharness (its own copy of the login, in a config home) lies there as well. The directory is
# removed on exit, also after a failed call; after a SIGKILL the runner removes it with its own.
# In every other case nothing changes: metaharness gets the HOME the caller has (the driver's runs).
# With SDC_LLM_ENDPOINT the credentials are none and no login is copied.
set -uo pipefail
for p in jq "${METAHARNESS:-metaharness}"; do command -v "$p" >/dev/null 2>&1 || { echo "llm-adapter-metaharness.sh: $p is not on PATH" >&2; exit 127; }; done # program error, not a result
req=$(cat "$1")
r() { jq -r "$1" <<< "$req"; }
rec=$(r .record_dir)
mkdir -p "$rec/transcripts"

# The frame: the operations admitted here, sealed with metaharness.frame/1's digest (sha256 of the
# compact, key-sorted document without `digest` and `format`).
jq -S --arg node "$(r .node)" --argjson attempt "$(r .attempt)" '{
    workflow: {id: "sdc", version: "1"}, node: {id: $node},
    step: {workflow: "sdc", state: $node, index: 1, attempt: $attempt},
    prior: [], obligations: [{asked_by: null, text: "write the output under out/ only"}],
    reaching: [], next: [], handoff: {handoff: "none"},
    operations: ([.operations[] | {op: .}] | sort_by(.op)), entities: null}' <<< "$req" > "$rec/frame.body.json"
digest=$(jq -cjS . "$rec/frame.body.json" | sha256sum | cut -d' ' -f1)
jq -S --arg d "$digest" '. + {format: "metaharness.frame/1", digest: $d}' "$rec/frame.body.json" > "$rec/frame.json"
rm -f "$rec/frame.body.json"

argv=("${METAHARNESS:-metaharness}" run claude --frame "$rec/frame.json" --decisions frame
  --cwd "$(r .cwd)" --scratch-cwd --model "$(r .model)" --max-turns "$(r .max_turns)"
  --max-budget-usd "$(r .max_budget_usd)" --retain-dir "$rec/transcripts")
[[ -n $(r '.effort // ""') ]] && argv+=(--effort "$(r .effort)")
home_env=()
if [[ -n ${SDC_LLM_ENDPOINT:-} ]]; then
  argv+=(--model-endpoint "$SDC_LLM_ENDPOINT" --credentials none)
elif [[ -n ${SDC_CLAUDE_CREDENTIALS:-} && ! -f ${HOME:-/nonexistent}/.claude/.credentials.json ]] &&
  priv=$(dirname -- "$SDC_CLAUDE_CREDENTIALS") && [[ -d $priv && -O $priv && $(stat -c %a "$priv") == 700 ]]; then
  [[ -f $SDC_CLAUDE_CREDENTIALS ]] || { echo "llm-adapter-metaharness.sh: SDC_CLAUDE_CREDENTIALS names no file" >&2; exit 127; }
  # In the private directory of the login and never in the record directory: a SIGKILL of this
  # script leaves the home, and the runner removes its private directory with all it holds.
  home=$(mktemp -d "$priv/h.XXXXXX") && chmod 700 "$home" || exit 127
  trap 'rm -rf "$home"' EXIT
  trap 'exit 143' TERM INT HUP
  mkdir -m 700 "$home/.claude" "$home/.local" "$home/.local/bin" || exit 127
  install -m 600 "$SDC_CLAUDE_CREDENTIALS" "$home/.claude/.credentials.json" || exit 127
  if claude_bin=$(command -v claude); then
    [[ $claude_bin == /* ]] || claude_bin=$PWD/$claude_bin
    ln -s "$claude_bin" "$home/.local/bin/claude" || exit 127
  fi
  # TMPDIR too: metaharness makes its scratch root, which holds a second copy of the login in the
  # config home, below the temporary directory, and a SIGKILL of metaharness leaves it there.
  home_env=(HOME="$home" TMPDIR="$home")
fi
argv+=(-p "$(cat "$(r .prompt_file)")")
printf '%s\0' "${argv[@]}" | jq -Rs 'split("\u0000") | .[:-2] + ["<prompt_file>"]' > "$rec/argv.json"

start=$(date +%s%3N)
env "${home_env[@]}" "${argv[@]}" < /dev/null > "$rec/events.jsonl" 2> "$rec/stderr.txt"
code=$?
end=$(date +%s%3N)

jq -s --arg model "$(r .model)" --arg effort "$(r '.effort // "default"')" --argjson code "$code" --argjson ms "$((end - start))" '
  (map(select(.event == "session.ended")) | last) as $e
  | (map(select(.event == "tool.decided")) | map({key: .call_id, value: .}) | from_entries) as $dec
  | {adapter: "metaharness", model: $model, effort: $effort, exit: $code, duration_ms: ($e.duration_ms // $ms),
     wall_ms: $ms, cost_usd: $e.total_cost_usd, num_turns: $e.num_turns, usage: $e.usage,
     observed_models: $e.observed_models, is_error: $e.is_error, subtype: $e.subtype,
     terminal_reason: $e.terminal_reason, api_error_status: $e.api_error_status,
     offered_tools: (map(select(.event == "session.started")) | last | .offered_tools),
     tool_calls: [.[] | select(.event == "tool.requested")
       | {name, operations, input: (.input | tostring | .[0:300]),
          decision: ($dec[.call_id].decision // $dec[.call_id].verdict // "allowed")}],
     stream_end: (map(select(.event == "stream.closed")) | last | {reason, process})}' \
  "$rec/events.jsonl" 2>/dev/null || jq -n --arg model "$(r .model)" --arg effort "$(r '.effort // "default"')" --argjson code "$code" \
  --argjson ms "$((end - start))" '{adapter: "metaharness", model: $model, effort: $effort, exit: $code, wall_ms: $ms,
  error: "no readable event stream"}'
exit $code
