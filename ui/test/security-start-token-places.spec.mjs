// Security review of the unit daemon-page-starts-a-run: where the token of the start form can be found
// after a start, a refused start, a 401, each way the form closes and a reload: the cookies, the storages, the
// URL, the history state, the DOM (attributes, text and values) and what the page's global objects reach.
// The shared daemon of page-daemon.mjs with a token (stub runtime, floor linux); each test counts the runs it
// adds. Invented texts; no model call; no request beyond 127.0.0.1.
import {test, expect as plain} from '@playwright/test';
import {pageDaemon, TOKEN} from './page-daemon.mjs';

const expect = plain.configure({timeout: 15000});
const SUBMIT = '**/daemon/commands/submit-run';
const RUNBOOKS = '**/daemon/views/runbooks';
const TYPED = 'typed-and-not-sent-9876543210-9876543210';
let s, n0;

const runs = () => fetch(`${s.base}/daemon/views/runs`, {headers: {authorization: `Bearer ${TOKEN}`}}).then((r) => r.json()).then((j) => j.rows);
const added = async () => (await runs()).length - n0;
const isStart = (r) => r.method() === 'POST' && new URL(r.url()).pathname === '/daemon/commands/submit-run';

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon: one project is enough'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  s = await pageDaemon('token');
});
test.beforeEach(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  n0 = (await runs()).length;
});

async function enter(page) {
  await page.goto(`${s.base}/`);
  await page.locator('input[name=token]').fill(TOKEN);
  await page.getByRole('button', {name: 'Open'}).click();
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
}

async function form(page) {
  await page.click('#startbtn');
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
  await expect.poll(() => page.locator('#st-runbook option').count()).toBeGreaterThan(0);
  await expect(page.locator('#st-intake')).toBeFocused();
  await page.locator('#st-runbook').selectOption('sdc');
  await page.locator('#st-confinement').selectOption('linux');
}

// every place of the page and of the browser's stores for this origin that holds the needle
async function places(page, context, needle) {
  const found = await page.evaluate(async (n) => {
    const hit = [];
    const has = (v) => typeof v === 'string' && v.includes(n);
    if (has(document.cookie)) hit.push('document.cookie');
    for (const [name, store] of [['localStorage', localStorage], ['sessionStorage', sessionStorage]]) {
      for (let i = 0; i < store.length; i++) if (has(store.key(i)) || has(store.getItem(store.key(i)))) hit.push(`${name}.${store.key(i)}`);
    }
    if (has(location.href)) hit.push('location');
    if (has(document.referrer)) hit.push('referrer');
    if (has(window.name)) hit.push('window.name');
    if (has(document.title)) hit.push('title');
    try { if (has(JSON.stringify(history.state))) hit.push('history.state'); } catch { hit.push('history.state unreadable'); }
    if (has(document.documentElement.outerHTML)) hit.push('markup (attribute or text)');
    for (const el of document.querySelectorAll('*')) {
      if (has(el.value)) hit.push(`value of ${el.tagName.toLowerCase()}#${el.id}`);
      if (el.dataset) for (const [k, v] of Object.entries(el.dataset)) if (has(v)) hit.push(`data-${k} of #${el.id}`);
      if (el.shadowRoot && has(el.shadowRoot.innerHTML)) hit.push(`shadow of #${el.id}`);
    }
    for (const e of performance.getEntries()) if (has(e.name)) hit.push(`performance entry ${e.entryType}`);
    try { for (const db of await indexedDB.databases()) if (has(db.name)) hit.push('indexedDB'); } catch { /* none */ }
    try { for (const k of await caches.keys()) if (has(k)) hit.push('caches'); } catch { /* none */ }
    // what the global object reaches: its own properties, three levels deep, strings only
    const seen = new Set();
    let budget = 60000;
    const walk = (v, where, depth) => {
      if (budget-- <= 0) return;
      if (typeof v === 'string') { if (has(v)) hit.push(`global ${where}`); return; }
      if (v === null || (typeof v !== 'object' && typeof v !== 'function') || depth === 0 || seen.has(v)) return;
      if (v instanceof Node || v === window.document) return;
      seen.add(v);
      let names = [];
      try { names = Object.getOwnPropertyNames(v); } catch { return; }
      for (const k of names) {
        let child;
        try { child = v[k]; } catch { continue; }
        walk(child, `${where}.${k}`, depth - 1);
      }
    };
    walk(window, 'window', 3);
    return hit;
  }, needle);
  for (const c of await context.cookies()) if (c.value.includes(needle) || c.name.includes(needle)) found.push(`cookie ${c.name}`);
  const state = JSON.stringify(await context.storageState());
  if (state.includes(needle)) found.push('storage state of the browser context');
  return found;
}

