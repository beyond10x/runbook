// Screenshots of each view, saved under screenshots/<project>-<name>.png, over the committed fixture
// (test/recorded/trial-todo-1-slim, played into ui/target/fixture-runs/screens-<project> by
// test/fixture-run.mjs and served by a server of this file on a port the kernel gives). The tour of the
// real recorded runs, with the billing cases the fixture does not have, is screens-runs.spec.mjs (slow lane).
import {test, expect} from '@playwright/test';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {startServer} from './serve.mjs';
import {settled} from './settled.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
let base, srv;
test.beforeAll(async ({}, info) => {
  const run = path.join(FIXTURE_DIR, `screens-${info.project.name}`);
  const r = spawnSync(process.execPath, ['test/fixture-run.mjs', 'person', run], {cwd: ui, encoding: 'utf8'});
  if (r.status !== 0) throw new Error(`fixture person: ${r.stderr}`);
  srv = await startServer([path.join(run, 'store')]);
  base = srv.url;
});
test.afterAll(() => srv?.proc.kill());
const at = (u) => base + u.replace(/FULL|TRIAL/g, '0');
const dir = path.join(ui, 'screenshots');
const shots = [
  ['overview', '/?source=FULL&case=.'],
  ['intake-excluded', '/?source=FULL&case=IDEA-1.intake&node=claim:dissection.valid'],
  ['direction-awaiting', '/?source=FULL&case=IDEA-1.direction&at=20&node=action:context.approve'],
  ['autonomy', '/?source=FULL&case=IDEA-1.direction&at=20&tab=autonomy'],
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
    await settled(page);
    await page.screenshot({path: path.join(dir, `${info.project.name}-${name}.png`)});
    expect(errors).toEqual([]);
  });
}

test('agent step card', async ({page}, info) => {
  await page.goto(at('/?source=TRIAL&case=IDEA-1/children/IDEA-1.intake&tab=security'));
  const card = page.locator('.agent').first();
  await card.scrollIntoViewIfNeeded();
  await settled(page);
  await page.screenshot({path: path.join(dir, `${info.project.name}-trial-agent-step.png`)});
});

test('live source', async ({page}, info) => {
  await page.goto(at('/?source=TRIAL'));
  await settled(page);
  await page.screenshot({path: path.join(dir, `${info.project.name}-live.png`)});
});
