// The view file names the product of a runbook's run: a top-level `product`, one string, a path relative to the
// run directory (beside `good_outcomes`; ui/README.md "The view file"). A value that is not a string, is absolute,
// is empty, leaves the run or has a component that starts with a dot is refused where the view is checked: by
// the inspector's check (web/view.js checkView, which refuses the start of the server) and by `runbook check`
// (runbook/src/check.rs, its own test). No browser.
import {test, expect} from '@playwright/test';
import {spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {checkView} from '../web/view.js';
import {EXAMPLE_VIEW} from './loom-run-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const root = path.join(FIXTURE_DIR, 'view-product');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'file and process checks run once'));

test('the view check knows product: a relative path passes; the other types and the paths that must not be served are refused by name', () => {
  expect(checkView(EXAMPLE_VIEW)).toEqual([]);
  expect(checkView({...EXAMPLE_VIEW, product: 'work/example/example.md'})).toEqual([]);
  expect(checkView({...EXAMPLE_VIEW, product: 'report.md'})).toEqual([]);
  const refused = (product) => checkView({...EXAMPLE_VIEW, product}).join(' ');
  expect(refused(5)).toContain('product: expected string, found number');
  expect(refused(['work/a.md'])).toContain('product: expected string, found array');
  expect(refused(null)).toContain('product: expected string, found null');
  expect(refused('')).toContain('product: is empty');
  expect(refused('/etc/hostname')).toContain('product: "/etc/hostname" is absolute');
  expect(refused('../outside.md')).toContain('product: "../outside.md" leaves the run');
  expect(refused('work/../../outside.md')).toContain('leaves the run');
  expect(refused('work\\..\\x.md')).toContain('product: ');
  expect(refused('work/.cache/token.md')).toContain('product: "work/.cache/token.md" has a component that starts with a dot');
  expect(refused('.hidden.md')).toContain('starts with a dot');
  expect(refused('work/a\u0000b.md')).toContain('product: ');
  expect(checkView({...EXAMPLE_VIEW, product: 'work/a.md', produkt: 'x'}).join(' ')).toContain('produkt');
});

test('the server refuses to start with a view whose product is refused, and names the member', () => {
  fs.rmSync(root, {recursive: true, force: true});
  fs.mkdirSync(root, {recursive: true});
  for (const [name, product] of [['absolute', '/etc/hostname'], ['leaves', '../x.md'], ['dot', 'work/.cache/x.md'], ['type', 3]]) {
    const view = path.join(root, `${name}.view.json`);
    fs.writeFileSync(view, JSON.stringify({...EXAMPLE_VIEW, product}));
    const r = spawnSync(process.execPath, ['server.mjs', '--port', '0', '--view', view, '--runs', root], {cwd: ui, encoding: 'utf8', timeout: 15000});
    expect(r.status, name).toBe(2);
    expect(r.stderr, name).toContain('refused');
    expect(r.stderr, name).toContain('product');
  }
});
