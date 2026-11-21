// The five run KPIs that stay on screen (docs/kpis.md ids). One computation, shown in the strip
// under the header and as the first rows of the Metrics tab. A value the run did not record is
// "not recorded", never 0.
import {phaseStates, llmTotals, esc} from './lenses.js';
import {fmtDur} from './now.js';

const tOf = (c, e) => c.times?.[e.seq] ?? (Date.parse(e.at) || null);
const NOT_PERSON = /^(non-interactive|derived|dry-run|rule:|decider:)/;
export const isPerson = (p) => !!p && !NOT_PERSON.test(p);

// union length of [a, b) intervals
function unionMs(iv) {
  const s = iv.filter(([a, b]) => a != null && b != null && b > a).sort((x, y) => x[0] - y[0]);
  let total = 0, cur = null;
  for (const [a, b] of s) {
    if (!cur || a > cur[1]) { if (cur) total += cur[1] - cur[0]; cur = [a, b]; } else cur[1] = Math.max(cur[1], b);
  }
  return cur ? total + cur[1] - cur[0] : 0;
}

export function computeKpis(cases, run, history, ns, now = Date.now()) {
  // R7 wall time: first event to the root's close, or to now while the run is open. L2's waiting
  // part: the union of the spans from a verdict awaiting a person or an authority to the case's
  // next event (to now while it still waits).
  let first = null, last = null;
  const waits = [];
  let openWait = null;
  for (const c of cases) {
    const evs = c.events || [];
    for (let i = 0; i < evs.length; i++) {
      const t = tOf(c, evs[i]);
      if (t == null) continue;
      if (first == null || t < first) first = t;
      if (last == null || t > last) last = t;
      if (evs[i].event === 'awaiting_person' || evs[i].event === 'awaiting_authority') {
        const nx = evs.slice(i + 1).find((e) => tOf(c, e) != null);
        if (nx) waits.push([t, tOf(c, nx)]);
        else { waits.push([t, now]); openWait = openWait == null ? t : Math.min(openWait, t); }
      }
    }
  }
  const closed = ns?.mode === 'closed' || ns?.mode === 'stuck';
  const end = closed ? last : now;
  const wall = first != null ? Math.max(0, end - first) : null;
  const waitMs = unionMs(waits.map(([a, b]) => [a, Math.min(b, end ?? b)]));
  const kp = [];
  kp.push({key: 'time', ids: 'R7 · L2', label: 'Elapsed', value: wall, live: !closed && first != null ? first : null,
    text: wall == null ? null : fmtDur(wall), sub: waits.length ? {label: 'waiting on a person', text: fmtDur(waitMs), live: openWait != null ? now - waitMs : null} : {label: 'waiting on a person', text: '0:00'},
    title: 'R7: first event of the tree to its close, or to now while it runs. Event times are the driver\'s or this server\'s clock where one was observed, else the run\'s --at. Waiting: from each verdict awaiting a person or authority to the case\'s next event (L2 counts it as not autonomous).'});
  // R6 + H4 cost; R4 + decider calls
  const llmFile = (run?.llm || []).some((x) => x.where === 'llm-calls.jsonl');
  const lt = llmTotals(run);
  const dec = run?.decisions || [];
  const decCalls = dec.filter((d) => d.model || d.cost != null);
  const decUsd = decCalls.reduce((s, d) => s + (Number(d.cost) || 0), 0);
  const workRecorded = llmFile || lt.calls > 0;
  const decRecorded = dec.length > 0;
  kp.push({key: 'cost', ids: 'R6 · H4', label: 'Cost', value: workRecorded || decRecorded ? lt.usd + decUsd : null,
    text: workRecorded || decRecorded ? `$${(lt.usd + decUsd).toFixed(2)}` : null,
    parts: [{label: 'work', text: workRecorded ? `$${lt.usd.toFixed(3)}` : null}, {label: 'decider', text: decRecorded ? `$${decUsd.toFixed(3)}` : null}],
    title: 'R6: Σ cost_usd of llm-calls.jsonl (agent nodes). H4: Σ cost_usd of decisions.jsonl (decider calls).'});
  kp.push({key: 'calls', ids: 'R4', label: 'LLM calls', value: workRecorded || decRecorded ? lt.calls + decCalls.length : null,
    text: workRecorded || decRecorded ? String(lt.calls + decCalls.length) : null,
    parts: [{label: 'work', text: workRecorded ? String(lt.calls) : null}, {label: 'decider', text: decRecorded ? String(decCalls.length) : null}],
    title: `R4: agent-node calls in llm-calls.jsonl${lt.calls ? ` (${lt.toolCalls} tool calls, ${(lt.ms / 1000).toFixed(0)} s model time)` : ''}; decider calls in decisions.jsonl.`});
  // H1 / L1 person decisions; H2 escalations
  const hist = history || [];
  const person = hist.filter((d) => isPerson(d.principal) && !d.refused && d.src !== 'ui-actions.jsonl');
  const esc2 = dec.filter((d) => /^escalat/.test(String(d.decision || '')) || d.invalid);
  kp.push({key: 'decisions', ids: 'H1 · H2', label: 'Person decisions', value: person.length,
    text: String(person.length), parts: [{label: 'escalated', text: decRecorded ? String(esc2.length) : null}, {label: 'by decider', text: decRecorded ? String(hist.filter((d) => /^decider:/.test(d.principal || '')).length) : null}],
    title: 'H1/L1: approvals, answers and manual actions whose principal is a person (not non-interactive, derived, rule:* or decider:*). H2: decider decisions that escalated (or were invalid) in decisions.jsonl.'});
  // F10 + phase
  const opened = cases.length, done = cases.filter((c) => (c.events || []).some((e) => e.event === 'terminated')).length;
  const ph = phaseStates(cases);
  const front = ph.find((p) => p.status !== 'done' && p.status !== 'none') || (ph.every((p) => p.status === 'done') ? {label: 'all done'} : ph.find((p) => p.status !== 'done'));
  kp.push({key: 'progress', ids: 'F10', label: 'Progress', value: opened ? done / opened : null, text: opened ? `${done}/${opened}` : null, unit: 'cases closed',
    parts: [{label: 'phase', text: front?.short || front?.label || null}],
    title: 'F10: cases of the tree that ended (terminated) / cases opened. Phase: the first SDC phase (docs/phases.md) not yet done, derived from Canon claims.'});
  return kp;
}

