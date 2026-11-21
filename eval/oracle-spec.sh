#!/usr/bin/env bash
# Hidden oracle for spec.author (docs/eval-design.md § 5.5): the conformance suite synthesized from
# the authored specification, run against ESS's built-in billing reference implementation, must
# execute at least one scenario and pass. The agent never sees this check or its output.
#
#   eval/oracle-spec.sh <spec-dir> <scratch-dir>
#
# Prints one line `held|contradicted|unknown <passed>/<total>`; exit 0 held, 1 contradicted,
# 2 unknown (ess missing or the spec directory absent).
set -uo pipefail
spec=$1 tmp=$2
command -v ess >/dev/null && [[ -d $spec ]] || { echo "unknown 0/0"; exit 2; }
rm -rf "$tmp" && mkdir -p "$tmp" && cp -r "$spec" "$tmp/spec"
cd "$tmp" || exit 2
if ! ess verify conform synthesize --path spec --suite-format 5 --out suite.json >/dev/null 2>&1; then
  echo "contradicted 0/0"; exit 1
fi
ess verify conform run --path spec --target billing --suite suite.json --report-format 2 \
  --report-out report.json --strict >/dev/null 2>&1
if [[ ! -s report.json ]]; then echo "contradicted 0/0"; exit 1; fi
read -r status passed total < <(jq -r '"\(.conformance_status) \(.counts.passed) \(.counts.total)"' report.json)
if [[ $status == passed && $total -gt 0 ]]; then echo "held $passed/$total"; exit 0; fi
echo "contradicted $passed/$total"; exit 1
