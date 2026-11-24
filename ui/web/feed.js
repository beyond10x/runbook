// "What changed": every change of the run by what it is (planning, specification, direction,
// decision, code), newest first, with the meaning in a sentence and the record one click away.
// Sources: fs/changes.jsonl with each change's meaning (server, web/meaning.js) where the run has
// it; else what the case store still says (revisions per action, AEP transitions); decisions from
// the inbox history (decisions.jsonl, approvals and manual steps in events.jsonl, answered questions).
import {esc} from './lenses.js';
import {actorOf} from './model.js';
import {KIND_LABEL, fmtAgo, meaningFor, classBadge, stoppedNote} from './now.js';
import {CLASSES, CLASS_LABEL, classify, storeItems} from './meaning.js';
import {questionsKind} from './view.js';
import {principalOf, autopilot, runSettings} from './principal.js';

const clock = (t) => (t == null || isNaN(t) ? '' : new Date(t).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'}));
// who decided, from the record's principal (web/principal.js): "Person" only for a person
const cap = (s) => s[0].toUpperCase() + s.slice(1);
const whoOfPrincipal = (p, model) => { const x = principalOf(p, {model}); return x.kind === 'none' ? null : x.kind === 'agent' ? {kind: 'driver', label: x.label} : {kind: x.kind, label: x.kind === 'person' ? `Person ${x.id}` : cap(x.label)}; };
const DECISION_WORD = {approve: 'approved', approved: 'approved', reject: 'rejected', rejected: 'rejected', escalate: 'escalated to a person', escalation_suppressed: 'escalation suppressed', granted: 'granted'};

function whoOfChange(x, cases) {
  const a = x.attribution || {};
  if (a.record) return {kind: a.record.startsWith('decider/') ? 'decider' : 'llm', label: `${a.record.startsWith('decider/') ? 'Decider' : 'LLM agent'}${a.tool ? ` · ${a.tool.name}` : ''}`};
  if (a.action) {
    const c = cases.find((k) => k.id === a.case);
    const k = c ? actorOf(c, a.action).kind : 'driver';
    return {kind: k, label: KIND_LABEL[k] || k};
  }
  return {kind: 'none', label: 'unattributed'};
}

export function feedItems(cases, live, history, facts) {
  const out = [];
  const fsOn = !!live?.fs?.present;
  if (fsOn) {
    let group = null;
    for (const x of live.fs.changes) {
      const m = meaningFor(x) || {cls: 'code', items: [{text: x.path}]};
      const a = x.attribution || {};
      const base = {case: a.case || null, action: a.action || null, who: whoOfChange(x, cases), t: Date.parse(x.at)};
      if (m.cls === 'code') {
        // consecutive code changes of one step make one row
        const key = `${a.case}|${a.action}|${a.record || ''}`;
        if (group && group.key === key) { group.files.push(x); group.t = base.t; group.id = `fs:${group.files[0].seq}`; continue; }
        group = {id: `fs:${x.seq}`, key, cls: 'code', files: [x], ...base};
        out.push(group);
        continue;
      }
      group = null;
      out.push({id: `fs:${x.seq}`, cls: m.cls, text: m.items[0]?.text || x.path, more: m.items.slice(1).map((i) => i.text), parsed: m.parsed !== false, fs: x.seq, path: x.path, ...base});
    }
    for (const g of out) if (g.cls === 'code') {
      const add = g.files.reduce((n, x) => n + (x.lines_added || 0), 0), del = g.files.reduce((n, x) => n + (x.lines_removed || 0), 0);
      const paths = [...new Set(g.files.map((x) => x.path))];
      g.text = paths.length === 1 ? `${g.files.length > 1 ? `${g.files.length} changes to` : g.files[0].op === 'create' ? 'created' : g.files[0].op === 'delete' ? 'deleted' : 'changed'} ${paths[0]}` : `${paths.length} code files changed`;
      g.add = add; g.del = del; g.fs = g.files[g.files.length - 1].seq;
    }
  } else {
    for (const it of storeItems(cases, facts?.files || [])) {
      const c = cases.find((k) => k.rel === it.rel || k.id === it.case);
      const who = whoOfPrincipal(it.principal) || (c && it.action ? (() => { const k = actorOf(c, it.action).kind; return k === 'person' && autopilot(runSettings()) ? {kind: 'decider', label: 'Decider'} : {kind: k, label: KIND_LABEL[k] || k}; })() : {kind: 'none', label: '—'});
      out.push({...it, who, store: true});
    }
  }
  const qFromFs = fsOn && live.fs.changes.some((x) => classify(x.path).kind === questionsKind());
  (history || []).forEach((d, i) => {
    if (d.src === 'ui-actions.jsonl' || d.refused) return;
    const who = whoOfPrincipal(d.principal, d.model) || {kind: 'none', label: 'Principal not recorded'};
    if (who.kind === 'driver') return;
    const t = Date.parse(d.at);
    if (d.question || (d.src && /questions/.test(d.src))) {
      if (qFromFs) return;
      out.push({id: `q:${d.case}:${d.what}`, cls: 'direction', text: `question ${d.what} answered: “${String(d.decision ?? '').slice(0, 70)}”`, more: d.question ? [d.question] : [], case: d.case, action: null, who, t: isNaN(t) ? null : t, grounds: i, note: d.at ? null : 'when it was answered is not recorded'});
      return;
    }
    const dec = String(d.decision ?? '');
    const word = DECISION_WORD[dec.split(':')[0]] || (dec.startsWith('recorded ') ? `took it: ${dec.slice(9)}` : dec);
    out.push({id: `d:${i}:${d.case}:${d.what}:${d.at}`, cls: 'decision', text: `${d.what}: ${word}`, more: (d.reasons || []).slice(0, 1).map((r) => (typeof r === 'string' ? r : r?.text || JSON.stringify(r))), case: d.case, action: /^grant /.test(d.what || '') ? null : d.what, who, t: isNaN(t) ? null : t, grounds: i});
  });
  // newest first; changes written in the same instant keep their record order, the later on top
  out.forEach((x, i) => { x.ord = i; });
  out.sort((a, b) => (b.t ?? 0) - (a.t ?? 0) || b.ord - a.ord);
  return out;
}

// loom: the run is one of `runbook run` (its cases carry parts.loom): without fs/changes.jsonl the run was started without --watch
export function renderFeed(items, filter, now = Date.now(), fsOn = true, loom = false, stopped = null) {
  const counts = Object.fromEntries(CLASSES.map((c) => [c, items.filter((x) => x.cls === c).length]));
  const shown = items.filter((x) => !filter || x.cls === filter).slice(0, 120);
  const chips = `<div class="fd-chips" data-key="chips"><button class="fchip${!filter ? ' on' : ''}" data-feed="" data-key="fc:all">All<i>${items.length}</i></button>${CLASSES.map((c) => `<button class="fchip c-${c}${filter === c ? ' on' : ''}${counts[c] ? '' : ' zero'}" data-feed="${c}" data-key="fc:${c}" title="${esc(CLASS_LABEL[c])}">${esc(CLASS_LABEL[c])}<i>${counts[c]}</i></button>`).join('')}</div>`;
  const rows = shown.map((x) => {
    const tt = x.t != null ? `<span class="fd-t" title="${esc(new Date(x.t).toLocaleString())}">${x.t > now - 3600000 ? `<span data-ago="${x.t}">${esc(fmtAgo(now - x.t))}</span>` : esc(clock(x.t))}</span>` : `<span class="fd-t muted" title="${esc(x.note || 'time not recorded')}">—</span>`;
    const target = x.fs != null ? ` data-fs="${x.fs}"` : x.grounds != null ? ` data-grounds="${x.grounds}"` : '';
    const files = x.cls === 'code' && x.files?.length > 1 ? `<span class="fd-files">${[...new Map(x.files.map((f) => [f.path, f])).values()].slice(-4).map((f) => `<a href="#" data-fs="${f.seq}" class="fd-f">${esc(f.path.split('/').pop())}</a>`).join('')}</span>` : '';
    return `<div class="fd-row c-${x.cls}${target ? ' click' : ''}${x.parsed === false ? ' unparsed' : ''}" data-key="fd:${esc(x.id)}"${target} title="${esc([x.text, ...(x.more || []), x.path ? `file: ${x.path}` : '', x.note || ''].filter(Boolean).join('\n'))}">
      <div class="fd-1">${classBadge(x.cls)}${target ? `<button class="fd-s"${target}>${esc(x.text)}</button>` : `<span class="fd-s">${esc(x.text)}</span>`}${x.add != null ? `<span class="fa">+${x.add}</span><span class="fd">−${x.del}</span>` : ''}</div>
      ${x.more?.length ? `<div class="fd-m">${esc(x.more[0])}${x.more.length > 1 ? ` <i>+${x.more.length - 1}</i>` : ''}</div>` : ''}${files}
      <div class="fd-2">${x.case ? `<span class="fd-c">${esc(x.case.split('.').slice(-1)[0])}</span>` : ''}${x.action ? `<span class="fd-a">${esc(x.action)}</span>` : ''}<span class="fd-w w-${esc(x.who?.kind || 'none')}">${esc(x.who?.label || '—')}</span>${tt}</div></div>`;
  }).join('');
  const src = fsOn ? '' : loom ? `<p class="fd-note" data-key="fdnote">Changes were not recorded for this run: it was started without <code>--watch</code>. <code>runbook run … --watch</code> records every file change of the run in <code>fs/changes.jsonl</code>.</p>` : `<p class="fd-note" data-key="fdnote">No <code>fs/changes.jsonl</code> in this run: revisions and decisions from the case store; what changed inside a revision is not recorded.</p>`;
  return `<div class="fd-h" data-key="fdh"><span class="fd-title">What changed</span><span class="muted">newest first</span></div>${chips}${src}${stoppedNote(stopped)}<div class="fd-list" data-key="fdl" tabindex="0" role="region" aria-label="What changed, newest first">${rows || (!fsOn && loom ? '' : `<p class="muted fd-empty">Nothing ${filter ? `of class ${esc(CLASS_LABEL[filter])} ` : ''}yet.</p>`)}</div>`;
}
