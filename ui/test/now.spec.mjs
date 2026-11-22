// The "now" band against runs that are happening: test/fixture-run.mjs plays four scenarios out of
// a recorded run into SDC_UI_FIXTURE_DIR (default test/fixtures/runs/<project>), and this file starts
// its own server over them (test/serve.mjs: a port the kernel gives). The agent test plays on a
// shared clock (test/shared-clock.mjs) with a second server of its own.
// Screenshots: screenshots/<project>-now-*.png.
import {test, expect} from '@playwright/test';
import {spawn, spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needRun} from './need-run.mjs';
import {sharedClock} from './shared-clock.mjs';
import {startServer} from './serve.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const shots = path.join(ui, 'screenshots');
const procs = [];
let base, dir;

test.beforeAll(async ({}, info) => {
  needRun(path.resolve(process.env.SDC_UI_FIXTURE_SOURCE || path.join(ui, '../runs/trial-todo-1')), 'now.spec.mjs (fixture source)', test);
  needRun(path.resolve(ui, '../runs/trial-todo-1/store'), 'now.spec.mjs', test);
  dir = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), info.project.name);
  fs.mkdirSync(dir, {recursive: true});
  const run = (args) => { const p = spawn('node', args, {cwd: ui, stdio: 'ignore'}); procs.push(p); return p; };
  const person = run(['test/fixture-run.mjs', 'person', path.join(dir, 'person')]);
  await new Promise((r) => person.on('exit', r));
  const stores = ['agent', 'two', 'decider', 'person'].map((s) => path.join(dir, s, 'store'));
  for (const s of ['agent', 'two', 'decider']) fs.rmSync(path.join(dir, s), {recursive: true, force: true});
  // sources 5 and 6: runs with a step started and never finished; gone is a case store of the old runtime, legacy a run directory of the runner
  for (const s of ['gone', 'legacy']) await new Promise((r) => run(['test/fixture-run.mjs', s, path.join(dir, s)]).on('exit', r));
  const srv = await startServer([...stores, path.resolve(ui, '../runs/trial-todo-1/store'), ...['gone', 'legacy'].map((s) => path.join(dir, s, 'store'))]);
  base = srv.url;
  procs.push(srv.proc);
});
test.afterAll(() => { for (const p of procs) if (p.exitCode == null) p.kill(); });

const play = (scenario, speed = '1.5', env = process.env) => { const p = spawn('node', ['test/fixture-run.mjs', scenario, path.join(dir, scenario), speed], {cwd: ui, stdio: 'ignore', env}); procs.push(p); return p; };
const shot = (page, info, name) => page.screenshot({path: path.join(shots, `${info.project.name}-now-${name}.png`)});
const errorsOf = (page) => { const e = []; page.on('pageerror', (x) => e.push(String(x))); return e; };

