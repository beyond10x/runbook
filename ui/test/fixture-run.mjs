#!/usr/bin/env node
// A run that is happening now, made from a recorded one, for watching and testing the live view.
//
//   node test/fixture-run.mjs <scenario> <target-run-dir> [speed]
//
// What it writes is canned: its run-settings.jsonl and llm-calls.jsonl lines name fixture-run.mjs as
// the adapter, so the server lists the run as a stub run (server.mjs programOf).
//
// Copies the recorded store (default ../runs/trial-todo-1, else test/recorded/trial-todo-1-slim for the
// scenarios person, stopped, human, decider and autopilot; SDC_UI_FIXTURE_SOURCE to change) into
// <target-run-dir>/store, cuts each case's events.jsonl back to the scenario's point, then plays
// the rest as the driver and the adapters would write it: an `action_started` line, an LLM or
// decider record directory whose response.json is empty while its metaharness event stream grows,
// file changes in fs/changes.jsonl (docs/fs-changes.md), then response.json, `action_finished` and
// the next steps. Never writes to the source. Scenarios:
//   agent    one LLM agent call in flight (change.implement), with file changes, then the next steps
//   two      two cases in flight: an LLM agent (spec.author) and a program (plan.cover, then more);
//            the author edits the storage domain, the plan steps create and activate a story
//   changes  every class of change written at once (questions, roadmap, spec, story, ADR, code, an
//            unparsable story, a decider decision); the steps of `two` stay in flight
//   decider  a case awaiting a person while the decider (an LLM) decides in its place
//   person   a case awaiting a person, nothing running
//   stopped  the person stop of `person`, then stopped on purpose: run-stopped.json (sdc.run-stopped/1,
//            by the operator 11 minutes ago, 19 minutes into the wait) and run-settings.jsonl giving
//            the decisions to a decider (haiku), so the wait is a failure of the run (should be 0)
//   autopilot  `decider` in autopilot: run-settings.jsonl with autonomy A5 and stand_in true, and no
//            person principal anywhere (every recorded principal and answered_by is decider:haiku)
//   human    the person stop of `person` as a legacy run: every principal and answered_by is "human",
//            without an id
// Static scenarios (nothing is played; two cases have an action started and never finished):
//   gone         driver.json names a pid that is not running
//   gone-nofile  no driver.json, while observed.jsonl exists
//   legacy       neither file: a run written before the driver's observation files
// The played scenarios (agent, two, decider) write what the driver and the adapters now write
// beside the old files: `observed.jsonl` next to each events.jsonl (sdc.observed/1), `driver.json`
// in the tree root while the script runs (sdc.driver/1), `case` in the call's request.json and a
// `status.json` (sdc.call-status/1). `person` is written without them, as an older driver did.
// speed: 1 plays gaps between events as recorded, capped at 2.5 s (default 1).
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {unifiedDiff} from '../udiff.mjs';
import {missingRun, fixtureSource} from './need-run.mjs';
import {installClock, readGate, RealDate} from '../vclock.mjs';

// The ux loop's measured run (SDC_CLOCK_DIR, vclock.mjs): the time is the shared clock, each sleep is
// one step the loop releases, and every file written in a step carries the clock's time as mtime.
const GATE = process.env.SDC_CLOCK_DIR ? path.resolve(process.env.SDC_CLOCK_DIR) : null;
if (GATE) installClock(GATE);
const utimes = fs.utimesSync;
const stamped = new Map();

