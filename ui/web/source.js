// Where the page reads from. Served by ui/server.mjs, the page reads that server's routes (/api/*, /events)
// and this module changes nothing. Served by the daemon (`runbook serve --ui`), the page reads the daemon's
// views: the list of runs from GET /daemon/views/runs and the events of every run from the server-sent
// stream of GET /daemon/views/daemon-events. The daemon's runs carry no case store, so the routes that
// describe one answer an empty store; nothing is made up (the fields the daemon does not have are listed in
// the story ui-embedded-in-the-binary, "Slice 1").
import {esc} from './lenses.js';

// Which server sent this page: the daemon names itself in the Server-Timing header of every file it serves
// (read from the page's own navigation entry: no request, so a Node server sees no failed probe).
export const daemon = performance.getEntriesByType('navigation')[0]?.serverTiming?.some((t) => t.name === 'runbookd') ?? false;

const json = (p) => fetch(p).then((r) => (r.ok ? r.json() : Promise.reject(new Error(`${p}: ${r.status}`))));

// the runs seen so far, oldest first: a run keeps its index, so the page's selection holds when runs come
const order = [];
const events = new Map();
const ms = (t) => { const v = Date.parse(t); return Number.isNaN(v) ? null : v; };
const lastAt = (id, fallback) => ms(events.get(id)?.at(-1)?.at) ?? ms(fallback);

const DAEMON_STATE = {Submitted: 'run', Running: 'run', WaitingOnDecision: 'run', Ended: 'idle', Cancelled: 'idle', FailedToStart: 'bad'};

function sourceRow(r, index) {
  const words = [r.state, r.outcome, r.failure].filter(Boolean).join(' · ');
  return {index, name: `${r.runbook_id} ${r.run_id.slice(-8)}`, path: r.run_id, exists: true, version: 0, cases: 0, runDir: '', found: true,
    stopped: null, program: null, experiments: [], view: null, lastWrite: lastAt(r.run_id, r.submitted_at),
    run: {daemon: {cls: DAEMON_STATE[r.state] || 'idle', text: words, short: r.state}, started: ms(r.started_at), ended: ms(r.ended_at)},
    daemonRun: r};
}

async function sources() {
  const rows = (await json('/daemon/views/runs')).rows;
  for (const r of [...rows].reverse()) if (!order.includes(r.run_id)) order.push(r.run_id);
  const byId = new Map(rows.map((r) => [r.run_id, r]));
  return order.map((id, i) => sourceRow(byId.get(id), i));
}

const EMPTY = {
  '/api/meta': {},
  '/api/view': null,
  '/api/view-info': null,
  '/api/run': {proc: null, settings: null, hasLog: false, runDir: ''},
  '/api/workfacts': {files: []},
  '/api/products': {served: false},
};

// The open decision requests of the selected run, from the daemon's view open-decisions (rows as the daemon
// has them: request_id, run_id, question, options, opened_at). The decisions made and the grounds are not in
// the daemon's views, so the history stays empty. ui/daemon/decide.js (only the daemon serves it) shows them and sends the answers.
async function inbox(index) {
  const run = order[index];
  const rows = run ? (await json(`/daemon/views/open-decisions?run_id=${encodeURIComponent(run)}`)).rows : [];
  return {write: true, principal: null, daemon: true, run, items: rows.map((r) => ({...r, id: r.request_id})), history: [], missing: [], missingText: null, stopped: null,
    tag: 'Runbook daemon · you answer a decision from the Decisions drawer · the daemon records it'};
}

async function daemonApi(p) {
  const {pathname, searchParams} = new URL(p, location.href);
  if (pathname === '/api/sources') return sources();
  if (pathname === '/api/inbox') return inbox(Number(searchParams.get('source') || 0));
  if (pathname === '/api/tree') return {source: Number(searchParams.get('source') || 0), version: 0, now: Date.now(), exists: true, present: [], cases: []};
  if (pathname === '/api/live') return {now: Date.now(), fs: {present: false, changes: []}};
  if (pathname in EMPTY) return EMPTY[pathname];
  throw new Error(`${p}: the daemon has no equal of this route yet`);
}

export const api = (p) => (daemon ? daemonApi(p) : json(p));

