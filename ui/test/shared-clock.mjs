// A shared clock for one spec (vclock.mjs): the server and the fixture player read the time from
// <gate>/clock, the page gets the same instant (page.clock), and only the spec moves it, STEP_MS per
// step. Each step releases STEP_MS of the player's sleeps, waits until the player has written them and
// the server has rescanned, then lets the page fetch and paint. What the page shows follows the
// steps, not the machine's speed. ux/specs/env.mjs does the same for the ux loop.
import fs from 'node:fs';
import path from 'node:path';
import {readGate} from '../vclock.mjs';

export const STEP_MS = 1400;
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(what, ok, ms = 60000) {
  for (const end = Date.now() + ms; Date.now() < end; await wait(10)) if (await ok()) return;
  throw new Error(`timed out waiting for ${what}`);
}

export function sharedClock(gate, t0 = Date.parse('2026-11-21T12:00:00Z')) {
  fs.rmSync(gate, {recursive: true, force: true});
  fs.mkdirSync(gate, {recursive: true});
  const put = (name, v) => { const tmp = path.join(gate, `.${name}`); fs.writeFileSync(tmp, String(v)); fs.renameSync(tmp, path.join(gate, name)); };
  put('clock', t0);
  put('go', 0);
  put('at', '');
  let n = 0, playFrom = 0, playing = false, base = null;
  const now = () => t0 + n * STEP_MS;
  const rescanned = () => until('the server to rescan', async () => (await (await fetch(base + '/api/clock')).json()).at === readGate(gate, 'at'));
  const sync = async (page) => { await rescanned(); await page.evaluate(() => window.__sdcSync?.()); };
  const step = async (page) => {
    n++;
    put('clock', now());
    await page.clock.setFixedTime(now());
    const at = readGate(gate, 'at');
    if (playing && at !== 'done' && at !== '') {
      const budget = (n - playFrom) * STEP_MS;
      put('go', budget);
      await until('the player to write its step', () => { const a = readGate(gate, 'at'); return a === 'done' || Number(a) > budget; });
    }
    await sync(page);
  };
  return {
    // the environment of the server and the player
    env: {...process.env, SDC_CLOCK_DIR: gate},
    // the server that reads this clock, once it answers
    serve: async (url) => { base = url; await until('the server', () => fetch(url + '/api/clock').then(() => true, () => false)); },
    // before the page loads
    prepare: (page) => page.clock.setFixedTime(now()),
    // a player started with env: its first step is on disk and the server has it
    play: async (start) => {
      put('go', 0);
      put('at', '');
      playFrom = n;
      playing = true;
      const p = start();
      await until('the player to write its first step', () => readGate(gate, 'at') !== '');
      await rescanned();
      return p;
    },
    sync, step,
    // step until ok() holds, at most max steps
    stepUntil: async (page, what, ok, max = 500) => {
      for (let i = 0; i < max; i++) { if (await ok()) return; await step(page); }
      throw new Error(`${what}: not after ${max} steps of ${STEP_MS} ms`);
    },
  };
}
