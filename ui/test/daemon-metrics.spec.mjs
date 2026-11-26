// The metrics pane of the page the daemon serves (ui/daemon/metrics.js; story the-daemon-serves-a-runs-metrics): the
// row of the daemon's view run-metrics drawn as a pane, a number the row leaves out said as "unknown", never 0, and
// every value from the daemon set as text. No daemon and no server: the page and the module are answered by
// `page.route` from this file and from ui/daemon/, and the view by a synthetic row of the shape the daemon answers
// (runbook/tests/daemon/run_metrics.rs). Every other request is aborted and counted.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const ORIGIN = 'http://metrics.invalid';
const MODULE = fs.readFileSync(path.join(ui, 'daemon', 'metrics.js'), 'utf8');

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'a pane drawn from a fixed row: one project is enough'));

const calls = (model_calls, input, output, without, cost, unknown, booked) => ({
  model_calls, ...(input == null ? {} : {input_tokens: input}), ...(output == null ? {} : {output_tokens: output}),
  calls_without_tokens: without, cost_micro_usd: cost, unknown_cost_calls: unknown, booked_micro_usd: booked,
});

const ROW = {
  run_id: 'run-1', run_state: 'Ended', outcome: 'answered',
  submitted_at: '2026-11-26T10:00:00.000Z', started_at: '2026-11-26T10:00:01.000Z', ended_at: '2026-11-26T10:00:10.000Z', duration_ms: 9000,
  steps: [
    {case_id: 'root', action: 'draft.write', role: 'program', tries: 2, step_state: 'ended', started_at: '2026-11-26T10:00:02.000Z',
      ended_at: '2026-11-26T10:00:06.250Z', exit: 0, duration_ms: 2750, calls: calls(3, 140, 30, 1, 210, 1, 7210)},
    {case_id: 'review-1', action: 'review.read', role: 'program', tries: 1, step_state: 'running', started_at: '2026-11-26T10:00:08.000Z',
      running_ms: 65000, calls: calls(2, null, null, 2, 0, 2, 500)},
  ],
  steps_left_out: 3, step_limit: 500, totals: calls(6, 145, 31, 3, 213, 3, 7713), calls_outside_steps: 1,
};

const HOSTILE = '<img src=x onerror="window.__owned=1">';

// The page at ORIGIN: an empty document and the module; `view` answers GET /daemon/views/run-metrics. Returns the
// requests that were aborted.
async function page0(page, view) {
  const aborted = [];
  await page.route('**/*', async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== ORIGIN) { aborted.push(url.href); return route.abort(); }
    if (url.pathname === '/') return route.fulfill({contentType: 'text/html', body: '<!doctype html><html><head><meta charset="utf-8"></head><body></body></html>'});
    if (url.pathname === '/metrics.js') return route.fulfill({contentType: 'text/javascript', body: MODULE});
    if (url.pathname === '/daemon/views/run-metrics') return view(url, route);
    aborted.push(url.href);
    return route.abort();
  });
  await page.goto(`${ORIGIN}/`);
  return aborted;
}

const cell = (page, n, field) => page.locator('#metrics-pane .metrics-steps tbody tr').nth(n).locator(`td[data-field="${field}"]`);
const sum = (page, field) => page.locator(`#metrics-pane .metrics-sum dd[data-field="${field}"]`);

test('the pane says the run, each step and the totals, and a number the row leaves out as unknown, never 0', async ({page}) => {
  const aborted = await page0(page, (url, route) => route.fulfill({contentType: 'application/json', body: JSON.stringify({rows: [ROW]})}));
  await page.evaluate(async () => {
    const m = await import('/metrics.js');
    const {row, error} = await m.readMetrics('run-1');
    document.body.append(m.metricsPane(row, error));
  });
  await expect(sum(page, 'state')).toHaveText('Ended');
  await expect(sum(page, 'outcome')).toHaveText('answered');
  await expect(sum(page, 'duration')).toHaveText('9 s');
  await expect(sum(page, 'calls')).toHaveText('6');
  await expect(sum(page, 'tokens')).toHaveText('145 / 31');
  await expect(sum(page, 'cost')).toHaveText('$0.000213');
  await expect(sum(page, 'unknown_cost')).toHaveText('3');
  await expect(sum(page, 'booked')).toHaveText('$0.007713');
  await expect(page.locator('#metrics-pane .metrics-steps tbody tr')).toHaveCount(2);
  await expect(cell(page, 0, 'action')).toHaveText('draft.write');
  await expect(cell(page, 0, 'tries')).toHaveText('2');
  await expect(cell(page, 0, 'duration')).toHaveText('2.75 s');
  await expect(cell(page, 0, 'exit')).toHaveText('0');
  await expect(cell(page, 0, 'cost')).toHaveText('$0.000210');
  // the second step runs: no end, no exit, its tokens are not known
  await expect(cell(page, 1, 'state')).toHaveText('running');
  await expect(cell(page, 1, 'duration')).toHaveText('running for 1 min 5 s');
  await expect(cell(page, 1, 'ended_at')).toHaveText('running');
  await expect(cell(page, 1, 'exit')).toHaveText('unknown');
  await expect(cell(page, 1, 'tokens_in')).toHaveText('unknown');
  await expect(cell(page, 1, 'tokens_out')).toHaveText('unknown');
  await expect(cell(page, 1, 'cost')).toHaveText('$0.000000');
  await expect(cell(page, 1, 'unknown_cost')).toHaveText('2');
  await expect(page.locator('#metrics-pane')).toContainText('3 more steps not listed: the daemon lists at most 500.');
  await expect(page.locator('#metrics-pane')).toContainText('1 call of no listed step');
  expect(aborted).toEqual([]);
});

test('a refusal of the daemon is said with its status and its word, and no row is drawn', async ({page}) => {
  const asked = [];
  await page0(page, (url, route) => {
    asked.push(url.href);
    return route.fulfill({status: 404, contentType: 'application/json', body: JSON.stringify({refused: 'no such run', message: 'the daemon has no run x'})});
  });
  await page.evaluate(async () => {
    const m = await import('/metrics.js');
    const {row, error} = await m.readMetrics('a b&c');
    document.body.append(m.metricsPane(row, error));
  });
  expect(asked).toEqual([`${ORIGIN}/daemon/views/run-metrics?run_id=a%20b%26c`]);
  await expect(page.locator('#metrics-pane [role=status]')).toHaveText('The metrics of the run could not be read: the daemon answered 404 no such run.');
  await expect(page.locator('#metrics-pane table')).toHaveCount(0);
});

test('text from the daemon stays text: a case, an action, an outcome and a state that read as markup make no element', async ({page}) => {
  const hostile = {...ROW, outcome: HOSTILE, run_state: HOSTILE, steps: [{...ROW.steps[0], case_id: HOSTILE, action: HOSTILE, role: HOSTILE, step_state: HOSTILE, started_at: HOSTILE}]};
  await page0(page, (url, route) => route.fulfill({contentType: 'application/json', body: JSON.stringify({rows: [hostile]})}));
  await page.evaluate(async () => {
    const m = await import('/metrics.js');
    const {row, error} = await m.readMetrics('run-1');
    document.body.append(m.metricsPane(row, error));
  });
  await expect(sum(page, 'outcome')).toHaveText(HOSTILE);
  await expect(cell(page, 0, 'case')).toHaveText(HOSTILE);
  await expect(cell(page, 0, 'action')).toHaveText(HOSTILE);
  expect(await page.locator('#metrics-pane img').count()).toBe(0);
  expect(await page.evaluate(() => window.__owned)).toBeUndefined();
});
