// Independent review, invariant 4 of story inspector-shows-step-products: the content of a run never becomes markup or
// code of the page. File names, evidence records, the output of an action, JSON, YAML and Markdown links written
// to look like markup or like a script are opened in the page; after each the DOM is audited (no element that loads or
// runs, no handler or source attribute, no style but the one the page sets, no link that is not http, https or the
// page's own #file=), nothing ran, and nothing was asked of an address that is not the server's own.
// Own server, run written by hand (products-fixture.mjs), invented text only.
import {test, expect} from '@playwright/test';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeProductsRun, ROOT, OUTPUT_DIGEST} from './products-fixture.mjs';
import {fixtureRoot, put, quiet as quietAfter} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-page');
const E = 'work/example';
const NAME_IMG = '"><img src=x onerror=window.__pwned=1>.txt';
const NAME_MD = "'><svg onload=window.__pwned=2>.md";
const EV_ID = 'e0010-"><img src=x onerror=window.__pwned=3>';
let srv, base;

const LINKS = [
  ['a01', 'javascript:window.__pwned=21'], ['a02', 'JAVASCRIPT:window.__pwned=22'], ['a03', '  javascript:window.__pwned=23'], ['a04', 'java&#115;cript:window.__pwned=24'],
  ['a05', '&#106;avascript:window.__pwned=25'], ['a06', 'vbscript:msgbox(1)'], ['a07', 'data:text/html,<script>window.__pwned=26</script>'], ['a08', '//evil.invalid/x'],
  ['a09', '\\\\evil.invalid\\x'], ['a10', '/\\evil.invalid/x'], ['a11', '\\/evil.invalid/x'], ['a12', 'file:///etc/passwd'], ['a13', 'blob:https://evil.invalid/x'],
  ['a14', 'http:evil.invalid'], ['a15', '<javascript:window.__pwned=27>'], ['a16', 'javascript&colon;window.__pwned=28'], ['a17', 'jav%61script:window.__pwned=29'],
  ['a18', '%6a%61vascript:window.__pwned=30'], ['a19', ' javascript:window.__pwned=31'], ['a20', ' javascript:window.__pwned=32'], ['a21', 'javascript​:window.__pwned=33'],
  ['a23', 'java\u001bscript:window.__pwned=35'], ['a24', 'data:image/svg+xml,<svg onload=window.__pwned=36>'], ['a25', 'x "javascript:window.__pwned=37"'],
  ['a26', 'https://evil.invalid/"onmouseover="window.__pwned=38'], ['a27', 'work/../../../etc/passwd'], ['a28', '%2e%2e/%2e%2e/etc/passwd'], ['a29', 'work/.cache/token.txt'], ['a30', 'ok.txt?x=javascript:window.__pwned=39#y'],
];
const LINKS_MD = `# links\n\n${LINKS.map(([n, d]) => `[${n}](${d})`).join('\n\n')}\n\n![img](https://evil.invalid/pixel.png "title") ![](javascript:window.__pwned=40)\n\n[ref]: javascript:window.__pwned=41\n[use][ref] [ref]\n\n<https://evil.invalid/auto> <javascript:window.__pwned=42>\n`;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  fs.rmSync(root, {recursive: true, force: true});
  const dir = path.join(root, 'runs', 'r');
  writeProductsRun(dir, {product: 'newest'});
  const W = path.join(dir, 'work', 'example');
  put(path.join(W, NAME_IMG), '<img src=x onerror=window.__pwned=4> result: <b>valid</b>\n');
  put(path.join(W, NAME_MD), '# <img src=x onerror=window.__pwned=5>\n\n<script>window.__pwned = 6</script>\n');
  fs.utimesSync(path.join(W, NAME_MD), new Date('2030-01-01T00:00:00Z'), new Date('2030-01-01T00:00:00Z'));
  put(path.join(W, 'links.md'), LINKS_MD);
  put(path.join(W, 'ok.txt'), 'x\n');
  put(path.join(W, 'evil.json'), JSON.stringify({a: '<img src=x onerror=window.__pwned=7>', '<b>': ['</pre><script>window.__pwned=8</script>', {'a"b': "'"}], __proto__x: 1}));
  put(path.join(W, 'evil.yaml'), 'a: <img src=x onerror=window.__pwned=9>\nb: "</pre><script>window.__pwned=10</script>"\n');
  put(path.join(W, 'evil2.json'), '{"__proto__": {"x": "<img src=x onerror=window.__pwned=11>"}, "constructor": {"prototype": 1}}');
  // an evidence record whose file name, id, kind, result and nested fields look like markup
  put(path.join(dir, 'store', ROOT, 'evidence', `${EV_ID}.json`), JSON.stringify({
    format: 'canon-evidence/1', id: EV_ID, kind: '<script>window.__pwned=12</script>', result: '"><b onmouseover=window.__pwned=13>', subject: '<i>', producer: "p'q",
    observed_at: 'x', details: {'<img src=x onerror=window.__pwned=14>': ['</pre><script>window.__pwned=15</script>', {'a"b': "'", n: null, e: [], o: {}}]}, 'x" onclick="window.__pwned=16': 1,
  }));
  // the output of an action that looks like markup, with an escape sequence
  const hostile = '<img src=x onerror=window.__pwned=17>\n</pre><script>window.__pwned=18</script>\n\u001b[31mred\u001b[0m\n';
  fs.writeFileSync(path.join(dir, 'store', 'blobs', OUTPUT_DIGEST['notes.check']), hostile);
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', path.join(root, 'runs')]));
});

