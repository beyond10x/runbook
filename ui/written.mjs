// What a run wrote (story products-are-what-the-run-wrote): the products of a run are the files its own records name as
// written or changed by an action, grouped by the step that wrote them, and the trees the run was seeded with are
// inputs, one line. Two records of the run directory say it, both made by the runner and its watcher:
//   fs/changes.jsonl                 every effective change of a file under a work root: `root`, `path`, `op`, `after` (the
//                                    SHA-256 of the content the change ended at) and `attribution.step` (docs/fs-changes.md)
//   seeded.json + seeded-files.jsonl what the run was seeded with, listed once when the seed ended: the top-level entries of
//                                    work/ and every seeded file with the SHA-256 of its content (runbook/src/loom_run/seeded.rs)
// A seeded file is a `create` of step 1 in the first record, the same as a file the first step writes (the watcher starts
// before work/ exists), so the second record tells them apart by hash: a path that is listed and whose last change ends at
// the listed hash is an input; listed and ending elsewhere it is `changed`; not listed it is a product (`added` when it lies in
// a seeded tree); listed and deleted it is `removed`. Nothing here compares times.
// Every path in a record is data: it is checked as a name (one run-relative path, no dot-part, under a work root) and its file
// is reached only through the run-path check and `refusalOf`. A record that names a path that leaves a work root is not listed
// and not opened, and is counted, never printed. The files are read through the checked open (runfiles.mjs).
import fs from 'node:fs';
import path from 'node:path';
import {runPath, readRegular, hasDotPart, dotName} from './runfiles.mjs';
import {scopeOf, refusalOf, formatOf, resultOf} from './products.mjs';

export const SEEDED_FORMAT = 'runbook.seeded/1';
export const LIST_LIMIT = 8 * 1048576; // the list of seeded files is read to this size; a longer one is not used
export const MAX_LINES = 500; // the products the page lists; the answer says how many are not shown
const MAX_TREES = 5000, SHA = /^[0-9a-f]{64}$/, KINDS = new Set(['dir', 'file', 'link', 'other']);
const posix = (p) => p.split(path.sep).join('/');

// A run-relative path from a record: {rel} for a plain path, {dot: true} for a path with a dot-part (never listed, never
// counted as wrong), null for anything else (absolute, a `..`, a NUL or a backslash, not a string).
export function cleanRel(p) {
  if (typeof p !== 'string' || p === '' || p.length > 4096 || p.includes('\0') || p.includes('\\') || p.startsWith('/')) return null;
  const parts = p.split('/').filter((n) => n !== '' && n !== '.');
  if (!parts.length || parts.includes('..')) return null;
  if (parts.some(dotName)) return {dot: true};
  return {rel: parts.join('/')};
}

const jsonOf = (r) => { try { return JSON.parse(r.bytes.toString('utf8')); } catch { return null; } };

