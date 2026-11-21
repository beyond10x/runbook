// Exploratory: for a product whose README's first run command starts a different binary than the
// one the rubric uses, start that first command and look at its page and whether it lists, adds
// and shares tasks. Diagnostic only.
//   slot node netns.ts readme-first.ts <id> <binary> [args with {port} {data}]
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {Q, freePort} from './products.ts';
import {bwrapArgs} from './confine.ts';
const [id, bin, ...rest] = process.argv.slice(2);
const dir = path.join(Q, 'products', id), data = path.join(dir, 'data-first');
fs.rmSync(data, {recursive: true, force: true}); fs.mkdirSync(data, {recursive: true});
const port = await freePort();
const args = rest.map(a => a.replace('{port}', String(port)).replace('{data}', data));
const c = spawn('/usr/bin/bwrap', bwrapArgs('serve', dir, path.join(Q, 'cargo-home', id), [path.join(dir, bin), ...args]), {env: {}, stdio: 'ignore'});
await new Promise(r => setTimeout(r, 800));
const b = await chromium.launch();
const p = await b.newPage();
await p.goto(`http://127.0.0.1:${port}/`);
await p.waitForTimeout(1500);
console.log('page text:', (await p.locator('body').innerText()).replace(/\s+/g, ' ').slice(0, 300));
const input = p.locator('input[type=text], input:not([type])').first();
if (await input.count()) {
  await input.fill('First-command task'); await input.press('Enter'); await p.waitForTimeout(2500);
  await p.reload(); await p.waitForTimeout(1500);
  console.log('after add + reload:', (await p.locator('body').innerText()).replace(/\s+/g, ' ').slice(0, 300));
  const p2 = await (await b.newContext()).newPage();
  await p2.goto(`http://127.0.0.1:${port}/`); await p2.waitForTimeout(2500);
  console.log('second context:', (await p2.locator('body').innerText()).replace(/\s+/g, ' ').slice(0, 300));
}
for (const u of ['/api/shared', '/api/tasks']) console.log(u, await fetch(`http://127.0.0.1:${port}${u}`).then(async r => r.status + ' ' + (await r.text()).slice(0, 200)));
await b.close(); c.kill('SIGKILL');
fs.rmSync(data, {recursive: true, force: true});
