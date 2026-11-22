#!/bin/sh
# Setup step of the slow lane `task test:replay` (runbook/.config/nextest.toml, profile `replay`, script `replay-runs`): makes the
# runs of `runbook run --recorded` that tests/loom_run/replay.rs reads, once per test run and all at
# once, so that no test makes a run of its own. A run is the stub cycle of about 70 Loom steps.
# Run from runbook/ (cargo-nextest does). Needs driver/target/debug built.
set -u

here=$(pwd)
repo=$(cd "$here/.." && pwd)
target=${CARGO_TARGET_DIR:-$here/target}
bin=$target/debug/runbook
drv=$repo/driver/target/debug
rec=$repo/driver/examples/sdc/fixtures/recorded/stub
intake=$repo/driver/examples/sdc/fixtures/idea.yaml

for f in "$bin" "$drv/sdc-replay" "$drv/sdc-node"; do
    [ -x "$f" ] || { echo "replay-runs: $f is not built (cargo build --locked --bins in driver/)" >&2; exit 1; }
done

base=${RUNBOOK_REPLAY_WORK:-${XDG_STATE_HOME:-$HOME/.local/state}/runbook/replay-tests}
mkdir -p "$base"
find "$base" -maxdepth 1 -name 'run-*' -mmin +120 -exec rm -rf {} + 2>/dev/null
work=$(mktemp -d "$base/run-XXXXXX") || exit 1

scenario() {
    name=$1
    shift
    dir=$work/$name
    mkdir -p "$dir"
    echo '{}' > "$dir/settings.json"
    env -i PATH="$drv:$PATH" HOME="$HOME" "$bin" run sdc --run "$name" --runs-dir "$dir/runs" \
        --intake "$intake" --confinement local --settings "$dir/settings.json" \
        --spend-ledger "$dir/ledger.json" --cap-micro-usd 100000000 --max-llm-calls 100 \
        --max-wall-seconds 600 --recorded "$rec" "$@" > "$dir/stdout.txt" 2> "$dir/stderr.txt"
    printf '{"code": %s}\n' "$?" > "$dir/done.json"
}

scenario unchanged &
scenario reject-stop --inject 'reject@merge.approve#1=the change misses a refusal' &
scenario reject-nearest --on-divergence nearest --inject 'reject@merge.approve#1=the change misses a refusal' &
scenario timeout --on-divergence nearest --inject 'timeout@IDEA-1.billing.invoices/change.implement#1' &
scenario live-dry --on-divergence live --live-adapter dry --live-cap 1 --inject 'reject@merge.approve#1' &
scenario check-fail --on-divergence nearest --inject 'check-fail@tests.run#1' --set max_runs.implement=2 &
wait

echo "RUNBOOK_REPLAY_RUNS=$work" >> "$NEXTEST_ENV"