// ---- the record of what the run was seeded with ----------------------------------------------------------
// {state: 'ok', trees, files: Map(run-relative path -> sha256), count} | {state: 'absent' | 'unusable', why}
// The answer is kept while both files are as they were (size, time, inode): the page asks again and again, and the list may
// hold 20 000 lines. A file that is not a plain regular file is never kept.
const seededCache = new Map();
const sigOf = (p) => { try { const s = fs.lstatSync(p); return s.isFile() ? `${s.size}:${s.mtimeMs}:${s.ino}` : null; } catch { return '-'; } };
export function readSeeded(runDir) {
  const sig = `${sigOf(path.join(runDir, 'seeded.json'))}|${sigOf(path.join(runDir, 'seeded-files.jsonl'))}`;
  const hit = seededCache.get(runDir);
  if (hit && hit.sig === sig && !sig.includes('null')) return hit.value;
  const value = readSeededNow(runDir);
  if (!sig.includes('null')) { seededCache.set(runDir, {sig, value}); if (seededCache.size > 64) seededCache.delete(seededCache.keys().next().value); }
  return value;
}
function readSeededNow(runDir) {
  const rec = readRegular(path.join(runDir, 'seeded.json'), {root: runDir, limit: 1048576});
  if (rec.state === 'missing') return {state: 'absent', why: 'this run has no record of what it was seeded with (seeded.json): an older run'};
  const no = (why) => ({state: 'unusable', why});
  if (rec.state !== 'ok' || rec.cut) return no('the record of what the run was seeded with (seeded.json) is not a readable file');
  const v = jsonOf(rec);
  if (!v || typeof v !== 'object' || v.format !== SEEDED_FORMAT) return no('seeded.json is not a runbook.seeded/1 record');
  if (v.complete !== true) return no('the record of what the run was seeded with is not complete (the seeded tree holds more files than the runner lists)');
  if (!Array.isArray(v.entries) || v.entries.length > MAX_TREES || !Number.isInteger(v.files) || v.files < 0) return no('seeded.json does not say what it must');
  const trees = [];
  for (const e of v.entries) {
    const c = cleanRel(e?.path);
    if (!c?.rel || c.rel.includes('/') || !KINDS.has(e.kind) || !Number.isInteger(e.files) || e.files < 0) return no('an entry of seeded.json is not a top-level entry of work/ with a kind and a file count');
    trees.push({path: c.rel, kind: e.kind, files: e.files});
  }
  const list = readRegular(path.join(runDir, 'seeded-files.jsonl'), {root: runDir, limit: LIST_LIMIT});
  if (list.state !== 'ok' || list.cut) return no('the list of seeded files (seeded-files.jsonl) is not there, not a regular file or longer than the page reads');
  const files = new Map();
  for (const line of list.bytes.toString('utf8').split('\n')) {
    if (!line) continue;
    let x;
    try { x = JSON.parse(line); } catch { return no('a line of seeded-files.jsonl is not JSON'); }
    const c = cleanRel(x?.path);
    if (!c?.rel || typeof x.sha256 !== 'string' || !SHA.test(x.sha256)) return no('a line of seeded-files.jsonl names no plain path and a SHA-256');
    files.set(`work/${c.rel}`, x.sha256);
  }
  if (files.size !== v.files) return no('seeded.json and seeded-files.jsonl do not agree on the number of files');
  return {state: 'ok', trees, files, count: files.size};
}

// The number of steps that have finished: the lines of rounds.jsonl (one per `tree run` step), or null with no such file.
export function finishedSteps(runDir) {
  const r = readRegular(path.join(runDir, 'rounds.jsonl'), {root: runDir, limit: 4 * 1048576});
  if (r.state !== 'ok') return null;
  let n = 0;
  for (const l of r.bytes.toString('utf8').split('\n')) { if (!l.trim()) continue; try { JSON.parse(l); n++; } catch { /* a half line */ } }
  return n;
}

