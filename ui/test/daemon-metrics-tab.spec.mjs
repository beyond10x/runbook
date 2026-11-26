// The Metrics tab of the page `runbook serve --ui` serves (story the-daemon-serves-a-runs-metrics; tabsOf in
// web/source.js, the pane of ui/daemon/metrics.js): present only when the daemon's view run-metrics answers a row for the
// run on screen, and the step list in the centre takes a step's model calls from that row where the run's events name
// none, never an unknown number as 0. Over the synthetic daemon of daemon-live-fixture.mjs: one that answers the view
// for the live run and refuses it for the ended one, one that answers no row.
import {test, expect as plain} from '@playwright/test';
import {serve, LIVE, DONE} from './daemon-live-fixture.mjs';

const expect = plain.configure({timeout: 10000});
let fx, empty;

const calls = (model_calls, input, output, without, cost, unknown, booked) => ({
  model_calls, ...(input == null ? {} : {input_tokens: input}), ...(output == null ? {} : {output_tokens: output}),
  calls_without_tokens: without, cost_micro_usd: cost, unknown_cost_calls: unknown, booked_micro_usd: booked,
});
// the live run's row: report.write's call is in the events too; evidence.repo's two calls (a run on no route books them
// with no tokens and no known cost) are in the row only
const ROW = {
  run_id: LIVE, run_state: 'Running', submitted_at: '2026-11-26T18:53:00.000Z', started_at: '2026-11-26T18:53:00.600Z', running_ms: 150000,
  steps: [
    {case_id: LIVE, action: 'evidence.repo', role: 'program', tries: 1, step_state: 'ended', started_at: '2026-11-26T18:53:05.000Z', ended_at: '2026-11-26T18:53:14.500Z', exit: 10, duration_ms: 9500, calls: calls(2, null, null, 2, 0, 2, 3000)},
    {case_id: LIVE, action: 'report.write', role: 'program', tries: 1, step_state: 'ended', started_at: '2026-11-26T18:53:16.000Z', ended_at: '2026-11-26T18:53:18.100Z', exit: 0, duration_ms: 2100, calls: calls(1, 1200, 800, 0, 4200, 0, 4200)},
  ],
  steps_left_out: 0, step_limit: 500, totals: calls(3, 1200, 800, 2, 4200, 2, 7200), calls_outside_steps: 0,
};

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon; the tab and the centre at one size'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  [fx, empty] = await Promise.all([
    serve({metrics: {[LIVE]: {body: {rows: [ROW]}}, [DONE]: {status: 404, body: {refused: 'no such run', message: 'the daemon has no metrics of this run'}}}}),
    serve({metrics: {[LIVE]: {body: {rows: []}}}}),
  ]);
});
test.afterAll(() => { fx?.close(); empty?.close(); });

// open the run's page; resolves once the steps are drawn and the page has had the view's answer for the run
async function open(page, d, run) {
  await fetch(`${d.base}/__fixture/anchor`);
  const view = page.waitForResponse((r) => r.url() === `${d.base}/daemon/views/run-metrics?run_id=${run}`, {timeout: 10000});
  await page.goto(`${d.base}/?run=${run}`);
  await view;
  await expect(page.locator('#dsteps tbody tr')).toHaveCount(7);
}
const tabs = (page) => page.locator('#tabs button[data-tab]:visible');

test('a run the view answers a row for has a Metrics tab with the view\'s numbers', async ({page}) => {
  await open(page, fx, LIVE);
  await expect(tabs(page)).toHaveText(['Metrics', 'Products', 'Events']);
  await page.locator('#tabs button[data-tab=metrics]').click();
  const sum = (f) => page.locator(`#panel #metrics-pane .metrics-sum dd[data-field="${f}"]`);
  await expect(sum('calls')).toHaveText('3');
  // the tokens and the cost in the step list's words: the counts above the steps say the same cost
  await expect(sum('tokens')).toHaveText('1,200 in, 800 out');
  await expect(sum('cost')).toHaveText('$0.0042');
  await expect(page.locator('#dsteps .ds-counts')).toContainText(`${await sum('cost').textContent()} and calls of unknown cost`);
  await expect(sum('unknown_cost')).toHaveText('2');
  await expect(sum('booked')).toHaveText('$0.0072');
  await expect(page.locator('#panel #metrics-pane .metrics-steps tbody tr td[data-field="action"]')).toHaveText(['evidence.repo', 'report.write']);
});

