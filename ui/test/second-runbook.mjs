// A second runbook for the view tests: cases of a protocol whose ids are not SDC's (triage.ticket:
// claims ticket.read and ticket.answered, actions ticket.read, ticket.reply and ticket.ask), with files
// written by the run. Hand-made and tiny; the shape is that of the recorded runs (test/recorded), the
// names are not. It has a decision of its own (a person answers the owner's asks, in asks.yaml, with
// field names, statuses and an action that are not SDC's) and a record of its own that a view parser reads.
//
//   writeSecondRunbook(runDir)   writes runDir/store/{T-1,T-2,T-3} and runDir/fs/changes.jsonl, returns runDir/store
//     T-1  the ticket and its reply
//     T-2  waiting on the action ticket.ask (a person answers the open ask a1)
//     T-3  an open ask b1 and no wait (answered by the page's act, written with the view's field names)
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';

export const SECOND = {
  protocol: 'triage.ticket',
  claims: ['ticket.read', 'ticket.answered'],
  actions: ['ticket.read', 'ticket.reply'],
  files: ['notes/ticket.md', 'reply.txt', 'tools/classify.ts'],
  ask: {action: 'ticket.ask', check: 'ticket.check', file: 'asks.yaml', text: 'Which queue handles refunds?', options: ['queue-a', 'queue-b']},
};

// A view of this runbook (format runbook.view/1) with two phases, two file rules, a parser of its own for asks.yaml,
// two tool programs and the decision a person takes: answer an ask (fields, statuses and the action are its own).
export const SECOND_VIEW = {
  format: 'runbook.view/1',
  runbook: 'triage',
  phases: {
    stages: ['Triage'],
    items: [
      {id: 'read', stage: 'Triage', label: 'Read', protocol: SECOND.protocol, claims: ['ticket.read']},
      {id: 'answer', stage: 'Triage', label: 'Answer', protocol: SECOND.protocol, claims: ['ticket.answered']},
    ],
  },
  file_kinds: {
    rules: [
      {match: '^notes/.*\\.md$', class: 'direction', kind: '$basename'},
      {match: '(^|/)asks\\.ya?ml$', class: 'direction', kind: 'asks'},
    ],
    artifacts: {ticket: 'direction', asks: 'direction'},
    parsers: {
      asks: {
        type: 'keyed-list',
        subject: 'asks',
        list: 'asks',
        word: 'ask',
        added: [{text: 'ask {id} raised: {text!60}'}],
        changed: {rules: [{when: {was_unset: ['reply'], set: ['reply']}, text: 'ask {id} answered: {reply!40}[ by {replied_by}]'}], otherwise: 'ask {id} changed'},
      },
    },
  },
  tool_owners: {always: ['hoist'], continue: ['crane'], tools: []},
  decisions: {
    scope_answer: {
      action: SECOND.ask.action,
      check: SECOND.ask.check,
      artifact: 'asks',
      list: 'asks',
      flag: 'needs_owner',
      fields: {question: 'text', answer: 'reply', answered_by: 'replied_by', answered_at: 'replied_at'},
      statuses: {open: 'pending', accepted: 'agreed', changed: 'edited'},
      refused_answerers: ['bot'],
    },
  },
};

const w = (p, o) => { fs.mkdirSync(path.dirname(p), {recursive: true}); fs.writeFileSync(p, typeof o === 'string' ? o : JSON.stringify(o, null, 2) + '\n'); };
const jl = (a) => a.map((x) => JSON.stringify(x)).join('\n') + '\n';
const sha = (n) => n.toString(16).padStart(64, '0');
const sha256 = (t) => crypto.createHash('sha256').update(t).digest('hex');

const ASKS_RAISED = `asks:\n- id: a1\n  text: ${SECOND.ask.text}\n  needs_owner: true\n  status: pending\n  options: [${SECOND.ask.options.join(', ')}]\n  default: ${SECOND.ask.options[0]}\n- id: a2\n  text: Is the customer in the EU?\n  needs_owner: false\n  status: pending\n`;
const ASKS_ANSWERED = ASKS_RAISED.replace('  status: pending\n  options', '  status: edited\n  reply: queue-b\n  replied_by: owner-1\n  options');
const asksOf = (id) => `asks:\n- id: ${id}\n  text: ${SECOND.ask.text}\n  needs_owner: true\n  status: pending\n  options: [${SECOND.ask.options.join(', ')}]\n  default: ${SECOND.ask.options[0]}\n`;
export const ASKS = {raised: ASKS_RAISED, answered: ASKS_ANSWERED};

