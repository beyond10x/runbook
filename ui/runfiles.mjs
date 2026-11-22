// Every read of a file of a run goes through this module. A run directory is written by a confined
// model, so a link inside it must not make the inspector read, list or serve a file outside it:
// `runPath` is the one check, and every other function here calls it. A file is used only when its
// real path (fs.realpathSync) lies under the real path of the run directory.
import fs from 'node:fs';
import path from 'node:path';

const runDirs = new Set();

// a run directory the server was given or found; the owner of a path is the longest one it lies under
export const registerRun = (dir) => { runDirs.add(path.resolve(dir)); };

const within = (child, root) => child === root || child.startsWith(root + path.sep);

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

export function readText(p, root) {
  const real = runPath(p, root);
  if (!real) return null;
  try { return fs.readFileSync(real, 'utf8'); } catch { return null; }
}
export const readJson = (p, fallback = null) => {
  const text = readText(p);
  if (text == null) return fallback;
  try { return JSON.parse(text); } catch { return fallback; }
};
export function statOf(p, root) {
  const real = runPath(p, root);
  if (!real) return null;
  try { return fs.statSync(real); } catch { return null; }
}
export const exists = (p, root) => runPath(p, root) !== null;
export const statSig = (p) => { const s = statOf(p); return s ? `${s.size}:${s.mtimeMs}` : '-'; };
// the names in a directory of a run; an entry whose real path leaves the run is not listed
export function listDir(p, root) {
  const real = runPath(p, root);
  if (!real) return [];
  try { return fs.readdirSync(real).sort().filter((n) => runPath(path.join(p, n), root) !== null); } catch { return []; }
}
