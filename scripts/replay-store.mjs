#!/usr/bin/env node
// Replay a recorded case store into a new directory, file by file in the order the run wrote them,
// so a live viewer sees the run happen again.
// Usage: node scripts/replay-store.mjs <recorded-store> <target-store> [delay-ms]
import fs from 'node:fs';
import path from 'node:path';

const [src, dst, delayArg] = process.argv.slice(2);
if (!src || !dst) {
  console.error('usage: replay-store.mjs <recorded-store> <target-store> [delay-ms]');
  process.exit(2);
}
if (fs.existsSync(dst) && fs.readdirSync(dst).length) {
  console.error(`refused: ${dst} is not empty`);
  process.exit(2);
}
const delay = Number(delayArg || 400);

const files = [];
(function walk(dir) {
  for (const e of fs.readdirSync(dir, {withFileTypes: true})) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p);
    else if (e.isFile()) files.push({p, t: fs.statSync(p).mtimeMs});
  }
})(src);
files.sort((a, b) => a.t - b.t || a.p.localeCompare(b.p));

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let n = 0;
for (const f of files) {
  const rel = path.relative(src, f.p);
  const out = path.join(dst, rel);
  fs.mkdirSync(path.dirname(out), {recursive: true});
  fs.copyFileSync(f.p, out);
  n++;
  if (/events|evidence|evaluations|case\.json/.test(rel)) {
    console.log(`${n}/${files.length} ${rel}`);
    await sleep(delay);
  }
}
console.log(`done: ${n} files`);
