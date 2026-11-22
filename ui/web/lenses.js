// Lenses over the same data: phases, autonomy, security, metrics.
import {actorOf, ACTOR_LABEL, principalKind, caseState, evalAt, claimHistory, caseMetrics, describeEvent, seqOfEvidence} from './model.js';
import {principalOf, autopilot, actorLabel, runSettings} from './principal.js';
import {phaseItems, phaseStages, absentStages} from './view.js';

export const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (ch) => ({'&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;'}[ch]));
const nr = (why) => `<span class="nr" title="${esc(why)}">not recorded yet</span>`;

// ---- phases, derived from Canon's claims, never from a stored phase --------------------------
// The phases are the runbook's view (web/view.js, views/*.view.json). Without one, each protocol found
// in the store is one phase over all its claims, in one stage: a flat list, no grouping.
export function phaseDefs(cases) {
  const items = phaseItems();
  if (items.length) return items.map(({protocol, has_source, root_outcome, ...ph}) => ({...ph, p: protocol, hasSource: !!has_source, root: root_outcome, claims: ph.claims || []}));
  const byP = new Map();
  for (const c of cases) {
    const p = c.case?.protocol || c.ir?.protocol?.id;
    if (!p) continue;
    if (!byP.has(p)) byP.set(p, new Set());
    for (const cl of Object.keys(c.ir?.claims || {})) byP.get(p).add(cl);
  }
  return [...byP].map(([p, cl]) => ({id: p, label: p, p, claims: [...cl], stage: ''}));
}

export function phaseStates(cases) {
  const byP = {};
  for (const c of cases) (byP[c.case?.protocol || c.ir?.protocol?.id] ||= []).push(c);
  const openSeen = new Set();
  return phaseDefs(cases).map((ph) => {
    const cs = byP[ph.p] || [];
    if (!cs.length) return {...ph, status: 'none', detail: `no ${ph.p} case yet`, cases: []};
    let truths = 0, total = 0, invalidated = 0;
    const lines = [];
    for (const c of cs) {
      const ev = evalAt(c, null);
      for (const cl of ph.claims) {
        if (!c.ir?.claims?.[cl]) continue;
        total++;
        const v = ev?.claims?.[cl]?.value || 'unknown';
        if (v === 'true') truths++;
        const h = claimHistory(c, cl);
        for (let i = 1; i < h.length; i++) if (h[i - 1].value === 'true' && h[i].value !== 'true') invalidated++;
        lines.push(`${c.id}: ${cl} = ${v.toUpperCase()}`);
      }
    }
    let source = null;
    if (ph.hasSource) {
      for (const c of cs) for (const r of Object.values(c.evidence)) if (r.kind === 'spec_source') source = r.result;
    }
    const waiting = cs.find((c) => ['awaiting_authority', 'awaiting_person'].includes(caseState(c).state));
    const stuck = cs.find((c) => caseState(c).state === 'stuck');
    let status = total && truths === total ? 'done' : truths ? 'partial' : 'active';
    if (status !== 'done' && waiting && !openSeen.has(ph.p)) status = 'waiting';
    if (status !== 'done') openSeen.add(ph.p);
    if (status !== 'done' && stuck) status = 'stuck';
    if (ph.root) {
      const root = cases.find((c) => !c.parent);
      const term = root?.events.find((e) => e.event === 'terminated');
      if (status === 'done' && term?.outcome === ph.root) lines.push(`${root.id}: ended ${ph.root}`);
    }
    return {...ph, status, truths, total, invalidated, source, cases: cs, detail: lines.join('\n') + (invalidated ? `\n${invalidated}× a TRUE claim here went back (a revision invalidated it)` : '') + (waiting ? `\n${waiting.id}: ${caseState(waiting).label}` : '')};
  });
}

