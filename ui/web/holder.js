// Who holds an open decision, what taking it does, and the fields one decision covers. Pure, shared by
// the Now band (now.js) and the Decisions drawer (inbox.js); tested in test/holder-model.spec.mjs.
//
// The run's decider setting (SDC_DECIDER of scripts/run-trial.sh) is not recorded durably: the line
// "resuming <run>: adapter …, decider <kind>" goes to the terminal before console.txt starts. What
// the run does record: a decider call in progress (decider/NNN-<action>/ with no response yet) and
// every decider decision (decisions.jsonl, principal decider:<kind>). So the holder is read from those,
// and the absence is said, never guessed.

const RULES = new Set(['approve', 'reject']);
const deciderKind = (p) => (/^decider:/.test(p || '') ? p.slice(8) : null);

// The decider decisions of the run (decisions.jsonl rows as decisionRecord shapes them), oldest first.
export function deciderDecisions(decisions) {
  return (decisions || []).filter((d) => deciderKind(d.principal)).slice().sort((a, b) => String(a.at || '').localeCompare(String(b.at || '')));
}

// The decider's decision on this wait, when there is one: same case and action, and the same revision
// when both name one.
export function deciderOn(decisions, caseId, action, revision) {
  return deciderDecisions(decisions).filter((d) => d.case === caseId && d.what === action && (!revision || !d.revision || d.revision === revision)).at(-1) || null;
}

// w: a waiting entry of nowState ({c, action, principal, kind, record, revision, subject, capability}).
// Returns {who: 'decider'|'rule'|'person', label, note, tone}.
export function holderOf(w, decisions) {
  const all = deciderDecisions(decisions);
  const lastKind = all.length ? deciderKind(all.at(-1).principal) : null;
  if (w.kind === 'decider' && w.record) {
    const m = w.record.model || null;
    return {who: 'decider', label: `Decider ${m || ''}`.trim(), note: 'deciding now, in a person’s place', tone: 'decider'};
  }
  const mine = w.action ? deciderOn(decisions, w.c?.id, w.action, w.revision) : null;
  const you = `You${w.principal ? ` (${w.principal})` : ''}`;
  if (mine) {
    const k = deciderKind(mine.principal);
    const verb = /^reject/.test(mine.decision || '') ? 'refused it' : /escalat/.test(mine.decision || '') ? 'handed it to a person' : mine.invalid ? 'gave no valid reply' : `decided ${mine.decision}`;
    const by = RULES.has(k) ? `the rule decider (${k})` : `the decider (${mine.model || k})`;
    return {who: 'person', label: you, note: `${by} ${verb}${mine.reasons?.[0] ? `: ${mine.reasons[0]}` : ''}`, tone: 'person', decider: mine};
  }
  if (lastKind && RULES.has(lastKind)) return {who: 'rule', label: `Rule (${lastKind})`, note: `the run's earlier decisions were taken by the rule decider:${lastKind}; it takes this one when the run continues`, tone: 'decider'};
  if (lastKind) return {who: 'person', label: you, note: `nobody decides this until the run continues; this run's decider setting is not recorded, its earlier decisions were taken by decider:${lastKind}`, tone: 'person'};
  return {who: 'person', label: you, note: 'no decider has decided in this run (the decider setting is not recorded; a person is the default)', tone: 'person'};
}

// What taking the decision does, in one sentence.
export function takenText(w) {
  if (w.capability && w.subject) return `Approving records ${w.capability} for ${w.subject}${w.revision ? ` @ ${String(w.revision).slice(0, 9)}` : ''}; Continue run then takes the next step.`;
  if (w.capability) return `Approving records ${w.capability}; Continue run then takes the next step.`;
  if (w.handoff) return 'Doing what it asks and continuing the run lets the driver take the next step.';
  return 'Deciding lets the run take its next step when it continues.';
}

// A structured artifact whose top-level entries are {value, status, reason}: the fields one approval
// covers. status: default (kept as proposed), accepted, changed. Who changed or accepted a field is
// the first part of its reason when the page or the program wrote it ("operator, 2026-11-21, …").
export function fieldsOf(json) {
  if (!json || typeof json !== 'object' || Array.isArray(json)) return null;
  const rows = Object.entries(json).filter(([, v]) => v && typeof v === 'object' && 'value' in v);
  if (!rows.length) return null;
  return rows.map(([name, v]) => {
    const m = /^([A-Za-z][\w.:-]*),\s*\d{4}-\d\d-\d\d\b/.exec(String(v.reason || ''));
    const by = m ? m[1] : null;
    const status = v.status || 'default';
    const state = status === 'default' ? 'default kept' : `${status}${by ? ` by ${by}` : ' (by whom not recorded)'}`;
    return {name, value: v.value, status, reason: v.reason || '', by, state};
  });
}
export function fieldCounts(fields) {
  const n = {};
  for (const f of fields || []) n[f.status] = (n[f.status] || 0) + 1;
  return Object.entries(n).map(([k, v]) => `${v} ${k}`).join(', ');
}

// Per field, what the decider's reply says about it: a reason that names the field (its key, or the
// key with spaces) says the decider would change it; the rest it kept. No reason names any field:
// "approved as a whole" (or "refused as a whole").
export function deciderPerField(fields, d) {
  if (!d || !fields) return null;
  const text = (d.reasons || []).join(' ');
  const hit = (f) => new RegExp(`\\b${f.name.replace(/_/g, '[ _]')}\\b`, 'i').test(text);
  const named = fields.filter(hit);
  const refused = /^reject/.test(d.decision || '');
  if (!named.length) return {whole: refused ? 'refused as a whole' : 'approved as a whole', fields: {}};
  const out = {};
  for (const f of fields) out[f.name] = hit(f) ? (refused ? 'decider would change this' : 'decider named this') : 'decider kept this';
  return {whole: null, fields: out};
}
