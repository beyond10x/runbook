// The content of a run never becomes markup or code of the page (story inspector-shows-step-products, safety).
// A run directory is written by a confined model, so a Markdown file in it is hostile input: a script element,
// elements with event handlers, links with a script scheme, an image that would load something. The page's
// renderer (web/markdown.js) builds nodes from text; this spec opens such a file in the page and looks at the
// DOM, the requests and the window, and reads the sources of the two modules that show run content.
// Own server on a port the kernel gives (test/serve.mjs); the run is written by hand (test/products-fixture.mjs).
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {writeProductsRun, HOSTILE_MD} from './products-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'markdown-hostile');
let srv, base;

test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  writeProductsRun(path.join(root, 'runs', 'hostile'));
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', path.join(root, 'runs')]));
});

async function openHostile(page) {
  const requests = [], dialogs = [], errors = [];
  page.on('request', (r) => requests.push(r.url()));
  page.on('dialog', (d) => { dialogs.push(d.message()); d.dismiss(); });
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.route('**/*example.invalid/**', (r) => r.abort());
  await page.goto(`${base}/?run=hostile`);
  await expect(page.locator('#gtitle .gt-id').first()).toBeVisible();
  const scripts = await page.locator('script').count();
  await expect(page.locator('#tabs button[data-tab=products]')).toBeVisible();
  await page.locator('#tabs button[data-tab=products]').click();
  await page.locator('#panel .prod-file[data-path="work/example/hostile.md"]').click();
  await expect(page.locator('#panel .prod-detail[data-kind=file] .md')).toBeVisible();
  return {requests, dialogs, errors, scripts};
}

