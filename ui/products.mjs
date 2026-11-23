// What a step produced: the files of a run, the content of one of them, the product of the run and the output of an
// action (story inspector-shows-step-products). The inspector reads and acts on nothing, so everything here
// reads. Every file is reached through ./runfiles.mjs: the real path lies under the run, the type is read
// before the file is opened and only a regular file is read, and a name that starts with a dot (a tool's cache or
// login) is neither listed nor served. The content leaves as text in a JSON member, never as a file with its own
// type, and the page builds nodes from it (web/markdown.js).
import fs from 'node:fs';
import path from 'node:path';
import {runPath, readRegular, hasDotPart, dotName, within} from './runfiles.mjs';

export const LIMIT = 1048576; // the display limit of one file, in bytes
export const TAIL = 4096; // the tail of an action's output, in bytes
// the listing of a run visits at most this many files, directories and entries (of any kind) before it says it was cut
const MAX_FILES = 500, MAX_DIRS = 2000, MAX_ENTRIES = 20000, MAX_DEPTH = 6, RESULT_BYTES = 8192;
// the run log of a case is read from its end, at most this much of it, in pieces of CHUNK
export const LOG_WINDOW = 8 * 1048576;
const CHUNK = 1048576;
const MAX_RECORD_DEPTH = 64; // a record nested deeper than this is cut: no honest record is

// A value with everything nested deeper than `max` replaced by a string that says so. Reaches `max` levels only, so a
// record nested 100000 deep costs as much as one nested `max` deep.
export function limitDepth(v, max = MAX_RECORD_DEPTH, depth = 0) {
  if (v === null || typeof v !== 'object') return v;
  if (depth >= max) return `cut at depth ${max}`;
  if (Array.isArray(v)) return v.map((x) => limitDepth(x, max, depth + 1));
  const o = {};
  for (const [k, x] of Object.entries(v)) o[k] = limitDepth(x, max, depth + 1);
  return o;
}
export const OUTPUT_STREAM = 'standard output followed by standard error';

const posix = (p) => p.split(path.sep).join('/');
const FORMATS = {'.md': 'markdown', '.markdown': 'markdown', '.json': 'json', '.yaml': 'yaml', '.yml': 'yaml'};
export const formatOf = (name) => FORMATS[path.extname(name).toLowerCase()] ?? 'text';

// The text of some bytes that were cut out of a longer file, or null when they are not text: a NUL byte or a
// sequence that is not UTF-8. A cut that fell inside a character does not make the file binary: the partial
// character at the cut is dropped.
export function utf8Text(buf, {cutStart = false, cutEnd = false} = {}) {
  let a = 0, b = buf.length;
  if (cutStart) while (a < b && a < 3 && (buf[a] & 0xc0) === 0x80) a++;
  if (cutEnd) {
    let i = b - 1, back = 0;
    while (i >= a && back < 3 && (buf[i] & 0xc0) === 0x80) { i--; back++; }
    if (i >= a && buf[i] >= 0xc0) {
      const need = buf[i] >= 0xf0 ? 4 : buf[i] >= 0xe0 ? 3 : 2;
      if (b - i < need) b = i;
    }
  }
  const part = buf.subarray(a, b);
  if (part.includes(0)) return null;
  try { return new TextDecoder('utf-8', {fatal: true, ignoreBOM: true}).decode(part); } catch { return null; }
}

