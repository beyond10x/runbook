# eval/

History. `reports/` and `baseline.json` are the record of the adversary evaluation of the old
driver (`docs/eval-design.md`).

- The scripts that made the record (`run.sh`, `run-adversaries.sh`, `adversary.sh`) drove the
  driver binary and are deleted with it. They are in the git history before the removal of the
  driver runtime.
- They are not ported: a port to `runbook test` entries is not done.
- `oracle-spec.sh`, `levels/`, `personas/` and `quality/` stay as they were.