test('one agent in flight: action, model, live progress, elapsed time, files', async ({page}, info) => {
  test.setTimeout(120000);
  const errors = errorsOf(page);
  // the shared clock moves only when this test steps it: every state below is reached by stepping
  // until it shows, never by waiting a duration, so the machine's load cannot make it late. The
  // clock starts two minutes back, so the times the player writes are past ones for the tests after.
  const clock = sharedClock(path.join(dir, 'agent-clock'), Date.now() - 120000);
  const clockSrv = await startServer([path.join(dir, 'agent', 'store')], {env: clock.env});
  procs.push(clockSrv.proc);
  const clockBase = clockSrv.url;
  await clock.serve(clockBase);
  const fx = await clock.play(() => play('agent', '1.5', clock.env));
  await clock.prepare(page);
  await page.goto(clockBase + '/?source=0');
  await clock.sync(page);
  const card = page.locator('#now .nc.run.k-llm');
  await clock.stepUntil(page, 'the agent card', () => card.isVisible());
  await expect(card).toBeVisible();
  // the action has started; its call record follows a moment of play later
  const liveOf = async () => (await fetch(clockBase + '/api/live?source=0')).json();
  await clock.stepUntil(page, 'the call record', async () => (await liveOf()).records.length > 0);
  await expect(card.locator('.nc-act')).toHaveText('change.implement');
  await expect(card.locator('.nc-case')).toHaveText('IDEA-1.todolist.storage');
  await expect(card.locator('.nc-2')).toContainText('claude-haiku-5-5');
  await expect(page.locator('#now .now-s')).toHaveText('1 running');
  const storage = path.join(dir, 'agent/store/IDEA-1/children/IDEA-1.todolist/children/IDEA-1.todolist.storage');
  const obs = fs.readFileSync(path.join(storage, 'observed.jsonl'), 'utf8').trim().split('\n').map((l) => JSON.parse(l));
  const started = obs.filter((o) => o.event === 'action_started').at(-1);
  expect(started).toMatchObject({format: 'sdc.observed/1', action: 'change.implement'});
  const tree = await (await fetch(clockBase + '/api/tree?source=0')).json();
  const sc = tree.cases.find((c) => c.id === 'IDEA-1.todolist.storage');
  const sseq = sc.events[started.index].seq;
  expect(sc.times[sseq]).toBe(Date.parse(started.observed_at));
  expect(sc.timeSource[sseq]).toBe('driver');
  await expect(card).toHaveAttribute('title', /the driver’s observed_at/);
  // the call joins its case by request.json.case and says it runs by status.json
  const live0 = await liveOf();
  expect(live0.records[0]).toMatchObject({case: 'IDEA-1.todolist.storage', caseFrom: 'request.json', status: {state: 'running', alive: true}});
  expect(live0.liveness).toMatchObject({state: 'unknown'});
  // the run directory has no end record and the runner writes no live record: the band says so, and the timer stands
  await expect(page.locator('#now .now-sub')).toContainText(/live state unknown, last activity/);
  const tk = card.locator('.nc-time .tk');
  const t1 = await tk.textContent();
  await clock.step(page);
  await clock.step(page);
  await expect(tk).toHaveText(t1);
  // the graph and the phase strip point at the same action
  await expect(page.locator('#graph .node.live[data-id="action:change.implement"]')).toBeVisible();
  await expect(page.locator('#phases .phase.ph-live')).toHaveCount(1);
  await expect(page.locator('#phases .phase.ph-live')).toHaveAttribute('data-phase', 'implement');
  // tool calls stream in, then file changes with the call that made them
  await clock.stepUntil(page, '13 tool calls', async () => Number((await card.locator('.np-n b').allTextContents())[1] || 0) >= 13);
  await clock.stepUntil(page, 'a file change', () => card.locator('.nc-files .fr').first().isVisible());
  await expect(card.locator('.nc-files .fr').first()).toContainText(/Write|Edit/);
  await page.waitForTimeout(400);
  await shot(page, info, 'agent');
  await clock.stepUntil(page, 'the lib.rs change', () => card.locator('.fr', {hasText: 'lib.rs'}).first().isVisible());
  // the agent's second pass over lib.rs, 1400 ms of play later at speed 1.5
  await clock.step(page);
  await clock.step(page);
  await shot(page, info, 'agent-files');
  // the diff of one change, highlighted
  await card.locator('.fr', {hasText: 'lib.rs'}).first().click();
  const dlg = page.locator('#diffdlg');
  await expect(dlg).toBeVisible();
  await expect(dlg.locator('.dh-lang')).toHaveText('Rust');
  await expect(dlg.locator('.dl.add .hl-k').first()).toBeVisible();
  await page.waitForTimeout(300);
  await shot(page, info, 'diff');
  await page.keyboard.press('Escape');
  // the call ends: the step lands in "just finished" and the next one starts
  await clock.stepUntil(page, 'the player to end', () => fx.exitCode != null);
  await clock.sync(page);
  await expect(page.locator('#now .trail .tr').first()).toBeVisible();
  await expect(page.locator('#now .trail')).toContainText('change.implement');
  await page.waitForTimeout(800);
  await shot(page, info, 'agent-after');
  expect(errors).toEqual([]);
});

