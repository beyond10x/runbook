#!/usr/bin/env node
// sdc-ui: a read-only inspector for the case stores of run directories, with a decisions list.
//
//   node server.mjs [--port 7490] <store-or-case-dir>...
//
// Each argument is a case directory (holds case.json) or a store directory whose subdirectories
// are root cases. A path that does not exist yet is waited for. The server reads files, polls their
// stat signatures, and pushes `change` events over SSE. It listens on 127.0.0.1 only.
//
// It writes nothing and starts no program of the runtime: there is no act route, no `--write` and no
// principal. A decision is answered through the daemon (`runbook serve`, its page or `runbook decide`).
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {inbox, history, jsonl, decisionRecord, whoKind, scopeQuestions, waitsOnScope, amendStall} from './decisions.mjs';
import {unifiedDiff, diffCounts} from './udiff.mjs';
import {classify, meaningOf} from './web/meaning.js';
import {AsyncLocalStorage} from 'node:async_hooks';
import {setView, checkView, viewCovers, bindViewScope, newViewState} from './web/view.js';
import {installClock, readGate} from './vclock.mjs';

// the measured run of the ux loop: a shared clock, and a rescan when the fixture player reports a step
const CLOCK = process.env.SDC_CLOCK_DIR ? path.resolve(process.env.SDC_CLOCK_DIR) : null;
if (CLOCK) installClock(CLOCK);
let clockAt = null;

const here = path.dirname(fileURLToPath(import.meta.url));
const sdc = path.resolve(here, '..');
const args = process.argv.slice(2);
let port = 7490;
const host = '127.0.0.1';
const paths = [];
// --runs DIR (repeatable): every DIR/<run>/store is a source, and a run that appears later joins live
const runsDirs = [];
// --experiments DIR (repeatable): the experiment records (DIR/<id>.yaml); default experiments/ of the checkout served
const expDirs = [];
// --view FILE (repeatable): a runbook's view (web/view.js, format runbook.view/1); without it every views/*.view.json of the
// checkout is loaded. A run is shown with the first view that covers its protocols; --no-view loads none.
const viewFiles = [];
let noView = false;
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--port') port = Number(args[++i]);
  else if (args[i] === '--runs') runsDirs.push(path.resolve(args[++i]));
  else if (args[i] === '--view') viewFiles.push(path.resolve(args[++i]));
  else if (args[i] === '--no-view') noView = true;
  else if (args[i] === '--experiments') expDirs.push(path.resolve(args[++i]));
  else if (args[i].startsWith('--')) { console.error(`${args[i]}: unknown option. The inspector reads only: a decision is answered through the daemon (runbook serve, runbook decide)`); process.exit(2); }
  else paths.push(path.resolve(args[i]));
}
for (const r of runsDirs) if (!fs.existsSync(r)) { console.error(`--runs ${r}: no such directory`); process.exit(2); }
if (!expDirs.length) expDirs.push(path.join(sdc, 'experiments'));
if (noView && viewFiles.length) { console.error('--view and --no-view exclude each other'); process.exit(2); }
if (!viewFiles.length && !noView) {
  const dir = path.join(sdc, 'views');
  try { for (const n of fs.readdirSync(dir).sort()) if (n.endsWith('.view.json')) viewFiles.push(path.join(dir, n)); } catch { /* no views/ directory: no view */ }
}
// every view is checked when the server starts: a member the inspector does not know, or a wrong type, refuses the start
const views = viewFiles.map((file) => {
  let v = null;
  try { v = JSON.parse(fs.readFileSync(file, 'utf8')); } catch (e) { console.error(`--view ${file}: ${e.message}`); process.exit(2); }
  const problems = checkView(v);
  if (problems.length) { console.error(`--view ${file}: refused: ${problems.join('; ')}`); process.exit(2); }
  return {file, view: v};
});
if (!paths.length && !runsDirs.length) {
  // the default store is a recorded run; runs/ is git-ignored, so a fresh clone has none
  const d = path.join(sdc, 'runs/full-cycle-1/store');
  if (!fs.existsSync(d)) { console.error('no store given and the default one is missing: missing run directory runs/full-cycle-1/store (runs/ is git-ignored); pass a store directory: node server.mjs <run>/store'); process.exit(2); }
  paths.push(d);
}

const readJson = (p, fallback = null) => {
  try { return JSON.parse(fs.readFileSync(p, 'utf8')); } catch { return fallback; }
};
const readText = (p) => { try { return fs.readFileSync(p, 'utf8'); } catch { return null; } };
const statSig = (p) => { try { const s = fs.statSync(p); return `${s.size}:${s.mtimeMs}`; } catch { return '-'; } };
const listDir = (p) => { try { return fs.readdirSync(p).sort(); } catch { return []; } };
const isCase = (p) => fs.existsSync(path.join(p, 'case.json')) || fs.existsSync(path.join(p, 'events.jsonl'));

function readEvents(dir) {
  const text = readText(path.join(dir, 'events.jsonl')) || '';
  const out = [];
  for (const line of text.split('\n')) {
    if (!line.trim()) continue;
    try { out.push(JSON.parse(line)); } catch { /* a line mid-write */ }
  }
  return out;
}

// ---- sources ---------------------------------------------------------------------------------
const nameOf = (p) => { const r = path.relative(path.join(sdc, 'runs'), p); return r.startsWith('..') ? p.split(path.sep).slice(-2).join('/') : r; };
const sources = [];
const addSource = (p, found = false) => { const s = {index: sources.length, path: p, name: found || nameOf(p), found: !!found, sig: '', cases: new Map(), version: 0, times: new Map(), liveSig: ''}; sources.push(s); return s; };
for (const p of paths) addSource(p);
// the runs under --runs that have a store/, in name order at the start; a later one is appended
// (source indexes never move); returns the ones added. A directory named *.part is a run still being
// built aside (test/fixture-run.mjs moves it into place when done): it is not a run yet.
function discover() {
  const added = [];
  for (const r of runsDirs) for (const n of listDir(r)) {
    if (n.endsWith('.part')) continue;
    const st = path.join(r, n, 'store');
    if (!sources.some((s) => s.path === st) && fs.existsSync(st) && fs.statSync(st).isDirectory()) added.push(addSource(st, path.relative(path.join(sdc, 'runs'), path.join(r, n)).startsWith('..') ? n : path.relative(path.join(sdc, 'runs'), path.join(r, n))));
  }
  return added;
}
discover();

