// Exploratory probe on a slow connection (the train): every /api/ request is delayed by 2 s.
// The user adds "First", then types "Second" while the first add is still being sent, waits, and
// presses Enter. Checks that neither text is lost. Same steps for every product.
//   slot node netns.ts slow.ts <id>
import {chromium} from 'playwright';
import fs from 'node:fs';
import path from 'node:path';
import {Running, freePort, resetData, Q} from './products.ts';
import {ADAPTERS, item} from './adapters.ts';

const id = process.argv[2];
const ad = ADAPTERS[id];
resetData(id);
const r = new Running(id, await freePort(), await freePort());
await r.start();
const b = await chromium.launch();
const c = await b.newContext();
const p = await c.newPage();
const addr = await ad.address(r, p);
await p.goto(addr); await ad.gate(p, 'A'); await p.waitForTimeout(1000);
await c.route('**/api/**', async route => { await new Promise(x => setTimeout(x, 2000)); await route.continue().catch(() => {}); });
await ad.add(p, 'First', 'A');
await p.waitForTimeout(300);
if (id === 'p7') await p.locator('#who').fill('A');
await p.locator(ad.addInput).pressSequentially('Second', {delay: 60});
const typed = await p.locator(ad.addInput).inputValue();
await p.waitForTimeout(3000);
const before = await p.locator(ad.addInput).inputValue();
await p.locator(ad.addInput).press('Enter');
await p.waitForTimeout(6000);
await c.unroute('**/api/**');
await p.waitForTimeout(6000);
const c2 = await b.newContext(); const p2 = await c2.newPage();
await p2.goto(addr); await ad.gate(p2, 'B'); await p2.waitForTimeout(2000);
const res = {id, typed_into_field: typed, field_before_enter: before,
  first_in_A: await item(p, 'First').count(), second_in_A: await item(p, 'Second').count(),
  first_in_B: await item(p2, 'First').count(), second_in_B: await item(p2, 'Second').count()};
console.log(JSON.stringify(res));
fs.writeFileSync(path.join(Q, 'results', `slow-${id}.json`), JSON.stringify(res, null, 2) + '\n');
await b.close(); await r.stop();
