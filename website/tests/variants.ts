import {test, expect, type Page} from '@playwright/test';
import axe from 'axe-core';

export const MODES = [
  {name: '1440 light', viewport: {width: 1440, height: 900}, theme: 'light'},
  {name: '1440 dark', viewport: {width: 1440, height: 900}, theme: 'dark'},
  {name: '390 light', viewport: {width: 390, height: 844}, theme: 'light'},
  {name: '390 dark', viewport: {width: 390, height: 844}, theme: 'dark'},
] as const;

export type Mode = (typeof MODES)[number];

export const FIRST_MODE = MODES[0];

// One navigation per page. The theme is stored before the first paint, as a visitor's choice is,
// and the first mode is the state the page loads in.
export async function visit(page: Page, route: string): Promise<string[]> {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript((t) => window.localStorage.setItem('theme', t), FIRST_MODE.theme);
  const res = await page.goto(route, {waitUntil: 'networkidle'});
  expect(res?.status(), `status of ${route}`).toBe(200);
  return errors;
}

// Width by the viewport; theme the way the site switches it between tabs: the stored choice
// changes and the site's own storage listener applies it, with the system scheme moved along.
export async function setMode(page: Page, mode: Mode): Promise<void> {
  await page.setViewportSize(mode.viewport);
  await page.emulateMedia({colorScheme: mode.theme});
  await page.evaluate((t) => {
    const old = window.localStorage.getItem('theme');
    if (old === t) return;
    window.localStorage.setItem('theme', t);
    const e = document.createEvent('StorageEvent');
    e.initStorageEvent('storage', false, false, 'theme', old, t, window.location.href, window.localStorage);
    window.dispatchEvent(e);
  }, mode.theme);
  await expect(page.locator('html')).toHaveAttribute('data-theme', mode.theme);
  await settle(page);
}

// A switched page has run CSS transitions and re-laid out; a loaded page has not. Wait until no
// finite animation or transition is left and two frames have passed, so the checks see the state a
// fresh load shows.
async function settle(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const frame = () => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done())));
    for (let i = 0; i < 20; i++) {
      await frame();
      const running = document.getAnimations().filter((a) => a.effect?.getComputedTiming().iterations !== Infinity && a.playState === 'running');
      if (running.length === 0) return;
      await Promise.all(running.map((a) => a.finished.catch(() => undefined)));
    }
  });
}

// The same rules and options as AxeBuilder, run once in the page, without the helper page
// AxeBuilder opens for every call (the site has no iframe).
export async function seriousViolations(page: Page): Promise<string[]> {
  if (!(await page.evaluate(() => 'axe' in window))) await page.addScriptTag({content: axe.source});
  const result = await page.evaluate(() => (window as unknown as {axe: {run: (c: Document, o: object) => Promise<{violations: Array<{id: string; impact: string | null; nodes: Array<{target: unknown[]}>}>}>}}).axe.run(document, {resultTypes: ['violations']}));
  return result.violations
    .filter((v) => v.impact === 'serious' || v.impact === 'critical')
    .map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).slice(0, 3).join(', ')}`);
}

// Runs check in each of the four modes on one loaded page. A failure names page, width and theme,
// and the other modes still run.
export async function everyMode(page: Page, route: string, check: (mode: Mode, errors: string[]) => Promise<void>): Promise<void> {
  const errors = await visit(page, route);
  const failures: string[] = [];
  for (const mode of MODES) {
    try {
      await test.step(`/${route} at ${mode.name}`, async () => {
        await setMode(page, mode);
        await check(mode, errors);
      });
    } catch (e) {
      failures.push(`[/${route} at ${mode.name}] ${(e as Error).message}`);
    }
  }
  if (failures.length) throw new Error(failures.join('\n\n'));
}
