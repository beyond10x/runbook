// The five run KPIs that stay on screen (docs/kpis.md ids). One computation, shown in the strip
// under the header and as the first rows of the Metrics tab. A value the run did not record is
// "not recorded", never 0.
import {phaseStates, llmTotals, esc} from './lenses.js';
import {fmtDur} from './now.js';
import {isPersonPrincipal, principalOf, autopilot, standIn} from './principal.js';

const tOf = (c, e) => c.times?.[e.seq] ?? (Date.parse(e.at) || null);
// a person only for a person principal (web/principal.js): never a decider, a rule, the agent's own
// answer or a legacy record that says "human" without an id
export const isPerson = (p) => isPersonPrincipal(p);

// union length of [a, b) intervals
function unionMs(iv) {
  const s = iv.filter(([a, b]) => a != null && b != null && b > a).sort((x, y) => x[0] - y[0]);
  let total = 0, cur = null;
  for (const [a, b] of s) {
    if (!cur || a > cur[1]) { if (cur) total += cur[1] - cur[0]; cur = [a, b]; } else cur[1] = Math.max(cur[1], b);
  }
  return cur ? total + cur[1] - cur[0] : 0;
}

// Who the run's setting gives the decisions to (run-settings.jsonl, last line): 'person', a rule
// ('approve', 'reject') or an LLM decider kind; null when the run did not record it.
export const deciderKindOf = (run) => run?.settings?.decider?.kind || null;
export const deciderHolds = (run) => { const k = deciderKindOf(run); return !!k && k !== 'person'; };

const DECIDER = /^decider:/;
// The stops a person held: each span from a verdict awaiting a person or an authority to the case's
// next event (to `end` while it still waits), unless a decider took it: a decider decision of
// decisions.jsonl on the same case and one of the asked actions inside the span (an escalation does
// not count), the event that ends the span carrying a decider principal, or a decider call for it
// in progress now. Each stop: {c, seq, actions, start, end, open, ms}.
export function personStops(cases, run, end, ns) {
  const dec = (run?.decisions || []).filter((d) => DECIDER.test(d.principal || '') && !/^escalat/.test(String(d.decision || '')));
  const deciding = new Set((ns?.waiting || []).filter((w) => w.kind === 'decider').map((w) => `${w.c.id}|${w.event?.seq}`));
  const out = [];
  for (const c of cases) {
    const evs = c.events || [];
    for (let i = 0; i < evs.length; i++) {
      const e = evs[i];
      if (e.event !== 'awaiting_person' && e.event !== 'awaiting_authority') continue;
      const t = tOf(c, e);
      if (t == null || (end != null && t > end)) continue;
      const nxIdx = evs.findIndex((x, j) => j > i && tOf(c, x) != null);
      const nx = nxIdx >= 0 ? evs[nxIdx] : null;
      const t1 = nx ? Math.min(tOf(c, nx), end ?? Infinity) : end;
      const actions = (e.next || []).map((n) => n?.action).filter(Boolean);
      // who took it: the first event with a principal before the case waits or stops again
      let closer = null;
      for (let j = i + 1; j < evs.length && !closer; j++) {
        if (/^awaiting_|^stuck$|^terminated$/.test(evs[j].event)) break;
        if (evs[j].principal) closer = evs[j];
      }
      const byDecider = (closer && DECIDER.test(closer.principal)) || deciding.has(`${c.id}|${e.seq}`)
        || dec.some((d) => d.case === c.id && (!actions.length || actions.includes(d.what)) && (() => { const dt = Date.parse(d.at); return !isNaN(dt) && dt >= t - 1000 && dt <= (t1 ?? Infinity) + 5000; })());
      if (byDecider || t1 == null) continue;
      out.push({c, seq: e.seq, verdict: e.event, actions, start: t, end: t1, open: !nx || tOf(c, nx) > (end ?? Infinity), ms: Math.max(0, t1 - t)});
    }
  }
  return out.sort((a, b) => b.ms - a.ms || a.start - b.start);
}

