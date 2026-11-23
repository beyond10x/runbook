// Every read of a file of a run goes through this module. A run directory is written by a confined
// model, so a link inside it must not make the inspector read, list or serve a file outside it:
// `runPath` is the one check, and every other function here calls it. A file is used only when its
// real path (fs.realpathSync) lies under the real path of the run directory.
import fs from 'node:fs';
import path from 'node:path';

const runDirs = new Set();

// a run directory the server was given or found; the owner of a path is the longest one it lies under
export const registerRun = (dir) => { runDirs.add(path.resolve(dir)); };

export const within = (child, root) => child === root || child.startsWith(root + path.sep);

function ownerOf(abs) {
  let best = null;
  for (const d of runDirs) if (within(abs, d) && (!best || d.length > best.length)) best = d;
  return best;
}

// The real path of `p` when it lies under the real path of its run directory (`root` when given, else
// the registered run directory it lies under), else null: a path of no run, a link that leaves the
// run, a file that is not there.
export function runPath(p, root) {
  const abs = path.resolve(p);
  const run = root ? path.resolve(root) : ownerOf(abs);
  if (!run) return null;
  try {
    const real = fs.realpathSync.native(abs);
    return within(real, fs.realpathSync.native(run)) ? real : null;
  } catch { return null; }
}

// true when something is there and the check refuses it: a link that leaves the run
export function leavesRun(p, root) {
  try { fs.lstatSync(p); } catch { return false; }
  return runPath(p, root) === null;
}

// ---- the opened file is the file that was checked ---------------------------------------------------
// `runPath` checks a real path and the open that follows takes that path as a string again, so a directory on the
// way that is swapped for a link between the two is followed (O_NOFOLLOW guards the last component only). After the
// open, the kernel's own name for the descriptor must be the path that was checked: a file that is somewhere else,
// inside the run or not, is not the file the check saw and is not read.
// The name is read from the kernel; where it cannot be read (no /proc) the open is not trusted and the read is refused.
export const fdPath = (fd) => { try { return fs.realpathSync.native(`/proc/self/fd/${fd}`); } catch { return null; } };

// {fd, real} for an open descriptor of `real` whose own name is `real`, or {why: 'leaves the run' | 'changed' |
// 'unverified'}. `real` of the answer is the descriptor's name, the one name every later check is made on.
function openChecked(real, runReal, flags, nameOf = fdPath) {
  const fd = fs.openSync(real, flags);
  const now = nameOf(fd);
  if (now === real) return {fd, real: now};
  fs.closeSync(fd);
  if (now == null) return {why: 'unverified'};
  return {why: !within(now, runReal) ? 'leaves the run' : 'changed'};
}
const runRealOf = (abs, root) => {
  const run = root ? path.resolve(root) : ownerOf(abs);
  if (!run) return null;
  try { return fs.realpathSync.native(run); } catch { return null; }
};

// A regular file with more than one name is refused as in readRegular (a hard link may be a file outside the run, and only
// its link count tells), for every caller: the answer is null, as for an absent file.
// The text of a regular file of a run, or null (absent, refused, or over `limit` bytes: a file over it is not read at
// all, so no request can make the server hold more than that). The limit is the size of the largest file of its kind that
// the page reads; the default is the most any file of a run it reads can be.
export const TEXT_LIMIT = 64 * 1048576;
export function readText(p, root, limit = TEXT_LIMIT) {
  const real = runPath(p, root);
  if (!real) return null;
  let fd = null;
  try {
    const o = openChecked(real, runRealOf(path.resolve(p), root), fs.constants.O_RDONLY | fs.constants.O_NONBLOCK);
    if (o.fd == null) return null;
    fd = o.fd;
    const st = fs.fstatSync(fd);
    if (!st.isFile() || st.nlink > 1 || st.size > limit) return null;
    return fs.readFileSync(fd, 'utf8');
  } catch { return null; } finally { if (fd != null) fs.closeSync(fd); }
}
export const readJson = (p, fallback = null) => {
  const text = readText(p);
  if (text == null) return fallback;
  try { return JSON.parse(text) ?? fallback; } catch { return fallback; }
};
export function statOf(p, root) {
  const real = runPath(p, root);
  if (!real) return null;
  try { return fs.statSync(real); } catch { return null; }
}
export const exists = (p, root) => runPath(p, root) !== null;
export const statSig = (p) => { const s = statOf(p); return s ? `${s.size}:${s.mtimeMs}` : '-'; };
// the real path of `p` relative to the real path of its run directory, or null when the check refuses it
export function runRel(p, root) {
  const real = runPath(p, root);
  if (!real) return null;
  const run = root ? path.resolve(root) : ownerOf(path.resolve(p));
  try { return path.relative(fs.realpathSync.native(run), real); } catch { return null; }
}

