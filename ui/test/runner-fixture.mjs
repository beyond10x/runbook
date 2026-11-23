// A run directory of `runbook run` that has not ended, with the runner's own records (docs/loom-run-directory.md):
//   runner.json    {"format": "runbook.runner/1", pid, start_ticks, started_at, closed_at}
//   events.jsonl   run_started, then action_started (source, role, case, action, try) and step_made lines, whole seconds
// written over the hand-made run of loom-run-fixture.mjs with invented names, minus its end (status.json, outcome.json,
// the terminated event). The case store has what Loom wrote to it: for a run that is between two steps, the event
// step_budget_spent that ends every `tree run --max-steps 1`.
import fs from 'node:fs';
import path from 'node:path';
import {writeLoomRun} from './loom-run-fixture.mjs';

const jl = (rows) => rows.map((r) => JSON.stringify(r)).join('\n') + '\n';

// Field 22 of /proc/<pid>/stat: the start time of a process in clock ticks since boot (the name in field 2 may hold
// spaces and parentheses, so the fields count from the last closing parenthesis).
export function startTicks(pid) {
  const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8');
  return Number(stat.slice(stat.lastIndexOf(')') + 2).split(' ')[19]);
}

// options: runner ({pid, start_ticks, closed_at}; null writes no record), state ('in-step': an action started and no step
// finished since; 'between': the last step finished and Loom ended it on its step budget), startedAgo (seconds), action,
// caseId, tryNumber, source, inStore (false: the case store has no start of the action, as under a child that writes its run log when a step ends)
export function writeLiveRun(dir, {runner, state = 'in-step', startedAgo = 125, action = 'notes.write', caseId = 'CASE-1', tryNumber = 1, source = 'program_start', inStore = true, now = Date.now()} = {}) {
  writeLoomRun(dir);
  fs.rmSync(path.join(dir, 'status.json'));
  fs.rmSync(path.join(dir, 'outcome.json'));
  const sec = Math.floor(now / 1000);
  const at = sec - startedAgo;
  const store = path.join(dir, 'store', caseId);
  const opened = {event: 'opened', at: new Date((at - 30) * 1000).toISOString(), protocol: {id: 'example', revision: 1}, seq: 0, source: 'loom-runlog'};
  const started = {event: 'action_started', at: new Date(at * 1000).toISOString(), action, llm: false, retry_of: [], seq: 1, source: 'loom-runlog'};
  const rows = inStore ? [opened, started] : [opened];
  if (state === 'between' && inStore) {
    rows.push({event: 'action_finished', at: new Date((at + 5) * 1000).toISOString(), action, exit: '0', evidence: [], seq: 2, source: 'loom-runlog'});
    rows.push({event: 'step_budget_spent', at: new Date((at + 5) * 1000).toISOString(), next: [], seq: 3, source: 'loom-runlog'});
  }
  fs.writeFileSync(path.join(store, 'events.jsonl'), jl(rows));
  fs.writeFileSync(path.join(store, 'observed.jsonl'), jl(rows.map((e, i) => ({index: i, event: e.event, observed_at: e.at, format: 'sdc.observed/1'}))));
  const events = [{at: at - 30, event: 'run_started', run: 'r1', settings: {}}, {at, event: 'action_started', source, role: 'program', case: caseId, action, try: tryNumber}];
  if (state === 'between') events.push({at: at + 5, event: 'step_made', index: 1, exit: 0, stop: 'BudgetSpent', steps: 1});
  fs.writeFileSync(path.join(dir, 'events.jsonl'), jl(events));
  if (runner) fs.writeFileSync(path.join(dir, 'runner.json'), JSON.stringify({format: 'runbook.runner/1', pid: runner.pid, start_ticks: runner.start_ticks, started_at: at - 30, closed_at: runner.closed_at ?? null}, null, 2) + '\n');
  return dir;
}
