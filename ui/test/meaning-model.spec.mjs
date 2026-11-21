// What a change means (web/meaning.js), the "What changed" feed (web/feed.js) and the run KPIs
// (web/kpis.js), without a browser. Runs once (wide project).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {parseYaml, frontMatter, classify, meaningOf, storeItems} from '../web/meaning.js';
import {feedItems} from '../web/feed.js';
import {computeKpis, isPerson} from '../web/kpis.js';
import {needRun} from './need-run.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const WORK = path.resolve(here, '../../runs/trial-todo-1/store/IDEA-1/work');
const read = (p) => { needRun(WORK, 'meaning-model.spec.mjs'); return fs.readFileSync(path.join(WORK, p), 'utf8'); };
const texts = (m) => m.items.map((i) => i.text);
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure model tests run once'));

test('YAML reader: block and flow collections, scalars, comments, block scalars', () => {
  const v = parseYaml([
    '# a comment', 'name: todo # trailing', "quote: 'it''s # not a comment'", 'dq: "a\\nb"', 'list: [a, "b, c", 3]', 'map: {from: "draft", to: active, at: 2026-11-20T23:57:52Z}',
    'items:', '  - id: r1', '    cites: [d1, q1]', '  - id: r2', '    text: >-', '      folded', '      line', 'keep: |', '  one', '  two', 'nested:', '- x', '- y', 'empty:', 'n: 12', 'b: true',
  ].join('\n'));
  expect(v).toEqual({name: 'todo', quote: "it's # not a comment", dq: 'a\nb', list: ['a', 'b, c', 3], map: {from: 'draft', to: 'active', at: '2026-11-20T23:57:52Z'},
    items: [{id: 'r1', cites: ['d1', 'q1']}, {id: 'r2', text: 'folded line'}], keep: 'one\ntwo\n', nested: ['x', 'y'], empty: null, n: 12, b: true});
  expect(() => parseYaml('a: [unclosed')).toThrow();
});

test('YAML reader agrees with the recorded run files', () => {
  const spec = parseYaml(read('spec/domains/storage.yaml'));
  expect(spec.entities[0].lifecycle).toEqual({initial: 'Stored', states: ['Stored'], terminal: ['Stored']});
  expect(spec.commands[0].outcomes.map((o) => o.name)).toEqual(['stored', 'write-failed']);
  const fm = frontMatter(read('aep/story/storage.md'));
  expect(fm.data).toMatchObject({id: 'story:storage', status: 'active', relations: [{delivers: 'epic:todolist'}, {serves: 'vision:product'}], scope: [{confidence: 'cited', path: 'impl'}]});
  expect(fm.data.transitions[0]).toEqual({from: 'draft', to: 'active', at: '2026-11-20T23:57:52Z', actor: expect.stringMatching(/^human:/), revision: 3});
});

test('classify: planning, specification, direction, decision, code by path', () => {
  const c = (p) => classify(p).cls;
  expect(c('aep/story/storage.md')).toBe('planning');
  expect(c('repo/.engineering/planning/task/x.md')).toBe('planning');
  expect(c('aep/initiative/roadmap.md')).toBe('direction');
  expect(c('aep/vision/product.md')).toBe('direction');
  expect(c('out/spec/domains/storage.yaml')).toBe('spec');
  expect(c('spec/system.yaml')).toBe('spec');
  expect(c('questions.yaml')).toBe('direction');
  expect(c('requirements.yaml')).toBe('direction');
  expect(c('docs/adr/0001-data-file.md')).toBe('decision');
  expect(c('impl/src/lib.rs')).toBe('code');
  expect(c('impl/Cargo.lock')).toBe('code');
});

