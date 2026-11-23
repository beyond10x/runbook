// The start form of a page served by the daemon (`runbook serve --ui`): the page's second mode, "operate". The
// person picks a runbook from the daemon's view runbooks, writes the intake as text and picks the confinement
// (from the levels at or above the daemon's floor; no level is preset and Start stays disabled until one is
// picked); Start sends the daemon's own command submit-run. From the moment the form opens until it has read the
// floor of that opening, Start is disabled and a press sends nothing; an answer of an earlier press belongs to the
// opening that sent it and does not change what was typed into the form opened again.
//
// What the page sends: POST /daemon/commands/submit-run, Content-Type: application/json, an idempotency_key
// that belongs to one filled form, and Authorization: Bearer <token> when the daemon has a token. The key is
// made when the form is first sent; it stays the same for a press repeated after an answer that is not the
// command's whole answer (a network error, a reset, a body cut or not JSON, an answer of the HTTP layer, a 5xx), so
// the daemon's replay rule returns the first run and no second one starts; it changes when the daemon answered the
// command with a named outcome (accepted or refused), read whole, and when the runbook, the intake or the confinement
// changed (the page then says that the earlier press may have started a run). The cookie of the login is for
// reads: the command is sent without credentials. The token lives in the variable T.token of this module and
// nowhere else: not in a cookie, not in any storage of the browser, not in the URL, the history state or the
// DOM (the input is emptied as soon as it is read, and whenever the form closes). A token that a header cannot
// carry is forgotten at once and asked for again. A reload forgets it. A daemon without a token takes the
// command without a bearer and the page asks for none. Text from the daemon and from the person (a runbook
// name, the intake, a refusal) is set as text nodes, never as markup.
// This file is served by the daemon only: the inspector (ui/server.mjs) has no such file and no start form.

const PATH = '/daemon/commands/submit-run';
const RUNBOOKS = '/daemon/views/runbooks';
const CONFIGURATION = '/daemon/views/configuration';
const LEVELS = ['local', 'linux'];
// T.needs: does the daemon want a token (null: not asked yet). T.token: the token the person gave, in memory.
// T.key: the idempotency key of the filled form {value, sig, lost}: sig is its runbook, intake and level, lost
// says that a press with it got no answer. T.maybe: an earlier press with other content got no answer.
// T.opening: the number of the last opening of the form. T.ready: the floor of this opening was read and painted
// (until then Start is disabled and a press sends nothing: the levels offered are those of an earlier opening).
// T.picked: the runbook the person picked last; T.gone: it left the list that this opening read.
const T = {needs: null, token: null, busy: false, floor: null, rows: [], msg: null, key: null, maybe: false, opening: 0, ready: false, picked: null, gone: false};
let openRun = null;
let dlg = null;

const REFUSED = {
  'no-runbook': 'The daemon has no runbook of this name any more: close the form and open it again for the list.',
  'intake-too-large': 'The intake is larger than 1 MiB.',
  'below-floor': 'The confinement is below the daemon floor.',
  'context-outside-roots': 'The context is outside the directories the daemon may read.',
  'no-session': 'The daemon has no such session.',
  'session-closed': 'The session is closed.',
  'no-daemon': 'The daemon has no configuration row.',
};

function h(tag, props = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (v === false || v == null) continue;
    if (k === 'class') el.className = v;
    else if (k === 'text') el.textContent = v;
    else if (k in el && k !== 'list') el[k] = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const kid of kids) if (kid != null) el.append(kid);
  return el;
}

const $ = (id) => document.getElementById(id);

// a key no earlier form used: 128 random bits
function freshKey() {
  return `page-${Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, '0')).join('')}`;
}

// the key of this filled form: the one it has, or a new one when it has none or its content changed
function keyFor(runbook, intake, level) {
  const sig = JSON.stringify([runbook, intake, level]);
  if (T.key && T.key.sig === sig) return T.key;
  if (T.key?.lost) T.maybe = true;
  T.key = {value: freshKey(), sig, lost: false};
  return T.key;
}

// a token typed and not sent leaves the field
const forget = () => { $('st-token').value = ''; };