// A run stopped on purpose (sdc.run-stopped/1, written by the stop command into the run directory):
// who stopped it, when, why, and what it waited on at that moment. Null without the file.
function stoppedOf(runDir) {
  const j = readJson(path.join(runDir, 'run-stopped.json'));
  if (!j || j.format !== 'sdc.run-stopped/1' || isNaN(Date.parse(j.at))) return null;
  return {at: j.at, by: typeof j.by === 'string' && j.by ? j.by : null, reason: typeof j.reason === 'string' ? j.reason : '', verdict_at_stop: j.verdict_at_stop ?? null, case: j.case ?? null};
}
// The run's settings (sdc.run-settings/1, one line per start or resume of the run): the
// last line holds; its decider says who takes the decisions that wait on a person. Null without one.
function settingsOf(runDir) {
  const text = readText(path.join(runDir, 'run-settings.jsonl')) || '';
  const rows = text.split('\n').filter((l) => l.trim()).map((l) => { try { return JSON.parse(l); } catch { return null; } }).filter((r) => r && r.format === 'sdc.run-settings/1');
  const last = rows.at(-1);
  return last ? {...last, legs: rows.length, file: 'run-settings.jsonl'} : null;
}
// Whether a model did the run's work (real) or a stub, fixture or dry adapter wrote canned content
// (stub): the last run-settings.jsonl line names the adapter; a run without settings is real when one
// llm-calls.jsonl line is a model call that is not dry. Stub runs are never listed beside real runs
// without a label.
const STUB_ADAPTER = /stub|fixture|dry|fake|canned/i;
function programOf(runDir) {
  const set = settingsOf(runDir);
  if (set) return STUB_ADAPTER.test(String(set.adapter ?? '')) ? {kind: 'stub', why: `run-settings.jsonl: adapter ${set.adapter}`} : {kind: 'real', why: `run-settings.jsonl: adapter ${set.adapter ?? 'not named'}`};
  const calls = (readText(path.join(runDir, 'llm-calls.jsonl')) || '').split('\n').map((l) => { try { return JSON.parse(l); } catch { return null; } });
  const paid = calls.find((c) => c && typeof c.model === 'string' && c.model && c.dry !== true && !STUB_ADAPTER.test(String(c.adapter ?? '')));
  return paid ? {kind: 'real', why: `llm-calls.jsonl: ${paid.model} via ${paid.adapter ?? 'an adapter'}`} : {kind: 'stub', why: 'no run-settings.jsonl, and no model call in llm-calls.jsonl'};
}
// when the run last wrote: its directory, its settings, calls and log, its store
const lastWrite = (runDir, store) => Math.max(0, ...[runDir, store, ...['run-settings.jsonl', 'llm-calls.jsonl', 'run.log'].map((f) => path.join(runDir, f))].map((p) => { try { return fs.statSync(p).mtimeMs; } catch { return 0; } }));
// The view of one run: the first loaded view that covers the protocols of its cases (web/view.js viewCovers).
// A run whose protocols no loaded view covers has none: it renders with the generic parts only and the page
// says which view is missing (`missing`, `protocols`). No view is a default for another runbook's run.
const viewCache = new Map();
function viewOf(src) {
  const sig = `${src.version}:${src.cases.size}`;
  const hit = viewCache.get(src.index);
  if (hit && hit.sig === sig) return hit;
  const protocols = [...new Set([...src.cases.values()].map((e) => readJson(path.join(e.dir, 'case.json'), {}).protocol).filter(Boolean))].sort();
  const found = protocols.length ? views.find((x) => viewCovers(x.view, protocols)) : null;
  const out = {sig, protocols, view: found?.view || null, file: found ? path.relative(sdc, found.file) : null, runbook: found?.view.runbook || null, missing: protocols.length > 0 && !found};
  viewCache.set(src.index, out);
  return out;
}
// the web and server modules hold a current view per request and per tick (an AsyncLocalStorage store
// the view module reads): inView runs the work for one run under that run's view, and an await inside it
// cannot see the view another request set meanwhile
const viewScope = new AsyncLocalStorage();
bindViewScope(() => viewScope.getStore());
const useViewOf = (src) => { setView(src ? viewOf(src).view : null); };
const inView = (src, fn) => viewScope.run(newViewState(), () => { useViewOf(src); return fn(); });
const viewInfo = (src) => { const {view, sig, ...info} = viewOf(src); return info; };
const sourceRow = (s) => ({index: s.index, name: s.name, view: viewInfo(s), path: s.path, exists: fs.existsSync(s.path), version: s.version, cases: s.cases.size, runDir: runDirOf(s), found: s.found, stopped: stoppedOf(runDirOf(s)), program: programOf(runDirOf(s)), lastWrite: lastWrite(runDirOf(s), s.path), run: runSummary(s), experiments: experimentsOf(runDirOf(s))});

