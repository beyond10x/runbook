// The inspector over a runbook that is not SDC. A second runbook (test/second-runbook.mjs: protocol
// triage.ticket, three actions, files, a record of its own and a decision of its own) is served by three
// servers of this file, started once for all its tests, each on a port the kernel gives (test/serve.mjs),
// so two runs on one machine do not meet:
//   none      --no-view: one flat list of the protocols found in the store, no phase grouping, no SDC label,
//             and the page says which view is missing
//   two       --view <file> of two phases, a parser of asks.yaml and the decision of answering an ask: its
//             phases, its record sentences and its decision are shown
//   foreign   --view views/sdc.view.json over the same store: a view that does not cover the run is not
//             used (no SDC fallback), the page says which view is missing
// Plus the view's own rules (web/view.js) without a page: a view of another format is ignored, a member the
// inspector does not know is refused with a message that names it, a file kind comes from the first rule that
// matches, a parser from the view.
import {test, expect} from '@playwright/test';
import {spawn, spawnSync, execFileSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {writeSecondRunbook, SECOND, SECOND_VIEW, ASKS} from './second-runbook.mjs';
import {startServer} from './serve.mjs';
import {setView, getView, checkView, fileKindOf, phaseItems} from '../web/view.js';
import {classify, meaningOf} from '../web/meaning.js';
import {waitsOnScope, scopeQuestions, amendStall} from '../decisions.mjs';
import {sdcView} from './sdc-view.mjs';
import {registerRun} from '../runfiles.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const root = path.join(FIXTURE_DIR, 'view');
const servers = [];
const port = {};
let hasYq = true;
try { execFileSync('yq', ['--version'], {stdio: 'ignore'}); } catch { hasYq = false; }

const url = (k, p = '') => `http://127.0.0.1:${port[k]}${p}`;
const asks = (dir, id = 'T-2') => path.join(dir, 'store', id, 'work', SECOND.ask.file);

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  const store = writeSecondRunbook(path.join(root, 'run'));
  const viewFile = path.join(root, 'two-phases.view.json');
  fs.writeFileSync(viewFile, JSON.stringify(SECOND_VIEW, null, 2));
  const plan = {
    none: [['--no-view'], store],
    two: [['--view', viewFile], store],
    foreign: [['--view', path.join(ui, '..', 'views', 'sdc.view.json')], store],
  };
  for (const [k, [extra, st]] of Object.entries(plan)) {
    const s = await startServer([...extra, st]);
    port[k] = s.port;
    servers.push(s.proc);
  }
});
test.afterAll(() => { for (const s of servers) s.kill(); });
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));

async function open(page, k) {
  const errors = [];
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  page.on('console', (m) => m.type() === 'error' && errors.push(`console: ${m.text()}`));
  await page.goto(url(k, '/'));
  await expect(page.locator('#graph')).toContainText(SECOND.actions[0]);
  return errors;
}
const inbox = async (request, k) => (await (await request.get(url(k, '/api/inbox'))).json());

test('another runbook, no view file: its protocols, actions and files render, no SDC phase, no error, the missing view is named', async ({page, request}) => {
  expect(await (await request.get(url('none', '/api/view'))).json()).toBeNull();
  const errors = await open(page, 'none');
  const phases = page.locator('#phases');
  await expect(phases.locator('.phase')).toHaveCount(1);
  await expect(phases).toContainText(SECOND.protocol);
  await expect(phases).not.toContainText(/Intake|Input|Dissection|Direction|Roadmap|Ticket|Release/);
  await expect(phases.locator('.stage-h').first()).toContainText('Protocols');
  await expect(page.locator('#view-note')).toContainText(`No view for ${SECOND.protocol}`);
  await expect(page.locator('#view-note')).toContainText('--view FILE');
  for (const a of SECOND.actions) await expect(page.locator('#graph')).toContainText(a);
  await expect(page.locator('#center')).toContainText(SECOND.protocol);
  for (const f of SECOND.files) await expect(page.locator('#feed')).toContainText(path.basename(f));
  // the record of its own is a file like any other without a view: its path, no sentence of a parser
  await expect(page.locator('#feed')).toContainText(SECOND.ask.file);
  await expect(page.locator('#feed')).not.toContainText('ask a1');
  expect(errors).toEqual([]);
  // no decision of the runbook is offered without its view: no question card, the wait is a plain approval
  const ib = await inbox(request, 'none');
  expect(ib.items.filter((x) => x.kind === 'question')).toEqual([]);
  expect(ib.items.find((x) => x.action === SECOND.ask.action)).toMatchObject({kind: 'approve', scope: null});
  const info = await (await request.get(url('none', '/api/view-info'))).json();
  expect(info).toMatchObject({missing: true, protocols: [SECOND.protocol], file: null});
});