// whether a header can carry the token: the check fetch makes, made before any request and without its message
function sendable(token) {
  try { new Headers({authorization: `Bearer ${token}`}); return true; } catch { return false; }
}

async function rowsOf(path, init) {
  const r = await fetch(path, init);
  if (!r.ok) throw new Error(`${path}: ${r.status}`);
  return (await r.json()).rows;
}

// Whether the daemon wants a token: a read without credentials is answered 401 by a daemon that has one.
async function probe() {
  if (T.needs !== null) return;
  try { T.needs = (await fetch(CONFIGURATION, {credentials: 'omit'})).status === 401; } catch { T.needs = false; }
}

const say = (ok, text) => { T.msg = text ? {ok, text} : null; paint(); };

function levels() {
  const at = LEVELS.indexOf(T.floor);
  return at < 0 ? [LEVELS[LEVELS.length - 1]] : LEVELS.slice(at);
}

function note() {
  const floor = LEVELS.includes(T.floor)
    ? `The floor of this daemon is ${T.floor}: pick a confinement at or above it.`
    : 'The floor of this daemon could not be read: only the strictest level is offered.';
  if (!T.rows.length) return `The daemon lists no runbook it can start. ${floor}`;
  return `${T.rows.some((r) => r.runtime === 'stub')
    ? 'This daemon runs the stub runtime: the run is scripted and nothing is executed.'
    : 'The run is executed by this daemon on the Loom runtime.'} ${floor}`;
}

// Called whenever the form's data changed: it rewrites what the data decides and leaves what the person typed.
// No level is ever preset: a level stays only when the person picked it and it is still offered.
function paint(reset = false) {
  if (!dlg) return;
  const pick = $('st-runbook');
  const had = pick.value;
  pick.replaceChildren(...T.rows.map((r) => h('option', {value: String(r.runbook_id), text: r.runbook_version ? `${r.runbook_id} · version ${r.runbook_version}` : String(r.runbook_id)})));
  if (T.rows.some((r) => String(r.runbook_id) === had)) pick.value = had;
  // a runbook that left the list is not replaced by the first one: none is selected until the person picks
  if (T.gone) pick.selectedIndex = -1;
  const level = $('st-confinement');
  const was = level.value;
  const offered = levels();
  level.replaceChildren(...offered.map((l) => h('option', {value: l, text: l})));
  level.value = !reset && offered.includes(was) ? was : '';
  $('st-tokenrow').hidden = !(T.needs && T.token === null);
  $('st-note').textContent = note();
  $('st-maybe').textContent = T.maybe ? 'An earlier press of Start, with other content, got no answer and may have started a run: look in the list of runs before you go on.' : '';
  const msg = $('st-msg');
  msg.className = T.msg ? `res ${T.msg.ok ? 'ok' : 'bad'}` : '';
  msg.textContent = T.msg ? T.msg.text : '';
  $('st-start').disabled = T.busy || !T.ready || T.gone || !level.value;
}

// The daemon's refusal as one line: its name, its status and its text. A refused command has the body
// {outcome, error, payload}; an answer of the HTTP layer (401, 400, 415) has {refused, message}.
function refusal(r, body) {
  const name = body?.outcome || body?.refused || r.statusText || 'refused';
  const why = REFUSED[body?.outcome] || body?.message || body?.error || '';
  return `${name} (${r.status})${why ? `: ${why}` : ''}`;
}

