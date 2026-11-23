// The inspector acts on nothing (the run inspector reads run directories; a decision is answered through
// the daemon: `runbook serve --ui`, `runbook decide`). One server of its own (a port the kernel gives)
// over four copies of the committed fixture run (test/fixture-run.mjs person, slim source), each
// given what a run of `runbook run` leaves:
//   history  nothing more: a case store of an older runtime
//   unknown  settings.json, no end record
//   stopped  settings.json and run-stopped.json
//   closed   settings.json and a status.json with state closed
// What it proves: every route that took an act answers 404 or 405 and changes no file; an option of the
// removed acts exits 2; a Decisions drawer says where a decision is answered and has no control that
// acts; the page reports a run live only from the files the runner writes, and "unknown" or "not live"
// otherwise. Screenshot: screenshots/wide-decide-line.png.
import {test, expect} from '@playwright/test';
import {spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const shots = path.join(ui, 'screenshots');
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'read-only');
const KINDS = ['history', 'unknown', 'stopped', 'closed'];
let base, srv, sources;
const src = (name) => sources.find((s) => s.path.endsWith(`/${name}/store`)).index;
const tree = (dir) => fs.readdirSync(dir, {recursive: true}).map((f) => `${f}:${fs.statSync(path.join(dir, f)).mtimeMs}`).sort().join('\n');

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  fs.mkdirSync(root, {recursive: true});
  const first = path.join(root, 'history');
  const r = spawnSync('node', ['test/fixture-run.mjs', 'person', first], {cwd: ui, encoding: 'utf8'});
  if (r.status !== 0) throw new Error(`fixture: ${r.stderr}`);
  for (const k of KINDS.slice(1)) fs.cpSync(first, path.join(root, k), {recursive: true});
  const at = new Date().toISOString();
  for (const k of KINDS.slice(1)) fs.writeFileSync(path.join(root, k, 'settings.json'), '{"format":"runbook.settings/1"}\n');
  fs.writeFileSync(path.join(root, 'stopped', 'run-stopped.json'), JSON.stringify({format: 'sdc.run-stopped/1', at, by: 'operator', reason: 'stopped to look', verdict_at_stop: 'awaiting_person', case: 'IDEA-1.direction'}) + '\n');
  fs.writeFileSync(path.join(root, 'closed', 'status.json'), JSON.stringify({format: 'sdc.run-status/1', at, state: 'closed', outcome: 'operating', delivered: true}) + '\n');
  ({url: base, proc: srv} = await startServer(KINDS.map((k) => path.join(root, k, 'store'))));
  sources = await (await fetch(base + '/api/sources')).json();
});

test('every route that took an act answers 404 or 405, and no file of the run changes', async ({request}) => {
  const before = KINDS.map((k) => tree(path.join(root, k)));
  const bodies = [{kind: 'approve', rel: 'IDEA-1/children/IDEA-1.direction', action: 'context.approve', revision: 'x'}, {kind: 'continue'}, {kind: 'answer', qid: 'q1', answer: 'a'}, {kind: 'context', field: 'f', value: 'v'}];
  for (const p of ['/api/act', '/api/act?source=0', '/api/drive', '/api/continue', '/api/answer', '/api/approve', '/daemon/commands/answer-decision', '/daemon/commands/continue']) {
    for (const method of ['post', 'put', 'patch', 'delete']) {
      for (const data of bodies) {
        const res = await request[method](base + p, {headers: {'x-sdc-ui': '1'}, data});
        expect([404, 405], `${method.toUpperCase()} ${p}`).toContain(res.status());
      }
    }
    expect([404, 405], `GET ${p}`).toContain((await request.get(base + p)).status());
  }
  expect(KINDS.map((k) => tree(path.join(root, k)))).toEqual(before);
  for (const k of KINDS) expect(fs.existsSync(path.join(root, k, 'ui-actions.jsonl'))).toBe(false);
});

