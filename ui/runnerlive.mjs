// Is the process that runs a run still it, and what is it doing. `runbook run` writes `runner.json` (its pid, the start
// time the kernel gave the process, when it started and when it closed) and one `action_started` event for every action
// that starts (docs/loom-run-directory.md). This reads both through the checked open of runfiles.mjs and asks the kernel
// one question: has the process of that pid the start time of the record. No signal is sent, nothing is executed.
// Both files are what files of the run directory claim: a program of the run can write them under `--confinement local`.
//
// Answers {state, runner, step} or null for a run without the record (the caller keeps its own answer then):
//   running   the runner is alive and an action started after its last step finished
//   between   the runner is alive and no action is in flight: it decides, asks, or chooses the next step
//   not_live  the runner is closed, or no live process of that pid has that start time: nothing runs the run
//   unknown   the kernel did not answer for the pid (a read of /proc/<pid>/stat failed for another reason than "no such
//             process"): `why` says so
// `step` is the last action that started: {action, case, try, at, source, role}, or null.
import fs from 'node:fs';
import path from 'node:path';
import {readText, readRegular, statSig} from './runfiles.mjs';

const TAIL = 1048576;
// A record is about 150 bytes: a runner.json over this is not read.
export const RECORD_LIMIT = 4096;
// The longest text of a start line (action, case, source) that the answer carries, in characters.
export const TEXT_CUT = 256;

// What the kernel says of one pid, read from /proc/<pid>/stat: {gone: true} when no process has it, {error: <code>} when
// the read failed for another reason, else {state, ticks}: field 3 (the process state) and field 22, the start time in
// clock ticks since boot. The name of the process is field 2, in parentheses and may hold spaces and parentheses, so
// the fields count from the last closing one.
export function readStat(pid) {
  const n = Number(pid);
  if (!/^[0-9]{1,16}$/.test(String(pid)) || !Number.isSafeInteger(n) || n <= 0) return {gone: true};
  let stat;
  try { stat = fs.readFileSync(`/proc/${n}/stat`, 'utf8'); } catch (e) { return e?.code === 'ENOENT' || e?.code === 'ESRCH' ? {gone: true} : {error: /^[A-Z0-9_]{1,24}$/.test(e?.code) ? e.code : 'error'}; }
  const rest = stat.slice(stat.lastIndexOf(')') + 2).split(' ');
  const t = Number(rest[19]);
  return {state: rest[0], ticks: Number.isInteger(t) ? t : null};
}

const MARKS = new Set(['action_started', 'step_made', 'ended']);
const cache = new Map();

// the text cut to TEXT_CUT characters (code points: never inside a character), or null for what is no text
function cut(v) {
  if (typeof v !== 'string') return null;
  let out = '', n = 0;
  for (const ch of v) {
    if (n++ >= TEXT_CUT) break;
    out += ch;
  }
  return out.toWellFormed();
}

// the last action_started of the events file, and whether a step was made or the run ended after it
function lastStart(runDir) {
  const file = path.join(runDir, 'events.jsonl');
  const sig = statSig(file);
  const hit = cache.get(runDir);
  if (hit && hit.sig === sig) return hit.value;
  let value = {step: null, inFlight: false};
  const got = readRegular(file, {root: runDir, tail: true, limit: TAIL});
  if (got.state === 'ok') {
    const lines = got.bytes.toString('utf8').split('\n');
    // a window that starts inside a line: that line is not read
    if (got.from > 0) lines.shift();
    let last = null, inFlight = false;
    for (const l of lines) {
      if (!l.trim()) continue;
      let e;
      try { e = JSON.parse(l); } catch { continue; }
      if (!e || typeof e !== 'object' || !MARKS.has(e.event)) continue;
      if (e.event === 'action_started') { last = e; inFlight = true; } else inFlight = false;
    }
    const int = (v) => (Number.isSafeInteger(v) ? v : null);
    if (last) value = {inFlight, step: {action: cut(last.action), case: cut(last.case), try: int(last.try), at: int(last.at), source: cut(last.source), role: cut(last.role)}};
  }
  cache.set(runDir, {sig, value});
  return value;
}

function recordOf(runDir) {
  const text = readText(path.join(runDir, 'runner.json'), runDir, RECORD_LIMIT);
  if (text == null) return null;
  try { return JSON.parse(text); } catch { return null; }
}

// Why runner.json of a run was not read, in words of this file only (no byte of the file, no path), or null when there is
// no runner.json. The runner did write one: this says what kept the inspector from using it.
export function recordNotRead(runDir) {
  const got = readRegular(path.join(runDir, 'runner.json'), {root: runDir, limit: RECORD_LIMIT + 1});
  const head = 'runner.json was not read';
  if (got.state === 'missing') return null;
  if (got.state === 'refused') {
    if (got.why === 'not a regular file') return `${head}: it is not a regular file`;
    if (got.why === 'more than one link') return `${head}: the file has more than one name`;
    if (got.why === 'leaves the run') return `${head}: it does not lie inside the run`;
    return `${head}: its place in the run could not be checked`;
  }
  if (got.state === 'ok' && got.size > RECORD_LIMIT) return `${head}: it is larger than ${RECORD_LIMIT} bytes`;
  return `${head}: it is not a valid record of the runner`;
}

// `procStat` answers for a pid as readStat does; a test names its own.
export function runnerLiveness(runDir, procStat = readStat) {
  const rec = recordOf(runDir);
  if (!rec || typeof rec !== 'object' || rec.format !== 'runbook.runner/1' || !Number.isSafeInteger(rec.pid)) return null;
  const runner = {pid: rec.pid, started_at: Number.isSafeInteger(rec.started_at) ? rec.started_at : null, closed_at: Number.isSafeInteger(rec.closed_at) ? rec.closed_at : null};
  const {step, inFlight} = lastStart(runDir);
  if (runner.closed_at != null) return {state: 'not_live', why: 'the runner closed its record', runner, step};
  if (!(Number.isSafeInteger(rec.start_ticks) && rec.start_ticks > 0)) return {state: 'not_live', why: 'the record has no start time of the runner', runner, step};
  const st = procStat(rec.pid);
  if (st.error) return {state: 'unknown', why: `the kernel did not answer for the recorded process (${st.error})`, runner, step};
  if (st.gone) return {state: 'not_live', why: 'no process of the recorded pid and start time is running', runner, step};
  if (st.state === 'Z' || st.state === 'X') return {state: 'not_live', why: 'the process of the recorded pid has ended and is not yet collected by its parent', runner, step};
  if (st.ticks !== rec.start_ticks) return {state: 'not_live', why: 'no process of the recorded pid and start time is running', runner, step};
  return {state: inFlight ? 'running' : 'between', runner, step};
}
