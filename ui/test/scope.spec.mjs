// A scope-changing answer is an act that needs an authority grant: the drawer answers and records the
// scope.answer approval as the server's principal. Self-contained: test/scope-fixture.mjs makes three
// stores with the real driver (stub executors), each stopped at awaiting_authority for scope.answer, and
// this file starts its own write server (principal anna) over them on port 7522 (SDC_UI_SCOPE_PORT).
//   source 0  one scope-changing question (the stub interview)
//   source 1  two scope-changing questions, answered one after the other through the page
//   source 2  like 0, answered by editing the file the way a person does, then approved from the card
// Fixtures go to SDC_UI_FIXTURE_DIR (default test/fixtures/runs)/scope. Screenshots: screenshots/wide-scope-*.png.
import {test, expect} from '@playwright/test';
import {spawn, spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const shots = path.join(ui, 'screenshots');
const dirs = ['one', 'two', 'edit'].map((n) => path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'scope', n));
const [dOne, dTwo, dEdit] = dirs;
const port = Number(process.env.SDC_UI_SCOPE_PORT || 7522);
const base = `http://127.0.0.1:${port}`;
const procs = [];
const jl = (f) => (fs.existsSync(f) ? fs.readFileSync(f, 'utf8').trim().split('\n').filter(Boolean).map((l) => JSON.parse(l)) : []);
const audit = (d) => jl(path.join(d, 'ui-actions.jsonl'));
const intake = (d) => path.join(d, 'store/IDEA-1/children/IDEA-1.intake');
const events = (d) => jl(path.join(intake(d), 'events.jsonl'));
const qfile = (d) => path.join(d, 'store/IDEA-1/work/questions.yaml');
const questions = (d) => JSON.parse(spawnSync('yq', ['-o=json', '.questions', qfile(d)], {encoding: 'utf8'}).stdout);
const post = (request, source, data) => request.post(`${base}/api/act?source=${source}`, {headers: {'x-sdc-ui': '1'}, data});

test.describe.configure({mode: 'serial'});
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'writes once, on one project'));
test.use({baseURL: base});

test.beforeAll(async () => {
  test.setTimeout(120000);
  for (const [d, v] of [[dOne, 'one'], [dTwo, 'two'], [dEdit, 'one']]) {
    const r = spawnSync('node', ['test/scope-fixture.mjs', d, v], {cwd: ui, encoding: 'utf8'});
    if (r.status !== 0) throw new Error(`scope-fixture ${v}: ${r.stderr}`);
  }
  if (await fetch(base + '/api/sources').then(() => true, () => false)) throw new Error(`port ${port} is in use: set SDC_UI_SCOPE_PORT`);
  const p = spawn('node', ['server.mjs', '--port', String(port), '--write', '--principal', 'anna', ...dirs.map((d) => path.join(d, 'store'))], {cwd: ui, stdio: 'ignore'});
  procs.push(p);
  for (let i = 0; i < 50; i++) { try { await fetch(base + '/api/sources'); break; } catch { await new Promise((r) => setTimeout(r, 100)); } }
});
test.afterAll(() => { for (const p of procs) if (p.exitCode == null) p.kill(); });

test('a refused principal: non-interactive cannot start the server', () => {
  const r = spawnSync('node', ['server.mjs', '--port', String(port + 50), '--write', '--principal', 'non-interactive', path.join(dOne, 'store')], {cwd: ui, encoding: 'utf8', timeout: 15000});
  expect(r.status).toBe(2);
  expect(r.stderr).toMatch(/non-interactive/);
});

test('Now band and drawer name the scope.answer approval and its question', async ({page}) => {
  await page.goto('/?source=0');
  const wait = page.locator('#now .nc.wait');
  await expect(wait.first()).toContainText('scope.answer');
  await expect(page.locator('#now .nc-q').first()).toContainText('?');
  await page.goto('/?source=0&drawer=1');
  const cards = page.locator('.dcard.k-question');
  await expect(cards).toHaveCount(1);
  await expect(cards.first().locator('.chip-r.ap')).toContainText('waits on scope.answer');
  await expect(cards.first().locator('.chip-r.ap')).toContainText('capability scope.answer');
  await expect(cards.first().getByRole('button', {name: 'Answer and approve'})).toBeVisible();
  await expect(page.locator('.dcard.k-approve')).toHaveCount(0);
  await expect(page.locator('#deccount')).toHaveText('1');
});

