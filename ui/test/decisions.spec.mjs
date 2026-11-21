// The decisions inbox against COPIES of a trial store (never the live run). Needs a server with
// --write on SDC_UI_WRITE_URL whose source 0 has a pending context approval and source 1 an open
// scope-changing question; SDC_UI_WRITE_DIRS lists the two run directories (the stores' parents).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const shots = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'screenshots');
const url = process.env.SDC_UI_WRITE_URL;
const [dirA, dirB] = (process.env.SDC_UI_WRITE_DIRS || '').split(',');
const audit = (d) => (fs.existsSync(path.join(d, 'ui-actions.jsonl')) ? fs.readFileSync(path.join(d, 'ui-actions.jsonl'), 'utf8').trim().split('\n').map((l) => JSON.parse(l)) : []);

test.describe.configure({mode: 'serial'});
test.skip(!url, 'needs SDC_UI_WRITE_URL');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'writes once, on one project'));
test.use({baseURL: url});

test('drawer lists the pending decisions', async ({page}, info) => {
  await page.goto('/?source=0&drawer=1');
  await expect(page.locator('.dcard.k-approve').first()).toBeVisible();
  await expect(page.locator('#deccount')).not.toHaveText('0');
  await page.waitForTimeout(500);
  await page.screenshot({path: path.join(shots, `${info.project.name}-decisions-context.png`)});
});

test('stale view is refused', async ({request}) => {
  const r = await request.post('/api/act?source=0', {headers: {'x-sdc-ui': '1'}, data: {kind: 'approve', rel: 'IDEA-1/children/IDEA-1.direction', action: 'context.approve', revision: 'tdeadbeef'}});
  expect(r.status()).toBe(409);
  expect((await r.json()).error).toMatch(/stale view/);
  const noHeader = await request.post('/api/act?source=0', {data: {kind: 'continue'}});
  expect(noHeader.status()).toBe(403);
});

test('change a context field', async ({page}) => {
  await page.goto('/?source=0&drawer=1');
  const row = page.locator('table.ctx tr', {hasText: 'licence'});
  await row.getByRole('button', {name: 'Change'}).click();
  await row.locator('input.tin').first().fill('Apache-2.0');
  await row.locator('input.note').fill('test of the inbox');
  await row.getByRole('button', {name: 'Save'}).click();
  await expect(page.locator('#toast')).toContainText('Changed licence');
  const ctx = fs.readFileSync(path.join(dirA, 'store/IDEA-1/work/context.yaml'), 'utf8');
  expect(ctx).toMatch(/licence:\n\s+value: Apache-2.0\n\s+status: changed\n\s+reason: "?operator, \d{4}-\d\d-\d\d, test of the inbox/);
  expect(audit(dirA).at(-1)).toMatchObject({action: 'context', target: 'licence', principal: 'operator', exit: 0});
  const card = page.locator('.dcard', {hasText: 'Approve context'});
  await expect(card).toContainText('Changed since the run recorded it');
  await expect(card.getByRole('button', {name: /^Approve t/})).toHaveCount(0);
  await expect(card.getByRole('button', {name: 'Continue run to re-check'})).toBeVisible();
  await card.locator('[data-key="edited"]').scrollIntoViewIfNeeded();
  await page.waitForTimeout(600);
  await page.screenshot({path: path.join(shots, 'wide-decisions-edited.png')});
});

test('an edited artifact cannot be approved at its old revision', async ({request}) => {
  const ib = await (await request.get('/api/inbox?source=0')).json();
  const item = ib.items.find((i) => i.action === 'context.approve');
  const r = await request.post('/api/act?source=0', {headers: {'x-sdc-ui': '1'}, data: {kind: 'approve', rel: item.rel, action: item.action, revision: item.revision, sha: item.artifact.sha}});
  expect(r.status()).toBe(409);
  expect((await r.json()).error).toMatch(/was changed after the run recorded/);
});

test('approve runs the driver and the item leaves the inbox', async ({page}, info) => {
  await page.goto('/?source=0&drawer=1');
  await page.waitForTimeout(1500);
  const card = page.locator('.dcard', {hasText: 'Approve roadmap'});
  await card.getByRole('button', {name: /^Approve t/}).click();
  await expect(page.locator('#toast')).toContainText('Approved roadmap', {timeout: 60000});
  const last = audit(dirA).filter((a) => a.action === 'approve').at(-1);
  expect(last).toMatchObject({target: 'roadmap.approve', capability: 'owner.approve', exit: 0});
  expect(last.argv).toEqual(expect.arrayContaining(['approve', '--capability', 'owner.approve', '--principal', 'operator', '--action', 'roadmap.approve']));
  await expect(page.locator('.dcard', {hasText: 'Approve roadmap'})).toHaveCount(0);
  await expect(page.locator('.dr-hist')).toContainText('roadmap.approve');
  await page.waitForTimeout(800);
  await page.screenshot({path: path.join(shots, `${info.project.name}-decisions-approved.png`)});
  await page.locator('.dr-hist').scrollIntoViewIfNeeded();
  await page.screenshot({path: path.join(shots, `${info.project.name}-decisions-history.png`)});
});

test('autonomy shows decider decisions apart from person decisions', async ({page}, info) => {
  await page.goto('/?source=0&tab=autonomy');
  const rows = page.locator('#panel .drow');
  await expect(rows.filter({hasText: 'decider:haiku'}).first()).toBeVisible();
  await expect(rows.filter({hasText: 'decider:haiku'}).first().locator('.b-decider')).toBeVisible();
  await expect(rows.filter({hasText: 'by operator'}).first().locator('.b-person')).toBeVisible();
  await page.waitForTimeout(800);
  await page.screenshot({path: path.join(shots, `${info.project.name}-autonomy-deciders.png`)});
});

test('answer a scope-changing question', async ({page}, info) => {
  await page.goto('/?source=1&drawer=1');
  const card = page.locator('.dcard.k-question');
  await expect(card).toBeVisible();
  await page.waitForTimeout(800);
  await page.screenshot({path: path.join(shots, `${info.project.name}-decisions-question.png`)});
  await card.locator('input.tin').fill('Kept on the server, in the daemon data file');
  await card.getByRole('button', {name: 'Answer'}).click();
  await expect(page.locator('#toast')).toContainText('Answered q3');
  const q = fs.readFileSync(path.join(dirB, 'store/IDEA-1/work/questions.yaml'), 'utf8');
  expect(q).toMatch(/id: q3[\s\S]*?status: changed[\s\S]*?answer: Kept on the server, in the daemon data file[\s\S]*?answered_by: operator/);
  await expect(page.locator('.dcard.k-question')).toHaveCount(0);
});

test('continue is refused outside sdc/runs', async ({request}) => {
  const r = await request.post('/api/act?source=1', {headers: {'x-sdc-ui': '1'}, data: {kind: 'continue'}});
  expect(r.status()).toBe(409);
  expect((await r.json()).error).toMatch(/not a run under sdc\/runs/);
});
