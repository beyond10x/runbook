// A synthetic daemon with a live run, for the specs of story run-page-watches-a-live-run (daemon-live.spec.mjs,
// daemon-steps-model.spec.mjs, ux/specs/daemon.spec.mjs). Not a spec. It answers the views the page served by
// `runbook serve --ui` reads (runs, cases, products, open-decisions, routes, runbooks, configuration and the event
// stream daemon-events) in the daemon's shapes (docs/daemon-api.md), and serves ui/web/ and ui/daemon/ with the
// daemon's Server-Timing name and a content security policy of the same form as the daemon's
// (runbook/src/ui_embed.rs), so the page takes its operate mode. Every id, digest, name and number is made up here;
// nothing comes from a recorded run.
//
// The run: the runbook `investigate`, one case, six steps done (one ended with exit 10, one made a model call), each
// between the engine's own lines (`Evaluated` before a step, `Stopped` with `BudgetSpent`/`MaxSteps` after it: the
// runtime runs each step as one engine call; that stop ends the step, not the run), and the seventh step,
// `findings.judge`, started and not finished. Beside it an ended run of the same runbook. T0 is the submit of the
// live run and RUNNING_SINCE the start of the step in flight, before a shift: a spec anchors the times to its own
// now (`/__fixture/anchor`), the model spec reads them unshifted.
//
// As a program (`node test/daemon-live-fixture.mjs [--port N] [--live] [--hold N]`) it prints
// `runbook: listening on http://127.0.0.1:<port>` as the daemon does. --live moves every time so that the step in
// flight started 132 s before the server started (for screenshots on a clock that runs). --hold N keeps the last N
// events of the live run back; each GET /__fixture/next sends the next of them (a test moves the run; the page never
// asks for it). Every other method than GET or HEAD is answered 405: the fixture takes no command.
import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
export const T0 = Date.parse('2026-11-26T18:53:00.000Z');
export const LIVE = '01a2f000-0000-7000-8000-00000000a1b2';
export const DONE = '01a2e000-0000-7000-8000-00000000c3d4';
export const ROUTE = 'local-model';

// fake digests and ids: the same input always gives the same text
const hex = (s, n = 64) => crypto.createHash('sha256').update(s).digest('hex').slice(0, n);
const uuid = (s) => { const h = hex(s, 32); return `${h.slice(0, 8)}-${h.slice(8, 12)}-8${h.slice(13, 16)}-8${h.slice(17, 20)}-${h.slice(20, 32)}`; };
const iso = (ms) => new Date(ms).toISOString();
const isoS = (ms) => `${new Date(ms).toISOString().slice(0, 19)}Z`;

// the steps of the runbook: action, seconds it ran, exit, model call (input, output tokens, micro-USD)
export const STEPS = [
  {action: 'subject.read', secs: 0.12, exit: 0},
  {action: 'evidence.deploy', secs: 0.06, exit: 0},
  {action: 'evidence.tickets', secs: 4.0, exit: 0},
  {action: 'evidence.repo', secs: 9.5, exit: 10},
  {action: 'evidence.chat', secs: 0.35, exit: 0},
  {action: 'report.write', secs: 2.1, exit: 0, call: {input: 1200, output: 800, micro: 4200}},
  {action: 'findings.judge', secs: 61, exit: 0, call: {input: 1677, output: 2548, micro: 9100}},
];

// The events of one run, from its submit at t0: {kind, at (ms), record}. `live`: the last step has started and not
// finished.
function runEvents(run, t0, live) {
  const out = [];
  let seq = 0;
  let prev = hex(`${run}-start`);
  const loom = (kind, at, rest) => {
    const record = {seq, ...(seq ? {prev} : {}), at: isoS(at), kind, ...rest};
    prev = hex(`${run}-${seq}`);
    seq++;
    out.push({kind, at, record});
  };
  const own = (kind, at, record) => out.push({kind, at, record});
  own('runbook.submitted', t0, {run_id: run, runbook_id: 'investigate', confinement: 'local', confinement_source: 'submit', model_route: ROUTE, model_route_alias: ROUTE, model_route_source: 'run'});
  let t = t0 + 600;
  own('runbook.pinned', t, {runbook: 'investigate', protocol: 'investigate', revision: 1, bindings_sha256: hex(`${run}-bindings`), copy_sha256: hex(`${run}-copy`)});
  loom('CaseOpened', t, {case: run, protocol: 'investigate@1'});
  STEPS.forEach((s, i) => {
    const last = i === STEPS.length - 1;
    t += 60;
    loom('Evaluated', t, {case_revision: 1, frontier: uuid(`${run}-f${i}`), decision_digest: hex(`${run}-d${i}`)});
    t += 20;
    own('runbook.action-started', t, {case: run, action: s.action, role: 'program', try: 1});
    if (live && last) return;
    t += Math.round(s.secs * 1000);
    loom('ActionFinished', t, {action: s.action, exit: s.exit, blob_digest: hex(`${run}-b${i}`), evidence: [uuid(`${run}-e${i}`)], revisions: []});
    t += 15;
    loom('Stopped', t, {stop: 'BudgetSpent', next: [], budget: 'MaxSteps'});
    if (s.call) { t += 90; own('runbook.model-call', t, {what: 'model call', run, step: s.action, role: 'default', route: ROUTE, alias: ROUTE, source: 'run', wire: 'messages', gateway_call: `${hex(`${run}-g${i}`, 32)}-1`, input_tokens: s.call.input, output_tokens: s.call.output, cost_known: true, micro_usd: s.call.micro}); }
  });
  if (!live) {
    t += 30;
    loom('Evaluated', t, {case_revision: 1, frontier: uuid(`${run}-fend`), decision_digest: hex(`${run}-dend`)});
    t += 25;
    loom('Terminated', t, {reason: 'investigated'});
  }
  return out;
}

