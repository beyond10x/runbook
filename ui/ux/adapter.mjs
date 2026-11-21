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
  gating: [{name: '1600x1000', width: 1600, height: 1000}, {name: '1366x768', width: 1366, height: 768}],
  measured: [{name: '1280x720', width: 1280, height: 720}, {name: '1920x1080', width: 1920, height: 1080}],
};
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
