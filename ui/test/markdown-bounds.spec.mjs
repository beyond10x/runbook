// The cost of rendering a hostile Markdown file is bounded (story inspector-shows-step-products, fix round 1 of the
// security review): a line of whitespace at the size the page still renders (131072 characters) in each place the
// renderer reads whitespace is rendered in under 1 s, and a document, a table in it, makes a bounded number of
// nodes: past the bound the rest is shown as plain text with a line that says so. Each case runs in its own page,
// in Chromium, with the renderer the page loads. Invented text only.
import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {startServer} from './serve.mjs';
import {writeProductsRun} from './products-fixture.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(process.env.SDC_UI_FIXTURE_DIR || path.join(here, 'fixtures', 'runs'), 'markdown-bounds');
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
    return {ms: Math.round(performance.now() - t0), error, nodes: host.querySelectorAll('*').length, chars: md.length, text: host.textContent.length, notes: Array.from(host.querySelectorAll('.md-cut')).map((n) => n.textContent)};
  }, {src: build.toString()});
}

const N = 131000;
const shapes = {
  heading: () => `# a${' '.repeat(131000)}b`,
  headingTrailing: () => `# a b${' '.repeat(131000)}`,
  headingClosing: () => `# a${' '.repeat(65000)}#${' '.repeat(65000)}`,
  trailingWhitespace: () => `a${' '.repeat(131000)}\nb`,
  trailingWhitespaceTabs: () => `a${'\t '.repeat(65000)}\nb`,
  hardBreak: () => `a${' '.repeat(131000)}\nb`,
  fenceInfo: () => `\`\`\`${' '.repeat(131000)}\``,
  separator: () => `a | b\n${' '.repeat(131000)}x`,
  separatorPipes: () => `a | b\n${' |'.repeat(65000)}`,
  rule: () => `-${' '.repeat(131000)}x`,
  listMarker: () => `-${' '.repeat(131000)}`,
};
for (const [name, build] of Object.entries(shapes)) {
  test(`a ${name} line of ${N} characters is rendered in under 1 s`, async ({page}) => {
    test.setTimeout(30000);
    const r = await render(page, build);
    expect(r.error).toBeNull();
    expect(r.chars).toBeLessThanOrEqual(131072);
    expect(r.ms, `${name}: ${r.chars} characters: ${r.ms} ms`).toBeLessThan(1000);
  });
}

test('a table has at most 50 columns, 500 rows and 5000 cells: the rest is plain text with a line that says so', async ({page}) => {
  test.setTimeout(30000);
  const wide = await render(page, () => `|${'a|'.repeat(120)}\n|${'-|'.repeat(120)}\n|${'b|'.repeat(120)}`);
  expect(wide.nodes).toBeLessThan(400);
  expect(wide.notes.join(' ')).toMatch(/table/);
  const tall = await render(page, () => `|a|b|\n|-|-|\n${'|1|2|\n'.repeat(3000)}`);
  expect(tall.nodes, `${tall.nodes} nodes`).toBeLessThan(2500);
  expect(tall.notes.join(' ')).toMatch(/table/);
  expect(tall.text).toBeGreaterThan(3000 * 4);
  const small = await render(page, () => '|a|b|\n|-|-|\n|1|2|\n');
  expect(small.notes).toEqual([]);
});

test('a document makes a bounded number of nodes: 6 KB of table rows and 130 KB of emphasis stay under the bound, the rest is text', async ({page}) => {
  test.setTimeout(30000);
  const a = await render(page, () => `|${'a|'.repeat(500)}\n|${'-|'.repeat(500)}\n${'|\n'.repeat(2000)}`);
  expect(a.nodes, `${a.chars} characters made ${a.nodes} nodes`).toBeLessThan(20 * a.chars);
  const b = await render(page, () => '*a* '.repeat(32000));
  expect(b.nodes, `${b.chars} characters made ${b.nodes} nodes`).toBeLessThanOrEqual(61000);
  expect(b.text).toBeGreaterThan(b.chars / 2);
  const c = await render(page, () => Array.from({length: 40000}, (_, i) => `# h${i}`).join('\n'));
  expect(c.nodes, `${c.chars} characters made ${c.nodes} nodes`).toBeLessThanOrEqual(61000);
  expect(c.notes.join(' ')).toMatch(/plain text/);
  expect(c.ms, `${c.ms} ms`).toBeLessThan(5000);
});
