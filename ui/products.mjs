// What a step produced: the files of a run, the content of one of them, the product of the run and the output of an
// action (story inspector-shows-step-products). The inspector reads and acts on nothing, so everything here
// reads. Every file is reached through ./runfiles.mjs: the real path lies under the run, the type is read
// before the file is opened and only a regular file is read, and a name that starts with a dot (a tool's cache or
// login) is neither listed nor served. The content leaves as text in a JSON member, never as a file with its own
// type, and the page builds nodes from it (web/markdown.js).
import path from 'node:path';
import {listDir, statOf, readText, readRegular, runRel, hasDotPart, dotName} from './runfiles.mjs';

export const LIMIT = 1048576; // the display limit of one file, in bytes
export const TAIL = 4096; // the tail of an action's output, in bytes
const MAX_FILES = 500, MAX_DEPTH = 6, RESULT_BYTES = 8192;
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

// ---- one file ------------------------------------------------------------------------------------
// {status, body} for GET /api/file: `rel` is a path relative to the run directory.
export function fileAnswer(runDir, rel) {
  const no = (status, error, extra = {}) => ({status, body: {error, path: typeof rel === 'string' ? rel : null, ...extra}});
  if (typeof rel !== 'string' || rel === '') return no(400, 'the request names no path: name a path relative to the run directory');
  if (rel.includes('\0') || rel.includes('\\')) return no(400, 'the path holds a character that no path of a run holds');
  if (rel.startsWith('/')) return no(400, 'the path is absolute: name a path relative to the run directory');
  const parts = rel.split('/').filter((n) => n !== '' && n !== '.');
  if (!parts.length) return no(400, 'the request names no path: name a path relative to the run directory');
  if (parts.includes('..')) return no(400, 'the path leaves the run');
  if (parts.some(dotName)) return no(404, 'a name that starts with a dot is neither listed nor served');
  const abs = path.join(runDir, ...parts);
  // a link inside the run that points at a dot-directory of the run is served no more than the directory is
  const real = runRel(abs, runDir);
  if (real != null && hasDotPart(real)) return no(404, 'a name that starts with a dot is neither listed nor served');
  const r = readRegular(abs, {root: runDir, limit: LIMIT});
  if (r.state === 'missing') return no(404, 'no such file in the run');
  if (r.state === 'refused') {
    if (r.why === 'leaves the run') return no(403, 'the file leaves the run (a link that points outside it): it is not read');
    return no(415, `not a regular file (${r.type}): it is not opened`, {type: r.type});
  }
  const text = utf8Text(r.bytes, {cutEnd: r.cut});
  const base = {path: parts.join('/'), size: r.size, mtime: Math.round(r.mtimeMs)};
  if (text == null) return {status: 200, body: {...base, kind: 'binary', type: 'not valid UTF-8 text'}};
  return {status: 200, body: {...base, kind: 'text', format: formatOf(parts.at(-1)), text, bytes: r.size, shown: Buffer.byteLength(text), truncated: r.cut, limit: LIMIT}};
}

// ---- the files of a run --------------------------------------------------------------------------
// The result line of a step file (`result: valid`), or null: the first `result:` line of a short text file.
function resultOf(abs, runDir) {
  const r = readRegular(abs, {root: runDir, limit: RESULT_BYTES});
  if (r.state !== 'ok') return null;
  const text = utf8Text(r.bytes, {cutEnd: r.cut});
  const m = text == null ? null : /^result:[ \t]*(.*?)[ \t]*$/m.exec(text);
  return m && m[1] ? m[1].slice(0, 120) : null;
}

// The regular files under `roots` (the run's work/ and the work/ of each case), with their size, time, format
// and, for a `.txt` step file, its result. `artifacts` maps a path to the name of the artifact it holds.
export function listFiles(runDir, roots, artifacts = new Map()) {
  const files = [], seen = new Set();
  let hiddenDirectories = 0, hiddenFiles = 0, truncated = false;
  const walk = (dir, depth) => {
    for (const n of listDir(dir, runDir)) {
      if (files.length >= MAX_FILES) { truncated = true; return; }
      const p = path.join(dir, n);
      const st = statOf(p, runDir);
      if (!st) continue;
      if (dotName(n)) { if (st.isDirectory()) hiddenDirectories++; else hiddenFiles++; continue; }
      const real = runRel(p, runDir);
      if (real == null || hasDotPart(real)) continue;
      if (st.isDirectory()) { if (depth < MAX_DEPTH) walk(p, depth + 1); continue; }
      if (!st.isFile()) continue;
      const rel = posix(path.relative(runDir, p));
      if (seen.has(rel)) continue;
      seen.add(rel);
      const format = formatOf(n);
      files.push({path: rel, size: st.size, mtime: Math.round(st.mtimeMs), format, result: /\.txt$/i.test(n) && st.size <= 65536 ? resultOf(p, runDir) : null, artifact: artifacts.get(rel) ?? null});
    }
  };
  for (const r of roots) walk(r, 0);
  files.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  return {files, hiddenDirectories, hiddenFiles, truncated};
}

