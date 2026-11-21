#!/usr/bin/env bash
# LLM adapter for a dry run: builds exactly what the metaharness adapter would run (frame and
# argv), prints the command on stderr and calls nothing. Same contract as the other adapters.
set -uo pipefail
for p in jq; do command -v "$p" >/dev/null 2>&1 || { echo "llm-adapter-dry.sh: $p is not on PATH" >&2; exit 127; }; done # program error, not a result
req=$(cat "$1")
rec=$(jq -r .record_dir <<< "$req")
mkdir -p "$rec"
# Reuse the metaharness adapter's frame and argv with a stand-in binary that only records.
stand_in=$rec/metaharness-stand-in
printf '#!/usr/bin/env bash\nexit 0\n' > "$stand_in" && chmod +x "$stand_in"
METAHARNESS=$stand_in "$(dirname "$0")/llm-adapter-metaharness.sh" "$1" > /dev/null
rm -f "$stand_in"
jq -r --arg p "$(jq -r .prompt_file <<< "$req")" '"would run: metaharness " + (.[1:] | map(if . == "<prompt_file>" then "\"$(cat " + $p + ")\"" else @sh end) | join(" "))' \
  "$rec/argv.json" >> "${SDC_LLM_DRY_LOG:-/dev/stderr}"
jq -n --arg model "$(jq -r .model <<< "$req")" '{adapter: "dry", model: $model, exit: 0, dry: true}'