export function computeKpis(cases, run, history, ns, now = Date.now()) {
  // R7 wall time: first event to the root's close, or to now while the run is open, or to the stop
  // of a run stopped on purpose (run-stopped.json). L2's waiting part: the union of the spans a
  // person held (personStops), to the same end.
  let first = null, last = null;
  for (const c of cases) {
    for (const e of c.events || []) {
      const t = tOf(c, e);
      if (t == null) continue;
      if (first == null || t < first) first = t;
      if (last == null || t > last) last = t;
    }
  }
  const stopT = ns?.stopped ? Date.parse(ns.stopped.at) : null;
  const idleT = ns?.idle?.t ?? null;
  const closed = ns?.mode === 'closed' || ns?.mode === 'stuck' || stopT != null || idleT != null;
  const end = stopT != null ? stopT : idleT != null ? idleT : closed ? last : now;
  const wall = first != null ? Math.max(0, end - first) : null;
  const stops = personStops(cases, run, end, ns);
  const waitMs = unionMs(stops.map((s) => [s.start, s.end]));
  const openWait = !closed && stops.some((s) => s.open);
  // with a decider holding the decisions, any time a person held one is a failure (should be 0)
  const holds = deciderHolds(run);
  const fail = holds && waitMs > 0 ? {text: 'should be 0', why: `this run's decider (${run.settings.decider.model || deciderKindOf(run)}, run-settings.jsonl) holds the decisions: no step should wait${autopilot(run.settings) ? '' : ' on a person'}`, stops} : null;
  const kp = [];
  kp.push({key: 'time', ids: 'R7 · L2', label: 'Elapsed', value: wall, live: !closed && first != null ? first : null,
    text: wall == null ? null : fmtDur(wall), sub: {label: autopilot(run?.settings) ? 'waiting, nobody deciding' : 'waiting on a person', text: fmtDur(waitMs), live: openWait ? now - waitMs : null}, fail, waitMs, stops,
    title: `R7: first event of the tree to its close, or to now while it runs, or to the stop of a stopped run. Event times are the driver's or this server's clock where one was observed, else the run's --at. Waiting: from each verdict awaiting a person or authority to the case's next event, unless a decider took it (L2 counts it as not autonomous). ${holds ? `The run's decider (${deciderKindOf(run)}) holds the decisions, so this should be 0.` : deciderKindOf(run) === 'person' ? 'The run\'s setting gives the decisions to a person.' : 'The run did not record who holds its decisions (no run-settings.jsonl).'}`});
  // R6 + H4 cost; R4 + decider calls
  const llmFile = (run?.llm || []).some((x) => x.where === 'llm-calls.jsonl');
  const lt = llmTotals(run);
  const dec = run?.decisions || [];
  // Every decider model call made, whatever became of its reply, once: lines sharing a `record`
  // (decider/NNN-*) are one call, its cost the highest of theirs (a suppressed escalation has it on
  // its escalation_suppressed line, a fallback repeats the last invalid reply's call); a line
  // without a record is a call of its own. Rule deciders name no model.
  const byCall = new Map();
  dec.forEach((d, i) => {
    if (!d.model && d.cost == null) return;
    const key = d.record || `line ${i}`;
    byCall.set(key, Math.max(byCall.get(key) ?? 0, Number(d.cost) || 0));
  });
  const decCalls = [...byCall.values()];
  const decUsd = decCalls.reduce((s, c) => s + c, 0);
  const workRecorded = llmFile || lt.calls > 0;
  const decRecorded = dec.length > 0;
  kp.push({key: 'cost', ids: 'R6 · H4', label: 'Cost', value: workRecorded || decRecorded ? lt.usd + decUsd : null,
    text: workRecorded || decRecorded ? `$${(lt.usd + decUsd).toFixed(2)}` : null,
    parts: [{label: 'work', text: workRecorded ? `$${lt.usd.toFixed(3)}` : null}, {label: 'decider', text: decRecorded ? `$${decUsd.toFixed(3)}` : null}],
    title: 'R6: Σ cost_usd of llm-calls.jsonl (agent nodes). H4: Σ cost_usd of decisions.jsonl, one decider call per record, whatever became of its reply.'});
  kp.push({key: 'calls', ids: 'R4', label: 'LLM calls', value: workRecorded || decRecorded ? lt.calls + decCalls.length : null,
    text: workRecorded || decRecorded ? String(lt.calls + decCalls.length) : null,
    parts: [{label: 'work', text: workRecorded ? String(lt.calls) : null}, {label: 'decider', text: decRecorded ? String(decCalls.length) : null}],
    title: `R4: agent-node calls in llm-calls.jsonl${lt.calls ? ` (${lt.toolCalls} tool calls, ${(lt.ms / 1000).toFixed(0)} s model time)` : ''}; decider calls in decisions.jsonl.`});
  // H1 / L1 person decisions; H2 escalations
  const hist = history || [];
  const person = hist.filter((d) => isPerson(d.principal) && !d.refused && d.src !== 'ui-actions.jsonl');
  const esc2 = dec.filter((d) => /^escalat/.test(String(d.decision || '')) || d.invalid);
  // records that say "human" without an id are not counted as a person's: they are said as such
  const legacy = hist.filter((d) => principalOf(d.principal).kind === 'legacy' && !d.refused && d.src !== 'ui-actions.jsonl');
  const byDecider = hist.filter((d) => ['decider', 'rule'].includes(principalOf(d.principal).kind)).length;
  const auto = autopilot(run?.settings);
  kp.push({key: 'decisions', ids: 'H1 · H2', label: auto ? 'Decisions' : 'Person decisions', value: auto ? byDecider : person.length,
    text: auto ? String(byDecider) : String(person.length), unit: auto ? 'by the decider or a rule' : null,
    parts: [{label: 'escalated', text: decRecorded ? String(esc2.length) : null}, ...(auto ? [] : [{label: 'by decider', text: decRecorded ? String(byDecider) : null}]), ...(legacy.length ? [{label: 'recorded as human', text: String(legacy.length)}] : [])],
    title: `H1/L1: approvals, answers and manual actions whose principal is a person (person:<id>; not a decider, a rule, the agent's own answer or a record that says "human" without an id). H2: decider decisions that escalated (or were invalid) in decisions.jsonl.${auto ? ' Autopilot (autonomy A5): the decider or a rule holds every decision.' : ''}`});
  // F10 + phase
  const opened = cases.length, done = cases.filter((c) => (c.events || []).some((e) => e.event === 'terminated')).length;
  const ph = phaseStates(cases);
  const front = ph.find((p) => p.status !== 'done' && p.status !== 'none') || (ph.every((p) => p.status === 'done') ? {label: 'all done'} : ph.find((p) => p.status !== 'done'));
  kp.push({key: 'progress', ids: 'F10', label: 'Progress', value: opened ? done / opened : null, text: opened ? `${done}/${opened}` : null, unit: 'cases closed',
    parts: [{label: 'phase', text: front?.short || front?.label || null}],
    title: 'F10: cases of the tree that ended (terminated) / cases opened. Phase: the first phase (from the runbook view file) not yet done, derived from Canon claims.'});
  return kp;
}

const NR = '<span class="k-nr">not recorded</span>';
const shortId = (c) => (c.parent && c.id.startsWith(c.parent + '.') ? c.id.slice(c.parent.length + 1) : c.id);
const stopWhat = (x) => (x.actions.length ? x.actions.map((a) => (a === 'amend.upstream' ? 'findings (amend.upstream)' : a)).join(', ') : x.verdict.replace('_', ' '));
// one stop a person held, as a link to its event (the case, the action or the findings, how long)
export function stopLink(x, cls) {
  return `<button class="${cls}" data-goto="${esc(x.c.rel)}" data-seq="${x.seq}" title="${esc(`${x.c.id}: ${x.verdict.replace('_', ' ')} on ${stopWhat(x)} at event ${x.seq}, ${fmtDur(x.ms)}${x.open ? ' (still open)' : ''}`)}"><span class="ks-c">${esc(shortId(x.c))}</span><span class="ks-a">${esc(stopWhat(x))}</span><span class="ks-d">${esc(fmtDur(x.ms))}</span></button>`;
}
export function renderKpis(kp, now = Date.now()) {
  return kp.map((k) => {
    const main = k.text == null ? NR : k.live != null ? `<span data-since="${k.live}">${esc(fmtDur(now - k.live))}</span>` : esc(k.text);
    let sub = '';
    if (k.sub) sub = `<span class="k-p"><span class="k-pl">${esc(k.sub.label)}</span><b>${k.sub.live != null ? `<span data-since="${k.sub.live}">${esc(fmtDur(now - k.sub.live))}</span>` : esc(k.sub.text)}</b></span>`;
    for (const p of k.parts || []) sub += `<span class="k-p"><span class="k-pl">${esc(p.label)}</span><b>${p.text == null ? NR : esc(p.text)}</b></span>`;
    // a failed KPI: red, "should be 0", and the stops that made it, each a link to its event
    if (k.fail) {
      const s = k.fail.stops;
      return `<div class="kt k-${k.key} fail" data-key="kpi:${k.key}:fail"><button class="kt-main" data-kpi="${k.key}" title="${esc(`${k.ids} · ${k.title}\n${k.fail.why}\nClick for its row in Metrics.`)}"><span class="k-l">${esc(k.label)}<i>${esc(k.ids)}</i></span><span class="k-v">${main}</span><span class="k-s">${sub}</span></button>`
        + `<div class="k-fail"><b class="k-f0">${esc(k.fail.text)}</b>${s.slice(0, 1).map((x) => stopLink(x, 'k-stop')).join('')}${s.length > 1 ? `<button class="k-stop more" data-kpi="${k.key}">+${s.length - 1} more</button>` : ''}</div></div>`;
    }
    return `<button class="kt k-${k.key}" data-key="kpi:${k.key}" data-kpi="${k.key}" title="${esc(`${k.ids} · ${k.title}\nClick for its row in Metrics.`)}"><span class="k-l">${esc(k.label)}<i>${esc(k.ids)}</i></span><span class="k-v">${main}${k.unit && k.text != null ? `<small>${esc(k.unit)}</small>` : ''}</span><span class="k-s">${sub}</span></button>`;
  }).join('');
}
// The same values as rows of the Metrics tab, each with an anchor the strip links to.
export function kpiRows(kp, now = Date.now()) {
  return `<div class="sect">Run KPIs <span class="muted">the strip under the header</span></div><table class="steps kpirows"><tbody>${kp.map((k) => `<tr id="kpi-${k.key}" data-key="kr:${k.key}"${k.fail ? ' class="fail"' : ''}><td class="mono kid">${esc(k.ids)}</td><td>${esc(k.label)}</td><td class="n"><b>${k.text == null ? NR : k.live != null ? `<span data-since="${k.live}">${esc(fmtDur(now - k.live))}</span>` : esc(k.text)}</b>${[k.sub, ...(k.parts || [])].filter(Boolean).map((p) => `<div class="muted">${esc(p.label)} ${p.live != null ? `<span data-since="${p.live}">${esc(fmtDur(now - p.live))}</span>` : p.text == null ? NR : esc(p.text)}</div>`).join('')}</td></tr><tr class="kdef" data-key="kd:${k.key}"><td></td><td colspan="2" class="muted">${esc(k.title)}</td></tr>${k.fail ? `<tr class="kfail" data-key="kf:${k.key}"><td></td><td colspan="2"><b>${esc(k.fail.text)}</b> <span class="muted">${esc(k.fail.why)}</span><div class="kf-list">${k.fail.stops.map((x) => stopLink(x, 'kf-stop')).join('')}</div></td></tr>` : ''}`).join('')}</tbody></table>`;
}
