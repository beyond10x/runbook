// The run picker (decision D-009): a button in the header naming the run on screen and its state, and
// a panel with a search field and the runs, most recent first. Each row says what the run is at a
// glance: its state with a shape and a word (never colour alone), when it started, how long it ran,
// cost and calls, the models in short, and reopen or forced counts when the run records them. Running
// runs are on top, then the recent ones, grouped by the experiment they belong to when any does (its
// head: purpose, variants × runs, clean passes of the ended runs); stub and fixture runs (D-007) in a group
// of their own, collapsed until opened or matched by a search. Rows come from /api/sources (server.mjs
// runSummary, experimentsOf).
import {esc} from './lenses.js';
import {fmtDur, fmtAgo} from './now.js';

export const isStub = (s) => s?.program?.kind === 'stub';
const words = (x) => String(x || '').replace(/_/g, ' ');

// the state of a run: a class for its shape, the word in a row, the short word in the header button
export function runState(s) {
  const r = s?.run || {};
  if (r.daemon) return r.daemon;
  if (s && s.exists === false) return {cls: 'wait', text: 'waiting for its store', short: 'waiting'};
  if (r.state === 'running') return {cls: 'run', text: 'running', short: 'running'};
  if (r.state === 'unknown') return {cls: 'idle', text: 'live state unknown', short: 'unknown'};
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
// what a search matches: the name, the state and its cause, the outcome, the models, who closed or stopped it,
// the experiments and variants it belongs to
const exps = (s) => (Array.isArray(s?.experiments) ? s.experiments.filter((e) => e && e.id) : []);
const hay = (s) => [s.name, runState(s).text, s.run?.outcome, s.run?.cause, s.run?.setting, s.run?.stopBy, s.run?.closedBy, s.run?.autonomy === 'A5' ? 'autopilot' : '', isStub(s) ? 'stub fixture' : '',
  ...exps(s).map((e) => `${e.id} ${e.variant || ''}`)].map(words).join(' ').toLowerCase();
const recentFirst = (a, b) => lastOf(b) - lastOf(a) || a.name.localeCompare(b.name);
// an experiment's head: its purpose, its variants with their number of runs, and how many of its ended runs
// passed clean (closed and delivered, nothing forced or unstuck), over all its real runs whatever the search
function expHead(id, all) {
  const runs = all.map((s) => ({s, e: exps(s).find((e) => e.id === id)})).filter((x) => x.e);
  const e0 = runs[0]?.e || {};
  const n = new Map();
  for (const {e} of [...runs].sort((a, b) => (a.e.vi ?? 1e9) - (b.e.vi ?? 1e9) || String(a.e.variant).localeCompare(String(b.e.variant)))) n.set(e.variant || 'no variant', (n.get(e.variant || 'no variant') || 0) + 1);
  const counts = [...n.values()];
  const variants = counts.length > 1 && counts.every((c) => c === counts[0]) ? `${counts.length} variants × ${counts[0]}` : [...n].map(([v, c]) => `${v} ×${c}`).join(' · ');
  const ended = runs.filter(({s}) => s.run?.state === 'closed' || s.run?.state === 'stopped');
  const clean = ended.filter(({s}) => s.run.state === 'closed' && s.run.delivered && !s.run.forced && !s.run.unstuck).length;
  return {id, purpose: e0.purpose || e0.title || null, variants, passes: `${clean} clean ${clean === 1 ? 'pass' : 'passes'} of ${ended.length} ended`, last: Math.max(0, ...runs.map(({s}) => lastOf(s)))};
}
// the groups shown: running now; then the recent runs, by experiment when any run belongs to one (each a
// collapsible group, its rows by variant then ordinal; the runs of none under Unassigned), else in one group;
// then the stub and fixture runs. Each row and experiment head gets the id its option has in the list.
// expOpen: the experiments opened or closed by hand; by default the one of the run on screen and the most
// recent are open, and a search opens every experiment it matches in.
export function pickerGroups(sources, q = '', showStubs = false, {cur = null, expOpen = {}} = {}) {
  const terms = q.toLowerCase().split(/\s+/).filter(Boolean);
  const hit = (s) => terms.every((t) => hay(s).includes(t));
  const shown = sources.filter(hit);
  const real = shown.filter((s) => !isStub(s));
  const running = real.filter((s) => s.run?.state === 'running').sort((a, b) => (b.run?.started ?? 0) - (a.run?.started ?? 0) || a.name.localeCompare(b.name));
  const recent = real.filter((s) => s.run?.state !== 'running').sort(recentFirst);
  const stubs = shown.filter(isStub).sort(recentFirst);
  const allStubs = sources.filter(isStub).length;
  const allReal = sources.filter((s) => !isStub(s));
  const groups = [{key: 'running', label: 'Running now', rows: running}];
  const ids = [...new Set(allReal.filter((s) => s.run?.state !== 'running').flatMap((s) => exps(s).map((e) => e.id)))];
  if (ids.length) {
    const heads = ids.map((id) => expHead(id, allReal)).sort((a, b) => b.last - a.last || a.id.localeCompare(b.id));
    heads.forEach((h, n) => {
      const rows = recent.filter((s) => exps(s).some((e) => e.id === h.id)).sort((a, b) => {
        const x = exps(a).find((e) => e.id === h.id), y = exps(b).find((e) => e.id === h.id);
        return (x.vi ?? 1e9) - (y.vi ?? 1e9) || String(x.variant).localeCompare(String(y.variant)) || (x.ordinal ?? 1e9) - (y.ordinal ?? 1e9) || a.name.localeCompare(b.name);
      });
      const open = expOpen[h.id] ?? (terms.length > 0 || rows.some((s) => s.index === cur) || n === 0);
      groups.push({key: `x-${n}`, label: `Experiment ${h.id}`, exp: h, open, rows});
    });
    groups.push({key: 'unassigned', label: 'Unassigned', rows: recent.filter((s) => !exps(s).length)});
  } else groups.push({key: 'recent', label: 'Recent', rows: recent});
  groups.push({key: 'stub', label: 'Stub and fixture runs (no model calls)', rows: stubs});
  // option ids: a run's first row is pk-o-<index>, a further row of it (another experiment) pk-o-<index>-<n>
  const seen = new Map();
  for (const x of groups) {
    if (x.exp) x.hid = `pk-e-${x.key.slice(2)}`;
    x.items = x.rows.map((s) => { const k = (seen.get(s.index) || 0) + 1; seen.set(s.index, k); return {id: k === 1 ? `pk-o-${s.index}` : `pk-o-${s.index}-${k}`, s}; });
  }
  return {terms, stubCount: terms.length ? stubs.length : allStubs, stubsOpen: showStubs || terms.length > 0, grouped: ids.length > 0, groups};
}
const shownGroups = (g) => g.groups.filter((x) => x.rows.length && (x.key !== 'stub' || g.stubsOpen));
// the options the keyboard moves through, in the order shown: {id, index} for a run, {id, exp} for an experiment's head
export const visibleItems = (g) => shownGroups(g).flatMap((x) => [...(x.exp ? [{id: x.hid, exp: x.exp.id, open: x.open}] : []), ...(x.exp && !x.open ? [] : x.items.map((it) => ({id: it.id, index: it.s.index})))]);

const usd = (v) => (v == null ? null : `$${v < 10 ? v.toFixed(2) : v.toFixed(0)}`);
const when = (t) => new Date(t).toLocaleString([], {year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit'});
// the experiment tag of a row: in its experiment's group the variant and ordinal, elsewhere the experiment too
function expTag(s, inExp) {
  const e = inExp ? exps(s).find((x) => x.id === inExp) : exps(s)[0];
  if (!e) return '';
  const t = `${e.variant || 'no variant'}${e.ordinal != null ? ` #${e.ordinal}` : ''}`;
  const title = `experiment ${e.id}, variant ${e.variant || 'not named'}${e.ordinal != null ? `, run ${e.ordinal}` : ''}${e.commit ? `, tree ${e.commit.slice(0, 12)}` : ''}${e.retrospective ? ', registered after the fact' : ''} (${e.from === 'experiment.json' ? 'experiment.json' : `experiments/${e.id}.yaml`})`;
  return `<span class="pk-var" title="${esc(title)}">${esc(inExp ? t : `${e.id} · ${t}`)}</span>`;
}
function row(s, {id, cur, act, now, inExp}) {
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
  return `<div role="option" id="${id}" class="pk-row${act ? ' act' : ''}" data-key="${id}" data-pick="${s.index}" aria-selected="${cur ? 'true' : 'false'}" title="${esc(title)}">`
    + `<div class="pk-r1"><span class="pk-rn">${esc(s.name)}</span>${isStub(s) ? '<span class="pk-stub">stub</span>' : ''}${expTag(s, inExp)}${mark(st)}</div>`
    + `<div class="pk-r2">${bits.join('<span class="pk-sep" aria-hidden="true"> · </span>') || '<span>nothing recorded yet</span>'}</div></div>`;
}
// an experiment's head: an option that opens or closes its group (Enter or a click), never a run
function expRow(x, act) {
  const h = x.exp;
  return `<div role="option" id="${x.hid}" class="pk-eh${act ? ' act' : ''}${x.open ? ' open' : ''}" data-key="${x.hid}" data-exp="${esc(h.id)}" aria-selected="false" title="${esc(`${h.id}${h.purpose ? `: ${h.purpose}` : ''} (${x.open ? 'close' : 'open'} the group)`)}">`
    + `<div class="pk-r1"><i class="pk-chev" aria-hidden="true"></i><span class="pk-en">${esc(h.id)}</span><span class="pk-vh">${x.open ? 'open' : 'closed'}, ${x.rows.length} ${x.rows.length === 1 ? 'run' : 'runs'}</span>`
    + `<span class="pk-ec">${esc(h.passes)}</span></div>`
    + `<div class="pk-r2">${h.purpose ? `<span class="pk-ep">${esc(h.purpose)}</span><span class="pk-sep" aria-hidden="true"> · </span>` : ''}<span>${esc(h.variants)}</span></div></div>`;
}
// the list (role listbox, groups of options) and the stub group's toggle below it
export function renderPickerList(g, {cur, active, now}) {
  const groups = shownGroups(g);
  let recentHead = false;
  const list = groups.map((x) => {
    // the experiments and Unassigned stand under one "Recent" heading
    const sh = g.grouped && (x.exp || x.key === 'unassigned') && !recentHead ? (recentHead = true, '<div class="pk-sh" data-key="sh" aria-hidden="true">Recent</div>') : '';
    const rows = x.exp && !x.open ? '' : x.items.map((it) => row(it.s, {id: it.id, cur: it.s.index === cur, act: it.id === active, now, inExp: x.exp?.id})).join('');
    const head = x.exp ? expRow(x, x.hid === active) : `<div class="pk-gh" aria-hidden="true">${esc(x.label)} <b>${x.rows.length}</b></div>`;
    return `${sh}<div role="group" class="pk-g g-${x.exp ? 'exp' : x.key}" data-key="g-${x.key}" aria-label="${esc(x.label)}">${head}${rows}</div>`;
  }).join('');
  const none = !groups.length ? `<p class="pk-none">${g.terms.length ? `No run matches “${esc(g.terms.join(' '))}”` : 'No runs'}</p>` : '';
  return {list, none, toggle: g.stubCount && !g.terms.length ? `${g.stubsOpen ? 'Hide' : 'Show'} stub and fixture runs (${g.stubCount})` : ''};
}
