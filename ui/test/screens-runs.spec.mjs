// Screenshots of each view of the recorded runs runs/full-cycle-1 and runs/trial-todo-1 (git-ignored: a
// fresh clone has neither), saved under screenshots/<project>-runs-<name>.png. Slow lane only
// (playwright.slow.config.mjs): this file starts its own server over the two runs, on a port the kernel
// gives. screens.spec.mjs is the same tour over the committed fixture, in the gate's lane.
import {test, expect} from '@playwright/test';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needRun} from './need-run.mjs';
import {startServer} from './serve.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const FULL = 0, TRIAL = 1;
let base, srv;
test.beforeAll(async () => {
  needRun(path.resolve(ui, '../runs/full-cycle-1/store'), 'screens-runs.spec.mjs', test);
  needRun(path.resolve(ui, '../runs/trial-todo-1/store'), 'screens-runs.spec.mjs', test);
  srv = await startServer([path.resolve(ui, '../runs/full-cycle-1/store'), path.resolve(ui, '../runs/trial-todo-1/store')]);
  base = srv.url;
});
test.afterAll(() => srv?.proc.kill());
const at = (u) => base + u.replace(/FULL/g, FULL).replace(/TRIAL/g, TRIAL);
const dir = path.join(ui, 'screenshots');
const shots = [
  ['overview', '/?source=FULL&case=.'],
  ['intake-excluded', '/?source=FULL&case=IDEA-1.intake&node=claim:dissection.valid'],
  ['direction-awaiting', '/?source=FULL&case=IDEA-1.direction&at=20&node=action:context.approve'],
  ['spec-gate', '/?source=FULL&case=IDEA-1.billing/IDEA-1.billing.spec&node=claim:spec.complete&reads=1'],
  ['autonomy', '/?source=FULL&case=IDEA-1.direction&at=20&tab=autonomy'],
  ['security', '/?source=FULL&case=IDEA-1.billing/IDEA-1.billing.invoices&tab=security'],
  ['metrics', '/?source=FULL&case=IDEA-1.intake&tab=metrics'],
  ['events', '/?source=FULL&case=IDEA-1.intake&tab=events&at=25'],
  ['trial-direction', '/?source=TRIAL&case=IDEA-1.direction&tab=autonomy'],
  ['trial-security', '/?source=TRIAL&case=IDEA-1.intake&tab=security'],
  ['trial-metrics', '/?source=TRIAL&case=IDEA-1.intake&tab=metrics'],
];

for (const [name, url] of shots) {
  test(name, async ({page}, info) => {
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    await page.goto(at(url).replace(/case=([^&]+)/, (m, c) => `case=${c === '.' ? 'IDEA-1' : 'IDEA-1/children/' + c.split('/').join('/children/')}`));
    await expect(page.locator('#tree .case').first()).toBeVisible();
    await page.waitForTimeout(700);
    await page.screenshot({path: path.join(dir, `${info.project.name}-runs-${name}.png`)});
    expect(errors).toEqual([]);
  });
}

test('agent step card', async ({page}, info) => {
  await page.goto(at('/?source=TRIAL&case=IDEA-1/children/IDEA-1.intake&tab=security'));
  const card = page.locator('.agent').first();
  await card.scrollIntoViewIfNeeded();
  await page.waitForTimeout(400);
  await page.screenshot({path: path.join(dir, `${info.project.name}-runs-trial-agent-step.png`)});
});

test('live source', async ({page}, info) => {
  await page.goto(at('/?source=TRIAL'));
  await page.waitForTimeout(1500);
  await page.screenshot({path: path.join(dir, `${info.project.name}-runs-live.png`)});
});
