// The Decisions drawer of a page served by the daemon (`runbook serve --ui`): the open decision requests of the
// selected run, read from the view open-decisions (source.js hands them over in S.inbox.items), and the answer,
// which is the daemon's own command answer-decision, the one `runbook decide` sends.
//
// What the page sends for one answer: POST /daemon/commands/answer-decision, Content-Type: application/json,
// and Authorization: Bearer <token> when the daemon has a token. The cookie of the login is for reads: the
// command is sent without credentials, and the daemon would refuse a cookie there anyway. The token lives in
// the variable D.token of this module and nowhere else: not in a cookie, not in any storage of the browser
// (local, session, IndexedDB), not in the URL, the history state or the DOM (the input is emptied as soon as it is read). A
// reload forgets it, and the next command asks again. A daemon without a token takes the command without a
// bearer and the page asks for none. Text from the daemon is set as text nodes, never as markup.
import {refresh} from './source.js';

const PATH = '/daemon/commands/answer-decision';
// D.needs: does the daemon want a token (null: not asked yet). D.token: the token the person gave, in memory.
const D = {needs: null, token: null, busy: false, msg: null, other: {}, probing: false};
let last = null;

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

// Whether the daemon wants a token: a read without credentials is answered 401 by a daemon that has one.
function probe() {
  if (D.needs !== null || D.probing) return;
  D.probing = true;
  fetch('/daemon/views/configuration', {credentials: 'omit'})
    .then((r) => { D.needs = r.status === 401; })
    .catch(() => { D.needs = false; })
    .finally(() => { D.probing = false; if (last) renderDaemonDrawer(last.S, last.el); });
}

function consequence(option) {
  const c = option.consequence || {};
  const part = (name, list) => (Array.isArray(list) && list.length ? `${name}: ${list.join(', ')}` : '');
  return [option.cost && option.cost !== 'none' ? `costs ${option.cost}` : '', part('admits', c.admits), part('blocks', c.blocks)].filter(Boolean).join(' · ');
}

function field(name, text) {
  const el = h('span', {class: 'chip-r', text});
  el.dataset.field = name;
  return el;
}

function card(row) {
  const id = String(row.request_id);
  const opts = (Array.isArray(row.options) ? row.options : []).map((o, i) => {
    const b = h('button', {type: 'button', class: i === 0 ? 'btn primary' : 'btn', disabled: D.busy, text: `${o.letter}: ${o.does}`});
    b.dataset.answer = String(o.letter);
    return h('div', {class: 'dd-opt'}, b, h('span', {class: 'muted', text: consequence(o)}));
  });
  const other = h('input', {class: 'tin', type: 'text', placeholder: 'another answer', maxLength: 500, value: D.other[id] || '', 'aria-label': 'another answer'});
  other.dataset.request = id;
  const send = h('button', {type: 'button', class: 'btn', disabled: D.busy, text: 'Send this answer'});
  send.dataset.send = id;
  const art = h('article', {class: 'dcard k-question'},
    h('div', {class: 'dc-h'}, h('span', {class: 'dk q', text: 'Decision'}), h('h3', {text: String(row.question)})),
    h('div', {class: 'dc-meta'},
      field('case_id', `case ${row.case_id ?? ''}`),
      field('action', `action ${row.action ?? ''}`),
      field('capability', `capability ${row.capability ?? ''}`),
      h('span', {class: 'chip-r', text: `request ${id}`}),
      h('span', {class: 'chip-r', text: `opened ${String(row.opened_at || '').slice(0, 19).replace('T', ' ')}`})),
    ...opts,
    h('div', {class: 'dd-other'}, other, send));
  art.dataset.request = id;
  art.dataset.run = String(row.run_id);
  return art;
}

function build(el) {
  const by = h('input', {id: 'dd-by', class: 'tin', type: 'text', maxLength: 100, autocomplete: 'off', placeholder: 'your name'});
  const token = h('input', {id: 'dd-token', class: 'tin', type: 'password', autocomplete: 'off', placeholder: 'the daemon token'});
  const who = h('div', {id: 'dd-who', class: 'dd-who'},
    h('label', {}, 'Answer as ', by),
    h('label', {id: 'dd-tokenrow'}, 'Token ', token),
    h('span', {class: 'muted', text: 'The daemon does not check who you are: the name is a label it records with the answer.'}));
  el.replaceChildren(
    h('div', {class: 'dr-h'}, h('div', {}, h('h2', {text: 'Decisions'}), h('span', {id: 'dd-count', class: 'muted'})),
      h('button', {id: 'drclose', class: 'ib wide', type: 'button', text: 'Close'})),
    who,
    h('div', {id: 'dd-msg', role: 'status'}),
    h('div', {id: 'dd-list', class: 'dr-list'}));
  el.addEventListener('click', (e) => {
    const b = e.target.closest('button[data-answer], button[data-send]');
    if (!b) return;
    const art = b.closest('.dcard');
    const text = b.dataset.send ? art.querySelector('input.tin').value.trim() : b.dataset.answer;
    answer(art.dataset.run, art.dataset.request, text);
  });
  el.addEventListener('input', (e) => { if (e.target.dataset?.request) D.other[e.target.dataset.request] = e.target.value; });
  el.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && e.target.dataset?.request) e.target.closest('.dcard').querySelector('button[data-send]').click();
  });
}

