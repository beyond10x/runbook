// Every place a change of the tree can land while a synchronous reader of the inspector runs, tried one after another.
// Not a spec. A race between the reader and a process that swaps a directory or a file in a loop hits a window only by
// chance, and for as long as it runs; here the change is made in this process, synchronously, right before the n-th call
// the reader makes to the file system, for every n (and every set of up to `depth` such places), so each window between
// two calls of the reader is tried, every run, in a few milliseconds.
// The reader's calls are counted on the module object of node:fs (the inspector's modules call `fs.<name>` on it), and on
// child_process.execFileSync (the parse of a YAML text): `places()` counts them for one call of the reader, `before(set,
// change, read, seen)` runs the reader with `change()` made before each call whose number is in the set. A change is made
// with the counting off, so its own calls are not places.
import fs from 'node:fs';
import cp from 'node:child_process';
import {syncBuiltinESMExports} from 'node:module';

const FS = ['statSync', 'lstatSync', 'openSync', 'fstatSync', 'readSync', 'closeSync', 'readdirSync', 'existsSync', 'readFileSync'];

// run `read()` with every counted call numbered; before call number n, `at(n, name)` is called first
function counted(at, read) {
  const saved = FS.map((k) => [k, fs[k]]);
  const native = fs.realpathSync.native;
  const exec = cp.execFileSync;
  let n = 0, off = false;
  const wrap = (name, f) => function (...a) { if (!off) { n++; off = true; try { at(n, name); } finally { off = false; } } return f.apply(this, a); };
  for (const [k, f] of saved) fs[k] = wrap(k, f);
  fs.realpathSync.native = wrap('realpathSync.native', native);
  cp.execFileSync = wrap('execFileSync', exec);
  syncBuiltinESMExports();
  try { return {value: read(), calls: n}; } finally {
    for (const [k, f] of saved) fs[k] = f;
    fs.realpathSync.native = native;
    cp.execFileSync = exec;
    syncBuiltinESMExports();
  }
}

// the calls one run of `read()` makes, by name in order
export function places(read) {
  const names = [];
  const {value} = counted((_, name) => names.push(name), read);
  return {names, value};
}

// `read()` with `change()` made right before every call whose number is in `set`; `seen(name)` is told of each call after
// the change before it
export const before = (set, change, read, seen = () => {}) => counted((n, name) => { if (set.has(n)) change(); seen(name); }, read).value;

// every set of 1 to `depth` numbers out of 1..n
export function* sets(n, depth) {
  function* from(start, left, acc) {
    if (acc.length) yield new Set(acc);
    if (!left) return;
    for (let i = start; i <= n; i++) yield* from(i + 1, left - 1, [...acc, i]);
  }
  yield* from(1, depth, []);
}
