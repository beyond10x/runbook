// The page's own time, moved by the test (Playwright's page.clock), for a case that proves what a page does or does not do
// over a stretch of its time (how often it asks a daemon that refuses it, or that it sends nothing again by itself), or
// that moves the page's time past a wait of its own (page.clock.fastForward).
// `installPageTime(page)` goes before the page loads, so that every timer of the page is on that clock; the clock keeps
// pace with the host's until `spendPageTime` stops it. `spendPageTime(page, ms)` spends `ms` of the page's time in steps
// of STEP_MS: each step fires the timers of the page that fall due in it, then waits until the test has been told of
// every fetch the page asked for so far and every request it made has ended. An answer so reaches the page at the end of
// the step its request was asked in: a read chained on an answer (fetch, then a timer, then fetch again) starts up to
// STEP_MS later than on the host's clock with an answer that takes no time, so a count of such reads over a stretch can
// come out lower by one in about every (timer / STEP_MS + 1) reads; a read that a page's interval asks for falls into the
// stretch as it does on the host's clock, and a loop that asks again with no timer at all still asks while the steps run.
// The stretch costs what the page does in it, not its length; the clock then keeps pace with the host's again.
import {expect} from '@playwright/test';

export const STEP_MS = 50;
// how far ahead of the page's time the stretch starts: the host's clock moves on while the call that stops the page's travels
const AHEAD_MS = 100;

// Playwright's clock replaces `performance` with an object whose entry lists are empty; the page reads its navigation entry
// to know that the daemon served it (web/source.js, `daemon`). The entry lists of the page's own `performance` are kept: a
// script before the clock's takes them, one after it gives them back. The first script also counts each fetch the page asks
// for when it asks (`__pageTimeAsked`): Playwright tells the test of a request a moment after the page asked for it.
export async function installPageTime(page) {
  await page.addInitScript(() => {
    const real = performance;
    window.__pageTimeEntries = {getEntries: real.getEntries.bind(real), getEntriesByName: real.getEntriesByName.bind(real), getEntriesByType: real.getEntriesByType.bind(real)};
    const fetch = window.fetch;
    window.__pageTimeAsked = 0;
    window.fetch = function (...args) { window.__pageTimeAsked += 1; return fetch.apply(this, args); };
  });
  await page.clock.install();
  await page.addInitScript(() => Object.assign(performance, window.__pageTimeEntries));
}

export async function spendPageTime(page, ms) {
  const open = new Set();
  let told = 0;
  const started = (r) => { told += 1; open.add(r); };
  const ended = (r) => open.delete(r);
  page.on('request', started);
  page.on('requestfinished', ended);
  page.on('requestfailed', ended);
  const asked = () => page.evaluate(() => window.__pageTimeAsked);
  try {
    const before = await asked();
    // pauseAt jumps forward to a moment ahead of the page's time and stops there
    const now = await page.evaluate(() => Date.now());
    await page.clock.pauseAt(now + AHEAD_MS);
    for (let spent = 0; spent < ms; spent += STEP_MS) {
      await page.clock.runFor(Math.min(STEP_MS, ms - spent));
      const fetches = (await asked()) - before;
      await expect.poll(() => (told >= fetches ? open.size : -1), {message: 'a request of the page has not ended', intervals: [5], timeout: 10000}).toBe(0);
    }
  } finally {
    page.off('request', started);
    page.off('requestfinished', ended);
    page.off('requestfailed', ended);
    await page.clock.resume();
  }
}