const here = path.dirname(fileURLToPath(import.meta.url));
const [scenario, targetArg, speedArg] = process.argv.slice(2);
// the recorded run when it is there (runs/ is git-ignored); else, for the scenarios that need only the
// cases intake and direction, its committed slim copy (need-run.mjs fixtureSource)
const SRC = fixtureSource(scenario);
const TWO = {cut: {'IDEA-1': 22, 'IDEA-1.todolist': 24, 'IDEA-1.todolist.spec': 7, 'IDEA-1.todolist.plan': 11, 'IDEA-1.todolist.storage': 'drop', 'IDEA-1.todolist.release': 'drop'}, active: ['IDEA-1.todolist.spec', 'IDEA-1.todolist.plan']};
const SCEN = {
  agent: {cut: {'IDEA-1': 22, 'IDEA-1.todolist': 30, 'IDEA-1.todolist.storage': 7, 'IDEA-1.todolist.release': 'drop'}, active: ['IDEA-1.todolist.storage'], signals: true},
  two: {...TWO, signals: true},
  changes: {...TWO},
  decider: {cut: {'IDEA-1': 11, 'IDEA-1.direction': 109, 'IDEA-1.todolist': 'drop'}, active: ['IDEA-1.direction'], signals: true},
  person: {cut: {'IDEA-1': 11, 'IDEA-1.direction': 20, 'IDEA-1.todolist': 'drop'}, active: ['IDEA-1.direction']},
  autopilot: {cut: {'IDEA-1': 11, 'IDEA-1.direction': 109, 'IDEA-1.todolist': 'drop'}, active: ['IDEA-1.direction'], signals: true, principal: 'decider:haiku'},
  human: {cut: {'IDEA-1': 11, 'IDEA-1.direction': 20, 'IDEA-1.todolist': 'drop'}, active: ['IDEA-1.direction'], principal: 'human'},
  stopped: {cut: {'IDEA-1': 11, 'IDEA-1.direction': 20, 'IDEA-1.todolist': 'drop'}, active: ['IDEA-1.direction']},
  gone: {...TWO, still: 'dead-driver'},
  'gone-nofile': {...TWO, still: 'observed-only'},
  legacy: {...TWO, still: 'none'},
};
if (!SCEN[scenario] || !targetArg) {
  console.error(`usage: fixture-run.mjs <${Object.keys(SCEN).join('|')}> <target-run-dir> [speed]`);
  process.exit(2);
}
const target = path.resolve(targetArg);
if (missingRun(SRC)) { console.error(`fixture-run.mjs: ${missingRun(SRC)}`); process.exit(2); }
if (target.startsWith(path.resolve(SRC))) { console.error('refused: the target is inside the source run'); process.exit(2); }
const speed = Number(speedArg || 1);
const realSleep = (ms) => new Promise((r) => setTimeout(r, ms));
// play time in the measured run: every sleep is a timer on a play clock of its own; the timers fire
// one at a time in order of play time (then of creation), each after the work the last one started
// has run to its next sleep, and only up to the budget the loop releases (go)
let playMs = 0, timerSeq = 0, pumping = false;
const timers = [];
// path inside the run directory, the same before and after the move from .part into place
const inRun = (p) => (p.startsWith(part) ? p.slice(part.length) : p.startsWith(target) ? p.slice(target.length) : p);
if (GATE) fs.utimesSync = (p, a, m) => { utimes(p, a, m); stamped.set(inRun(path.resolve(p)), fs.statSync(p).mtimeMs); };
function settle() {
  if (!GATE || !fs.existsSync(target)) return;
  const v = new RealDate(Date.now());
  const walk = (d) => {
    for (const n of fs.readdirSync(d)) {
      const p = path.join(d, n);
      const st = fs.lstatSync(p);
      if (st.isDirectory()) walk(p);
      else if (st.isFile() && stamped.get(inRun(p)) !== st.mtimeMs) { utimes(p, v, v); stamped.set(inRun(p), fs.statSync(p).mtimeMs); }
    }
  };
  walk(target);
}
async function pump() {
  if (pumping) return;
  pumping = true;
  for (;;) {
    // the work the last timer started runs on until it sleeps again (fs is synchronous here)
    await new Promise((r) => setImmediate(r));
    if (!timers.length) break;
    timers.sort((a, b) => a.due - b.due || a.n - b.n);
    const t = timers[0];
    // past the budget: what the step wrote is stamped and reported, then the player waits
    if (Number(readGate(GATE, 'go')) < t.due) {
      settle();
      fs.writeFileSync(path.join(GATE, 'at'), String(t.due));
      while (Number(readGate(GATE, 'go')) < t.due) await realSleep(10);
    }
    timers.shift();
    playMs = t.due;
    t.fire();
  }
  pumping = false;
}
const after = (ms, fire) => { timers.push({due: playMs + ms, n: timerSeq++, fire}); pump(); };
const sleep = GATE ? (ms) => new Promise((r) => after(ms, r)) : realSleep;
const sha = (t) => crypto.createHash('sha256').update(t).digest('hex');
const lines = (f) => fs.readFileSync(f, 'utf8').split('\n').filter((l) => l.trim());

