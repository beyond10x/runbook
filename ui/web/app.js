import {renderGraph} from './graph.js';
import {renderExplain} from './explain.js';
import {renderPhases, renderAutonomy, renderSecurity, renderMetrics, phaseStates, esc} from './lenses.js';
import {caseState, evalAt, evalEventAt, lastSeq, describeEvent, EVENT_CLASS} from './model.js';
import {patch} from './morph.js';
import {renderDrawer} from './inbox.js';
import {nowState, renderNow, renderNowBar, activePhases, graphLive, renderChanges, renderDiff, fmtDur, fmtAgo} from './now.js';
import {computeKpis, renderKpis, kpiRows} from './kpis.js';
import {feedItems, renderFeed} from './feed.js';
import {decisionRows} from './lenses.js';
import {setRunSettings, autopilot, standIn} from './principal.js';

const q0 = new URLSearchParams(location.search);
// a phone (decision D-006): the panel is a sheet, so it does not open by itself on load
const PHONE = matchMedia('(max-width: 900px)');
const S = {sources: [], src: Number(q0.get('source') || 0), meta: null, cases: new Map(), run: null,
  sel: null, node: null, atSeq: null, tab: q0.get('tab') || (() => { try { return localStorage.getItem('sdc-ui.tab'); } catch { return null; } })() || 'explain', zoom: 1, fit: true, showReads: false,
  scopeCase: false, follow: true, live: 'connecting', lastChange: null, exists: true,
  inbox: {items: [], write: false, principal: null}, drawer: q0.get('drawer') === '1', form: {}, editing: null, busy: null, results: {}, proc: null, dirty: false, toast: null,
  liveData: null, offset: 0, diff: null,
  // the right panel: a rail of tab names until a tab is opened; remembered across loads
  panel: q0.get('tab') || q0.get('node') ? true : !PHONE.matches && localStorage.getItem('sdc-ui.panel') === 'open', feedCls: localStorage.getItem('sdc-ui.feed') || '', facts: null, kpis: null, feed: []};
const LS = (k, v) => { try { localStorage.setItem(k, v); } catch { /* private mode */ } };
function setPanel(open, tab) {
  if (tab) S.tab = tab;
  S.panel = open;
  LS('sdc-ui.panel', open ? 'open' : 'closed');
  LS('sdc-ui.tab', S.tab);
  document.documentElement.classList.toggle('rp-open', open);
}
const openPanel = (tab) => setPanel(true, tab);
const $ = (q) => document.querySelector(q);
const api = (p) => fetch(p).then((r) => (r.ok ? r.json() : Promise.reject(new Error(`${p}: ${r.status}`))));

const firstAt = (c) => (c.events.find((e) => e.at && e.at !== '-')?.at || '~');
const lastAt = (c) => { for (let i = c.events.length - 1; i >= 0; i--) if (c.events[i].at && c.events[i].at !== '-') return c.events[i].at; return ''; };
function ordered() {
  const all = Array.from(S.cases.values());
  const kids = (id) => all.filter((c) => c.parent === id).sort((a, b) => firstAt(a).localeCompare(firstAt(b)) || a.id.localeCompare(b.id));
  const out = [];
  const visit = (c) => { out.push(c); kids(c.id).forEach(visit); };
  all.filter((c) => !c.parent || !all.some((p) => p.id === c.parent)).forEach(visit);
  return out;
}
const current = () => S.cases.get(S.sel);

async function loadTree(only) {
  const q = only ? `&cases=${encodeURIComponent(only.join(','))}` : '';
  const t = await api(`/api/tree?source=${S.src}${q}`);
  S.exists = t.exists;
  if (!only) S.cases.clear();
  for (const c of t.cases) S.cases.set(c.rel, c);
  for (const k of Array.from(S.cases.keys())) if (!t.present.includes(k)) S.cases.delete(k);
}
async function loadRun() { S.frozenAt = null; S.run =await api(`/api/run?source=${S.src}`).catch(() => null); if (S.run?.proc) S.proc = S.run.proc; setRunSettings(S.run?.settings); }
async function loadLive() { const t = Date.now(); const d = await api(`/api/live?source=${S.src}`).catch(() => null); if (d) { S.liveData = d; S.offset = d.now - (t + Date.now()) / 2; } if (d && !d.fs?.present) await loadFacts(); }
// without fs/changes.jsonl the feed reads the planning artifacts' front matter (an older server has no /api/workfacts)
async function loadFacts() { S.facts = await api(`/api/workfacts?source=${S.src}`).catch(() => ({files: []})); }
// the page's clock: the server's, or the stop of a run stopped on purpose (every clock stands there)
const stopAt = () => { const t = Date.parse(S.run?.stopped?.at); return isNaN(t) ? null : t; };
// or the last event of a run no driver is attached to (S.frozenAt, set by each render)
const serverNow = () => stopAt() ?? S.frozenAt ?? Date.now() + S.offset;
async function loadInbox() { S.inbox = await api(`/api/inbox?source=${S.src}`).catch(() => S.inbox); }