test('file changes: run-wide timeline and a modify diff', async ({page}, info) => {
  const errors = errorsOf(page);
  // a late sdc.fs-attribution/1 line supplies the tool of an earlier change and is no change itself
  const live = await (await fetch(base + '/api/live?source=0')).json();
  const lock = live.fs.changes.find((x) => x.path.endsWith('Cargo.lock'));
  expect(lock.attribution).toMatchObject({basis: 'tool-window', tool: {name: 'Write'}});
  expect(live.fs.changes.every((x) => x.format === 'sdc.fs-change/1')).toBe(true);
  await page.goto(base + '/?source=0&tab=changes');
  await expect(page.locator('#panel .fr', {hasText: 'Cargo.lock'}).locator('.fb')).toHaveText('inferred');
  await expect(page.locator('#panel .chg-strip .cs-t').first()).toBeVisible();
  await expect(page.locator('#panel .cg').first()).toBeVisible();
  await page.waitForTimeout(500);
  await shot(page, info, 'changes');
  await page.locator('#panel .fr', {hasText: 'lib.rs'}).filter({has: page.locator('.op-modify')}).first().click();
  await expect(page.locator('#diffdlg .dl.del').first()).toBeVisible();
  await expect(page.locator('#diffdlg .fb')).toHaveText('inferred');
  await page.locator('#diffdlg .dl.del').first().scrollIntoViewIfNeeded();
  await page.waitForTimeout(300);
  await shot(page, info, 'diff-modify');
  const pos = await page.locator('#diffdlg .dh-pos').textContent();
  await page.keyboard.press('ArrowRight');
  await expect(page.locator('#diffdlg .dh-pos')).not.toHaveText(pos);
  await page.keyboard.press('Escape');
  await page.locator('#panel .fr').filter({has: page.locator('.b-exact')}).first().click();
  await expect(page.locator('#diffdlg .fb')).toHaveText('exact');
  await expect(page.locator('#diffdlg .dh-2')).toContainText('the step');
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});

test('two cases in flight', async ({page}, info) => {
  test.setTimeout(90000);
  const errors = errorsOf(page);
  play('two');
  await page.goto(base + '/?source=1');
  await expect(page.locator('#now .nc.run')).toHaveCount(2, {timeout: 20000});
  await expect(page.locator('#now .now-s')).toHaveText('2 running');
  await expect(page.locator('#now .nc.run.k-llm .nc-act')).toHaveText('spec.author');
  await expect(page.locator('#now .nc.run.k-program .nc-act')).toHaveText('plan.cover');
  await expect(page.locator('#now .nc.run.k-program .nc-2')).toContainText('sdc-tool.sh');
  await expect(page.locator('#tree .cst.st-running')).toHaveCount(2);
  await expect.poll(async () => Number(await page.locator('#now .nc.run.k-llm .np-n b').nth(1).textContent()), {timeout: 30000}).toBeGreaterThanOrEqual(6);
  await page.waitForTimeout(500);
  await shot(page, info, 'two');
  expect(errors).toEqual([]);
});

test('a decider deciding in a person’s place', async ({page}, info) => {
  test.setTimeout(60000);
  const errors = errorsOf(page);
  play('decider', '0.8');
  await page.goto(base + '/?source=2');
  const card = page.locator('#now .nc.wait.k-decider');
  await expect(card).toBeVisible({timeout: 20000});
  await expect(card.locator('.nc-act')).toHaveText('roadmap.approve');
  await expect(card).toContainText('owner.approve');
  await expect.poll(async () => Number(await card.locator('.np-n b').nth(1).textContent()), {timeout: 30000}).toBeGreaterThanOrEqual(3);
  await expect(page.locator('#graph .node.live.lv-decider')).toBeVisible();
  await page.waitForTimeout(500);
  await shot(page, info, 'decider');
  expect(errors).toEqual([]);
});

test('waiting on a person, nothing running', async ({page}, info) => {
  const errors = errorsOf(page);
  await page.goto(base + '/?source=3');
  const card = page.locator('#now .nc.wait.k-person');
  await expect(card).toBeVisible();
  await expect(page.locator('#now .now-s')).toHaveText('Waiting on you: nothing runs until this is decided');
  await expect(card.locator('.nc-act')).toHaveText('context.approve');
  await expect(card.locator('.nc-hold')).toContainText('Held by');
  await expect(card.locator('.nc-hold')).toContainText(/You \([a-z][a-z0-9:_-]*\)/);
  await expect(card).toContainText('owner.approve');
  // a case store of the old runtime is not live: the clock stands at the last event
  await expect(page.locator('#now .now-sub')).toHaveText(/^not live, last activity [\d: APM]+$/);
  await expect(card.locator('.tk')).toHaveText('0:00');
  await expect(card).toContainText(/since [\d: APM]+/);
  await page.waitForTimeout(500);
  await shot(page, info, 'person');
  expect(errors).toEqual([]);
});

