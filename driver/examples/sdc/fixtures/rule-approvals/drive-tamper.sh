#!/usr/bin/env bash
# Test fixture of the cycles rule-approvals-check-failed and rule-approvals-revision-changed
# (scripts/run-finding-cycles.sh), used as DRIVE (the sdc-drive of scripts/run-trial.sh). It passes
# every call to the real driver (TAMPER_REAL) and, in the window between the driver's `run` that
# stops the direction case at context.approve and the `approve` that follows, makes the evidence
# on disk disagree with the claim Canon had just evaluated true: the context check's record reads
# `finding` (TAMPER_MODE=result) or is about an earlier revision (TAMPER_MODE=revision). Before the
# real `approve` the record is put back, so Canon accepts the approval. What the rule reads in that
# window is what the cycles test: the decider must be asked, not rule:autopilot.
set -uo pipefail
real=${TAMPER_REAL:?the real sdc-drive}
mode=${TAMPER_MODE:?result or revision}
args=("$@")
verb=${args[4]:-}
cdir=""
for ((i = 0; i < ${#args[@]}; i++)); do [[ ${args[i]} == --case-dir ]] && cdir=${args[i + 1]:-}; done
case $verb in
  run)
    out=$("$real" "$@" 2>&1)
    code=$?
    while IFS= read -r d; do
      [[ -n $d && ! -e $d/.tampered ]] || continue
      ls "$d"/evidence/*-context.approve.json > /dev/null 2>&1 && continue
      f=$(grep -l '"result": "valid"' "$d"/evidence/*-context.check.json 2> /dev/null | sort | tail -n 1)
      [[ -n $f ]] || continue
      mkdir -p "$d/.evidence-backup" && cp "$f" "$d/.evidence-backup/"
      case $mode in
        result) jq '.result = "finding"' "$f" > "$f.tmp" ;;
        revision) jq '.subject_revision = "t0000000000000000"' "$f" > "$f.tmp" ;;
      esac && mv "$f.tmp" "$f"
      touch "$d/.tampered"
    done < <(find "$cdir" -type d -name '*.direction' 2> /dev/null)
    printf '%s\n' "$out"
    exit $code ;;
  approve)
    if [[ -d $cdir/.evidence-backup ]]; then
      cp "$cdir"/.evidence-backup/* "$cdir/evidence/" && rm -rf "$cdir/.evidence-backup"
    fi
    exec "$real" "$@" ;;
  *) exec "$real" "$@" ;;
esac
