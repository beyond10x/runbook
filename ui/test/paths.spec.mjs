// No absolute home path in anything committed (AGENTS.md): every file git would commit under ui/ (tracked
// or untracked, not ignored; images skipped), generated round files (ux/rounds/*/report.md, round.json,
// contact.html, trend.md) included. What the server serves is paths-served.spec.mjs (slow lane).
import {test, expect} from '@playwright/test';
import {spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const HOME_PATH = '/' + 'home/';
const hits = (text) => text.split('\n').map((l, i) => [i + 1, l]).filter(([, l]) => l.includes(HOME_PATH)).map(([n, l]) => `${n}: ${l.slice(0, 160)}`);

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'once'));

test('no committed file under ui/ names an absolute home path', () => {
  const ls = spawnSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z', '.'], {cwd: ui, encoding: 'utf8'});
  expect(ls.status, ls.stderr).toBe(0);
  const files = ls.stdout.split('\0').filter((f) => f && !/\.(png|jpe?g|gif|webp|ico|woff2?)$/i.test(f) && fs.existsSync(path.join(ui, f)) && fs.statSync(path.join(ui, f)).isFile());
  expect(files.some((f) => /^ux\/rounds\/R-\d\d\/round\.json$/.test(f)), 'the generated round files are among the checked files').toBe(true);
  const bad = files.flatMap((f) => hits(fs.readFileSync(path.join(ui, f), 'utf8')).map((h) => `${f}:${h}`));
  expect(bad, 'absolute home paths in files git would commit').toEqual([]);
});
