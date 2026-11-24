// What a run of `runbook run` did not record is said as not recorded, never as zero or nothing:
//   what changed   without fs/changes.jsonl (no --watch): "changes were not recorded for this run", naming --watch in
//                  the words of docs/cli.md; "nothing" only when the record exists and is empty
//   the revision   the case header and the event list show the protocol revision of the `opened` event, and no
//                  bare "r" and no "case" without a number when it has none
//   the claims     the phase band, its stage head, the case tree and the metrics tile say "claims not recorded for
//                  this run" for a run with no evaluations/*.json (the runner writes none); a count only when they exist
// The SDC runbook's run (test/fixture-run.mjs, scenario person) keeps every text it had. Own server on a port the
// kernel gives (test/serve.mjs); the Loom runs are written by hand with invented text (test/loom-run-fixture.mjs).
import {test, expect} from '@playwright/test';
import {spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {needRun, fixtureSource} from './need-run.mjs';
import {startServer} from './serve.mjs';
import {writeLoomRun, EXAMPLE_VIEW} from './loom-run-fixture.mjs';
import {describeEvent} from '../web/model.js';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const root = path.join(FIXTURE_DIR, 'loom-run-record');
const NOT_RECORDED = 'claims not recorded for this run';
let srv, base, sources;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const runs = path.join(root, 'runs');
  writeLoomRun(path.join(runs, 'plain'));
  writeLoomRun(path.join(runs, 'watch-empty'), {watch: 'empty'});
  writeLoomRun(path.join(runs, 'watch-changes'), {watch: 'changes'});
  writeLoomRun(path.join(runs, 'no-revision'), {revision: null, protocols: false, bindings: false});
  writeLoomRun(path.join(runs, 'evaluated'), {evaluations: true});
  needRun(fixtureSource('person'), 'loom-run-record.spec.mjs (fixture source)', test);
  const r = spawnSync('node', ['test/fixture-run.mjs', 'person', path.join(runs, 'sdc')], {cwd: ui, encoding: 'utf8'});
  if (r.status !== 0) throw new Error(`fixture person: ${r.stderr}`);
  const view = path.join(root, 'example.view.json');
  fs.writeFileSync(view, JSON.stringify(EXAMPLE_VIEW));
  const s = await startServer(['--view', view, '--view', path.resolve(ui, '../views/sdc.view.json'), '--runs', runs]);
  srv = s.proc;
  base = s.url;
  sources = await (await fetch(base + '/api/sources')).json();
});

const open = async (page, name) => {
  await page.goto(`${base}/?run=${name}`);
  await expect(page.locator('#gtitle .gt-id').first()).toBeVisible();
  await expect(page.locator('#feed .fd-h')).toBeVisible();
};
const sub = (page) => page.locator('#gtitle .gt-sub');

test('without fs/changes.jsonl the page says changes were not recorded and names --watch; it does not say nothing', async ({page}) => {
  await open(page, 'plain');
  const feed = page.locator('#feed');
  await expect(feed.locator('.fd-note')).toContainText('Changes were not recorded for this run');
  await expect(feed.locator('.fd-note code').first()).toHaveText('--watch');
  await expect(feed.locator('.fd-note')).toContainText('fs/changes.jsonl');
  await expect(feed.locator('.fd-note')).toContainText('records every file change of the run in');
  await expect(feed.locator('.fd-empty')).toHaveCount(0);
  await feed.locator('.fchip[data-feed=code]').click();
  await expect(feed.locator('.fd-row')).toHaveCount(0);
  await expect(feed.locator('.fd-empty')).toHaveCount(0);
  await expect(feed.locator('.fd-note')).toBeVisible();
  await expect(feed).not.toContainText('Nothing');
  await page.locator('#tabs button[data-tab=changes]').click();
  const panel = page.locator('#panel');
  await expect(panel).toContainText('File changes were not recorded for this run');
  await expect(panel.locator('code').first()).toHaveText('--watch');
  await expect(panel).not.toContainText('yet');
});

test('with fs/changes.jsonl present and empty, nothing changed is said; with changes, they are listed', async ({page}) => {
  await open(page, 'watch-empty');
  const feed = page.locator('#feed');
  await expect(feed.locator('.fd-note')).toHaveCount(0);
  await feed.locator('.fchip[data-feed=code]').click();
  await expect(feed.locator('.fd-empty')).toContainText('Nothing');
  await page.locator('#tabs button[data-tab=changes]').click();
  await expect(page.locator('#panel')).toContainText('No change recorded');
  await expect(page.locator('#panel')).not.toContainText('--watch');
  await open(page, 'watch-changes');
  await expect(feed.locator('.fd-note')).toHaveCount(0);
  await expect(feed.locator('.fd-empty')).toHaveCount(0);
  await expect(feed.locator('.fd-row')).not.toHaveCount(0);
  await expect(feed).toContainText('notes.txt');
});