test('another runbook, a two-phase view file: the phases of the file are shown, its parsed record and its decision too', async ({page, request}) => {
  const v = await (await request.get(url('two', '/api/view'))).json();
  expect(v.format).toBe('runbook.view/1');
  expect(await (await request.get(url('two', '/api/view-info'))).json()).toMatchObject({missing: false, runbook: 'triage'});
  const errors = await open(page, 'two');
  const phases = page.locator('#phases');
  await expect(phases.locator('.phase')).toHaveCount(2);
  await expect(phases.locator('.stage-h').first()).toContainText('Triage');
  await expect(phases.locator('.phase').nth(0)).toContainText('Read');
  await expect(phases.locator('.phase').nth(1)).toContainText('Answer');
  await expect(phases).not.toContainText(/Intake|Dissection|Roadmap|Ticket|triage\.ticket/);
  await expect(page.locator('#view-note')).toBeEmpty();
  // the record of its own, read by the view's parser: sentences in the view's own words
  const feed = page.locator('#feed');
  await expect(feed).toContainText('ask a1 raised: “Which queue handles refunds?”');
  await expect(feed).toContainText('ask a1 answered: “queue-b” by owner-1');
  expect(errors).toEqual([]);
  // the decision of its own: the open ask the owner answers, and the approval of the action the view names
  const ib = await inbox(request, 'two');
  const a1 = ib.items.find((x) => x.kind === 'question' && x.qid === 'a1');
  expect(a1).toMatchObject({question: SECOND.ask.text, options: SECOND.ask.options, default: SECOND.ask.options[0], case: 'T-2',
    approval: expect.objectContaining({action: SECOND.ask.action, capability: SECOND.ask.action, waiting: 1, last: true})});
  expect(ib.items.find((x) => x.qid === 'a2')).toBeUndefined();
  expect(ib.items.find((x) => x.qid === 'b1')).toMatchObject({kind: 'question', case: 'T-3', approval: null});
  await page.locator('#decbtn').click();
  const drawer = page.locator('#drawer');
  await expect(drawer).toContainText(SECOND.ask.text);
  await expect(drawer).toContainText(SECOND.ask.action);
  await expect(drawer).not.toContainText('scope.answer');
});

test('a view with a decision shows it as a read-only card and a line on where it is answered; no route takes an answer', async ({page, request}) => {
  await open(page, 'two');
  await page.locator('#decbtn').click();
  const drawer = page.locator('#drawer');
  await expect(drawer.locator('.dcard.k-question').first()).toBeVisible();
  await expect(drawer.locator('#decide-at')).toContainText('runbook decide <run> <request> <answer>');
  await expect(drawer.locator('button.btn, input, textarea, select')).toHaveCount(0);
  const before = fs.readFileSync(asks(path.join(root, 'run'), 'T-3'), 'utf8');
  for (const [method, p] of [['post', '/api/act'], ['put', '/api/act'], ['post', '/api/answer'], ['post', '/daemon/commands/answer-decision']]) {
    const r = await request[method](url('two', p), {headers: {'x-sdc-ui': '1'}, data: {kind: 'answer', rel: 'T-3', qid: 'b1', answer: 'queue-b'}});
    expect([404, 405], `${method} ${p}`).toContain(r.status());
  }
  expect((await request.get(url('two', '/api/act'))).status()).toBe(404);
  expect(fs.readFileSync(asks(path.join(root, 'run'), 'T-3'), 'utf8')).toBe(before);
});