// The whole state at a shift of `shift` ms (0: the times above). {runs, events, cases, products, routes, runningSince}
export function fixture(shift = 0) {
  const doneT0 = T0 - 30 * 60000 + shift, liveT0 = T0 + shift;
  const done = runEvents(DONE, doneT0, false);
  const live = runEvents(LIVE, liveT0, true);
  let position = 0;
  const rows = [];
  for (const [run, list] of [[DONE, done], [LIVE, live]]) list.forEach((e, i) => rows.push({position: ++position, run_id: run, sequence: i + 1, kind: e.kind, at: iso(e.at), record: e.record}));
  const route = {model_route: ROUTE, model_route_alias: ROUTE, model_route_source: 'run'};
  const runs = [
    {run_id: LIVE, runbook_id: 'investigate', state: 'Running', confinement: 'local', confinement_source: 'submit', ...route, submitted_at: iso(liveT0), started_at: iso(liveT0 + 600)},
    {run_id: DONE, runbook_id: 'investigate', state: 'Ended', confinement: 'local', confinement_source: 'submit', ...route, submitted_at: iso(doneT0), started_at: iso(doneT0 + 600), ended_at: iso(done.at(-1).at), outcome: 'investigated'},
  ];
  const cases = {
    [LIVE]: [{case_id: LIVE, protocol: 'investigate', case_state: 'Running'}],
    [DONE]: [{case_id: DONE, protocol: 'investigate', case_state: 'Ended', outcome: 'investigated'}],
  };
  const files = ['subject.yaml', 'evidence/deploy.json', 'evidence/tickets.json', 'evidence/repo.txt', 'evidence/chat.json', 'report.md'];
  const products = Object.fromEntries([LIVE, DONE].map((r) => [r, {files: files.map((f, i) => ({path: f, size: 200 + 37 * i, modified_at: iso(liveT0 + 1000 * i)})), hidden: 0, left_out: 0}]));
  const routes = [{route: ROUTE, alias: ROUTE, kind: 'managed', status: 'ready', phase: 'ready', gateway: 'stand-in'}];
  const runningSince = live.at(-1).at;
  return {runs, events: rows, cases, products, routes, runningSince};
}
// the start of the step in flight, on the unshifted clock
export const RUNNING_SINCE = fixture().runningSince;

const TYPES = {'.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.woff2': 'font/woff2', '.svg': 'image/svg+xml', '.json': 'application/json'};
// the daemon's policy (runbook/src/ui_embed.rs `policy`): scripts from the page's origin and the inline scripts of
// index.html by their digest, styles from the origin and in attributes
function policy() {
  const index = fs.readFileSync(path.join(ui, 'web', 'index.html'), 'utf8');
  const hashes = [...index.matchAll(/<script>([\s\S]*?)<\/script>/g)].map((m) => ` 'sha256-${crypto.createHash('sha256').update(m[1]).digest('base64')}'`).join('');
  return `default-src 'none'; script-src 'self'${hashes}; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'`;
}
function file(rel) {
  if (rel.includes('..') || rel.includes('\0')) return null;
  for (const dir of ['web', 'daemon']) {
    const p = path.join(ui, dir, rel);
    try { if (fs.statSync(p).isFile()) return p; } catch { /* next */ }
  }
  return null;
}

// The shift that puts the start of the step in flight `ago` ms before now.
export const shiftFor = (ago = 132000) => Date.now() - ago - RUNNING_SINCE;

