// L0 engine scenario, hard gate 6 (cue truth): the recorded trial-todo-1 run replayed one event at a
// time through the Now band's derivation (web/now.js), from the files alone. The cue is the band's
// waiting state: which case waits, on which action, and whether a person or a decider decides. The
// truth comes from the events directly: a case whose latest event (after evaluations and revisions)
// is awaiting_person or awaiting_authority waits on each action of its `next`.
// The cue is never ahead of the truth, never flip-flops, names every waited-on action, and
// cue_accuracy (frames where cue = truth) is at least 95 %. Runs once (first gating viewport).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {RUNS, viewports} from '../adapter.mjs';
import {nowState} from '../../web/now.js';
import {record} from '../core/metrics.mjs';
import {OUT} from './env.mjs';

const read = (p, d = null) => { try { return JSON.parse(fs.readFileSync(p, 'utf8')); } catch { return d; } };
function load(root) {
  const out = [];
  const walk = (dir, parent, depth) => {
    const c = read(path.join(dir, 'case.json'), {});
    const events = fs.readFileSync(path.join(dir, 'events.jsonl'), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));
    out.push({id: c.id, rel: path.relative(root, dir), dir, parent, depth, tree: '.', case: c, ir: read(path.join(dir, 'ir.json')), bindings: read(path.join(dir, 'bindings.json'), {}), events, times: {}, evidence: {}, evaluations: {}});
    const kids = path.join(dir, 'children');
    if (fs.existsSync(kids)) for (const n of fs.readdirSync(kids).sort()) if (fs.existsSync(path.join(kids, n, 'case.json'))) walk(path.join(kids, n), c.id, depth + 1);
  };
  walk(root, null, 0);
  return out;
}
const truthOf = (cases) => {
  const out = new Set();
  for (const c of cases) {
    const last = [...c.events].reverse().find((e) => e.event !== 'evaluated' && e.event !== 'revised');
    if (last && (last.event === 'awaiting_person' || last.event === 'awaiting_authority')) for (const n of last.next || [{action: null}]) out.add(`${c.id}:${n.action}`);
  }
  return out;
};

test('cue truth over the recorded trial-todo-1 run, event by event', async ({}, info) => {
  test.skip(info.project.name !== viewports.gating[0].name, 'pure, runs once');
  const full = load(path.join(RUNS, 'trial-todo-1/store/IDEA-1'));
  // one global order: the run's --at, then tree depth, then seq
  const order = full.flatMap((c) => c.events.map((e) => ({c, e}))).sort((a, b) => a.e.at.localeCompare(b.e.at) || a.c.depth - b.c.depth || a.e.seq - b.e.seq);
  const shown = new Map(full.map((c) => [c, []]));
  let frames = 0, right = 0, ahead = 0, missing = 0, flips = 0;
  const hist = [];
  for (const {c, e} of order) {
    shown.get(c).push(e);
    const cases = full.filter((x) => shown.get(x).length).map((x) => ({...x, events: shown.get(x)}));
    const st = nowState(cases, null, null, Date.now());
    const cueSet = new Set(st.waiting.map((w) => `${w.c.id}:${w.action}`));
    const truth = truthOf(cases);
    frames++;
    const extra = [...cueSet].filter((k) => !truth.has(k)).length, miss = [...truth].filter((k) => !cueSet.has(k)).length;
    ahead += extra; missing += miss;
    if (!extra && !miss) right++;
    const key = [...cueSet].sort().join('|');
    if (hist.length >= 2 && key === hist[hist.length - 2] && key !== hist[hist.length - 1]) {
      // A→B→A is a flip-flop only when the truth did not do the same
      const t = [...truth].sort().join('|');
      if (t !== key) flips++;
    }
    hist.push(key);
  }
  const accuracy = right / frames;
  record(OUT, info.project.name, 'engine', 'cue.ahead', ahead);
  record(OUT, info.project.name, 'engine', 'cue.missing', missing);
  record(OUT, info.project.name, 'engine', 'cue.flip_flops', flips);
  record(OUT, info.project.name, 'engine', 'cue.inaccuracy_pct', 100 * (1 - accuracy));
  expect(ahead, 'the cue is never ahead of the truth').toBe(0);
  expect(flips, 'the cue never flip-flops').toBe(0);
  expect(missing, 'the cue names every waited-on action').toBe(0);
  expect(accuracy).toBeGreaterThanOrEqual(0.95);
});
