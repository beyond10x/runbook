// B1 and B2 for one product: the README's build command and the tree's test command, each in the
// confinement through slot. Writes results/build-<id>.json and results/build-<id>.log.
//   node build.ts <quality dir> <id> <cargo build args...>
import {spawnSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const [q, id, ...buildArgs] = process.argv.slice(2);
const dir = path.join(q, 'products', id);
const ch = path.join(q, 'cargo-home', id);
const confine = path.join(path.dirname(new URL(import.meta.url).pathname), 'confine.ts');
fs.mkdirSync(path.join(q, 'results'), {recursive: true});
const log = path.join(q, 'results', `build-${id}.log`);
fs.writeFileSync(log, '');

function run(label: string, args: string[]) {
  const t0 = Date.now();
  const r = spawnSync('slot', ['node', confine, ...args], {encoding: 'utf8', maxBuffer: 64 << 20, timeout: 900_000});
  const out = (r.stdout ?? '') + (r.stderr ?? '');
  fs.appendFileSync(log, `### ${label}: slot node confine.ts ${args.join(' ')}\n${out}\n### exit ${r.status}\n`);
  return {command: `cargo ${args[0]} ${args.slice(3).join(' ')}`.trim(), exit: r.status, seconds: (Date.now() - t0) / 1000, out};
}

const b = run('build', ['build', dir, ch, ...buildArgs]);
const t = run('test', ['test', dir, ch, '--locked']);
let passed = 0, failed = 0, ignored = 0;
for (const m of t.out.matchAll(/test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored/g)) {
  passed += +m[1]; failed += +m[2]; ignored += +m[3];
}
const res = {
  id,
  B1: {score: b.exit === 0 ? 'pass' : 'fail', step: b.command + ' (in confinement, offline)', got: `exit ${b.exit}, ${b.seconds.toFixed(0)} s`},
  B2: {score: t.exit === 0 && failed === 0 && passed > 0 ? 'pass' : 'fail', step: 'cargo test --locked (Taskfile check)',
    got: `exit ${t.exit}; ${passed} passed, ${failed} failed, ${ignored} ignored`},
  tests: {passed, failed, ignored},
};
fs.writeFileSync(path.join(q, 'results', `build-${id}.json`), JSON.stringify(res, null, 2) + '\n');
console.log(id, res.B1.score, res.B1.got, '|', res.B2.score, res.B2.got);