test('specification: entities, fields, states, transitions, outcomes, named', () => {
  const before = read('spec/domains/storage.yaml');
  const after = before.replace(/( {4}fields:\n {6}- name: title\n {8}type: String\n)/, '$1      - name: due\n        type: Date\n')
    .replace('      states: [Stored]\n      terminal: [Stored]', '      states: [Stored, Done]\n      terminal: [Done]\n      transitions:\n        - from: Stored\n          to: Done\n          on: todo.storage.CompleteItem')
    .replace(/( {6}# Decided by the data file)/, '      - name: duplicate\n        error: todo.storage.StorageUnavailable\n\n$1')
    .replace(/views:[\s\S]*$/, '');
  const m = meaningOf({path: 'spec/domains/storage.yaml', op: 'modify'}, before, after);
  expect(m.cls).toBe('spec');
  expect(texts(m)).toEqual(['entity TodoItem: field `due` added', 'entity TodoItem: state `Done` added', 'entity TodoItem: transition Stored → Done on CompleteItem added', 'entity TodoItem: terminal states Stored → Done', 'command AddItem: outcome `duplicate` added', 'view AllItems removed']);
  const created = meaningOf({path: 'spec/domains/storage.yaml', op: 'create'}, null, before);
  expect(texts(created)[0]).toBe('domain todo.storage created: 1 type, 1 entity, 1 command, 1 event, 1 view, 1 error');
  const sys = read('spec/system.yaml');
  expect(texts(meaningOf({path: 'spec/system.yaml', op: 'modify'}, sys, sys.replace('  - todo.storage', '  - todo.storage\n  - todo.export')))).toEqual(['domain todo.export added to the system']);
});

test('planning: created, status moved, edges and scope, acceptance', () => {
  const active = read('aep/story/storage.md');
  const draft = active.replace('status: active', 'status: draft').replace('- serves: vision:product\n', '').replace(/scope:\n- confidence: cited\n {2}path: impl\n/, '').replace(/transitions:\n.*\n/, '');
  expect(texts(meaningOf({path: 'aep/story/storage.md', op: 'create'}, null, draft))).toEqual(['story storage created: “Persist to-do items server-side” (draft)', 'edge: delivers epic:todolist']);
  expect(texts(meaningOf({path: 'aep/story/storage.md', op: 'modify'}, draft, active))).toEqual(['story storage: draft → active', 'story storage: edge added: serves vision:product', 'story storage: scope added: impl']);
  const withAcc = active + '\n## Acceptance\n\nAn added item is listed after a restart.\n';
  expect(texts(meaningOf({path: 'aep/story/storage.md', op: 'modify'}, withAcc, withAcc.replace('after a restart', 'after two restarts')))).toEqual(['story storage: acceptance changed']);
  expect(texts(meaningOf({path: 'aep/story/storage.md', op: 'modify'}, active, active.replace('- serves: vision:product', '- serves: vision:product\n- depends_on: story:list')))).toEqual(['story storage: edge added: depends on story:list']);
  expect(texts(meaningOf({path: 'aep/story/storage.md', op: 'delete'}, active, null))).toEqual(['story storage removed']);
});

test('direction: requirements, questions, context, roadmap', () => {
  const req = read('requirements.yaml');
  expect(texts(meaningOf({path: 'requirements.yaml'}, req, req + '  - id: r4\n    text: The list can be exported as JSON.\n'))).toEqual(['requirement r4 added: “The list can be exported as JSON.”']);
  const q = read('questions.yaml');
  const open = q.replace("    status: changed\n    answer: Kept on the server, in the daemon's data file\n    answered_by: human", '    status: open');
  expect(texts(meaningOf({path: 'questions.yaml', op: 'modify'}, open, q))).toEqual(['question q3 answered: “Kept on the server, in the daemon\'s data file” (by human)']);
  const ctx = read('context.yaml');
  expect(texts(meaningOf({path: 'context.yaml', op: 'modify'}, ctx, ctx.replace('value: MIT', 'value: Apache-2.0')))).toEqual(['context licence: “MIT” → “Apache-2.0” (changed)']);
  const rm = read('aep/initiative/roadmap.md');
  const two = rm.replace('## Sequence', '### export: Export the list\n\n- x\n\n## Sequence');
  expect(texts(meaningOf({path: 'aep/initiative/roadmap.md', op: 'modify'}, rm, two))).toEqual(['roadmap: epic export added']);
  const swapped = two.replace(/### todolist:([\s\S]*?)(### export:[\s\S]*?)(## Sequence)/, '$2### todolist:$1$3');
  expect(texts(meaningOf({path: 'aep/initiative/roadmap.md', op: 'modify'}, two, swapped))).toEqual(['roadmap: epic order changed (todolist, export → export, todolist)']);
});

test('decisions written down, code, and what cannot be parsed', () => {
  const adr = '---\ntitle: Keep items in a data file\nstatus: proposed\n---\n# 0001 Keep items in a data file\n';
  expect(texts(meaningOf({path: 'docs/adr/0001.md', op: 'create'}, null, adr))).toEqual(['decision recorded: “Keep items in a data file” (proposed)']);
  expect(texts(meaningOf({path: 'docs/adr/0001.md', op: 'modify'}, adr, adr.replace('proposed', 'accepted')))).toEqual(['decision “Keep items in a data file”: proposed → accepted']);
  expect(meaningOf({path: 'impl/src/lib.rs', op: 'modify'}, null, null)).toMatchObject({cls: 'code', items: [{text: 'changed impl/src/lib.rs'}]});
  const bad = meaningOf({path: 'aep/story/x.md', op: 'create'}, null, '---\nkind: story\nrelations: [unclosed\n---\n');
  expect(bad).toMatchObject({cls: 'planning', parsed: false});
  expect(texts(bad)[0]).toMatch(/^x\.md created \(not parsed: /);
  expect(meaningOf({path: 'spec/system.yaml', op: 'modify', binary: true}, null, null)).toMatchObject({cls: 'spec', parsed: false, items: [{text: 'system.yaml changed: content not recorded (binary)'}]});
});

// ---- runs without fs/changes.jsonl, the feed, the KPIs ---------------------------------------------
const ev = (seq, event, extra = {}) => ({seq, event, at: '2026-11-20T23:16:04Z', ...extra});
const mk = (id, events, extra = {}) => ({id, rel: id, parent: null, depth: 0, events, times: {}, evidence: {}, bindings: {}, ir: {actions: {}}, case: {protocol: 'sdc.direction'}, ...extra});

test('without fs/changes.jsonl: revisions by action and AEP transitions, with what is not recorded', () => {
  const c = mk('IDEA-1.direction', [ev(1, 'action_started', {action: 'roadmap.write'}), ev(2, 'action_finished', {action: 'roadmap.write', exit: 0, revises: {roadmap: 't1'}}), ev(3, 'action_finished', {action: 'roadmap.write', exit: 0, revises: {roadmap: 't2'}})]);
  const items = storeItems([c], [{path: 'IDEA-1/work/aep/story/storage.md', text: read('aep/story/storage.md')}]);
  expect(items.map((x) => [x.cls, x.text])).toEqual([['direction', 'roadmap written'], ['direction', 'roadmap revised'], ['planning', 'story storage: draft → active']]);
  expect(items[0].note).toMatch(/not recorded/);
  expect(items[2]).toMatchObject({principal: expect.stringMatching(/^human:/), t: Date.parse('2026-11-20T23:57:52Z')});
});

test('feed: one row per meaning, code changes of one step grouped, decisions with grounds, newest first', () => {
  const at = (s) => `2026-11-21T00:00:${String(s).padStart(2, '0')}Z`;
  const a = {case: 'IDEA-1.todolist.spec', action: 'spec.author', record: 'llm/013-spec.author', tool: {seq: 4, name: 'Edit', input: ''}, basis: 'tool-window'};
  const live = {fs: {present: true, changes: [
    {seq: 1, at: at(1), path: 'impl/src/a.rs', op: 'create', lines_added: 3, lines_removed: 0, attribution: {...a, action: 'change.implement'}},
    {seq: 2, at: at(2), path: 'impl/src/b.rs', op: 'modify', lines_added: 1, lines_removed: 1, attribution: {...a, action: 'change.implement'}},
    {seq: 3, at: at(3), path: 'out/spec/domains/storage.yaml', op: 'modify', attribution: a, meaning: {cls: 'spec', parsed: true, items: [{text: 'entity TodoItem: field `due` added'}, {text: 'command AddItem: outcome `duplicate` added'}]}},
  ]}};
  const history = [{at: at(9), principal: 'decider:haiku', case: 'IDEA-1.direction', what: 'roadmap.approve', decision: 'reject', reasons: ['the roadmap is empty'], src: 'decisions.jsonl'},
    {at: at(8), principal: 'operator', case: 'IDEA-1.intake', what: 'q3', question: 'Where are items kept?', decision: 'on the server', src: 'work/questions.yaml'}];
  const f = feedItems([mk('IDEA-1.todolist.spec', [])], live, history, null);
  expect(f.map((x) => [x.cls, x.text])).toEqual([
    ['decision', 'roadmap.approve: rejected'], ['direction', 'question q3 answered: “on the server”'], ['spec', 'entity TodoItem: field `due` added'], ['code', '2 code files changed']]);
  expect(f[0]).toMatchObject({grounds: 0, who: {kind: 'decider'}, more: ['the roadmap is empty']});
  expect(f[2]).toMatchObject({fs: 3, who: {kind: 'llm', label: 'LLM agent · Edit'}, more: ['command AddItem: outcome `duplicate` added']});
  expect(f[3]).toMatchObject({add: 4, del: 1, fs: 2});
});

test('KPIs: values where recorded, "not recorded" where not, waiting time as a union', () => {
  const t = (m) => `2026-11-21T00:${String(m).padStart(2, '0')}:00Z`;
  const c1 = mk('A', [{seq: 1, event: 'action_started', at: t(0)}, {seq: 2, event: 'awaiting_person', at: t(10)}, {seq: 3, event: 'action_finished', at: t(20)}, {seq: 4, event: 'terminated', outcome: 'passed', at: t(30)}]);
  const c2 = mk('B', [{seq: 1, event: 'awaiting_authority', at: t(15)}, {seq: 2, event: 'action_finished', at: t(25)}]);
  const now = Date.parse(t(40));
  const none = computeKpis([c1, c2], {llm: [], decisions: []}, [], {mode: 'closed'}, now);
  const by = (k, key) => k.find((x) => x.key === key);
  expect(by(none, 'time')).toMatchObject({value: 30 * 60000, text: '30:00', sub: {text: '15:00'}});
  expect(by(none, 'cost')).toMatchObject({value: null, text: null});
  expect(by(none, 'calls').text).toBe(null);
  expect(by(none, 'progress')).toMatchObject({text: '1/2'});
  const run = {llm: [{where: 'llm-calls.jsonl', record: {model: 'm', cost_usd: 0.25}}, {where: 'llm-calls.jsonl', record: {model: 'm', cost_usd: 0.5}}],
    decisions: [{model: 'm', cost: 0.01, decision: 'approve'}, {model: 'm', cost: 0.02, decision: 'escalate'}]};
  const hist = [{principal: 'operator'}, {principal: 'decider:haiku'}, {principal: 'non-interactive'}, {principal: 'decider:approve'}];
  const k = computeKpis([c1, c2], run, hist, {mode: 'running'}, now);
  expect(by(k, 'cost')).toMatchObject({text: '$0.78', parts: [{label: 'work', text: '$0.750'}, {label: 'decider', text: '$0.030'}]});
  expect(by(k, 'calls')).toMatchObject({text: '4', parts: [{text: '2'}, {text: '2'}]});
  expect(by(k, 'decisions')).toMatchObject({text: '1', parts: [{label: 'escalated', text: '1'}, {label: 'by decider', text: '2'}]});
  expect(by(k, 'time')).toMatchObject({value: 40 * 60000, live: Date.parse(t(0))});
  expect([isPerson('operator'), isPerson('decider:haiku'), isPerson('non-interactive'), isPerson('rule:x'), isPerson(null)]).toEqual([true, false, false, false, false]);
});