// ---- the store, cut back -----------------------------------------------------------------------
// built aside, then moved into place, so a watching server never sees the uncut copy
const part = target + '.part';
fs.rmSync(target, {recursive: true, force: true});
fs.rmSync(part, {recursive: true, force: true});
fs.mkdirSync(part, {recursive: true});
fs.cpSync(path.join(SRC, 'store'), path.join(part, 'store'), {recursive: true, verbatimSymlinks: true});
// the slim copy names its own directory @SRC@: the copy names the target
const unslim = (d) => { for (const n of fs.readdirSync(d)) { const p = path.join(d, n); const st = fs.lstatSync(p); if (st.isDirectory()) unslim(p); else if (st.isFile() && /\.(json|jsonl|ya?ml|md)$/.test(n)) { const t = fs.readFileSync(p, 'utf8'); if (t.includes('@SRC@')) fs.writeFileSync(p, t.split('@SRC@').join(target)); } } };
unslim(path.join(part, 'store'));
const caseDirs = new Map();
const visit = (d) => {
  caseDirs.set(JSON.parse(fs.readFileSync(path.join(d, 'case.json'), 'utf8')).id, d);
  const kids = path.join(d, 'children');
  if (fs.existsSync(kids)) for (const n of fs.readdirSync(kids)) if (fs.existsSync(path.join(kids, n, 'case.json'))) visit(path.join(kids, n));
};
for (const n of fs.readdirSync(path.join(part, 'store'))) if (fs.existsSync(path.join(part, 'store', n, 'case.json'))) visit(path.join(part, 'store', n));
const sc = SCEN[scenario];
const rest = {};
const stash = new Map();
for (const [id, cut] of Object.entries(sc.cut)) {
  const d = caseDirs.get(id);
  if (!d) continue;
  if (cut === 'drop') { fs.rmSync(d, {recursive: true, force: true}); continue; }
  const all = lines(path.join(d, 'events.jsonl'));
  rest[id] = all.filter((l) => JSON.parse(l).seq > cut);
  const keep = all.filter((l) => JSON.parse(l).seq <= cut);
  fs.writeFileSync(path.join(d, 'events.jsonl'), keep.join('\n') + '\n');
  const later = new Set(rest[id].flatMap((l) => { const e = JSON.parse(l); return Array.isArray(e.evidence) ? e.evidence : []; }));
  // records of later steps wait aside until the step that wrote them is played
  for (const f of fs.readdirSync(path.join(d, 'evidence'))) { const m = /^e(\d+)-/.exec(f); if (m && Number(m[1]) > cut && later.has(f.replace(/\.json$/, ''))) { stash.set(`${id}/${f.replace(/\.json$/, '')}`, fs.readFileSync(path.join(d, 'evidence', f))); fs.rmSync(path.join(d, 'evidence', f)); } }
}
// every recorded principal and answer as one kind (autopilot: the decider; human: the legacy record)
if (sc.principal) {
  const swap = (p) => (/^(decider|rule):|^(non-interactive|derived|dry-run)$/.test(p) ? p : sc.principal);
  for (const d of caseDirs.values()) {
    const ev = path.join(d, 'events.jsonl');
    if (fs.existsSync(ev)) fs.writeFileSync(ev, lines(ev).map((l) => { const e = JSON.parse(l); if (e.principal) e.principal = swap(e.principal); return JSON.stringify(e); }).join('\n') + '\n');
  }
  const q = path.join(part, 'store/IDEA-1/work/questions.yaml');
  if (fs.existsSync(q)) fs.writeFileSync(q, fs.readFileSync(q, 'utf8').replace(/answered_by: .*/g, `answered_by: ${sc.principal}`));
}
if (scenario === 'autopilot') {
  const sec = (t) => new Date(t).toISOString().replace(/\.\d+Z$/, 'Z');
  fs.writeFileSync(path.join(part, 'run-settings.jsonl'), JSON.stringify({format: 'sdc.run-settings/1', at: sec(Date.now() - 600000), leg: 'started', autonomy: 'A5', stand_in: true, decider: {kind: 'haiku', model: 'claude-haiku-5-5', escalate: 'final', max_decider_calls: 20}, adapter: 'fixture-run.mjs', max_llm_calls: 45, max_wall_seconds: 5400, llm_calls_before: 0}) + '\n');
}
// cases that did not move during the scenario get a past mtime, so nothing reads as just written
const past = new Date(Date.now() - 9 * 60000);
for (const [id, d] of caseDirs) if (fs.existsSync(d) && !sc.active.includes(id)) fs.utimesSync(path.join(d, 'events.jsonl'), past, past);
if (scenario === 'person') { const t = new Date(Date.now() - 252000); fs.utimesSync(path.join(caseDirs.get('IDEA-1.direction'), 'events.jsonl'), t, t); }
if (scenario === 'stopped') {
  const waitAt = Date.now() - 30 * 60000, stopAt = Date.now() - 11 * 60000;
  fs.utimesSync(path.join(caseDirs.get('IDEA-1.direction'), 'events.jsonl'), new Date(waitAt), new Date(waitAt));
  const sec = (t) => new Date(t).toISOString().replace(/\.\d+Z$/, 'Z');
  fs.writeFileSync(path.join(part, 'run-settings.jsonl'), JSON.stringify({format: 'sdc.run-settings/1', at: sec(waitAt - 20 * 60000), leg: 'started', decider: {kind: 'haiku', model: 'claude-haiku-5-5', escalate: 'stop', max_decider_calls: 20}, adapter: 'fixture-run.mjs', max_llm_calls: 45, max_wall_seconds: 5400, llm_calls_before: 0}) + '\n');
  fs.writeFileSync(path.join(part, 'run-stopped.json'), JSON.stringify({format: 'sdc.run-stopped/1', at: sec(stopAt), by: 'operator', reason: 'stopped to look at the context before the decider takes it', verdict_at_stop: 'awaiting_person', case: 'IDEA-1.direction'}, null, 2) + '\n');
}

