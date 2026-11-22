// What a change means (web/meaning.js) against the specification files of the recorded run
// runs/trial-todo-1 (git-ignored; its committed slim copy has none): slow lane. The other tests of
// web/meaning.js are in meaning-model.spec.mjs.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {parseYaml, frontMatter, meaningOf} from '../web/meaning.js';
import {needRun} from './need-run.mjs';
import {useSdcView} from './sdc-view.mjs';
useSdcView();

const here = path.dirname(fileURLToPath(import.meta.url));
const WORK = path.resolve(here, '../../runs/trial-todo-1/store/IDEA-1/work');
const read = (p) => { needRun(WORK, 'meaning-runs.spec.mjs', test); return fs.readFileSync(path.join(WORK, p), 'utf8'); };
const texts = (m) => m.items.map((i) => i.text);
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'pure model tests run once'));

test('YAML reader agrees with the recorded run files', () => {
  const spec = parseYaml(read('spec/domains/storage.yaml'));
  expect(spec.entities[0].lifecycle).toEqual({initial: 'Stored', states: ['Stored'], terminal: ['Stored']});
  expect(spec.commands[0].outcomes.map((o) => o.name)).toEqual(['stored', 'write-failed']);
  const fm = frontMatter(read('aep/story/storage.md'));
  expect(fm.data).toMatchObject({id: 'story:storage', status: 'active', relations: [{delivers: 'epic:todolist'}, {serves: 'vision:product'}], scope: [{confidence: 'cited', path: 'impl'}]});
  expect(fm.data.transitions[0]).toEqual({from: 'draft', to: 'active', at: '2026-11-20T23:57:52Z', actor: expect.stringMatching(/^human:/), revision: 3});
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