export function renderPhases(cases, active = new Map()) {
  const st = phaseStates(cases);
  const byId = new Map(st.map((p) => [p.id, p]));
  const firstOpen = st.find((p) => p.status !== 'done')?.id;
  const word = (p) => (p.status === 'none' ? '—' : p.status === 'done' ? 'done' : p.status === 'waiting' ? 'needs you' : p.status === 'stuck' ? 'stuck' : `${p.truths}/${p.total}`);
  const stages = (phaseItems().length ? phaseStages() : [['Protocols', st.map((p) => p.id)]]).map(([name, ids]) => {
    const ps = ids.map((id) => byId.get(id));
    const done = ps.filter((p) => p.status === 'done').length;
    return `<div class="stage" data-key="st:${esc(name)}" style="flex-grow:${ps.length}"><div class="stage-h"><span>${esc(name)}</span><span class="stage-n">${done}/${ps.length}</span></div><div class="stage-row">${ps.map((p) => `<button data-key="ph:${esc(p.id)}" class="phase ph-${p.status}${p.id === firstOpen ? ' ph-front' : ''}${active.has(p.id) ? ` ph-live lk-${active.get(p.id).kind}` : ''}" data-phase="${esc(p.id)}" data-v="${p.status}:${p.truths}" title="${esc(`${p.label}${p.doc ? ` — ${p.doc}` : ''}\nDerived from ${p.p} claims: ${p.claims.join(', ')}\n${p.detail}${active.has(p.id) ? `\nnow ${active.get(p.id).verb} ${active.get(p.id).action} in ${active.get(p.id).case}` : ''}`)}"><span class="rail"><span class="pdot"></span></span><span class="pl">${esc(p.short || p.label)}</span><span class="ps">${active.has(p.id) ? `<span class="ps-live">${esc(active.get(p.id).verb)}</span>` : esc(p.source || word(p))}${p.invalidated ? `<span class="inv" title="a TRUE claim of this phase was invalidated ${p.invalidated}× by a revision"> ↺${p.invalidated}</span>` : ''}</span></button>`).join('')}</div></div>`;
  }).join('');
  const t = absentStages().map((x) => {
    if (cases.some((c) => new RegExp(x.protocol_pattern).test(c.case?.protocol || ''))) return '';
    return `<div class="ticket" data-key="${esc(x.stage.toLowerCase())}" title="${esc(x.note)}"><span class="stage-h"><span>${esc(x.stage)}</span></span><span class="muted">${x.steps.map(esc).join(' → ')}</span> ${nr(`no ${x.protocol_pattern} protocol case in this store`)}</div>`;
  }).join('');
  return `<div class="stages" data-key="stages">${stages}</div>${t}`;
}

// ---- next human action --------------------------------------------------------------------------
export function humanNext(cases, meta) {
  const out = [];
  for (const c of cases) {
    const s = caseState(c);
    if (s.state !== 'awaiting_authority' && s.state !== 'awaiting_person') continue;
    if (Array.isArray(s.next) && s.next.some((n) => n && n.command)) {
      for (const n of s.next) out.push({case: c, action: n.action, why: n.decides || `${n.action}: ${s.label}`, cmd: n.command, from: 'the driver (verdict event `next`)'});
      continue;
    }
    const ev = evalAt(c, null);
    const base = `${meta.drive} --canon ${meta.canon} --case-dir ${c.dir} approve --at $(date -u +%FT%TZ)`;
    for (const [a, x] of Object.entries(ev?.actions || {})) {
      const b = c.bindings?.[a] || {};
      if (x.status === 'approval-required') {
        for (const r of x.reasons || []) if (r.capability) out.push({case: c, action: a, why: `${a} is approval-required: capability ${r.capability} has decision ${r.decision}`, cmd: `${base} --capability ${r.capability} --principal <you>${b.manual ? ` --action ${a}` : ''}`});
      } else if (x.status === 'admissible' && b.manual && s.state === 'awaiting_person') {
        const cap = c.ir.actions[a]?.requires?.[0]?.capability;
        out.push({case: c, action: a, why: `${a} is admissible and bound \`manual\`: a person takes it`, cmd: `${base}${cap ? ` --capability ${cap}` : ''} --principal <you> --action ${a}`});
      }
    }
    if (!out.some((o) => o.case === c)) out.push({case: c, action: null, why: s.label, cmd: null});
  }
  return out;
}

// ---- steps across the tree -----------------------------------------------------------------------
export function steps(cases) {
  const out = [];
  for (const c of cases) {
    let started = null;
    for (const e of c.events) {
      if (e.event === 'action_started') started = e;
      if (e.event === 'action_finished') {
        const discarded = c.events.find((x) => x.event === 'discarded' && x.seq > (started?.seq ?? 0) && x.seq < e.seq && x.action === e.action);
        out.push({case: c, start: started, end: e, actor: actorOf(c, e.action), discarded});
        started = null;
      }
    }
  }
  out.sort((a, b) => (a.end.at || '').localeCompare(b.end.at || '') || (a.case.depth - b.case.depth) || a.end.seq - b.end.seq);
  return out;
}

