// Every page and API answer of a server over a real-driver store (test/scope-fixture.mjs: needs the built
// driver and a canon binary, so the slow lane) whose bindings, run.log and decisions.jsonl name absolute
// paths under $HOME: the server shows them relative to the repository, its workspace or ~. Its own port.
import {test, expect} from '@playwright/test';
import {spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {needDriver} from './need-run.mjs';
import {startServer} from './serve.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const HOME_PATH = '/' + 'home/';
const dir = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'paths', 'run');
let base;
const hits = (text) => text.split('\n').map((l, i) => [i + 1, l]).filter(([, l]) => l.includes(HOME_PATH)).map(([n, l]) => `${n}: ${l.slice(0, 160)}`);
let srv;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'once'));
test.afterAll(() => srv?.kill());

test('no served page or API answer names an absolute home path', async ({page}) => {
  test.setTimeout(120000);
  needDriver('paths-served.spec.mjs (test/scope-fixture.mjs)', test);
  const r = spawnSync('node', ['test/scope-fixture.mjs', dir, 'one'], {cwd: ui, encoding: 'utf8'});
  if (r.status !== 0) throw new Error(`scope-fixture: ${r.stderr}`);
  // the store already names absolute paths (the bindings' tool argv); add the run's own records, with
  // the driver of the checkout this test runs in, wherever that checkout lives
  fs.writeFileSync(path.join(dir, 'run.log'), `$ ${path.resolve(ui, '..')}/driver/target/debug/sdc-drive --case-dir ${dir}/store/IDEA-1 run\nexit 0\n`);
  fs.writeFileSync(path.join(dir, 'decisions.jsonl'), JSON.stringify({at: '2026-11-20T12:07:00Z', case: 'IDEA-1.intake', action: 'scope.answer', principal: 'decider:claude', decision: 'approve', record: `${dir}/decider/001-scope.answer`}) + '\n');
  // (the paths are absolute; under a home directory when the checkout or the fixture directory is)
  expect(fs.readFileSync(path.join(dir, 'run.log'), 'utf8')).toContain(path.resolve(ui, '..') + '/driver/');
  ({url: base, proc: srv} = await startServer([path.join(dir, 'store')]));
  const bad = [];
  const statics = ['/', '/index.html', '/style.css', ...fs.readdirSync(path.join(ui, 'web')).filter((n) => n.endsWith('.js')).map((n) => '/' + n), '/vendor/graph-layout.js'];
  const apis = ['/api/sources', '/api/meta', '/api/tree?source=0', '/api/run?source=0', '/api/live?source=0', '/api/workfacts?source=0', '/api/inbox?source=0'];
  for (const u of [...statics, ...apis]) {
    const res = await fetch(base + u);
    bad.push(...hits(await res.text()).map((h) => `${u} ${h}`));
  }
  // the absolute paths reach the page, made relative to the repository and to the run's directory
  const runInfo = await (await fetch(base + '/api/run?source=0')).text();
  expect(runInfo, 'run.log served with paths relative to the repository and the run').toMatch(/"cmd":"driver\/target\/debug\/sdc-drive --case-dir ([\w.-]+\/)*paths\/run\/store\/IDEA-1 run/);
  // the page itself, with the Decisions drawer open
  await page.goto(`${base}/?source=0&drawer=1`);
  await page.locator('#tree .case').first().waitFor();
  await page.waitForTimeout(700);
  bad.push(...hits(await page.content()).map((h) => `page ${h}`));
  expect(bad, 'absolute home paths served').toEqual([]);
});
