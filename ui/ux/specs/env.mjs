// One server per viewport project over the adapter's scenarios: played fixture runs (the driver bot)
// in ux/out/fixtures/<viewport>/, recorded runs as they are. The person scenario's store is
// writable (--write-only), so the screen-only user can take the decision on a copy.
//
// The measurement is deterministic (decision D-005): the server, the fixture player and the page
// share one clock (vclock.mjs, page.clock) that only the loop moves, STEP_MS per step from T0; the
// player writes one step when the loop releases it, the server rescans when the player reports the
// step, and the page fetches and paints before anything is sampled. Animations and transitions are
// off in the measured page. Nothing depends on how fast the machine runs or how many workers share it.
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {UI, scenarios, viewports} from '../adapter.mjs';
import {needRun} from '../../test/need-run.mjs';
import {readGate} from '../../vclock.mjs';

export const OUT = path.join(UI, 'ux', 'out');
export const ROUND = process.env.UX_ROUND || null;
export const SHOTS = ROUND ? path.join(UI, 'ux', 'rounds', ROUND, 'shots') : null;
export const STEP_MS = 1400;
export const T0 = Date.parse('2026-11-21T12:00:00Z');
const all = [...viewports.gating, ...viewports.measured];
export const gating = (name) => viewports.gating.some((v) => v.name === name);
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const STILL = '*, *::before, *::after { animation: none !important; transition: none !important; caret-color: transparent !important; }';

async function until(what, ok, ms = 60000) {
  for (const end = Date.now() + ms; Date.now() < end; await wait(10)) if (await ok()) return;
  throw new Error(`timed out waiting for ${what}`);
}

export async function start(viewport) {
  const k = all.findIndex((v) => v.name === viewport);
  const port = Number(process.env.SDC_UI_UX_PORT || 7510) + k;
  const dir = path.join(OUT, 'fixtures', viewport);
  fs.rmSync(dir, {recursive: true, force: true});
  fs.mkdirSync(dir, {recursive: true});
  const gate = path.join(dir, '.clock');
  fs.mkdirSync(gate, {recursive: true});
  const put = (name, v) => fs.writeFileSync(path.join(gate, name), String(v));
  let clockStep = 0;
  put('clock', T0);
  put('go', 0);
  put('at', '');
  const env = {...process.env, SDC_CLOCK_DIR: gate};
  const procs = [];
  const stores = [];
  // recorded runs (runs/ is git-ignored): say which one is missing before anything starts
  for (const s of scenarios) if (s.store) needRun(path.dirname(s.store), `ux scenario ${s.name}`);
  needRun(path.resolve(process.env.SDC_UI_FIXTURE_SOURCE || path.join(UI, '../runs/trial-todo-1')), 'ux played scenarios (test/fixture-run.mjs source)');
  // static scenarios are written before the server starts; played ones start per test
  for (const s of scenarios) {
    if (s.store) { stores.push(s.store); continue; }
    stores.push(path.join(dir, s.name, 'store'));
    if (s.fixture === 'person' || s.fixture === 'changes') await new Promise((r) => spawn('node', ['test/fixture-run.mjs', s.fixture, path.join(dir, s.name)], {cwd: UI, stdio: 'ignore', env}).on('exit', r));
  }
  const person = scenarios.find((s) => s.write);
  const srv = spawn('node', ['server.mjs', '--port', String(port), '--principal', 'ux-bot', '--write-only', path.join(dir, person.name, 'store'), ...stores], {cwd: UI, stdio: 'ignore', env});
  procs.push(srv);
  const base = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 80; i++) { try { await fetch(base + '/api/sources'); break; } catch { await wait(100); } }

  // the server has rescanned what the player reported last
  const rescanned = () => until('the server to rescan', async () => (await (await fetch(base + '/api/clock')).json()).at === readGate(gate, 'at'));
  let player = null, playFrom = 0;
  const play = async (s) => {
    if (!s.fixture || s.fixture === 'person' || s.fixture === 'changes') return null;
    put('go', 0);
    put('at', '');
    playFrom = clockStep;
    const p = spawn('node', ['test/fixture-run.mjs', s.fixture, path.join(dir, s.name), String(s.speed || 3)], {cwd: UI, stdio: 'ignore', env});
    procs.push(p);
    player = new Promise((r) => p.on('exit', r));
    // the store as the scenario starts: the player's first step
    await until('the player to write its first step', () => readGate(gate, 'at') !== '');
    await rescanned();
    // an object, not the promise itself: awaiting play() must not wait for the player to end
    return {done: player};
  };
  const now = () => T0 + clockStep * STEP_MS;
  const syncPage = async (page) => { await rescanned(); await page.evaluate(() => window.__sdcSync?.()); await page.evaluate(() => document.fonts.ready.then(() => true)); };
  return {
    base, dir, play, source: (s) => scenarios.indexOf(s),
    // before the page loads: its clock is the shared one, and nothing in it moves by itself
    prepare: async (page) => {
      await page.clock.setFixedTime(now());
      await page.addInitScript((css) => { document.addEventListener('DOMContentLoaded', () => { const st = document.createElement('style'); st.textContent = css; document.head.appendChild(st); }); }, STILL);
    },
    // the page has fetched and painted what the server has now
    sync: syncPage,
    // one step: the clock moves STEP_MS, the player writes its next step, the page catches up
    step: async (page) => {
      clockStep++;
      put('clock', now());
      await page.clock.setFixedTime(now());
      const at = readGate(gate, 'at');
      if (player && at !== 'done' && at !== '') {
        // the player plays STEP_MS of its own sleeps per step, then waits
        const budget = (clockStep - playFrom) * STEP_MS;
        put('go', budget);
        await until('the player to write its step', () => { const a = readGate(gate, 'at'); return a === 'done' || Number(a) > budget; });
      }
      await syncPage(page);
    },
    // the rest of the play runs through without the loop
    release: () => { put('go', 1e15); player = null; },
    minutes: (steps) => steps * STEP_MS / 60000,
    stop: () => { put('go', 1e15); for (const p of procs) if (p.exitCode == null) p.kill(); },
  };
}