async function start() {
  const tokenInput = $('st-token');
  // the token leaves the field first, on every path below: into the memory of the page, never left in the DOM
  const given = tokenInput.value;
  tokenInput.value = '';
  if (given) {
    // a token a header cannot carry is forgotten at once: the form asks for it again, and the message does not repeat it
    if (!sendable(given)) { T.token = null; paint(); tokenInput.focus(); return say(false, 'The token cannot be sent: it holds a character that an HTTP header cannot carry. Type it again.'); }
    T.token = given;
  }
  const runbook = $('st-runbook').value;
  const intake = $('st-intake').value;
  const level = $('st-confinement').value;
  if (!runbook) return say(false, 'Pick a runbook first.');
  if (!intake.trim()) return say(false, 'Write the intake first.');
  if (!level) return say(false, 'Pick a confinement first.');
  if (!levels().includes(level)) return say(false, `The confinement ${level} is below the floor of this daemon (${T.floor || 'unknown'}).`);
  // the levels offered while the floor is read are those of an earlier opening: nothing is sent until this opening has read it
  if (!T.ready) { T.msg = {ok: false, text: 'The form is still reading the floor of the daemon: press Start again when it is done.', reading: true}; return paint(); }
  if (T.needs && T.token === null) { paint(); tokenInput.focus(); return say(false, 'This daemon needs its token: type it above, once, and press Start again.'); }
  const opening = T.opening;
  const key = keyFor(runbook, intake, level);
  T.busy = true;
  say(true, 'Sending the run to the daemon…');
  let r = null;
  let body = null;
  try {
    r = await fetch(PATH, {
      method: 'POST',
      credentials: 'omit',
      headers: {'content-type': 'application/json', ...(T.token !== null ? {authorization: `Bearer ${T.token}`} : {})},
      body: JSON.stringify({runbook_id: runbook, intake, confinement: level, idempotency_key: key.value}),
    });
    // the whole body: a body that is cut on the way or is not JSON throws here, and the key stays
    body = await r.json();
  } catch {
    // r is null (no answer) or the body is unreadable; the text of the engine's error is not shown, since an
    // engine may put the request into it
    body = null;
  }
  T.busy = false;
  // Which answers end the key of this filled form: only an answer of the command itself, read whole and JSON, with a
  // 2xx or a 4xx status that names its outcome ({outcome: 'submitted'}, {outcome: 'no-runbook'}, ...): the daemon
  // looked at the command, so the key is used up whether it accepted or refused (the outcomes of submit-run are
  // 202, 409 and 422: runbook/src/commands.rs:130-138). Every other answer keeps it, so that the next press with
  // the same content sends the same key and the daemon's replay rule gives the first run back: no answer at all
  // (a network error, a reset), a body that is cut or is not JSON, an answer of the HTTP layer (401, 403, 400,
  // 415: the command was not looked at) and a failure of the daemon's own, a 5xx, whatever its body names (this
  // daemon's 5xx body is {refused, message} and names no outcome: runbook/src/api/routes.rs:72-75). The earlier
  // press is marked as possibly taken (key.lost) unless the HTTP layer refused it before the command: the note
  // about it then stays on the form for as long as the content changes. A 2xx that names an outcome and no run
  // is no answer this daemon sends: the key ends with it, and the form says that the daemon named no run.
  const named = typeof body?.outcome === 'string' && body.outcome !== '';
  const ended = named && r !== null && ((r.status >= 200 && r.status < 300) || (r.status >= 400 && r.status < 500));
  // an answer of an earlier opening: the form opened again is not the one that sent it
  const ours = opening === T.opening;
  // a run taken for an earlier opening leaves the key, which the form opened again may still hold with the same content: a
  // press of it gives the first run back
  if (ended && !(r.ok && !ours)) T.key = null;
  else if (!ended && !(r && [400, 401, 403, 415].includes(r.status))) key.lost = true;
  if (!r) return say(false, 'The daemon could not be reached, so the page does not know whether it took the run. Press Start again with the same content: at most one run is started.');
  if (!ended && r.ok) return say(false, 'The answer of the daemon could not be read, so the page does not know whether it took the run. Press Start again with the same content: at most one run is started.');
  if (!r.ok) {
    if (r.status === 401) { T.token = null; T.needs = true; }
    return say(false, `${refusal(r, body)}${ended || !key.lost ? '' : ' Press Start again with the same content: at most one run is started.'}`);
  }
  const run = body.published?.[0]?.payload?.run_id;
  if (!run) return say(false, 'The daemon took the run but named none.');
  if (!ours) {
    // what the person typed into the form opened again is theirs: the answer shows the run and changes nothing in the form
    paint();
    if (openRun) await openRun(String(run));
    return;
  }
  T.msg = null;
  T.maybe = false;
  $('st-intake').value = '';
  paint();
  forget();
  dlg.close();
  if (openRun) await openRun(String(run));
}