// ---- what the driver now writes: observed.jsonl, driver.json -------------------------------------------
const iso = (t) => new Date(t).toISOString();
const observe = (dir, index, e, t = Date.now()) => fs.appendFileSync(path.join(dir, 'observed.jsonl'),
  JSON.stringify({format: 'sdc.observed/1', index, event: e.event, action: e.action ?? null, observed_at: iso(t), pid: process.pid}) + '\n');
const deadPid = () => spawnSync(process.execPath, ['-e', '']).pid;
const rootCase = [...caseDirs.entries()].find(([, d]) => path.basename(path.dirname(d)) === 'store')?.[1];
const driverJson = (pid) => JSON.stringify({format: 'sdc.driver/1', pid, started_at: iso(Date.now() - 20000), argv: ['sdc-drive', '--canon', 'canon', '--case-dir', rootCase ? rootCase.replace(part, target) : '', 'run', '--at', '2026-11-21T10:00:00Z']}) + '\n';
if (sc.still) {
  // the events of the active cases, dated by the driver about three minutes ago
  if (sc.still !== 'none') for (const id of sc.active) {
    const evs = lines(path.join(caseDirs.get(id), 'events.jsonl')).map((l) => JSON.parse(l));
    evs.forEach((e, i) => observe(caseDirs.get(id), i, e, Date.now() - 190000 + i * 40));
  }
  if (sc.still === 'dead-driver') fs.writeFileSync(path.join(rootCase, 'driver.json'), driverJson(deadPid()));
}
if (sc.signals) for (const id of sc.active) {
  // the events already on disk when the playing starts were observed a few seconds ago
  lines(path.join(caseDirs.get(id), 'events.jsonl')).forEach((l, i) => observe(caseDirs.get(id), i, JSON.parse(l), Date.now() - 8000 + i * 40));
}
fs.renameSync(part, target);
for (const [id, d] of caseDirs) caseDirs.set(id, d.replace(part, target));
const signals = !!sc.signals;
const append = (id, line) => {
  const dir = caseDirs.get(id);
  const index = fs.existsSync(path.join(dir, 'events.jsonl')) ? lines(path.join(dir, 'events.jsonl')).length : 0;
  fs.appendFileSync(path.join(dir, 'events.jsonl'), line + '\n');
  if (signals) observe(dir, index, JSON.parse(line));
};
if (signals) {
  const root = [...caseDirs.values()].find((d) => path.basename(path.dirname(d)) === 'store');
  fs.writeFileSync(path.join(root, 'driver.json'), driverJson(process.pid));
  process.on('exit', () => fs.rmSync(path.join(root, 'driver.json'), {force: true}));
}
// the last step: what the exit leaves, then done
if (GATE) process.on('exit', () => { settle(); fs.writeFileSync(path.join(GATE, 'at'), 'done'); });
const calls = fs.existsSync(path.join(SRC, 'llm-calls.jsonl')) ? lines(path.join(SRC, 'llm-calls.jsonl')).map((l) => JSON.parse(l)) : [];
const rel = (p) => p.split(SRC).join(target).split('@SRC@').join(target);