test('closed run: says so with the outcome', async ({page}, info) => {
  const errors = errorsOf(page);
  await page.goto(base + '/?source=4');
  await expect(page.locator('#now .nc.idle.k-done')).toBeVisible();
  await expect(page.locator('#now .now-s')).toHaveText('Closed: ended operating');
  await expect(page.locator('#now .nc-out')).toHaveText('operating');
  await expect(page.locator('#now .trail .tr')).toHaveCount(4);
  await page.waitForTimeout(500);
  await shot(page, info, 'closed');
  expect(errors).toEqual([]);
});

// A step started and never finished in a case store of the old runtime: not live, not running.
test('a case store of the old runtime is not live: the started steps say so', async ({page}, info) => {
  const errors = errorsOf(page);
  await page.goto(base + '/?source=5');
  await expect(page.locator('#now .nc.gone')).toHaveCount(2);
  await expect(page.locator('#now .nc.run')).toHaveCount(0);
  await expect(page.locator('#now .now-s')).toHaveText('Nothing running');
  await expect(page.locator('#now .now-sub')).toContainText('a started step never finished, the run is not live');
  await expect(page.locator('#now .now-drv.off')).toHaveText('Not live');
  await expect(page.locator('#now .nc.gone .nc-verb').first()).toHaveText('Not live');
  await expect(page.locator('#now .nc.gone').first()).toContainText('a case store of the old driver');
  // nothing says it moves: every clock stands at the last event
  await expect(page.locator('#now .now-sub')).toContainText(/the run is not live, last activity [\d: APM]+/);
  await expect(page.locator('#now .nc.gone .tk').first()).toHaveText('0:00');
  await page.waitForTimeout(400);
  await shot(page, info, 'driver-gone');
  expect(errors).toEqual([]);
});

test('a run directory of the runner with no observed.jsonl: live state unknown, the steps stay running', async ({page}) => {
  const errors = errorsOf(page);
  await page.goto(base + '/?source=6');
  await expect(page.locator('#now .nc.run')).toHaveCount(2);
  await expect(page.locator('#now .nc.gone')).toHaveCount(0);
  await expect(page.locator('#now .now-s')).toHaveText('2 running');
  await expect(page.locator('#now .now-drv')).toHaveCount(0);
  await expect(page.locator('#now .now-sub')).toContainText(/live state unknown, last activity/);
  // the server's own clock: it saw the files a moment ago
  await expect(page.locator('#now .nc.run .tk').first()).toHaveText(/^0:\d\d$/);
  const live = await (await fetch(base + '/api/live?source=6')).json();
  expect(live.liveness).toMatchObject({state: 'unknown'});
  const tree = await (await fetch(base + '/api/tree?source=6')).json();
  expect(tree.cases.every((c) => !c.observed && Object.values(c.timeSource).every((s) => s === 'server'))).toBe(true);
  expect(errors).toEqual([]);
});

