// Every read of a file of a run goes through the one checked open of runfiles.mjs (readText, readRegular), and no program
// opens a run's file by its name. This holds the inventory of the calls that open, read or list a file in the modules of the
// server (ui/*.mjs): each is either in runfiles.mjs, the one place, or reads something that is not a file of a run. A new call
// is a change to the table below and a decision about which of the two it is; a direct open of a file of a run in server.mjs,
// decisions.mjs or products.mjs, or a program given the name of a run's file, turns this red.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const CALLS = ['fs.openSync(', 'fs.readSync(', 'fs.readFileSync(', 'fs.createReadStream(', 'fs.opendirSync(', 'fs.readdirSync(', 'execFileSync(', 'execSync(', 'spawnSync(', 'spawn(', 'fork('];
// what each module may call, and why it is not an open of a run's file by name
const ALLOWED = {
  'runfiles.mjs': {'fs.openSync(': 1, 'fs.readSync(': 1, 'fs.readFileSync(': 1, 'fs.readdirSync(': 1}, // openChecked and what reads its descriptor; listDir after runPath
  'products.mjs': {'fs.opendirSync(': 1}, // the listing walk: names of a real directory under a work/ root, entry by entry
  'decisions.mjs': {'execFileSync(': 1}, // yq, given the text on standard input
  'runnerlive.mjs': {'fs.readFileSync(': 1}, // /proc/<pid>/stat of the runner: the kernel's start time of a process, not a file of a run
  'vclock.mjs': {'fs.readFileSync(': 2}, // the operator's --clock directory
  'server.mjs': {'fs.readFileSync(': 4, 'fs.readdirSync(': 2}, // --view files, the checkout (readRepoJson, readRepoText, listRepoDir), the page's own files
};
const modules = fs.readdirSync(ui).filter((n) => n.endsWith('.mjs')).sort();
const count = (text, call) => text.split(call).length - 1;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'no server: one project is enough'));

test('the calls that open, read or list a file in the modules of the server are the ones of the table', () => {
  const seen = {};
  for (const m of modules) {
    const text = fs.readFileSync(path.join(ui, m), 'utf8');
    for (const call of CALLS) if (count(text, call)) (seen[m] ??= {})[call] = count(text, call);
  }
  expect(seen).toEqual(ALLOWED);
});

test('the one program the server starts for a run is yq, with the text on standard input and no file name', () => {
  const text = fs.readFileSync(path.join(ui, 'decisions.mjs'), 'utf8');
  const call = text.split('\n').find((l) => l.includes('execFileSync('));
  expect(call).toContain("['-o=json', '.']");
  expect(call).toContain('input: text');
  expect(modules.filter((m) => /node:child_process/.test(fs.readFileSync(path.join(ui, m), 'utf8')))).toEqual(['decisions.mjs']);
});

test('the reads of server.mjs that do not go through runfiles.mjs take a checkout file, a --view file or a file of the page', () => {
  const lines = fs.readFileSync(path.join(ui, 'server.mjs'), 'utf8').split('\n').filter((l) => /fs\.(readFileSync|readdirSync)\(/.test(l));
  const kinds = lines.map((l) => (/readRepoJson = |JSON\.parse\(fs\.readFileSync\(p,/.test(l) ? 'readRepoJson' : /readRepoText = /.test(l) ? 'readRepoText' : /listRepoDir = /.test(l) ? 'listRepoDir'
    : /--view \$\{file\}/.test(l) ? '--view file' : /\.view\.json/.test(l) ? '--view directory' : /fs\.readFileSync\(abs\)/.test(l) ? 'page file' : l.trim()));
  expect(kinds.sort()).toEqual(['--view directory', '--view file', 'listRepoDir', 'page file', 'readRepoJson', 'readRepoText'].sort());
});
