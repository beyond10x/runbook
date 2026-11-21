import {test, expect} from '@playwright/test';

test.use({viewport: {width: 1440, height: 900}});

test('evaluation: three releases, a release without runs says no data', async ({page}) => {
  await page.goto('evaluation', {waitUntil: 'networkidle'});
  await expect(page.getByText('Fixture build')).toBeVisible();
  const labels = await page.locator('#metric-cost .sdc-metric__release').allTextContents();
  expect(labels).toEqual(['0.1.0', '0.1.1', '0.2.0']);
  await expect(page.locator('#metric-cost .sdc-metric__nodata')).toHaveCount(1);
  await expect(page.locator('#metric-clean-passes .sdc-metric__nodata')).toHaveCount(1);
  await expect(page.locator('#metric-clean-passes .sdc-metric__k')).toHaveText(['0/3', '3/3', '3/3', '1/3', '3/3', '3/3']);
  const cost = await page.locator('#metric-cost svg').getAttribute('aria-label');
  expect(cost).toContain('0.1.1: no data');
  expect(cost).toMatch(/0\.2\.0: All Haiku median \$[0-9.]+, min \$[0-9.]+, max \$[0-9.]+, n 3/);
  await expect(page.locator('#runs-0\\.2\\.0')).toBeAttached();
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
