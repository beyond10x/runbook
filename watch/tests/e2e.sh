#!/usr/bin/env bash
# End-to-end check: a copy of a finished run, sdc-watch on it, and one imitated action that edits
# impl/src/lib.rs with sed -i and rustfmt and adds a file. Prints the resulting changes.jsonl.
#
#   watch/tests/e2e.sh <scratch-dir> [<source-run>]   (default source: runs/trial-todo-1, only read)
set -euo pipefail
sdc=$(cd "$(dirname "$0")/../.." && pwd)
scratch=${1:?scratch dir}
src=${2:-$sdc/runs/trial-todo-1}
run=$scratch/e2e/$(basename "$src")
rm -rf "$run"
mkdir -p "$scratch/e2e"
cp -a "$src" "$run"

"$sdc/scripts/watch-run.sh" start "$run"
case_dir=$run/store/IDEA-1/children/IDEA-1.todolist/children/IDEA-1.todolist.storage
work=$case_dir/work
log=$case_dir/events.jsonl
at() { date -u +%Y-%m-%dT%H:%M:%SZ; }
n=$(wc -l < "$log")

printf '{"seq":%d,"at":"%s","event":"action_started","action":"change.implement","argv":["imitation"],"llm":false}\n' "$n" "$(at)" >> "$log"
sleep 0.5
(cd "$work" && sed -i '$a pub fn added_by_sed( )->u32{ 42 }' impl/src/lib.rs)
sleep 0.5
(cd "$work" && rustfmt --edition 2021 impl/src/lib.rs)
sleep 0.5
(cd "$work" && printf '# Notes\n\nadded by the imitated action\n' > impl/NOTES.md)
sleep 0.5
printf '{"seq":%d,"at":"%s","event":"action_finished","action":"change.implement","code":"0"}\n' "$((n + 1))" "$(at)" >> "$log"
sleep 0.5
(cd "$work" && echo "after the action" > impl/AFTER.md)
sleep 0.5
"$sdc/scripts/watch-run.sh" stop "$run"

echo "--- $run/fs/changes.jsonl"
cat "$run/fs/changes.jsonl"