function pickFollow() {
  if (!S.follow) return;
  const cs = ordered();
  if (!cs.length) return;
  const st = nowState(cs, S.liveData, S.run);
  // stay on a case that is still in flight; else go to the one that started last
  const hots = [...st.running.map((r) => r.c), ...st.gone.map((r) => r.c), ...st.waiting.filter((w) => w.kind === 'decider').map((w) => w.c)];
  if (hots.some((c) => c.rel === S.sel)) return;
  const hot = st.running[st.running.length - 1]?.c || hots[0];
  if (hot) { if (hot.rel !== S.sel) { S.sel = hot.rel; S.node = null; S.atSeq = null; } return; }
  let best = cs[0];
  for (const c of cs) {
    const s = caseState(c).state;
    if (['awaiting_authority', 'awaiting_person', 'running', 'stuck'].includes(s)) { best = c; break; }
    if (lastAt(c) > lastAt(best) || (lastAt(c) === lastAt(best) && s !== 'terminated' && caseState(best).state === 'terminated')) best = c;
  }
  if (best.rel !== S.sel) { S.sel = best.rel; S.node = null; S.atSeq = null; }
}

// ---- render, once per animation frame -------------------------------------------------------------
let raf = 0;
const schedule = () => { if (!raf) raf = requestAnimationFrame(() => { raf = 0; render(); }); };