test('an option of the removed acts, or any other unknown option, exits 2 and names the way to decide', () => {
  for (const args of [['--write'], ['--write', '--principal', 'operator'], ['--write-only', path.join(root, 'history', 'store')], ['--principal', 'anna'], ['--drive', '/bin/true']]) {
    const r = spawnSync(process.execPath, ['server.mjs', '--port', '0', ...args, path.join(root, 'history', 'store')], {cwd: ui, encoding: 'utf8', timeout: 20000});
    expect(r.status, args.join(' ')).toBe(2);
    expect(r.stderr).toMatch(/unknown option/);
    expect(r.stderr).toContain('runbook decide');
  }
});

test('the page offers no control that acts and never reads a token', async ({page, request}) => {
  const s = src('history');
  const served = fs.readdirSync(path.join(ui, 'web'), {recursive: true}).filter((f) => /\.(js|html)$/.test(f));
  expect(served.length).toBeGreaterThan(15);
  for (const f of served) {
    const res = await request.get(base + '/' + f);
    expect(res.status(), f).toBe(200);
    const text = await res.text();
    expect(text, f).not.toMatch(new RegExp(['/api/act', 'x-sdc-ui', 'sdc-' + 'drive', 'run-' + 'trial', 'toast', '/daemon/commands', 'answer-' + 'decision', 'authorization', 'Bearer'].join('|'), 'i'));
  }
  for (const f of ['decide.js', 'daemon/decide.js']) expect((await request.get(base + '/' + f)).status(), f).toBe(404);
  await page.goto(`${base}/?source=${s}&drawer=1`);
  const drawer = page.locator('#drawer');
  await expect(drawer.locator('.dcard').first()).toBeVisible();
  await expect(drawer.locator('#decide-at')).toContainText('runbook serve --ui');
  await expect(drawer.locator('#decide-at')).toContainText('runbook decide <run> <request> <answer>');
  await expect(drawer.locator('#decide-at')).toContainText('runbook runs decisions <run>');
  await expect(drawer.locator('button.btn, button.primary, input, textarea, select')).toHaveCount(0);
  await expect(drawer.getByRole('button', {name: /approve|continue|answer|apply/i})).toHaveCount(0);
  const store = await page.evaluate(() => JSON.stringify([...Object.entries(localStorage), ...Object.entries(sessionStorage), document.cookie, location.href]));
  expect(store.toLowerCase()).not.toContain('token');
  await page.screenshot({path: path.join(shots, 'wide-decide-line.png')});
});

test('the Now band and the run picker say what the run directory shows about being live, and never "running"', async ({page}) => {
  const want = {history: ['Not live', /not live, last activity/], unknown: ['live state unknown', /live state unknown, last activity/]};
  const live = (k) => fetch(`${base}/api/live?source=${src(k)}`).then((r) => r.json());
  expect((await live('history')).liveness).toMatchObject({state: 'history'});
  expect((await live('unknown')).liveness).toMatchObject({state: 'unknown'});
  expect((await live('stopped')).liveness).toEqual({state: 'ended'});
  expect((await live('closed')).liveness).toEqual({state: 'ended'});
  const run = Object.fromEntries(sources.map((x) => [x.path.split('/').slice(-2)[0], x.run]));
  expect(run.history.state).toBe('idle');
  expect(run.unknown).toMatchObject({state: 'unknown', ended: null});
  expect(run.stopped.state).toBe('stopped');
  expect(run.closed.state).toBe('closed');
  for (const k of ['history', 'unknown']) {
    await page.goto(`${base}/?source=${src(k)}`);
    await expect(page.locator('#now .now-sub')).toHaveText(want[k][1]);
    await expect(page.locator('#now .now-s')).not.toContainText(/running/i);
  }
  await page.goto(`${base}/?source=${src('unknown')}`);
  await expect(page.locator('#runpick .pk-mt')).toHaveText('unknown');
  await page.locator('#runpick').click();
  await expect(page.locator(`#pk-o-${src('unknown')} .pk-mt`)).toHaveText('live state unknown');
});
