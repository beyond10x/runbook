// No absolute home path in anything committed or served (AGENTS.md). Two checks:
//   1. every file git would commit under ui/ (tracked or untracked, not ignored; images skipped),
//      generated round files (ux/rounds/*/report.md, round.json, contact.html, trend.md) included;
//   2. every page and API answer of a server over a real-driver store (test/scope-fixture.mjs) whose
//      bindings, run.log and decisions.jsonl name absolute paths under $HOME, on port 7526
//      (SDC_UI_PATHS_PORT): the server shows them relative to the repository, its workspace or ~.
import {test, expect} from '@playwright/test';
import {spawn, spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const HOME_PATH = '/' + 'home/';
const dir = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'paths', 'run');
const port = Number(process.env.SDC_UI_PATHS_PORT || 7526);
const base = `http://127.0.0.1:${port}`;
const hits = (text) => text.split('\n').map((l, i) => [i + 1, l]).filter(([, l]) => l.includes(HOME_PATH)).map(([n, l]) => `${n}: ${l.slice(0, 160)}`);
let srv;

test.describe.configure({mode: 'serial'});
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'once'));
test.afterAll(() => srv?.kill());

test('no committed file under ui/ names an absolute home path', () => {
  const ls = spawnSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z', '.'], {cwd: ui, encoding: 'utf8'});
  expect(ls.status, ls.stderr).toBe(0);
  const files = ls.stdout.split('\0').filter((f) => f && !/\.(png|jpe?g|gif|webp|ico|woff2?)$/i.test(f) && fs.existsSync(path.join(ui, f)));
  expect(files.some((f) => /^ux\/rounds\/R-\d\d\/round\.json$/.test(f)), 'the generated round files are among the checked files').toBe(true);
  const bad = files.flatMap((f) => hits(fs.readFileSync(path.join(ui, f), 'utf8')).map((h) => `${f}:${h}`));
  expect(bad, 'absolute home paths in files git would commit').toEqual([]);
});

test('no served page or API answer names an absolute home path', async ({page}) => {
  test.setTimeout(120000);
  const r = spawnSync('node', ['test/scope-fixture.mjs', dir, 'one'], {cwd: ui, encoding: 'utf8'});
  if (r.status !== 0) throw new Error(`scope-fixture: ${r.stderr}`);
  // the store already names absolute paths (the bindings' tool argv); add the run's own records
  const home = process.env.HOME;
  fs.writeFileSync(path.join(dir, 'run.log'), `$ ${home}/beyond10x/sdc/driver/target/debug/sdc-drive --case-dir ${dir}/store/IDEA-1 run\nexit 0\n`);
  fs.writeFileSync(path.join(dir, 'decisions.jsonl'), JSON.stringify({at: '2026-11-20T12:07:00Z', case: 'IDEA-1.intake', action: 'scope.answer', principal: 'decider:claude', decision: 'approve', record: `${dir}/decider/001-scope.answer`}) + '\n');
  expect(fs.readFileSync(path.join(dir, 'store/IDEA-1/case.json'), 'utf8') + fs.readFileSync(path.join(dir, 'run.log'), 'utf8')).toContain(HOME_PATH);
  if (await fetch(base + '/api/sources').then(() => true, () => false)) throw new Error(`port ${port} is in use: set SDC_UI_PATHS_PORT`);
  srv = spawn(process.execPath, ['server.mjs', '--port', String(port), path.join(dir, 'store')], {cwd: ui, stdio: 'ignore'});
  for (let i = 0; i < 50; i++) { try { await fetch(base + '/api/sources'); break; } catch { await new Promise((x) => setTimeout(x, 100)); } }
  const bad = [];
  const statics = ['/', '/index.html', '/style.css', ...fs.readdirSync(path.join(ui, 'web')).filter((n) => n.endsWith('.js')).map((n) => '/' + n), '/vendor/graph-layout.js'];
  const apis = ['/api/sources', '/api/meta', '/api/tree?source=0', '/api/run?source=0', '/api/live?source=0', '/api/workfacts?source=0', '/api/inbox?source=0'];
  for (const u of [...statics, ...apis]) {
    const res = await fetch(base + u);
    bad.push(...hits(await res.text()).map((h) => `${u} ${h}`));
  }
  // the absolute paths reach the page, made relative to the repository
  const runInfo = await (await fetch(base + '/api/run?source=0')).text();
  expect(runInfo.includes('"cmd":"driver/target/debug/sdc-drive --case-dir '), 'run.log served with a repository-relative path').toBe(true);
  // the page itself, with the Decisions drawer open
  await page.goto(`${base}/?source=0&drawer=1`);
  await page.locator('#tree .case').first().waitFor();
  await page.waitForTimeout(700);
  bad.push(...hits(await page.content()).map((h) => `page ${h}`));
  expect(bad, 'absolute home paths served').toEqual([]);
});
