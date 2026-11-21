import {test, expect} from '@playwright/test';

test.use({viewport: {width: 1440, height: 900}});

test('evaluation: three releases, a column only for a release with runs', async ({page}) => {
  await page.goto('evaluation', {waitUntil: 'networkidle'});
  await expect(page.getByText('Fixture build')).toBeVisible();
  const labels = await page.locator('#metric-cost .sdc-metric__release').allTextContents();
  expect(labels).toEqual(['0.1.0', '0.2.0']);
  await expect(page.locator('.sdc-metric__nodata')).toHaveCount(0);
  await expect(page.locator('#no-runs')).toHaveText('No runs yet on: 0.1.1.');
  await expect(page.locator('#runs-none')).toHaveText('No runs yet on: 0.1.1.');
  await expect(page.locator('#runs-0\\.1\\.1')).toHaveCount(0);
  await expect(page.locator('#metric-clean-passes .sdc-metric__k')).toHaveText(['0/3', '3/3', '3/3', '1/3', '3/3', '3/3']);
  const cost = await page.locator('#metric-cost svg[role="img"]').getAttribute('aria-label');
  expect(cost).not.toContain('0.1.1');
  expect(cost).toMatch(/0\.2\.0: All Haiku median \$[0-9.]+, min \$[0-9.]+, max \$[0-9.]+, n 3/);
  await expect(page.locator('#runs-0\\.2\\.0')).toBeAttached();
  const runHeads = await page.locator('section[aria-labelledby="runs"] > section > h3').allTextContents();
  expect(runHeads[0]).toContain('Release 0.2.0');
});

test('evaluation: the columns are spread over the plot width', async ({page}) => {
  await page.goto('evaluation', {waitUntil: 'networkidle'});
  const svg = page.locator('#metric-cost svg[role="img"]');
  const width = Number((await svg.getAttribute('viewBox'))!.split(' ')[2]);
  const xs = (await page.locator('#metric-cost .sdc-metric__release').evaluateAll((els) => els.map((e) => Number(e.getAttribute('x')))));
  expect(xs[0]).toBeGreaterThan(width * 0.2);
  expect(xs[xs.length - 1]).toBeGreaterThan(width * 0.6);
});

test('evaluation: tiles from the latest release with runs and the cost target line', async ({page}) => {
  await page.goto('evaluation', {waitUntil: 'networkidle'});
  await expect(page.locator('#tile-clean')).toContainText('release 0.2.0');
  await expect(page.locator('#tile-clean li')).toHaveText([/haiku\s*1 of 3/, /mixed\s*3 of 3/, /strong\s*3 of 3/]);
  await expect(page.locator('#tile-target .sdc-tile__big')).toHaveText('$1.25');
  await expect(page.locator('#tile-conformance')).toContainText('lower bound');
  await expect(page.locator('#tile-conformance')).toContainText('release 0.2.0');
  await expect(page.locator('#tile-quality')).toHaveCount(0);
  await expect(page.locator('#metric-cost .sdc-metric__key--target')).toContainText('target $1.25 per run (fixture-cost-target)');
  await expect(page.locator('#metric-cost .sdc-metric__ref--target .sdc-metric__reflabel')).toHaveText('target');
  await expect(page.locator('#cost-split')).toHaveCount(0);
  const rows = page.locator('section[aria-labelledby="experiments"] tbody tr');
  await expect(rows.filter({hasText: 'fixture-free-agent'})).toContainText('inconclusive');
  await expect(rows.filter({hasText: 'baseline-0.1.0'})).toHaveCount(0);
});

test('evaluation: the conformance chart per release, not observed and not measured are not 0', async ({page}) => {
  await page.goto('evaluation', {waitUntil: 'networkidle'});
  const conf = page.locator('#metric-conformance');
  await expect(conf.locator('.sdc-metric__release')).toHaveText(['0.1.0', '0.2.0']);
  await expect(conf.locator('.sdc-metric__nodata')).toHaveCount(0);
  await expect(conf.locator('.sdc-metric__missing')).toHaveText(['not observed', 'not measured']);
  await expect(conf.locator('.sdc-metric__n')).toHaveText(['n=3', 'n=3', 'n=3', 'n=3']);
  const label = (await conf.locator('svg').getAttribute('aria-label')) ?? '';
  expect(label).toContain('lower bound');
  expect(label).not.toContain('0.1.1');
  expect(label).toContain('0.2.0: All Haiku not measured; Sonnet work, Haiku decider median 42 %, min 25 %, max 93 %, n 3; Sonnet work, Opus decider median 76 %, min 45 %, max 100 %, n 3');
  const runs = page.locator('section[aria-labelledby="runs-0.2.0"]');
  await expect(runs).toContainText('not measured (table_missing), 32 scenarios');
  await expect(runs).toContainText('spec scenarios proven passed: 17 of 17 (lower bound)');
  await expect(runs).toContainText('of which 8: precondition not in the specification');
});

test('the timeline page draws a run and the comparison', async ({page}) => {
  await page.goto('llm-is-not-the-runtime', {waitUntil: 'networkidle'});
  await expect(page.locator('.sdc-tl').first()).toBeVisible();
  await expect(page.locator('.sdc-totals')).toBeVisible();
  const model = page.locator('.sdc-tl__span--model[role="button"]').first();
  await model.focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('.sdc-tl__panel')).toContainText('API requests');
  await expect(page.locator('.sdc-tl__panel')).toContainText('Tokens');
  await expect(page.locator('.sdc-compare .sdc-tl')).toHaveCount(2);
  await expect(page.getByText('Runs pending')).toHaveCount(0);
  await expect(page.getByText('Fixture hypothesis, not a measurement.')).toBeVisible();
});
