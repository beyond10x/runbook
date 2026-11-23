// The start form of a page served by the daemon (`runbook serve --ui`): the page's second mode, "operate". The
// person picks a runbook from the daemon's view runbooks, writes the intake as text and picks the confinement
// (from the levels at or above the daemon's floor; no level is preset and Start stays disabled until one is
// picked); Start sends the daemon's own command submit-run.
//
// What the page sends: POST /daemon/commands/submit-run, Content-Type: application/json, an idempotency_key
// that belongs to one filled form, and Authorization: Bearer <token> when the daemon has a token. The key is
// made when the form is first sent; it stays the same for a press repeated after an answer that did not arrive
// (a network error, a reset), so the daemon's replay rule returns the first run and no second one starts; it
// changes when the daemon answered (accepted or refused) and when the runbook, the intake or the confinement
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
const T = {needs: null, token: null, busy: false, floor: null, rows: [], msg: null, key: null, maybe: false};
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
  $('st-start').disabled = T.busy || !level.value;
}

// The daemon's refusal as one line: its name, its status and its text. A refused command has the body
// {outcome, error, payload}; an answer of the HTTP layer (401, 400, 415) has {refused, message}.
async function refusal(r) {
  let body = null;
  try { body = await r.json(); } catch { /* not JSON */ }
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
  if (T.needs && T.token === null) { paint(); tokenInput.focus(); return say(false, 'This daemon needs its token: type it above, once, and press Start again.'); }
  const key = keyFor(runbook, intake, level);
  T.busy = true;
  say(true, 'Sending the run to the daemon…');
  try {
    const r = await fetch(PATH, {
      method: 'POST',
      credentials: 'omit',
      headers: {'content-type': 'application/json', ...(T.token !== null ? {authorization: `Bearer ${T.token}`} : {})},
      body: JSON.stringify({runbook_id: runbook, intake, confinement: level, idempotency_key: key.value}),
    });
    T.busy = false;
    // the daemon answered, accepted or refused: its key is used up
    T.key = null;
    if (!r.ok) {
      if (r.status === 401) { T.token = null; T.needs = true; }
      return say(false, await refusal(r));
    }
    const run = (await r.json())?.published?.[0]?.payload?.run_id;
    if (!run) return say(false, 'The daemon took the run but named none.');
    T.msg = null;
    T.maybe = false;
    $('st-intake').value = '';
    paint();
    forget();
    dlg.close();
    if (openRun) await openRun(String(run));
  } catch {
    // no answer: the daemon may have taken the run, and the key stays for the next press of this form; the text
    // of the engine's error is not shown, since an engine may put the request into it
    T.busy = false;
    key.lost = true;
    say(false, 'The daemon could not be reached, so the page does not know whether it took the run. Press Start again with the same content: at most one run is started.');
  }
}

async function open() {
  T.msg = null;
  dlg.showModal();
  paint(true);
  await probe();
  try { T.floor = (await rowsOf(CONFIGURATION))[0]?.confinement_floor ?? null; } catch { T.floor = null; }
  try { T.rows = await rowsOf(RUNBOOKS); } catch (e) { T.rows = []; T.msg = {ok: false, text: `The list of runbooks could not be read: ${e.message}`}; }
  paint(true);
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
  dlg.addEventListener('change', (e) => { if (e.target.closest('#st-confinement')) paint(); });
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
