// The graph of a run of `runbook run` is built from what the runner wrote: the protocol's artifacts and actions
// (protocols/*.protocol.yaml of the run directory) and the run's copy of the bindings (bindings/*.yaml,
// loom.bindings/1). An action is never drawn as an artifact; a part the inspector cannot read says "not
// available for this run", never "unbound". Own server on a port the kernel gives (test/serve.mjs); the runs are
// written by hand with invented text (test/loom-run-fixture.mjs).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {writeLoomRun, EXAMPLE_VIEW, ACTIONS, ARTIFACTS} from './loom-run-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'graph-any-runbook');
let srv, base, sources;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  const runs = path.join(root, 'runs');
  writeLoomRun(path.join(runs, 'full'));
  writeLoomRun(path.join(runs, 'no-bindings'), {bindings: false});
  writeLoomRun(path.join(runs, 'no-protocol'), {protocols: false, bindings: false});
  const view = path.join(root, 'example.view.json');
  fs.mkdirSync(root, {recursive: true});
  fs.writeFileSync(view, JSON.stringify(EXAMPLE_VIEW));
  const s = await startServer(['--view', view, '--runs', runs]);
  srv = s.proc;
  base = s.url;
  sources = await (await fetch(base + '/api/sources')).json();
});

const indexOf = (name) => sources.find((s) => s.name === name).index;
const caseOf = async (request, name) => (await (await request.get(`${base}/api/tree?source=${indexOf(name)}`)).json()).cases[0];
const graphOf = async (page, name) => {
  await page.goto(`${base}/?run=${name}`);
  await expect(page.locator('#graph .node').first()).toBeVisible();
  return page.locator('#graph');
};
const chips = (g) => g.locator('.node.k-action').evaluateAll((els) => Object.fromEntries(els.map((e) => [e.dataset.id, e.querySelector('.chip text')?.textContent])));

test('the API carries the protocol\'s artifacts and actions, and each action\'s binding from the run\'s bindings copy', async ({request}) => {
  const c = await caseOf(request, 'full');
  expect(Object.keys(c.ir.artifacts)).toEqual(ARTIFACTS);
  expect(Object.keys(c.ir.actions)).toEqual(ACTIONS);
  expect(c.ir.actions['notes.check']).toMatchObject({effect: 'read', may_produce: [{evidence: 'notes_shape'}]});
  expect(c.bindings.$artifacts).toEqual({notes: 'notes.txt', report: 'report.md'});
  expect(c.bindings['notes.check']).toMatchObject({argv: ['tools/example-check.sh', 'check-notes'], producer: 'example-check'});
  expect(c.bindings['notes.write']).toMatchObject({llm: true});
  expect(c.bindings['report.approve']).toMatchObject({manual: true});
  expect(c.bindings['report.read']).toMatchObject({copy: {from: 'example-first@1'}});
});

test('the graph draws the two artifacts as artifacts and the six actions as actions, each with who runs it', async ({page}) => {
  const g = await graphOf(page, 'full');
  expect(await g.locator('.node.k-artifact').evaluateAll((els) => els.map((e) => e.dataset.id))).toEqual(ARTIFACTS.map((a) => `artifact:${a}`));
  expect(await chips(g)).toEqual({
    'action:notes.write': 'LLM', 'action:notes.check': 'program', 'action:report.write': 'LLM', 'action:report.check': 'program', 'action:report.approve': 'person', 'action:report.read': 'driver',
  });
  // no action is drawn as an artifact; none is unbound or unreadable
  await expect(g.locator('.node.k-artifact', {hasText: /\.(write|check|approve|read)\b/})).toHaveCount(0);
  await expect(g.locator('.chip', {hasText: /unbound|not available/})).toHaveCount(0);
  // an artifact shows where the run placed it, not "not written": the run records no revision
  await expect(g.locator('[data-id="artifact:notes"]')).toHaveAttribute('data-v', 'notes.txt');
  // the bound program is on the page
  await g.locator('[data-id="action:notes.check"]').click();
  await expect(page.locator('#panel')).toContainText('tools/example-check.sh');
  await expect(page.locator('#panel')).not.toContainText('unbound');
});

test('without the bindings copy the actions say "not available for this run", never "unbound"', async ({page, request}) => {
  const g = await graphOf(page, 'no-bindings');
  const got = await chips(g);
  expect(got['action:notes.check']).toBe('not available');
  expect(got['action:report.write']).toBe('not available');
  await expect(g.locator('.chip', {hasText: 'unbound'})).toHaveCount(0);
  await expect(g.locator('[data-id="action:notes.check"] title')).toContainText('not available for this run');
  await g.locator('[data-id="action:notes.check"]').click();
  await expect(page.locator('#panel')).toContainText('not available for this run');
  await expect(page.locator('#panel')).not.toContainText('unbound');
  expect((await caseOf(request, 'no-bindings')).bindings.notes).toBeUndefined();
});

test('without the protocol copy the graph says it is not available for this run', async ({page}) => {
  await page.goto(`${base}/?run=no-protocol`);
  await expect(page.locator('#graph')).toContainText('not available for this run');
  await expect(page.locator('#graph')).not.toContainText('no ir.json');
});