test('a view that does not cover the run is not used: no SDC fallback, the page says which view is missing', async ({page, request}) => {
  expect(await (await request.get(url('foreign', '/api/view'))).json()).toBeNull();
  expect(await (await request.get(url('foreign', '/api/view-info'))).json()).toMatchObject({missing: true, protocols: [SECOND.protocol], file: null});
  const errors = await open(page, 'foreign');
  await expect(page.locator('#view-note')).toContainText(`No view for ${SECOND.protocol}`);
  const phases = page.locator('#phases');
  await expect(phases.locator('.phase')).toHaveCount(1);
  await expect(phases).not.toContainText(/Intake|Input|Dissection|Direction|Roadmap|Ticket|Release/);
  await expect(page.locator('#feed')).not.toContainText('ask a1');
  expect(errors).toEqual([]);
  expect((await inbox(request, 'foreign')).items.filter((x) => x.kind === 'question')).toEqual([]);
});

test('a view with a member the inspector does not know is refused, and the message names it', () => {
  expect(checkView(SECOND_VIEW)).toEqual([]);
  expect(checkView(sdcView)).toEqual([]);
  expect(checkView({...SECOND_VIEW, theme: 'dark'})).toEqual([expect.stringContaining('view member "theme" is not known')]);
  const deep = structuredClone(SECOND_VIEW);
  deep.file_kinds.rules[0].colour = 'red';
  deep.decisions.scope_answer.fields.nickname = 'x';
  deep.file_kinds.parsers.asks.changed.rules[0].when.sometimes = ['reply'];
  const said = checkView(deep).join('\n');
  expect(said).toContain('file_kinds.rules[0].colour: view member "colour" is not known');
  expect(said).toContain('decisions.scope_answer.fields.nickname: view member "nickname" is not known');
  expect(said).toContain('file_kinds.parsers.asks.changed.rules[0].when.sometimes: view member "sometimes" is not known');
  const wrong = structuredClone(SECOND_VIEW);
  wrong.file_kinds.parsers.asks.type = 'tree';
  wrong.decisions.scope_answer.flag = 'a b; .x';
  wrong.phases.items[0].claims = 'ticket.read';
  const said2 = checkView(wrong).join('\n');
  expect(said2).toContain('"tree" is not a type of the inspector');
  expect(said2).toContain('decisions.scope_answer.flag: "a b; .x" is not a field name');
  expect(said2).toContain('phases.items[0].claims: expected an array, found string');
  expect(checkView({format: 'other/1'})).toEqual(['format is "other/1", not runbook.view/1']);
  // a refused view is not used
  expect(setView({...SECOND_VIEW, theme: 'dark'})).toHaveLength(1);
  expect(getView()).toBeNull();
  setView(null);
});

test('the server refuses to start with a view that has an unknown member, and names it', () => {
  fs.mkdirSync(root, {recursive: true});
  const bad = path.join(root, 'bad.view.json');
  fs.writeFileSync(bad, JSON.stringify({...SECOND_VIEW, decisions: {...SECOND_VIEW.decisions, scope_answer: {...SECOND_VIEW.decisions.scope_answer, escalate: true}}}));
  const r = spawnSync(process.execPath, ['server.mjs', '--port', '0', '--view', bad, path.join(root, 'run', 'store')], {cwd: ui, encoding: 'utf8', timeout: 20000});
  expect(r.status).toBe(2);
  expect(r.stderr).toContain('decisions.scope_answer.escalate: view member "escalate" is not known');
  expect(r.stderr).toContain(bad);
});