// ---- experiments: which experiment, variant and ordinal a run belongs to -----------------------------
// A run directory's experiment.json (sdc.experiment-run/1, written by sdc-experiment run) says it; a run
// registered without one is found in an experiment record's variants[].runs (experiments/<id>.yaml,
// sdc.experiment/1), its ordinal the place in that list. A run may be in more than one record. The
// records are read with a reader for the few keys used here (the server has no YAML dependency).
const yamlScalar = (v) => {
  v = String(v ?? '').trim();
  if (/^'.*'$/.test(v)) return v.slice(1, -1).replace(/''/g, "'");
  if (/^".*"$/.test(v)) { try { return JSON.parse(v); } catch { return v.slice(1, -1); } }
  v = v.replace(/\s+#.*$/, '');
  return v === '' || v === '~' || v === 'null' ? null : v;
};
// {id, title, purpose, variants: [{id, runs}]} from the record's top-level keys and its variants list
function experimentRecord(text) {
  const rec = {format: null, id: null, title: null, purpose: null, variants: []};
  let inVariants = false, listInd = -1, v = null, inRuns = false;
  for (const raw of text.split('\n')) {
    if (!raw.trim() || /^\s*#/.test(raw)) continue;
    const ind = raw.length - raw.trimStart().length, line = raw.trim();
    if (ind === 0 && !line.startsWith('- ')) {
      inVariants = false; inRuns = false;
      const m = /^([A-Za-z_][\w-]*):\s*(.*)$/.exec(line);
      if (m && m[1] === 'variants') { inVariants = true; listInd = -1; v = null; }
      else if (m && ['format', 'id', 'title', 'purpose'].includes(m[1])) rec[m[1]] = yamlScalar(m[2]);
      continue;
    }
    if (!inVariants) continue;
    if (listInd < 0 && line.startsWith('- ')) listInd = ind;
    let body = line, bInd = ind;
    if (ind === listInd && line.startsWith('- ')) { v = {id: null, runs: []}; rec.variants.push(v); inRuns = false; body = line.slice(2).trim(); bInd = ind + 2; }
    if (!v) continue;
    if (inRuns && body === line && line.startsWith('- ') && bInd >= listInd + 2) { const n = yamlScalar(line.slice(2)); if (n) v.runs.push(n); continue; }
    if (bInd !== listInd + 2) continue;
    inRuns = false;
    const k = /^([\w-]+):\s*(.*)$/.exec(body);
    if (!k) continue;
    if (k[1] === 'id') v.id = yamlScalar(k[2]);
    else if (k[1] === 'runs') { const val = k[2].trim(); if (val.startsWith('[')) v.runs = val.slice(1, val.lastIndexOf(']')).split(',').map(yamlScalar).filter(Boolean); else if (!val) inRuns = true; }
  }
  return rec;
}
let expCache = {sig: null, records: new Map(), byRun: new Map()};
function experimentIndex() {
  const files = expDirs.flatMap((d) => listDir(d).filter((n) => /\.ya?ml$/.test(n)).map((n) => path.join(d, n)));
  const sig = files.map((f) => `${f}:${statSig(f)}`).join('|');
  if (expCache.sig === sig) return expCache;
  const records = new Map(), byRun = new Map();
  for (const f of files) {
    const rec = experimentRecord(readText(f) || '');
    const id = rec.id || path.basename(f).replace(/\.ya?ml$/, '');
    if (rec.format && rec.format !== 'sdc.experiment/1') continue;
    if (records.has(id)) continue;
    records.set(id, {id, title: rec.title, purpose: rec.purpose, variants: rec.variants.map((x) => x.id).filter(Boolean)});
    rec.variants.forEach((x, vi) => x.runs.forEach((run, i) => {
      if (!byRun.has(run)) byRun.set(run, []);
      byRun.get(run).push({id, variant: x.id, ordinal: i + 1, vi, from: 'record'});
    }));
  }
  expCache = {sig, records, byRun};
  return expCache;
}
// [{id, title, purpose, variant, vi (the variant's place in its record, null without one), ordinal, from, ...}]
function experimentsOf(runDir) {
  const ix = experimentIndex();
  const meta = (e) => { const r = ix.records.get(e.id); const vi = r ? r.variants.indexOf(e.variant) : -1; return {...e, title: r?.title ?? null, purpose: r?.purpose ?? null, vi: vi >= 0 ? vi : null}; };
  const j = readJson(path.join(runDir, 'experiment.json'));
  if (j && j.format === 'sdc.experiment-run/1' && typeof j.experiment === 'string' && j.experiment) {
    return [meta({id: j.experiment, variant: typeof j.variant === 'string' ? j.variant : null, ordinal: Number.isInteger(j.ordinal) ? j.ordinal : null, from: 'experiment.json',
      run: j.run ?? null, commit: typeof j.tree_commit === 'string' ? j.tree_commit : null, startedAt: j.started_at ?? null, retrospective: j.retrospective === true})];
  }
  return (ix.byRun.get(path.basename(runDir)) || []).map(meta);
}

// ---- the run picker's rows (decision D-009): what each run is, read in one request ----------------
// From status.json (sdc.run-status/1: closed, outcome, delivered; forced, unstuck and reopens when it
// carries them), run-stopped.json (who, when, the cause), run-settings.jsonl (when it started, the
// models), llm-calls.jsonl and decisions.jsonl (cost and calls), the cases' first and last event times
// (the root's terminated event), and whether it runs now (a driver, or a call in progress). Cached per run on the
// signatures of those files; only the check whether it runs now is made on every request.
const MODEL = (m) => { const x = /claude-([a-z]+)/i.exec(String(m || '')); return x ? x[1][0].toUpperCase() + x[1].slice(1).toLowerCase() : m ? String(m) : null; };
const msOf = (t) => { const v = typeof t === 'string' && t !== '-' ? Date.parse(t) : NaN; return isNaN(v) ? null : v; };
const lines = (p) => (readText(p) || '').split('\n').filter((l) => l.trim()).map((l) => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);
const count = (v) => (Number.isInteger(v) && v >= 0 ? v : Array.isArray(v) ? v.length : null);
// "decider Opus · work Sonnet": the decider's model (or a person), and the models of the work classes
function settingText(set, calls) {
  const work = [...new Set(set?.models ? Object.entries(set.models).filter(([k]) => k !== 'decider').map(([, v]) => MODEL(v?.model)).filter(Boolean)
    : calls.map((c) => MODEL(c.model)).filter(Boolean))];
  const d = set?.decider || null;
  const dec = !d ? null : d.kind === 'person' ? 'person' : MODEL(set.models?.decider?.model || d.model) || d.kind || null;
  return [dec && `decider ${dec}`, work.length && `work ${work.join('/')}`].filter(Boolean).join(' · ') || null;
}
const summaries = new Map();
function runSummary(s) {
  const runDir = runDirOf(s);
  const files = ['status.json', 'run-stopped.json', 'run-settings.jsonl', 'llm-calls.jsonl', 'decisions.jsonl'].map((f) => path.join(runDir, f));
  const caseDirs = Array.from(s.cases.values());
  const sig = files.map(statSig).join('|') + '#' + caseDirs.map((e) => `${e.dir}:${e.sig}`).join('|');
  let c = summaries.get(s.path);
  if (!c || c.sig !== sig) {
    const st = readJson(files[0]);
    const status = st && st.format === 'sdc.run-status/1' ? st : null;
    const stop = readJson(files[1]);
    const sets = lines(files[2]).filter((r) => r.format === 'sdc.run-settings/1');
    const calls = lines(files[3]).filter((r) => r.model || r.usage || r.adapter);
    const decs = lines(files[4]).filter((r) => r.model || r.cost_usd != null || r.cost != null);
    // the cases' event times: the first and the last, and the root's close
    let first = null, last = null, term = null;
    const times = c?.times || new Map();
    for (const e of caseDirs) {
      let t = times.get(e.dir);
      if (!t || t.sig !== e.sig) {
        const evs = readEvents(e.dir);
        const ats = evs.map((x) => msOf(x.at)).filter((x) => x != null);
        const tm = evs.findLast?.((x) => x.event === 'terminated') ?? [...evs].reverse().find((x) => x.event === 'terminated');
        t = {sig: e.sig, first: ats.length ? Math.min(...ats) : null, last: ats.length ? Math.max(...ats) : null, term: tm ? {at: msOf(tm.at), outcome: tm.outcome ?? null} : null};
        times.set(e.dir, t);
      }
      if (t.first != null && (first == null || t.first < first)) first = t.first;
      if (t.last != null && (last == null || t.last > last)) last = t.last;
      if (e.depth === 0 && t.term) term = t.term;
    }
    const started = msOf(sets[0]?.at) ?? msOf(calls[0]?.started) ?? first;
    const recorded = fs.existsSync(files[3]) || fs.existsSync(files[4]);
    const cost = recorded ? calls.reduce((a, r) => a + (Number(r.cost_usd) || 0), 0) + decs.reduce((a, r) => a + (Number(r.cost_usd ?? r.cost) || 0), 0) : null;
    const v = {
      started, last,
      status: status ? {state: status.state ?? null, at: msOf(status.at), outcome: status.outcome ?? null, delivered: typeof status.delivered === 'boolean' ? status.delivered : null, closedBy: status.closed_by ?? null} : null,
      stop: stop && stop.format === 'sdc.run-stopped/1' && msOf(stop.at) != null ? {at: msOf(stop.at), by: stop.by ?? null, cause: stop.cause || stop.verdict_at_stop || null} : null,
      term,
      cost, calls: recorded ? calls.length + decs.length : null, work: recorded ? calls.length : null, decider: recorded ? decs.length : null,
      setting: settingText(sets.at(-1), calls), autonomy: sets.at(-1)?.autonomy ?? null,
      forced: count(status?.forced), unstuck: count(status?.unstuck), reopens: count(status?.reopens),
    };
    c = {sig, times, v};
    summaries.set(s.path, c);
  }
  const v = c.v;
  // Whether it is live (livenessOf): the runner writes no pid and no lock, so a run that has not ended is
  // 'unknown' (a run directory of `runbook run`) or history (any other directory): never 'running'.
  const closed = v.status?.state === 'closed' ? {at: v.status.at, outcome: v.status.outcome ?? v.term?.outcome ?? null, delivered: v.status.delivered}
    : v.term ? {at: v.term.at, outcome: v.term.outcome, delivered: null} : null;
  if (closed && closed.delivered == null) closed.delivered = /^(operating|delivered)$/.test(String(closed.outcome || ''));
  const state = v.stop ? 'stopped' : closed ? 'closed' : fs.existsSync(path.join(runDir, 'settings.json')) ? 'unknown' : 'idle';
  const ended = state === 'stopped' ? v.stop.at : state === 'closed' ? closed.at ?? v.last : state === 'unknown' ? null : v.last;
  return {state, started: v.started, ended, last: v.last, outcome: closed?.outcome ?? null, delivered: closed ? closed.delivered : null, closedBy: v.status?.closedBy ?? null,
    stopBy: v.stop?.by ?? null, cause: v.stop?.cause ?? null, cost: v.cost, calls: v.calls, work: v.work, decider: v.decider, setting: v.setting, autonomy: v.autonomy,
    forced: v.forced, unstuck: v.unstuck, reopens: v.reopens};
}

// When each event was written. The events' own `at` is the instant the caller passed to a whole leg
// of the run (every step of the leg shares it), so the wall clock of a step comes from here:
// the mtime of events.jsonl when the server saw a line appear. On first sight only the last line
// is dated (the file's mtime is when that line was appended); earlier ones stay undated.
function observe(src, dir) {
  let mtime;
  try { mtime = fs.statSync(path.join(dir, 'events.jsonl')).mtimeMs; } catch { return; }
  const evs = readEvents(dir);
  let prev = src.times.get(dir);
  if (prev && evs.length < prev.n) prev = null; // the file was replaced: date it as on first sight
  const t = prev ? prev.t : {};
  if (!prev) { if (evs.length) t[evs[evs.length - 1].seq] = Math.round(mtime); }
  else for (const e of evs.slice(prev.n)) if (t[e.seq] == null) t[e.seq] = Math.round(mtime);
  src.times.set(dir, {n: evs.length, t});
}

// the root of a case tree, as the driver finds it (main.rs tree_root): up while the parent is a `children` directory
function treeRootOf(dir) {
  let d = dir;
  while (path.basename(path.dirname(d)) === 'children') d = path.dirname(path.dirname(d));
  return d;
}

function rootsOf(src) {
  if (!fs.existsSync(src.path)) return [];
  if (isCase(src.path)) return [src.path];
  return listDir(src.path).map((n) => path.join(src.path, n)).filter(isCase);
}

function walk(dir, parent, depth, out) {
  out.push({dir, parent, depth});
  for (const n of listDir(path.join(dir, 'children'))) {
    const c = path.join(dir, 'children', n);
    if (isCase(c)) walk(c, dir, depth + 1, out);
  }
  return out;
}

function caseSig(dir) {
  return ['events.jsonl', 'observed.jsonl', 'case.json', 'authority.json', 'decisions.json', 'evidence', 'evaluations', 'work/questions.yaml']
    .map((f) => statSig(path.join(dir, f))).join('|');
}

function runDirOf(src) {
  const p = src.path;
  if (path.basename(p) === 'store') return path.dirname(p);
  if (isCase(p)) { const up = path.dirname(p); return path.basename(up) === 'store' ? path.dirname(up) : up; }
  return p;
}

function scan(src) {
  const dirs = rootsOf(src).flatMap((r) => walk(r, null, 0, []));
  const changed = [];
  const seen = new Set();
  for (const {dir, parent, depth} of dirs) {
    seen.add(dir);
    const sig = caseSig(dir);
    const prev = src.cases.get(dir);
    if (!prev || prev.sig !== sig) {
      observe(src, dir);
      src.cases.set(dir, {dir, parent, depth, sig, data: null});
      changed.push(dir);
    }
  }
  for (const dir of Array.from(src.cases.keys())) if (!seen.has(dir)) { src.cases.delete(dir); src.times.delete(dir); changed.push(dir); }
  const runDir = runDirOf(src);
  const runSig = ['run.log', 'console.txt', 'llm-calls.jsonl', 'decisions.jsonl', 'ui-actions.jsonl', 'run-stopped.json', 'run-settings.jsonl'].map((f) => statSig(path.join(runDir, f))).join('|') + listDir(runDir).join(',');
  const runChanged = runSig !== src.runSig;
  src.runSig = runSig;
  const liveSig = liveSignature(src, runDir);
  const liveChanged = liveSig !== src.liveSig;
  src.liveSig = liveSig;
  if (changed.length || runChanged) src.version++;
  return {changed, runChanged, liveChanged};
}

// ---- live: agent and decider calls in progress, and the run's file changes -----------------------
// A call is in progress while its record directory has request.json and response.json is missing
// or empty: a model or decider call's adapter stdout goes into response.json, which is created empty
// when the call starts and filled when it ends.
// With status.json (sdc.call-status/1) the call says itself: state running is in progress, finished
// and failed are not, whatever response.json holds. Without it the rule above applies.
const RECORD_KINDS = ['llm', 'decider'];
const alive = (pid) => Number.isInteger(Number(pid)) && Number(pid) > 0 && fs.existsSync(`/proc/${Number(pid)}`);
function callStatus(dir) {
  const s = readJson(path.join(dir, 'status.json'));
  if (!s || typeof s !== 'object') return null;
  return {state: typeof s.state === 'string' ? s.state : null, pid: s.pid ?? null, started_at: s.started_at ?? null, finished_at: s.finished_at ?? null, exit: s.exit ?? null};
}
function openRecords(runDir) {
  const out = [];
  for (const kind of RECORD_KINDS) for (const name of listDir(path.join(runDir, kind))) {
    const d = path.join(runDir, kind, name);
    if (!fs.existsSync(path.join(d, 'request.json'))) continue;
    const status = callStatus(d);
    if (status?.state) {
      if (status.state !== 'running') continue;
    } else {
      let resp;
      try { resp = fs.statSync(path.join(d, 'response.json')); } catch { resp = null; }
      if (resp && resp.size > 0) continue;
    }
    out.push({kind, name, dir: d, status});
  }
  return out;
}
// Is the run live. `runbook run` writes no pid and no lock file (docs/loom-run-directory.md): what it leaves
// is settings.json (first), run-stopped.json or status.json (the end). So a run directory with an end record
// is `ended`; one with settings.json and no end record is `unknown` (running, or killed: the files do not say);
// any other directory (a case store of the old driver) is `history`: shown, never reported as live.
function livenessOf(src) {
  const runDir = runDirOf(src);
  if (readJson(path.join(runDir, 'run-stopped.json')) || readJson(path.join(runDir, 'status.json'))?.state === 'closed') return {state: 'ended'};
  if (fs.existsSync(path.join(runDir, 'settings.json'))) return {state: 'unknown', why: 'the runner writes no record of a live process'};
  return {state: 'history', why: 'a case store of the old driver: nothing runs it'};
}
function liveSignature(src, runDir) {
  const recs = openRecords(runDir).map((r) => `${r.kind}/${r.name}:${statSig(path.join(r.dir, 'events.jsonl'))}:${r.status ? alive(r.status.pid) : '-'}`);
  return recs.join('|') + '#' + statSig(path.join(runDir, 'fs', 'changes.jsonl')) + '#' + RECORD_KINDS.map((k) => listDir(path.join(runDir, k)).length).join(',') + '#' + livenessOf(src).state;
}

const argShort = (input) => {
  let j = input;
  if (typeof j === 'string') { try { j = JSON.parse(j); } catch { return j.slice(0, 160); } }
  if (!j || typeof j !== 'object') return String(j ?? '');
  const v = j.file_path || j.path || j.pattern || j.command || j.url || j.query || j.description || '';
  return String(v).replace(/^.*\/(llm|decider)\/\d+-[^/]+\/cwd\//, '').slice(0, 160);
};

// The progress of one call, from its metaharness.event/1 stream (the adapter redirects metaharness'
// stdout into events.jsonl; metaharness prints one line per event as it happens).
const progressCache = new Map();
function recordProgress(r, runDir) {
  const ev = path.join(r.dir, 'events.jsonl');
  const sig = statSig(ev) + statSig(path.join(r.dir, 'request.json')) + statSig(path.join(r.dir, 'response.json')) + statSig(path.join(r.dir, 'status.json')) + (r.status ? alive(r.status.pid) : '');
  const hit = progressCache.get(r.dir);
  if (hit && hit.sig === sig) return hit.value;
  const req = readJson(path.join(r.dir, 'request.json'), {});
  const dreq = r.kind === 'decider' ? readJson(path.join(r.dir, 'decide-request.json'), {}) : null;
  const mtime = (f) => { try { return Math.round(fs.statSync(path.join(r.dir, f)).mtimeMs); } catch { return null; } };
  // request.json.case (a case id, or a path whose last part is one) joins the call to its case;
  // before it existed a decider call named its case in decide-request.json and an agent call none
  const caseOf = typeof req.case === 'string' && req.case ? path.basename(req.case) : dreq?.case_dir ? path.basename(dreq.case_dir) : null;
  const startedAt = Date.parse(r.status?.started_at || '');
  const p = {kind: r.kind, rel: `${r.kind}/${r.name}`, n: Number(r.name.split('-')[0]) || null,
    action: r.kind === 'decider' ? (dreq.action || r.name.replace(/^\d+-/, '')) : (req.node || r.name.replace(/^\d+-/, '')),
    case: caseOf, caseFrom: typeof req.case === 'string' && req.case ? 'request.json' : dreq?.case_dir ? 'decide-request.json' : null,
    status: r.status ? {state: r.status.state, pid: r.status.pid, alive: alive(r.status.pid)} : null,
    model: req.model || null, maxTurns: req.max_turns ?? null, budgetUsd: req.max_budget_usd ?? null,
    started: (isNaN(startedAt) ? null : startedAt) ?? (r.kind === 'decider' ? mtime('decide-request.json') : null) ?? mtime('request.json'),
    lastAt: mtime('events.jsonl'), events: 0, turns: 0, tools: 0, denied: 0, outTokens: 0, last: null, recent: [], ended: false};
  if (dreq) Object.assign(p, {type: dreq.type, capability: dreq.capability, subject: dreq.subject, revision: dreq.revision, question: dreq.question?.id || null});
  const text = readText(ev) || '';
  const reqs = new Map();
  const calls = new Map();
  for (const line of text.split('\n')) {
    if (!line.trim()) continue;
    let e;
    try { e = JSON.parse(line); } catch { continue; }
    p.events++;
    if (e.event === 'session.started' && e.model) p.model = e.model;
    if (e.request_id) {
      const u = e.usage?.output_tokens;
      reqs.set(e.request_id, Math.max(reqs.get(e.request_id) || 0, Number(u) || 0));
    }
    if (e.event === 'tool.requested') {
      p.tools++;
      const c = {seq: e.seq, name: e.name, arg: argShort(e.input), at: e.at || null, decision: null};
      calls.set(e.call_id, c);
      p.recent.push(c);
      p.last = {kind: 'tool', name: e.name, arg: c.arg, seq: e.seq};
    } else if (e.event === 'tool.decided') {
      const c = calls.get(e.call_id);
      const d = e.decision?.decision || e.decision?.verdict || e.decision;
      if (c) c.decision = d;
      if (d && d !== 'allow') p.denied++;
    } else if (e.event === 'text' && e.text) p.last = {kind: 'text', text: String(e.text).slice(0, 160)};
    else if (e.event === 'thinking' && (!p.last || p.last.kind !== 'tool')) p.last = {kind: 'thinking'};
    else if (e.event === 'session.ended') { p.ended = true; if (e.num_turns != null) p.turnsFinal = Number(e.num_turns); }
  }
  // A turn is one model reply; every tool round trip starts a new one (a finished call reports
  // num_turns = tool calls + 1), so the running count is tool calls + 1.
  p.turns = p.turnsFinal ?? (p.events ? p.tools + 1 : 0);
  p.outTokens = Array.from(reqs.values()).reduce((a, b) => a + b, 0);
  p.recent = p.recent.slice(-8);
  progressCache.set(r.dir, {sig, value: p});
  return p;
}

// runs/<run>/fs/changes.jsonl (docs/fs-changes.md), read incrementally; the list keeps every
// field but `diff`, which /api/fschange reads back from the line's byte offset. A late
// `sdc.fs-attribution/1` line (no seq) replaces the attribution of the change it names.
const fsCache = new Map();
function fsChanges(runDir) {
  const file = path.join(runDir, 'fs', 'changes.jsonl');
  let size;
  try { size = fs.statSync(file).size; } catch { fsCache.delete(runDir); return {present: false, list: []}; }
  let c = fsCache.get(runDir);
  if (!c || size < c.offset) { c = {offset: 0, list: [], at: new Map(), bySeq: new Map(), late: new Map()}; fsCache.set(runDir, c); }
  if (size > c.offset) {
    const fd = fs.openSync(file, 'r');
    const buf = Buffer.alloc(size - c.offset);
    fs.readSync(fd, buf, 0, buf.length, c.offset);
    fs.closeSync(fd);
    let start = 0;
    for (let i = 0; i < buf.length; i++) {
      if (buf[i] !== 10) continue;
      const line = buf.subarray(start, i).toString('utf8');
      const off = c.offset + start, len = i - start;
      start = i + 1;
      if (!line.trim()) continue;
      try {
        const r = JSON.parse(line);
        if (r.format === 'sdc.fs-attribution/1') {
          const late = {...r.attribution, late_at: r.at};
          c.late.set(r.change, late);
          const x = c.bySeq.get(r.change);
          if (x) x.attribution = late;
          continue;
        }
        const {diff, ...rest} = r;
        if (c.late.has(r.seq)) rest.attribution = c.late.get(r.seq);
        if (rest.lines_added == null && diff) Object.assign(rest, (({add, del}) => ({lines_added: add, lines_removed: del}))(diffCounts(diff)));
        rest.has_diff = !!diff;
        rest.meaning = changeMeaning(runDir, rest);
        c.list.push(rest);
        c.bySeq.set(r.seq, rest);
        c.at.set(r.seq, {off, len});
      } catch { /* not JSON: skipped */ }
    }
    c.offset += start; // a line without its newline yet is read on the next call
  }
  return {present: true, list: c.list};
}
function fsChange(runDir, seq) {
  fsChanges(runDir);
  const c = fsCache.get(runDir);
  const loc = c?.at.get(seq);
  if (!loc) return null;
  const fd = fs.openSync(path.join(runDir, 'fs', 'changes.jsonl'), 'r');
  const buf = Buffer.alloc(loc.len);
  fs.readSync(fd, buf, 0, loc.len, loc.off);
  fs.closeSync(fd);
  const r = JSON.parse(buf.toString('utf8'));
  if (c.late.has(seq)) r.attribution = c.late.get(seq);
  if (!r.diff && !r.binary && !r.too_large && !(r.op === 'rename' && r.before === r.after)) {
    const blob = (h) => (h && /^[0-9a-f]{64}$/.test(h) ? readText(path.join(runDir, 'fs', 'blobs', h)) : null);
    const a = blob(r.before), b = blob(r.after);
    if (a != null || b != null) { r.diff = unifiedDiff(a, b, r.from_path || r.path, r.path); r.diff_source = 'computed by the inspector from fs/blobs'; }
  }
  r.meaning = c.bySeq.get(seq)?.meaning || changeMeaning(runDir, r);
  return r;
}

// What a change means (web/meaning.js): planning, spec, direction and decision files are parsed
// before and after from fs/blobs; code is classified by its path alone.
function changeMeaning(runDir, r) {
  if (classify(r.path).cls === 'code') return meaningOf(r, null, null);
  const blob = (h) => (h && /^[0-9a-f]{64}$/.test(h) ? readText(path.join(runDir, 'fs', 'blobs', h)) : null);
  const m = meaningOf(r, blob(r.before), blob(r.after));
  return m;
}
// Without fs/changes.jsonl: the planning artifacts as they stand in each tree's work/ (their front
// matter keeps every transition with its time and actor).
function workFacts(src) {
  const files = [];
  const walkMd = (dir, rel, depth) => {
    if (depth > 4 || files.length > 300) return;
    for (const n of listDir(dir)) {
      const p = path.join(dir, n);
      let st;
      try { st = fs.statSync(p); } catch { continue; }
      if (st.isDirectory()) walkMd(p, `${rel}/${n}`, depth + 1);
      else if (n.endsWith('.md') && st.size < 65536) files.push({path: `${rel}/${n}`.replace(/^\//, ''), text: readText(p) || ''});
    }
  };
  for (const r of rootsOf(src)) for (const sub of ['aep', '.engineering/planning']) walkMd(path.join(r, 'work', sub), `${path.relative(src.path, r) || '.'}/work/${sub}`, 0);
  return {files};
}

function liveInfo(src) {
  const runDir = runDirOf(src);
  const records = openRecords(runDir).map((r) => recordProgress(r, runDir));
  const changes = fsChanges(runDir);
  return {now: Date.now(), runDir, records, liveness: livenessOf(src), fs: {present: changes.present, changes: changes.list.slice(-3000)}};
}

function loadCase(src, entry) {
  if (entry.data) return entry.data;
  const d = entry.dir;
  const evidence = {};
  for (const f of listDir(path.join(d, 'evidence'))) if (f.endsWith('.json')) {
    const r = readJson(path.join(d, 'evidence', f));
    if (r) evidence[r.id || f.replace(/\.json$/, '')] = r;
  }
  const evaluations = {};
  for (const f of listDir(path.join(d, 'evaluations'))) if (f.endsWith('.json')) {
    const r = readJson(path.join(d, 'evaluations', f));
    if (r) evaluations['evaluations/' + f] = r;
  }
  const caseJson = readJson(path.join(d, 'case.json'), {});
  const events = readEvents(d);
  const bindings = readJson(path.join(d, 'bindings.json'), {});
  // observed.jsonl (sdc.observed/1): the driver's wall clock of each event line, by the line's
  // index in events.jsonl; it dates the step, where the server's own clock only dates what it saw
  const times = {...(src.times.get(d)?.t || {})};
  const observed = readText(path.join(d, 'observed.jsonl'));
  const timeSource = {};
  for (const k of Object.keys(times)) timeSource[k] = 'server';
  if (observed != null) {
    for (const l of observed.split('\n')) {
      if (!l.trim()) continue;
      let o;
      try { o = JSON.parse(l); } catch { continue; }
      const t = Date.parse(o?.observed_at);
      const e = Number.isInteger(o?.index) ? events[o.index] : null;
      if (e && !isNaN(t) && (o.event == null || o.event === e.event)) { times[e.seq] = t; timeSource[e.seq] = 'driver'; }
    }
  }
  const root = treeRootOf(d);
  entry.data = {
    tree: path.relative(src.path, root) || '.',
    observed: observed != null,
    timeSource,
    scope: waitsOnScope(events) ? scopeQuestions({dir: d, bindings, events}) : null,
    stall: amendStall({dir: d, bindings, events, id: caseJson.id || path.basename(d)}, runDirOf(src)),
    id: caseJson.id || path.basename(d),
    dir: d,
    rel: path.relative(src.path, d) || '.',
    parent: entry.parent ? (readJson(path.join(entry.parent, 'case.json'), {}).id || path.basename(entry.parent)) : null,
    depth: entry.depth,
    case: caseJson,
    ir: readJson(path.join(d, 'ir.json')),
    bindings,
    link: readJson(path.join(d, 'link.json')),
    authority: readJson(path.join(d, 'authority.json'), []),
    decisions: readJson(path.join(d, 'decisions.json'), []),
    events,
    times,
    evidence,
    evaluations,
  };
  return entry.data;
}

// ---- run log: commands, verdicts, and whatever LLM call records the run wrote ------------------
const LLM_KEYS = ['model', 'input_tokens', 'output_tokens', 'tokens', 'usage', 'adapter', 'tool_calls', 'allowed_tools'];
function collectRecords(value, where, out) {
  if (Array.isArray(value)) { value.forEach((v) => collectRecords(v, where, out)); return; }
  if (!value || typeof value !== 'object') return;
  const keys = Object.keys(value);
  if (keys.some((k) => LLM_KEYS.includes(k))) out.push({where, record: value});
  else for (const k of keys) collectRecords(value[k], where, out);
}

function parseJsonBlocks(text, where, out) {
  // single-line JSON records
  for (const line of text.split('\n')) {
    const t = line.trim();
    if (t.startsWith('{') && t.endsWith('}')) { try { collectRecords(JSON.parse(t), where, out); } catch { /* not json */ } }
  }
}

function runInfo(src) {
  const runDir = runDirOf(src);
  const log = readText(path.join(runDir, 'run.log'));
  const commands = [];
  if (log) {
    const lines = log.split('\n');
    let cur = null;
    for (const line of lines) {
      if (line.startsWith('$ ')) { cur = {cmd: line.slice(2), out: [], exit: null}; commands.push(cur); }
      else if (cur && /^exit -?\d+$/.test(line)) { cur.exit = Number(line.slice(5)); cur = null; }
      else if (cur) cur.out.push(line);
    }
    for (const c of commands) {
      const text = c.out.join('\n');
      c.out = text.length > 6000 ? text.slice(0, 6000) + '\n…' : text;
      try { const j = JSON.parse(text); c.verdict = j.verdict || null; c.case_dir = j.case_dir || null; } catch { c.verdict = null; }
    }
  }
  // LLM call records: llm-calls.jsonl (one record per agent-node call, written by the adapter
  // wrapper), else single-line records in run.log or traces/*.jsonl
  const llm = [];
  const calls = readText(path.join(runDir, 'llm-calls.jsonl'));
  if (calls != null) {
    for (const line of calls.split('\n')) {
      if (!line.trim()) continue;
      try {
        const r = JSON.parse(line);
        const dir = listDir(path.join(runDir, 'llm')).find((n) => n.endsWith('-' + r.action) && (r.record ? r.record.includes(n) : true));
        llm.push({where: 'llm-calls.jsonl', record: r, dir: r.record || (dir ? path.join('llm', dir) : null)});
      } catch { /* a line mid-write */ }
    }
  } else {
    if (log) parseJsonBlocks(log, 'run.log', llm);
    for (const n of listDir(path.join(runDir, 'traces'))) if (n.endsWith('.jsonl')) parseJsonBlocks(readText(path.join(runDir, 'traces', n)) || '', 'traces/' + n, llm);
  }
  const expected = readText(path.join(runDir, 'expected-llm-calls.txt'));
  const decisions = jsonl(path.join(runDir, 'decisions.jsonl')).map((r) => ({...decisionRecord(r), who: whoKind(r.principal) || 'none'}));
  return {runDir, hasLog: !!log, commands, llm: llm.slice(0, 2000), expected, decisions, console: readText(path.join(runDir, 'console.txt')), stopped: stoppedOf(runDir), settings: settingsOf(runDir)};
}

// ---- structural data: compiled IRs, canon check output, binding examples, eval levels ----------
function meta() {
  const compiled = {};
  for (const f of listDir(path.join(sdc, 'protocol/compiled'))) if (f.endsWith('.ir.json')) {
    const ir = readJson(path.join(sdc, 'protocol/compiled', f));
    if (ir) compiled[ir.protocol.id] = ir;
  }
  const check = {};
  const text = readText(path.join(sdc, 'docs/check-output.txt')) || '';
  for (const m of text.matchAll(/checked: protocol `([^`]+)` revision (\d+): (\d+) states, (\d+) properties, (\d+) findings?/g)) {
    const key = `${m[1]}@${m[2]}`;
    if (!check[key]) check[key] = {protocol: m[1], revision: Number(m[2]), states: Number(m[3]), properties: Number(m[4]), findings: Number(m[5])};
  }
  const levels = {};
  for (const f of listDir(path.join(sdc, 'eval/levels'))) if (f.endsWith('.json')) levels[f] = readJson(path.join(sdc, 'eval/levels', f));
  return {sdc, compiled, check, checkSource: 'docs/check-output.txt', levels};
}

// What decisions.mjs needs of a source: its paths and its cases.
function ctx(src) {
  const cases = Array.from(src.cases.values()).map((e) => ({dir: e.dir, rel: path.relative(src.path, e.dir) || '.', ...pick(loadCase(src, e))}));
  return {root: sdc, storePath: src.path, runDir: runDirOf(src), cases, index: src.index};
}
const pick = (c) => ({id: c.id, case: c.case, ir: c.ir, bindings: c.bindings, events: c.events, evaluations: c.evaluations, evidence: c.evidence, parent: c.parent});

// ---- HTTP ------------------------------------------------------------------------------------
const clients = new Set();
// Every path leaves the server relative to the repository (sdc/), its workspace (../) or ~, never
// as an absolute home path: what the page shows and what a screenshot or a saved page keeps.
const HOME = process.env.HOME || '/nonexistent';
// a worktree whose runs/ links to another checkout's runs/ has that checkout as a second root
const roots = (() => { try { const r = path.dirname(fs.realpathSync(path.join(sdc, 'runs'))); return r !== sdc ? [sdc, r] : [sdc]; } catch { return [sdc]; } })();
const rootRel = (s, r) => s.split(r + '/').join('').split(r).join('.').split(path.dirname(r) + '/').join('../');
// every directory the server was given (--runs DIR, the run directory of each store argument) shows by
// the name of its parent: <runs>/<run>/..., wherever the checkout or the run lives
const givenDirs = () => [...new Set([...runsDirs.map((d) => path.join(d, 'x')), ...sources.map(runDirOf)].map((d) => path.dirname(d)))]
  .filter((d) => d !== path.dirname(d)).sort((x, y) => y.length - x.length);
const givenRel = (s, d) => s.split(d + '/').join(path.basename(d) + '/');
const shownPaths = (s) => givenDirs().reduce(givenRel, roots.reduce(rootRel, s))
  .split(HOME + '/').join('~/').split(HOME).join('~').replace(/\/home\/[^/\s"'<>\\]+/g, '~');
function push(name, data) {
  const msg = `event: ${name}\ndata: ${shownPaths(JSON.stringify(data))}\n\n`;
  for (const res of clients) res.write(msg);
}

let sourcesSig = '';
function tick() {
  discover();
  for (const src of sources) inView(src, () => {
    const existed = src.exists;
    src.exists = fs.existsSync(src.path);
    const {changed, runChanged, liveChanged} = scan(src);
    if (changed.length || runChanged || liveChanged || existed !== src.exists) {
      push('change', {source: src.index, version: src.version, cases: changed.map((d) => path.relative(src.path, d) || '.'), run: runChanged, live: liveChanged || changed.length > 0});
    }
  });
  // the list the run picker shows: a run that appeared, a store that came, a run that started, closed or
  // was stopped (its state); cost, calls and times are fetched when the picker opens
  const sig = sources.map((s) => inView(s, () => `${s.path}:${fs.existsSync(s.path)}:${statSig(path.join(runDirOf(s), 'run-stopped.json'))}:${runSummary(s).state}`)).join('|');
  if (sig !== sourcesSig) { if (sourcesSig) push('sources', {count: sources.length}); sourcesSig = sig; }
}

const MIME = {'.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.svg': 'image/svg+xml'};
const send = (res, code, body, type = 'application/json; charset=utf-8') => {
  res.writeHead(code, {'content-type': type, 'cache-control': 'no-store'});
  if (Buffer.isBuffer(body) && !/^(text|image\/svg)/.test(type)) return res.end(body);
  res.end(shownPaths(typeof body === 'string' ? body : Buffer.isBuffer(body) ? body.toString('utf8') : JSON.stringify(body)));
};

const server = http.createServer((req, res) => {
  const src = sources[Number(new URL(req.url, 'http://x').searchParams.get('source') || 0)];
  inView(src, () => handle(req, res));
});

function handle(req, res) {
  // the inspector reads: no route takes a body or a command
  if (req.method !== 'GET' && req.method !== 'HEAD') return send(res, 405, {error: 'this server only reads'});
  const url = new URL(req.url, 'http://x');
  const p = url.pathname;
  const src = sources[Number(url.searchParams.get('source') || 0)];
  if (p === '/events') {
    res.writeHead(200, {'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive'});
    res.write(`event: hello\ndata: ${JSON.stringify({versions: sources.map((s) => s.version)})}\n\n`);
    clients.add(res);
    req.on('close', () => clients.delete(res));
    return;
  }
  if (p === '/api/sources') return send(res, 200, sources.map(sourceRow));
  if (p === '/api/clock') return send(res, 200, {clock: !!CLOCK, now: Date.now(), at: clockAt});
  if (p === '/api/meta') return send(res, 200, meta());
  if (p === '/api/view') return send(res, 200, src ? viewOf(src).view : null);
  if (p === '/api/view-info') return send(res, 200, src ? viewInfo(src) : null);
  if (!src && p.startsWith('/api/')) return send(res, 404, {error: 'no such source'});
  if (p === '/api/tree') {
    const only = url.searchParams.get('cases');
    const want = only == null ? null : new Set(only.split(',').filter(Boolean));
    const all = Array.from(src.cases.values());
    const cases = all.filter((e) => !want || want.has(path.relative(src.path, e.dir) || '.')).map((e) => loadCase(src, e));
    return send(res, 200, {source: src.index, version: src.version, now: Date.now(), exists: fs.existsSync(src.path), present: all.map((e) => path.relative(src.path, e.dir) || '.'), cases});
  }
  if (p === '/api/run') return send(res, 200, runInfo(src));
  if (p === '/api/live') return send(res, 200, liveInfo(src));
  if (p === '/api/workfacts') return send(res, 200, workFacts(src));
  if (p === '/api/fschange') {
    const r = fsChange(runDirOf(src), Number(url.searchParams.get('seq')));
    return send(res, r ? 200 : 404, r || {error: 'no such change'});
  }
  if (p === '/api/inbox') {
    const x = ctx(src);
    return send(res, 200, {items: inbox(x), history: history(x), stopped: stoppedOf(runDirOf(src))});
  }
  if (p === '/api/blob') {
    const rel = url.searchParams.get('case') || '.';
    const hash = url.searchParams.get('hash') || '';
    if (!/^[0-9a-f]{64}$/.test(hash) || rel.includes('..')) return send(res, 400, {error: 'bad request'});
    const t = readText(path.join(src.path, rel, 'blobs', hash));
    return send(res, t == null ? 404 : 200, t == null ? 'not found' : t.slice(0, 200000), 'text/plain; charset=utf-8');
  }
  const file = p === '/' ? 'index.html' : p.slice(1);
  const abs = path.join(here, 'web', file);
  if (!abs.startsWith(path.join(here, 'web')) || !fs.existsSync(abs)) return send(res, 404, 'not found', 'text/plain');
  return send(res, 200, fs.readFileSync(abs), MIME[path.extname(abs)] || 'application/octet-stream');
}

tick();
if (CLOCK) setInterval(() => { const a = readGate(CLOCK, 'at'); if (a !== clockAt) { tick(); clockAt = a; } }, 25);
else setInterval(tick, 700);
setInterval(() => { for (const res of clients) res.write(': ping\n\n'); }, 15000);
// --port 0 takes a free port; the line below names the one taken (the ux loop reads it)
server.on('error', (e) => { console.error(`sdc-ui: cannot listen on ${host}:${port}: ${e.code || e.message}`); process.exit(1); });
server.listen(port, host, () => {
  console.log(`sdc-ui on http://${host}:${server.address().port}/`);
  for (const s of sources) console.log(`  source ${s.index}: ${s.path}${fs.existsSync(s.path) ? '' : ' (waiting for it to appear)'}`);
});
