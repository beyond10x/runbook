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

// A server this loop started, once it answers (as a clock-mode server unless clock is false): its
// address comes from its own first line (it takes a free port, so two loops on one machine never
// share one, and a request never reaches another loop's server). If it exits or does not answer
// within a minute, the error says which, with its stderr. srv: spawned with stdout and stderr piped.
export async function serverUp(srv, viewport, {clock = true, ms = 60000} = {}) {
  let out = '', err = '', exit = null;
  srv.stdout.on('data', (d) => { out += d; });
  srv.stderr.on('data', (d) => { err += d; });
  srv.on('exit', (code, signal) => { exit = signal || `exit code ${code}`; });
  srv.stderrTail = () => err.trim().split('\n').slice(-20).join('\n') || '(empty)';
  const fail = (state) => { srv.kill(); return new Error(`ux server for ${viewport} is not up: ${state}\n--- its stderr ---\n${srv.stderrTail()}`); };
  let base = null;
  for (const end = Date.now() + ms; Date.now() < end; await wait(25)) {
    if (exit) throw fail(`it ended (${exit}) before answering`);
    base ||= /sdc-ui on (http:\/\/[^/\s]+)\//.exec(out)?.[1] ?? null;
    if (!base) continue;
    const health = await fetch(base + (clock ? '/api/clock' : '/api/sources')).then((r) => r.json(), () => null);
    if (health && (!clock || health.clock === true)) return base;
    if (health) throw fail(`${base} answers but not in clock mode (SDC_CLOCK_DIR not seen)`);
  }
  throw fail(base ? `${base} named, but no answer within ${ms / 1000} s` : `no listening line within ${ms / 1000} s; stdout: ${out.trim() || '(empty)'}`);
}

// slot: 'loop' or 'phone' (the phone contract, with a fixture dir of its own), so the two spec files
// can run at the same viewport at once. A free port by default; SDC_UI_UX_PORT pins them (its value
// plus the viewport's index, 60 more for the phone slot).
export async function start(viewport, slot = 'loop') {
  const k = all.findIndex((v) => v.name === viewport);
  const port = process.env.SDC_UI_UX_PORT ? Number(process.env.SDC_UI_UX_PORT) + k + (slot === 'phone' ? 60 : 0) : 0;
  const dir = path.join(OUT, 'fixtures', slot === 'phone' ? `phone-${viewport}` : viewport);
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
  const srv = spawn('node', ['server.mjs', '--port', String(port), '--principal', 'ux-bot', '--write-only', path.join(dir, person.name, 'store'), ...stores], {cwd: UI, stdio: ['ignore', 'pipe', 'pipe'], env});
  procs.push(srv);
  const base = await serverUp(srv, viewport);

  // the server has rescanned what the player reported last
  const rescanned = () => until('the server to rescan', async () => {
    if (srv.exitCode != null || srv.signalCode != null) throw new Error(`ux server for ${viewport} ended (${srv.signalCode || `exit code ${srv.exitCode}`}) during the run\n--- its stderr ---\n${srv.stderrTail()}`);
    return (await (await fetch(base + '/api/clock')).json()).at === readGate(gate, 'at');
  });
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
      // UX_SHOWN_PID: the page shows this pid for every process, to measure a pid width the
      // machine did not happen to hand out (the run's own pids have six or seven digits)
      const pid = process.env.UX_SHOWN_PID;
      if (pid) await page.route(/\/api\/live\b/, async (r) => { const res = await r.fetch(); await r.fulfill({response: res, body: (await res.text()).replace(/"pid":\d+/g, `"pid":${Number(pid)}`)}); });
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
