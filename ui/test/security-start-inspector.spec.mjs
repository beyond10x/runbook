// Security review, pass 1, of the unit daemon-page-starts-a-run: the inspector (ui/server.mjs) stays the
// read-only watch mode. One server of its own on a port the kernel gives, over a store directory that does
// not exist (the server waits for it): no run is read. Requests are written by hand over node:http, so no
// client rewrites a path with `..` in it.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
let srv, port;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  const root = process.env.SEC_SCRATCH || path.join(ui, 'node_modules');
  ({proc: srv, port} = await startServer([path.join(root, '.pw-sec-inspector-no-such-store')]));
});
test.afterAll(() => srv?.kill());

function ask(method, rawPath, body = null, headers = {}) {
  return new Promise((resolve, reject) => {
    const req = http.request({host: '127.0.0.1', port, method, path: rawPath, headers: {...headers, ...(body ? {'content-type': 'application/json', 'content-length': Buffer.byteLength(body)} : {})}}, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => resolve({status: res.statusCode, text: Buffer.concat(chunks).toString('utf8'), headers: res.headers}));
    });
    req.on('error', reject);
    req.end(body ?? undefined);
  });
}

const START = JSON.stringify({runbook_id: 'sdc', intake: 'an invented intake', confinement: 'linux', idempotency_key: 'security-start-inspector-0123456789'});

test('the inspector serves neither the start script nor the decide script, under any spelling of the path', async () => {
  const startJs = fs.readFileSync(path.join(ui, 'daemon', 'start.js'), 'utf8');
  const spellings = [
    '/start.js', '/decide.js', '/daemon/start.js', '/daemon/decide.js', '/../daemon/start.js', '/..%2fdaemon%2fstart.js', '/..%2Fdaemon/start.js',
    '/%2e%2e/daemon/start.js', '/%2E%2E%2Fdaemon%2Fstart.js', '/..%5cdaemon%5cstart.js', '/web/../daemon/start.js', '/./../daemon/start.js', '//daemon/start.js',
    '/..//daemon/start.js', '/....//daemon/start.js', '/%252e%252e/daemon/start.js', '/fonts/../../daemon/start.js', '/vendor/..%2f..%2fdaemon/start.js',
    '/../daemon/start.js?source=0', '/../daemon/start.js#x', '/.%2e/daemon/start.js', '/..;/daemon/start.js', '/../daemon', '/../daemon/',
    '/../server.mjs', '/..%2fserver.mjs', '/../package.json', '/../test/security-start-inspector.spec.mjs',
  ];
  for (const p of spellings) {
    for (const method of ['GET', 'HEAD']) {
      const r = await ask(method, p);
      expect([400, 404], `${method} ${p} answered ${r.status}`).toContain(r.status);
      expect(r.text.includes('submit-run') || r.text.includes('answer-decision') || r.text === startJs, `${method} ${p} served code that sends a command`).toBe(false);
    }
  }
  // the page it does serve names no command and loads the act code only when the daemon named itself
  for (const p of ['/', '/app.js', '/source.js', '/index.html']) {
    const r = await ask('GET', p);
    expect(r.status, p).toBe(200);
    expect(r.text.includes('submit-run') || r.text.includes('/daemon/commands'), `${p} names a command`).toBe(false);
    expect(r.headers['server-timing'], `${p}: the inspector names itself the daemon`).toBeUndefined();
  }
});

test('the inspector takes no start, no list of runbooks and no token, by any method', async () => {
  for (const p of ['/daemon/commands/submit-run', '/daemon/views/runbooks', '/daemon/views/configuration', '/ui/login', '/start.js', '/', '/api/sources', '/api/start', '/api/submit', '/mcp']) {
    for (const method of ['POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS', 'PROPFIND']) {
      const r = await ask(method, p, method === 'OPTIONS' ? null : START, {authorization: 'Bearer some-token-0123456789-0123456789-01', origin: `http://127.0.0.1:${port}`});
      expect(r.status, `${method} ${p}`).toBe(405);
      expect(Object.keys(r.headers).filter((h) => h.startsWith('access-control-') || h === 'set-cookie' || h === 'location'), `${method} ${p}`).toEqual([]);
    }
  }
  for (const p of ['/daemon/commands/submit-run', '/daemon/views/runbooks', '/daemon/views/configuration', '/ui/login', '/mcp']) {
    const r = await ask('GET', p);
    expect(r.status, `GET ${p}`).toBe(404);
  }
  // the server's own text has no route to a daemon and starts no program
  const server = fs.readFileSync(path.join(ui, 'server.mjs'), 'utf8');
  for (const needle of ['/daemon/', 'submit-run', 'Bearer', 'authorization', 'RUNBOOK_TOKEN', 'child_process', 'writeFile', 'appendFile', "fetch('http", 'http.request']) {
    expect(server.includes(needle), `ui/server.mjs holds ${needle}`).toBe(false);
  }
});