// ---- the product of a run -------------------------------------------------------------------------
// In this order: the view's `product` (a path relative to the run, checked when the view is loaded); else
// work/<runbook>/<runbook>.md for the runbook's id; else the Markdown files directly under work/ and one level
// below, the newest first. `via` says which rule named it.
const ID = /^[A-Za-z0-9_][A-Za-z0-9._-]*$/;
function regular(runDir, rel) {
  if (hasDotPart(rel)) return false;
  const abs = path.join(runDir, rel);
  const real = runRel(abs, runDir);
  return real != null && !hasDotPart(real) && statOf(abs, runDir)?.isFile() === true;
}
export function productOf({runDir, files, view, ids}) {
  if (typeof view?.product === 'string') {
    const p = view.product.split('/').filter((n) => n !== '' && n !== '.').join('/');
    if (regular(runDir, p)) return {via: 'view', path: p, missing: false, note: 'named by the view'};
    return {via: 'view', path: p, missing: true, note: `the view names ${p}; this run has not written it`};
  }
  for (const id of ids) {
    if (!ID.test(id)) continue;
    const p = `work/${id}/${id}.md`;
    if (regular(runDir, p)) return {via: 'runbook-file', path: p, missing: false, note: `work/<runbook>/<runbook>.md for the runbook ${id}`};
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
export function outputOf({runDir, caseDir, storeDir, caseId, events, seq}) {
  const idx = events.findIndex((e) => e.seq === seq && e.event === 'action_finished');
  if (idx < 0) return null;
  const ev = events[idx];
  const no = (why) => ({recorded: false, why});
  let digest, blobs, stream;
  if (typeof ev.log === 'string' && ev.log) {
    digest = ev.log; blobs = path.join(caseDir, 'blobs'); stream = 'the output of the tool as the case store recorded it';
  } else if (ev.source === 'loom-runlog') {
    const text = readText(path.join(storeDir, `runlog-case-${hex(caseId)}.jsonl`), runDir);
    if (text == null) return no('the run directory holds no Loom run log of this case');
    const lines = [];
    for (const l of text.split('\n')) {
      let r;
      try { r = JSON.parse(l); } catch { continue; }
      if (r?.kind === 'ActionFinished') lines.push(r);
    }
    const k = events.slice(0, idx + 1).filter((e) => e.event === 'action_finished').length - 1;
    if (lines[k]?.action !== ev.action) return no('the Loom run log of the case does not hold this action finish');
    if (!lines[k].blob_digest) return no('the Loom run log names no output for this action (a step that starts no program has none)');
    digest = lines[k].blob_digest; blobs = path.join(storeDir, 'blobs'); stream = OUTPUT_STREAM;
  } else return no('this action finish names no output');
  if (!DIGEST.test(String(digest))) return no('the digest the run log names is not a SHA-256');
  const r = readRegular(path.join(blobs, digest), {root: runDir, limit: TAIL, tail: true});
  if (r.state === 'missing') return no(`the blob ${digest.slice(0, 12)}… the run log names is not in the run`);
  if (r.state === 'refused') return no(`the blob ${digest.slice(0, 12)}… is not a regular file inside the run`);
  let text = utf8Text(r.bytes, {cutStart: r.cut});
  if (text == null) return {recorded: true, digest, stream, bytes: r.size, text: null, truncated: r.cut, note: 'the output is not UTF-8 text'};
  if (r.cut && text.includes('\n')) text = text.slice(text.indexOf('\n') + 1);
  return {recorded: true, digest, stream, bytes: r.size, shown: Buffer.byteLength(text), truncated: r.cut, text};
}
