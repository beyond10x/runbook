// Independent review, invariant 4 of story inspector-shows-step-products: "The renderer's cost is bounded on hostile
// input (nested emphasis, long runs of backticks, brackets, deep lists, a 128 KiB single line): no spec may need over
// 5 s for one file." The page shows a Markdown file up to 131072 characters (web/products.js MARKDOWN_LIMIT) through
// web/markdown.js. Each case runs in its own page, in Chromium, with the renderer the page itself loads, and measures
// the time of renderMarkdown and the number of nodes it made. Invented text only.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {startServer} from './serve.mjs';
import {writeProductsRun} from './products-fixture.mjs';
import {fixtureRoot} from './security-products-lib.mjs';

const root = fixtureRoot('security-products-markdown-cost');
let srv, base;
test.beforeEach(({}, info) => test.skip(info.project.name !== 'wide', 'own server, one viewport'));
test.afterAll(() => srv?.kill());
test.beforeAll(async ({}, info) => {
  if (info.project.name !== 'wide') return;
  fs.rmSync(root, {recursive: true, force: true});
  writeProductsRun(path.join(root, 'runs', 'r'));
  ({url: base, proc: srv} = await startServer(['--no-view', '--runs', path.join(root, 'runs')]));
});

async function render(page, build) {
  await page.goto(base);
  return page.evaluate(async ({src}) => {
    const {renderMarkdown} = await import('/markdown.js');
    // eslint-disable-next-line no-new-func
    const md = new Function(`return (${src})()`)();
    const host = document.createElement('div');
    const t0 = performance.now();
    let error = null;
    try { renderMarkdown(md, {dir: 'work', open: () => {}}, host); } catch (e) { error = String(e); }
    return {ms: Math.round(performance.now() - t0), error, nodes: host.querySelectorAll('*').length, chars: md.length};
  }, {src: build.toString()});
}

const LIMIT = 131072;

test('inv 4: a line of 131000 spaces inside a paragraph is rendered in at most 5 s', async ({page}) => {
  test.setTimeout(60000);
  const r = await render(page, () => `a${' '.repeat(131000)}b`);
  expect(r.error).toBeNull();
  expect(r.ms, `${r.chars} characters: ${r.ms} ms`).toBeLessThan(5000);
});

test('inv 4: a heading followed by 131000 spaces and a word is rendered in at most 5 s', async ({page}) => {
  test.setTimeout(60000);
  const r = await render(page, () => `# a${' '.repeat(131000)}b`);
  expect(r.error).toBeNull();
  expect(r.ms, `${r.chars} characters: ${r.ms} ms`).toBeLessThan(5000);
});

test('inv 4: two lines, the first with 131000 tabs inside, are rendered in at most 5 s', async ({page}) => {
  test.setTimeout(60000);
  const r = await render(page, () => `a${'\t'.repeat(65000)}b\nc`);
  expect(r.error).toBeNull();
  expect(r.ms, `${r.chars} characters: ${r.ms} ms`).toBeLessThan(5000);
});

test('inv 4: a table of 500 columns and 2000 rows of one pipe each (about 6 KB) does not make a million nodes', async ({page}) => {
  test.setTimeout(60000);
  const r = await render(page, () => {
    const cols = 500, rows = 2000;
    const head = `|${'a|'.repeat(cols)}`, sep = `|${'-|'.repeat(cols)}`;
    return [head, sep, ...Array.from({length: rows}, () => '|')].join('\n');
  });
  expect(r.error).toBeNull();
  expect(r.nodes, `${r.chars} characters of input made ${r.nodes} nodes in ${r.ms} ms`).toBeLessThan(20 * r.chars);
});

// one test per shape: each is rendered in its own page and no test is near the bound of one test
const shapes = {
    nestedEmphasis: () => `${'*a '.repeat(20000)}${'a* '.repeat(20000)}`,
    backtickRuns: () => Array.from({length: 400}, (_, i) => `${'`'.repeat(i % 60 + 1)}[`).join('').repeat(8),
    bracketsInBackticks: () => `[${'`a`'.repeat(30000)}]`,
    deepList: () => Array.from({length: 2500}, (_, i) => `${" ".repeat(2 * (i % 30))}- item *a* [b](c)`).join('\n'),
    longLine: () => 'word '.repeat(26000),
    longLink: () => `[${'a'.repeat(60000)}](${'b'.repeat(60000)})`,
    manyLinks: () => '[a](b) '.repeat(18000),
    manyCodeSpans: () => '`a` '.repeat(30000),
    manyEntitiesLike: () => '&amp; &lt;script&gt; '.repeat(6000),
    manyHeadings: () => '# h\n'.repeat(30000),
    manyFences: () => '```\n'.repeat(30000),
    manyQuotes: () => '> a\n'.repeat(30000),
};
for (const [name, build] of Object.entries(shapes)) {
  test(`inv 4: the shape ${name} stays under 5 s`, async ({page}) => {
    test.setTimeout(30000);
    const r = await render(page, build);
    expect(r.chars, name).toBeLessThanOrEqual(LIMIT);
    expect(r.error, name).toBeNull();
    expect(r.ms, `${name}: ${r.chars} characters: ${r.ms} ms`).toBeLessThan(5000);
  });
}
