// Exploratory probes (not the rubric): things a user does beyond the rubric's script. Each probe
// logs what it did and what came back; defects are picked from this log by reading it.
//   slot node netns.ts explore.ts <id>
import {chromium} from 'playwright';
import type {Page, BrowserContext} from 'playwright';
import fs from 'node:fs';
import path from 'node:path';
import {Running, freePort, resetData, Q} from './products.ts';
import {ADAPTERS, item} from './adapters.ts';

const id = process.argv[2];
const ad = ADAPTERS[id];
const out = path.join(Q, 'results');
const log: {probe: string; step: string; got: string}[] = [];
const errs: string[] = [];
function note(probe: string, step: string, got: string) {
  got = got.split(Q).join('<quality>');
  log.push({probe, step, got: got.slice(0, 500)});
  console.log(probe, '|', step, '|', got.slice(0, 200));
}
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
async function visible(page: Page, t: string, ms = 4000) {
  try { await item(page, t).first().waitFor({state: 'visible', timeout: ms}); return true; } catch { return false; }
}
async function texts(page: Page) { return (await page.locator('li').allInnerTexts()).map(s => s.replace(/\s+/g, ' ').trim()); }

resetData(id);
const r = new Running(id, await freePort(), await freePort());
await r.start();
const browser = await chromium.launch();
async function ctx(who: string): Promise<{c: BrowserContext; p: Page}> {
  const c = await browser.newContext({viewport: {width: 900, height: 900}});
  const p = await c.newPage();
  p.on('console', m => { if (m.type() === 'error') errs.push(`${who}: ${m.text().slice(0, 200)}`); });
  p.on('pageerror', e => errs.push(`${who} pageerror: ${String(e).slice(0, 200)}`));
  return {c, p};
}
async function open(p: Page, url: string, who: string) { await p.goto(url); await ad.gate(p, who); await sleep(800); }
async function reload(p: Page, who: string) { await p.reload(); await ad.gate(p, who); await sleep(800); }

