// Explanation panel: Canon's own explanation (canon-decision/1 `explanation`) for one node,
// followed down to the evidence records, the event that produced each, and the revision that
// excluded it.
import {caseState, actorOf, ACTOR_LABEL, producerEvent, supersedingRevision, predicateText, claimHistory, seqOfEvidence} from './model.js';
import {esc} from './lenses.js';
import {autopilot, runSettings, actorLabel, principalOf} from './principal.js';

const val = (v) => `<span class="v v-${esc(v)}">${esc(String(v).toUpperCase())}</span>`;

function recordCard(c, id, status, reason, atSeq) {
  const r = c.evidence[id];
  if (!r) return `<div class="rec">${esc(id)} (record not in evidence/)</div>`;
  const pe = producerEvent(c, id);
  const cur = c._revisions?.[r.subject]?.revision;
  let why = '';
  if (status === 'excluded') {
    const sup = supersedingRevision(c, r);
    why = `<div class="why bad">Canon set it aside: <b>${esc(reason)}</b>.`;
    if (reason === 'revision_mismatch') why += ` The record is about <code>${esc(r.subject)}@${esc(r.subject_revision)}</code>; the case now holds <code>${esc(r.subject)}@${esc(cur)}</code>.` +
      (sup ? ` Revision event #${sup.event.seq} (${esc(sup.event.at)}) moved <code>${esc(r.subject)}</code> to <code>${esc(sup.to)}</code>${sup.by ? `, written by action <b>${esc(sup.by)}</b> (${ACTOR_LABEL[actorOf(c, sup.by).kind]})` : ''}.` : '');
    else if (reason === 'not_independent') why += ` Producer <code>${esc(r.producer)}</code> is not listed in the case's <code>independence</code>.`;
    why += `</div>`;
  } else if (status === 'applied') {
    why = `<div class="why ok">Applied: about <code>${esc(r.subject)}@${esc(r.subject_revision)}</code>, which is the current revision.</div>`;
  }
  return `<div class="rec ${status === 'excluded' ? 'excl' : ''}">
    <div><b>${esc(r.id)}</b> <span class="muted">canon-evidence/1</span></div>
    <div>kind <b>${esc(r.kind)}</b> = <b>${esc(r.result ?? '(any)')}</b> · producer <b>${esc(r.producer)}</b>${c.case?.independence?.[r.producer] ? ' (independent)' : ''}</div>
    <div>about <code>${esc(r.subject)}@${esc(r.subject_revision)}</code>${r.upstream_revisions ? ` · upstream ${esc(Object.entries(r.upstream_revisions).map(([a, v]) => `${a}@${v}`).join(', '))}` : ''}${r.child_case ? ` · child <b>${esc(r.child_case)}</b>` : ''}</div>
    <div class="muted">observed ${esc(r.observed_at)}${pe ? ` · from event #${pe.seq} ${esc(pe.event === 'action_finished' ? `${pe.action} exit ${pe.exit}${pe.principal ? ' by ' + principalOf(pe.principal).label : ''}` : pe.event)}` : ''}${pe?.log ? ` · <a href="#" data-blob="${esc(pe.log)}" data-case="${esc(c.rel)}">tool output</a>` : ''}</div>
    ${why}</div>`;
}

function becauseList(c, because, ev, atSeq) {
  if (!because?.length) return '<p class="muted">Canon lists no explanation entry for this. The predicate above shows each input with its current value.</p>';
  return because.map((b) => {
    if (b.evidence) return recordCard(c, b.evidence, b.status, b.reason, atSeq);
    if (b.claim) return `<div class="bec">claim <a href="#" data-node="claim:${esc(b.claim)}">${esc(b.claim)}</a> is ${val(b.value)}</div>`;
    if (b.kind) return `<div class="bec warn">no applicable <b>${esc(b.kind)}</b> record about <code>${esc(b.subject)}</code> at its current revision <code>${esc(c._revisions?.[b.subject]?.revision)}</code> (${esc(b.status)})</div>`;
    if (b.capability) return `<div class="bec warn">capability <b>${esc(b.capability)}</b>: decision <b>${esc(b.decision)}</b> in authority.json</div>`;
    if (b.decision) return `<div class="bec">decision <b>${esc(b.decision)}</b> ${b.present ? 'present' : 'absent'} in decisions.json</div>`;
    if (b.child_case || b.children) return `<div class="bec">${esc(JSON.stringify(b))}</div>`;
    return `<div class="bec"><code>${esc(JSON.stringify(b))}</code></div>`;
  }).join('');
}

export function renderExplain(c, nodeId, ev, atSeq) {
  if (!c) return '<p class="muted">Select a case.</p>';
  if (!nodeId) return intro(c, ev);
  const [kind, ...rest] = nodeId.split(':');
  const name = rest.join(':');
  const ir = c.ir;
  const x = ev?.explanation || {};
  let h = `<div class="ehead"><span class="kind">${esc(kind)}</span> <b>${esc(name)}</b></div>`;
  if (kind === 'claim') {
    const d = ir.claims[name];
    const v = ev?.claims?.[name]?.value || 'unknown';
    h += `<p>${esc(d?.description)}</p><p>Value: ${val(v)}</p><pre class="pred">true when\n${esc(predicateText(d?.true_when, ev?.claims, 1))}</pre>`;
    h += `<h4>Why (Canon's explanation)</h4>` + becauseList(c, x.claims?.[name]?.because, ev, atSeq);
    const ce = '';
    const hist = claimHistory(c, name);
    h += ce + `<h4>History over the stored evaluations</h4><div class="hist">${hist.map((p) => `<a href="#" data-seq="${p.seq}" class="hp v-${esc(p.value)}" title="event #${p.seq} ${esc(p.at)}">#${p.seq} ${esc(String(p.value).toUpperCase())}</a>`).join(' → ')}</div>`;
    const inv = Object.entries(ir.invalidation || {}).filter(([, i]) => (i.invalidates || []).includes(name));
    if (inv.length) h += `<h4>Invalidated by</h4>` + inv.map(([id, i]) => `<div class="bec">${esc(id)}: a revision of <b>${esc(i.upstream)}</b> — ${esc(i.description)}</div>`).join('');
  } else if (kind === 'action') {
    const d = ir.actions[name];
    const a = actorOf(c, name);
    const st = ev?.actions?.[name]?.status;
    h += `<p>${esc(d?.description)}</p><p>Status: <span class="v st-${esc(st)}">${esc(st)}</span> · effect <b>${esc(d?.effect)}</b> · actor <span class="badge-h b-${a.kind}">${actorLabel(a.kind, ACTOR_LABEL)}</span></p><p class="muted">${esc(a.why)}</p>`;
    h += `<pre class="pred">precondition\n${esc(predicateText(d?.precondition, ev?.claims, 1))}${d?.requires?.length ? `\nrequires capability ${d.requires.map((r) => r.capability).join(', ')}` : ''}\nmay produce: ${(d?.may_produce || []).map((m) => m.evidence).join(', ') || 'nothing (a writer: its only effect is a new artifact revision)'}</pre>`;
    h += `<h4>Why (Canon's explanation)</h4>` + (x.actions?.[name] ? becauseList(c, x.actions[name].because, ev, atSeq) : `<p class="muted">${st === 'admissible' ? 'Admissible: every precondition holds and no capability is missing.' : 'No explanation entry.'}</p>`);
    const b = c.bindings?.[name];
    h += `<h4>Binding</h4><pre class="pred">${esc((b === undefined && a.kind === 'unavailable' ? a.why : JSON.stringify(b ?? 'unbound', null, 1)).replace(/\/home\/[^"]*\/tools\//g, 'tools/'))}</pre>`;
    const runs = c.events.filter((e) => e.action === name && e.event === 'action_finished' && (atSeq == null || e.seq <= atSeq));
    h += `<h4>Runs (${runs.length})</h4>` + (runs.map((e) => `<div class="bec"><a href="#" data-seq="${e.seq}">#${e.seq}</a> ${esc(e.at)} exit <b>${esc(e.exit)}</b>${e.principal ? ` by ${esc(principalOf(e.principal).label)}` : ''} → ${esc((e.evidence || []).join(', ') || 'no evidence')}${Object.keys(e.revises || {}).length ? ` · revised ${esc(Object.keys(e.revises).join(', '))}` : ''} · <a href="#" data-blob="${esc(e.log)}" data-case="${esc(c.rel)}">output</a></div>`).join('') || '<p class="muted">never ran</p>');
  } else if (kind === 'outcome' || kind === 'obligation') {
    const d = kind === 'outcome' ? ir.outcomes[name] : ir.obligations[name];
    const st = kind === 'outcome' ? ev?.outcomes?.[name]?.status : (ev?.obligations || []).find((o) => o.id === name)?.status;
    h += `<p>${esc(d?.description)}</p><p>Status: <span class="v st-${esc(st)}">${esc(st)}</span></p><pre class="pred">${kind === 'outcome' ? 'requires' : 'discharged when'}\n${esc(predicateText(kind === 'outcome' ? d?.requires : d?.discharged_when, ev?.claims, 1))}</pre>`;
    h += `<h4>Why (Canon's explanation)</h4>` + becauseList(c, (kind === 'outcome' ? x.outcomes : x.obligations)?.[name]?.because, ev, atSeq);
  } else if (kind === 'evidence') {
    const d = ir.evidence_kinds?.[name];
    h += `<p>${esc(d?.description || (name === 'case_outcome' ? 'one record per child case: open, then the child\'s outcome' : ''))}</p>`;
    const recs = Object.values(c.evidence).filter((r) => r.kind === name && (atSeq == null || (seqOfEvidence(r.id) ?? 0) <= atSeq));
    const used = {};
    for (const [cl, e] of Object.entries(x.claims || {})) for (const b of e.because || []) if (b.evidence) used[b.evidence] = {claim: cl, status: b.status, reason: b.reason};
    h += `<h4>Records (${recs.length})</h4>` + (recs.map((r) => recordCard(c, r.id, used[r.id]?.status, used[r.id]?.reason, atSeq) + (used[r.id] ? `<div class="muted small">used by claim ${esc(used[r.id].claim)}</div>` : '')).join('') || '<p class="muted">none yet</p>');
  } else if (kind === 'artifact') {
    const d = ir.artifacts?.[name];
    h += `<p>${esc(d?.description)}</p><p>Current revision <code>${esc(c._revisions?.[name]?.revision)}</code> · path <code>${esc(c.bindings?.$artifacts?.[name] ?? '(not placed in work/)')}</code></p>`;
    const revs = c.events.filter((e) => e.event === 'revised' && e.artifacts?.[name] && (atSeq == null || e.seq <= atSeq));
    h += `<h4>Revisions (${revs.length})</h4>` + revs.map((e) => {
      const started = [...c.events].reverse().find((s) => s.seq < e.seq && s.event === 'action_started');
      return `<div class="bec"><a href="#" data-seq="${e.seq}">#${e.seq}</a> ${esc(e.at)} → <code>${esc(e.artifacts[name])}</code>${started ? ` after ${esc(started.action)}` : ' (before any action, or written outside the driver)'} · case ${esc(e.case_revision)}</div>`;
    }).join('');
    const inv = Object.entries(ir.invalidation || {}).filter(([, i]) => i.upstream === name);
    if (inv.length) h += `<h4>A revision of ${esc(name)} invalidates</h4>` + inv.map(([, i]) => `<div class="bec">${(i.invalidates || []).map((cl) => `<a href="#" data-node="claim:${esc(cl)}">${esc(cl)}</a>`).join(', ')} — ${esc(i.description)}</div>`).join('');
  } else if (kind === 'decision') {
    h += `<p>An explicit outcome decision, recorded in decisions.json (canon-decisions/1). Current: ${esc(JSON.stringify(c.decisions))}</p>`;
  }
  return h;
}

