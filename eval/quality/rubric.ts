// The rubric run for one blinded product: the same steps, timings and inputs for every product.
// Run inside the network namespace of netns.ts (loopback plus a dummy default route):
//   slot node netns.ts rubric.ts <id>
// Writes results/<id>.json and screenshots results/<id>-*.png.
import {chromium} from 'playwright';
import type {Browser, BrowserContext, Page} from 'playwright';
import fs from 'node:fs';
import path from 'node:path';
import {Running, freePort, resetData, Q, START} from './products.ts';
import {ADAPTERS, item} from './adapters.ts';

const id = process.argv[2];
const ad = ADAPTERS[id];
const out = path.join(Q, 'results');
fs.mkdirSync(out, {recursive: true});
type Score = 'pass' | 'fail' | 'not testable';
const res: Record<string, {score: Score; step: string; got: string}> = {};
const consoleErrors: string[] = [];
function rec(k: string, score: Score, step: string, got: string) {
  got = got.split(Q).join('<quality>').split(process.env.HOME ?? '~').join('~');
  res[k] = {score, step, got: got.slice(0, 400)};
  console.log(k.padEnd(3), score.padEnd(12), '|', step, '|', got.slice(0, 160));
}
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
async function visible(page: Page, title: string, ms = 5000): Promise<boolean> {
  try { await item(page, title).first().waitFor({state: 'visible', timeout: ms}); return true; } catch { return false; }
}
async function gone(page: Page, title: string, ms = 5000): Promise<boolean> {
  try { await item(page, title).first().waitFor({state: 'detached', timeout: ms}); return true; } catch { return false; }
}
async function bodyText(page: Page): Promise<string> {
  try { return ((await page.locator('body').innerText({timeout: 2000})) ?? '').replace(/\s+/g, ' ').slice(0, 200); }
  catch (e) { return 'no body: ' + String(e).slice(0, 80); }
}
async function count(page: Page, title: string) { return item(page, title).count(); }

const build = JSON.parse(fs.readFileSync(path.join(out, `build-${id}.json`), 'utf8'));
for (const k of ['B1', 'B2']) res[k] = build[k];

resetData(id);
const r = new Running(id, await freePort(), await freePort());
const browser: Browser = await chromium.launch();
const ctxs: BrowserContext[] = [];
async function newCtx(who: string): Promise<{ctx: BrowserContext; page: Page}> {
  const ctx = await browser.newContext({viewport: {width: 900, height: 900}});
  ctxs.push(ctx);
  const page = await ctx.newPage();
  page.on('console', m => { if (m.type() === 'error') consoleErrors.push(`${who}: ${m.text().slice(0, 160)}`); });
  page.on('pageerror', e => consoleErrors.push(`${who} pageerror: ${String(e).slice(0, 160)}`));
  return {ctx, page};
}
async function load(page: Page, url: string, who: string) {
  await page.goto(url, {timeout: 10000});
  await ad.gate(page, who);
  await sleep(800);
}
async function reload(page: Page, who: string) {
  await page.reload({timeout: 10000});
  await ad.gate(page, who);
  await sleep(800);
}
const unavailable = 'the page offers no control for it';

