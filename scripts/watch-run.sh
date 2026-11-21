#!/usr/bin/env bash
# Starts or stops sdc-watch (sdc/watch/) for one run directory: the record of every effective file
# change, attributed to the action and tool call that caused it (docs/fs-changes.md).
#
#   scripts/watch-run.sh start <run-dir>   start in the background; returns once it watches
#   scripts/watch-run.sh stop <run-dir>    SIGTERM by PID, wait until it has flushed and exited
#   scripts/watch-run.sh status <run-dir>  exit 0 when it runs
#
# Writes <run-dir>/fs/ (changes.jsonl, blobs/, snapshot.json, sdc-watch.pid, watch.log).
# SDC_WATCH: the binary (default sdc/watch/target/release/sdc-watch, built when missing).
set -euo pipefail
sdc=$(cd "$(dirname "$0")/.." && pwd)
cmd=${1:-}; run=${2:-}
[[ -n $cmd && -n $run && -d $run ]] || { echo "usage: $0 start|stop|status <run-dir>" >&2; exit 64; }
fs=$run/fs
pidf=$fs/sdc-watch.pid
bin=${SDC_WATCH:-$sdc/watch/target/release/sdc-watch}

running() { [[ -f $pidf ]] && kill -0 "$(cat "$pidf")" 2>/dev/null; }

case $cmd in
start)
  if running; then echo "sdc-watch already runs for $run (pid $(cat "$pidf"))"; exit 0; fi
  if [[ ! -x $bin ]]; then
    cargo build --release --quiet --manifest-path "$sdc/watch/Cargo.toml" >&2
  fi
  mkdir -p "$fs"
  rm -f "$pidf"
  nohup "$bin" run "$run" >> "$fs/watch.log" 2>&1 < /dev/null &
  pid=$!
  # The pid file appears once the initial scan is done and every dir is watched.
  for _ in $(seq 1 600); do
    [[ -f $pidf ]] && { echo "sdc-watch watching $run (pid $pid)"; exit 0; }
    kill -0 "$pid" 2>/dev/null || { echo "sdc-watch exited; see $fs/watch.log" >&2; exit 1; }
    sleep 0.1
  done
  echo "sdc-watch not ready after 60 s; see $fs/watch.log" >&2; exit 1 ;;
stop)
  running || { echo "sdc-watch not running for $run"; exit 0; }
  pid=$(cat "$pidf")
  kill -TERM "$pid"
  for _ in $(seq 1 300); do
    kill -0 "$pid" 2>/dev/null || { echo "sdc-watch stopped ($pid)"; exit 0; }
    sleep 0.1
  done
  echo "sdc-watch $pid did not stop within 30 s" >&2; exit 1 ;;
status)
  running && { echo "running (pid $(cat "$pidf"))"; exit 0; }
  echo "not running"; exit 1 ;;
*) echo "usage: $0 start|stop|status <run-dir>" >&2; exit 64 ;;
esac