test('a hostile Markdown file shows no script node, no handler attribute and no script link; raw html is text', async ({page}) => {
  const seen = await openHostile(page);
  const md = page.locator('#panel .md');
  const bad = await md.evaluate((root) => {
    const out = {nodes: [], attrs: [], links: []};
    for (const el of root.querySelectorAll('*')) {
      if (/^(script|iframe|object|embed|img|style|link|form|input|svg|video|audio|base|meta|source|picture|math|template|noscript)$/i.test(el.tagName)) out.nodes.push(el.tagName);
      for (const a of el.attributes) if (/^on/i.test(a.name) || /^(src|srcset|style|srcdoc|formaction|action|xlink:href|background|poster)$/i.test(a.name)) out.attrs.push(`${el.tagName}.${a.name}`);
      if (el.tagName === 'A') out.links.push({href: el.getAttribute('href'), target: el.getAttribute('target'), rel: el.getAttribute('rel'), text: el.textContent});
    }
    return out;
  });
  expect(bad.nodes).toEqual([]);
  expect(bad.attrs).toEqual([]);
  for (const l of bad.links) {
    const href = l.href;
    expect(href === null || /^https?:\/\//.test(href) || /^#file=/.test(href), JSON.stringify(l)).toBe(true);
    expect(href || '', JSON.stringify(l)).not.toMatch(/javascript|data:|mailto|^\/\//i);
    if (href && /^https?:/.test(href)) { expect(l.target).toBe('_blank'); expect(l.rel).toBe('noopener noreferrer'); }
  }
  // the raw html is text, byte for byte
  const text = await md.innerText();
  expect(text).toContain('<script>window.__pwned = 1</script>');
  expect(text).toContain('<img src="https://example.invalid/pixel.png" onerror="window.__pwned = 2">');
  expect(text).toContain('<div onclick="window.__pwned = 3">a block of raw html</div>');
  expect(text).toContain('<b onmouseover=window.__pwned=10>');
  // a link that is not followed is shown as text with its address, so nothing is hidden
  expect(text).toContain('javascript:window.__pwned=4');
  expect(text).toContain('data:text/html;base64,PHNjcmlwdD4=');
  // the page has the scripts it had, nothing ran, nothing was asked of the network address
  expect(await page.locator('script').count()).toBe(seen.scripts);
  expect(await page.evaluate(() => window.__pwned)).toBeUndefined();
  expect(seen.dialogs).toEqual([]);
  expect(seen.errors).toEqual([]);
  expect(seen.requests.filter((u) => /example\.invalid/.test(u))).toEqual([]);
  expect(await page.locator('[onerror],[onclick],[onmouseover],[onload]').count()).toBe(0);
  expect(await page.locator('a[href^="javascript" i], a[href^="data:" i]').count()).toBe(0);
});

test('an image is a link with its alt text and is not loaded; an external link opens in a new tab without the opener; a link inside the run opens its file in the same panel', async ({page}) => {
  const seen = await openHostile(page);
  const md = page.locator('#panel .md');
  expect(await md.locator('img').count()).toBe(0);
  const alt = md.locator('a', {hasText: 'the alt text'});
  await expect(alt).toHaveCount(1);
  await expect(alt).toHaveAttribute('href', 'https://example.invalid/pic.png');
  await expect(alt).toHaveAttribute('rel', 'noopener noreferrer');
  const ext = md.locator('a', {hasText: 'good external'});
  await expect(ext).toHaveAttribute('href', 'https://example.com/page');
  await expect(ext).toHaveAttribute('target', '_blank');
  await expect(ext).toHaveAttribute('rel', 'noopener noreferrer');
  // a path that climbs above the run, a rooted path and a protocol-relative address are not links
  for (const label of ['up', 'proto', 'abs', 'js', 'tab', 'data', 'mail', 'upper', 'spaced', 'bad']) {
    const a = md.locator('a', {hasText: new RegExp(`^${label}$`)});
    await expect(a, label).toHaveCount(0);
  }
  const rel = md.locator('a', {hasText: 'relative'});
  await expect(rel).toHaveAttribute('href', '#file=work%2Fexample%2Fdata.json');
  await expect(rel).not.toHaveAttribute('target', /.+/);
  await rel.click();
  await expect(page.locator('#panel .prod-detail[data-kind=file] .prod-path')).toHaveText('work/example/data.json');
  expect(seen.requests.filter((u) => /example\.invalid|example\.com/.test(u))).toEqual([]);
  expect(await page.evaluate(() => window.__pwned)).toBeUndefined();
});

test('the renderer builds nodes from text: its source never assigns markup, and the hostile text through it gives text nodes only', async ({page}) => {
  for (const f of ['web/markdown.js', 'web/products.js']) {
    const src = fs.readFileSync(path.join(ui, f), 'utf8');
    for (const word of ['innerHTML', 'outerHTML', 'insertAdjacentHTML', 'document.write', 'createContextualFragment', 'DOMParser', 'eval(', 'new Function', 'srcdoc', 'setAttribute(\'on', 'setAttribute("on']) expect(src.includes(word), `${f}: ${word}`).toBe(false);
    expect(src, f).not.toMatch(/https?:\/\/[a-z0-9.-]+\.[a-z]{2,}/i);
    expect(src, f).not.toMatch(/\bimport\s*\(\s*['"]https?:/);
  }
  await page.goto(base);
  const r = await page.evaluate(async (md) => {
    const {renderMarkdown} = await import('/markdown.js');
    const host = document.createElement('div');
    renderMarkdown(md, {dir: 'work/example', open: () => {}}, host);
    const tags = new Set([...host.querySelectorAll('*')].map((e) => e.tagName.toLowerCase()));
    return {tags: [...tags].sort(), html: host.innerHTML};
  }, HOSTILE_MD);
  expect(r.tags.every((t) => ['h1', 'p', 'a', 'pre', 'code', 'br'].includes(t)), r.tags.join(',')).toBe(true);
  expect(r.html).not.toMatch(/<script|<img|<div|<b /i);
  expect(r.html).toContain('&lt;script&gt;');
});

test('a file made to keep the renderer busy (a hundred thousand brackets, stars, backticks, quotes and lists nested deep) is rendered within a few seconds and does not throw', async ({page}) => {
  await page.goto(base);
  const r = await page.evaluate(async () => {
    const {renderMarkdown} = await import('/markdown.js');
    const hostile = {
      brackets: '['.repeat(100000),
      links: '[a](b '.repeat(20000),
      stars: '*a '.repeat(40000),
      ticks: '`a``b```'.repeat(15000),
      quotes: '> '.repeat(20000) + 'x',
      lists: Array.from({length: 3000}, (_, i) => `${' '.repeat(i % 40)}- item`).join('\n'),
    };
    const out = {};
    for (const [name, md] of Object.entries(hostile)) {
      const host = document.createElement('div');
      const t0 = performance.now();
      let error = null;
      try { renderMarkdown(md, {dir: 'work', open: () => {}}, host); } catch (e) { error = String(e); }
      out[name] = {ms: Math.round(performance.now() - t0), error, nodes: host.querySelectorAll('*').length};
    }
    return out;
  });
  for (const [name, x] of Object.entries(r)) {
    expect(x.error, name).toBeNull();
    expect(x.ms, `${name}: ${x.ms} ms`).toBeLessThan(5000);
  }
});