function ir(withAsk) {
  const o = {
    format: 'canon-ir/1',
    protocol: {id: SECOND.protocol, description: 'Read a ticket and reply to it.'},
    artifacts: {ticket: {}},
    claims: {
      'ticket.read': {description: 'the ticket was read', true_when: {evidence: {kind: 'ticket_read', result: 'ok', subject: 'ticket'}}},
      'ticket.answered': {description: 'the ticket was answered', true_when: {evidence: {kind: 'ticket_answered', result: 'ok', subject: 'ticket'}}},
    },
    evidence_kinds: {ticket_read: {description: 'the ticket text was fetched'}, ticket_answered: {description: 'a reply was sent'}},
    actions: {
      'ticket.read': {description: 'read the ticket', effect: 'read', may_produce: [{evidence: 'ticket_read'}], precondition: {all: []}, requires: []},
      'ticket.reply': {description: 'reply to the ticket', effect: 'write', may_produce: [{evidence: 'ticket_answered'}], precondition: {all: [{claim: {id: 'ticket.read', is: 'true'}}]}, requires: []},
    },
    invalidation: {},
    obligations: {},
    outcomes: {answered: {description: 'the ticket is answered', when: {claim: {id: 'ticket.answered', is: 'true'}}}},
  };
  if (withAsk) o.actions[SECOND.ask.action] = {description: 'ask the owner', effect: 'write', may_produce: [{evidence: 'ticket_read'}], precondition: {all: []}, requires: [{capability: SECOND.ask.action}]};
  return o;
}

function writeCase(store, id, {waiting = false, asks = null} = {}) {
  const d = path.join(store, id);
  const at = '2026-11-21T10:00:00Z';
  const doc = ir(!!asks);
  w(path.join(d, 'case.json'), {artifacts: {ticket: {revision: 'r1'}}, format: 'canon-case/1', id, protocol: SECOND.protocol, revision: 'c1'});
  w(path.join(d, 'ir.json'), doc);
  w(path.join(d, 'authority.json'), []);
  w(path.join(d, 'decisions.json'), []);
  w(path.join(d, 'bindings.json'), {
    $artifacts: {ticket: 'ticket.md', ...(asks ? {asks: SECOND.ask.file} : {})},
    'ticket.read': {argv: ['tools/fetch.sh'], producer: 'fetch', produces: 'ticket_read', results: {'*': 'failed', 0: 'ok'}, subject: 'ticket'},
    'ticket.reply': {argv: ['tools/reply.sh'], producer: 'reply', produces: 'ticket_answered', results: {'*': 'failed', 0: 'ok'}, subject: 'ticket'},
    ...(asks ? {[SECOND.ask.action]: {argv: ['tools/ask.sh'], producer: 'ask', produces: 'ticket_read', results: {'*': 'failed', 0: 'ok'}, subject: 'ticket', manual: true}} : {}),
  });
  if (asks) w(path.join(d, 'work', SECOND.ask.file), asks);
  w(path.join(d, 'evaluations', '0002.json'), {
    case: id,
    claims: {'ticket.read': {value: 'unknown'}, 'ticket.answered': {value: 'unknown'}},
    actions: {'ticket.read': {status: 'admissible'}, 'ticket.reply': {status: 'blocked', reasons: [{claim: 'ticket.read', value: 'unknown'}]}},
    explanation: {
      claims: {
        'ticket.read': {value: 'unknown', because: [{kind: 'ticket_read', status: 'absent', subject: 'ticket'}]},
        'ticket.answered': {value: 'unknown', because: [{kind: 'ticket_answered', status: 'absent', subject: 'ticket'}]},
      },
      actions: {'ticket.reply': {status: 'blocked', because: [{claim: 'ticket.read', value: 'unknown'}]}},
    },
  });
  w(path.join(d, 'events.jsonl'), jl([
    {at: '-', bindings: 'triage.bindings.json', event: 'opened', ir: sha(1), prev: null, protocol: doc.protocol, seq: 0},
    {artifacts: {ticket: 'r1'}, at, case_revision: 'c1', event: 'revised', prev: sha(2), seq: 1},
    {at, decision: sha(3), event: 'evaluated', file: 'evaluations/0002.json', prev: sha(4), seq: 2, state: sha(5)},
    ...(waiting ? [{at, event: 'awaiting_authority', next: [{action: SECOND.ask.action, decides: 'answer the owner asks'}], prev: sha(6), seq: 3}] : []),
  ]));
}

export function writeSecondRunbook(runDir) {
  const store = path.join(runDir, 'store');
  writeCase(store, 'T-1');
  writeCase(store, 'T-2', {waiting: true, asks: ASKS_RAISED});
  writeCase(store, 'T-3', {asks: asksOf('b1')});
  const rec = (i, p, o) => ({
    format: 'sdc.fs-change/1', seq: i, at: `2026-11-21T10:00:${String(i).padStart(2, '0')}.000Z`, root: 'store/T-1/work', path: p, op: 'create', from_path: null,
    before: null, after: null, binary: false, too_large: false, lines_added: 1, lines_removed: 0, diff: `--- /dev/null\n+++ b/${p}\n@@ -0,0 +1 @@\n+line\n`,
    attribution: {case: 'T-1', action: 'ticket.read', record: null, tool: null, basis: 'action-window'}, ...o,
  });
  const blob = (t) => { const h = sha256(t); w(path.join(runDir, 'fs', 'blobs', h), t); return h; };
  const raised = blob(ASKS_RAISED), answered = blob(ASKS_ANSWERED);
  const changes = [
    ...SECOND.files.map((p, i) => rec(i + 1, p)),
    rec(4, SECOND.ask.file, {after: raised}),
    rec(5, SECOND.ask.file, {op: 'modify', before: raised, after: answered, diff: null}),
  ];
  w(path.join(runDir, 'fs', 'changes.jsonl'), jl(changes));
  return store;
}
