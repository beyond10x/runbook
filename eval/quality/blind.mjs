// Copies each product's implementation to products/p1..p9 in a shuffled order; writes key.json.
// Usage: node blind.mjs <runs dir> <quality dir>
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';

const [runs, q] = process.argv.slice(2);
const sources = [];
for (const k of ['strong', 'mixed']) for (const i of [1, 2, 3])
  sources.push({run: `trial-0.1.0-${k}-${i}`, tree: path.join(runs, `trial-0.1.0-${k}-${i}`, 'store/IDEA-1/work/published')});
for (const i of [1, 2, 3])
  sources.push({run: `free-agent-0.1.0-sonnet-${i}`, tree: path.join(runs, `free-agent-0.1.0-sonnet-${i}`, 'work/impl')});

const skip = new Set(['PROVENANCE', '.derived-from', 'target', '.git', '.gitignore']);
function copy(src, dst) {
  fs.mkdirSync(dst, {recursive: true});
  for (const e of fs.readdirSync(src, {withFileTypes: true})) {
    if (skip.has(e.name)) continue;
    const s = path.join(src, e.name), d = path.join(dst, e.name);
    if (e.isDirectory()) copy(s, d);
    else if (e.isFile()) fs.copyFileSync(s, d);
  }
}
// Fisher-Yates with crypto randomness
const order = sources.map((_, i) => i);
for (let i = order.length - 1; i > 0; i--) {
  const j = crypto.randomInt(i + 1);
  [order[i], order[j]] = [order[j], order[i]];
}
const out = path.join(q, 'products');
if (fs.existsSync(out)) throw new Error('products/ exists already');
const key = {};
order.forEach((si, n) => {
  const id = `p${n + 1}`;
  if (!fs.existsSync(sources[si].tree)) throw new Error('missing ' + sources[si].tree);
  copy(sources[si].tree, path.join(out, id));
  key[id] = sources[si].run;
});
fs.writeFileSync(path.join(q, 'key.json'), JSON.stringify(key, null, 2) + '\n');
console.log('copied', Object.keys(key).length, 'products');