function intro(c, ev) {
  const cf = ev?.explanation?.computed_from;
  const st = caseState(c);
  const waiting = (st.state === 'awaiting_authority' || st.state === 'awaiting_person') && Array.isArray(st.next)
    ? st.next.filter((n) => n && n.command).map((n) => `<div class="next"><div class="nx1">${autopilot(runSettings()) ? '<span class="badge-h b-decider">DECIDER</span>' : '<span class="badge-h b-person">YOU</span>'}<b class="mono">${esc(n.action)}</b><span class="muted">${esc(st.label)}</span></div><div class="nx2">${esc(n.decides || '')}</div><pre class="cmd">${esc(n.command)}</pre></div>`).join('') : '';
  return waiting + `<div class="ehead"><span class="kind">case</span> <b>${esc(c.id)}</b> <span class="muted">${esc(c.case?.protocol)} r${esc(c.ir?.protocol?.revision)}</span></div>
  <p>${esc(c.ir?.protocol?.description)}</p>
  ${cf ? `<p class="muted">This picture is the stored canon-decision/1 computed at ${esc(cf.at)} from case revision ${esc(cf.case?.revision)}, ${cf.evidence?.length} evidence records, ${cf.authority?.length} authority entries, ${cf.decisions?.length} decisions (Canon ${esc(cf.semantics?.canon)}).</p>` : ''}
  <div class="learn"><b>How to read the graph.</b> Programs and LLMs run <i>actions</i>. A program's exit code becomes a <i>canon-evidence/1</i> record about one artifact at one revision. A <i>claim</i> is TRUE only while a record about the <u>current</u> revision supports it; when an artifact is revised, older records are set aside (<i>excluded</i>), so the claim falls back to UNKNOWN. An LLM action (purple) writes only an artifact: it never produces evidence itself. Actions are admissible when the claims they read hold; outcomes become legitimate when the claims they require are TRUE. Click any node for Canon's explanation down to the records.</div>`;
}
