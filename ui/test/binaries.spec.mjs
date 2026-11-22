// Which canon and driver the server runs: SDC_UI_CANON and SDC_UI_DRIVE when set, else the default
// places; a program that is not there is a refusal that names its path. Own servers on ports the
// kernel gives (test/serve.mjs); nothing is run, the programs are files that exist or do not.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {writeSecondRunbook} from './second-runbook.mjs';
import {canonPath, drivePath} from '../binaries.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'binaries');
const servers = [];

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own servers, one viewport'));
test.afterAll(() => { for (const s of servers) s.kill(); });

test('the paths come from SDC_UI_CANON and SDC_UI_DRIVE, else the default places', () => {
  expect(canonPath({SDC_UI_CANON: '/x/canon', HOME: '/h'})).toBe('/x/canon');
  expect(canonPath({HOME: '/h'})).toBe('/h/beyond10x/canon/target/debug/canon');
  expect(drivePath('/repo', {SDC_UI_DRIVE: '/x/drive'})).toBe('/x/drive');
  expect(drivePath('/repo', {})).toBe('/repo/driver/target/debug/sdc-drive');
});

async function inbox(extraEnv, name) {
  fs.rmSync(path.join(root, name), {recursive: true, force: true});
  const store = writeSecondRunbook(path.join(root, name));
  const s = await startServer(['--write', '--principal', 'owner-1', store], {env: {...process.env, ...extraEnv}});
  servers.push(s.proc);
  return (await fetch(`${s.url}/api/inbox`)).json();
}

test('a canon and a driver that are not there are named in the refusal', async () => {
  const gone = {SDC_UI_CANON: path.join(root, 'no-such-dir/canon'), SDC_UI_DRIVE: path.join(root, 'no-such-dir/drive')};
  const ib = await inbox(gone, 'missing');
  expect(ib.missing.map((p) => p.replace(/^.*binaries\//, ''))).toEqual(['no-such-dir/drive', 'no-such-dir/canon']);
  expect(ib.missingText).toMatch(/drive \(.*binaries\/no-such-dir\/drive\)/);
  expect(ib.missingText).toMatch(/canon \(.*binaries\/no-such-dir\/canon\)/);
});

test('a canon and a driver that are there are not missing', async () => {
  const bin = path.join(root, 'bin');
  fs.mkdirSync(bin, {recursive: true});
  for (const n of ['canon', 'drive']) fs.writeFileSync(path.join(bin, n), '#!/bin/sh\nexit 0\n', {mode: 0o755});
  const ib = await inbox({SDC_UI_CANON: path.join(bin, 'canon'), SDC_UI_DRIVE: path.join(bin, 'drive')}, 'present');
  expect(ib.missing).toEqual([]);
});