// The LLM call records of one step: the n-th run of an action in a case matches the n-th record
// for that case and action (records carry case, action and attempt, not the event seq).
function llmFor(run, step) {
  const recs = (run?.llm || []).filter(({record}) => record.action === step.end.action && (record.case == null || record.case === step.case.id));
  if (!recs.length) return [];
  const runs = step.case.events.filter((e) => e.event === 'action_started' && e.action === step.end.action);
  const n = runs.findIndex((e) => e.seq === step.start?.seq);
  return n >= 0 && recs[n] ? [recs[n]] : [];
}
export function llmTotals(run) {
  const t = {calls: 0, tin: 0, tout: 0, cache: 0, ms: 0, usd: 0, toolCalls: 0, denied: 0};
  for (const {record: r} of run?.llm || []) {
    if (!r.model && !r.usage && !r.adapter) continue;
    t.calls++;
    const u = r.usage || {};
    t.tin += Number(r.input_tokens ?? u.input_tokens ?? 0);
    t.tout += Number(r.output_tokens ?? u.output_tokens ?? 0);
    t.cache += Number(u.cache_read_input_tokens || 0) + Number(u.cache_creation_input_tokens || 0);
    t.ms += Number(r.duration_ms || 0);
    t.usd += Number(r.cost_usd || 0);
    t.toolCalls += (r.tool_calls || []).length;
    t.denied += (r.tool_calls || []).filter((c) => c.decision && c.decision.decision !== 'allow').length;
  }
  return t;
}

function llmLine(r) {
  const u = r.usage || {};
  const tin = r.input_tokens ?? u.input_tokens, tout = r.output_tokens ?? u.output_tokens;
  const dur = r.duration_ms ?? (r.duration_s != null ? r.duration_s * 1000 : null) ?? r.wall_ms;
  const bits = [r.adapter && `adapter ${r.adapter}`, r.model && `model ${r.model}`, tin != null && `in ${Number(tin).toLocaleString()} tok`, tout != null && `out ${Number(tout).toLocaleString()} tok`,
    r.tokens != null && `${Number(r.tokens).toLocaleString()} tok`, dur != null && `${(dur / 1000).toFixed(1)} s`, r.turns != null && `${r.turns} turns`, r.cost_micro_usd != null && `$${(r.cost_micro_usd / 1e6).toFixed(3)}`].filter(Boolean);
  return `<div class="llmrec">${bits.length ? esc(bits.join(' · ')) : esc(JSON.stringify(r).slice(0, 240))}</div>`;
}

const short = (id) => (id.includes('.') ? id.slice(id.indexOf('.') + 1) : id);
const badge = (k) => `<span class="badge-h b-${k}">${esc(actorLabel(k, ACTOR_LABEL))}</span>`;

// One decision already taken: by a person or by a decider agent in a person's place.
const reasonText = (r) => (typeof r === 'string' ? r : r?.text || r?.reason || r?.why || JSON.stringify(r));
export function decisionRows(list, prefix) {
  const seen = new Map();
  return list.map((d) => {
    let key = `${prefix}:${d.src}:${d.at}:${d.case}:${d.what}`;
    const n = seen.get(key) || 0;
    seen.set(key, n + 1);
    if (n) key += `:${n}`;
    const reasons = (d.reasons || []).map(reasonText).filter(Boolean);
    return `<div class="drow w-${esc(d.who)}${d.refused ? ' bad' : ''}" data-key="${esc(key)}">
      <div class="d1"><span class="stime">${esc((d.at || '').slice(5, 16).replace('T', ' ') || '—')}</span>${badge(d.who)}<b class="mono">${esc(d.what || '')}</b></div>
      ${d.question ? `<div class="d3">${esc(d.question)}</div>` : ''}
      <div class="d2"><span class="dv dv-${esc(String(d.decision || '').split(':')[0])}">${esc(d.decision ?? '—')}</span><span class="muted">by <b title="${esc(d.principal || 'no principal in the record')}">${esc(principalOf(d.principal, {model: d.model}).label)}</b>${d.ms != null ? ` · ${(d.ms / 1000).toFixed(1)} s` : ''}${d.cost != null ? ` · $${Number(d.cost).toFixed(4)}` : ''}</span>${d.dry ? '<span class="cs cs-default">dry run</span>' : ''}${d.invalid ? `<span class="cs bad">invalid: ${esc(d.invalid)}</span>` : ''}</div>
      ${reasons.length ? `<ul class="dreasons">${reasons.map((r) => `<li>${esc(r)}</li>`).join('')}</ul>` : ''}
      ${d.cites?.length ? `<div class="dcites"><span class="muted">cites</span>${d.cites.map((x) => `<code>${esc(x)}</code>`).join('')}</div>` : ''}
      <div class="foot">${d.case ? `${esc(d.case)} · ` : ''}${esc(d.src)}${d.seq ? ` #${d.seq}` : ''}${d.revision ? ` · revision ${esc(d.revision)}` : ''}${d.record ? ` · ${esc(d.record.split('/').slice(-2).join('/'))}` : ''}</div></div>`;
  }).join('');
}
export function decisionSummary(list) {
  const by = (k) => list.filter((d) => d.who === k);
  const usd = by('decider').reduce((s, d) => s + (Number(d.cost) || 0), 0);
  const legacy = by('legacy').length, rule = by('rule').length;
  return `<div class="whoact">${autopilot(runSettings()) && !by('person').length ? '' : `<span class="wa">${badge('person')}<b>${by('person').length}</b></span>`}<span class="wa">${badge('decider')}<b>${by('decider').length}</b></span>${rule ? `<span class="wa">${badge('rule')}<b>${rule}</b></span>` : ''}${legacy ? `<span class="wa">${badge('legacy')}<b>${legacy}</b></span>` : ''}${usd ? `<span class="wa"><span class="muted">decider cost</span><b>$${usd.toFixed(4)}</b></span>` : ''}</div>`;
}

