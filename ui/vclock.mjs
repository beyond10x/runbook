// A clock the measured run shares (the ux loop, ux/specs/env.mjs): with SDC_CLOCK_DIR set, the server
// and the fixture player read the time from <dir>/clock (epoch ms) instead of the wall clock, and
// the page gets the same instant (Playwright's page.clock). The loop moves it one fixed step at a
// time, so every time the page shows depends on the step, not on how fast the machine is.
// <dir>/go and <dir>/at gate the player: go is the play time (the sum of its sleeps, in ms) it may
// reach; past it, the player reports its play time in at and waits. The server rescans the stores
// when at changes.
import fs from 'node:fs';
import path from 'node:path';

export const RealDate = Date;

export function installClock(dir) {
  const file = path.join(dir, 'clock');
  let seen = -1, value = RealDate.now();
  const now = () => {
    try {
      const st = fs.statSync(file);
      if (st.mtimeMs !== seen) { seen = st.mtimeMs; value = Number(fs.readFileSync(file, 'utf8')) || value; }
    } catch { /* no clock yet: the last value */ }
    return value;
  };
  class ClockDate extends RealDate {
    constructor(...a) { if (a.length) super(...a); else super(now()); }
    static now() { return now(); }
  }
  globalThis.Date = ClockDate;
  return now;
}

export const readGate = (dir, name) => { try { return fs.readFileSync(path.join(dir, name), 'utf8').trim(); } catch { return ''; } };
