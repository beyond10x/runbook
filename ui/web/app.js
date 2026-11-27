import {renderGraph} from './graph.js';
import {renderExplain} from './explain.js';
import {renderPhases, renderAutonomy, renderSecurity, renderMetrics, phaseStates, esc} from './lenses.js';
import {setView} from './view.js';
import {caseState, evalAt, evalEventAt, lastSeq, describeEvent, EVENT_CLASS, protocolRevisionOf, claimsRecorded, CLAIMS_NOT_RECORDED} from './model.js';
import {patch} from './morph.js';
import {renderDrawer} from './inbox.js';
import {nowState, nowSay, renderNow, renderNowBar, activePhases, graphLive, renderChanges, renderDiff, fmtDur, fmtAgo, STUB_NOTE} from './now.js';
import {computeKpis, renderKpis, kpiRows} from './kpis.js';
import {feedItems, renderFeed} from './feed.js';
import {decisionRows} from './lenses.js';
import {setRunSettings, autopilot, standIn} from './principal.js';
import {api, openEvents, daemonView, daemon, missingRun, runEvents, tabsOf} from './source.js';
import {runSteps, withViewCalls, renderStepsHead, renderStepsTable, renderStepsBar, runStateWord, fmtTook, eventsPane} from './runsteps.js';
import {clockTime} from './clock.js';
import {isStub, renderPickerButton, pickerTitle, pickerGroups, visibleItems, renderPickerList} from './picker.js';
import {renderProducts, renderProductBar, bindProducts, showProduct} from './products.js';
// the act code (the token field, the POST of an answer) is a file only the daemon serves (ui/daemon/): this server
// has no such file, and a page the daemon served loads it only because the daemon named itself
const renderDaemonDrawer = daemon ? (await import('./decide.js')).renderDaemonDrawer : null;
// the same for the start form (ui/daemon/start.js): it names the mode "operate" and hands the run it started to openRun
const initStart = daemon ? (await import('./start.js')).initStart : null;
// and for the Routes view (ui/daemon/routes.js): the daemon's model routes, and the kind and status of a run's route
const {initRoutes, routeWords} = daemon ? await import('./routes.js') : {};
// and for the Metrics tab (ui/daemon/metrics.js): the row of the view run-metrics of the run on screen, drawn as a pane
const {readMetrics, metricsPane} = daemon ? await import('./metrics.js') : {};