const say = (ok, text) => { D.msg = {ok, text}; if (last) renderDaemonDrawer(last.S, last.el); };

// The daemon's refusal as one line: its status and the message of its body.
async function refusal(r) {
  let body = null;
  try { body = await r.json(); } catch { /* not JSON */ }
  const why = body?.message || body?.error || body?.refused || r.statusText || 'refused';
  return `${r.status} ${why}`;
}

async function answer(run, request, text) {
  const by = document.getElementById('dd-by');
  const tokenInput = document.getElementById('dd-token');
  // the token leaves the field first, on every path below: into the memory of the page, never left in the DOM
  const given = tokenInput.value;
  tokenInput.value = '';
  if (given) D.token = given;
  const who = by.value.trim();
  if (!text) return say(false, 'Pick an option, or write the answer first.');
  if (!who) { by.focus(); return say(false, 'Say who you are first: the daemon records that name with the answer.'); }
  if (D.needs && D.token === null) { tokenInput.focus(); return say(false, 'This daemon needs its token: type it above, once, and answer again.'); }
  D.busy = true;
  say(true, 'Sending the answer to the daemon…');
  try {
    const r = await fetch(PATH, {
      method: 'POST',
      credentials: 'omit',
      headers: {'content-type': 'application/json', ...(D.token !== null ? {authorization: `Bearer ${D.token}`} : {})},
      body: JSON.stringify({run_id: run, request_id: request, answer: text, answered_by: who}),
    });
    D.busy = false;
    if (r.ok) { delete D.other[request]; D.msg = {ok: true, text: `Answered ${request} as ${who}.`}; }
    else {
      if (r.status === 401) { D.token = null; D.needs = true; }
      D.msg = {ok: false, text: await refusal(r)};
    }
  } catch (e) {
    D.busy = false;
    D.msg = {ok: false, text: `The daemon could not be reached: ${e.message}`};
  }
  if (last) renderDaemonDrawer(last.S, last.el);
  refresh();
}

// Called by the page's render while the drawer is open; it changes the DOM only when what it shows changed,
// so a field the person is typing in keeps its text and its focus.
export function renderDaemonDrawer(S, el) {
  last = {S, el};
  probe();
  if (!el.querySelector('#dd-list')) build(el);
  const rows = S.inbox?.items || [];
  const state = D.needs === null ? 'unknown' : !D.needs ? 'none' : D.token === null ? 'needed' : 'held';
  const who = el.querySelector('#dd-who');
  who.dataset.token = state;
  el.querySelector('#dd-tokenrow').hidden = state !== 'needed';
  const count = `${rows.length} open${S.inbox?.run ? ` in run ${String(S.inbox.run).slice(-8)}` : ''}`;
  if (el.querySelector('#dd-count').textContent !== count) el.querySelector('#dd-count').textContent = count;
  const msg = el.querySelector('#dd-msg');
  const shown = D.msg ? `${D.msg.ok ? 'ok' : 'bad'} ${D.msg.text}` : '';
  if (msg.dataset.shown !== shown) {
    msg.dataset.shown = shown;
    msg.className = D.msg ? `res ${D.msg.ok ? 'ok' : 'bad'}` : '';
    msg.textContent = D.msg ? D.msg.text : '';
  }
  const list = el.querySelector('#dd-list');
  const sig = JSON.stringify([rows, D.busy]);
  if (list.dataset.sig === sig) return;
  list.dataset.sig = sig;
  if (!rows.length) {
    list.replaceChildren(h('div', {class: 'empty-d'}, h('div', {class: 'big', text: '✓'}), h('p', {text: 'No decision is waiting. The run goes on by itself until it needs you.'})));
    return;
  }
  list.replaceChildren(...rows.map(card));
}
