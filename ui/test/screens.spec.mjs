// Screenshots of each view, saved under screenshots/<project>-<name>.png. Run against a server:
//   node server.mjs --port 7490 ../runs/full-cycle-1/store &   slot npx playwright test --workers=2
import {test, expect} from '@playwright/test';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

let FULL = 0, TRIAL = 1;
test.beforeAll(async ({request, baseURL}) => {
  // a checkout without runs/ (git-ignored) has no server over full-cycle-1: skipped, with the reason
  const s = await request.get('/api/sources').then((r) => r.json(), () => null);
  test.skip(!s, `screens.spec.mjs: no server at ${baseURL} (SDC_UI_URL); start one over runs/full-cycle-1/store as the head of this file says`);
  test.skip(!s.some((x) => x.name.startsWith('full-cycle-1')), `screens.spec.mjs: the server at ${baseURL} serves no full-cycle-1 store (missing run directory runs/full-cycle-1: runs/ is git-ignored)`);
  FULL = s.find((x) => x.name.startsWith('full-cycle-1'))?.index ?? 0;
  TRIAL = s.find((x) => x.name.startsWith('trial-todo-1'))?.index ?? 1;
});
const at = (u) => u.replace(/FULL/g, FULL).replace(/TRIAL/g, TRIAL);
const dir = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'screenshots');
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
    await page.screenshot({path: path.join(dir, `${info.project.name}-${name}.png`)});
    expect(errors).toEqual([]);
  });
}

test('agent step card', async ({page}, info) => {
  await page.goto(at('/?source=TRIAL&case=IDEA-1/children/IDEA-1.intake&tab=security'));
  const card = page.locator('.agent').first();
  await card.scrollIntoViewIfNeeded();
  await page.waitForTimeout(400);
  await page.screenshot({path: path.join(dir, `${info.project.name}-trial-agent-step.png`)});
});

test('live source', async ({page}, info) => {
  await page.goto(at('/?source=TRIAL'));
  await page.waitForTimeout(1500);
  await page.screenshot({path: path.join(dir, `${info.project.name}-live.png`)});
});
