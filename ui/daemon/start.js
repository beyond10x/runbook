// The start form of a page served by the daemon (`runbook serve --ui`): the page's second mode, "operate". The
// person picks a runbook from the daemon's view runbooks, writes the intake as text and picks the confinement
// (never below the daemon's floor, which is the preset); Start sends the daemon's own command submit-run.
//
// What the page sends: POST /daemon/commands/submit-run, Content-Type: application/json, a fresh
// idempotency_key for each press, and Authorization: Bearer <token> when the daemon has a token. The cookie of
// the login is for reads: the command is sent without credentials. The token lives in the variable T.token of
// this module and nowhere else: not in a cookie, not in any storage of the browser, not in the URL, the history
// state or the DOM (the input is emptied as soon as it is read). A reload forgets it. A daemon without a token
// takes the command without a bearer and the page asks for none. Text from the daemon and from the person
// (a runbook name, the intake, a refusal) is set as text nodes, never as markup.
// This file is served by the daemon only: the inspector (ui/server.mjs) has no such file and no start form.

const PATH = '/daemon/commands/submit-run';
const LEVELS = ['local', 'linux'];
// T.needs: does the daemon want a token (null: not asked yet). T.token: the token the person gave, in memory.
const T = {needs: null, token: null, busy: false, floor: null, rows: [], msg: null};
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

// a key no earlier press used: 128 random bits
function freshKey() {
  return `page-${Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, '0')).join('')}`;
}

async function rowsOf(view, init) {
  const r = await fetch(`/daemon/views/${view}`, init);
  if (!r.ok) throw new Error(`${view}: ${r.status}`);
  return (await r.json()).rows;
}

// Whether the daemon wants a token: a read without credentials is answered 401 by a daemon that has one.
async function probe() {
  if (T.needs !== null) return;
  try { T.needs = (await fetch('/daemon/views/configuration', {credentials: 'omit'})).status === 401; } catch { T.needs = false; }
}

const say = (ok, text) => { T.msg = text ? {ok, text} : null; paint(); };

function levels() {
  const at = LEVELS.indexOf(T.floor);
  return at < 0 ? [LEVELS[LEVELS.length - 1]] : LEVELS.slice(at);
}

function note() {
  if (!T.rows.length) return 'The daemon lists no runbook it can start.';
  return T.rows.some((r) => r.runtime === 'stub')
    ? 'This daemon runs the stub runtime: the run is scripted and nothing is executed.'
    : 'The run is executed by this daemon on the Loom runtime.';
}

// Called whenever the form's data changed: it rewrites what the data decides and leaves what the person typed.
function paint() {
  if (!dlg) return;
  const pick = $('st-runbook');
  const had = pick.value;
  pick.replaceChildren(...T.rows.map((r) => h('option', {value: String(r.runbook_id), text: r.runbook_version ? `${r.runbook_id} · version ${r.runbook_version}` : String(r.runbook_id)})));
  if (T.rows.some((r) => String(r.runbook_id) === had)) pick.value = had;
  const level = $('st-confinement');
  const was = level.value;
  const offered = levels();
  level.replaceChildren(...offered.map((l) => h('option', {value: l, text: l})));
  level.value = offered.includes(was) ? was : offered[0];
  $('st-tokenrow').hidden = !(T.needs && T.token === null);
  $('st-note').textContent = note();
  const msg = $('st-msg');
  msg.className = T.msg ? `res ${T.msg.ok ? 'ok' : 'bad'}` : '';
  msg.textContent = T.msg ? T.msg.text : '';
  $('st-start').disabled = T.busy;
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
  if (given) T.token = given;
  const runbook = $('st-runbook').value;
  const intake = $('st-intake').value;
  const level = $('st-confinement').value;
  if (!runbook) return say(false, 'Pick a runbook first.');
  if (!intake.trim()) return say(false, 'Write the intake first.');
  if (!levels().includes(level)) return say(false, `The confinement ${level} is below the floor of this daemon (${T.floor || 'unknown'}).`);
  if (T.needs && T.token === null) { paint(); tokenInput.focus(); return say(false, 'This daemon needs its token: type it above, once, and press Start again.'); }
  T.busy = true;
  say(true, 'Sending the run to the daemon…');
  try {
    const r = await fetch(PATH, {
      method: 'POST',
      credentials: 'omit',
      headers: {'content-type': 'application/json', ...(T.token !== null ? {authorization: `Bearer ${T.token}`} : {})},
      body: JSON.stringify({runbook_id: runbook, intake, confinement: level, idempotency_key: freshKey()}),
    });
    T.busy = false;
    if (!r.ok) {
      if (r.status === 401) { T.token = null; T.needs = true; }
      return say(false, await refusal(r));
    }
    const run = (await r.json())?.published?.[0]?.payload?.run_id;
    if (!run) return say(false, 'The daemon took the run but named none.');
    T.msg = null;
    $('st-intake').value = '';
    paint();
    dlg.close();
    if (openRun) await openRun(String(run));
  } catch (e) {
    T.busy = false;
    say(false, `The daemon could not be reached: ${e.message}`);
  }
}

async function open() {
  T.msg = null;
  dlg.showModal();
  paint();
  await probe();
  try { T.floor = (await rowsOf('configuration'))[0]?.confinement_floor ?? null; } catch { T.floor = null; }
  try { T.rows = await rowsOf('runbooks'); } catch (e) { T.rows = []; T.msg = {ok: false, text: `The list of runbooks could not be read: ${e.message}`}; }
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
    h('div', {id: 'st-msg', role: 'status'}),
    h('div', {class: 'st-f'},
      h('button', {id: 'st-close', class: 'btn', type: 'button', text: 'Close'}),
      h('button', {id: 'st-start', class: 'btn primary', type: 'button', text: 'Start'})));
  dlg.addEventListener('click', (e) => {
    if (e.target.closest('#st-close')) dlg.close();
    else if (e.target.closest('#st-start')) start();
  });
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