test('two questions: answers one by one, the last one records the approval', async ({page}, info) => {
  test.setTimeout(120000);
  await page.goto('/?source=1&drawer=1');
  const cards = page.locator('.dcard.k-question');
  await expect(cards).toHaveCount(2);
  for (const n of [0, 1]) await expect(cards.nth(n).getByRole('button', {name: 'Answer', exact: true})).toBeVisible();
  await page.waitForTimeout(600);
  await page.screenshot({path: path.join(shots, `${info.project.name}-scope-two.png`)});

  await cards.filter({hasText: 'Which states'}).locator('input.tin').fill('draft, issued, paid');
  await cards.filter({hasText: 'Which states'}).getByRole('button', {name: 'Answer', exact: true}).click();
  await expect(page.locator('#toast')).toContainText('Answered q1');
  const q1 = questions(dTwo);
  expect(q1.find((q) => q.id === 'q1')).toMatchObject({status: 'changed', answer: 'draft, issued, paid'});
  expect(q1.find((q) => q.id === 'q1').answered_by).toBeUndefined();
  expect(q1.find((q) => q.id === 'q2').status).toBe('open');
  expect(events(dTwo).some((e) => e.event === 'authority_recorded')).toBe(false);
  expect(audit(dTwo).map((a) => a.action)).toEqual(['answer']);
  await expect(page.locator('.dcard.k-question')).toHaveCount(1);
  await expect(page.locator('.dcard.k-question').getByRole('button', {name: 'Answer and approve'})).toBeVisible();

  await page.locator('.dcard.k-question input.tin').fill('the billing manager');
  await page.locator('.dcard.k-question').getByRole('button', {name: 'Answer and approve'}).click();
  await expect(page.locator('#toast')).toContainText('scope.answer approved', {timeout: 60000});
  await expect(page.locator('#toast')).toContainText('anna');
  const qs = questions(dTwo);
  expect(qs.map((q) => q.answered_by)).toEqual(['anna', 'anna']);
  expect(qs.find((q) => q.id === 'q2')).toMatchObject({status: 'changed', answer: 'the billing manager'});
  const ev = events(dTwo);
  expect(ev.find((e) => e.event === 'authority_recorded')).toMatchObject({principal: 'anna', capability: 'scope.answer'});
  expect(ev.find((e) => e.event === 'action_finished' && e.action === 'scope.answer')).toMatchObject({principal: 'anna', exit: expect.stringMatching(/^0$/)});
  const a = audit(dTwo);
  expect(a.map((x) => x.action)).toEqual(['answer', 'answer', 'check', 'approve']);
  expect(a.at(-1)).toMatchObject({target: 'scope.answer', capability: 'scope.answer', exit: 0});
  expect(a.at(-1).argv).toEqual(expect.arrayContaining(['approve', '--capability', 'scope.answer', '--principal', 'anna', '--action', 'scope.answer']));
  expect(JSON.parse(fs.readFileSync(path.join(intake(dTwo), 'authority.json'), 'utf8'))).toEqual(expect.arrayContaining([expect.objectContaining({capability: 'scope.answer', decision: 'granted'})]));
  await expect(page.locator('.dcard.k-question')).toHaveCount(0);
  await expect(page.locator('.dr-hist')).toContainText('scope.answer');
  await page.waitForTimeout(800);
  await page.screenshot({path: path.join(shots, `${info.project.name}-scope-approved.png`)});
});

test('the API refuses a stale view, and a generic approve while a question is awaiting', async ({request}) => {
  const ib = await (await request.get(`${base}/api/inbox?source=0`)).json();
  const q = ib.items.find((i) => i.kind === 'question');
  const stale = await post(request, 0, {kind: 'answer', rel: q.rel, qid: q.qid, sha: 'deadbeef', revision: q.revision, answer: q.default});
  expect(stale.status()).toBe(409);
  expect((await stale.json()).error).toMatch(/stale view/);
  const early = await post(request, 0, {kind: 'approve', rel: q.rel, action: 'scope.answer', revision: q.revision});
  expect((await early.json()).error).toMatch(/answer q\d+ first/);
  expect(questions(dOne).every((x) => x.status === 'open' || !x.scope_changing)).toBe(true);
  expect(audit(dOne).every((x) => x.refused)).toBe(true);
});

test('answered by editing the file: the approve card shows it and Approve checks, then approves', async ({page}, info) => {
  test.setTimeout(120000);
  const id = questions(dEdit).find((q) => q.scope_changing).id;
  const r = spawnSync('yq', ['-i', `(.questions[] | select(.id == "${id}")) |= (.answer = "edited by hand" | .status = "changed")`, qfile(dEdit)], {encoding: 'utf8'});
  expect(r.status).toBe(0);
  await page.goto('/?source=2&drawer=1');
  const card = page.locator('.dcard.k-approve');
  await expect(card).toHaveCount(1);
  await expect(card).toContainText('Approving takes');
  await expect(card).toContainText('edited by hand');
  await expect(card).not.toContainText('Changed since the run recorded it');
  await page.goto('/?source=2');
  await expect(page.locator('#now .nc.wait').first()).toContainText('scope.answer');
  await page.goto('/?source=2&drawer=1');
  await page.waitForTimeout(600);
  await page.screenshot({path: path.join(shots, `${info.project.name}-scope-approve-card.png`)});
  await page.locator('.dcard.k-approve').getByRole('button', {name: /^Approve /}).click();
  await expect(page.locator('.dcard.k-approve')).toHaveCount(0, {timeout: 60000});
  await expect.poll(() => audit(dEdit).map((x) => x.action), {timeout: 60000}).toEqual(['check', 'approve']);
  const q = questions(dEdit).find((x) => x.id === id);
  expect(q).toMatchObject({answer: 'edited by hand', answered_by: 'anna'});
  expect(events(dEdit).find((e) => e.event === 'authority_recorded')).toMatchObject({principal: 'anna', capability: 'scope.answer'});
});
