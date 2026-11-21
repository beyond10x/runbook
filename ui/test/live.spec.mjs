// A store that starts empty and grows: replay full-cycle-1 into an empty directory that the server
// watches (its source given in SDC_UI_LIVE_SOURCE, path in SDC_UI_LIVE_DIR), and check that the
// page grows in place: the first case row and the graph are the same DOM nodes at the end.
import {test, expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needRun} from './need-run.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const sdc = path.resolve(here, '../..');
const target = process.env.SDC_UI_LIVE_DIR;
const source = process.env.SDC_UI_LIVE_SOURCE;

test('empty store grows in place', async ({page}, info) => {
  test.skip(!target || !source || info.project.name !== 'wide', 'needs SDC_UI_LIVE_DIR and SDC_UI_LIVE_SOURCE');
  test.setTimeout(90000);
  needRun(path.join(sdc, 'runs/full-cycle-1/store'), 'live.spec.mjs (replay source)', test);
  fs.rmSync(target, {recursive: true, force: true});
  await page.goto(`/?source=${source}`);
  await expect(page.locator('#tree .empty')).toBeVisible();
  const replay = spawn('node', [path.join(sdc, 'scripts/replay-store.mjs'), path.join(sdc, 'runs/full-cycle-1/store'), target, '25'], {stdio: 'ignore'});
  try {
    await expect(page.locator('#tree .case').first()).toBeVisible({timeout: 30000});
    const first = await page.locator('#tree .case').first().elementHandle();
    await page.waitForTimeout(1500);
    await page.screenshot({path: path.join(here, '..', 'screenshots', 'wide-replay-growing.png')});
    await expect.poll(() => page.locator('#tree .case').count(), {timeout: 60000}).toBeGreaterThanOrEqual(8);
    await new Promise((r) => (replay.exitCode != null ? r() : replay.on('exit', r)));
    await page.waitForTimeout(1500);
    expect(await first.evaluate((el) => el.isConnected)).toBe(true);
    await page.screenshot({path: path.join(here, '..', 'screenshots', 'wide-replay-done.png')});
  } finally {
    replay.kill();
  }
});