// Serve the fixture on 127.0.0.1:port. Resolves {server, base, close}. live: the times shifted by shiftFor() at the
// start; GET /__fixture/anchor?ago=<ms> shifts them again from now (a page's clock cannot be faked here: a faked
// `performance` hides the Server-Timing name the page reads to know the daemon served it). metrics: per run id the
// answer of the view run-metrics, {status, body}; a run not named is answered with no row (the fixture holds no metrics).
export function serve({port = 0, live = false, hold = 0, metrics = {}} = {}) {
  let f = fixture(live ? shiftFor() : 0);
  let all = f.events;
  let shown = all.length - hold;
  const streams = new Set();
  const csp = policy();
  const json = (res, body, status = 200) => { res.writeHead(status, {'content-type': 'application/json'}); res.end(JSON.stringify(body)); };
  const sse = (row) => `id: ${row.position}\nevent: ${row.kind}\ndata: ${JSON.stringify(row)}\n\n`;
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method !== 'GET' && req.method !== 'HEAD') return json(res, {refused: 'method not allowed', message: 'the fixture takes no command'}, 405);
    const run = url.searchParams.get('run_id');
    const p = url.pathname;
    if (p === '/__fixture/anchor') {
      f = fixture(shiftFor(Number(url.searchParams.get('ago') ?? 132000)));
      all = f.events;
      return json(res, {running_since: f.runningSince});
    }
    if (p === '/__fixture/next') {
      if (shown < all.length) { const row = all[shown++]; for (const s of streams) s.write(sse(row)); }
      return json(res, {shown, of: all.length});
    }
    if (p === '/daemon/views/runs') return json(res, {rows: f.runs});
    if (p === '/daemon/views/cases') return f.cases[run] ? json(res, {rows: f.cases[run]}) : json(res, {refused: 'no such run'}, 404);
    if (p === '/daemon/views/products') return f.products[run] ? json(res, {rows: [f.products[run]]}) : json(res, {refused: 'no such run'}, 404);
    if (p === '/daemon/views/product') return json(res, {refused: 'no such file', message: 'the fixture serves no file'}, 404);
    if (p === '/daemon/views/open-decisions') return json(res, {rows: []});
    if (p === '/daemon/views/routes') return json(res, {rows: f.routes});
    if (p === '/daemon/views/runbooks') return json(res, {rows: [{runbook_id: 'investigate', runtime: 'loom'}]});
    if (p === '/daemon/views/configuration') return json(res, {rows: [{confinement_floor: 'local'}]});
    if (p === '/daemon/views/run-metrics') return Object.hasOwn(metrics, run) ? json(res, metrics[run].body, metrics[run].status ?? 200) : json(res, {rows: []});
    if (p === '/daemon/views/daemon-events' || p === '/daemon/views/events') {
      const after = Number(req.headers['last-event-id'] ?? url.searchParams.get('after_position') ?? 0);
      const pick = (row) => row.position > after && (p === '/daemon/views/daemon-events' || row.run_id === run);
      if (!/text\/event-stream/.test(req.headers.accept || '')) return json(res, {rows: all.slice(0, shown).filter(pick)});
      res.writeHead(200, {'content-type': 'text/event-stream', 'cache-control': 'no-store'});
      res.write(all.slice(0, shown).filter(pick).map(sse).join(''));
      streams.add(res);
      req.on('close', () => streams.delete(res));
      return undefined;
    }
    if (p.startsWith('/daemon/')) return json(res, {refused: 'not found', message: 'no such route'}, 404);
    const at = file(p === '/' ? 'index.html' : decodeURIComponent(p.slice(1)));
    if (!at) return json(res, {refused: 'not found', message: 'no such route'}, 404);
    res.writeHead(200, {'content-type': TYPES[path.extname(at)] || 'application/octet-stream', 'server-timing': 'runbookd', 'content-security-policy': csp, 'x-content-type-options': 'nosniff', 'cache-control': 'no-store'});
    res.end(req.method === 'HEAD' ? undefined : fs.readFileSync(at));
    return undefined;
  });
  return new Promise((resolve) => server.listen(port, '127.0.0.1', () => {
    const base = `http://127.0.0.1:${server.address().port}`;
    server.on('close', () => { for (const s of streams) s.end(); });
    resolve({server, base, close: () => { for (const s of streams) s.destroy(); server.close(); }});
  }));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const opt = (k, d) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : d; };
  const {base} = await serve({port: Number(opt('--port', 0)), live: args.includes('--live'), hold: Number(opt('--hold', 0))});
  console.log(`runbook: listening on ${base}`);
}
