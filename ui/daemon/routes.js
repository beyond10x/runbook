// The model routes of a page served by the daemon (`runbook serve --ui`; docs/model-routes.md, section 6): the
// Routes view lists each route the daemon declares with its alias, kind, gateway, status and phase from the daemon's
// view routes (GET /daemon/views/routes), and the header of a run on a route names its kind and status from the same
// rows. The daemon reads its gateways at most once every 15 s and answers from that read; this page asks the view at
// most once every 15 s as well, whatever the answer (a refusal, a network error, rows), and only while it shows a run
// on a route or the Routes view is open. A read that fails keeps the rows of the last read and says so.
// The daemon serves no count of today's calls or booked spend per route: the view says so and shows none.
// Text from the daemon is set as text nodes, never as markup. This file is served by the daemon only.

const VIEW = '/daemon/views/routes';
export const EVERY_MS = 15000;
// R.rows: the rows of the last read that answered (null: none yet). R.tried: when the last read started (page clock).
// R.error: what the last read got when it got no rows. R.reading: a read runs.
const R = {rows: null, tried: null, error: null, reading: false};
let changed = () => {};
let dlg = null;
let timer = 0;

function h(tag, props = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (v === false || v == null) continue;
    if (k === 'class') el.className = v;
    else if (k === 'text') el.textContent = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const kid of kids) if (kid != null) el.append(kid);
  return el;
}

const $ = (id) => document.getElementById(id);

// One read of the view, unless one runs or the last one started less than EVERY_MS ago.
async function read() {
  const now = Date.now();
  if (R.reading || (R.tried != null && now - R.tried >= 0 && now - R.tried < EVERY_MS)) return;
  R.reading = true;
  R.tried = now;
  try {
    const r = await fetch(VIEW);
    if (!r.ok) throw new Error(`the daemon answered ${r.status}`);
    const body = await r.json();
    if (!Array.isArray(body?.rows)) throw new Error('the daemon answered no rows');
    R.rows = body.rows;
    R.error = null;
  } catch (e) {
    R.error = String(e?.message || e);
  } finally {
    R.reading = false;
  }
  paint();
  changed();
}

const word = (v) => (typeof v === 'string' ? v : '');

// What a status says in the header when the gateway gave no phase.
const STATUS = {'gateway-unreachable': 'gateway unreachable', 'gateway-not-ready': 'gateway not ready', 'model-unknown': 'model unknown at the gateway', listed: 'listed, no status', 'not-ready': 'not ready', ready: 'ready'};

// The header's words for the route `name` of a run: its kind and its status (the model's phase when the gateway gave
// one) from the last read, or why they are not known. Asking starts a read when the last is EVERY_MS old.
export function routeWords(name) {
  if (!name) return null;
  read();
  const row = R.rows?.find((r) => r.route === name);
  if (row) return {kind: word(row.kind), status: word(row.phase) || STATUS[word(row.status)] || word(row.status)};
  if (R.rows) return {missing: 'not among the routes the daemon lists'};
  return {missing: R.error ? `status not read: ${R.error}` : 'status not read yet'};
}

function paint() {
  if (!dlg) return;
  const body = $('routes-body');
  const note = $('routes-note');
  const msg = $('routes-msg');
  const rows = R.rows ?? [];
  body.replaceChildren(...rows.map((r) => h('tr', {'data-route': word(r.route)},
    ...['route', 'alias', 'kind', 'gateway', 'status', 'phase'].map((f) => h('td', {'data-field': f, text: word(r[f])})),
    h('td', {'data-field': 'wires', text: Array.isArray(r.wires) ? r.wires.filter((w) => typeof w === 'string').join(', ') : ''}),
    h('td', {'data-field': 'read_at', text: word(r.read_at)}))));
  note.textContent = R.rows == null ? (R.reading || !R.error ? 'Reading the routes of the daemon.' : '')
    : R.rows.length ? `${R.rows.length} route${R.rows.length === 1 ? '' : 's'} the daemon declares. The daemon reads its gateways at most once every 15 s; this view asks it again every 15 s while it is open.`
      : 'The daemon declares no model route.';
  msg.textContent = R.error ? `The routes could not be read: ${R.error}.${R.rows ? ' The rows are those of the last read that answered.' : ''}` : '';
  msg.className = R.error ? 'res bad' : '';
}

function close() {
  clearInterval(timer);
  timer = 0;
  dlg.close();
}

function open() {
  dlg.showModal();
  paint();
  read();
  clearInterval(timer);
  timer = setInterval(read, EVERY_MS);
}

function build() {
  const head = h('tr', {}, ...['Route', 'Alias', 'Kind', 'Gateway', 'Status', 'Phase', 'Wires', 'Read at'].map((t) => h('th', {scope: 'col', text: t})));
  dlg = h('dialog', {id: 'routesdlg', 'aria-label': 'Model routes'},
    h('div', {class: 'mh'}, h('h3', {text: 'Model routes'}), h('button', {id: 'routes-close', class: 'ib wide', type: 'button', text: 'Close'})),
    h('p', {id: 'routes-note', class: 'muted'}),
    h('div', {class: 'routes-wrap'}, h('table', {class: 'routes'}, h('thead', {}, head), h('tbody', {id: 'routes-body'}))),
    h('p', {class: 'muted', text: 'Today\'s calls and booked spend per route: the daemon serves no count of them yet.'}),
    h('div', {id: 'routes-msg', role: 'status'}));
  dlg.addEventListener('click', (e) => { if (e.target.closest('#routes-close')) close(); });
  dlg.addEventListener('close', () => { clearInterval(timer); timer = 0; });
  document.body.append(dlg);
}

// Called once by the page when the daemon served it; `onChange` repaints the page when a read ended.
export function initRoutes(onChange) {
  changed = onChange;
  build();
  $('decbtn').before(h('button', {id: 'routesbtn', class: 'btn', type: 'button', text: 'Routes', title: 'the model routes of the daemon'}));
  $('routesbtn').addEventListener('click', open);
}
