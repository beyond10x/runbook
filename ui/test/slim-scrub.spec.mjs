// The committed fixture run names the role, never a person: no reason in test/recorded opens with
// one capitalised word, a comma and a date; test/slim-source.mjs writes the role itself.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {scrubReasons} from './scrub.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'file checks run once'));

const files = (dir) => fs.readdirSync(dir, {withFileTypes: true}).flatMap((e) => (e.isDirectory() ? files(path.join(dir, e.name)) : [path.join(dir, e.name)]));

test('a reason that opens with a first name and a date gets the role, nothing else changes', () => {
  const text = [
    'reason: "Alice, 2026-11-21, a daemon."',
    "reason: 'Alice, 2026-11-21: the whole ecosystem.'",
    'reason: Alice, 2026-11-21, plain',
    '{"reason": "Alice, 2026-11-21, in json"}',
    'reason: "No other language is requested, so English."',
    'reason: "Coordinator default for the trial, replacing 2026-11-21."',
    'note: "Alice, 2026-11-21, not a reason"',
    'reason: "The operator, 2026-11-21, already"',
  ].join('\n');
  expect(scrubReasons(text)).toBe([
    'reason: "The operator, 2026-11-21, a daemon."',
    "reason: 'The operator, 2026-11-21: the whole ecosystem.'",
    'reason: The operator, 2026-11-21, plain',
    '{"reason": "The operator, 2026-11-21, in json"}',
    'reason: "No other language is requested, so English."',
    'reason: "Coordinator default for the trial, replacing 2026-11-21."',
    'note: "Alice, 2026-11-21, not a reason"',
    'reason: "The operator, 2026-11-21, already"',
  ].join('\n'));
});

test('no file of the committed recorded runs holds a reason that opens with a first name and a date', () => {
  const bad = [];
  for (const f of files(path.join(here, 'recorded')).filter((x) => /\.(json|jsonl|ya?ml|md|txt)$/.test(x))) {
    const t = fs.readFileSync(f, 'utf8');
    if (scrubReasons(t) !== t) bad.push(path.relative(here, f));
  }
  expect(bad).toEqual([]);
});