export function renderAutonomy(cases, run, meta, onlyCase, history) {
  const next = humanNext(cases, meta);
  const llmAny = (run?.llm || []).length > 0;
  const st = steps(cases).filter((s) => !onlyCase || s.case.id === onlyCase);
  const counts = st.reduce((m, s) => { const k = principalKind(s.end.principal) || s.actor.kind; m[k] = (m[k] || 0) + 1; return m; }, {});
  // autopilot (autonomy A5): no person acts in the run, so nothing is listed as a person's to do
  const auto = autopilot(run?.settings);
  let h = auto ? `<div class="sect">Who acts next</div><p class="muted">Autopilot (autonomy A5): the decider or a rule takes every decision of this run.</p>` : `<div class="sect">Where a person must act next</div>`;
  if (!auto) h += next.length ? next.map((n) => `<div class="next" data-key="nx:${esc(n.case.id)}:${esc(n.action)}"><div class="nx1"><span class="badge-h b-person">YOU</span><b class="mono">${esc(n.action || '')}</b><span class="muted mono">${esc(n.case.id)}</span></div><div class="nx2">${esc(n.why)}</div>${n.cmd ? `<pre class="cmd">${esc(n.cmd)}</pre>` : ''}${n.from ? `<div class="foot">command from ${esc(n.from)}</div>` : ''}</div>`).join('') : `<p class="muted">Nobody right now: no case is awaiting a person or an authority.</p>`;
  const tt = llmTotals(run);
  h += `<div class="sect">Who acted ${onlyCase ? `in ${esc(onlyCase)}` : 'across the tree'}</div><div class="whoact">${Object.entries(counts).map(([k, n]) => `<span class="wa">${badge(k)}<b>${n}</b></span>`).join('')}</div>`;
  h += llmAny ? `<div class="llmsum"><span><b>${tt.calls}</b> LLM calls</span><span><b>${(tt.tin + tt.tout).toLocaleString()}</b> tokens in+out</span><span><b>${(tt.cache).toLocaleString()}</b> cache tokens</span><span><b>${(tt.ms / 1000).toFixed(1)} s</b> model time</span><span><b>$${tt.usd.toFixed(4)}</b></span></div>${run.expected ? `<p class="foot">${esc(run.expected.trim())}</p>` : ''}`
    : `<p class="muted">LLM calls (model, tokens, duration): ${nr('no llm-calls.jsonl next to the store; the stub executors call no model')}</p>`;
  const hist = (history || []).filter((d) => !onlyCase || d.case === onlyCase);
  h += `<div class="sect">Decisions taken <span class="muted">${auto ? 'decider or rule' : 'person or decider'}, newest first</span></div>`;
  h += hist.length ? decisionSummary(hist) + `<div class="steplist">${decisionRows(hist.slice(0, 60), 'ah')}</div>` : `<p class="muted">None yet.</p>`;
  if (!(run?.decisions || []).length) h += `<p class="foot">Decider records (model, reasons, cost): ${nr('no decisions.jsonl in the run directory')}</p>`;
  const deciderFor = (s) => (run?.decisions || []).filter((d) => d.case === s.case.id && d.what === s.end.action && d.principal === s.end.principal);
  h += `<div class="sect">Steps <span class="muted">${st.length}, oldest first</span></div><div class="steplist">`;
  for (const s of st) {
    const who = principalKind(s.end.principal) || s.actor.kind;
    const dec = who === 'decider' ? deciderFor(s) : [];
    const ev = (s.end.evidence || []).map((id) => { const r = s.case.evidence[id]; return r ? `${r.kind} = ${r.result ?? 'recorded'}` : id; }).join(', ');
    const rev = Object.keys(s.end.revises || {}).length ? `revised ${Object.keys(s.end.revises).join(', ')}` : '';
    const calls = who === 'llm' ? llmFor(run, s) : [];
    const ok = String(s.end.exit) === '0';
    h += `<div data-key="s:${esc(s.case.rel)}:${s.end.seq}" class="step w-${who}${s.discarded ? ' bad' : ''}" data-case="${esc(s.case.rel)}" data-seq="${s.end.seq}">
      <div class="s1"><span class="stime">${esc((s.end.at || '').slice(11, 16))}</span><span class="sact">${esc(s.end.action)}</span>${badge(who)}</div>
      <div class="s2"><span class="scase">${esc(s.case.id)}</span><span class="sexit ${ok ? '' : 'nz'}">exit ${esc(s.end.exit)}</span><span class="sres">${esc(ev || rev || 'no evidence')}${s.end.principal ? ` · by ${esc(principalOf(s.end.principal).label)}` : ''}</span>${s.discarded ? '<b class="bad"> discarded</b>' : ''}</div>
      ${who === 'llm' ? `<div class="s3">${calls.length ? calls.map(({record}) => llmLine(record)).join('') : nr('no LLM call record for this step')}</div>` : ''}
      ${who === 'decider' ? `<div class="s3">${dec.length ? dec.map((d) => `<div class="llmrec">${esc([d.model, d.cost != null ? `$${Number(d.cost).toFixed(4)}` : null].filter(Boolean).join(' · ') || d.principal)}</div>${(d.reasons || []).map((r) => `<div class="dr1">${esc(reasonText(r))}</div>`).join('')}`).join('') : nr('no decisions.jsonl record for this step')}</div>` : ''}</div>`;
  }
  return h + `</div>`;
}

