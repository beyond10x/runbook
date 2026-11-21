// Who a record names, read from its principal, and how a run is set up: the one place the page
// decides whether to say "person". Pure; shared by every view (and decisions.mjs on the server).
//
//   person:<id>      a person; "you" when the id is the page's own principal
//   decider:<kind>   an agent deciding on the operator's behalf ("decider <model>, on the operator's
//                    behalf"); decider:approve and decider:reject are the rule deciders
//   rule:<name>      a rule ("rule <name>")
//   human            a legacy record without an id: "recorded as human (principal not recorded)",
//                    never a person
//   non-interactive, derived, dry-run   the agent's own answer, a derived fact, a dry run
//   <id>             a bare id: the person a driver's --principal named before the person: prefix
//                    (the driver and this server refuse non-person names there)
// "it should never say person if there isn't a person" (the operator, 2026-11-21).

const RULE_DECIDERS = new Set(['approve', 'reject']);
const SYSTEM = {'non-interactive': 'the agent’s own answer (non-interactive)', derived: 'derived', 'dry-run': 'dry run'};

export function principalOf(p, {model = null} = {}) {
  const s = p == null ? '' : String(p);
  let m;
  if (!s) return {kind: 'none', label: 'principal not recorded', person: false, raw: s};
  if ((m = /^person:(.+)$/.exec(s))) return {kind: 'person', id: m[1], label: m[1], person: true, raw: s};
  if ((m = /^decider:(.+)$/.exec(s))) {
    if (RULE_DECIDERS.has(m[1])) return {kind: 'rule', id: m[1], label: `rule ${m[1]}`, person: false, raw: s};
    return {kind: 'decider', id: m[1], model: model || null, label: `decider ${model || m[1]}, on the operator’s behalf`, short: `decider ${model || m[1]}`, person: false, raw: s};
  }
  if ((m = /^rule:(.+)$/.exec(s))) return {kind: 'rule', id: m[1], label: `rule ${m[1]}`, person: false, raw: s};
  if (s === 'human') return {kind: 'legacy', label: 'recorded as human (principal not recorded)', short: 'recorded as human', person: false, raw: s};
  if (SYSTEM[s]) return {kind: 'agent', label: SYSTEM[s], person: false, raw: s};
  if (/^[A-Za-z0-9._-]+$/.test(s)) return {kind: 'person', id: s, label: s, person: true, legacy: true, raw: s};
  return {kind: 'none', label: s, person: false, raw: s};
}
export const isPersonPrincipal = (p) => principalOf(p).person;
// the short form for a badge or a chip
export const principalShort = (p, o) => { const x = principalOf(p, o); return x.short || x.label; };

// The run's settings record (run-settings.jsonl, last line): autopilot is autonomy A5, where the
// decider or a rule holds every decision; stand_in marks an end-to-end test whose person is a stand-in.
const level = (s) => { const a = s?.autonomy ?? s?.SDC_AUTONOMY ?? null; return a && typeof a === 'object' ? a.level || null : a; };
export const autopilot = (settings) => String(level(settings) || '').toUpperCase() === 'A5';
export const standIn = (settings) => settings?.stand_in === true;

// The page's current run settings, for views that do not get the run passed (the graph's chips and
// legend, the explanation panel); app.js sets it when it loads the run.
let current = null;
export const setRunSettings = (s) => { current = s || null; };
export const runSettings = () => current;
// An actor kind's label: in autopilot no action is a person's, the decider takes it
export const actorLabel = (kind, labels, settings = current) => (kind === 'person' && autopilot(settings) ? 'decider' : labels[kind] || kind);
