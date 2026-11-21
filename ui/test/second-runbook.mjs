// A second runbook for the view tests: one case of a protocol whose ids are not SDC's
// (triage.ticket: claims ticket.read and ticket.answered, actions ticket.read and ticket.reply),
// with three files written by the run. Hand-made and tiny (one case, three events, one evaluation);
// the shape is that of the recorded runs (test/recorded), the names are not.
//
//   writeSecondRunbook(runDir)   writes runDir/store/T-1 and runDir/fs/changes.jsonl, returns runDir/store
import fs from 'node:fs';
import path from 'node:path';

export const SECOND = {
  protocol: 'triage.ticket',
  claims: ['ticket.read', 'ticket.answered'],
  actions: ['ticket.read', 'ticket.reply'],
  files: ['notes/ticket.md', 'reply.txt', 'tools/classify.ts'],
};

// A view of this runbook (format runbook.view/1) with two phases, one file rule and two tool programs.
export const SECOND_VIEW = {
  format: 'runbook.view/1',
  phases: {
    stages: ['Triage'],
    items: [
      {id: 'read', stage: 'Triage', label: 'Read', protocol: SECOND.protocol, claims: ['ticket.read']},
      {id: 'answer', stage: 'Triage', label: 'Answer', protocol: SECOND.protocol, claims: ['ticket.answered']},
    ],
  },
  file_kinds: {rules: [{match: '^notes/.*\\.md$', class: 'direction', kind: '$basename'}], artifacts: {ticket: 'direction'}},
  tool_owners: {always: ['hoist'], continue: ['crane'], tools: []},
};

const w = (p, o) => { fs.mkdirSync(path.dirname(p), {recursive: true}); fs.writeFileSync(p, typeof o === 'string' ? o : JSON.stringify(o, null, 2) + '\n'); };
const jl = (a) => a.map((x) => JSON.stringify(x)).join('\n') + '\n';
const sha = (n) => n.toString(16).padStart(64, '0');

export function writeSecondRunbook(runDir) {
  const store = path.join(runDir, 'store');
  const d = path.join(store, 'T-1');
  const at = '2026-11-21T10:00:00Z';
  const ir = {
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
  w(path.join(d, 'case.json'), {artifacts: {ticket: {revision: 'r1'}}, format: 'canon-case/1', id: 'T-1', protocol: SECOND.protocol, revision: 'c1'});
  w(path.join(d, 'ir.json'), ir);
  w(path.join(d, 'authority.json'), {});
  w(path.join(d, 'decisions.json'), []);
  w(path.join(d, 'bindings.json'), {
    $artifacts: {ticket: 'ticket.md'},
    'ticket.read': {argv: ['tools/fetch.sh'], producer: 'fetch', produces: 'ticket_read', results: {'*': 'failed', 0: 'ok'}, subject: 'ticket'},
    'ticket.reply': {argv: ['tools/reply.sh'], producer: 'reply', produces: 'ticket_answered', results: {'*': 'failed', 0: 'ok'}, subject: 'ticket'},
  });
  w(path.join(d, 'evaluations', '0002.json'), {
    case: 'T-1',
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
    {at: '-', bindings: 'triage.bindings.json', event: 'opened', ir: sha(1), prev: null, protocol: ir.protocol, seq: 0},
    {artifacts: {ticket: 'r1'}, at, case_revision: 'c1', event: 'revised', prev: sha(2), seq: 1},
    {at, decision: sha(3), event: 'evaluated', file: 'evaluations/0002.json', prev: sha(4), seq: 2, state: sha(5)},
  ]));
  const changes = SECOND.files.map((p, i) => ({
    format: 'sdc.fs-change/1', seq: i + 1, at: `2026-11-21T10:00:0${i + 1}.000Z`, root: 'store/T-1/work', path: p, op: 'create', from_path: null,
    before: null, after: null, binary: false, too_large: false, lines_added: 1, lines_removed: 0, diff: `--- /dev/null\n+++ b/${p}\n@@ -0,0 +1 @@\n+line\n`,
    attribution: {case: 'T-1', action: 'ticket.read', record: null, tool: null, basis: 'action-window'},
  }));
  w(path.join(runDir, 'fs', 'changes.jsonl'), jl(changes));
  return store;
}
