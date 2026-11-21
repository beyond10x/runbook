#!/usr/bin/env node
// The ratchet (SKILL § 6): every measured value against ux/baseline.json, all lower is better.
//   node ux/core/ratchet.mjs [--out DIR] [--baseline FILE] [--update] [--decision ID] [--viewports a,b]
// Exit 1 on: a value worse than the baseline beyond its tolerance; an improvement not taken into
// the baseline (run with --update in the same change); a missing value; a baseline made worse than
// the committed one without --decision. --update writes improved and new values (worse ones only
// with --decision, which is recorded on them). Generic: no project code.
import fs from 'node:fs';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import {fold} from './metrics.mjs';

const args = process.argv.slice(2);
const opt = (k, d) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : d; };
const out = path.resolve(opt('--out', 'ux/out'));
const file = path.resolve(opt('--baseline', 'ux/baseline.json'));
const update = args.includes('--update');
const decision = opt('--decision', null);
const only = opt('--viewports', null)?.split(',');
const base = JSON.parse(fs.readFileSync(file, 'utf8'));
const tol = base.tolerances || {counts: 0, steps: 0.05, layout_shift: 0.01};
const tolOf = (key, b) => b?.tolerance ?? (key.startsWith('layout_shift') ? tol.layout_shift : /time_to_act|glance|reading/.test(key) ? tol.steps : tol.counts);
const worse = (key, v, b) => { const t = tolOf(key, b); return /time_to_act|glance|reading/.test(key) ? v > b.value * (1 + t) + 1e-9 : v > b.value + t + 1e-9; };
const better = (key, v, b) => { const t = tolOf(key, b); return /time_to_act|glance|reading/.test(key) ? v < b.value * (1 - t) - 1e-9 : v < b.value - t - 1e-9; };
const now = fold(out);
const lines = [];
let fail = false;
const inScope = (k) => !only || only.some((v) => k.endsWith('@' + v));

// the committed baseline, when the repository has one
let committed = null;
try { committed = JSON.parse(execFileSync('git', ['show', `HEAD:${path.relative(execFileSync('git', ['rev-parse', '--show-toplevel'], {encoding: 'utf8'}).trim(), file)}`], {encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore']})); } catch { committed = null; }

for (const [sc, keys] of Object.entries(base.scenarios || {})) {
  for (const [k, b] of Object.entries(keys)) {
    if (!inScope(k)) continue;
    const v = now[sc]?.[k];
    if (v == null) { lines.push(`MISSING  ${sc} ${k}`); fail = true; continue; }
    if (worse(k, v, b)) {
      if (update && decision) { keys[k] = {...b, value: v, decision}; lines.push(`ACCEPTED ${sc} ${k} ${b.value} → ${v} (decision ${decision})`); continue; }
      lines.push(`WORSE    ${sc} ${k} ${b.value} → ${v}`); fail = true; continue;
    }
    // an improvement is a value below the baseline beyond the same tolerance band: a sampled metric
    // that moves within its noise is the same value both ways (a taken low sample would fail the next)
    if (better(k, v, b)) {
      if (update) { keys[k] = {...b, value: v}; lines.push(`TAKEN    ${sc} ${k} ${b.value} → ${v}`); }
      else { lines.push(`IMPROVED ${sc} ${k} ${b.value} → ${v}: not in the baseline (run with --update)`); fail = true; }
      continue;
    }
    lines.push(`same     ${sc} ${k} ${v}`);
  }
}
for (const [sc, keys] of Object.entries(now)) for (const [k, v] of Object.entries(keys)) {
  if (!inScope(k) || base.scenarios?.[sc]?.[k]) continue;
  if (update) { ((base.scenarios ||= {})[sc] ||= {})[k] = {value: v}; lines.push(`NEW      ${sc} ${k} ${v}`); }
  else { lines.push(`NEW      ${sc} ${k} ${v}: not in the baseline (run with --update)`); fail = true; }
}
if (committed) {
  for (const [sc, keys] of Object.entries(base.scenarios || {})) for (const [k, b] of Object.entries(keys)) {
    const c = committed.scenarios?.[sc]?.[k];
    if (c && worse(k, b.value, c) && !b.decision) { lines.push(`BASELINE WORSE than HEAD without a decision: ${sc} ${k} ${c.value} → ${b.value}`); fail = true; }
  }
}
if (update) { fs.writeFileSync(file, JSON.stringify(base, null, 2) + '\n'); if (!lines.some((l) => /^(WORSE|MISSING|BASELINE)/.test(l))) fail = false; }
console.log(lines.filter((l) => !l.startsWith('same')).join('\n') || 'every value as in the baseline');
console.log(`${lines.filter((l) => l.startsWith('same')).length} same, ${lines.filter((l) => !l.startsWith('same')).length} other`);
process.exit(fail ? 1 : 0);