// status.json (sdc.call-status/1) and request.json.case, on records written beside the legacy run
test('a call is in progress by status.json when there is one, by an empty response.json otherwise', async ({page}) => {
  const run = path.join(dir, 'legacy');
  const deadPid = spawnSync(process.execPath, ['-e', '']).pid;
  const mk = (name, request, response, status) => {
    const d = path.join(run, 'llm', name);
    fs.mkdirSync(d, {recursive: true});
    fs.writeFileSync(path.join(d, 'request.json'), JSON.stringify({model: 'claude-haiku-5-5', node: name.replace(/^\d+-/, ''), max_turns: 20, ...request}));
    fs.writeFileSync(path.join(d, 'response.json'), response);
    fs.writeFileSync(path.join(d, 'events.jsonl'), '');
    if (status) fs.writeFileSync(path.join(d, 'status.json'), JSON.stringify({format: 'sdc.call-status/1', pid: deadPid, started_at: new Date().toISOString(), finished_at: null, exit: null, ...status}));
  };
  try {
    mk('001-spec.author', {case: 'IDEA-1.todolist.spec'}, '{"result":"x"}', {state: 'running'}); // response written, status says running
    mk('002-plan.cover', {}, '', {state: 'finished', exit: 0, finished_at: new Date().toISOString()}); // empty response, status says finished
    mk('003-plan.scope', {}, '', {state: 'failed', exit: 1}); // failed
    mk('004-plan.status', {}, ''); // no status.json: the empty response means in progress
    mk('005-plan.other', {}, '{"result":"y"}'); // no status.json, response written
    await expect.poll(async () => (await (await fetch(base + '/api/live?source=6')).json()).records.map((r) => r.rel).sort(), {timeout: 10000}).toEqual(['llm/001-spec.author', 'llm/004-plan.status']);
    const live = await (await fetch(base + '/api/live?source=6')).json();
    const a = live.records.find((r) => r.rel === 'llm/001-spec.author');
    expect(a).toMatchObject({case: 'IDEA-1.todolist.spec', caseFrom: 'request.json', status: {state: 'running', pid: deadPid, alive: false}});
    expect(live.records.find((r) => r.rel === 'llm/004-plan.status')).toMatchObject({status: null, case: null, caseFrom: null});
    // the spec.author card takes its call by the case it names, and says the call's process is gone
    await page.goto(base + '/?source=6');
    const card = page.locator('#now .nc.run', {hasText: 'spec.author'});
    await expect(card.locator('.nc-3')).not.toContainText('no call record yet');
    await expect(card.locator('.nc-warn')).toContainText(`call pid ${deadPid} is gone`);
    // finished by status.json: it leaves the list
    fs.writeFileSync(path.join(run, 'llm/001-spec.author/status.json'), JSON.stringify({format: 'sdc.call-status/1', state: 'finished', pid: deadPid, started_at: a.status && new Date().toISOString(), finished_at: new Date().toISOString(), exit: 0}));
    await expect.poll(async () => (await (await fetch(base + '/api/live?source=6')).json()).records.map((r) => r.rel), {timeout: 10000}).toEqual(['llm/004-plan.status']);
  } finally {
    fs.rmSync(path.join(run, 'llm'), {recursive: true, force: true});
  }
});

// The real recorder: SDC_UI_WATCH_RUN names a run directory that sdc-watch recorded (for example
// the copy `sdc/watch/tests/e2e.sh <scratch>` leaves in <scratch>/e2e/trial-todo-1).
test('changes recorded by sdc-watch', async ({page}, info) => {
  const run = process.env.SDC_UI_WATCH_RUN;
  test.skip(!run, 'needs SDC_UI_WATCH_RUN');
  const watched = await startServer([path.join(run, 'store')]);
  procs.push(watched.proc);
  const url = watched.url;
  const errors = errorsOf(page);
  const recs = fs.readFileSync(path.join(run, 'fs', 'changes.jsonl'), 'utf8').trim().split('\n').map((l) => JSON.parse(l)).filter((r) => r.format === 'sdc.fs-change/1');
  await page.goto(url + '/?case=IDEA-1/children/IDEA-1.todolist/children/IDEA-1.todolist.storage&tab=changes');
  await expect(page.locator('#panel .cg .fr')).toHaveCount(recs.length);
  await expect(page.locator('#panel .cg .fb.b-exact')).toHaveCount(recs.filter((r) => r.attribution.basis === 'action-window').length);
  await page.waitForTimeout(400);
  await shot(page, info, 'changes-watch');
  await page.locator('#panel .fr', {hasText: 'lib.rs'}).last().click();
  await expect(page.locator('#diffdlg .dl.add').first()).toBeVisible();
  await expect(page.locator('#diffdlg .dh-lang')).toHaveText('Rust');
  await page.waitForTimeout(300);
  await shot(page, info, 'diff-watch');
  expect(errors).toEqual([]);
});