const q0 = new URLSearchParams(location.search);
// a phone (decision D-006): the panel is a sheet, so it does not open by itself on load
// the phone layout's query, defined once in index.html (html.phone)
const PHONE = matchMedia(window.SDC_PHONE);
const S = {sources: [], src: Number(q0.get('source') || 0), meta: null, cases: new Map(), run: null,
  sel: null, node: null, atSeq: null, tab: q0.get('tab') || (() => { try { return localStorage.getItem('sdc-ui.tab'); } catch { return null; } })() || 'explain', zoom: 1, fit: true, showReads: false,
  scopeCase: false, follow: true, live: 'connecting', lastChange: null, exists: true,
  inbox: {items: []}, drawer: q0.get('drawer') === '1',
  liveData: null, offset: 0, diff: null, dall: false,
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
// The tabs this page has data for (web/source.js tabsOf): any other tab is no button, and a remembered one opens the first.
// The daemon's Metrics tab shows once the view answered a row for the run on screen (renderDaemon), so a remembered
// Metrics tab is kept: until then the first tab stands for it.
const TABS = tabsOf();
for (const b of document.querySelectorAll('#tabs button[data-tab]')) b.hidden = !TABS.includes(b.dataset.tab);
if (!tabsOf({metrics: true}).includes(S.tab)) S.tab = TABS[0];
// The daemon's page: its own layout rules (html.daemon in style.css), and the run's files line with its steps.
if (daemon) {
  document.documentElement.classList.add('daemon');
  $('#dhead').after($('#product'));
  $('#dsteps').hidden = false;
}

const firstAt = (c) => (c.events.find((e) => e.at && e.at !== '-')?.at || '~');
const lastAt = (c) => { for (let i = c.events.length - 1; i >= 0; i--) if (c.events[i].at && c.events[i].at !== '-') return c.events[i].at; return ''; };
function ordered() {
  const all = Array.from(S.cases.values());
  const kids = (id) => all.filter((c) => c.parent === id).sort((a, b) => firstAt(a).localeCompare(firstAt(b)) || a.id.localeCompare(b.id));
  const out = [];
  const seen = new Set();
  const visit = (c) => { if (seen.has(c)) return; seen.add(c); out.push(c); kids(c.id).forEach(visit); };
  all.filter((c) => !c.parent || !all.some((p) => p.id === c.parent)).forEach(visit);
  // cases no root reaches (parents that name each other) are drawn too, each from the first of them: none is hidden
  all.forEach(visit);
  return out;
}
const current = () => S.cases.get(S.sel);

// the runbook's view of this run (the server picks the one that covers the protocols of its cases) and, when none does, which view is missing
async function loadView() {
  const [v, info] = await Promise.all([api(`/api/view?source=${S.src}`).catch(() => null), api(`/api/view-info?source=${S.src}`).catch(() => null)]);
  S.viewInfo = info;
  const problems = setView(v);
  if (problems.length) S.viewInfo = {...info, missing: false, problems};
}
const viewNote = () => (S.viewInfo?.missing ? `<p class="view-note-t" data-ux="view-note"><b>No view for ${esc(S.viewInfo.protocols.join(', '))}.</b> This run shows the generic parts only: the protocols found in the store, every file by its path, no phases, no file kinds, no decisions of its runbook. Serve its view with <code>--view FILE</code> (format runbook.view/1, for example <code>views/&lt;runbook&gt;.view.json</code>).</p>` : S.viewInfo?.problems?.length ? `<p class="view-note-t" data-ux="view-note"><b>The view was refused.</b> ${esc(S.viewInfo.problems.join('; '))}</p>` : '');

async function loadTree(only) {
  if (!only || !S.viewInfo?.protocols?.length || only.some((k) => !S.cases.has(k))) await loadView();
  const q = only ? `&cases=${encodeURIComponent(only.join(','))}` : '';
  // the daemon's tree may not be readable for a moment (a 503 while the run writes): the tree shown stays
  const t = daemon ? await api(`/api/tree?source=${S.src}${q}`).catch(() => null) : await api(`/api/tree?source=${S.src}${q}`);
  if (!t) return;
  S.exists = t.exists;
  if (!only) S.cases.clear();
  for (const c of t.cases) S.cases.set(c.rel, c);
  for (const k of Array.from(S.cases.keys())) if (!t.present.includes(k)) S.cases.delete(k);
}
// the files of the run and the rule that named its product (what a step produced: web/products.js); the daemon's
// adapter answers {served: false}
async function loadProducts(force) {
  const now = Date.now();
  if (S.productsSrc !== S.src) { S.products = null; S.productsAt = 0; }
  if (!force && S.productsSrc === S.src && now - S.productsAt < (S.panel && S.tab === 'products' ? 1500 : 10000)) return;
  S.productsSrc = S.src; S.productsAt = now;
  S.products = await api(`/api/products?source=${S.src}`).catch(() => null);
}
// the Products tab is built outside patch(): what a run holds never becomes markup (web/products.js)
function mountProducts(c) {
  const p = $('#panel');
  if (p.dataset.tab !== 'products') { p.dataset.tab = 'products'; p.scrollTop = 0; p.replaceChildren(); }
  p.__html = null;
  renderProducts(p, {c, products: S.products, src: S.src, daemon, run: S.sources[S.src]?.path});
}
bindProducts({refresh: () => render(), open: (view) => { showProduct(view); openPanel('products'); render(); }});
// The row of the daemon's view run-metrics for the run on screen ({run, row}; row null when the view refused or answered
// none), read like the files: again after 1.5 s while the tab is open, else after 10 s.
async function loadMetrics(force) {
  if (!readMetrics) return;
  const run = S.missingRun == null ? S.sources[S.src]?.daemonRun?.run_id ?? null : null;
  const now = Date.now();
  if (S.metrics?.run !== run) S.metrics = {run, row: null, at: 0};
  if (!run || (!force && now - S.metrics.at < (S.panel && S.tab === 'metrics' ? 1500 : 10000))) return;
  S.metrics.at = now;
  const {row} = await readMetrics(run);
  if (S.metrics.run === run) S.metrics = {run, row: row ?? null, at: now};
}
// the Metrics tab is built outside patch(), like the files: every value of the row is a text node (ui/daemon/metrics.js);
// drawn again only when the row changed
function mountMetrics(row) {
  const p = $('#panel');
  if (p.dataset.tab !== 'metrics') { p.dataset.tab = 'metrics'; p.scrollTop = 0; p.__metrics = null; }
  p.__html = null;
  const key = JSON.stringify(row);
  if (p.__metrics === key) return;
  p.__metrics = key;
  const pane = document.createElement('div');
  pane.className = 'tabpane';
  pane.dataset.key = 'pane:metrics';
  pane.append(metricsPane(row));
  p.replaceChildren(pane);
}
async function loadRun() { S.frozenAt = null; S.run =await api(`/api/run?source=${S.src}`).catch(() => null); setRunSettings(S.run?.settings); }
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
  // the daemon's tree has a state per case and no feed of steps: follow the case that runs or waits, else the root
  if (daemon) {
    const hot = cs.find((c) => c.daemonState === 'Running') || cs.find((c) => ['AwaitingPerson', 'AwaitingAuthority', 'Stuck'].includes(c.daemonState)) || cs[0];
    if (hot.rel !== S.sel) { S.sel = hot.rel; S.node = null; S.atSeq = null; }
    return;
  }
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
// stub, fixture and dry runs (server.mjs programOf): listed apart, labelled, never the default
const byNewest = (a, b) => (b.lastWrite || 0) - (a.lastWrite || 0);
const defaultSource = (list) => list.filter((s) => !isStub(s)).sort(byNewest)[0]?.index ?? 0;
const schedule = () => { if (!raf) raf = requestAnimationFrame(() => { raf = 0; render(); }); };

// ---- the run picker (decision D-009): a button, a search field and the runs as a listbox -------------
// The search field keeps the focus while the arrow keys move through the rows (aria-activedescendant);
// Enter opens the row's run, Escape closes, a click outside or a focus that leaves the panel closes.
const PK = {open: false, q: '', active: null, stubs: false, exp: {}};
const pkGroups = () => pickerGroups(S.sources, PK.q, PK.stubs, {cur: S.src, expOpen: PK.exp});
const pkActive = () => (PK.active ? document.getElementById(PK.active) : null);
const runNow = () => Date.now() + S.offset;
function renderPicker(src = S.sources[S.src]) {
  const btn = $('#runpick');
  patch(btn, renderPickerButton(src));
  const title = pickerTitle(src);
  if (btn.title !== title) btn.title = title;
  btn.setAttribute('aria-expanded', PK.open ? 'true' : 'false');
  $('#pk-panel').hidden = !PK.open;
  document.documentElement.classList.toggle('pk-open', PK.open);
  if (!PK.open) return;
  const g = pkGroups();
  const items = visibleItems(g);
  if (!items.some((x) => x.id === PK.active)) PK.active = items.find((x) => x.index === S.src)?.id ?? items[0]?.id ?? null;
  const out = renderPickerList(g, {cur: S.src, active: PK.active, now: runNow()});
  patch($('#pk-list'), out.list);
  patch($('#pk-none'), out.none);
  const t = $('#pk-stubs');
  t.hidden = !out.toggle;
  if (t.textContent !== out.toggle) t.textContent = out.toggle;
  t.setAttribute('aria-expanded', g.stubsOpen ? 'true' : 'false');
  $('#pk-q').setAttribute('aria-activedescendant', PK.active || '');
}
async function openPicker() {
  // the stub group opens with the panel when the run on screen is one of them
  PK.open = true; PK.q = ''; PK.active = null; PK.stubs = isStub(S.sources[S.src]);
  $('#pk-q').value = '';
  renderPicker();
  $('#pk-q').focus();
  // the rows' cost, calls and times as they are now
  S.sources = await api('/api/sources').catch(() => S.sources);
  if (PK.open) { renderPicker(); pkActive()?.scrollIntoView({block: 'nearest'}); }
}
function closePicker(focusButton = true) {
  if (!PK.open) return;
  PK.open = false;
  renderPicker();
  if (focusButton) $('#runpick').focus();
}
function moveActive(d) {
  const items = visibleItems(pkGroups());
  if (!items.length) return;
  const i = items.findIndex((x) => x.id === PK.active);
  const k = d === 'first' ? 0 : d === 'last' ? items.length - 1 : Math.max(0, Math.min(items.length - 1, (i < 0 ? -1 : i) + d));
  PK.active = items[k].id;
  renderPicker();
  pkActive()?.scrollIntoView({block: 'nearest'});
}
// an experiment's head opens or closes its group; the keyboard stays on the head
function toggleExp(id) {
  const x = pkGroups().groups.find((y) => y.exp?.id === id);
  if (!x) return;
  PK.exp[id] = !x.open;
  PK.active = x.hid;
  renderPicker();
  pkActive()?.scrollIntoView({block: 'nearest'});
}
// open a run: the page reloads its data; the URL names the run (source index and name), so a link opens it
async function selectSource(i) {
  closePicker();
  if (i === S.src || !S.sources[i]) return;
  S.src = i; S.sel = null; S.node = null; S.atSeq = null; S.follow = true; S.missingRun = null;
  const q = new URLSearchParams(location.search);
  for (const k of ['case', 'node', 'at']) q.delete(k);
  q.set('source', String(i));
  // the daemon's address names the run by its id (the index is the page's own order); the inspector's by its name
  q.set('run', daemon ? S.sources[i].path : S.sources[i].name);
  history.replaceState(null, '', `${location.pathname}?${q}`);
  if (stream && !daemon) connect(); // the stream names the run on screen
  showProduct({kind: 'list'});
  await loadTree(); await loadRun(); await loadLive(); loadProducts(true).then(schedule); loadMetrics(true).then(schedule); if (daemon) await loadInbox(); pickFollow(); render();
}
// a run the page started: read the run list again and show that run
async function openRun(id) {
  S.sources = await api('/api/sources');
  const s = S.sources.find((x) => x.path === id);
  if (s) await selectSource(s.index);
}
if (initStart) initStart(openRun);
if (initRoutes) initRoutes(schedule);
document.addEventListener('click', (ev) => {
  const t = ev.target;
  if (t.closest('#runpick')) { if (PK.open) closePicker(); else openPicker(); return; }
  if (t.closest('#pk-close')) { closePicker(); return; }
  if (t.closest('#pk-stubs')) { PK.stubs = !PK.stubs; renderPicker(); $('#pk-q').focus(); return; }
  const e = t.closest('[data-exp]');
  if (e) { toggleExp(e.dataset.exp); $('#pk-q').focus(); return; }
  const o = t.closest('[data-pick]');
  if (o) { selectSource(Number(o.dataset.pick)); return; }
  if (PK.open && !t.closest('#picker')) closePicker(false);
});
document.addEventListener('focusin', (ev) => { if (PK.open && !ev.target.closest('#picker')) closePicker(false); });
document.addEventListener('input', (ev) => { if (ev.target.id === 'pk-q') { PK.q = ev.target.value; PK.active = null; renderPicker(); $('#pk-list').scrollTop = 0; } });
document.addEventListener('keydown', (ev) => {
  if (!PK.open) return;
  if (ev.key === 'Escape') { ev.preventDefault(); ev.stopPropagation(); closePicker(); return; }
  if (ev.target.id !== 'pk-q') return;
  const k = {ArrowDown: 1, ArrowUp: -1, PageDown: 8, PageUp: -8}[ev.key];
  if (k) { ev.preventDefault(); moveActive(k); return; }
  if ((ev.key === 'Home' || ev.key === 'End') && ev.ctrlKey) { ev.preventDefault(); moveActive(ev.key === 'Home' ? 'first' : 'last'); return; }
  if (ev.key === 'Enter' && PK.active) {
    ev.preventDefault();
    const el = pkActive();
    if (el?.dataset.exp) toggleExp(el.dataset.exp); else if (el?.dataset.pick) selectSource(Number(el.dataset.pick));
  }
}, true);

// the case header's line: protocol, protocol revision (only when the run says one), case revision (only when there is one), evidence
function headParts(c, ev) {
  const rev = protocolRevisionOf(c), crev = ev?.explanation?.computed_from?.case?.revision || c.case.revision;
  return [esc(c.case.protocol), rev != null ? `protocol r${esc(rev)}` : '', crev != null && crev !== '' ? `case ${esc(crev)}` : '', `${Object.keys(c.evidence).length} evidence records`].filter(Boolean).join(' · ');
}

function render() {
  const src = S.sources[S.src];
  // the run picker (D-009): the header button names the run and its state; the open panel lists the runs
  renderPicker(src);
  // who this run is: an end-to-end test with a stand-in, autopilot; the graph legend follows
  const set = S.run?.settings;
  patch($('#runmark'), [isStub(src) ? `<span class="rm rm-stub" title="${esc(`${STUB_NOTE} (${src.program.why})`)}">${esc(STUB_NOTE)}</span>` : '', standIn(set) ? '<span class="rm rm-standin" title="run-settings.jsonl: stand_in true">end-to-end test with a stand-in</span>' : '', autopilot(set) ? '<span class="rm rm-auto" title="run-settings.jsonl: autonomy A5, the decider or a rule holds every decision">autopilot</span>' : ''].join(''));
  const lg = $('#lg-person');
  const lgText = autopilot(set) ? 'decider (on the operator’s behalf)' : 'person';
  if (lg && lg.textContent !== lgText) lg.innerHTML = `<i class="lg c-person"></i>${lgText}`;
  $('#follow').checked = S.follow;
  $('#reads').checked = S.showReads;
  const live = $('#live');
  live.className = `live ${S.live}`;
  patch(live, `<i></i><span>${S.live === 'on' ? 'Live' : S.live === 'connecting' ? 'Connecting' : 'Reconnecting'}</span>${S.live === 'on' && S.lastChange ? ((t) => `<span class="lt" title="${t}">${t}</span>`)(`updated ${clockTime(S.lastChange.getTime())}`) : ''}`);
  renderDecisions();
  if (daemon) return renderDaemon(src);
  const cases = ordered();
  if (!cases.length) {
    patch($('#view-note'), '');
    patch($('#phases'), '');
    patch($('#now'), '');
    patch($('#nowbar'), '');
    patch($('#kpis'), '');
    const dv = daemonView(src);
    patch($('#feed'), dv?.feed ?? '');
    patch($('#tree'), dv?.tree ?? `<div class="empty"><div class="spinner"></div><p>${S.exists ? 'No case yet in' : 'Waiting for'}</p><code>${esc(src?.path)}</code><p class="muted">The view fills in as the run writes its store.</p></div>`);
    $('#graph').innerHTML = '';
    patch($('#gtitle'), '');
    renderProductBar($('#product'), {products: S.products, daemon});
    if (S.panel && S.tab === 'products') mountProducts(null);
    else { if ($('#panel').dataset.tab === 'products') $('#panel').dataset.tab = ''; patch($('#panel'), ''); }
    document.querySelectorAll('#tabs button[data-tab]').forEach((b) => { b.classList.toggle('on', S.panel && b.dataset.tab === S.tab); b.setAttribute('aria-expanded', String(S.panel && b.dataset.tab === S.tab)); });
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
  else { nowEl.className = `m-${ns.mode}`; patch(nowEl, renderNow(ns, now, S.inbox?.items, isStub(src) ? src.program : null)); patch($('#nowbar'), renderNowBar(ns)); }
  patch($('#view-note'), viewNote());
  renderProductBar($('#product'), {products: S.products, daemon});
  patch($('#phases'), renderPhases(cases, activePhases(ns)));
  S.kpis = computeKpis(cases, S.run, S.inbox?.history, ns, now);
  // an end-to-end test with a stand-in for the person: its numbers are not eval numbers
  patch($('#kpis'), renderKpis(S.kpis, now) + (standIn(S.run?.settings) ? '<span class="k-standin" data-key="kstand" title="run-settings.jsonl: stand_in true. A stand-in took the person’s part; these values do not count as an evaluation.">not eval numbers</span>' : ''));
  S.feed = feedItems(cases, S.liveData, S.inbox?.history, S.facts);
  patch($('#feed'), renderFeed(S.feed, S.feedCls, now, !!S.liveData?.fs?.present, cases.some((x) => x.parts?.loom), S.liveData?.fs?.recordsStopped));
  patch($('#tree'), renderTree(cases, ns, now));
  const last = lastSeq(c);
  const at = S.atSeq ?? last;
  const st = caseState(c);
  patch($('#gtitle'), `<div class="gt-main"><span class="gt-id">${esc(c.id)}</span><span class="pill st-${st.state}" data-v="${esc(st.label)}">${esc(st.label)}</span>${S.atSeq != null ? `<span class="pill past">past · event ${at} of ${last}</span>` : ''}</div><div class="gt-sub">${headParts(c, ev)}</div>`);
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
  } else $('#graph').innerHTML = `<p class="muted pad">${c.parts?.loom ? `The graph is not available for this run: ${esc(c.parts.protocol?.why || 'the run wrote no protocol of this case')}.` : 'no ir.json in this case'}</p>`;
  patch($('#timeline'), renderTimeline(c, at, last));
  document.documentElement.classList.toggle('rp-open', S.panel);
  if (PHONE.matches) { $('#panel').setAttribute('tabindex', '0'); $('#panel').setAttribute('role', 'region'); $('#panel').setAttribute('aria-label', 'Panel'); }
  document.querySelectorAll('#tabs button[data-tab]').forEach((b) => { b.classList.toggle('on', S.panel && b.dataset.tab === S.tab); b.setAttribute('aria-expanded', String(S.panel && b.dataset.tab === S.tab)); });
  if (!S.panel) return;
  const scope = S.scopeCase ? c.id : null;
  const p = $('#panel');
  if (S.tab === 'products') return mountProducts(c);
  if (p.dataset.tab === 'products') p.dataset.tab = '';
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

// The page the daemon serves: the run's case tree (the view `cases`) in the tree's place with the run's events as the
// feed; in the centre the run's steps, read from its events (web/runsteps.js), with what runs now and the counts; the
// panel has the tabs the daemon serves data for (tabsOf): the run's files, its events and, when the view run-metrics
// answers a row for the run, its metrics.
function renderDaemon(src) {
  for (const id of ['#view-note', '#phases', '#now', '#kpis', '#timeline', '#graph']) patch($(id), '');
  const now = Date.now();
  const cases = S.missingRun == null ? ordered() : [];
  const dv = S.missingRun == null ? daemonView(src, routeWords, {all: S.dall}) : null;
  patch($('#feed'), dv?.feed ?? '');
  if (cases.length && (!S.sel || !S.cases.has(S.sel))) S.sel = cases[0].rel;
  const c = cases.length ? current() : null;
  const tree = S.missingRun != null ? missingRun(S.missingRun) : (dv?.head ?? '') + (cases.length ? renderTree(cases, {stopped: null, running: []}, now) : dv?.tree ?? '');
  patch($('#tree'), tree);
  const run = S.missingRun == null ? src?.daemonRun : null;
  // the row of the view run-metrics for this run, when the daemon answered one: the Metrics tab, and the calls of a
  // step whose events name none
  const metrics = run && S.metrics?.run === run.run_id ? S.metrics.row : null;
  const m = run ? withViewCalls(runSteps(runEvents(run.run_id)), metrics) : null;
  $('#dsteps').hidden = !run;
  if (run) {
    // the root case is the run: titled by its runbook, its id short beside it; a case under it by its link name
    const root = !c || !c.parent;
    const st = c ? caseState(c) : null;
    const pill = root ? `<span class="pill ds-pill ds-${esc(run.state)}" data-v="${esc(run.state)}">${esc(runStateWord(run))}</span>` : `<span class="pill st-${st.state}" data-v="${esc(st.label)}">${esc(st.label)}</span>`;
    const name = root ? run.runbook_id : c.name ?? c.id;
    const sub = root ? [c?.case?.protocol && c.case.protocol !== run.runbook_id ? `protocol ${c.case.protocol}` : '', run.started_at ? `started ${clockTime(Date.parse(run.started_at), now)}` : '', run.confinement ? `${run.confinement} confinement` : ''].filter(Boolean).join(' · ') : `case of ${run.runbook_id} · ${c.case.protocol}`;
    patch($('#gtitle'), `<div class="gt-main"><span class="gt-id">${esc(name)}</span>${root ? `<span class="gt-run" title="${esc(`run ${run.run_id}`)}">run ${esc(run.run_id.slice(-8))}</span>` : ''}${pill}</div><div class="gt-sub">${esc(sub)}</div>`);
    patch($('#dhead'), renderStepsHead(run, m, now));
    patch($('#dlist'), renderStepsTable(run, m, now, (id) => S.cases.get(id)?.name ?? (S.cases.get(id) && !S.cases.get(id).parent ? run.runbook_id : null)));
    patch($('#nowbar'), renderStepsBar(run, m, now));
  } else { patch($('#gtitle'), ''); patch($('#dhead'), ''); patch($('#dlist'), ''); patch($('#nowbar'), ''); }
  renderProductBar($('#product'), {products: S.missingRun == null ? S.products : null, daemon});
  document.documentElement.classList.toggle('rp-open', S.panel);
  // the tabs of this run: Metrics only when the view answered a row for it; a chosen tab this run lacks shows the first
  const shown = tabsOf({metrics: !!metrics});
  const tab = shown.includes(S.tab) ? S.tab : shown[0];
  document.querySelectorAll('#tabs button[data-tab]').forEach((b) => { b.hidden = !shown.includes(b.dataset.tab); b.classList.toggle('on', S.panel && b.dataset.tab === tab); b.setAttribute('aria-expanded', String(S.panel && b.dataset.tab === tab)); });
  const p = $('#panel');
  if (S.panel && tab === 'products') return mountProducts(c);
  if (p.dataset.tab === 'products') p.dataset.tab = '';
  if (S.panel && tab === 'metrics') return mountMetrics(metrics);
  if (p.dataset.tab === 'metrics') p.dataset.tab = '';
  // the Events tab: every event of the run, oldest first, the engine's lines too
  patch(p, S.panel && tab === 'events' ? eventsPane(run ? runEvents(run.run_id) : [], now) : '');
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
    // a case of the daemon's tree is labelled by its link name; the root, whose id is the run's, by the last 8
    // characters of it (as the run list names a run, web/source.js), the whole id on hover and by a copy control
    const name = x.name ?? (x.parent ? x.id.slice(x.parent.length + 1) : x.id);
    const runRoot = x.daemon && !x.parent && x.name == null;
    const label = runRoot
      ? `<span class="cid cid-run" title="${esc(x.id)}">${esc(x.id.slice(-8))}</span><button type="button" class="ib cid-copy" data-copy="${esc(x.id)}" aria-label="Copy run id" title="${esc(`Copy the run id ${x.id}`)}">Copy</button>`
      : `<span class="cid">${esc(name)}</span>`;
    const guides = Array.from({length: x.depth}, (_, i) => `<span class="guide" style="left:${16 + i * 16}px"></span>`).join('');
    return `<div data-key="c:${esc(x.rel)}" class="case${x.rel === S.sel ? ' sel' : ''}" data-case="${esc(x.rel)}" style="padding-left:${16 + x.depth * 16}px">${guides}
      <div class="cn"><span class="sdot st-${st.state}" data-v="${st.state}"></span>${label}${run.has(x) ? `<span class="cst st-running k-${run.get(x).kind}" data-v="${esc(st.label)}" title="${esc(st.label)}"><i class="cst-spin"></i>${run.get(x).since != null ? `<span class="tk" data-since="${run.get(x).since}">${fmtDur(now - run.get(x).since)}</span>` : 'running'}</span>` : x === stopCase ? `<span class="cst st-stopped" data-v="stopped" title="${esc(st.label)} when the run was stopped">stopped</span>` : `<span class="cst st-${st.state}" data-v="${esc(st.label)}">${esc(st.label)}</span>`}</div>
      <div class="cp"><span class="proto">${esc((x.case.protocol || '').replace(/^sdc\.?/, '') || 'root')}</span>${x.daemon ? '' : claimsRecorded(x) ? `<span class="bar"><i style="width:${claims.length ? (100 * t) / claims.length : 0}%"></i></span><span class="cnt">${t}/${claims.length}</span>` : `<span class="cnt nr-cnt" title="${CLAIMS_NOT_RECORDED}">claims not recorded</span>`}</div></div>`;
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
  const tag = S.inbox.tag || 'Canon decides · the run records · this page only reads';
  // the header may cut it (a stub run's label takes its room): the title keeps it readable
  if ($('#tagline').textContent !== tag) { $('#tagline').textContent = tag; $('#tagline').title = tag; }
  const btn = $('#decbtn');
  btn.classList.toggle('has', n > 0 && !S.inbox.stopped);
  btn.classList.toggle('stopped', !!S.inbox.stopped);
  btn.classList.toggle('on', S.drawer);
  patch($('#deccount'), String(n));
  document.body.classList.toggle('drawer-open', S.drawer);
  if (S.drawer) {
    if (daemon) renderDaemonDrawer(S, $('#drawer')); else patch($('#drawer'), renderDrawer(S));
    // a phone scrolls the list with the keyboard too (axe scrollable-region-focusable)
    if (PHONE.matches) $('#drawer .dr-list')?.setAttribute('tabindex', '0');
  }
  // focus follows the work: opening the drawer moves focus to its first decision control, closing it
  // back to the control that opened it
  if (S.drawer && !S.drawerWas) {
    S.drawerFrom = document.activeElement;
    const first = $('#drawer .dcard .btn.primary:not([disabled])') || $('#drclose');
    first?.focus({preventScroll: false});
  } else if (!S.drawer && S.drawerWas && S.drawerFrom?.isConnected) S.drawerFrom.focus({preventScroll: true});
  S.drawerWas = S.drawer;
}

function renderRun() {
  const r = S.run;
  if (!r || !r.hasLog) return `<p class="muted">No run.log next to the store (${esc(r?.runDir)}).</p>`;
  return `<div class="sect">run.log <span class="muted">${esc(r.runDir)}</span></div>${r.console ? `<pre class="pred">${esc(r.console)}</pre>` : ''}
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
  // the step in flight of a daemon's run: its duration in words
  for (const el of document.querySelectorAll('[data-took-since]')) { const v = fmtTook(now - Number(el.dataset.tookSince), true); if (el.textContent !== v) el.textContent = v; }
  // the band's head says how many minutes the action in flight has run: it is text, set as text
  const ns = S.nowState;
  if (ns?.proc && ns.mode === 'running') {
    ns.now = now;
    const head = nowSay(ns).head;
    const h = document.querySelector('#now .now-st');
    if (h && h.textContent !== head) h.textContent = head;
    const b = document.querySelector('#nowbar .nb-state');
    if (b?.lastChild && b.lastChild.textContent !== head) b.lastChild.textContent = head;
  }
  // the run picker's rows tick on the clock of now, not on a stopped run's frozen one
  for (const el of document.querySelectorAll('[data-run-since]')) { const v = fmtDur(runNow() - Number(el.dataset.runSince)); if (el.textContent !== v) el.textContent = v; }
  for (const el of document.querySelectorAll('[data-run-ago]')) { const v = fmtAgo(runNow() - Number(el.dataset.runAgo)); if (el.textContent !== v) el.textContent = v; }
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
  // the copy control of the root case card: the whole run id to the clipboard; the card is not selected by it
  const copy = t.closest('[data-copy]');
  if (copy) {
    ev.preventDefault();
    try { await navigator.clipboard.writeText(copy.dataset.copy); copy.classList.add('on'); } catch { copy.title = `The browser did not allow the copy of the run id ${copy.dataset.copy}`; }
    return;
  }
  if (t.closest('#decbtn') || t.closest('[data-open-drawer]')) { S.drawer = !S.drawer || !!t.closest('[data-open-drawer]'); return render(); }
  if (t.closest('#rpclose')) { setPanel(false); return render(); }
  if (t.closest('#railbtn')) { setPanel(true); return render(); }
  const fold = t.closest('[data-dfold]');
  if (fold) { S.dall = fold.dataset.dfold === '1'; render(); $('#feed [data-dfold]')?.focus(); return; }
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
  // a link from the explanation to what a step produced: an evidence record, an action
  const opn = t.closest('a[data-open]');
  if (opn) {
    ev.preventDefault();
    if (opn.dataset.case && opn.dataset.case !== S.sel && S.cases.has(opn.dataset.case)) selectCase(opn.dataset.case);
    showProduct(opn.dataset.open === 'evidence' ? {kind: 'evidence', id: opn.dataset.id} : {kind: 'action', seq: Number(opn.dataset.aseq)});
    openPanel('products');
    return render();
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
});
document.addEventListener('change', async (ev) => {
  const id = ev.target.id;
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
const pending = {cases: new Set(), run: false, full: false, live: false, sources: false, tree: false};
let inflight = false;
async function drain() {
  if (inflight) return;
  inflight = true;
  try {
    while (pending.full || pending.cases.size || pending.run || pending.live || pending.sources || pending.tree) {
      // an empty tree is a tree not read yet on the inspector's page; on the daemon's page it is a run with no case or a
      // refused read, and the tick reads the tree whole (pending.tree): a full drain there would read the run list and
      // the files of the run once more on every tick
      const full = pending.full || (!daemon && !S.cases.size);
      const cases = Array.from(pending.cases);
      const run = pending.run, live = pending.live || full, srcs = pending.sources || full, tree = pending.tree;
      pending.full = false; pending.cases.clear(); pending.run = false; pending.live = false; pending.sources = false; pending.tree = false;
      // the run list (states, rows) with the rest, so one render shows both
      if (srcs) S.sources = await api('/api/sources').catch(() => S.sources);
      // the daemon's tree is read whole: a change of the run and the two-second tick ask for it (pending.tree)
      if (full || cases.length || tree) await loadTree(full || tree ? null : cases);
      if (run) await loadRun();
      if (live) await loadLive();
      // never waited for: the page shows the run first and the product line when the files are read
      if (full || cases.length || run || live) { loadProducts(full).then(schedule); loadMetrics(full).then(schedule); }
      if (full || cases.length || run) await loadInbox();
    }
    S.lastChange = new Date();
    pickFollow();
    schedule();
  } catch { /* the next change retries */ } finally { inflight = false; }
}
// The ux loop's measured run (ux/specs/env.mjs) calls this after each step of its shared clock: fetch
// everything, render, resolve when the frame is painted. Nothing calls it otherwise.
window.__sdcSync = async () => {
  while (inflight) await new Promise((r) => setTimeout(r, 20));
  pending.full = true;
  await drain();
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
};
let stream = null;
function connect() {
  stream?.close();
  const es = stream = openEvents(S.src);
  es.addEventListener('hello', () => { if (S.live !== 'on') { S.live = 'on'; pending.full = true; drain(); } });
  es.addEventListener('change', (m) => {
    const d = JSON.parse(m.data);
    if (S.sources[d.source]) S.sources[d.source].exists = true;
    if (d.source !== S.src) return;
    d.cases.forEach((k) => pending.cases.add(k));
    if (d.run) pending.run = true;
    if (daemon && d.run) pending.tree = true;
    if (d.live) pending.live = true;
    drain();
  });
  es.onerror = () => { S.live = 'reconnecting'; schedule(); };
  // a run that appeared under --runs, a run stopped or resumed, or one that started or ended: the run picker follows
  es.addEventListener('sources', () => { pending.sources = true; drain(); });
}

(async () => {
  S.sources = await api('/api/sources');
  // ?run=<name> names the run (it survives a server that lists the runs in another order), else ?source=<index>; on
  // the daemon's page ?run=<run id>, and an id the daemon does not have shows a note and no other run
  const named = q0.get('run') ? S.sources.find((s) => (daemon ? s.path : s.name) === q0.get('run')) : null;
  if (named) S.src = named.index;
  else if (daemon && q0.get('run')) { S.src = -1; S.missingRun = q0.get('run'); }
  else if (!q0.has('source')) S.src = defaultSource(S.sources);
  S.meta = await api('/api/meta');
  await loadTree();
  await loadRun();
  await loadLive();
  loadProducts(true).then(schedule);
  loadMetrics(true).then(schedule);
  if (q0.get('case')) { S.sel = q0.get('case'); S.follow = false; } else pickFollow();
  if (q0.get('node')) S.node = q0.get('node');
  if (q0.get('at')) S.atSeq = Number(q0.get('at'));
  if (q0.get('reads')) S.showReads = true;
  await loadInbox();
  render();
  connect();
})();