async function openPage(page) {
  const seen = {requests: [], dialogs: [], errors: []};
  page.on('request', (r) => { if (!r.url().startsWith(base)) seen.requests.push(r.url()); else if (/\/x(\?|$)|pixel|evil/.test(new URL(r.url()).pathname)) seen.requests.push(r.url()); });
  page.on('dialog', (d) => { seen.dialogs.push(d.message()); d.dismiss(); });
  page.on('pageerror', (e) => seen.errors.push(String(e)));
  await page.route((u) => !u.href.startsWith(base), (r) => r.abort());
  await page.goto(`${base}/?run=r`);
  await expect(page.locator('#gtitle .gt-id').first()).toBeVisible();
  const scripts = await page.locator('script').count();
  await page.locator('#tabs button[data-tab=products]').click();
  await expect(page.locator('#panel .prod')).toBeVisible();
  return {...seen, scripts};
}

// the audit and the checks after each step are shared with the daemon's page (security-products-lib.mjs)
const quiet = (page, seen, label) => quietAfter(expect, page, seen, label);

test('inv 4: file names, an evidence id and the output of an action that look like markup are text in the lists and the details', async ({page}) => {
  const seen = await openPage(page);
  await quiet(page, seen, 'lists');
  const names = await page.locator('#panel .prod-file .prod-fp').allTextContents();
  expect(names.some((n) => n.includes(NAME_IMG))).toBe(true);
  await page.locator('#panel .prod-file', {hasText: 'onerror=window.__pwned=1'}).click();
  await expect(page.locator('#panel .prod-detail[data-kind=file] pre')).toContainText('<img src=x onerror=window.__pwned=4>');
  await quiet(page, seen, 'file detail');
  await page.locator('#panel .prod-back').click();
  await page.locator('#panel .prod-ev', {hasText: 'onerror=window.__pwned=3'}).click();
  await expect(page.locator('#panel .prod-detail[data-kind=evidence]')).toBeVisible();
  await expect(page.locator('#panel .prod-detail[data-kind=evidence]')).toContainText('<script>window.__pwned=12</script>');
  await quiet(page, seen, 'evidence detail');
  await page.locator('#panel .prod-back').click();
  await page.locator('#panel .prod-act', {hasText: 'notes.check'}).click();
  await expect(page.locator('#panel .prod-detail[data-kind=action] .prod-text')).toContainText('<img src=x onerror=window.__pwned=17>');
  await quiet(page, seen, 'action detail');
  await page.locator('#panel .prod-back').click();
  await quiet(page, seen, 'back to the list');
});

test('inv 4: the product line and a Markdown file named like markup are text; a heading and a script element in it are text', async ({page}) => {
  const seen = await openPage(page);
  await expect(page.locator('#product .prod-link').first()).toContainText('svg onload=window.__pwned=2');
  await quiet(page, seen, 'product line');
  await page.locator('#product .prod-link').first().click();
  await expect(page.locator('#panel .prod-detail[data-kind=file] .md')).toBeVisible();
  await expect(page.locator('#panel .md')).toContainText('<img src=x onerror=window.__pwned=5>');
  await expect(page.locator('#panel .md')).toContainText('<script>window.__pwned = 6</script>');
  await quiet(page, seen, 'markdown detail');
});