// ---- where a file may lie -------------------------------------------------------------------------------
// The one place that says which files of a run are served. A file is served when its REAL path is under the real path of a
// work/ directory of the run (the run's own and each case's), under no dot-directory. A work/ counts only when it is a real
// directory called work in a real directory of the run: a work/ that is a link to anything else is no root, so the run
// directory or store/ cannot be made a root by replacing it. The check is made on the descriptor's own real path after the
// open (the `accept` of readRegular), never on a path computed before it, and the listing and the product use the same function.
const DOT_MSG = 'a name that starts with a dot is neither listed nor served';
const OUTSIDE_WORK = 'only a file under a work/ directory of the run is served';
export function scopeOf(runDir, roots) {
  let runReal = null;
  try { runReal = fs.realpathSync.native(runDir); } catch { return {runReal: null, roots: []}; }
  const real = [];
  for (const lex of roots) {
    const rr = runPath(lex, runDir), parent = runPath(path.dirname(lex), runDir);
    if (rr != null && parent != null && rr === path.join(parent, 'work') && !hasDotPart(path.relative(runReal, rr))) real.push({lex, real: rr});
  }
  return {runReal, roots: real};
}
// why a file with this real path is not served, or null
export const refusalOf = (scope, real) => (scope.runReal == null || !within(real, scope.runReal) ? OUTSIDE_WORK : hasDotPart(path.relative(scope.runReal, real)) ? DOT_MSG : scope.roots.some((r) => within(real, r.real)) ? null : OUTSIDE_WORK);
// the same for a file that must lie directly in one directory of the run (the blobs of a case, its run log)
export function inDirectory(runDir, dir) {
  let runReal = null;
  try { runReal = fs.realpathSync.native(runDir); } catch { /* no run directory */ }
  const rel = path.relative(runDir, dir);
  const home = runReal && !rel.startsWith('..') ? path.join(runReal, rel) : null;
  return (real) => (home == null || path.dirname(real) !== home || hasDotPart(path.relative(runReal, real)) ? 'not a file directly in the directory it must be in' : null);
}

// ---- one file ------------------------------------------------------------------------------------
// {status, body} for GET /api/file: `rel` is a path relative to the run directory.
export function fileAnswer(runDir, rel, roots = [path.join(runDir, 'work')]) {
  const no = (status, error, extra = {}) => ({status, body: {error, path: typeof rel === 'string' ? rel : null, ...extra}});
  if (typeof rel !== 'string' || rel === '') return no(400, 'the request names no path: name a path relative to the run directory');
  if (rel.includes('\0') || rel.includes('\\')) return no(400, 'the path holds a character that no path of a run holds');
  if (rel.startsWith('/')) return no(400, 'the path is absolute: name a path relative to the run directory');
  const parts = rel.split('/').filter((n) => n !== '' && n !== '.');
  if (!parts.length) return no(400, 'the request names no path: name a path relative to the run directory');
  if (parts.includes('..')) return no(400, 'the path leaves the run');
  if (parts.some(dotName)) return no(404, DOT_MSG);
  const abs = path.join(runDir, ...parts);
  // by its name it lies under a work/ directory (a cheap answer for the paths that never could), and by its real path
  // it must too, which readRegular asks about the open descriptor
  if (!roots.some((r) => abs === r || abs.startsWith(r + path.sep))) return no(404, OUTSIDE_WORK);
  const scope = scopeOf(runDir, roots);
  if (!scope.roots.length) return no(404, OUTSIDE_WORK);
  const r = readRegular(abs, {root: runDir, limit: LIMIT, accept: (real) => refusalOf(scope, real)});
  if (r.state === 'missing') return no(404, 'no such file in the run');
  if (r.state === 'refused') {
    if (r.why === 'not served') return no(404, r.reason);
    if (r.why === 'unverified') return no(403, 'the system cannot name the opened file, so it is not read');
    if (r.why === 'leaves the run') return no(403, 'the file leaves the run (a link that points outside it): it is not read');
    if (r.why === 'more than one link') return no(403, 'the file has more than one link (a hard link may be a file outside the run): it is not read');
    return no(415, `not a regular file (${r.type}): it is not opened`, {type: r.type});
  }
  const text = utf8Text(r.bytes, {cutEnd: r.cut});
  const base = {path: parts.join('/'), size: r.size, mtime: Math.round(r.mtimeMs)};
  if (text == null) return {status: 200, body: {...base, kind: 'binary', type: 'not valid UTF-8 text'}};
  return {status: 200, body: {...base, kind: 'text', format: formatOf(parts.at(-1)), text, bytes: r.size, shown: Buffer.byteLength(text), truncated: r.cut, limit: LIMIT}};
}

