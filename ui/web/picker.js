// The run picker (decision D-009): a button in the header naming the run on screen and its state, and
// a panel with a search field and the runs, most recent first. Each row says what the run is at a
// glance: its state with a shape and a word (never colour alone), when it started, how long it ran,
// cost and calls, the models in short, and reopen or forced counts when the run records them. Running
// runs are on top, then the recent ones; stub and fixture runs (D-007) in a group of their own,
// collapsed until opened or matched by a search. Rows come from /api/sources (server.mjs runSummary).
import {esc} from './lenses.js';
import {fmtDur, fmtAgo} from './now.js';

export const isStub = (s) => s?.program?.kind === 'stub';
const words = (x) => String(x || '').replace(/_/g, ' ');

// the state of a run: a class for its shape, the word in a row, the short word in the header button
export function runState(s) {
  const r = s?.run || {};
  if (s && s.exists === false) return {cls: 'wait', text: 'waiting for its store', short: 'waiting'};
  if (r.state === 'running') return {cls: 'run', text: 'running', short: 'running'};
  if (r.state === 'stopped') return {cls: 'stop', text: `stopped · ${words(r.cause) || `by ${r.stopBy || 'someone'}`}`, short: 'stopped'};
  if (r.state === 'closed' && r.delivered) return {cls: 'ok', text: `closed · ${words(r.outcome) || 'delivered'}`, short: 'closed'};
  if (r.state === 'closed') return {cls: 'bad', text: `closed · not delivered${r.outcome && !/^not.delivered$/.test(r.outcome) ? ` (${words(r.outcome)})` : ''}`, short: 'not delivered'};
  return {cls: 'idle', text: 'not running', short: 'not running'};
}
const mark = (st, text = st.text) => `<span class="pk-mk m-${st.cls}"><i aria-hidden="true"></i><span class="pk-mt">${esc(text)}</span></span>`;

// the header button's content
export function renderPickerButton(s) {
  if (!s) return '<span class="pk-nm">no run</span>';
  const st = runState(s);
  return `${mark(st, st.short)}<span class="pk-nm">${esc(s.name)}</span>${isStub(s) ? '<span class="pk-stub">stub</span>' : ''}`;
}
export const pickerTitle = (s) => (s ? `${s.name}: ${runState(s).text}${isStub(s) ? ' · stub run' : ''} (choose another run)` : 'choose a run');

// most recent: when the run last wrote (its files), else when it ended or last recorded an event
const lastOf = (s) => s.lastWrite || s.run?.ended || s.run?.last || 0;
// what a search matches: the name, the state and its cause, the outcome, the models, who closed or stopped it
const hay = (s) => [s.name, runState(s).text, s.run?.outcome, s.run?.cause, s.run?.setting, s.run?.stopBy, s.run?.closedBy, s.run?.autonomy === 'A5' ? 'autopilot' : '', isStub(s) ? 'stub fixture' : ''].map(words).join(' ').toLowerCase();
export function pickerGroups(sources, q = '', showStubs = false) {
  const terms = q.toLowerCase().split(/\s+/).filter(Boolean);
  const hit = (s) => terms.every((t) => hay(s).includes(t));
  const shown = sources.filter(hit);
  const real = shown.filter((s) => !isStub(s));
  const running = real.filter((s) => s.run?.state === 'running').sort((a, b) => (b.run?.started ?? 0) - (a.run?.started ?? 0) || a.name.localeCompare(b.name));
  const recent = real.filter((s) => s.run?.state !== 'running').sort((a, b) => lastOf(b) - lastOf(a) || a.name.localeCompare(b.name));
  const stubs = shown.filter(isStub).sort((a, b) => lastOf(b) - lastOf(a) || a.name.localeCompare(b.name));
  const allStubs = sources.filter(isStub).length;
  return {terms, stubCount: terms.length ? stubs.length : allStubs, stubsOpen: showStubs || terms.length > 0,
    groups: [{key: 'running', label: 'Running now', rows: running}, {key: 'recent', label: 'Recent', rows: recent}, {key: 'stub', label: 'Stub and fixture runs (no model calls)', rows: stubs}]};
}
// the options the keyboard moves through, in the order shown
export const visibleRows = (g) => g.groups.filter((x) => x.key !== 'stub' || g.stubsOpen).flatMap((x) => x.rows);