test('the step list takes a step\'s calls from the view where the events name none, and says an unknown as unknown', async ({page}) => {
  await open(page, fx, LIVE);
  const repo = page.locator('#dsteps tbody tr').nth(3);
  await expect(repo.locator('.ds-calls')).toHaveText('2 model calls');
  await expect(repo.locator('.ds-calls')).toHaveAttribute('title', 'tokens not known, cost not known');
  // report.write's call is in the events: the view adds none to it
  await expect(page.locator('#dsteps tbody tr').nth(5).locator('.ds-calls')).toHaveText('1 model call');
  const counts = page.locator('#dsteps .ds-counts');
  await expect(counts).toContainText('3 model calls');
  await expect(counts).toContainText('2,000 tokens and calls with no token count');
  await expect(counts).toContainText('$0.0042 and calls of unknown cost');
  await expect(counts).not.toContainText(/\b0 tokens|\$0\.00\b/);
});

test('a run the view refuses, and a run it answers no row for, have no Metrics tab and the events\' numbers only', async ({page}) => {
  await open(page, fx, DONE);
  await expect(tabs(page)).toHaveText(['Products', 'Events']);
  await expect(page.locator('#tabs button[data-tab=metrics]')).toHaveAttribute('hidden', '');
  await expect(page.locator('#dsteps .ds-counts')).toContainText('2 model calls');
  await open(page, empty, LIVE);
  await expect(tabs(page)).toHaveText(['Products', 'Events']);
  await expect(page.locator('#dsteps tbody tr').nth(3).locator('.ds-calls')).toHaveCount(0);
  await expect(page.locator('#dsteps .ds-counts')).toContainText('1 model call');
});

// The pane at the laptop sizes (1366x768, and 1280x800 of the laptop project; this file sets both itself): a step's
// columns (step, role, state, took, calls, cost) are inside the panel with no horizontal scroll, the times are in the
// page's clock words (web/clock.js), never the daemon's ISO form, and the run's own case is named by no id.
for (const [width, height] of [[1366, 768], [1280, 800]]) {
  test(`at ${width}x${height} the Metrics pane fits the panel, says times as the page does and names no case id`, async ({page}) => {
    await page.setViewportSize({width, height});
    await open(page, fx, LIVE);
    await page.locator('#tabs button[data-tab=metrics]').click();
    const pane = page.locator('#panel #metrics-pane');
    await expect(pane.locator('.metrics-steps thead th')).toHaveText(['Step', 'Role', 'State', 'Took', 'Calls', 'Cost']);
    const fit = await page.evaluate(() => {
      const over = (e) => e.scrollWidth - e.clientWidth;
      const pane = document.querySelector('#metrics-pane');
      const right = pane.getBoundingClientRect().right;
      const out = [...pane.querySelectorAll('th, td')].filter((e) => e.getBoundingClientRect().right > right + 0.5).map((e) => e.textContent);
      return {panel: over(document.querySelector('#panel')), pane: over(pane), wrap: over(pane.querySelector('.metrics-wrap')), out};
    });
    expect(fit).toEqual({panel: 0, pane: 0, wrap: 0, out: []});
    await expect(pane).not.toContainText(/\d{4}-\d{2}-\d{2}T\d|\d:\d{2}(\.\d+)?Z\b/);
    const started = await page.evaluate(async (t) => (await import('/clock.js')).clockTime(Date.parse(t)), ROW.started_at);
    await expect(pane.locator('.metrics-sum dd[data-field="started_at"]')).toHaveText(started);
    await expect(pane).not.toContainText(LIVE);
    await expect(pane.locator('[data-field="case"]')).toHaveCount(0);
  });
}