// The daemon's stream, read with fetch so that every kind of event arrives (an EventSource listens to
// named kinds only, and a kind is open text). It has the shape of the EventSource the page listens to.
function daemonStream() {
  const handlers = {};
  const on = {};
  let last = 0, closed = false;
  const emit = (name, data) => (handlers[name] || []).forEach((f) => f({data: JSON.stringify(data)}));
  const up = (batch) => {
    const runs = new Set();
    for (const row of batch) {
      if (!events.has(row.run_id)) events.set(row.run_id, []);
      events.get(row.run_id).push(row);
      runs.add(row.run_id);
    }
    emit('sources', {count: order.length});
    for (const id of runs) if (order.includes(id)) emit('change', {source: order.indexOf(id), version: 0, cases: [], run: true, live: true});
  };
  again = () => {
    emit('sources', {count: order.length});
    order.forEach((_, source) => emit('change', {source, version: 0, cases: [], run: true, live: false}));
  };
  const timer = setInterval(() => { if (!closed && document.visibilityState === 'visible') again(); }, 2000);
  (async () => {
    while (!closed) {
      try {
        const r = await fetch(`/daemon/views/daemon-events?after_position=${last}`, {headers: {accept: 'text/event-stream'}});
        if (!r.ok) throw new Error(String(r.status));
        emit('hello', {versions: []});
        const reader = r.body.getReader();
        const text = new TextDecoder();
        let buf = '';
        for (;;) {
          const {done, value} = await reader.read();
          if (done) break;
          buf = (buf + text.decode(value, {stream: true})).replace(/\r\n/g, '\n');
          const batch = [];
          for (let i = buf.indexOf('\n\n'); i >= 0; i = buf.indexOf('\n\n')) {
            const block = buf.slice(0, i);
            buf = buf.slice(i + 2);
            const data = block.split('\n').filter((l) => l.startsWith('data:')).map((l) => l.slice(5).trimStart()).join('\n');
            if (data) { const row = JSON.parse(data); last = Math.max(last, row.position); batch.push(row); }
          }
          if (batch.length) up(batch);
        }
      } catch { /* reconnects below */ }
      if (closed) break;
      (on.error || (() => {}))();
      await new Promise((ok) => setTimeout(ok, 1000));
    }
  })();
  return {addEventListener: (name, f) => (handlers[name] ||= []).push(f), set onerror(f) { on.error = f; }, close() { closed = true; clearInterval(timer); }};
}

export const openEvents = () => (daemon ? daemonStream() : new EventSource('/events'));

// Read the daemon's views again: the run list and the open decisions of the selected run. The daemon records no
// event when a run starts to wait or goes on after an answer, so the page asks again after an answer of its own
// and every two seconds while it is visible. (No-op for a page the Node server serves.)
let again = () => {};
export const refresh = () => again();

const when = (t) => { const v = ms(t); return v == null ? '' : new Date(v).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'}); };
const short = (rec) => { const t = JSON.stringify(rec ?? null); return t === '{}' || t === 'null' ? '' : t.length > 160 ? `${t.slice(0, 159)}…` : t; };

// What the left side shows for a daemon's run in place of the case tree and the feed of changes: the run's
// own events, newest first, each with its place in the run and the record the runtime wrote.
export function daemonView(src) {
  if (!daemon || !src?.daemonRun) return null;
  const id = src.daemonRun.run_id;
  const list = [...(events.get(id) || [])].reverse();
  const rows = list.map((e) => { const position = Number(e.position), sequence = Number(e.sequence); return `<div class="fd-row" data-key="ev:${position}" data-run="${esc(e.run_id)}" data-sequence="${sequence}">`
    + `<div class="fd-1"><span class="fd-s">${esc(e.kind)}</span></div>`
    + (short(e.record) ? `<div class="fd-m">${esc(short(e.record))}</div>` : '')
    + `<div class="fd-2"><span class="fd-c">event ${sequence}</span><span class="fd-t" title="${esc(e.at)}">${esc(when(e.at))}</span></div></div>`; }).join('');
  return {
    feed: `<div class="fd-h" data-key="fdh"><span class="fd-title">Run events</span><span class="muted">${list.length} from the daemon, newest first</span></div>`
      + `<div class="fd-list" data-key="fdl" tabindex="0" role="region" aria-label="Run events" data-run="${esc(id)}">${rows}</div>`,
    tree: `<div class="empty" data-key="dnote"><p>${esc(src.daemonRun.runbook_id)} · ${esc(src.daemonRun.state)}${src.daemonRun.autonomy ? ` · autonomy ${esc(src.daemonRun.autonomy)}` : ''}${src.daemonRun.decider ? ` · decider ${esc(src.daemonRun.decider)}` : ''}</p>`
      + '<p class="muted">The daemon records events for a run, not a case store: the case tree, the graph and the phases fill in when it serves them.</p></div>',
  };
}
