// A run directory is data from elsewhere. A line of an event log that is valid JSON and no event, and a run
// that cannot be read, must not end the inspector: the scan at start-up and the periodic rescan run outside
// the request guard. One server over several runs: the start-up scan reads them all, the timer reads them again.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {FIXTURE_DIR} from './fixture-dir.mjs';
import {startServer} from './serve.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(FIXTURE_DIR, 'event-log-lines');
const EVENT = (seq, event = 'opened') => JSON.stringify({seq, at: '2026-01-01T00:00:00Z', event});
const NOT_EVENTS = {'null-line': 'null', 'string-line': '"opened"', 'number-line': '42', 'array-line': '[1,2]', 'no-seq-line': '{"event":"opened"}', 'text-seq-line': '{"seq":"1","event":"opened"}'};
let base, srv, stderr = '';

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());

const sources = async (request) => (await (await request.get(`${base}/api/sources`)).json());
const indexOf = (rows, name) => rows.find((r) => r.name === name)?.index;
const tree = async (request, rows, name) => (await request.get(`${base}/api/tree?source=${indexOf(rows, name)}`));

test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  fs.mkdirSync(path.join(root, 'runs'), {recursive: true});
  const runs = path.join(root, 'runs');
  const make = (name, lines) => { const c = path.join(runs, name, 'store', 'C1'); fs.mkdirSync(c, {recursive: true}); fs.writeFileSync(path.join(c, 'case.json'), '{"id":"C1"}\n'); fs.writeFileSync(path.join(c, 'events.jsonl'), lines.join('\n') + '\n'); };
  make('good-run', [EVENT(1), EVENT(2, 'terminated')]);
  for (const [name, line] of Object.entries(NOT_EVENTS)) make(name, [EVENT(1), line, EVENT(2, 'terminated')]);
  make('only-null', ['null']);
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', runs]));
  srv.stderr.on('data', (d) => { stderr += d; });
  // a run that cannot be read appears while the server runs: more event lines than one call can take
  make('too-long-run', Array.from({length: 200000}, (_, i) => EVENT(i + 1)));
  await new Promise((r) => setTimeout(r, 2500));
});

test('a line that is no event (null, string, number, array, no seq, a seq that is no number) does not end the server', async ({request}) => {
  const rows = await sources(request);
  for (const name of [...Object.keys(NOT_EVENTS), 'only-null']) expect(indexOf(rows, name), name).not.toBeUndefined();
  expect((await request.get(`${base}/api/sources`)).status()).toBe(200);
});

test('the events around a line that is no event are read, the line is skipped', async ({request}) => {
  const rows = await sources(request);
  for (const name of Object.keys(NOT_EVENTS)) {
    const res = await tree(request, rows, name);
    expect(res.status(), name).toBe(200);
    const events = (await res.json()).cases[0].events;
    expect(events.map((e) => e.seq), name).toEqual([1, 2]);
  }
  const only = await (await tree(request, rows, 'only-null')).json();
  expect(only.cases[0].events).toEqual([]);
});

test('a run that cannot be read does not stop the server: it is listed, and the other run is served', async ({request}) => {
  const rows = await sources(request);
  expect(indexOf(rows, 'too-long-run')).not.toBeUndefined();
  const good = await tree(request, rows, 'good-run');
  expect(good.status()).toBe(200);
  expect((await good.json()).cases[0].events.map((e) => e.seq)).toEqual([1, 2]);
  const run = rows.find((r) => r.name === 'good-run').run;
  expect(run.state).toBe('closed');
});

test('a run that cannot be read is logged once, by the name of its directory', async () => {
  const logged = stderr.split('\n').filter((l) => l.includes('too-long-run'));
  expect(logged).toHaveLength(1);
  expect(stderr).not.toContain(root);
});
