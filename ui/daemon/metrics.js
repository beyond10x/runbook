// The metrics of a run on a page served by the daemon (`runbook serve --ui`): the row of the daemon's view run-metrics
// (GET /daemon/views/run-metrics?run_id=...; docs/daemon-api.md, "The metrics of a run") drawn as a pane: the run's
// state, outcome, start, end or time running, duration and the totals of its model calls, then a row per step (case,
// action, role, tries, state, start, end, exit, duration, calls, tokens and cost). A number the daemon left out of the
// row is said as "unknown", never as 0: the daemon writes no number its record does not hold. Every value from the
// daemon is set as a text node, never as markup. This file is served by the daemon only; the page wires it.

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

// Milliseconds in words: `850 ms`, `2.75 s`, `3 min 5 s`, `2 h 4 min`.
export function duration(ms) {
  const n = count(ms);
  if (n == null) return UNKNOWN;
  if (n < 1000) return `${n} ms`;
  if (n < 60000) return `${(n / 1000).toFixed(2).replace(/\.?0+$/, '')} s`;
  const s = Math.floor(n / 1000);
  if (s < 3600) return `${Math.floor(s / 60)} min ${s % 60} s`;
  return `${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min`;
}

// Micro USD as dollars with six places: 210 is `$0.000210`.
export function usd(micro) {
  const n = count(micro);
  if (n == null) return UNKNOWN;
  const whole = Math.floor(n / 1e6);
  return `$${whole}.${String(n - whole * 1e6).padStart(6, '0')}`;
}

const num = (v) => {
  const n = count(v);
  return n == null ? UNKNOWN : String(n);
};

// What a run or a step has run: its duration once it ended, the time running while it runs, else unknown.
function took(o) {
  if (count(o?.duration_ms) != null) return duration(o.duration_ms);
  if (count(o?.running_ms) != null) return `running for ${duration(o.running_ms)}`;
  return UNKNOWN;
}

// The words of a block of calls: the calls, the tokens, the known cost, the calls of unknown cost and what was booked.
function callWords(c) {
  return {
    calls: num(c?.model_calls),
    tokens_in: num(c?.input_tokens),
    tokens_out: num(c?.output_tokens),
    without_tokens: num(c?.calls_without_tokens),
    cost: usd(c?.cost_micro_usd),
    unknown_cost: num(c?.unknown_cost_calls),
    booked: usd(c?.booked_micro_usd),
  };
}

function summary(row) {
  const t = callWords(row.totals);
  const items = [
    ['state', 'State', word(row.run_state) || UNKNOWN],
    ['outcome', 'Outcome', word(row.outcome) || (row.run_state === 'Ended' ? UNKNOWN : 'none yet')],
    ['started_at', 'Started', word(row.started_at) || 'not started'],
    ['ended_at', 'Ended', word(row.ended_at) || 'not ended'],
    ['duration', 'Duration', took(row)],
    ['calls', 'Model calls', t.calls],
    ['tokens', 'Tokens in / out', `${t.tokens_in} / ${t.tokens_out}`],
    ['without_tokens', 'Calls without tokens', t.without_tokens],
    ['cost', 'Known cost', t.cost],
    ['unknown_cost', 'Calls of unknown cost', t.unknown_cost],
    ['booked', 'Booked in the ledger', t.booked],
  ];
  return h('dl', {class: 'metrics-sum'}, ...items.flatMap(([f, label, value]) => [h('dt', {text: label}), h('dd', {'data-field': f, text: value})]));
}

const COLUMNS = [['case', 'Case'], ['action', 'Action'], ['role', 'Role'], ['tries', 'Tries'], ['state', 'State'], ['started_at', 'Started'], ['ended_at', 'Ended'], ['exit', 'Exit'], ['duration', 'Duration'], ['calls', 'Calls'], ['tokens_in', 'Tokens in'], ['tokens_out', 'Tokens out'], ['cost', 'Known cost'], ['unknown_cost', 'Unknown cost'], ['booked', 'Booked']];

function stepRow(s) {
  const c = callWords(s?.calls);
  const v = {
    case: word(s?.case_id) || UNKNOWN,
    action: word(s?.action) || UNKNOWN,
    role: word(s?.role),
    tries: num(s?.tries),
    state: word(s?.step_state),
    started_at: word(s?.started_at),
    ended_at: word(s?.ended_at) || (s?.step_state === 'running' ? 'running' : 'no end recorded'),
    exit: Number.isSafeInteger(s?.exit) ? String(s.exit) : UNKNOWN,
    duration: took(s),
    calls: c.calls,
    tokens_in: c.tokens_in,
    tokens_out: c.tokens_out,
    cost: c.cost,
    unknown_cost: c.unknown_cost,
    booked: c.booked,
  };
  return h('tr', {}, ...COLUMNS.map(([f]) => h('td', {'data-field': f, text: v[f]})));
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
  const head = h('tr', {}, ...COLUMNS.map(([, t]) => h('th', {scope: 'col', text: t})));
  pane.append(h('div', {class: 'metrics-wrap'}, h('table', {class: 'metrics-steps'}, h('thead', {}, head), h('tbody', {}, ...steps.map(stepRow)))));
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
