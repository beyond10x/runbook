// The metrics pane of the page the daemon serves (ui/daemon/metrics.js; story the-daemon-serves-a-runs-metrics): the
// row of the daemon's view run-metrics drawn as a pane, a number the row leaves out said as "unknown", never 0, and
// every value from the daemon set as text. No daemon and no server: the page and the module are answered by
// `page.route` from this file and from ui/web/ and ui/daemon/ (the modules the pane imports, as the daemon serves them),
// and the view by a synthetic row of the shape the daemon answers (runbook/tests/daemon/run_metrics.rs). Every other
// request is aborted and counted. Times, tokens and costs are said in the words of the page's step list (web/clock.js,
// web/runsteps.js).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const ORIGIN = 'http://metrics.invalid';
// a module the daemon serves at /<name>.js: ui/daemon/ or ui/web/, as runbook/src/ui_embed.rs embeds them
const moduleAt = (name) => {
  if (!/^\/[a-z0-9-]+\.js$/.test(name)) return null;
  for (const dir of ['daemon', 'web']) {
    const p = path.join(ui, dir, name.slice(1));
    if (fs.existsSync(p)) return fs.readFileSync(p, 'utf8');
  }
  return null;
};

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
    const module = moduleAt(url.pathname);
    if (module != null) return route.fulfill({contentType: 'text/javascript', body: module});
    if (url.pathname === '/daemon/views/run-metrics') return view(url, route);
    aborted.push(url.href);
    return route.abort();
  });
  await page.goto(`${ORIGIN}/`);
  return aborted;
}

// a step is two rows of the table: its columns (tr.metrics-step) and the line under them with the rest (tr.metrics-more)
const cell = (page, n, field) => page.locator(`#metrics-pane .metrics-steps tbody [data-step="${n}"] [data-field="${field}"]`);
const stepRows = (page) => page.locator('#metrics-pane .metrics-steps tbody tr.metrics-step');
// the page's clock words (web/clock.js) for an ISO time, as the browser says them
const clock = (page, iso) => page.evaluate(async (t) => (await import('/clock.js')).clockTime(Date.parse(t)), iso);
// a time as the daemon writes it (ISO, UTC): the pane never shows one
const ISO_TIME = /\d{4}-\d{2}-\d{2}T\d|\d:\d{2}(\.\d+)?Z\b/;
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
  // tokens and costs in the words of the step list: `1,200 in, 800 out`, `$0.0042` (usd of web/runsteps.js)
  await expect(sum(page, 'tokens')).toHaveText('145 in, 31 out');
  await expect(sum(page, 'cost')).toHaveText('$0.0002');
  await expect(sum(page, 'unknown_cost')).toHaveText('3');
  await expect(sum(page, 'booked')).toHaveText('$0.0077');
  // times in the page's clock words, never the daemon's ISO form
  await expect(sum(page, 'started_at')).toHaveText(await clock(page, ROW.started_at));
  await expect(sum(page, 'ended_at')).toHaveText(await clock(page, ROW.ended_at));
  await expect(page.locator('#metrics-pane')).not.toContainText(ISO_TIME);
  await expect(stepRows(page)).toHaveCount(2);
  await expect(cell(page, 0, 'action')).toHaveText('draft.write');
  await expect(cell(page, 0, 'started_at')).toHaveText(await clock(page, ROW.steps[0].started_at));
  await expect(cell(page, 0, 'ended_at')).toHaveText(await clock(page, ROW.steps[0].ended_at));
  await expect(cell(page, 0, 'tokens_in')).toHaveText('140');
  await expect(cell(page, 0, 'booked')).toHaveText('$0.0072');
  await expect(cell(page, 0, 'tries')).toHaveText('2');
  await expect(cell(page, 0, 'duration')).toHaveText('2.75 s');
  await expect(cell(page, 0, 'exit')).toHaveText('0');
  await expect(cell(page, 0, 'cost')).toHaveText('$0.0002');
  // the second step runs: no end, no exit, its tokens are not known
  await expect(cell(page, 1, 'state')).toHaveText('running');
  await expect(cell(page, 1, 'duration')).toHaveText('running for 1 min 5 s');
  await expect(cell(page, 1, 'ended_at')).toHaveText('running');
  await expect(cell(page, 1, 'exit')).toHaveText('unknown');
  await expect(cell(page, 1, 'tokens_in')).toHaveText('unknown');
  await expect(cell(page, 1, 'tokens_out')).toHaveText('unknown');
  await expect(cell(page, 1, 'cost')).toHaveText('$0.00');
  await expect(cell(page, 1, 'unknown_cost')).toHaveText('2');
  await expect(page.locator('#metrics-pane')).toContainText('3 more steps not listed: the daemon lists at most 500.');
  await expect(page.locator('#metrics-pane')).toContainText('1 call of no listed step');
  expect(aborted).toEqual([]);
});

test('a step of the run\'s own case names no case, a child case is named by its short id, and a count of 0 calls without tokens or of unknown cost is not said', async ({page}) => {
  const run = '01a2f000-0000-7000-8000-00000000a1b2';
  const child = '01a2f000-0000-7000-8000-0000000c41d0';
  const row = {...ROW, run_id: run, totals: calls(4, 145, 31, 0, 213, 0, 213),
    steps: [{...ROW.steps[0], case_id: run, calls: calls(3, 140, 30, 0, 210, 0, 210)}, {...ROW.steps[0], case_id: child, calls: calls(1, 5, 1, 0, 3, 0, 3)}]};
  await page0(page, (url, route) => route.fulfill({contentType: 'application/json', body: JSON.stringify({rows: [row]})}));
  await page.evaluate(async (id) => {
    const m = await import('/metrics.js');
    const {row: r, error} = await m.readMetrics(id);
    document.body.append(m.metricsPane(r, error));
  }, run);
  await expect(stepRows(page)).toHaveCount(2);
  await expect(cell(page, 0, 'case')).toHaveCount(0);
  await expect(cell(page, 1, 'case')).toHaveText('000c41d0');
  await expect(cell(page, 1, 'case')).toHaveAttribute('title', child);
  await expect(page.locator('#metrics-pane')).not.toContainText(run);
  await expect(page.locator('#metrics-pane')).not.toContainText(child);
  await expect(sum(page, 'without_tokens')).toHaveCount(0);
  await expect(sum(page, 'unknown_cost')).toHaveCount(0);
  await expect(page.locator('#metrics-pane .metrics-sum')).not.toContainText(/without tokens|unknown cost/);
  await expect(cell(page, 0, 'unknown_cost')).toHaveCount(0);
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
