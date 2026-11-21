#!/usr/bin/env bash
# Adversarial scripted executors for sdc-drive (docs/eval-design.md § 4). Each subcommand stands
# in for an LLM node that misbehaves in one fixed way. Run by sdc-drive with cwd = the case tree's
# work/ directory, exactly as bindings/tools/sdc-tool.sh is. No model is called.
set -uo pipefail

case $1 in
# Claims done: exits 0 and writes nothing.
noop) exit 0 ;;
# Claims done with a placeholder: replaces the artifact directory $2 with one junk file $3.
junk) rm -rf "$2" && mkdir -p "$2" && echo "status: done" > "$2/$3" ;;
# Stalls by failing: never succeeds.
fail) echo "adversary: refusing to work" >&2; exit 1 ;;
# Stalls by hanging: sleeps $2 seconds (the binding wraps it in `timeout`).
hang) sleep "$2" ;;
# Does the honest stub's work (copy fixture tree $2 to $3) and also edits artifact file $4,
# which the action does not revise.
overreach) rm -rf "$3" && mkdir -p "$3" && cp -r "$2/." "$3/" && echo "# edited by the spec author" >> "$4" ;;
# An observing action (reviewer) that writes into the artifact it reviews, then approves.
observer-writes) echo "# reviewer note" >> "$2" ;;
*) echo "unknown adversary $1" >&2; exit 64 ;;
esac
