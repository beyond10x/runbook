// The metrics of a run on a page served by the daemon (`runbook serve --ui`): the row of the daemon's view run-metrics
// (GET /daemon/views/run-metrics?run_id=...; docs/daemon-api.md, "The metrics of a run") drawn as a pane: the run's
// state, outcome, start, end or time running, duration and the totals of its model calls, then each step as a row of
// the columns step, role, state, took, calls and cost and a line under it with the rest (its case when that is not the
// run's own, tries, exit, start and end, tokens, calls of unknown cost and what it booked). Times, durations, counts and
// costs are in the words of the page's step list (web/clock.js, web/runsteps.js): `19:53:00 CET`, `2.8 s`,
// `1,200 in, 800 out`, `$0.0042`. A number the daemon left out of the row is said as "unknown", never as 0: the daemon writes no number its
// record does not hold; a count of calls without tokens or of unknown cost that is 0 is not said. Every value from the
// daemon is set as a text node, never as markup. This file is served by the daemon only; the page wires it.
import {clockTime, fullTime} from './clock.js';
import {usd as usdWords, fmtCount, fmtTook} from './runsteps.js';

export const VIEW = '/daemon/views/run-metrics';

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

const UNKNOWN = 'unknown';
const word = (v) => (typeof v === 'string' ? v : '');
const count = (v) => (Number.isSafeInteger(v) && v >= 0 ? v : null);

// Milliseconds in the step list's words (fmtTook of web/runsteps.js): `0.8 s`, `2.8 s`, `3 min 5 s`, `2 h 4 min`; whole
// seconds under a minute for a time still running (`whole`), as the list says a step in flight.
export function duration(ms, whole = false) {
  const n = count(ms);
  return n == null ? UNKNOWN : fmtTook(n, whole);
}

// Micro USD in the step list's words: 4200 is `$0.0042`.
export function usd(micro) {
  const n = count(micro);
  return n == null ? UNKNOWN : usdWords(n);
}

const num = (v) => {
  const n = count(v);
  return n == null ? UNKNOWN : fmtCount(n);
};

// An ISO time of the daemon as {text, title}: the page's clock words (`19:53:00 CET`, the date in front when not of
// today) and the full moment for a tooltip; null for no time or one that does not read as a time.
function moment(v) {
  const t = typeof v === 'string' && v ? Date.parse(v) : NaN;
  return Number.isNaN(t) ? null : {text: clockTime(t), title: fullTime(t)};
}

// A case id short, as the page names ids elsewhere: the last 8 characters of a UUID, a child id without its parent's
// `<run>.` in front, else the id.
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export const shortCase = (id, run) => (UUID.test(id) ? id.slice(-8) : run && id.startsWith(`${run}.`) ? id.slice(run.length + 1) : id);

// What a run or a step has run: its duration once it ended, the time running while it runs, else unknown.
function took(o) {
  if (count(o?.duration_ms) != null) return duration(o.duration_ms);
  if (count(o?.running_ms) != null) return `running for ${duration(o.running_ms, true)}`;
  return UNKNOWN;
}

// The words of a block of calls: the calls, the tokens, the known cost, the calls of unknown cost and what was booked.
// without_tokens and unknown_cost are null when the count is 0 (not said).
function callWords(c) {
  const zero = (v) => (count(v) === 0 ? null : num(v));
  return {
    calls: num(c?.model_calls),
    tokens_in: num(c?.input_tokens),
    tokens_out: num(c?.output_tokens),
    without_tokens: zero(c?.calls_without_tokens),
    cost: usd(c?.cost_micro_usd),
    unknown_cost: zero(c?.unknown_cost_calls),
    booked: usd(c?.booked_micro_usd),
  };
}

function summary(row) {
  const t = callWords(row.totals);
  const started = moment(row.started_at), ended = moment(row.ended_at);
  const items = [
    ['state', 'State', word(row.run_state) || UNKNOWN],
    ['outcome', 'Outcome', word(row.outcome) || (row.run_state === 'Ended' ? UNKNOWN : 'none yet')],
    ['started_at', 'Started', started?.text ?? (word(row.started_at) ? UNKNOWN : 'not started'), started?.title],
    ['ended_at', 'Ended', ended?.text ?? (word(row.ended_at) ? UNKNOWN : 'not ended'), ended?.title],
    ['duration', 'Duration', took(row)],
    ['calls', 'Model calls', t.calls],
    ['tokens', 'Tokens', `${t.tokens_in} in, ${t.tokens_out} out`],
    ['without_tokens', 'Calls without tokens', t.without_tokens],
    ['cost', 'Known cost', t.cost],
    ['unknown_cost', 'Calls of unknown cost', t.unknown_cost],
    ['booked', 'Booked in the ledger', t.booked],
  ].filter(([, , value]) => value != null);
  return h('dl', {class: 'metrics-sum'}, ...items.flatMap(([f, label, value, title]) => [h('dt', {text: label}), h('dd', {'data-field': f, title, text: value})]));
}