const NR = '<span class="k-nr">not recorded</span>';
export function renderKpis(kp, now = Date.now()) {
  return kp.map((k) => {
    const main = k.text == null ? NR : k.live != null ? `<span data-since="${k.live}">${esc(fmtDur(now - k.live))}</span>` : esc(k.text);
    let sub = '';
    if (k.sub) sub = `<span class="k-p"><span class="k-pl">${esc(k.sub.label)}</span><b>${k.sub.live != null ? `<span data-since="${k.sub.live}">${esc(fmtDur(now - k.sub.live))}</span>` : esc(k.sub.text)}</b></span>`;
    for (const p of k.parts || []) sub += `<span class="k-p"><span class="k-pl">${esc(p.label)}</span><b>${p.text == null ? NR : esc(p.text)}</b></span>`;
    return `<button class="kt k-${k.key}" data-key="kpi:${k.key}" data-kpi="${k.key}" title="${esc(`${k.ids} · ${k.title}\nClick for its row in Metrics.`)}"><span class="k-l">${esc(k.label)}<i>${esc(k.ids)}</i></span><span class="k-v">${main}${k.unit && k.text != null ? `<small>${esc(k.unit)}</small>` : ''}</span><span class="k-s">${sub}</span></button>`;
  }).join('');
}
// The same values as rows of the Metrics tab, each with an anchor the strip links to.
export function kpiRows(kp, now = Date.now()) {
  return `<div class="sect">Run KPIs <span class="muted">the strip under the header</span></div><table class="steps kpirows"><tbody>${kp.map((k) => `<tr id="kpi-${k.key}" data-key="kr:${k.key}"><td class="mono kid">${esc(k.ids)}</td><td>${esc(k.label)}</td><td class="n"><b>${k.text == null ? NR : k.live != null ? `<span data-since="${k.live}">${esc(fmtDur(now - k.live))}</span>` : esc(k.text)}</b>${[k.sub, ...(k.parts || [])].filter(Boolean).map((p) => `<div class="muted">${esc(p.label)} ${p.live != null ? `<span data-since="${p.live}">${esc(fmtDur(now - p.live))}</span>` : p.text == null ? NR : esc(p.text)}</div>`).join('')}</td></tr><tr class="kdef" data-key="kd:${k.key}"><td></td><td colspan="2" class="muted">${esc(k.title)}</td></tr>`).join('')}</tbody></table>`;
}