// ---- the content of a file of a run, for the page that shows it ----------------------------------------
// A name that starts with a dot is a tool's cache or login, never listed and never served. This is the one test.
export const dotName = (n) => n.startsWith('.');
export const hasDotPart = (rel) => String(rel).split(/[\\/]+/).some((n) => n !== '' && dotName(n));

const TYPES = [['isDirectory', 'directory'], ['isFIFO', 'named pipe'], ['isSocket', 'socket'], ['isBlockDevice', 'block device'], ['isCharacterDevice', 'character device'], ['isSymbolicLink', 'symbolic link']];
export const typeOf = (st) => TYPES.find(([m]) => st[m]())?.[1] ?? (st.isFile() ? 'file' : 'unknown');

// The bytes of a regular file of a run, at most `limit` of them (from the start, or the last `limit` with
// `tail`). The type is read with a stat of the checked real path BEFORE anything opens the file: a directory, a
// named pipe, a socket or a device is refused and never opened, so a pipe cannot block the server. The file is
// then opened without blocking and its type read again from the descriptor, so a file swapped for a pipe between
// the two is refused too. Answers {state: 'ok', size, mtimeMs, bytes, cut} | {state: 'missing'} |
// {state: 'refused', why, type?}.
// The window of the file that is read: its first `limit` bytes, its last `limit` with `tail`, or the `limit` bytes that
// end at `end` (a position in the file). `from` and `to` say which bytes `bytes` are.
// After the open the descriptor must still be the checked file (see openChecked), and a regular file with more than
// one link is refused: a hard link to a file outside the run looks like a file of the run by its path, only its
// link count tells.
// `accept(real)` is asked once, with the descriptor's own real path, after the open and before a byte is read: a string
// is the reason the file is not served (answer {state: 'refused', why: 'not served', reason}), null lets it through.
// It is the one place a caller says where a file may lie, because it is asked about the file that is open, not about
// a path computed before. `nameOf` is the kernel's name for a descriptor (a test supplies one that cannot answer).
// `at` starts the window at a position of the file (the `limit` bytes from there): an incremental read, or one record by its
// offset, on the same checked descriptor.
export function readRegular(p, {root, limit = 1048576, tail = false, end = null, at = null, accept = null, nameOf = fdPath} = {}) {
  const abs = path.resolve(p);
  const real = runPath(abs, root);
  if (!real) {
    try { fs.lstatSync(abs); } catch { return {state: 'missing'}; }
    return {state: 'refused', why: 'leaves the run'};
  }
  let st;
  try { st = fs.statSync(real); } catch { return {state: 'missing'}; }
  if (!st.isFile()) return {state: 'refused', why: 'not a regular file', type: typeOf(st)};
  let fd = null;
  try {
    const o = openChecked(real, runRealOf(abs, root), fs.constants.O_RDONLY | fs.constants.O_NONBLOCK | fs.constants.O_NOFOLLOW, nameOf);
    if (o.fd == null) return o.why === 'leaves the run' || o.why === 'unverified' ? {state: 'refused', why: o.why} : {state: 'missing'};
    fd = o.fd;
    const reason = accept ? accept(o.real) : null;
    if (reason) return {state: 'refused', why: 'not served', reason};
    const now = fs.fstatSync(fd);
    if (!now.isFile()) return {state: 'refused', why: 'not a regular file', type: typeOf(now)};
    if (now.nlink > 1) return {state: 'refused', why: 'more than one link', type: 'file'};
    const begin = at != null ? Math.min(at, now.size) : 0;
    const to = end != null ? Math.min(end, now.size) : tail ? now.size : Math.min(now.size, begin + limit);
    const from = end != null || tail ? Math.max(0, to - limit) : begin;
    const want = to - from;
    const buf = Buffer.alloc(want);
    let got = 0;
    while (got < want) {
      const n = fs.readSync(fd, buf, got, want - got, from + got);
      if (n === 0) break;
      got += n;
    }
    return {state: 'ok', size: now.size, mtimeMs: now.mtimeMs, bytes: buf.subarray(0, got), cut: from > 0 || from + got < now.size, from, to: from + got};
  } catch { return {state: 'missing'}; } finally { if (fd != null) fs.closeSync(fd); }
}

// true when `real` (a real path) lies under one of `roots`, directories of the run, by their real paths
export function underAny(real, roots, runDir) {
  return roots.some((r) => { const rr = runPath(r, runDir); return rr != null && within(real, rr); });
}

// the names in a directory of a run; an entry whose real path leaves the run is not listed
export function listDir(p, root) {
  const real = runPath(p, root);
  if (!real) return [];
  try { return fs.readdirSync(real).sort().filter((n) => runPath(path.join(p, n), root) !== null); } catch { return []; }
}