test('the SDC run without fs/changes.jsonl keeps its note: revisions and decisions from the case store', async ({page}) => {
  await open(page, 'sdc');
  const note = page.locator('#feed .fd-note');
  await expect(note).toContainText('No fs/changes.jsonl in this run: revisions and decisions from the case store; what changed inside a revision is not recorded.');
  await expect(note).not.toContainText('--watch');
  await page.locator('#tabs button[data-tab=changes]').click();
  await expect(page.locator('#panel')).toContainText('No fs/changes.jsonl in the run directory yet. The recorder (sdc-watch, docs/fs-changes.md)');
  await expect(page.locator('#panel')).not.toContainText('--watch');
});

test('the case header and the event list show the revision of the opened event; with none they show no "r" and no empty "case"', async ({page}) => {
  await open(page, 'plain');
  await expect(sub(page)).toHaveText(/^example · protocol r1( · case [^ ]+)? · \d+ evidence records$/);
  await page.locator('#tabs button[data-tab=explain]').click();
  await expect(page.locator('#panel .ehead .muted')).toHaveText('example r1');
  await open(page, 'no-revision');
  await expect(sub(page)).toHaveText(/^example( · case [^ ]+)? · \d+ evidence records$/);
  await expect(sub(page)).not.toContainText('protocol r');
  await expect(sub(page)).not.toContainText('undefined');
  await page.locator('#tabs button[data-tab=explain]').click();
  await expect(page.locator('#panel .ehead .muted')).toHaveText('example');
  expect([describeEvent({event: 'opened', protocol: {id: 'example'}}), describeEvent({event: 'opened', protocol: {id: 'example', revision: 4}})])
    .toEqual(['opened, protocol example', 'opened, protocol example r4']);
});

test('the SDC run keeps its header: protocol and revision, case revision, evidence records', async ({page}) => {
  await open(page, 'sdc');
  await expect(sub(page)).toHaveText(/^sdc(\.\w+)? · protocol r\d+ · case \w+ · \d+ evidence records$/);
  await page.locator('#tabs button[data-tab=explain]').click();
  await expect(page.locator('#panel .ehead .muted')).toHaveText(/^sdc(\.\w+)? r\d+$/);
});

test('a run with no evaluations says claims were not recorded: no 0/0 in the band, the stage head, the tree or the tiles', async ({page}) => {
  await open(page, 'plain');
  const band = page.locator('#phases');
  await expect(band.locator('.stage-h .stage-n')).toHaveText(NOT_RECORDED);
  await expect(band.locator('.phase .ps')).toHaveText(['not recorded', 'not recorded']);
  await expect(band.locator('.phase').first()).toHaveAttribute('title', new RegExp(NOT_RECORDED));
  await expect(band).not.toContainText(/\d+\/\d+/);
  await expect(page.locator('#tree .case .cnt')).toHaveText('claims not recorded');
  await expect(page.locator('#tree .case .cnt')).toHaveAttribute('title', NOT_RECORDED);
  await expect(page.locator('#tree')).not.toContainText(/\b0\/0\b/);
  await page.locator('#tabs button[data-tab=metrics]').click();
  const tile = page.locator('#panel .tile[data-key="t:Claims TRUE"]');
  await expect(tile).toContainText('not recorded');
  await expect(tile).not.toContainText('0/0');
  await expect(tile).toHaveAttribute('title', new RegExp(NOT_RECORDED));
});

test('a run with evaluations/*.json shows the count it holds', async ({page}) => {
  await open(page, 'evaluated');
  const band = page.locator('#phases');
  await expect(band).not.toContainText(NOT_RECORDED);
  await expect(band.locator('.phase .ps')).toHaveText(['done', '0/1']);
  await expect(band.locator('.stage-h .stage-n')).toHaveText('1/2');
  await expect(page.locator('#tree .case .cnt')).toHaveText('1/2');
  await page.locator('#tabs button[data-tab=metrics]').click();
  await expect(page.locator('#panel .tile[data-key="t:Claims TRUE"] .tv')).toHaveText('1/2');
});

test('the SDC run keeps its counts: a number of phases done over phases, a count of claims', async ({page}) => {
  await open(page, 'sdc');
  await expect(page.locator('#phases .stage-n').first()).toHaveText(/^\d+\/\d+$/);
  await expect(page.locator('#phases')).not.toContainText(NOT_RECORDED);
  await expect(page.locator('#tree .case .cnt').first()).toHaveText(/^\d+\/\d+$/);
  await expect(page.locator('#tree')).not.toContainText('claims not recorded');
});