test('inv 4: JSON and YAML that look like markup are shown as text', async ({page}) => {
  const seen = await openPage(page);
  for (const [name, text] of [['evil.json', '<img src=x onerror=window.__pwned=7>'], ['evil.yaml', '<img src=x onerror=window.__pwned=9>'], ['evil2.json', 'window.__pwned=11']]) {
    await page.locator(`#panel .prod-file[data-path="${E}/${name}"]`).click();
    await expect(page.locator('#panel .prod-detail[data-kind=file] pre')).toContainText(text);
    await quiet(page, seen, name);
    await page.locator('#panel .prod-back').click();
  }
});

test('inv 4: no link of the matrix of schemes, spellings and control characters is an anchor that is not http, https or the page\'s own #file=; images are not loaded', async ({page}) => {
  const seen = await openPage(page);
  await page.locator(`#panel .prod-file[data-path="${E}/links.md"]`).click();
  await expect(page.locator('#panel .prod-detail[data-kind=file] .md')).toBeVisible();
  await quiet(page, seen, 'links.md');
  const anchors = await page.locator('#panel .md a').evaluateAll((as) => as.map((a) => ({text: a.textContent, href: a.getAttribute('href')})));
  const byText = Object.fromEntries(anchors.map((a) => [a.text, a.href]));
  // the labels that must be anchors, and only these: the two schemes and the link inside the run
  for (const label of ['a14', 'a26', 'a30', 'a17', 'a18']) expect(byText[label] ?? null, label).not.toBeNull();
  for (const label of ['a01', 'a02', 'a03', 'a06', 'a07', 'a08', 'a09', 'a10', 'a11', 'a12', 'a13', 'a15', 'a19', 'a20', 'a23', 'a24', 'a29']) expect(byText[label] ?? null, `${label} is not a link`).toBeNull();
  expect(await page.locator('#panel .md img').count()).toBe(0);
  expect(seen.requests).toEqual([]);
});

test('inv 4: linkOf, the one decision of what a link may be, gives none, an http(s) address or a path inside the run for every spelling, control characters and NUL included', async ({page}) => {
  await page.goto(base);
  const dests = [...LINKS.map(([, d]) => d), 'java\u0000script:x', '\u0001javascript:x', 'jav\tascript:x', 'jav\nascript:x', 'jav\rascript:x', '\u000bjavascript:x', '\u000cjavascript:x', ' \t\n javascript:x', 'javascript :x', 'javascript :x', 'JaVa\u0009ScRiPt:x', 'https://a\u0000b', 'hTTp://x', 'http://', 'https:///x', 'http:\\\\evil.invalid', 'https:/\\evil.invalid', 'work/\u0000x', '..', '.', '', ' ', 'a'.repeat(100000), `${'../'.repeat(50000)}x`, `${'a/'.repeat(50000)}x`];
  const bad = await page.evaluate(async (list) => {
    const {linkOf} = await import('/markdown.js');
    const out = [];
    for (const d of list) {
      let r;
      try { r = linkOf(d, 'work/example'); } catch (e) { out.push(`throws ${JSON.stringify(d).slice(0, 40)}: ${e}`); continue; }
      if (r.kind === 'external') {
        let u = null;
        try { u = new URL(r.href); } catch { out.push(`external href that does not parse: ${JSON.stringify(r.href).slice(0, 60)}`); continue; }
        if (u.protocol !== 'http:' && u.protocol !== 'https:') out.push(`external ${u.protocol} ${JSON.stringify(d).slice(0, 40)}`);
        if (/[\u0000- \u007f]/.test(r.href)) out.push(`external href with a control character or a space ${JSON.stringify(r.href).slice(0, 60)}`);
      } else if (r.kind === 'file') {
        if (r.path.startsWith('/') || r.path.split('/').some((n) => n === '..' || n === '' || n.startsWith('.')) || /[\u0000-\u001f\u007f\\]/.test(r.path)) out.push(`file path ${JSON.stringify(r.path).slice(0, 60)} for ${JSON.stringify(d).slice(0, 40)}`);
      } else if (r.kind !== 'none') out.push(`kind ${r.kind}`);
    }
    return out;
  }, dests);
  expect(bad).toEqual([]);
});