const usd = (v) => (v == null ? null : `$${v < 10 ? v.toFixed(2) : v.toFixed(0)}`);
const when = (t) => new Date(t).toLocaleString([], {year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit'});
function row(s, {cur, act, now}) {
  const r = s.run || {};
  const st = runState(s);
  const bits = [];
  if (r.started != null) bits.push(`<span title="started ${esc(when(r.started))}">started <span data-run-ago="${r.started}">${esc(fmtAgo(now - r.started))}</span></span>`);
  if (r.started != null && r.state === 'running') bits.push(`<span title="running since ${esc(when(r.started))}">for <span class="pk-dur" data-run-since="${r.started}">${fmtDur(now - r.started)}</span></span>`);
  else if (r.started != null && r.ended != null) bits.push(`<span title="${esc(`${when(r.started)} to ${when(r.ended)}`)}">took <span class="pk-dur">${fmtDur(r.ended - r.started)}</span></span>`);
  if (r.cost != null) bits.push(`<span title="cost of the model and decider calls (llm-calls.jsonl, decisions.jsonl)">${usd(r.cost)}</span>`);
  if (r.calls != null) bits.push(`<span title="${r.work ?? 0} work calls, ${r.decider ?? 0} decider calls">${r.calls} calls</span>`);
  if (r.setting) bits.push(`<span class="pk-set" title="models (run-settings.jsonl)">${esc(r.setting)}</span>`);
  for (const [k, n] of [['reopens', r.reopens], ['forced', r.forced], ['unstuck', r.unstuck]]) if (n) bits.push(`<span class="pk-n" title="status.json: ${k}">${n} ${k === 'reopens' && n === 1 ? 'reopen' : k}</span>`);
  const title = [s.name, st.text, isStub(s) ? 'stub run: canned content, not a model\'s work' : '', r.setting].filter(Boolean).join(' · ');
  return `<div role="option" id="pk-o-${s.index}" class="pk-row${act ? ' act' : ''}" data-key="o${s.index}" data-pick="${s.index}" aria-selected="${cur ? 'true' : 'false'}" title="${esc(title)}">`
    + `<div class="pk-r1"><span class="pk-rn">${esc(s.name)}</span>${isStub(s) ? '<span class="pk-stub">stub</span>' : ''}${mark(st)}</div>`
    + `<div class="pk-r2">${bits.join('<span class="pk-sep" aria-hidden="true"> · </span>') || '<span>nothing recorded yet</span>'}</div></div>`;
}
// the list (role listbox, groups of options) and the stub group's toggle below it
export function renderPickerList(g, {cur, active, now}) {
  const groups = g.groups.filter((x) => x.rows.length && (x.key !== 'stub' || g.stubsOpen));
  const list = groups.map((x) => `<div role="group" class="pk-g g-${x.key}" data-key="g-${x.key}" aria-label="${esc(x.label)}"><div class="pk-gh" aria-hidden="true">${esc(x.label)} <b>${x.rows.length}</b></div>${x.rows.map((s) => row(s, {cur: s.index === cur, act: s.index === active, now})).join('')}</div>`).join('');
  const none = !groups.length ? `<p class="pk-none">${g.terms.length ? `No run matches “${esc(g.terms.join(' '))}”` : 'No runs'}</p>` : '';
  return {list, none, toggle: g.stubCount && !g.terms.length ? `${g.stubsOpen ? 'Hide' : 'Show'} stub and fixture runs (${g.stubCount})` : ''};
}
