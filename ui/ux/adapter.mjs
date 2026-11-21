// The project's adapter for the UI iteration loop (SKILL § 1): what this UI is, which states it
// is driven through, and the truth of each. The generic core (ux/core/) imports nothing from here
// or from web/; the specs join the two.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
export const UI = path.resolve(here, '..');
export const RUNS = path.resolve(UI, '../runs');
// The person a played run waits on is the principal its recorded commands name (`--principal <id>`),
// read from the recorded run, so no person's id is written here.
const SOURCE = path.resolve(process.env.SDC_UI_FIXTURE_SOURCE || path.join(RUNS, 'trial-todo-1'));
const principal = (() => {
  try {
    const events = fs.readFileSync(path.join(SOURCE, 'store/IDEA-1/children/IDEA-1.direction/events.jsonl'), 'utf8');
    return (events.match(/--principal ([A-Za-z0-9:_-]+)/) || [])[1] || 'operator';
  } catch { return 'operator'; }
})();
const you = `You (${principal})`;

export const viewports = {
  gating: [{name: '1600x1000', width: 1600, height: 1000}, {name: '1366x768', width: 1366, height: 768}, {name: '390x844', width: 390, height: 844, phone: true}],
  measured: [{name: '1280x720', width: 1280, height: 720}, {name: '1920x1080', width: 1920, height: 1080},
    {name: '360x800', width: 360, height: 800, phone: true}, {name: '414x896', width: 414, height: 896, phone: true}, {name: '768x1024', width: 768, height: 1024, phone: true}],
};
export const isPhone = (name) => [...viewports.gating, ...viewports.measured].some((v) => v.name === name && v.phone);
// The UI ships one theme.
export const themes = ['dark'];
// Always visible (hard gate 2): the Now band, the KPI strip, the store selector, the Decisions control.
export const alwaysVisible = ['#now', '#kpis', '#source', '#decbtn'];
// Elements that scroll by design (hard gate 1 exceptions, recorded as decision D-001 in baseline.json):
// the protocol graph pans, and long lists scroll inside their own box.
export const scrollAllowed = ['#graph', '#tree', '.fd-list', '#panel', '#drawer', '#drawer *', '.dwrap .db', '.db', '.tl-ticks', '#modal pre', '.now-cards', '.chg-strip'];
// The primary region: glance 1 when the control the user needs is inside it.
export const primary = '#now';
// Regions the contact sheet names.
export const regions = {now: '#now', kpis: '#kpis', phases: '#phases', tree: '#tree', feed: '#feed', graph: '#graph', panel: '#right'};
// The guidance cue: the Now band's waiting state and its decision control.
export const cue = {card: '#now .nc.wait', control: {role: 'button', name: 'Decide'}};

// Phone sizes (decision D-005 in ui/README.md): vertical page scroll is allowed, horizontal page scroll
// is not. Always visible at every state change, scrolled or not: the run's state, the Now band's current
// step and the open-decision indicator (each a list: the first one rendered stands for it, so a page
// without the status bar is measured by what it shows instead). The KPI strip within one scroll screen;
// every tap target at least 44x44 CSS px; no label under 12 px and no body text under 14 px; the right
// rail and the dialogs are full-screen sheets with their close control in view.
export const phone = {
  alwaysVisible: {state: ['#nowbar .nb-state', '#now .now-s'], step: ['#nowbar .nb-step', '#now .nc .nc-act'], decisions: ['#decbtn']},
  reach: '#kpis',
  target: 44,
  text: {min: 12, body: 14},
  targets: 'button, a[href], select, input:not([type=hidden]), textarea, summary, [role=button], [role=tab], label.switch, [data-goto], [data-case], [data-feed], [data-seq], [data-tab], #graph [data-id]',
  // element scroll inside its own box stays allowed (D-001), plus the graph's own box
  scrollAllowed: [...scrollAllowed, '#kpis', '#phases .stages', '.gscroll'],
  sheets: {
    rail: {open: async (page) => { const c = page.locator('#railbtn:visible, #tabs [data-tab="explain"]:visible').first(); await c.scrollIntoViewIfNeeded(); await c.click(); }, dialog: '#right', close: '#rpclose'},
    diff: {dialog: '#diffdlg[open]', close: '[data-fsclose], .dh-nav .ib'},
    drawer: {dialog: '#drawer', close: '#drclose'},
  },
  graph: '#graph',
  // the real recorded runs measured at phone sizes, each at its moments (skipped without runs/)
  runs: [
    {name: 'trial-2-train', dir: path.join(RUNS, 'trial-2-train'), moments: ['truth', 'drawer', 'graph']},
    {name: 'trial-3-haiku', dir: path.join(RUNS, 'trial-3-haiku'), moments: ['truth', 'drawer']},
    {name: 'trial-todo-1', dir: path.join(RUNS, 'trial-todo-1'), moments: ['truth', 'rail', 'graph']},
  ],
};
// The checks of one viewport: at a phone size the phone values, else the desktop ones.
export function policy(vp) {
  if (!isPhone(vp)) return {alwaysVisible, scrollAllowed, pageScrollY: false};
  return {alwaysVisible: Object.values(phone.alwaysVisible).map((s) => s[0]), scrollAllowed: phone.scrollAllowed, pageScrollY: true};
}

// Scenarios: a run played by test/fixture-run.mjs (the driver bot: deterministic code that plays the
// run as the driver and the adapters would write it), or a recorded run read as it is. Each has its
// truth: what runs now, who is waited on, what changed last, and the completion state.
export const scenarios = [
  {name: 'agent', fixture: 'agent', speed: 3, anchor: true,
    truth: {running: [{action: 'change.implement', executor: 'LLM agent', detail: 'claude-haiku-5-5'}], waiting: null, completion: 'running'},
    tasks: ['name the action in flight and its executor', 'open the diff of the last change']},
  {name: 'two', fixture: 'two', speed: 3,
    truth: {running: [{action: 'spec.author', executor: 'LLM agent'}, {action: 'plan.cover', executor: 'Program'}], waiting: null, completion: 'running'},
    tasks: ['name both actions in flight and their executors']},
  {name: 'decider', fixture: 'decider', speed: 3,
    truth: {running: [], waiting: {action: 'roadmap.approve', who: 'Decider', detail: 'claude-haiku-5-5', holder: 'Decider claude-haiku-5-5'}, completion: 'waiting'},
    tasks: ['name who decides, for which action']},
  {name: 'person', fixture: 'person', anchor: true, write: true,
    truth: {running: [], waiting: {action: 'context.approve', who: you, holder: you, say: 'Waiting on you: nothing runs until this is decided'}, completion: 'waiting'},
    tasks: ['find why the run waits and take the decision']},
  {name: 'closed', store: path.join(RUNS, 'trial-todo-1/store'),
    truth: {running: [], waiting: null, completion: 'closed', outcome: 'operating', cost: /^\$\d+\.\d\d$/},
    tasks: ['read the outcome and the total cost']},
  {name: 'changes', fixture: 'changes', rotating: true,
    truth: {running: [{action: 'spec.author', executor: 'LLM agent'}, {action: 'plan.cover', executor: 'Program'}], waiting: null, completion: 'running',
      lastChange: {cls: 'planning', text: 'broken.md created'}},
    tasks: ['open the last change and read what it means', 'show only the specification changes']},
];