// ---- the files of a run --------------------------------------------------------------------------
// The result line of a step file (`result: valid`), or null: the first `result:` line of a short text file.
function resultOf(abs, runDir, scope) {
  const r = readRegular(abs, {root: runDir, limit: RESULT_BYTES, accept: (real) => refusalOf(scope, real)});
  if (r.state !== 'ok') return null;
  const text = utf8Text(r.bytes, {cutEnd: r.cut});
  const m = text == null ? null : /^result:[ \t]*(.*?)[ \t]*$/m.exec(text);
  return m && m[1] ? m[1].slice(0, 120) : null;
}

// The regular files under `roots` (the run's work/ and the work/ of each case), with their size, time, format
// and, for a `.txt` step file, its result. `artifacts` maps a path to the name of the artifact it holds.
// The walk is bounded in every direction: it stops at MAX_FILES files, MAX_DIRS directories and MAX_ENTRIES entries of
// any kind, and says which (`cut`: 'files' | 'directories' | 'entries'), and a directory below MAX_DEPTH levels is not
// entered and is said (`cut`: 'depth'); a directory is read an entry at a time, so one of a million entries costs no more
// than the bound. It does not descend a link to a directory, whatever it points at, so a link back into its own directory
// costs nothing; it lists by lstat, and visits each real directory once. It starts only from the roots of `scopeOf` (a
// work/ that is a link is no root). A link to a file is listed when the file is served (`refusalOf`); a file that is
// not listed because it has more than one link, or is a link that is not served, is counted in `leftOut`.
export function listFiles(runDir, roots, artifacts = new Map()) {
  const scope = scopeOf(runDir, roots);
  const files = [], seen = new Set(), visited = new Set();
  let hiddenDirectories = 0, hiddenFiles = 0, leftOut = 0, cut = null, deeper = false, dirs = 0, entries = 0;
  const walk = (dir, real, depth) => {
    if (visited.has(real)) return;
    visited.add(real);
    if (++dirs > MAX_DIRS) { cut ??= 'directories'; return; }
    const names = [];
    try {
      const d = fs.opendirSync(real);
      try { for (let e = d.readSync(); e && names.length <= MAX_ENTRIES - entries; e = d.readSync()) names.push(e.name); } finally { d.closeSync(); }
    } catch { return; }
    names.sort();
    for (const n of names) {
      if (cut) return;
      if (files.length >= MAX_FILES) { cut = 'files'; return; }
      if (++entries > MAX_ENTRIES) { cut = 'entries'; return; }
      const p = path.join(dir, n), rp = path.join(real, n);
      let st;
      try { st = fs.lstatSync(rp); } catch { continue; }
      if (dotName(n)) { if (st.isDirectory()) hiddenDirectories++; else hiddenFiles++; continue; }
      if (st.isDirectory()) { if (depth < MAX_DEPTH) walk(p, rp, depth + 1); else deeper = true; continue; }
      if (st.isSymbolicLink()) {
        const target = runPath(p, runDir);
        if (target == null || refusalOf(scope, target)) { leftOut++; continue; }
        try { st = fs.statSync(target); } catch { continue; }
      }
      if (!st.isFile()) continue;
      if (st.nlink > 1) { leftOut++; continue; }
      const rel = posix(path.relative(runDir, p));
      if (seen.has(rel)) continue;
      seen.add(rel);
      files.push({path: rel, size: st.size, mtime: Math.round(st.mtimeMs), format: formatOf(n), result: /\.txt$/i.test(n) && st.size <= 65536 ? resultOf(p, runDir, scope) : null, artifact: artifacts.get(rel) ?? null});
    }
  };
  for (const r of scope.roots) walk(r.lex, r.real, 0);
  files.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  cut ??= deeper ? 'depth' : null;
  return {files, hiddenDirectories, hiddenFiles, leftOut, truncated: cut != null, cut};
}

