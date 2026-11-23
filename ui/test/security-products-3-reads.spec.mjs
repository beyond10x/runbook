// Pass 3 of the independent review of story inspector-shows-step-products, on the reads that fix round 2 left on the
// default limit of readText (64 MiB per file) and without the checks the file route has. Own server, run written by hand
// (products-fixture.mjs), invented text.
//   (a) a file that is a second name of a file outside the run is not read: console.txt of the run, which GET /api/run
//       answers verbatim (the file route, the listing and the blob route refuse a file with more than one link)
// The control measures the time a small request (GET /api/sources) takes while /api/run of a run without large files runs.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeProductsRun, SECRET_TEXT} from './products-fixture.mjs';
import {fixtureRoot, put, ask, sourceIndex} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-3-reads');
let srv, base;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => { srv?.kill(); fs.rmSync(root, {recursive: true, force: true}); });
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const runs = path.join(root, 'runs');

  const hard = path.join(runs, 'hard');
  writeProductsRun(hard);
  const outside = path.join(root, 'outside', 'secret.txt');
  put(outside, `${SECRET_TEXT}\n`);
  fs.linkSync(outside, path.join(hard, 'console.txt'));
  fs.linkSync(outside, path.join(hard, 'expected-llm-calls.txt'));

  const calm = path.join(runs, 'calm');
  writeProductsRun(calm);

  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', runs]));
});

const run = async (name) => ask(base, `/api/run?source=${await sourceIndex(base, name)}`, {timeout: 50000});

test('(a) a second name of a file outside the run is not read: console.txt and expected-llm-calls.txt of /api/run', async () => {
  const r = await run('hard');
  expect(r.status).toBe(200);
  expect(r.text.includes(SECRET_TEXT) ? 'the text of the other name is in the answer' : 'not in the answer').toBe('not in the answer');
});

async function stall(name) {
  const src = await sourceIndex(base, name);
  const big = ask(base, `/api/run?source=${src}`, {timeout: 55000});
  await new Promise((ok) => setTimeout(ok, 100));
  const ping = await ask(base, '/api/sources', {timeout: 55000});
  const r = await big;
  return {ping: ping.ms, big: r.ms, status: r.status, hung: ping.hung || r.hung};
}

test('(b) control: a run without large files answers /api/run and a small request beside it at once', async () => {
  const r = await stall('calm');
  console.log(`calm: /api/run ${r.big} ms, small request ${r.ping} ms`);
  expect(r.hung).toBe(false);
  expect(r.ping).toBeLessThan(3000);
});