const COLUMNS = [['action', 'Step'], ['role', 'Role'], ['state', 'State'], ['duration', 'Took'], ['calls', 'Calls'], ['cost', 'Cost']];

// A step: the row of its columns and the line under it with the rest, both marked with the step's index.
function stepRows(s, i, run) {
  const c = callWords(s?.calls);
  const v = {action: word(s?.action) || UNKNOWN, role: word(s?.role), state: word(s?.step_state), duration: took(s), calls: c.calls, cost: c.cost};
  // an action breaks after its dots when the column is narrow (`evidence.` / `tickets`): text nodes and <wbr> only
  const cellOf = (f) => (f === 'action' ? h('td', {'data-field': f}, ...v[f].split(/(?<=\.)/).flatMap((p, k) => (k ? [h('wbr'), p] : [p]))) : h('td', {'data-field': f, text: v[f]}));
  const main = h('tr', {class: 'metrics-step', 'data-step': i}, ...COLUMNS.map(([f]) => cellOf(f)));
  const field = (f, text, title) => h('span', {'data-field': f, title, text});
  const parts = [];
  const id = word(s?.case_id);
  if (id && id !== run) parts.push(['case ', field('case', shortCase(id, run), id)]);
  parts.push(['try ', field('tries', num(s?.tries))]);
  parts.push(['exit ', field('exit', Number.isSafeInteger(s?.exit) ? String(s.exit) : UNKNOWN)]);
  const started = moment(s?.started_at), ended = moment(s?.ended_at);
  const from = field('started_at', started?.text ?? UNKNOWN, started?.title);
  if (ended) parts.push(['from ', from, ' to ', field('ended_at', ended.text, ended.title)]);
  else if (s?.step_state === 'running') parts.push(['from ', from, ', still ', field('ended_at', 'running')]);
  else parts.push(['from ', from, ', ', field('ended_at', 'no end recorded')]);
  // a step that made no model call has no tokens, cost or booking to say
  if (count(s?.calls?.model_calls) !== 0) {
    parts.push([field('tokens_in', c.tokens_in), ' in, ', field('tokens_out', c.tokens_out), ' out']);
    if (c.unknown_cost != null) parts.push([field('unknown_cost', c.unknown_cost), ' of unknown cost']);
    parts.push(['booked ', field('booked', c.booked)]);
  }
  const line = h('td', {colspan: COLUMNS.length});
  parts.forEach((p, k) => { if (k) line.append(' · '); line.append(...p); });
  return [main, h('tr', {class: 'metrics-more', 'data-step': i}, line)];
}

// The pane of one row of the view, or of the reason it could not be read (`error`, a text).
export function metricsPane(row, error) {
  const pane = h('section', {id: 'metrics-pane', class: 'metrics', 'aria-label': 'Metrics of the run'});
  if (!row || typeof row !== 'object') {
    pane.append(h('p', {class: 'res bad', role: 'status', text: `The metrics of the run could not be read: ${word(error) || 'the daemon answered no row'}.`}));
    return pane;
  }
  const steps = Array.isArray(row.steps) ? row.steps : [];
  pane.append(summary(row));
  const head = h('tr', {}, ...COLUMNS.map(([f, t]) => h('th', {scope: 'col', 'data-field': f, text: t})));
  const body = h('tbody', {}, ...steps.flatMap((s, i) => stepRows(s, i, word(row.run_id))));
  pane.append(h('div', {class: 'metrics-wrap'}, h('table', {class: 'metrics-steps'}, h('thead', {}, head), body)));
  const notes = [];
  if (!steps.length) notes.push('No step of the run has started a program.');
  const left = count(row.steps_left_out);
  if (left) notes.push(`${left} more step${left === 1 ? '' : 's'} not listed: the daemon lists at most ${num(row.step_limit)}. Their calls are in the totals.`);
  const outside = count(row.calls_outside_steps);
  if (outside) notes.push(`${outside} call${outside === 1 ? '' : 's'} of no listed step (the decider's, a delegation's classify and build calls, a step not listed) ${outside === 1 ? 'is' : 'are'} in the totals.`);
  notes.push('A number the daemon\'s record does not hold is said as unknown. A call of unknown cost adds nothing to the known cost; what it booked is in the ledger.');
  for (const n of notes) pane.append(h('p', {class: 'muted', text: n}));
  return pane;
}

// One read of the view for the run `runId`: `{row}`, or `{error}` with the daemon's refusal or what went wrong.
export async function readMetrics(runId) {
  try {
    const r = await fetch(`${VIEW}?run_id=${encodeURIComponent(runId)}`);
    let body = null;
    try { body = await r.json(); } catch { body = null; }
    if (!r.ok) return {error: `the daemon answered ${r.status}${body && typeof body.refused === 'string' ? ` ${body.refused}` : ''}`};
    const rows = Array.isArray(body?.rows) ? body.rows : null;
    if (!rows || rows.length !== 1) return {error: 'the daemon answered no row'};
    return {row: rows[0]};
  } catch (e) {
    return {error: String(e?.message || e)};
  }
}
