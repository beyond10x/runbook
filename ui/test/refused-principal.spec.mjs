// A write server is never started for a principal that names no person: server.mjs refuses
// --principal non-interactive and exits 2 (it needs no driver and no run: the committed fixture stands in).
import {test, expect} from '@playwright/test';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'once'));

test('a refused principal: non-interactive cannot start the server', () => {
  const r = spawnSync(process.execPath, ['server.mjs', '--port', '0', '--write', '--principal', 'non-interactive', path.join(here, 'recorded/trial-todo-1-slim/store')], {cwd: ui, encoding: 'utf8', timeout: 15000});
  expect(r.status).toBe(2);
  expect(r.stderr).toMatch(/non-interactive/);
});