test('the token is in no cookie, storage, URL, history state, DOM place or global of the page after a start, a refused start, a 401, each close and a reload', async ({page, context}) => {
  const sent = [];
  const urls = [];
  const lines = [];
  page.on('request', (r) => { urls.push(r.url()); if (isStart(r)) sent.push(r); });
  page.on('console', (m) => lines.push(m.text()));
  await enter(page);
  const nowhere = async (when, needle = TOKEN) => expect(await places(page, context, needle), `after ${when}`).toEqual([]);
  await nowhere('the login');

  // a start
  await form(page);
  await page.locator('#st-intake').fill('an invented intake started with the token');
  await page.locator('#st-token').fill(TOKEN);
  const [ok] = await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  expect(ok.status()).toBe(202);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  await nowhere('a start');

  // a start the daemon refuses by name: the token stays in memory and is asked for no more
  await form(page);
  await expect(page.locator('#st-token')).toBeHidden();
  await page.locator('#st-intake').evaluate((el) => { el.value = 'a'.repeat(1048577); });
  const [refused] = await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  expect((await refused.json()).outcome).toBe('intake-too-large');
  expect((await sent.at(-1).allHeaders()).authorization).toBe(`Bearer ${TOKEN}`);
  await nowhere('a refused start');
  await page.locator('#st-intake').fill('short again');

  // a 401: the token is forgotten and asked for again
  let once = true;
  await page.route(SUBMIT, (route) => { if (!once) return route.continue(); once = false; return route.fulfill({status: 401, json: {refused: 'unauthorized', message: 'a bearer token is required'}}); });
  await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  await expect(page.locator('#st-token')).toBeVisible();
  await expect(page.locator('#st-token')).toHaveValue('');
  await nowhere('a 401');
  const before = sent.length;
  await page.click('#st-start');
  await expect(page.locator('#st-msg')).toContainText('needs its token');
  expect(sent.length, 'a start left the page with the forgotten token').toBe(before);

  // each way the form closes, with a token typed and not sent
  const ways = [
    ['Close', () => page.click('#st-close')],
    ['Escape', () => page.keyboard.press('Escape')],
    ['close() by script', () => page.locator('#startdlg').evaluate((d) => d.close())],
    ['requestClose() by script', () => page.locator('#startdlg').evaluate((d) => (d.requestClose ? d.requestClose() : d.close()))],
  ];
  for (const [how, close] of ways) {
    if (!(await page.locator('#startdlg').evaluate((d) => d.open))) await form(page);
    await page.locator('#st-token').fill(TYPED);
    await close();
    await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
    await expect.poll(() => places(page, context, TYPED), {message: `after ${how}`}).toEqual([]);
    await nowhere(how);
  }

  // closed by a started run: the right token, typed after the 401
  await form(page);
  await page.locator('#st-intake').fill('an invented intake started after the 401');
  await page.locator('#st-token').fill(TOKEN);
  const [again] = await Promise.all([page.waitForResponse((r) => isStart(r.request())), page.click('#st-start')]);
  expect(again.status()).toBe(202);
  await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
  await nowhere('a started run that closed the form');

  // a reload: the page asks again, and the token is nowhere
  await page.reload();
  await expect(page.locator('#live')).toHaveClass(/\bon\b/);
  await nowhere('a reload');
  await form(page);
  await expect(page.locator('#st-token')).toBeVisible();
  await expect(page.locator('#st-token')).toHaveValue('');
  await nowhere('a reload and a new opening');

  expect(urls.filter((u) => u.includes(TOKEN) || u.includes(TYPED))).toEqual([]);
  expect(lines.filter((l) => l.includes(TOKEN) || l.includes(TYPED))).toEqual([]);
  expect(s.log().includes(TYPED)).toBe(false);
  expect(await added()).toBe(2);
});

test('a token typed while the form still reads, and the form closed before the second paint: the late paint puts nothing back', async ({page, context}) => {
  await enter(page);
  await form(page);
  await page.click('#st-close');
  for (const [how, close] of [['Close', () => page.click('#st-close')], ['Escape', () => page.keyboard.press('Escape')]]) {
    // the list of runbooks is held until the form is closed: its answer, the second paint, comes after the close
    let release;
    const held = new Promise((r) => { release = r; });
    let asked;
    const reading = new Promise((r) => { asked = r; });
    await page.unroute(RUNBOOKS).catch(() => {});
    await page.route(RUNBOOKS, async (route) => { asked(); await held; return route.continue(); });
    await page.click('#startbtn');
    await expect(page.locator('#startdlg')).toHaveJSProperty('open', true);
    await page.locator('#st-token').fill(TYPED);
    await reading;
    await close();
    await expect(page.locator('#startdlg')).toHaveJSProperty('open', false);
    const answered = page.waitForEvent('requestfinished', (r) => new URL(r.url()).pathname === '/daemon/views/runbooks');
    release();
    await answered;
    // the page has the answer: two frames later its continuation and any paint it makes have run
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    await expect(page.locator('#startdlg'), `${how}: the late paint opened the form`).toHaveJSProperty('open', false);
    expect(await places(page, context, TYPED), `after ${how} and the late paint`).toEqual([]);
  }
  expect(await added()).toBe(0);
});