// ---- the products ---------------------------------------------------------------------------------------
// `changes`: the list of fs/changes.jsonl (server.mjs fsChanges). Answers the products with their groups, the inputs, the
// product to open first and the counts of what is not shown.
export function writtenOf({runDir, roots, seeded, changes, finished, artifacts = new Map()}) {
  const scope = scopeOf(runDir, roots);
  const rootRels = new Set(roots.map((r) => posix(path.relative(runDir, r))));
  const state = new Map(); // run-relative path -> what the last change of it says
  let refused = 0;
  const here = (root, p) => {
    const r = cleanRel(root), c = cleanRel(p);
    if (!r?.rel || !rootRels.has(r.rel)) return undefined; // a root that is no work root (an agent's cwd, a decider's): not a product
    if (c == null) { refused++; return undefined; }
    if (c.dot) return undefined;
    return `${r.rel}/${c.rel}`;
  };
  for (const ch of changes) {
    if (!ch || typeof ch !== 'object') continue;
    const step = Number.isInteger(ch.attribution?.step) ? ch.attribution.step : null;
    const action = typeof ch.attribution?.action === 'string' ? ch.attribution.action : null;
    if (ch.op === 'rename' && ch.from_path != null) {
      const from = here(ch.root, ch.from_path);
      if (from) state.set(from, {exists: false, step, seq: ch.seq, action});
    }
    const p = here(ch.root, ch.path);
    if (!p) continue;
    if (ch.op === 'delete') state.set(p, {exists: false, step, seq: ch.seq, action});
    else if (ch.op === 'create' || ch.op === 'modify' || ch.op === 'rename') state.set(p, {exists: true, after: typeof ch.after === 'string' ? ch.after : null, step, seq: ch.seq, action});
  }
  const trees = new Set(seeded.trees.filter((t) => t.kind === 'dir').map((t) => t.path));
  const inTree = (p) => p.startsWith('work/') && trees.has(p.split('/')[1]) && p.split('/').length > 2;
  const changed = new Set(), removed = new Set();
  const products = [];
  for (const [p, s] of state) {
    const listed = seeded.files.get(p);
    if (!s.exists) {
      if (listed !== undefined) { removed.add(p); products.push({path: p, mark: 'removed', ...s}); }
      continue;
    }
    if (listed !== undefined) {
      if (s.after === listed) continue; // the last change ends at the seeded content: an input
      changed.add(p);
      products.push({path: p, mark: 'changed', ...s});
    } else products.push({path: p, mark: inTree(p) ? 'added' : null, ...s});
  }
  const isInput = (p) => seeded.files.has(p) && !changed.has(p) && !removed.has(p);
  // newest step first (a change without a step last), a path order inside a step
  const byStep = (a, b) => (a.step === b.step ? 0 : a.step == null ? 1 : b.step == null ? -1 : b.step - a.step);
  products.sort((a, b) => byStep(a, b) || (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  const total = products.length;
  const shownList = products.slice(0, MAX_LINES);
  let leftOut = 0, gone = 0;
  const files = [];
  for (const x of shownList) {
    const info = describe(runDir, x.path, scope);
    if (info === 'gone' && x.mark !== 'removed') { gone++; continue; }
    if (info === 'refused') { leftOut++; continue; }
    files.push({path: x.path, step: x.step, mark: x.mark, action: x.action, ...(typeof info === 'object' ? info : {size: null, mtime: null, format: formatOf(x.path), result: null}), artifact: artifacts.get(x.path) ?? null});
  }
  const groups = [];
  for (const f of files) {
    let g = groups.at(-1);
    if (!g || g.step !== f.step) { g = {step: f.step, actions: [], files: []}; groups.push(g); }
    g.files.push({path: f.path, mark: f.mark, size: f.size, mtime: f.mtime, format: f.format, result: f.result, artifact: f.artifact});
    if (f.action && !g.actions.includes(f.action)) g.actions.push(f.action);
  }
  // the file to open first: the last file the last finished step wrote, never an input, never a removed file
  const open = products.filter((x) => x.mark !== 'removed' && files.some((f) => f.path === x.path) && (finished == null ? true : x.step != null && x.step <= finished));
  let last = null;
  for (const x of open) if (!last || (x.step ?? -1) > (last.step ?? -1) || ((x.step ?? -1) === (last.step ?? -1) && (x.seq ?? -1) > (last.seq ?? -1))) last = x;
  const others = last ? open.filter((x) => x.step === last.step && x.path !== last.path).sort((a, b) => (b.seq ?? -1) - (a.seq ?? -1)).slice(0, 5).map((x) => ({path: x.path})) : [];
  return {
    groups, files, total, shown: shownList.length, notShown: total - shownList.length, leftOut, gone, refusedRecords: refused,
    inputs: {trees: seeded.trees, files: seeded.count},
    last: last ? {path: last.path, step: last.step, others} : null, finished, isInput,
  };
}

// the size, time, format and result of a file a record names, or 'gone' (nothing is there) or 'refused' (a file the file route
// would not serve). It reads no content but a step file's result line, and only through the checked open.
function describe(runDir, rel, scope) {
  if (hasDotPart(rel)) return 'refused';
  const abs = path.join(runDir, rel);
  try { fs.lstatSync(abs); } catch { return 'gone'; }
  const real = runPath(abs, runDir);
  if (real == null || refusalOf(scope, real)) return 'refused';
  let st;
  try { st = fs.statSync(real); } catch { return 'gone'; }
  if (!st.isFile() || st.nlink > 1) return 'refused';
  return {size: st.size, mtime: Math.round(st.mtimeMs), format: formatOf(rel), result: /\.txt$/i.test(rel) && st.size <= 65536 ? resultOf(abs, runDir, scope) : null};
}
