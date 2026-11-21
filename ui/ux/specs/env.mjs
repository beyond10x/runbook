// One server per viewport project over the adapter's scenarios: played fixture runs (the driver bot)
// in ux/out/fixtures/<viewport>/, recorded runs as they are. The person scenario's store is
// writable (--write-only), so the screen-only user can take the decision on a copy.
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {UI, scenarios, viewports} from '../adapter.mjs';
import {needRun} from '../../test/need-run.mjs';

export const OUT = path.join(UI, 'ux', 'out');
export const ROUND = process.env.UX_ROUND || null;
export const SHOTS = ROUND ? path.join(UI, 'ux', 'rounds', ROUND, 'shots') : null;
const all = [...viewports.gating, ...viewports.measured];
export const gating = (name) => viewports.gating.some((v) => v.name === name);

// slot: 'loop' (ports 7510 on) or 'phone' (the phone contract, 60 ports higher, its own fixture dir),
// so the two spec files can run at the same viewport at once
export async function start(viewport, slot = 'loop') {
  const k = all.findIndex((v) => v.name === viewport);
  const port = Number(process.env.SDC_UI_UX_PORT || 7510) + k + (slot === 'phone' ? 60 : 0);
  const dir = path.join(OUT, 'fixtures', slot === 'phone' ? `phone-${viewport}` : viewport);
  fs.rmSync(dir, {recursive: true, force: true});
  fs.mkdirSync(dir, {recursive: true});
  const procs = [];
  const stores = [];
  // recorded runs (runs/ is git-ignored): say which one is missing before anything starts
  for (const s of scenarios) if (s.store) needRun(path.dirname(s.store), `ux scenario ${s.name}`);
  needRun(path.resolve(process.env.SDC_UI_FIXTURE_SOURCE || path.join(UI, '../runs/trial-todo-1')), 'ux played scenarios (test/fixture-run.mjs source)');
  // static scenarios are written before the server starts; played ones start per test
  for (const s of scenarios) {
    if (s.store) { stores.push(s.store); continue; }
    stores.push(path.join(dir, s.name, 'store'));
    if (s.fixture === 'person' || s.fixture === 'changes') await new Promise((r) => spawn('node', ['test/fixture-run.mjs', s.fixture, path.join(dir, s.name)], {cwd: UI, stdio: 'ignore'}).on('exit', r));
  }
  const person = scenarios.find((s) => s.write);
  const srv = spawn('node', ['server.mjs', '--port', String(port), '--principal', 'ux-bot', '--write-only', path.join(dir, person.name, 'store'), ...stores], {cwd: UI, stdio: 'ignore'});
  procs.push(srv);
  const base = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 80; i++) { try { await fetch(base + '/api/sources'); break; } catch { await new Promise((r) => setTimeout(r, 100)); } }
  const play = (s) => {
    if (!s.fixture || s.fixture === 'person' || s.fixture === 'changes') return null;
    const p = spawn('node', ['test/fixture-run.mjs', s.fixture, path.join(dir, s.name), String(s.speed || 3)], {cwd: UI, stdio: 'ignore'});
    procs.push(p);
    return new Promise((r) => p.on('exit', r));
  };
  return {base, dir, play, source: (s) => scenarios.indexOf(s), stop: () => { for (const p of procs) if (p.exitCode == null) p.kill(); }};
}