try {
  const A = await ctx('A');
  const addr = await ad.address(r, A.p);
  await open(A.p, addr, 'A');
  const B = await ctx('B');
  await open(B.p, addr, 'B');

  // X1 special characters survive a round trip and a restart
  const special = ['Käse 🧀 & Brot', '50% + tax & fees', 'tab\there', 'quote " and \\ back'];
  for (const t of special) { await ad.add(A.p, t, 'A'); await sleep(400); }
  await sleep(2500);
  await r.stop(); await r.start();
  const C = await ctx('C');
  await open(C.p, addr, 'C');
  await sleep(1500);
  note('X1 special characters', `A adds ${JSON.stringify(special)}; server restarted; fresh context C opens`, `C lists: ${JSON.stringify(await texts(C.p))}`);
  await C.c.close();

  // X2 duplicate titles
  await ad.add(A.p, 'Eggs', 'A'); await sleep(600); await ad.add(A.p, 'Eggs', 'A'); await sleep(2500);
  await reload(B.p, 'B');
  note('X2 duplicate titles', 'A adds "Eggs" twice; B reloads', `A shows ${await item(A.p, 'Eggs').count()}, B shows ${await item(B.p, 'Eggs').count()}`);

  // X3 rapid adds
  for (let i = 1; i <= 5; i++) { await ad.add(A.p, `Quick ${i}`, 'A'); }
  await sleep(4000);
  const D = await ctx('D');
  await open(D.p, addr, 'D'); await sleep(1500);
  const quick = [];
  for (let i = 1; i <= 5; i++) quick.push(await item(D.p, `Quick ${i}`).count());
  note('X3 five quick adds', 'A adds "Quick 1".."Quick 5" without pause; fresh context D opens after 4 s', `D shows counts ${JSON.stringify(quick)}`);
  await D.c.close();

  // X4 one partner removes while the other ticks (online)
  await ad.add(A.p, 'Contested', 'A'); await sleep(2500); await reload(B.p, 'B');
  if (ad.remove && ad.tick && await visible(B.p, 'Contested')) {
    await ad.remove(A.p, 'Contested'); await sleep(200);
    try { await ad.tick(B.p, 'Contested'); } catch (e) { note('X4', 'B ticks', 'error ' + String(e).slice(0, 100)); }
    await sleep(3000); await reload(A.p, 'A'); await reload(B.p, 'B');
    note('X4 remove vs tick', 'A removes "Contested"; B (not reloaded) ticks it; both reload',
      `A shows ${await item(A.p, 'Contested').count()}, B shows ${await item(B.p, 'Contested').count()}; B text: ${(await B.p.locator('body').innerText()).replace(/\s+/g, ' ').slice(0, 160)}`);
  } else note('X4 remove vs tick', 'needs remove and tick', `remove: ${!!ad.remove}, tick: ${!!ad.tick}`);

  // X5 offline add, tab closed before reconnect, reopened later online
  const E = await ctx('A2');
  await open(E.p, addr, 'A');
  await E.c.setOffline(true); await sleep(1000);
  await ad.add(E.p, 'Closed tab task', 'A'); await sleep(1000);
  const shownOffline = await visible(E.p, 'Closed tab task', 2000);
  await E.p.close();
  await E.c.setOffline(false);
  const E2 = await E.c.newPage();
  E2.on('pageerror', e => errs.push(`A2 pageerror: ${String(e).slice(0, 200)}`));
  await open(E2, addr, 'A'); await sleep(8000);
  await reload(B.p, 'B');
  note('X5 offline task, tab closed, reopened online', 'context A2 offline adds "Closed tab task", closes the tab, goes online, opens the page again, waits 8 s; B reloads',
    `shown offline: ${shownOffline}; B shows: ${await visible(B.p, 'Closed tab task', 2000)}`);
  await E.c.close();

  // X6 server down while browser online: add, then server back
  await r.stop();
  await ad.add(A.p, 'While server down', 'A'); await sleep(1500);
  const kept = await visible(A.p, 'While server down', 1500);
  const msg = (await A.p.locator('body').innerText()).replace(/\s+/g, ' ').slice(0, 200);
  await r.start(); await sleep(16000);
  await reload(B.p, 'B');
  note('X6 add while server down', 'server stopped, A (online) adds "While server down"; server started; 16 s; B reloads',
    `kept in A: ${kept}; A text: ${msg}; B shows: ${await visible(B.p, 'While server down', 2000)}`);

  // X7 first open while offline (never visited)
  const F = await ctx('F');
  await F.c.setOffline(true);
  let first = '';
  try { await F.p.goto(addr, {timeout: 8000}); first = (await F.p.locator('body').innerText()).slice(0, 120); } catch (e) { first = 'error ' + String(e).slice(0, 80); }
  note('X7 first open offline', 'a context that never opened the app opens it offline', first.replace(/\s+/g, ' '));
  await F.c.close();

  // X8 edit to empty and tick twice
  if (ad.edit) {
    await ad.add(A.p, 'Edit me empty', 'A'); await sleep(1500);
    try { await ad.edit(A.p, 'Edit me empty', '   '); } catch (e) {}
    await sleep(1500); await reload(A.p, 'A');
    note('X8 edit to blank', 'A edits "Edit me empty" to "   ", reload', `items: ${JSON.stringify((await texts(A.p)).filter(t => /Edit me|^\s*\(|^$/.test(t)).slice(0, 5))}; text: ${(await A.p.locator('body').innerText()).replace(/\s+/g, ' ').slice(0, 200)}`);
  }
  await A.p.screenshot({path: path.join(out, `${id}-explore.png`), fullPage: true});
  note('X9 final list', 'A after all probes', JSON.stringify(await texts(A.p)).slice(0, 480));
} catch (e) { note('aborted', '', String(e).slice(0, 300)); }
fs.writeFileSync(path.join(out, `explore-${id}.json`), JSON.stringify({id, log, console_errors: [...new Set(errs)].slice(0, 30)}, null, 2) + '\n');
await browser.close();
await r.stop();
