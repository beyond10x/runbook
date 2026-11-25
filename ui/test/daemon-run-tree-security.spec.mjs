// Invariant 4 of the inspector's Products tab on the daemon's page (story daemon-page-shows-a-run-tree-and-products):
// the content of a run never becomes markup or code of the page. The hostile cases of security-products-page.spec.mjs
// that the daemon serves (file names, Markdown, JSON and YAML written to look like markup or like a script, the matrix
// of link spellings) are written into the work/ directory of a run of a Loom daemon (daemon-tree-fixture.mjs) and
// opened in the daemon's page; after each the DOM of the panel, the product line, the tree and the feed is audited
// (security-products-lib.mjs `audit`), nothing ran, and nothing was asked of an address that is not the daemon's own.
// Invented text only. The daemon serves no evidence record and no action output, so those cases have no equal here.
import {test, expect as plain} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {ui, bin, startDaemon, submit, review, workspace} from './daemon-tree-fixture.mjs';
import {put, quiet as quietAfter} from './security-products-lib.mjs';

const expect = plain.configure({timeout: 15000});
const NAME_IMG = '"><img src=x onerror=window.__pwned=1>.txt';
const NAME_MD = "'><svg onload=window.__pwned=2>.md";
const DIR_HOSTILE = '<b onmouseover=window.__pwned=3>';
const LINKS = [
  ['a01', 'javascript:window.__pwned=21'], ['a02', 'JAVASCRIPT:window.__pwned=22'], ['a06', 'vbscript:msgbox(1)'], ['a07', 'data:text/html,<script>window.__pwned=26</script>'],
  ['a08', '//evil.invalid/x'], ['a12', 'file:///etc/passwd'], ['a14', 'http:evil.invalid'], ['a24', 'data:image/svg+xml,<svg onload=window.__pwned=36>'],
  ['a27', '../../../etc/passwd'], ['a29', '.cache/token.txt'], ['a31', 'ok.txt'],
];
const LINKS_MD = `# links\n\n${LINKS.map(([n, d]) => `[${n}](${d})`).join('\n\n')}\n\n![img](https://evil.invalid/pixel.png "title") ![](javascript:window.__pwned=40)\n\n<https://evil.invalid/auto> <javascript:window.__pwned=42>\n`;
// the panel, the product line, the tree, the feed and the case header: the places the daemon's page shows what a run holds
const SCOPE = '#panel *, #product *, #tree *, #feed *, #gtitle *';
const STYLES = '^(--depth:\\d+|padding-left:\\d+px|left:\\d+px)$';
let d, dir, run;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own daemon, one viewport'));
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  test.setTimeout(60000);
  expect(fs.existsSync(bin), `${bin} is missing: build it with cargo build --bin runbook in runbook/ (or set SDC_RUNBOOK_BIN)`).toBe(true);
  dir = fs.mkdtempSync(path.join(ui, 'node_modules', '.pw-tree-sec-'));
  d = await startDaemon(dir);
  run = await submit(d, '<img src=x onerror=window.__pwned=50> an intake that looks like markup');
  await expect.poll(() => review(d, run).then((x) => x?.request_id ?? null), {timeout: 30000}).not.toBeNull();
  const w = workspace(d, run);
  put(path.join(w, NAME_IMG), '<img src=x onerror=window.__pwned=4> result: <b>valid</b>\n');
  put(path.join(w, NAME_MD), '# <img src=x onerror=window.__pwned=5>\n\n<script>window.__pwned = 6</script>\n');
  put(path.join(w, DIR_HOSTILE, 'inner.txt'), '</pre><script>window.__pwned=7</script>\n');
  put(path.join(w, 'links.md'), LINKS_MD);
  put(path.join(w, 'ok.txt'), 'x\n');
  put(path.join(w, 'evil.json'), JSON.stringify({a: '<img src=x onerror=window.__pwned=8>', '<b>': ['</pre><script>window.__pwned=9</script>', {'a"b': "'"}]}));
  put(path.join(w, 'evil.yaml'), 'a: <img src=x onerror=window.__pwned=10>\nb: "</pre><script>window.__pwned=11</script>"\n');
  put(path.join(w, '.cache', 'token.txt'), 'secret-token-4711\n');
  fs.symlinkSync('/etc/passwd', path.join(w, 'passwd-link.txt'));
});
test.afterAll(() => {
  if (d?.proc && d.proc.exitCode == null) d.proc.kill();
  if (dir) fs.rmSync(dir, {recursive: true, force: true});
});

async function openPage(page) {
  const seen = {requests: [], dialogs: [], errors: []};
  page.on('request', (r) => { if (!r.url().startsWith(d.base)) seen.requests.push(r.url()); else if (/\/x(\?|$)|pixel|evil|passwd/.test(new URL(r.url()).pathname)) seen.requests.push(r.url()); });
  page.on('dialog', (x) => { seen.dialogs.push(x.message()); x.dismiss(); });
  page.on('pageerror', (e) => seen.errors.push(String(e)));
  await page.route((u) => !u.href.startsWith(d.base), (r) => r.abort());
  await page.goto(`${d.base}/?run=${run}`);
  await expect(page.locator('#tree .case')).toHaveCount(3);
  const scripts = await page.locator('script').count();
  await page.locator('#tabs button[data-tab=products]').click();
  await expect(page.locator('#panel .prod-file').first()).toBeVisible();
  return {...seen, scripts};
}
const quiet = (page, seen, label) => quietAfter(expect, page, seen, label, SCOPE, STYLES);
const open = async (page, p) => { await page.locator('#panel .prod-file').filter({has: page.locator('.prod-fp', {hasText: p})}).first().click(); await expect(page.locator('#panel .prod-detail[data-kind=file]')).toBeVisible(); };
const back = (page) => page.locator('#panel .prod-back').click();