test('the view rules: format, first matching file rule, parsers, decisions; no view means generic', () => {
  setView(null);
  expect(classify('notes/idea.yaml')).toEqual({cls: 'code', kind: 'code'});
  expect(classify(SECOND.ask.file)).toEqual({cls: 'code', kind: 'code'});
  expect(phaseItems()).toEqual([]);
  expect(waitsOnScope([{event: 'awaiting_authority', next: [{action: SECOND.ask.action}]}])).toBe(false);
  expect(amendStall({events: [{event: 'awaiting_person', next: [{action: 'amend.upstream'}]}], bindings: {}}, null)).toBeNull();
  setView({format: 'other/1', phases: {items: [{id: 'x'}]}});
  expect(getView()).toBeNull();
  setView(SECOND_VIEW);
  expect(phaseItems().map((p) => p.id)).toEqual(['read', 'answer']);
  expect(fileKindOf('notes/ticket.md')).toEqual({cls: 'direction', kind: 'ticket'});
  expect(fileKindOf('tools/classify.ts')).toBeNull();
  // the decision is the view's: its action waits, SDC's does not
  expect(waitsOnScope([{event: 'awaiting_authority', next: [{action: SECOND.ask.action}]}])).toBe(true);
  expect(waitsOnScope([{event: 'awaiting_authority', next: [{action: 'scope.answer'}]}])).toBe(false);
  setView(sdcView);
  expect(waitsOnScope([{event: 'awaiting_authority', next: [{action: 'scope.answer'}]}])).toBe(true);
  setView(null);
});

test('a parser of the view says what a change of its record means; a kind with no parser is read as keys', () => {
  setView(SECOND_VIEW);
  const at = (before, after, p = SECOND.ask.file) => meaningOf({path: p}, before, after);
  const created = at(null, ASKS.raised);
  expect(created).toMatchObject({cls: 'direction', kind: 'asks', parsed: true, subject: 'asks'});
  expect(created.items.map((x) => x.text)).toEqual(['ask a1 raised: “Which queue handles refunds?”', 'ask a2 raised: “Is the customer in the EU?”']);
  expect(at(ASKS.raised, ASKS.answered).items.map((x) => x.text)).toEqual(['ask a1 answered: “queue-b” by owner-1']);
  // [ by {replied_by}] is left out when the field is empty
  expect(at(ASKS.raised, ASKS.answered.replace('  replied_by: owner-1\n', '')).items.map((x) => x.text)).toEqual(['ask a1 answered: “queue-b”']);
  expect(at(ASKS.raised, ASKS.raised.replace('needs_owner: true', 'needs_owner: false')).items.map((x) => x.text)).toEqual(['ask a1 changed']);
  expect(at(ASKS.raised, ASKS.raised.replace(/- id: a2[\s\S]*$/, '')).items.map((x) => x.text)).toEqual(['ask a2 removed']);
  // a kind the view has a rule for and no parser: top-level keys
  expect(meaningOf({path: 'notes/ticket.md'}, null, 'x: 1\n').items[0].text).toBe('ticket: x set');
  // a parser that cannot read the file says so and keeps the class
  expect(meaningOf({path: SECOND.ask.file}, null, 'a: [unclosed').parsed).toBe(false);
  setView(null);
});

test('without a decision rule in the view, nothing is read from the questions of a run', () => {
  registerRun(path.join(root, 'run'));
  setView(null);
  expect(scopeQuestions({dir: path.join(root, 'run', 'store', 'T-2'), bindings: {$artifacts: {asks: SECOND.ask.file}}}, path.join(root, 'run'))).toBeNull();
  test.skip(!hasYq, 'yq is not on the PATH');
  setView(SECOND_VIEW);
  const q = scopeQuestions({dir: path.join(root, 'run', 'store', 'T-2'), bindings: {$artifacts: {asks: SECOND.ask.file}}}, path.join(root, 'run'));
  expect(q).toMatchObject({path: SECOND.ask.file, questions: [{id: 'a1', question: SECOND.ask.text, status: 'pending', awaiting: true, default: SECOND.ask.options[0]}]});
  setView(null);
});
