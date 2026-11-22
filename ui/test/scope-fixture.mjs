#!/usr/bin/env node
// A run stopped where the interview waits for scope.answer, made by the real driver with the stub
// executors (the way scripts/run-full-cycle.sh makes its stores): sdc-drive init, then `run` until
// the intake case is awaiting_authority for the capability scope.answer.
//
//   node test/scope-fixture.mjs <target-run-dir> [one|two]
//
// one: the stub interview (q2 is scope-changing). two: q1 and q2 are both scope-changing and open.
// Writes <target-run-dir>/store/IDEA-1. Never touches anything else.
import fs from 'node:fs';
import path from 'node:path';
import {execFileSync, spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {missingTool} from './need-run.mjs';
import {canonPath, drivePath} from '../binaries.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const sdc = path.resolve(here, '../..');
const [targetArg, variant = 'one'] = process.argv.slice(2);
if (!targetArg || !['one', 'two'].includes(variant)) { console.error('usage: scope-fixture.mjs <target-run-dir> [one|two]'); process.exit(2); }
const target = path.resolve(targetArg);
const ex = path.join(sdc, 'driver/examples/sdc');
const drive = drivePath(sdc);
const canon = canonPath();
for (const [f, how] of [[drive, 'cargo build in driver/'], [canon, 'cargo build in the canon repository']]) { const m = missingTool(f, how); if (m) { console.error(`scope-fixture.mjs: ${m}`); process.exit(2); } }
const root = path.join(target, 'store', 'IDEA-1');
const stubs = variant === 'two' ? path.join(here, 'scope-fixtures/two') : path.join(ex, 'fixtures/scope-question');
const env = {...process.env, SDC_STUB_FIXTURES: stubs};

fs.rmSync(target, {recursive: true, force: true});
fs.mkdirSync(target, {recursive: true});
const d = (...args) => spawnSync(drive, ['--canon', canon, '--case-dir', root, ...args], {env, encoding: 'utf8', timeout: 120000});
const init = d('init', '--protocol', path.join(sdc, 'protocol/sdc.protocol.yaml'), '--id', 'IDEA-1', '--bindings', path.join(ex, 'sdc.bindings.json'));
if (init.status !== 0) { console.error(init.stderr); process.exit(1); }
fs.copyFileSync(path.join(ex, 'fixtures/idea.yaml'), path.join(root, 'work/idea.yaml'));
for (let i = 1; i <= 6; i++) {
  const r = d('run', '--at', `2026-11-20T12:0${i}:00Z`);
  let out;
  try { out = JSON.parse(r.stdout); } catch { console.error(r.stdout, r.stderr); process.exit(1); }
  if (out.verdict === 'awaiting_authority' && (out.next || []).some((n) => n.action === 'scope.answer')) { console.log(`scope fixture ${variant}: ${target}`); process.exit(0); }
}
console.error('the run never waited for scope.answer');
process.exit(1);
