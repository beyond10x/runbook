// A driver step is never started from the page when a program it needs is missing: a check whose tool
// cannot run would be recorded (a server without yq once wrote an `invalid` questions.check into a real
// run). The server here runs with a PATH that holds jq and not yq, over a store waiting on scope.answer
// (test/scope-fixture.mjs), on port 7524 (SDC_UI_PREFLIGHT_PORT). Screenshot: screenshots/wide-preflight.png.
import {test, expect} from '@playwright/test';
import {spawn, spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needDriver} from './need-run.mjs';
import {missingPrograms, onPath} from '../decisions.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'preflight');
const dir = path.join(root, 'run');
const bin = path.join(root, 'bin');
const port = Number(process.env.SDC_UI_PREFLIGHT_PORT || 7524);
const base = `http://127.0.0.1:${port}`;
const lines = (f) => (fs.existsSync(f) ? fs.readFileSync(f, 'utf8').trim().split('\n').filter(Boolean) : []);
const events = () => lines(path.join(dir, 'store/IDEA-1/children/IDEA-1.intake/events.jsonl'));
const qtext = () => fs.readFileSync(path.join(dir, 'store/IDEA-1/work/questions.yaml'), 'utf8');
let srv;

test.describe.configure({mode: 'serial'});
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'once'));

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(120000);
  needDriver('preflight.spec.mjs (test/scope-fixture.mjs)', test);
  fs.rmSync(root, {recursive: true, force: true});
  const r = spawnSync('node', ['test/scope-fixture.mjs', dir, 'one'], {cwd: ui, encoding: 'utf8'});
  if (r.status !== 0) throw new Error(`scope-fixture: ${r.stderr}`);
  // a PATH with jq and without yq
  fs.mkdirSync(bin, {recursive: true});
  const jq = spawnSync('sh', ['-c', 'command -v jq'], {encoding: 'utf8'}).stdout.trim();
  fs.symlinkSync(jq, path.join(bin, 'jq'));
  if (await fetch(base + '/api/sources').then(() => true, () => false)) throw new Error(`port ${port} is in use: set SDC_UI_PREFLIGHT_PORT`);
  srv = spawn(process.execPath, ['server.mjs', '--port', String(port), '--write', '--principal', 'anna', path.join(dir, 'store')], {cwd: ui, stdio: 'ignore', env: {...process.env, PATH: bin}});
  for (let i = 0; i < 50; i++) { try { await fetch(base + '/api/sources'); break; } catch { await new Promise((x) => setTimeout(x, 100)); } }
});
test.afterAll(() => { if (srv?.exitCode == null) srv?.kill(); });

test('the preflight names what is missing, per act', () => {
  const ctx = {drive: '/nonexistent/sdc-drive', canon: '/bin/sh'};
  const env = {PATH: bin};
  expect(onPath('jq', env)).toBe(true);
  expect(onPath('yq', env)).toBe(false);
  expect(missingPrograms(ctx, {kind: 'context'}, env)).toEqual(['yq']);
  expect(missingPrograms(ctx, {kind: 'approve'}, env)).toEqual(['yq', '/nonexistent/sdc-drive']);
  const c = {bindings: {'spec.gate': {argv: ['{dir}/../../../bindings/tools/sdc-tool.sh', 'ess-gate']}}};
  expect(missingPrograms(ctx, {kind: 'approve', c, action: 'spec.gate'}, env)).toEqual(['yq', '/nonexistent/sdc-drive', 'ess']);
  expect(missingPrograms(ctx, {kind: 'continue'}, env)).toEqual(['yq', '/nonexistent/sdc-drive', 'ess', 'aep']);
});

test('without yq: answering and continuing are refused, nothing runs, the drawer says why', async ({page, request}) => {
  const ib = await (await request.get(`${base}/api/inbox?source=0`)).json();
  expect(ib.missing).toContain('yq');
  // without yq the server cannot read the questions either: the card is the scope.answer approval itself,
  // the act that runs questions.check (the step that once recorded `invalid`)
  const q = ib.items.find((x) => x.kind === 'approve' && x.action === 'scope.answer');
  expect(q).toBeTruthy();
  const ev0 = events().length, q0 = qtext();
  const post = (data) => request.post(`${base}/api/act?source=0`, {headers: {'x-sdc-ui': '1'}, data}).then((x) => x.json());
  const a = await post({kind: 'approve', rel: q.rel, action: q.action, revision: q.revision});
  expect(a.ok).toBe(false);
  expect(a.error).toMatch(/cannot start: yq .*not on the server's PATH/);
  const n = await post({kind: 'answer', rel: q.rel, qid: 'q1', sha: 'x', revision: q.revision, answer: 'x'});
  expect(n.error).toMatch(/yq/);
  const k = await post({kind: 'continue'});
  expect(k.ok).toBe(false);
  expect(k.error).toMatch(/yq/);
  // no driver step, no check record, the file untouched; the refusal is in the audit log
  expect(events().length).toBe(ev0);
  expect(qtext()).toBe(q0);
  const audit = lines(path.join(dir, 'ui-actions.jsonl')).map((l) => JSON.parse(l));
  expect(audit.at(-1)).toMatchObject({action: 'continue', missing: expect.arrayContaining(['yq'])});
  expect(audit.some((x) => x.action === 'check')).toBe(false);

  await page.goto(`${base}/?source=0&drawer=1`);
  const gap = page.locator('#drawer .gap[data-key="missing"]');
  await expect(gap).toBeVisible();
  await expect(gap).toContainText('Driver steps refused');
  await expect(gap).toContainText('yq');
  await page.locator('.dcard.k-approve').first().getByRole('button', {name: /^Approve/}).click();
  await expect(page.locator('#toast')).toContainText('not on the server\'s PATH');
  await page.waitForTimeout(400);
  await page.screenshot({path: path.join(ui, 'screenshots', 'wide-preflight.png')});
  expect(events().length).toBe(ev0);
});