// Start stays disabled from here until the floor of this opening was read and painted (T.ready): a failed read
// leaves the strictest level as the only one offered, as before. Reads that end after a newer opening began are dropped.
async function open() {
  const mine = ++T.opening;
  T.msg = null;
  T.ready = false;
  T.gone = false;
  dlg.showModal();
  paint(true);
  await probe();
  let floor = null;
  try { floor = (await rowsOf(CONFIGURATION))[0]?.confinement_floor ?? null; } catch { floor = null; }
  if (mine !== T.opening) return;
  T.floor = floor;
  T.ready = true;
  if (T.msg?.reading) T.msg = null;
  // a level the person picked meanwhile stays picked when this floor offers it
  paint();
  let rows = [];
  let failed = null;
  try { rows = await rowsOf(RUNBOOKS); } catch (e) { failed = `The list of runbooks could not be read: ${e.message}`; }
  if (mine !== T.opening) return;
  T.rows = rows;
  T.msg = failed ? {ok: false, text: failed} : T.msg;
  // a runbook the person picked that this list no longer has is not replaced by another: none is selected and the form says so
  if (!failed && T.picked !== null && !rows.some((r) => String(r.runbook_id) === T.picked)) {
    T.msg = {ok: false, text: `The runbook you picked (${T.picked}) is no longer listed: pick another.`};
    T.gone = true;
    T.picked = null;
  }
  // not a reset: a level or a runbook the person picked since the first paint stays picked, when the floor and the
  // list just read still offer it (a level below the floor just read is dropped and Start stays disabled)
  paint();
  $('st-intake').focus();
}

function build() {
  dlg = h('dialog', {id: 'startdlg', 'aria-label': 'Start a run'},
    h('div', {class: 'mh'}, h('h3', {text: 'Start a run'})),
    h('label', {}, 'Runbook', h('select', {id: 'st-runbook'})),
    h('label', {}, 'Intake', h('textarea', {id: 'st-intake', class: 'tin', rows: 6, placeholder: 'What the run is for, as plain text'})),
    h('label', {}, 'Confinement', h('select', {id: 'st-confinement'})),
    h('label', {id: 'st-tokenrow', hidden: true}, 'Token', h('input', {id: 'st-token', class: 'tin', type: 'password', autocomplete: 'off', placeholder: 'the daemon token'})),
    h('p', {id: 'st-note', class: 'muted'}),
    h('p', {id: 'st-maybe', class: 'muted'}),
    h('div', {id: 'st-msg', role: 'status'}),
    h('div', {class: 'st-f'},
      h('button', {id: 'st-close', class: 'btn', type: 'button', text: 'Close'}),
      h('button', {id: 'st-start', class: 'btn primary', type: 'button', text: 'Start', disabled: true})));
  dlg.addEventListener('click', (e) => {
    if (e.target.closest('#st-close')) { forget(); dlg.close(); }
    else if (e.target.closest('#st-start')) start();
  });
  dlg.addEventListener('change', (e) => {
    if (e.target.closest('#st-runbook')) {
      T.picked = $('st-runbook').value || null;
      if (T.gone) { T.gone = false; T.msg = null; }
      paint();
    } else if (e.target.closest('#st-confinement')) paint();
  });
  // however the form closes (Close, Escape, a started run), a token typed and not sent does not stay in the field:
  // it is emptied before the form closes (Escape: its cancel event) and once more when the close is done
  dlg.addEventListener('cancel', forget);
  dlg.addEventListener('close', forget);
  document.body.append(dlg);
}

// Called once by the page when the daemon served it. `open` shows the run the daemon took.
export function initStart(show) {
  openRun = show;
  const mode = $('mode');
  mode.textContent = 'operate';
  mode.className = 'bm operate';
  mode.title = 'operate: this page starts runs and takes decisions';
  build();
  $('decbtn').before(h('button', {id: 'startbtn', class: 'btn', type: 'button', text: 'Start a run', onclick: open}));
}