test('inv 4 on the daemon page: file and directory names that look like markup are text in the list and the details; the tree and the feed stay text', async ({page}) => {
  const seen = await openPage(page);
  await quiet(page, seen, 'lists');
  const names = await page.locator('#panel .prod-file .prod-fp').allTextContents();
  for (const n of [NAME_IMG, NAME_MD, `${DIR_HOSTILE}/inner.txt`]) expect(names, n).toContain(n);
  // the hidden file and the link are neither listed nor served
  expect(names.some((n) => n.includes('token') || n.includes('passwd'))).toBe(false);
  await expect(page.locator('#panel .prod-hidden')).toContainText('1 entry whose name starts with a dot is not listed');
  await open(page, NAME_IMG);
  await expect(page.locator('#panel .prod-detail[data-kind=file] pre')).toContainText('<img src=x onerror=window.__pwned=4>');
  await expect(page.locator('#panel .prod-path')).toHaveText(NAME_IMG);
  await quiet(page, seen, 'file detail');
  await back(page);
  await open(page, `${DIR_HOSTILE}/inner.txt`);
  await expect(page.locator('#panel .prod-detail[data-kind=file] pre')).toContainText('</pre><script>window.__pwned=7</script>');
  await quiet(page, seen, 'file in a directory named like markup');
  await back(page);
  await expect(page.locator('#feed')).toContainText('runbook.submitted');
  await quiet(page, seen, 'back to the list');
});

test('inv 4 on the daemon page: a Markdown file named like markup renders a heading and a script element as text', async ({page}) => {
  const seen = await openPage(page);
  await open(page, NAME_MD);
  await expect(page.locator('#panel .md')).toContainText('<img src=x onerror=window.__pwned=5>');
  await expect(page.locator('#panel .md')).toContainText('<script>window.__pwned = 6</script>');
  await quiet(page, seen, 'markdown detail');
});

test('inv 4 on the daemon page: JSON and YAML that look like markup are shown as text', async ({page}) => {
  const seen = await openPage(page);
  for (const [name, text] of [['evil.json', '<img src=x onerror=window.__pwned=8>'], ['evil.yaml', '<img src=x onerror=window.__pwned=10>']]) {
    await page.locator(`#panel .prod-file[data-path="${name}"]`).click();
    await expect(page.locator('#panel .prod-detail[data-kind=file] pre')).toContainText(text);
    await quiet(page, seen, name);
    await back(page);
  }
});

test('inv 4 on the daemon page: no link of a Markdown file is an anchor that is not http, https or a file of the run; images are not loaded; a link to a file opens it through the daemon', async ({page}) => {
  const seen = await openPage(page);
  await page.locator('#panel .prod-file[data-path="links.md"]').click();
  await expect(page.locator('#panel .prod-detail[data-kind=file] .md')).toBeVisible();
  await quiet(page, seen, 'links.md');
  const anchors = await page.locator('#panel .md a').evaluateAll((as) => Object.fromEntries(as.map((a) => [a.textContent, a.getAttribute('href')])));
  for (const label of ['a01', 'a02', 'a06', 'a07', 'a08', 'a12', 'a24', 'a27', 'a29']) expect(anchors[label] ?? null, `${label} is not a link`).toBeNull();
  expect(anchors.a31).toBe('#file=ok.txt');
  expect(await page.locator('#panel .md img').count()).toBe(0);
  await page.locator('#panel .md a', {hasText: 'a31'}).click();
  await expect(page.locator('#panel .prod-path')).toHaveText('ok.txt');
  await expect(page.locator('#panel .prod-detail[data-kind=file] pre')).toHaveText('x\n');
  await quiet(page, seen, 'a link inside the run');
  expect(seen.requests).toEqual([]);
});

// The adversary's case: a tree whose case ids, link names, protocols and outcome look like markup (a runtime that
// answers what a hostile runbook named) is drawn as text in the tree and the case header.
test('inv 4 on the daemon page: case names, ids, protocols and an outcome that look like markup are text in the tree and the header', async ({page}) => {
  const hostile = [
    {case_id: 'r"><img src=x onerror=window.__pwned=60>', protocol: '<svg onload=window.__pwned=61>', case_state: 'Open'},
    {case_id: 'k', parent: 'r"><img src=x onerror=window.__pwned=60>', name: '<b onmouseover=window.__pwned=62>kid</b>', protocol: 'p" style="x', case_state: 'Ended', outcome: '<script>window.__pwned=63</script>'},
  ];
  const seen = {requests: [], dialogs: [], errors: []};
  page.on('dialog', (x) => { seen.dialogs.push(x.message()); x.dismiss(); });
  page.on('pageerror', (e) => seen.errors.push(String(e)));
  page.on('request', (r) => { if (!r.url().startsWith(d.base)) seen.requests.push(r.url()); });
  await page.route((u) => !u.href.startsWith(d.base), (r) => r.abort());
  await page.route((u) => u.pathname === '/daemon/views/cases', (q) => q.fulfill({status: 200, contentType: 'application/json', body: JSON.stringify({rows: hostile})}));
  await page.goto(`${d.base}/?run=${run}`);
  await expect(page.locator('#tree .case')).toHaveCount(2);
  seen.scripts = await page.locator('script').count();
  await expect(page.locator('#tree .case .cid')).toHaveText([hostile[0].case_id, hostile[1].name]);
  await page.locator('#tree .case').nth(1).click();
  await expect(page.locator('#gtitle .gt-id')).toHaveText(hostile[1].name);
  await expect(page.locator('#tree .case').nth(1).locator('.cst')).toHaveAttribute('data-v', `ended ${hostile[1].outcome}`);
  await quiet(page, seen, 'hostile tree');
});