// ---- security ------------------------------------------------------------------------------------
function toolArg(input) {
  let j = input;
  try { j = typeof input === 'string' ? JSON.parse(input) : input; } catch { /* raw */ }
  const v = j && typeof j === 'object' ? (j.file_path || j.path || j.pattern || j.command || j.url || JSON.stringify(j)) : String(input ?? '');
  return String(v).replace(/^.*\/llm\/\d+-[^/]+\/cwd\//, '').slice(0, 120);
}
function agentCard(s, run, meta) {
  const b = s.case.bindings[s.end.action] || {};
  const decl = s.case.ir.actions[s.end.action] || {};
  const lvl = Object.entries(meta.levels || {}).find(([f]) => (b.argv || []).some((a) => a.includes(f)));
  const profile = b.llm && typeof b.llm === 'object' ? b.llm : lvl ? lvl[1] : null;
  const rec = llmFor(run, s)[0]?.record;
  const did = Object.keys(s.end.revises || {});
  const kv = (k, v) => `<div class="kv"><span>${esc(k)}</span><span>${v}</span></div>`;
  let allowed = kv('argv', `<code>${esc((b.argv || []).map((a) => a.replace(/^.*\/(tools|scripts|eval)\//, '$1/')).join(' ') || '(driver)')}</code>`)
    + kv('effect', `<b>${esc(decl.effect)}</b> · may produce ${esc((decl.may_produce || []).map((m) => m.evidence).join(', ') || 'nothing')}`);
  if (rec) {
    allowed += kv('operations', esc((rec.allowed_operations || []).join(', ') || '—'))
      + kv('budget', esc(`${rec.max_turns ?? '?'} turns · $${rec.max_budget_usd ?? '?'}`))
      + kv('offered tools', `${(rec.offered_tools || []).length} <span class="muted">(gate decides per call)</span>`)
      + kv('output', `<code>${esc(rec.output || '—')}</code>`);
  } else if (profile) {
    allowed += kv('tools', esc((profile.tools?.allow || []).join(', '))) + kv('write scope', esc((profile.write_scope || []).join(', '))) + kv('egress', esc(profile.egress)) + kv('budget', esc(JSON.stringify(profile.budget || {})));
  } else allowed += kv('tools · files · egress · budget', nr('no agent profile in the binding and no call record (stub executor)'));
  let tcs = '';
  let done = kv('exit', `${esc(s.end.exit)}${rec ? ` · adapter ${esc(rec.exit)} · applied ${esc(rec.applied_exit)}` : ''}`)
    + kv('revised', did.length ? esc(did.join(', ')) : 'nothing')
    + (decl.effect === 'read' ? kv('no-write check', s.discarded ? '<b class="bad">failed · result discarded</b>' : did.length ? '<b class="bad">moved an artifact</b>' : '<span class="ok">held</span> <span class="muted">(artifact paths)</span>') : '')
    + kv('evidence', esc((s.end.evidence || []).map((id) => { const r = s.case.evidence[id]; return r ? `${r.kind}=${r.result ?? '·'} by ${r.producer}` : id; }).join(', ') || 'none'));
  if (rec) {
    const calls = rec.tool_calls || [];
    done += kv('model', esc(`${rec.model} · ${rec.num_turns} turns · ${((rec.duration_ms || 0) / 1000).toFixed(1)} s`));
    tcs = `<div class="sect">Tool calls</div><div class="tcalls">${calls.map((c) => `<div class="tc ${c.decision?.decision === 'allow' ? '' : 'deny'}"><b>${esc(c.name)}</b><span class="muted">${esc((c.operations || []).join(','))}</span><span class="tcin">${esc(toolArg(c.input))}</span><span class="tcd">${esc(c.decision?.decision || '?')}</span></div>`).join('') || '<span class="muted">no tool calls</span>'}</div>`;
  } else if (s.actor.kind === 'llm') done += kv('tool calls', nr('no tool-call trace for this step'));
  return `<div class="agent a-${s.actor.kind}" data-key="ag:${esc(s.case.rel)}:${s.end.seq}"><div class="ag-h"><b class="mono">${esc(s.end.action)}</b>${badge(s.actor.kind)}<span class="muted mono">${esc(s.case.id)} · #${s.end.seq}</span><a href="#" data-blob="${esc(s.end.log)}" data-case="${esc(s.case.rel)}">output</a></div>
    <div class="cols2"><div><i>allowed</i>${allowed}</div><div><i>did</i>${done}</div></div>${tcs}</div>`;
}

export function renderSecurity(cases, run, meta, onlyCase) {
  const cs = onlyCase ? cases.filter((c) => c.id === onlyCase) : cases;
  let h = `<div class="gap"><b>Known gap:</b> Canon authority is not bound to a revision. A grant in <code>authority.json</code> has no principal and no revision (<code>canon-authority/1</code>), so one grant admits every later revision; only the driver's <code>manual</code> rule keeps each approval a separate person's step (docs/driver.md "Authority", gap 1).</div>`;
  h += `<h3>Approvals and what they are bound to</h3><table class="steps"><tr><th>case</th><th>record</th><th>bound to</th></tr>`;
  for (const c of cs) {
    for (const g of c.authority || []) h += `<tr class="warn"><td class="mono">${esc(short(c.id))}</td><td>grant <b>${esc(g.capability)}</b> ${esc(g.decision)}${(() => { const e = c.events.find((x) => x.event === 'authority_recorded' && x.capability === g.capability); return e ? ` by ${esc(e.principal)} at ${esc(e.at)} (event #${e.seq})` : ''; })()}</td><td>nothing: any revision</td></tr>`;
    for (const r of Object.values(c.evidence)) if (/approval|merge_record|release_record/.test(r.kind)) {
      const copied = c.bindings?.[r.id.replace(/^e\d+-/, '').replace(/\.[^.]*$/, '')]?.copy;
      h += `<tr><td class="mono">${esc(short(c.id))}</td><td>${esc(r.kind)}=${esc(r.result ?? '·')} by <b>${esc(r.producer)}</b>${copied ? ' (copied from another case)' : ''}</td><td>${esc(r.subject)}@${esc(r.subject_revision)}${r.upstream_revisions ? ' + ' + esc(Object.entries(r.upstream_revisions).map(([a, v]) => `${a}@${v}`).join(', ')) : ''}</td></tr>`;
    }
  }
  if (!/<td>/.test(h.split("Approvals and what")[1] || "")) h += `<tr><td colspan="3" class="muted">No grant and no approval record yet.</td></tr>`;
  h += `</table><h3>Independent producers</h3><table class="steps"><tr><th>case</th><th>producer</th><th>records</th><th>independent on</th></tr>`;
  for (const c of cs) {
    const prods = {};
    for (const r of Object.values(c.evidence)) (prods[r.producer] ||= []).push(r.kind);
    const ind = c.case?.independence || {};
    for (const [p, kinds] of Object.entries(prods)) h += `<tr><td class="mono">${esc(short(c.id))}</td><td>${esc(p)}</td><td>${esc([...new Set(kinds)].join(', '))}</td><td>${ind[p] ? `<b>${esc(ind[p].join(', '))}</b>` : '—'}</td></tr>`;
  }
  h += `</table><h3>Agent steps: allowed vs did</h3>`;
  const st = steps(cs).filter((s) => s.actor.kind === 'llm' || s.actor.kind === 'review' || s.actor.kind === 'program');
  const agent = st.filter((s) => s.actor.kind !== 'program');
  for (const s of agent) h += agentCard(s, run, meta);
  const disc = cs.flatMap((c) => c.events.filter((e) => e.event === 'discarded').map((e) => ({c, e})));
  h += `<h3>Discarded results</h3>${disc.length ? disc.map(({c, e}) => `<div class="bad">${esc(c.id)} #${e.seq} ${esc(e.action)}: ${esc(e.reason)}</div>`).join('') : '<p class="muted">0 discarded events (an observing action that changed an artifact would show here).</p>'}`;
  return h;
}

// ---- metrics ------------------------------------------------------------------------------------
// Every event of the tree in one order: event clock first, then tree depth, then sequence.
export function globalEvents(cases) {
  const all = [];
  for (const c of cases) for (const e of c.events) all.push({c, e});
  all.sort((a, b) => (a.e.at === '-' ? '' : a.e.at || '').localeCompare(b.e.at === '-' ? '' : b.e.at || '') || a.c.depth - b.c.depth || a.e.seq - b.e.seq);
  return all;
}
function cumulative(all, pred, n = 36) {
  if (!all.length) return [];
  const out = [];
  let k = 0, j = 0;
  for (let i = 1; i <= n; i++) {
    const upto = Math.round((i / n) * all.length);
    for (; j < upto; j++) if (pred(all[j])) k++;
    out.push(k);
  }
  return out;
}
export function spark(values, cls = '') {
  if (!values || values.length < 2) return `<svg class="spark ${cls}" viewBox="0 0 100 28"></svg>`;
  const max = Math.max(...values, 1), min = Math.min(...values, 0);
  const pts = values.map((v, i) => [(i / (values.length - 1)) * 100, 26 - ((v - min) / (max - min || 1)) * 23]);
  const line = pts.map((p, i) => `${i ? 'L' : 'M'}${p[0].toFixed(1)},${p[1].toFixed(1)}`).join('');
  return `<svg class="spark ${cls}" viewBox="0 0 100 28" preserveAspectRatio="none"><path class="area" d="${line}L100,28L0,28Z"/><path class="line" d="${line}"/></svg>`;
}

export function renderMetrics(cases, run, meta, selected) {
  const rows = cases.map((c) => ({c, m: caseMetrics(c, meta)}));
  const sum = (k) => rows.reduce((n, r) => n + (r.m[k] || 0), 0);
  const sel = rows.find((r) => r.c.id === selected);
  const llmRecs = run?.llm || [];
  const lt = llmTotals(run);
  const tok = lt.tin + lt.tout;
  const all = globalEvents(cases);
  const kind = (x) => (x.e.event === 'action_finished' ? (principalKind(x.e.principal) || actorOf(x.c, x.e.action).kind) : null);
  const tile = (label, v, kpi, series, title = '', cls = '') => `<div class="tile ${cls}" data-key="t:${esc(label)}" data-v="${esc(v)}" title="${esc(title)}"><div class="tl">${esc(label)}${kpi ? `<span class="kpi">${kpi}</span>` : ''}</div><div class="tv">${v ?? '<span class="na">—</span>'}</div>${series ? spark(series, cls) : ''}</div>`;
  const wall = (() => { const ats = all.map((x) => Date.parse(x.e.at)).filter((x) => !isNaN(x)); return ats.length ? Math.round((Math.max(...ats) - Math.min(...ats)) / 60000) : null; })();
  let h = `<div class="sect">Whole tree · ${rows.length} cases</div><div class="tiles">
    ${tile('Driver steps', sum('steps'), 'R9', cumulative(all, (x) => x.e.event === 'action_finished'))}
    ${tile('LLM steps', sum('llmSteps'), 'R11', cumulative(all, (x) => kind(x) === 'llm'), 'steps of actions bound to an LLM executor', 'llm')}
    ${tile('Person steps', sum('personSteps'), 'R8', cumulative(all, (x) => kind(x) === 'person'), 'steps taken by a person through approve --action', 'person')}
    ${sum('deciderSteps') ? tile('Decider steps', sum('deciderSteps'), '', cumulative(all, (x) => kind(x) === 'decider'), 'approve --action by a decider:* principal', 'decider') : ''}
    ${tile('Grants', sum('approvals'), 'R8', cumulative(all, (x) => x.e.event === 'authority_recorded'), 'authority_recorded events', 'person')}
    ${tile('Revisions', sum('revisions'), '', cumulative(all, (x) => x.e.event === 'revised'), 'artifact revisions (revised events)')}
    ${tile('Invalidated', sum('invalidations'), '', null, 'evidence records Canon excluded as revision_mismatch at some evaluation', 'bad')}
    ${tile('Evaluations', sum('evaluations'), '', cumulative(all, (x) => x.e.event === 'evaluated'))}
    ${tile('Discarded', sum('discarded'), 'R15', null, 'observations that changed an artifact', sum('discarded') ? 'bad' : '')}
    ${tile('Failed exits', sum('failedSteps'), '', cumulative(all, (x) => x.e.event === 'action_finished' && String(x.e.exit) !== '0'), 'tool exits other than 0 (a check that said no is evidence too)')}
    ${tile('LLM calls', lt.calls || null, 'R4', null, lt.calls ? `${(lt.ms / 1000).toFixed(1)} s model time, ${lt.toolCalls} tool calls, ${lt.denied} denied` : 'not recorded yet: no llm-calls.jsonl', 'llm')}${tile('LLM cost', lt.calls ? `$${lt.usd.toFixed(3)}` : null, 'R6', null, lt.calls ? 'sum of cost_usd in llm-calls.jsonl' : 'not recorded yet', 'llm')}
    ${tile('Tokens', tok ? tok.toLocaleString() : null, 'R5', null, tok ? '' : 'not recorded yet: the run log holds no token counts')}
    ${tile('Wall time', wall != null ? `${wall} min` : null, 'R7', null, 'from the --at instants the caller passed, not a real clock')}
  </div>
  <div class="tiles tiles-struct">${tile('State space Σ', rows.reduce((n, r) => n + (r.m.states || 0), 0).toLocaleString(), 'S1', null, 'sum of canon check states over the protocols of this tree (docs/check-output.txt)')}
    ${tile('LLM actions', sum('llmActions'), 'S14', null, 'actions bound to an LLM executor, over the protocols of this tree', 'llm')}
    ${tile('Person actions', sum('personActions'), '', null, 'actions bound manual', 'person')}</div>`;
  if (sel) {
    const m = sel.m;
    const truths = sel.c.events.filter((e) => e.event === 'evaluated' && sel.c.evaluations[e.file]).map((e) => Object.values(sel.c.evaluations[e.file].claims || {}).filter((v) => v.value === 'true').length);
    h += `<div class="sect">Selected · ${esc(sel.c.id)}</div><div class="tiles">
      ${tile('Claims TRUE', `${truths[truths.length - 1] ?? 0}<small>/${m.claims}</small>`, '', truths, 'claims TRUE at each stored evaluation; a dip is an invalidation', 'true')}
      ${tile('States', m.states?.toLocaleString(), 'S1', null, m.stateSource)}
      ${tile('Check findings', m.findings, 'S3–S7', null, 'canon check findings for this protocol revision')}
      ${tile('Actions', `${m.actions}<small> · ${m.llmActions} LLM</small>`, '', null, `${m.personActions} person, ${m.reviewActions} review`)}
      ${tile('Steps', m.steps, 'R9', null)}
      ${tile('Open obligations', m.open, '', null)}
    </div>`;
  }
  h += `<div class="sect">Per case</div><table class="steps"><thead><tr><th>case</th><th class="n">states</th><th class="n">actions</th><th class="n">LLM</th><th class="n">steps</th><th class="n">LLM steps</th><th class="n">person</th><th class="n">rev</th><th class="n">inval</th></tr></thead><tbody>`;
  for (const {c, m} of rows) h += `<tr data-key="mc:${esc(c.rel)}" data-case="${esc(c.rel)}" class="${c.id === selected ? 'cur' : ''}"><td class="mono">${esc(c.id)}</td><td class="n">${m.states?.toLocaleString() ?? '—'}</td><td class="n">${m.actions}</td><td class="n">${m.llmActions}</td><td class="n">${m.steps}</td><td class="n">${m.llmSteps}</td><td class="n">${m.personSteps}</td><td class="n">${m.revisions}</td><td class="n">${m.invalidations}</td></tr>`;
  h += `</tbody></table><p class="foot">KPI ids from docs/kpis.md. States from the <code>canon check</code> lines in docs/check-output.txt. LLM call durations and tokens: ${llmRecs.length ? 'see Autonomy' : nr('not in the run log')}.</p>`;
  return h;
}

export {seqOfEvidence, describeEvent};