// ---- the product of a run -------------------------------------------------------------------------
// In this order: the view's `product` (a path relative to the run, checked when the view is loaded); else
// work/<runbook>/<runbook>.md for the runbook's id; else the Markdown files directly under work/ and one level
// below, the newest first. `via` says which rule named it. A file is named only when the file route would serve it:
// a regular file with one link that `refusalOf` lets through.
const ID = /^[A-Za-z0-9_][A-Za-z0-9._-]*$/;
function servable(runDir, rel, scope) {
  if (hasDotPart(rel)) return false;
  const real = runPath(path.join(runDir, rel), runDir);
  if (real == null || refusalOf(scope, real)) return false;
  try { const st = fs.statSync(real); return st.isFile() && st.nlink <= 1; } catch { return false; }
}
export function productOf({runDir, roots = [path.join(runDir, 'work')], files, view, ids}) {
  const scope = scopeOf(runDir, roots);
  if (typeof view?.product === 'string') {
    const p = view.product.split('/').filter((n) => n !== '' && n !== '.').join('/');
    if (servable(runDir, p, scope)) return {via: 'view', path: p, missing: false, note: 'named by the view'};
    const there = runPath(path.join(runDir, p), runDir) != null;
    return {via: 'view', path: p, missing: true, note: there ? `the view names ${p}; the inspector does not serve it (only a file under a work/ directory, with one link)` : `the view names ${p}; this run has not written it`};
  }
  for (const id of ids) {
    if (!ID.test(id)) continue;
    const p = `work/${id}/${id}.md`;
    if (servable(runDir, p, scope)) return {via: 'runbook-file', path: p, missing: false, note: `work/<runbook>/<runbook>.md for the runbook ${id}`};
  }
  const md = files.filter((f) => f.format === 'markdown' && /^work\/([^/]+\/)?[^/]+$/.test(f.path)).sort((a, b) => b.mtime - a.mtime || (a.path < b.path ? -1 : 1));
  if (md.length) return {via: 'newest-markdown', path: md[0].path, missing: false, note: 'the newest Markdown file under work/', others: md.slice(1, 6).map((f) => ({path: f.path, mtime: f.mtime}))};
  return {via: null, path: null, missing: false, note: 'not named by this run'};
}

// ---- the output of an action -----------------------------------------------------------------------
// Where a run keeps what an action printed. The driver's case store records `log` on the action's finish: the
// digest of a blob in the case's `blobs/`. `runbook run` writes no such member into the events it projects
// (runbook/src/loom_run/run_dir.rs drops `blob_digest`), but Loom's own run log of the case, which stays in the
// run directory (`store/runlog-case-<hex of the case id>.jsonl`), names the digest of the blob that holds the
// program's standard output followed by its standard error (`store/blobs/<sha256>`). The inspector reads it
// from there: the k-th `ActionFinished` line is the k-th `action_finished` event of the case.
const DIGEST = /^[0-9a-f]{64}$/;
const hex = (s) => Buffer.from(String(s)).toString('hex');
const MIB = 1048576;