// ---- file changes ---------------------------------------------------------------------------------
let fsSeq = 0;
const content = new Map();
function change(root, p, after, attribution, op) {
  fs.mkdirSync(path.join(target, 'fs', 'blobs'), {recursive: true});
  const key = `${root}/${p}`;
  const before = content.get(key) ?? null;
  const store = (t) => { if (t == null) return null; const h = sha(t); fs.writeFileSync(path.join(target, 'fs', 'blobs', h), t); return h; };
  const diff = unifiedDiff(before, after, p, p);
  let add = 0, del = 0;
  for (const l of diff.split('\n')) { if (/^\+(?!\+\+)/.test(l)) add++; else if (/^-(?!--)/.test(l)) del++; }
  content.set(key, after);
  const r = {format: 'sdc.fs-change/1', seq: ++fsSeq, at: new Date().toISOString(), root, path: p, op: op || (before == null ? 'create' : after == null ? 'delete' : 'modify'), from_path: null,
    before: store(before), after: store(after), binary: false, too_large: false, lines_added: add, lines_removed: del, diff, attribution};
  fs.appendFileSync(path.join(target, 'fs', 'changes.jsonl'), JSON.stringify(r) + '\n');
}

// ---- one agent or decider call, played from its recorded directory --------------------------------
async function playRecord(kind, name, {caseId, action, onTool} = {}) {
  const from = path.join(SRC, kind, name), to = path.join(target, kind, name);
  fs.mkdirSync(path.join(to, 'cwd', 'out'), {recursive: true});
  if (fs.existsSync(path.join(from, 'decide-request.json'))) fs.writeFileSync(path.join(to, 'decide-request.json'), rel(fs.readFileSync(path.join(from, 'decide-request.json'), 'utf8')));
  await sleep(300 / speed);
  for (const f of ['prompt.md', 'request.json', 'frame.json', 'argv.json']) if (fs.existsSync(path.join(from, f))) fs.writeFileSync(path.join(to, f), rel(fs.readFileSync(path.join(from, f), 'utf8')));
  const status = (state, extra = {}) => signals && fs.writeFileSync(path.join(to, 'status.json'), JSON.stringify({format: 'sdc.call-status/1', state, pid: process.pid, started_at: startedAt, finished_at: null, exit: null, ...extra}) + '\n');
  const startedAt = iso(Date.now());
  if (signals && caseId) {
    const req = JSON.parse(fs.readFileSync(path.join(to, 'request.json'), 'utf8'));
    fs.writeFileSync(path.join(to, 'request.json'), JSON.stringify({...req, case: caseId}));
  }
  fs.writeFileSync(path.join(to, 'response.json'), '');
  status('running');
  fs.writeFileSync(path.join(to, 'events.jsonl'), '');
  let prevAt = null;
  for (const l of lines(path.join(from, 'events.jsonl'))) {
    const e = JSON.parse(l);
    if (e.at) {
      const t = Date.parse(e.at);
      if (prevAt != null) await sleep(Math.max(40, Math.min(2500, t - prevAt)) / speed);
      prevAt = t;
    } else await sleep(25 / speed);
    fs.appendFileSync(path.join(to, 'events.jsonl'), rel(l) + '\n');
    if (e.event === 'tool.requested' && onTool) await onTool(e, `${kind}/${name}`);
  }
  await sleep(600 / speed);
  fs.writeFileSync(path.join(to, 'response.json'), rel(fs.readFileSync(path.join(from, 'response.json'), 'utf8')));
  status('finished', {finished_at: iso(Date.now()), exit: 0});
  const rec =calls.find((c) => c.record?.endsWith(`/${kind}/${name}`));
  if (rec) fs.appendFileSync(path.join(target, 'llm-calls.jsonl'), rel(JSON.stringify({...rec, adapter: 'fixture-run.mjs'})) + '\n');
}
const writes = (caseId, action) => async (e, record) => {
  if (!/^(Write|Edit)$/.test(e.name) || !e.input?.file_path) return;
  const p = e.input.file_path.replace(/^.*\/cwd\//, '');
  await sleep(120 / speed);
  const tool = {seq: e.seq, name: e.name, input: p};
  if (/Cargo\.lock$/.test(p)) {
    // the stream lagged behind this write: recorded against the call, its tool supplied later
    change(`${record}/cwd`, p, e.input.content ?? '', {case: caseId, action, record, tool: null, basis: 'record'});
    const seq = fsSeq;
    const lag = () => fs.appendFileSync(path.join(target, 'fs', 'changes.jsonl'), JSON.stringify({format: 'sdc.fs-attribution/1', change: seq, at: new Date().toISOString(),
      attribution: {case: caseId, action, record, tool, basis: 'tool-window'}}) + '\n');
    if (GATE) after(1500 / speed, lag); else setTimeout(lag, 1500 / speed);
    return;
  }
  change(`${record}/cwd`, p, e.input.content ?? '', {case: caseId, action, record, tool, basis: 'tool-window'});
  if (/spec\/domains\/storage\.yaml$/.test(p)) {
    // the author's second pass over the storage domain: a field, a state and an outcome
    await sleep(900 / speed);
    change(`${record}/cwd`, p, MEANING.specEdit(content.get(`${record}/cwd/${p}`)), {case: caseId, action, record, tool: {seq: e.seq + 2, name: 'Edit', input: p}, basis: 'tool-window'});
  }
  if (/src\/lib\.rs$/.test(p)) {
    // the agent's second pass over lib.rs, an Edit the recorded call did not make
    await sleep(1400 / speed);
    const before = content.get(`${record}/cwd/${p}`);
    const after = before.replace(/^(\s*)pub fn /m, '$1/// Validated against the storage domain before any write.\n$1#[must_use]\n$1pub fn ').replace('Vec::new()', 'Vec::with_capacity(16)');
    change(`${record}/cwd`, p, after, {case: caseId, action, record, tool: {seq: e.seq + 2, name: 'Edit', input: p}, basis: 'tool-window'});
  }
};
async function replay(id, gap, onEvent) {
  for (const l of rest[id] || []) {
    const e = JSON.parse(l);
    for (const ev of e.evidence && Array.isArray(e.evidence) ? e.evidence : []) { const b = stash.get(`${id}/${ev}`); if (b) fs.writeFileSync(path.join(caseDirs.get(id), 'evidence', ev + '.json'), b); }
    append(id, l);
    if (onEvent) await onEvent(e);
    if (e.event === 'awaiting_authority' || e.event === 'awaiting_person' || e.event === 'terminated') break;
    await sleep((e.event === 'action_started' ? gap : 200) / speed);
  }
}

// ---- changes that are not code: planning, specification, direction, decisions --------------------
// Built from the recorded run's final files, so each before/after pair is a real edit of them.
const W = 'store/IDEA-1/work';
const srcWork = (p) => fs.readFileSync(path.join(SRC, 'store/IDEA-1/work', p), 'utf8');
const exact = (caseId, action) => ({case: caseId, action, record: null, tool: null, basis: 'action-window'});
const MEANING = {
  specEdit: (t) => t.replace(/( {4}fields:\n {6}- name: title\n {8}type: String\n)/, '$1      - name: due\n        type: Date\n')
    .replace('      states: [Stored]\n      terminal: [Stored]', '      states: [Stored, Done]\n      terminal: [Done]\n      transitions:\n        - from: Stored\n          to: Done\n          on: todo.storage.CompleteItem')
    .replace(/( {6}# Decided by the data file)/, '      - name: duplicate\n        error: todo.storage.StorageUnavailable\n        summary: An item with the same title is already stored.\n\n$1'),
  storyDraft: () => srcWork('aep/story/storage.md').replace('status: active', 'status: draft').replace('- serves: vision:product\n', '').replace(/scope:\n- confidence: cited\n {2}path: impl\n/, '').replace(/transitions:\n.*\n/, '').replace('revision: 3', 'revision: 1'),
  storyActive: () => srcWork('aep/story/storage.md'),
  roadmap: () => srcWork('aep/initiative/roadmap.md'),
  roadmap2: () => srcWork('aep/initiative/roadmap.md').replace("summary: 'epics: todolist'", "summary: 'epics: todolist, export'").replace('## Sequence', '### export: Export the list\n\n- **Delivers:** a JSON export of every item.\n\n## Sequence'),
  questionsOpen: () => srcWork('questions.yaml').replace("    status: changed\n    answer: Kept on the server, in the daemon's data file\n    answered_by: human", '    status: open'),
  questions: () => srcWork('questions.yaml'),
  adr: () => '---\ntitle: Keep the items in the daemon\'s data file\nstatus: accepted\n---\n# 0001 Keep the items in the daemon\'s data file\n\n## Context\n\nItems must outlive a browser session (r3) and the daemon runs as several instances.\n\n## Decision\n\nEach instance keeps its items in its own data file, written whole on every change.\n',
};
const planningSteps = (caseId) => async (e) => {
  if (e.event !== 'action_started') return;
  if (e.action === 'plan.scope' && !content.has(`${W}/aep/story/storage.md`)) { await sleep(600 / speed); change(W, 'aep/story/storage.md', MEANING.storyDraft(), exact(caseId, e.action)); }
  else if (e.action === 'plan.status' && content.get(`${W}/aep/story/storage.md`) === MEANING.storyDraft()) { await sleep(600 / speed); change(W, 'aep/story/storage.md', MEANING.storyActive(), exact(caseId, e.action)); }
};

// ---- scenarios ------------------------------------------------------------------------------------
console.log(`fixture ${scenario}: ${target}`);
if (scenario === 'agent') {
  const id = 'IDEA-1.todolist.storage';
  await playRecord('llm', '015-change.implement', {caseId: id, onTool: writes(id, 'change.implement')});
  const lib = content.get('llm/015-change.implement/cwd/out/impl/src/lib.rs');
  if (lib) change('store/IDEA-1/work', 'impl/src/lib.rs', lib, {case: id, action: 'change.implement', record: null, tool: null, basis: 'action-window'});
  change(W, 'docs/adr/0001-data-file-storage.md', MEANING.adr(), exact(id, 'change.implement'));
  await sleep(500 / speed);
  await replay(id, 2600);
} else if (scenario === 'two') {
  await Promise.all([
    playRecord('llm', '013-spec.author', {caseId: 'IDEA-1.todolist.spec', onTool: writes('IDEA-1.todolist.spec', 'spec.author')}).then(() => replay('IDEA-1.todolist.spec', 2200)),
    sleep(7000 / speed).then(() => replay('IDEA-1.todolist.plan', 5200, planningSteps('IDEA-1.todolist.plan'))),
  ]);
} else if (scenario === 'changes') {
  // every class of change at once, nothing played: the steps of `two` stay in flight
  change(W, 'questions.yaml', MEANING.questionsOpen(), exact('IDEA-1.intake', 'questions.write'));
  change(W, 'questions.yaml', MEANING.questions(), exact('IDEA-1.intake', 'questions.answer'));
  change(W, 'aep/initiative/roadmap.md', MEANING.roadmap(), exact('IDEA-1.direction', 'roadmap.write'));
  change(W, 'aep/initiative/roadmap.md', MEANING.roadmap2(), exact('IDEA-1.direction', 'roadmap.write'));
  const storage = srcWork('spec/domains/storage.yaml');
  change('llm/013-spec.author/cwd', 'out/spec/domains/storage.yaml', storage, {case: 'IDEA-1.todolist.spec', action: 'spec.author', record: 'llm/013-spec.author', tool: {seq: 82, name: 'Write', input: 'out/spec/domains/storage.yaml'}, basis: 'tool-window'});
  change('llm/013-spec.author/cwd', 'out/spec/domains/storage.yaml', MEANING.specEdit(storage), {case: 'IDEA-1.todolist.spec', action: 'spec.author', record: 'llm/013-spec.author', tool: {seq: 84, name: 'Edit', input: 'out/spec/domains/storage.yaml'}, basis: 'tool-window'});
  change(W, 'aep/story/storage.md', MEANING.storyDraft(), exact('IDEA-1.todolist.plan', 'plan.scope'));
  change(W, 'aep/story/storage.md', MEANING.storyActive(), exact('IDEA-1.todolist.plan', 'plan.status'));
  change(W, 'docs/adr/0001-data-file-storage.md', MEANING.adr(), exact('IDEA-1.todolist.plan', 'plan.status'));
  change(W, 'impl/src/main.rs', 'fn main() {\n    todo::serve();\n}\n', exact('IDEA-1.todolist.plan', 'plan.status'));
  change(W, 'aep/story/broken.md', '---\nkind: story\nrelations: [unclosed\n---\n', exact('IDEA-1.todolist.plan', 'plan.status'));
  const d = fs.existsSync(path.join(SRC, 'decisions.jsonl')) ? lines(path.join(SRC, 'decisions.jsonl')).find((l) => l.includes('001-roadmap.approve')) : null;
  if (d) fs.appendFileSync(path.join(target, 'decisions.jsonl'), rel(d) + '\n');
} else if (scenario === 'decider' || scenario === 'autopilot') {
  change(W, 'aep/initiative/roadmap.md', MEANING.roadmap(), exact('IDEA-1.direction', 'roadmap.write'));
  change(W, 'aep/initiative/roadmap.md', MEANING.roadmap2(), exact('IDEA-1.direction', 'roadmap.write'));
  await playRecord('decider', '001-roadmap.approve', {caseId: 'IDEA-1.direction'});
  const d = fs.existsSync(path.join(SRC, 'decisions.jsonl')) ? lines(path.join(SRC, 'decisions.jsonl')).find((l) => l.includes('001-roadmap.approve')) : null;
  if (d) fs.appendFileSync(path.join(target, 'decisions.jsonl'), rel(d) + '\n');
}
console.log('fixture done');