try {
  // B3
  const s = await r.start();
  rec('B3', s.ok ? 'pass' : 'fail', START[id].readme + ' (binary of B1, in confinement, 127.0.0.1:<free port>)',
    s.ok ? `GET / answered ${s.status} after ${s.ms} ms; stdout: ${r.out.trim().split('\n')[0]}` : `no HTTP answer within 15 s; output: ${r.out.slice(0, 200)}`);
  if (!s.ok) throw new Error('not started');

  const A = await newCtx('A');
  // F1 + U1
  let addr = '';
  try {
    addr = await ad.address(r, A.page);
    await load(A.page, addr, 'A');
    const ct = await A.page.evaluate(() => document.contentType);
    const ctl = await A.page.locator(ad.addInput).isVisible();
    rec('U1', ct === 'text/html' && ctl ? 'pass' : 'fail', `open ${addr.replace(r.base, '')} in Chromium`,
      `content type ${ct}, add control ${ad.addInput} visible: ${ctl}; text: ${await bodyText(A.page)}`);
    rec('F1', ctl ? 'pass' : 'fail', ad.listKind === 'explicit' ? 'create a list in the first page\'s form, open its address' : 'open the address',
      ad.listKind === 'explicit' ? `list created, address ${addr.replace(r.base, '')}` : 'one shared list, usable on the first page without setup');
  } catch (e) {
    rec('U1', 'fail', 'open the address in Chromium', String(e).slice(0, 200));
    rec('F1', 'fail', 'get a list', String(e).slice(0, 200));
    throw e;
  }
  await A.page.screenshot({path: path.join(out, `${id}-main-empty.png`)});

  // F2
  await ad.add(A.page, 'Buy milk', 'A');
  rec('F2', (await visible(A.page, 'Buy milk')) ? 'pass' : 'fail', 'type "Buy milk", submit', `shown: ${await visible(A.page, 'Buy milk', 100)}`);
  // F3
  if (ad.edit) {
    try {
      await ad.edit(A.page, 'Buy milk', 'Buy oat drink');
      await sleep(1500);
      await reload(A.page, 'A');
      const ok = await visible(A.page, 'Buy oat drink');
      rec('F3', ok && (await count(A.page, 'Buy milk')) === 0 ? 'pass' : 'fail', 'Edit "Buy milk" to "Buy oat drink", reload',
        `after reload: ${await bodyText(A.page)}`);
    } catch (e) { rec('F3', 'fail', 'Edit "Buy milk"', String(e).slice(0, 200)); }
  } else rec('F3', 'fail', 'edit a task title', unavailable);
  // F4
  await ad.add(A.page, 'Pay rent', 'A');
  await visible(A.page, 'Pay rent');
  if (ad.tick) {
    try {
      await ad.tick(A.page, 'Pay rent');
      await sleep(1500);
      await reload(A.page, 'A');
      const d = await visible(A.page, 'Pay rent') && await ad.isDone(A.page, 'Pay rent');
      rec('F4', d ? 'pass' : 'fail', 'tick "Pay rent" done, reload', `done after reload: ${d}`);
    } catch (e) { rec('F4', 'fail', 'tick "Pay rent"', String(e).slice(0, 200)); }
  } else rec('F4', 'fail', 'tick a task', unavailable);
  // F5
  await ad.add(A.page, 'Remove me', 'A');
  await visible(A.page, 'Remove me');
  if (ad.remove) {
    try {
      await ad.remove(A.page, 'Remove me');
      await sleep(1500);
      await reload(A.page, 'A');
      const g = await gone(A.page, 'Remove me', 3000);
      rec('F5', g ? 'pass' : 'fail', 'remove "Remove me", reload', `gone after reload: ${g}`);
    } catch (e) { rec('F5', 'fail', 'remove "Remove me"', String(e).slice(0, 200)); }
  } else rec('F5', 'fail', 'remove a task', unavailable);
  await A.page.screenshot({path: path.join(out, `${id}-main.png`)});

  // S1
  const B = await newCtx('B');
  await load(B.page, addr, 'B');
  const s1 = await visible(B.page, 'Pay rent');
  rec('S1', s1 ? 'pass' : 'fail', 'second context B opens the same address', `B shows "Pay rent": ${s1}; text: ${await bodyText(B.page)}`);
  // S2
  await ad.add(A.page, 'Bread from A', 'A');
  await visible(A.page, 'Bread from A');
  await sleep(1500);
  await reload(B.page, 'B');
  const s2a = await visible(B.page, 'Bread from A');
  await ad.add(B.page, 'Soap from B', 'B');
  await visible(B.page, 'Soap from B');
  await sleep(1500);
  await reload(A.page, 'A');
  const s2b = await visible(A.page, 'Soap from B');
  rec('S2', s2a && s2b ? 'pass' : 'fail', 'A adds "Bread from A", B reloads; B adds "Soap from B", A reloads',
    `B sees A's: ${s2a}; A sees B's: ${s2b}`);
  // S3
  await ad.add(A.page, 'Live from A', 'A');
  const t3 = Date.now();
  const s3 = await visible(B.page, 'Live from A', 10000);
  rec('S3', s3 ? 'pass' : 'fail', 'A adds "Live from A"; B waits 10 s without reload', s3 ? `shown in B after ${Date.now() - t3} ms` : 'not shown in B within 10 s');

  // P1
  await sleep(1500);
  await r.stop();
  const s2r = await r.start();
  const C = await newCtx('C');
  let p1 = false;
  try { await load(C.page, addr, 'C'); p1 = await visible(C.page, 'Pay rent') && await visible(C.page, 'Soap from B', 1000); } catch {}
  rec('P1', s2r.ok && p1 ? 'pass' : 'fail', 'stop the server, start it again with the same command and data file, open in a fresh context C',
    `restarted: ${s2r.ok}; C shows "Pay rent" and "Soap from B": ${p1}; text: ${await bodyText(C.page)}`);
  await C.ctx.close();
  await reload(A.page, 'A');
  await reload(B.page, 'B');

  // O1
  await A.ctx.setOffline(true);
  await sleep(5000);
  const o1 = await visible(A.page, 'Pay rent', 1000);
  rec('O1', o1 ? 'pass' : 'fail', 'A context offline, wait 5 s', `"Pay rent" still shown: ${o1}; text: ${await bodyText(A.page)}`);
  // O4 (add) and O6 (tick) while offline
  let o4a = false, o6a = false;
  try { await ad.add(A.page, 'Tickets offline', 'A'); o4a = await visible(A.page, 'Tickets offline', 3000); } catch (e) { }
  if (ad.tick) {
    try { await ad.tick(A.page, 'Soap from B'); await sleep(500); o6a = await ad.isDone(A.page, 'Soap from B'); } catch (e) { }
  }
  await A.page.screenshot({path: path.join(out, `${id}-offline.png`)});
  // O3
  let o3 = false, o3got = '';
  try {
    await A.page.reload({timeout: 10000});
    await ad.gate(A.page, 'A');
    await sleep(1500);
    o3 = await visible(A.page, 'Pay rent', 2000);
    o3got = `after offline reload: ${await bodyText(A.page)}`;
  } catch (e) { o3got = 'reload failed: ' + String(e).slice(0, 150); }
  rec('O3', o3 ? 'pass' : 'fail', 'reload A while offline', o3got);
  await A.page.screenshot({path: path.join(out, `${id}-offline-reload.png`)});
  // back online
  await A.ctx.setOffline(false);
  if (!o3) { try { await reload(A.page, 'A'); } catch {} }
  let o4 = false;
  if (o4a) o4 = await visible(A.page, 'Tickets offline', 15000);
  rec('O4', o4a && o4 ? 'pass' : 'fail', 'A offline: add "Tickets offline"; then A online', `shown offline: ${o4a}; still shown after going online: ${o4}`);
  await sleep(15000);
  await reload(B.page, 'B');
  const o5 = await visible(B.page, 'Tickets offline', 3000);
  rec('O5', o4a && o5 ? 'pass' : 'fail', 'after A reconnects (15 s), B reloads', `B shows "Tickets offline": ${o5}`);
  if (ad.tick) {
    let o6 = false;
    try { o6 = await visible(B.page, 'Soap from B', 1000) && await ad.isDone(B.page, 'Soap from B'); } catch {}
    rec('O6', o6a && o6 ? 'pass' : 'fail', 'A offline: tick "Soap from B"; A reconnects; B reloads', `done in A offline: ${o6a}; done in B: ${o6}`);
  } else rec('O6', 'fail', 'tick offline', unavailable);

  // O2
  await reload(A.page, 'A');
  await r.stop();
  await sleep(5000);
  const o2 = await visible(A.page, 'Pay rent', 1000);
  rec('O2', o2 ? 'pass' : 'fail', 'stop the server process, wait 5 s on the loaded page A', `"Pay rent" still shown: ${o2}; text: ${await bodyText(A.page)}`);
  await A.page.screenshot({path: path.join(out, `${id}-server-stopped.png`)});
  await r.start();
  await sleep(1000);
  await reload(A.page, 'A');
  await reload(B.page, 'B');

  // O7
  if (ad.edit) {
    try {
      await ad.add(A.page, 'Shared chore', 'A');
      await visible(A.page, 'Shared chore');
      await sleep(2000);
      await reload(B.page, 'B');
      if (!(await visible(B.page, 'Shared chore'))) throw new Error('B never saw "Shared chore"');
      await A.ctx.setOffline(true); await B.ctx.setOffline(true);
      await sleep(1000);
      await ad.edit(A.page, 'Shared chore', 'Chore by A'); await sleep(300);
      await ad.edit(B.page, 'Shared chore', 'Chore by B'); await sleep(300);
      await A.ctx.setOffline(false); await sleep(6000);
      await B.ctx.setOffline(false); await sleep(15000);
      await reload(A.page, 'A'); await reload(B.page, 'B');
      const ta = await A.page.locator('li', {hasText: /Chore by [AB]|Shared chore/}).allInnerTexts();
      const tb = await B.page.locator('li', {hasText: /Chore by [AB]|Shared chore/}).allInnerTexts();
      const norm = (xs: string[]) => xs.map(x => (x.match(/Chore by [AB]|Shared chore/) || [''])[0]).sort().join(',');
      const keep = await visible(A.page, 'Pay rent', 1000) && await visible(B.page, 'Pay rent', 1000)
        && await visible(A.page, 'Bread from A', 1000) && await visible(B.page, 'Bread from A', 1000);
      const same = norm(ta) === norm(tb) && ta.length === 1 && tb.length === 1;
      rec('O7', same && keep ? 'pass' : 'fail', 'A and B offline edit "Shared chore" to "Chore by A"/"Chore by B"; A reconnects, 6 s later B; 15 s; both reload',
        `A: [${norm(ta)}] B: [${norm(tb)}]; other tasks kept in both: ${keep}`);
    } catch (e) { rec('O7', 'fail', 'conflicting offline edits', String(e).slice(0, 200)); }
  } else rec('O7', 'fail', 'conflicting offline edits', 'the page offers no edit control');

  // R1
  try {
    const before = await A.page.locator('li').count();
    await A.page.locator(ad.addInput).fill('');
    await A.page.locator(ad.addInput).press('Enter');
    await A.page.locator(ad.addInput).fill('   ');
    await A.page.locator(ad.addInput).press('Enter');
    await sleep(2000);
    const after = await A.page.locator('li').count();
    await ad.add(A.page, 'After empty', 'A');
    const works = await visible(A.page, 'After empty');
    rec('R1', after === before && works ? 'pass' : 'fail', 'submit "" and "   ", then add "After empty"', `items before ${before}, after ${after}; page still adds: ${works}`);
  } catch (e) { rec('R1', 'fail', 'submit empty titles', String(e).slice(0, 200)); }
  // R2
  try {
    const before = await A.page.locator('li').count();
    await ad.add(A.page, 'L'.repeat(10000), 'A');
    await sleep(3000);
    const alive = await fetch(r.base + '/', {signal: AbortSignal.timeout(3000)}).then(x => x.status, () => 0);
    await reload(A.page, 'A');
    const lists = await visible(A.page, 'Pay rent', 3000);
    const after = await A.page.locator('li').count();
    rec('R2', alive > 0 && alive < 500 && lists ? 'pass' : 'fail', 'add a 10 000-character title, reload',
      `server answers GET / with ${alive}; page lists tasks: ${lists}; items ${before} -> ${after}`);
  } catch (e) { rec('R2', 'fail', 'long title', String(e).slice(0, 200)); }
  // R3
  try {
    const x = '<img src=x onerror="window.__xss=1">';
    await ad.add(A.page, x, 'A');
    await sleep(2500);
    const shown = await A.page.locator('li', {hasText: '<img src=x'}).count();
    const xa = await A.page.evaluate(() => (window as any).__xss);
    await reload(B.page, 'B');
    await sleep(1000);
    const xb = await B.page.evaluate(() => (window as any).__xss);
    const shownB = await B.page.locator('li', {hasText: '<img src=x'}).count();
    rec('R3', xa === undefined && xb === undefined && shown > 0 ? 'pass' : 'fail', `add title ${x}; B reloads`,
      `shown as text in A: ${shown > 0}, in B: ${shownB > 0}; window.__xss A=${xa} B=${xb}`);
  } catch (e) { rec('R3', 'fail', 'markup title', String(e).slice(0, 200)); }
} catch (e) {
  console.log('aborted:', String(e).slice(0, 300));
}
const items = ['B1', 'B2', 'B3', 'U1', 'F1', 'F2', 'F3', 'F4', 'F5', 'P1', 'S1', 'S2', 'S3', 'O1', 'O2', 'O3', 'O4', 'O5', 'O6', 'O7', 'R1', 'R2', 'R3'];
for (const k of items) if (!res[k]) res[k] = {score: 'fail', step: 'not reached', got: 'the run stopped before this item (product not running or page unusable)'};
const pass = items.filter(k => res[k].score === 'pass').length;
const nt = items.filter(k => res[k].score === 'not testable').length;
fs.writeFileSync(path.join(out, `${id}.json`), JSON.stringify({
  id, score: pass, n: items.length, not_testable: nt, tests: build.tests,
  failed: items.filter(k => res[k].score === 'fail'), items: Object.fromEntries(items.map(k => [k, res[k]])),
  console_errors: [...new Set(consoleErrors)].slice(0, 30),
}, null, 2) + '\n');
console.log(`${id}: ${pass}/${items.length}, not testable ${nt}`);
for (const c of ctxs) await c.close().catch(() => {});
await browser.close();
await r.stop();
