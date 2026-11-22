// The inspector over a runbook that is not SDC. A second runbook (test/second-runbook.mjs: protocol
// triage.ticket, two actions, three files) is served by two servers of this file, started once for all
// its tests, on ports 7621 and 7622 (SDC_UI_VIEW_PORT, +1):
//   no view      --no-view: one flat list of the protocols found in the store, no phase grouping, no SDC label
//   two phases   --view <file> of two phases: the phases of the file are shown
// Plus the view's own rules (web/view.js) without a page: a view of another format is ignored, a
// file kind comes from the first rule that matches, a tool owner from the view.
import {test, expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {writeSecondRunbook, SECOND, SECOND_VIEW} from './second-runbook.mjs';
import {setView, getView, fileKindOf, phaseItems} from '../web/view.js';
import {classify} from '../web/meaning.js';
import {missingPrograms} from '../decisions.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'view');
const port0 = Number(process.env.SDC_UI_VIEW_PORT || 7621);
const servers = [];

const up = async (port) => { for (let i = 0; i < 50; i++) { try { await fetch(`http://127.0.0.1:${port}/api/sources`); return; } catch { await new Promise((x) => setTimeout(x, 100)); } } throw new Error(`no server on ${port}`); };

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  const store = writeSecondRunbook(path.join(root, 'run'));
  const viewFile = path.join(root, 'two-phases.view.json');
  fs.writeFileSync(viewFile, JSON.stringify(SECOND_VIEW, null, 2));
  for (const [i, extra] of [['--no-view'], ['--view', viewFile]].entries()) {
    const port = port0 + i;
    if (await fetch(`http://127.0.0.1:${port}/api/sources`).then(() => true, () => false)) throw new Error(`port ${port} is in use: set SDC_UI_VIEW_PORT`);
    servers.push(spawn(process.execPath, ['server.mjs', '--port', String(port), ...extra, store], {cwd: ui, stdio: 'ignore'}));
    await up(port);
  }
});
test.afterAll(() => { for (const s of servers) s.kill(); });
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));

async function open(page, k) {
  const errors = [];
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  page.on('console', (m) => m.type() === 'error' && errors.push(`console: ${m.text()}`));
  await page.goto(`http://127.0.0.1:${port0 + k}/`);
  await expect(page.locator('#graph')).toContainText(SECOND.actions[0]);
  return errors;
}

test('another runbook, no view file: its protocols, actions and files render, no SDC phase, no error', async ({page, request}) => {
  expect(await (await request.get(`http://127.0.0.1:${port0}/api/view`)).json()).toBeNull();
  const errors = await open(page, 0);
  const phases = page.locator('#phases');
  await expect(phases.locator('.phase')).toHaveCount(1);
  await expect(phases).toContainText(SECOND.protocol);
  await expect(phases).not.toContainText(/Intake|Input|Dissection|Direction|Roadmap|Ticket|Release/);
  await expect(phases.locator('.stage-h').first()).toContainText('Protocols');
  for (const a of SECOND.actions) await expect(page.locator('#graph')).toContainText(a);
  await expect(page.locator('#center')).toContainText(SECOND.protocol);
  for (const f of SECOND.files) await expect(page.locator('#feed')).toContainText(path.basename(f));
  expect(errors).toEqual([]);
});

test('another runbook, a two-phase view file: the phases of the file are shown', async ({page, request}) => {
  expect((await (await request.get(`http://127.0.0.1:${port0 + 1}/api/view`)).json()).format).toBe('runbook.view/1');
  const errors = await open(page, 1);
  const phases = page.locator('#phases');
  await expect(phases.locator('.phase')).toHaveCount(2);
  await expect(phases.locator('.stage-h').first()).toContainText('Triage');
  await expect(phases.locator('.phase').nth(0)).toContainText('Read');
  await expect(phases.locator('.phase').nth(1)).toContainText('Answer');
  await expect(phases).not.toContainText(/Intake|Dissection|Roadmap|Ticket|triage\.ticket/);
  expect(errors).toEqual([]);
});

test('the view rules: format, first matching file rule, tool owners; no view means generic', () => {
  setView(null);
  expect(classify('notes/idea.yaml')).toEqual({cls: 'code', kind: 'code'});
  expect(phaseItems()).toEqual([]);
  expect(missingPrograms({drive: '/nonexistent/drive', canon: '/nonexistent/canon'}, {kind: 'continue'}, {PATH: '/nonexistent'})).toEqual(['/nonexistent/drive', '/nonexistent/canon']);
  setView({format: 'other/1', phases: {items: [{id: 'x'}]}});
  expect(getView()).toBeNull();
  setView(SECOND_VIEW);
  expect(phaseItems().map((p) => p.id)).toEqual(['read', 'answer']);
  expect(fileKindOf('notes/ticket.md')).toEqual({cls: 'direction', kind: 'ticket'});
  expect(fileKindOf('tools/classify.ts')).toBeNull();
  expect(missingPrograms({drive: '/nonexistent/drive', canon: '/nonexistent/canon'}, {kind: 'continue'}, {PATH: '/nonexistent'})).toEqual(['hoist', '/nonexistent/drive', '/nonexistent/canon', 'crane']);
  setView(null);
});