// The `ActionFinished` line of Loom's run log of a case that belongs to the action finish number `k` of the case (counted
// from the start: `k`, or from the end: `fromEnd`). The log is read through the type check, in pieces from its end,
// at most LOG_WINDOW of it; a log that fits is read whole and the line is the k-th from the start. Answers
// {line} | {why}.
function actionFinishLine(file, runDir, k, fromEnd) {
  const accept = inDirectory(runDir, path.dirname(file));
  const lines = [];
  let end = Infinity, carry = Buffer.alloc(0), read = 0;
  for (;;) {
    const r = readRegular(file, {root: runDir, limit: CHUNK, end, accept});
    if (r.state === 'missing') return {why: 'the run directory holds no Loom run log of this case'};
    if (r.state === 'refused') return {why: 'the Loom run log of this case is not a regular file inside the run'};
    read += r.bytes.length;
    const atStart = r.from === 0;
    let buf = Buffer.concat([r.bytes, carry]), text;
    if (atStart) { text = buf; carry = Buffer.alloc(0); } else {
      const nl = buf.indexOf(10);
      if (nl < 0) { carry = buf; text = Buffer.alloc(0); } else { carry = buf.subarray(0, nl); text = buf.subarray(nl + 1); }
    }
    buf = null;
    for (const l of text.toString('utf8').split('\n').reverse()) {
      let x;
      try { x = JSON.parse(l); } catch { continue; }
      if (x?.kind === 'ActionFinished') lines.push(x);
    }
    if (atStart) { const all = lines.reverse(); return all[k] ? {line: all[k]} : {why: 'the Loom run log of the case does not hold this action finish'}; }
    if (lines.length > fromEnd) return {line: lines[fromEnd]};
    if (read >= LOG_WINDOW || carry.length >= LOG_WINDOW) return {why: `this action finish was not found within the last ${LOG_WINDOW / MIB} MiB of the run log (the log is ${Math.round(r.size / MIB)} MiB)`};
    end = r.from;
  }
}

export function outputOf({runDir, caseDir, storeDir, caseId, events, seq}) {
  const idx = events.findIndex((e) => e.seq === seq && e.event === 'action_finished');
  if (idx < 0) return null;
  const ev = events[idx];
  const no = (why) => ({recorded: false, why});
  let digest, blobs, stream;
  if (typeof ev.log === 'string' && ev.log) {
    digest = ev.log; blobs = path.join(caseDir, 'blobs'); stream = 'the output of the tool as the case store recorded it';
  } else if (ev.source === 'loom-runlog') {
    const k = events.slice(0, idx + 1).filter((e) => e.event === 'action_finished').length - 1;
    const fromEnd = events.slice(idx + 1).filter((e) => e.event === 'action_finished').length;
    const found = actionFinishLine(path.join(storeDir, `runlog-case-${hex(caseId)}.jsonl`), runDir, k, fromEnd);
    if (!found.line) return no(found.why);
    if (found.line.action !== ev.action) return no('the Loom run log of the case does not hold this action finish');
    if (!found.line.blob_digest) return no('the Loom run log names no output for this action (a step that starts no program has none)');
    digest = found.line.blob_digest; blobs = path.join(storeDir, 'blobs'); stream = OUTPUT_STREAM;
  } else return no('this action finish names no output');
  if (!DIGEST.test(String(digest))) return no('the digest the run log names is not a SHA-256');
  // the blob is a file of the blobs/ directory of the run, by the real path of the open descriptor, under no dot-directory
  const blobPath = path.join(blobs, digest);
  const refused = `the blob ${digest.slice(0, 12)}… is not a regular file inside the run's ${posix(path.relative(runDir, blobs))}/`;
  const r = readRegular(blobPath, {root: runDir, limit: TAIL, tail: true, accept: inDirectory(runDir, blobs)});
  if (r.state === 'missing') return no(`the blob ${digest.slice(0, 12)}… the run log names is not in the run`);
  if (r.state === 'refused') return no(refused);
  let text = utf8Text(r.bytes, {cutStart: r.cut});
  if (text == null) return {recorded: true, digest, stream, bytes: r.size, text: null, truncated: r.cut, note: 'the output is not UTF-8 text'};
  if (r.cut && text.includes('\n')) text = text.slice(text.indexOf('\n') + 1);
  return {recorded: true, digest, stream, bytes: r.size, shown: Buffer.byteLength(text), truncated: r.cut, text};
}