function render() {
  const src = S.sources[S.src];
  patch($('#source'), S.sources.map((s) => `<option value="${s.index}"${s.index === S.src ? ' selected' : ''}>${esc(s.name)}${s.stopped ? ' · stopped' : s.exists ? '' : ' · waiting'}</option>`).join(''));
  // who this run is: an end-to-end test with a stand-in, autopilot; the graph legend follows
  const set = S.run?.settings;
  patch($('#runmark'), [standIn(set) ? '<span class="rm rm-standin" title="run-settings.jsonl: stand_in true">end-to-end test with a stand-in</span>' : '', autopilot(set) ? '<span class="rm rm-auto" title="run-settings.jsonl: autonomy A5, the decider or a rule holds every decision">autopilot</span>' : ''].join(''));
  const lg = $('#lg-person');
  const lgText = autopilot(set) ? 'decider (on the operator’s behalf)' : 'person';
  if (lg && lg.textContent !== lgText) lg.innerHTML = `<i class="lg c-person"></i>${lgText}`;
  $('#follow').checked = S.follow;
  $('#reads').checked = S.showReads;
  const live = $('#live');
  live.className = `live ${S.live}`;
  patch(live, `<i></i><span>${S.live === 'on' ? 'Live' : S.live === 'connecting' ? 'Connecting' : 'Reconnecting'}</span>${S.live === 'on' && S.lastChange ? `<span class="lt">updated ${S.lastChange.toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'})}</span>` : ''}`);
  renderDecisions();
  const cases = ordered();
  if (!cases.length) {
    patch($('#phases'), '');
    patch($('#now'), '');
    patch($('#nowbar'), '');
    patch($('#kpis'), '');
    patch($('#feed'), '');
    patch($('#tree'), `<div class="empty"><div class="spinner"></div><p>${S.exists ? 'No case yet in' : 'Waiting for'}</p><code>${esc(src?.path)}</code><p class="muted">The view fills in as the run writes its store.</p></div>`);
    $('#graph').innerHTML = '';
    patch($('#gtitle'), '');
    patch($('#panel'), '');
    patch($('#timeline'), '');
    return;
  }
  if (!S.sel || !S.cases.has(S.sel)) S.sel = cases[0].rel;
  const c = current();
  const ev = evalAt(c, S.atSeq);
  c._revisions = ev?.explanation?.computed_from?.case?.artifacts || c.case.artifacts || {};
  const ns = nowState(cases, S.liveData, S.run, Date.now() + S.offset);
  S.frozenAt = ns.idle?.t ?? null;
  const now = serverNow();
  ns.now = now;
  S.nowState = ns;
  const nowEl = $('#now');
  // A step ends and the next starts within a moment: hold the band over that gap instead of
  // flashing "between steps" for one poll.
  if (ns.mode === 'running') S.lastRunAt = now;
  if (ns.mode === 'between' && S.lastRunAt && now - S.lastRunAt < 1500) setTimeout(schedule, 1550 - (now - S.lastRunAt));
  else { nowEl.className = `m-${ns.mode}`; patch(nowEl, renderNow(ns, now, S.inbox?.items)); patch($('#nowbar'), renderNowBar(ns)); }
  patch($('#phases'), renderPhases(cases, activePhases(ns)));
  S.kpis = computeKpis(cases, S.run, S.inbox?.history, ns, now);
  // an end-to-end test with a stand-in for the person: its numbers are not eval numbers
  patch($('#kpis'), renderKpis(S.kpis, now) + (standIn(S.run?.settings) ? '<span class="k-standin" data-key="kstand" title="run-settings.jsonl: stand_in true. A stand-in took the person’s part; these values do not count as an evaluation.">not eval numbers</span>' : ''));
  S.feed = feedItems(cases, S.liveData, S.inbox?.history, S.facts);
  patch($('#feed'), renderFeed(S.feed, S.feedCls, now, !!S.liveData?.fs?.present));
  patch($('#tree'), renderTree(cases, ns, now));
  const last = lastSeq(c);
  const at = S.atSeq ?? last;
  const st = caseState(c);
  patch($('#gtitle'), `<div class="gt-main"><span class="gt-id">${esc(c.id)}</span><span class="pill st-${st.state}" data-v="${esc(st.label)}">${esc(st.label)}</span>${S.atSeq != null ? `<span class="pill past">past · event ${at} of ${last}</span>` : ''}</div><div class="gt-sub">${esc(c.case.protocol)} · protocol r${esc(c.ir?.protocol?.revision)} · case ${esc(ev?.explanation?.computed_from?.case?.revision || c.case.revision)} · ${Object.keys(c.evidence).length} evidence records</div>`);
  if (c.ir) {
    const host = $('#graph');
    const live = graphLive(ns, c);
    const probe = renderGraph(host, c, ev, {zoom: S.zoom, selected: S.node, showReads: S.showReads, atSeq: S.atSeq, revisions: c._revisions, live, now});
    // on a phone the graph keeps a readable scale (labels 12 px and more) and scrolls in its own box
    const z = S.fit ? (PHONE.matches ? 1.3 : Math.min(1.15, Math.max(0.75, (host.clientWidth - 24) / probe.width))) : S.zoom;
    if (z !== S.zoom) { S.zoom = z; renderGraph(host, c, ev, {zoom: z, selected: S.node, showReads: S.showReads, atSeq: S.atSeq, revisions: c._revisions, live, now}); }
    const hint = $('#ghint');
    const wide = PHONE.matches && host.scrollWidth > host.clientWidth + 1;
    if (hint.hidden === wide) hint.hidden = !wide;
  } else $('#graph').innerHTML = '<p class="muted pad">no ir.json in this case</p>';
  patch($('#timeline'), renderTimeline(c, at, last));
  document.documentElement.classList.toggle('rp-open', S.panel);
  if (PHONE.matches) { $('#panel').setAttribute('tabindex', '0'); $('#panel').setAttribute('role', 'region'); $('#panel').setAttribute('aria-label', 'Panel'); }
  document.querySelectorAll('#tabs button[data-tab]').forEach((b) => { b.classList.toggle('on', S.panel && b.dataset.tab === S.tab); b.setAttribute('aria-expanded', String(S.panel && b.dataset.tab === S.tab)); });
  if (!S.panel) return;
  const scope = S.scopeCase ? c.id : null;
  const p = $('#panel');
  let html = '';
  if (S.tab === 'explain') html = renderExplain(c, S.node, ev, S.atSeq);
  else if (S.tab === 'autonomy') html = scopeBox() + renderAutonomy(cases, S.run, S.meta, scope, S.inbox?.history);
  else if (S.tab === 'security') html = scopeBox() + renderSecurity(cases, S.run, S.meta, scope);
  else if (S.tab === 'metrics') html = kpiRows(S.kpis, now) + renderMetrics(cases, S.run, S.meta, c.id);
  else if (S.tab === 'events') html = renderEvents(c, at);
  else if (S.tab === 'changes') html = scopeBox() + renderChanges(S.liveData, cases, scope, now);
  else if (S.tab === 'run') html = renderRun();
  if (p.dataset.tab !== S.tab) { p.dataset.tab = S.tab; p.scrollTop = 0; }
  patch(p, `<div class="tabpane" data-key="pane:${S.tab}">${html}</div>`);
}

function renderTree(cases, st, now) {
  // a run stopped on purpose: said above the tree, on the case it stopped in, and nothing spins
  const stop = st.stopped;
  const run = new Map(stop ? [] : st.running.map((r) => [r.c, r]));
  const stopCase = stop ? cases.find((c) => c.id === stop.case) || cases.find((c) => !c.parent) : null;
  const head = stop ? `<div class="tree-stop" data-key="tstop" title="${esc(stop.reason || '')}"><i></i>Run stopped by ${esc(stop.by || 'not recorded')} at ${esc(new Date(stop.t).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'}))}</div>` : '';
  return head + cases.map((x) => {
    const st = caseState(x);
    const e = evalAt(x, null);
    const claims = Object.values(e?.claims || {});
    const t = claims.filter((v) => v.value === 'true').length;
    const name = x.parent ? x.id.slice(x.parent.length + 1) : x.id;
    const guides = Array.from({length: x.depth}, (_, i) => `<span class="guide" style="left:${16 + i * 16}px"></span>`).join('');
    return `<div data-key="c:${esc(x.rel)}" class="case${x.rel === S.sel ? ' sel' : ''}" data-case="${esc(x.rel)}" style="padding-left:${16 + x.depth * 16}px">${guides}
      <div class="cn"><span class="sdot st-${st.state}" data-v="${st.state}"></span><span class="cid">${esc(name)}</span>${run.has(x) ? `<span class="cst st-running k-${run.get(x).kind}" data-v="${esc(st.label)}" title="${esc(st.label)}"><i class="cst-spin"></i>${run.get(x).since != null ? `<span class="tk" data-since="${run.get(x).since}">${fmtDur(now - run.get(x).since)}</span>` : 'running'}</span>` : x === stopCase ? `<span class="cst st-stopped" data-v="stopped" title="${esc(st.label)} when the run was stopped">stopped</span>` : `<span class="cst st-${st.state}" data-v="${esc(st.label)}">${esc(st.label)}</span>`}</div>
      <div class="cp"><span class="proto">${esc((x.case.protocol || '').replace(/^sdc\.?/, '') || 'root')}</span><span class="bar"><i style="width:${claims.length ? (100 * t) / claims.length : 0}%"></i></span><span class="cnt">${t}/${claims.length}</span></div></div>`;
  }).join('');
}

const scopeBox = () => `<label class="switch scope"><input type="checkbox" id="scopeCase"${S.scopeCase ? ' checked' : ''}><span class="sw"></span>only the selected case</label>`;

function renderTimeline(c, at, last) {
  const evs = c.events;
  const idx = Math.max(0, evs.findIndex((e) => e.seq === at));
  const cur = evs[idx] || evs[evs.length - 1];
  const pos = evs.length > 1 ? (idx / (evs.length - 1)) * 100 : 100;
  const ee = evalEventAt(c, at);
  const first = evs.find((e) => e.at && e.at !== '-')?.at || '';
  return `<div class="tl-head" data-key="tlh"><span class="tl-title">Timeline</span><span class="tl-cur"><span class="ech ${EVENT_CLASS[cur?.event] || ''}"></span><b>#${cur?.seq}</b><span class="muted">${esc((cur?.at || '').replace('T', ' ').replace('Z', ''))}</span><span>${esc(cur ? describeEvent(cur) : '')}</span></span>
    <span class="tl-eval">state from <code>${esc(ee?.file || 'no evaluation yet')}</code></span>
    <span class="tl-btns"><button id="prev" class="ib" title="previous event (←)">‹</button><button id="next" class="ib" title="next event (→)">›</button><button id="livebtn" class="ib wide${S.atSeq == null ? ' on' : ''}" title="follow the latest event">Latest</button></span></div>
  <div class="tl-track" data-key="tlt">
    <div class="tl-ticks">${evs.map((e) => `<span data-key="t${e.seq}" class="tick ${EVENT_CLASS[e.event] || ''}${e.seq > at ? ' future' : ''}" title="#${e.seq} ${esc(e.at)} ${esc(describeEvent(e))}"></span>`).join('')}</div>
    <div class="playhead" style="left:${pos}%"></div>
    <input type="range" id="scrub" min="0" max="${evs.length - 1}" value="${idx}" aria-label="event">
  </div>
  <div class="tl-foot" data-key="tlf"><span>${esc(first.replace('T', ' ').replace('Z', ''))}</span><span class="tl-legend"><i class="ech ev-act"></i>action <i class="ech ev-rev"></i>revision <i class="ech ev-eval"></i>evaluation <i class="ech ev-child"></i>child case <i class="ech ev-auth"></i>authority <i class="ech ev-wait"></i>waiting <i class="ech ev-end"></i>end <i class="ech ev-bad"></i>discarded or stuck</span><span>${evs.length} events</span></div>`;
}

function renderEvents(c, at) {
  return `<div class="sect">Event log · ${esc(c.id)} <span class="muted">hash-chained events.jsonl</span></div><table class="steps evs"><tbody>${c.events.map((e) => `<tr data-key="ev${e.seq}" class="${e.seq === at ? 'cur' : ''}${e.seq > at ? ' future' : ''}" data-seq="${e.seq}"><td class="n mono">${e.seq}</td><td class="mono muted">${esc((e.at || '').slice(11, 19))}</td><td><i class="ech ${EVENT_CLASS[e.event] || ''}"></i> ${esc(describeEvent(e))}</td></tr>`).join('')}</tbody></table>`;
}

function renderDecisions() {
  const n = S.inbox.items.length;
  const tag = S.inbox.write ? `Canon decides · the driver records · you decide as ${S.inbox.principal}` : 'Canon decides · the driver records · this page only reads';
  if ($('#tagline').textContent !== tag) $('#tagline').textContent = tag;
  const btn = $('#decbtn');
  btn.classList.toggle('has', n > 0 && !S.inbox.stopped);
  btn.classList.toggle('stopped', !!S.inbox.stopped);
  btn.classList.toggle('on', S.drawer);
  patch($('#deccount'), String(n));
  document.body.classList.toggle('drawer-open', S.drawer);
  if (S.drawer) {
    patch($('#drawer'), renderDrawer(S));
    // a phone scrolls the list with the keyboard too (axe scrollable-region-focusable)
    if (PHONE.matches) $('#drawer .dr-list')?.setAttribute('tabindex', '0');
    const pl = $('#proclog');
    if (pl) pl.scrollTop = pl.scrollHeight;
  }
  // focus follows the work: opening the drawer moves focus to its first decision control, closing it
  // back to the control that opened it
  if (S.drawer && !S.drawerWas) {
    S.drawerFrom = document.activeElement;
    const first = $('#drawer .dcard .btn.primary:not([disabled])') || $('#drclose');
    first?.focus({preventScroll: false});
  } else if (!S.drawer && S.drawerWas && S.drawerFrom?.isConnected) S.drawerFrom.focus({preventScroll: true});
  S.drawerWas = S.drawer;
  const t = $('#toast');
  t.className = S.toast ? `show ${S.toast.ok ? 'ok' : 'bad'}` : '';
  if (S.toast) patch(t, esc(S.toast.text));
}

async function doAct(kind, el) {
  const item = S.inbox.items.find((i) => i.id === el.dataset.item);
  let body;
  if (kind === 'approve') body = {kind, rel: item.rel, action: item.action, revision: item.revision, sha: item.artifact?.sha};
  else if (kind === 'answer') {
    const key = `q:${item.rel}:${item.qid}`;
    const pick = S.form[`${key}:pick`] ?? item.default;
    body = {kind, rel: item.rel, qid: item.qid, sha: item.sha, revision: item.revision, answer: pick === '__other' ? (S.form[`${key}:other`] || '').trim() : pick};
  } else if (kind === 'context') {
    const key = `ctx:${item.rel}:${el.dataset.field}`;
    const cur = item.artifact?.json?.[el.dataset.field]?.value;
    body = {kind, rel: item.rel, field: el.dataset.field, value: (S.form[`${key}:value`] ?? cur ?? '').trim(), note: (S.form[`${key}:note`] || '').trim(), sha: item.artifact?.sha, revision: item.revision};
  } else body = {kind: 'continue'};
  const id = item?.id || 'continue';
  S.busy = id;
  schedule();
  let r;
  try {
    r = await fetch(`/api/act?source=${S.src}`, {method: 'POST', headers: {'content-type': 'application/json', 'x-sdc-ui': '1'}, body: JSON.stringify(body)}).then((x) => x.json());
  } catch (e) { r = {ok: false, error: String(e)}; }
  S.busy = null;
  const msg = {approve: `Approved ${item?.subject || ''} ${item?.revision || ''}`, answer: `Answered ${item?.qid}${r.approved ? ` as ${r.answered_by} · ${item.approval?.capability || 'scope.answer'} approved` : ''}`, context: `Changed ${body.field}`, continue: `Started run-trial.sh (pid ${r.pid})`}[kind];
  S.results[id] = r.ok ? {ok: true, msg, output: r.output} : {ok: false, error: r.error || 'refused', output: r.output};
  S.toast = r.ok ? {ok: true, text: msg + (kind !== 'continue' ? ' · Continue run when you are done deciding' : '')} : {ok: false, text: r.error || 'refused'};
  clearTimeout(S.toastT);
  S.toastT = setTimeout(() => { S.toast = null; schedule(); }, 6000);
  if (r.ok) { if (kind !== 'continue') S.dirty = true; else S.dirty = false; S.editing = null; }
  await loadInbox();
  if (kind === 'continue') await loadRun();
  schedule();
}

function renderRun() {
  const r = S.run;
  const live = S.proc?.tail?.length ? `<div class="sect">Continue run · ${esc(S.proc.name)} <span class="muted">${S.proc.running ? 'running' : `exited ${esc(S.proc.exit)}`}</span></div><pre class="proc">${esc(S.proc.tail.slice(-200).join('\n'))}</pre>` : '';
  if (!r || !r.hasLog) return live + `<p class="muted">No run.log next to the store (${esc(r?.runDir)}).</p>`;
  return live + `<div class="sect">run.log <span class="muted">${esc(r.runDir)}</span></div>${r.console ? `<pre class="pred">${esc(r.console)}</pre>` : ''}
    <div class="cmds">${r.commands.map((c, i) => `<details data-key="cmd${i}" class="cmdrow"><summary><span class="n">${i + 1}</span><code>${esc(c.cmd.replace(/--case-dir \S+/, (m) => (m.length > 60 ? '--case-dir …' + m.slice(-40) : m)))}</code>${c.verdict ? `<span class="pill">${esc(c.verdict)}</span>` : ''}<span class="muted">exit ${esc(c.exit)}</span></summary><pre class="pred">${esc(c.out)}</pre></details>`).join('')}</div>`;
}

// ---- the diff of one file change, in a dialog ------------------------------------------------------
async function openDiff(seq) {
  const r = await api(`/api/fschange?source=${S.src}&seq=${seq}`).catch(() => null);
  if (!r) return;
  const list = S.liveData?.fs?.changes || [];
  const pos = list.findIndex((x) => x.seq === seq);
  S.diff = {seq, pos};
  const d = $('#diffdlg');
  d.querySelector('.dwrap').innerHTML = renderDiff(r, pos, list.length);
  d.querySelector('.db')?.scrollTo(0, 0);
  if (!d.open) d.showModal();
}
// the grounds of one decision (reasons, cites, model, cost), in the same dialog
function openGrounds(i) {
  const d = S.inbox?.history?.[i];
  if (!d) return;
  S.diff = null;
  const dlg = $('#diffdlg');
  dlg.querySelector('.dwrap').innerHTML = `<div class="dh"><div class="dh-1"><span class="cb c-decision">Decision</span><span class="dh-p"><b>${esc(d.what || '')}</b></span><span class="muted">${esc(d.case || '')}</span><span class="dh-nav"><button class="ib wide" data-fsclose="1">Close</button></span></div></div><div class="db grounds">${decisionRows([d], 'gr')}${(d.reasons || []).length ? '' : '<p class="d-note">No reasons recorded with this decision.</p>'}</div>`;
  if (!dlg.open) dlg.showModal();
}
function stepDiff(dir) {
  if (!S.diff) return;
  const list = S.liveData?.fs?.changes || [];
  const next = list[(S.diff?.pos ?? 0) + dir];
  if (next) openDiff(next.seq);
}

// ---- clocks: elapsed times tick here, in place, without a render ------------------------------------
function tickClocks() {
  const now = serverNow();
  for (const el of document.querySelectorAll('[data-since]')) { const v = fmtDur(now - Number(el.dataset.since)); if (el.textContent !== v) el.textContent = v; }
  for (const el of document.querySelectorAll('[data-ago]')) { const v = fmtAgo(now - Number(el.dataset.ago)); if (el.textContent !== v) el.textContent = v; }
  setTimeout(tickClocks, 1000 - (serverNow() % 1000) + 5);
}
tickClocks();

// ---- interaction --------------------------------------------------------------------------------
function selectCase(rel) {
  if (rel === S.sel) return;
  S.sel = rel; S.node = null; S.atSeq = null; S.follow = false; S.fit = true;
  render();
}
function setIdx(i) {
  const c = current();
  if (!c) return;
  const evs = c.events;
  const k = Math.max(0, Math.min(evs.length - 1, i));
  S.atSeq = k >= evs.length - 1 ? null : evs[k].seq;
  schedule();
}
const idxOf = (c) => (S.atSeq == null ? c.events.length - 1 : Math.max(0, c.events.findIndex((e) => e.seq === S.atSeq)));

document.addEventListener('click', async (ev) => {
  const t = ev.target;
  const id = t.closest('button')?.id;
  const c = current();
  if (id && c) {
    if (id === 'prev') return setIdx(idxOf(c) - 1);
    if (id === 'next') return setIdx(idxOf(c) + 1);
    if (id === 'livebtn') return setIdx(Infinity);
    if (id === 'zin') { S.fit = false; S.zoom = Math.min(2, S.zoom * 1.2); return render(); }
    if (id === 'zout') { S.fit = false; S.zoom = Math.max(0.3, S.zoom / 1.2); return render(); }
    if (id === 'zfit') { S.fit = true; return render(); }
  }
  if (t.closest('#decbtn') || t.closest('[data-open-drawer]')) { S.drawer = !S.drawer || !!t.closest('[data-open-drawer]'); return render(); }
  if (t.closest('#rpclose')) { setPanel(false); return render(); }
  if (t.closest('#railbtn')) { setPanel(true); return render(); }
  const chip = t.closest('[data-feed]');
  if (chip) { S.feedCls = S.feedCls === chip.dataset.feed ? '' : chip.dataset.feed; LS('sdc-ui.feed', S.feedCls); return render(); }
  const kpi = t.closest('[data-kpi]');
  if (kpi) {
    openPanel('metrics');
    render();
    const row = document.getElementById(`kpi-${kpi.dataset.kpi}`);
    if (row) { row.scrollIntoView({block: 'center'}); row.classList.remove('flash'); void row.offsetWidth; row.classList.add('flash'); }
    return;
  }
  const gr = t.closest('[data-grounds]');
  if (gr && !t.closest('[data-fs]')) { ev.preventDefault(); return openGrounds(Number(gr.dataset.grounds)); }
  const fsEl = t.closest('[data-fs]');
  if (fsEl) { ev.preventDefault(); return openDiff(Number(fsEl.dataset.fs)); }
  const nav = t.closest('[data-fsnav]');
  if (nav && !nav.disabled) return stepDiff(Number(nav.dataset.fsnav));
  if (t.closest('[data-fsclose]') || t.id === 'diffdlg') return $('#diffdlg').close();
  const go = t.closest('[data-goto]');
  // the Now band's cards and the stops a failed KPI lists (strip and Metrics) go to their case and event
  if (go && (t.closest('#now') || t.closest('.k-stop, .kf-stop'))) {
    if (t.closest('a[data-tab]')) { ev.preventDefault(); openPanel('changes'); S.scopeCase = true; selectCase(go.dataset.goto); return render(); }
    S.follow = false; S.sel = go.dataset.goto; S.atSeq = go.dataset.seq ? Number(go.dataset.seq) : null; S.fit = true;
    if (go.dataset.node) { S.node = go.dataset.node; openPanel('explain'); }
    return render();
  }
  if (t.closest('#drclose') || t.id === 'scrim') { S.drawer = false; return render(); }
  const actEl = t.closest('[data-act]');
  if (actEl && !actEl.disabled) { ev.preventDefault(); return doAct(actEl.dataset.act, actEl); }
  const editEl = t.closest('[data-edit]');
  if (editEl && !editEl.disabled) { S.editing = editEl.dataset.edit; return render(); }
  if (t.closest('[data-cancel]')) { S.editing = null; return render(); }
  const node = t.closest('[data-id]');
  if (node && t.closest('#graph')) { S.node = S.node === node.dataset.id ? null : node.dataset.id; if (S.node) openPanel('explain'); return render(); }
  const a = t.closest('a,[data-case],[data-seq],[data-phase],[data-tab],[data-node]');
  if (!a) return;
  if (a.dataset.blob) {
    ev.preventDefault();
    const text = await fetch(`/api/blob?source=${S.src}&case=${encodeURIComponent(a.dataset.case)}&hash=${a.dataset.blob}`).then((r) => r.text());
    $('#modal pre').textContent = text || '(empty output)';
    $('#modal h3').textContent = `Tool output · blob ${a.dataset.blob.slice(0, 12)}…`;
    $('#modal').showModal();
    return;
  }
  if (a.dataset.tab) {
    ev.preventDefault();
    // a rail tab toggles the panel; any other link to a tab opens it there
    if (a.closest('#tabs') && S.panel && S.tab === a.dataset.tab) setPanel(false); else openPanel(a.dataset.tab);
    return render();
  }
  if (a.dataset.node) { ev.preventDefault(); S.node = a.dataset.node; return render(); }
  if (a.dataset.phase) {
    const ph = phaseStates(ordered()).find((p) => p.id === a.dataset.phase);
    const open = (x) => ph.claims.find((cl) => x.ir?.claims?.[cl] && evalAt(x, null)?.claims?.[cl]?.value !== 'true');
    const cc = ph?.cases?.find(open) || ph?.cases?.[0];
    if (cc) { S.follow = false; S.sel = cc.rel; S.atSeq = null; S.node = `claim:${open(cc) || ph.claims[0]}`; openPanel('explain'); render(); }
    return;
  }
  if (a.dataset.case && a.closest('#tree,#panel')) {
    ev.preventDefault();
    selectCase(a.dataset.case);
    if (a.dataset.seq) { S.atSeq = Number(a.dataset.seq); render(); }
    return;
  }
  if (a.dataset.seq) { ev.preventDefault(); S.atSeq = Number(a.dataset.seq) >= lastSeq(c) ? null : Number(a.dataset.seq); render(); }
});
document.addEventListener('input', (ev) => {
  if (ev.target.id === 'scrub') return setIdx(Number(ev.target.value));
  const f = ev.target.dataset?.form;
  if (f) {
    S.form[f] = ev.target.value;
    if (/:other$/.test(f)) S.form[f.replace(/:other$/, ':pick')] = '__other';
    if (ev.target.type === 'radio' || /:other$/.test(f)) schedule();
  }
});
document.addEventListener('change', async (ev) => {
  const id = ev.target.id;
  if (id === 'source') { S.src = Number(ev.target.value); S.sel = null; S.node = null; S.atSeq = null; S.follow = true; await loadTree(); await loadRun(); await loadLive(); pickFollow(); render(); }
  if (id === 'scopeCase') { S.scopeCase = ev.target.checked; render(); }
  if (id === 'follow') { S.follow = ev.target.checked; if (S.follow) pickFollow(); render(); }
  if (id === 'reads') { S.showReads = ev.target.checked; render(); }
});
document.addEventListener('keydown', (ev) => {
  if ($('#diffdlg').open) { if (ev.key === 'ArrowLeft' || ev.key === 'ArrowRight') { ev.preventDefault(); stepDiff(ev.key === 'ArrowLeft' ? -1 : 1); } return; }
  if (ev.target.tagName === 'INPUT' && ev.target.type !== 'range') return;
  const c = current();
  if (!c) return;
  if (ev.key === 'ArrowLeft') { setIdx(idxOf(c) - 1); ev.preventDefault(); }
  if (ev.key === 'ArrowRight') { setIdx(idxOf(c) + 1); ev.preventDefault(); }
  if (ev.key === 'Escape') { S.node = null; if (PHONE.matches && S.panel) setPanel(false); render(); }
});
// drag the graph to pan; a click without movement still selects a node
let pan = null;
document.addEventListener('mousedown', (ev) => {
  const g = ev.target.closest('#graph');
  if (!g || ev.button !== 0) return;
  pan = {g, x: ev.clientX, y: ev.clientY, sl: g.scrollLeft, st: g.scrollTop, moved: false};
});
document.addEventListener('mousemove', (ev) => {
  if (!pan) return;
  const dx = ev.clientX - pan.x, dy = ev.clientY - pan.y;
  if (!pan.moved && Math.abs(dx) + Math.abs(dy) < 4) return;
  pan.moved = true;
  pan.g.classList.add('panning');
  pan.g.scrollLeft = pan.sl - dx;
  pan.g.scrollTop = pan.st - dy;
});
document.addEventListener('mouseup', () => {
  if (pan?.moved) { pan.g.classList.remove('panning'); const stop = (e) => { e.stopPropagation(); e.preventDefault(); }; document.addEventListener('click', stop, {capture: true, once: true}); }
  pan = null;
});
let resizeT = 0;
window.addEventListener('resize', () => { clearTimeout(resizeT); resizeT = setTimeout(() => S.fit && render(), 120); });

// ---- live: coalesce change bursts into one fetch and one frame --------------------------------------
const pending = {cases: new Set(), run: false, full: false, live: false};
let inflight = false;
async function drain() {
  if (inflight) return;
  inflight = true;
  try {
    while (pending.full || pending.cases.size || pending.run || pending.live) {
      const full = pending.full || !S.cases.size;
      const cases = Array.from(pending.cases);
      const run = pending.run, live = pending.live || full;
      pending.full = false; pending.cases.clear(); pending.run = false; pending.live = false;
      if (full || cases.length) await loadTree(full ? null : cases);
      if (run) await loadRun();
      if (live) await loadLive();
      if (full || cases.length || run) await loadInbox();
    }
    S.lastChange = new Date();
    pickFollow();
    schedule();
  } catch { /* the next change retries */ } finally { inflight = false; }
}
function connect() {
  const es = new EventSource('/events');
  es.addEventListener('hello', () => { if (S.live !== 'on') { S.live = 'on'; pending.full = true; drain(); } });
  es.addEventListener('change', (m) => {
    const d = JSON.parse(m.data);
    if (S.sources[d.source]) S.sources[d.source].exists = true;
    if (d.source !== S.src) return;
    d.cases.forEach((k) => pending.cases.add(k));
    if (d.run) pending.run = true;
    if (d.live) pending.live = true;
    drain();
  });
  es.onerror = () => { S.live = 'reconnecting'; schedule(); };
  es.addEventListener('proc', (m) => {
    const d = JSON.parse(m.data);
    if (d.source !== S.src) return;
    S.proc = S.proc && S.proc.name === d.name && S.proc.running ? S.proc : {name: d.name, running: true, exit: null, tail: []};
    S.proc.tail = [...(S.proc.tail || []), ...d.lines].slice(-400);
    schedule();
  });
  es.addEventListener('procend', (m) => {
    const d = JSON.parse(m.data);
    if (d.source !== S.src || !S.proc) return;
    S.proc.running = false; S.proc.exit = d.exit;
    pending.full = true; drain();
  });
  // a run that appeared under --runs, or a run stopped or resumed: the store selector follows
  es.addEventListener('sources', async () => { S.sources = await api('/api/sources').catch(() => S.sources); schedule(); });
  es.addEventListener('act', (m) => { if (JSON.parse(m.data).source === S.src) { pending.run = true; drain(); } });
}

(async () => {
  S.sources = await api('/api/sources');
  S.meta = await api('/api/meta');
  await loadTree();
  await loadRun();
  await loadLive();
  if (q0.get('case')) { S.sel = q0.get('case'); S.follow = false; } else pickFollow();
  if (q0.get('node')) S.node = q0.get('node');
  if (q0.get('at')) S.atSeq = Number(q0.get('at'));
  if (q0.get('reads')) S.showReads = true;
  await loadInbox();
  render();
  connect();
})();
